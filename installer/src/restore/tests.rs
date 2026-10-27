//! Planning a restore is a function of two ledgers and "is this payload cached",
//! so these run off Windows; they are written around the two ways a restore
//! can do harm: removing what it cannot replace, and putting files back
//! without the record of what they replaced.

const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
const EWA: &str = "rbr/public/codriver/polish/female/Ewa";
const POLISH: &str = "rbr/public/language/polish";

/// A finished entry owning one file, which replaced nothing.
fn entry(sha: &str, version: &str, file: &str) -> crate::ledger::LedgerEntry {
    entry_replacing(sha, version, file, crate::ledger::Prior::Absent)
}

/// A finished entry owning one file, with what was there before it.
fn entry_replacing(
    sha: &str,
    version: &str,
    file: &str,
    prior: crate::ledger::Prior,
) -> crate::ledger::LedgerEntry {
    crate::ledger::LedgerEntry {
        sha256: sha.to_string(),
        version: version.to_string(),
        exclusive_group: None,
        state: crate::ledger::EntryState::Installed,
        files: [(
            file.to_lowercase(),
            crate::ledger::LedgerFile {
                path: file.to_string(),
                sha256: format!("content-of-{sha}"),
                prior,
            },
        )]
        .into_iter()
        .collect(),
    }
}

fn snapshot(entries: &[(&str, crate::ledger::LedgerEntry)]) -> super::Snapshot {
    entries
        .iter()
        .map(|(key, entry)| (key.to_string(), entry.clone()))
        .collect()
}

/// Everything is in the cache.
fn all_present(_: &str) -> bool {
    true
}

/// THE case: a run updated a package and the new one is bad. Going back
/// means the OLD payload, which the cache index no longer points at — it
/// holds one version per key — so the plan has to name it by digest.
#[test]
fn a_bad_update_is_rolled_back_to_the_payload_the_snapshot_names() {
    let target = snapshot(&[(KAMIL, entry("sha-good", "2.1.0", "sounds/k.ogg"))]);
    let live = snapshot(&[(KAMIL, entry("sha-bad", "3.0.0", "sounds/k.ogg"))]);

    let plan = super::plan(&target, &live, &all_present);

    assert_eq!(plan.remove, vec![KAMIL.to_string()]);
    assert_eq!(
        plan.put_back,
        vec![super::PutBack {
            key: KAMIL.into(),
            sha256: "sha-good".into(),
            version: "2.1.0".into(),
        }],
        "the OLD digest, not whatever the cache index holds today"
    );
    assert!(plan.unavailable.is_empty());
    assert!(!plan.is_empty());
}

/// A package installed after the point goes away entirely.
#[test]
fn something_installed_since_the_point_is_removed() {
    let target = snapshot(&[(KAMIL, entry("k1", "1.0", "sounds/k.ogg"))]);
    let live = snapshot(&[
        (KAMIL, entry("k1", "1.0", "sounds/k.ogg")),
        (EWA, entry("e1", "1.0", "sounds/e.ogg")),
    ]);

    let plan = super::plan(&target, &live, &all_present);

    assert_eq!(plan.remove, vec![EWA.to_string()]);
    assert_eq!(plan.keep, vec![KAMIL.to_string()]);
    assert!(plan.put_back.is_empty());
}

/// A package removed after the point comes back.
#[test]
fn something_uninstalled_since_the_point_is_put_back() {
    let target = snapshot(&[(POLISH, entry("p1", "1.0", "language/p.ini"))]);
    let live = super::Snapshot::new();

    let plan = super::plan(&target, &live, &all_present);

    assert!(plan.remove.is_empty());
    assert_eq!(plan.put_back.len(), 1);
    assert_eq!(plan.put_back[0].key, POLISH);
}

/// Nothing to do, and the plan says so rather than taking a restore point
/// and rewriting a ledger for no reason.
#[test]
fn a_folder_that_already_matches_needs_no_work() {
    let same = snapshot(&[(KAMIL, entry("k1", "1.0", "sounds/k.ogg"))]);

    let plan = super::plan(&same, &same, &all_present);

    assert!(plan.is_empty());
    assert_eq!(plan.keep, vec![KAMIL.to_string()]);
}

/// RULE 2, and the reason the plan has an `unavailable` list at all: with
/// the old payload gone, removing the bad version would leave the user with
/// NOTHING. The wrong version beats an empty folder.
#[test]
fn a_package_that_cannot_be_put_back_is_left_installed() {
    let target = snapshot(&[(KAMIL, entry("sha-good", "2.1.0", "sounds/k.ogg"))]);
    let live = snapshot(&[(KAMIL, entry("sha-bad", "3.0.0", "sounds/k.ogg"))]);

    let plan = super::plan(&target, &live, &|sha| sha != "sha-good");

    assert!(
        plan.remove.is_empty(),
        "removing it would leave the user with neither version"
    );
    assert!(plan.put_back.is_empty());
    assert_eq!(plan.unavailable.len(), 1);
    assert_eq!(plan.unavailable[0].sha256, "sha-good");
    assert!(
        plan.is_partial(),
        "the preview has to say so before committing"
    );
}

/// The rest of the restore still happens — a partial way back beats none,
/// and the files the lost package replaced are in the point either way.
#[test]
fn one_missing_payload_does_not_stop_the_others() {
    let target = snapshot(&[
        (KAMIL, entry("sha-good", "2.1.0", "sounds/k.ogg")),
        (POLISH, entry("p1", "1.0", "language/p.ini")),
    ]);
    let live = snapshot(&[
        (KAMIL, entry("sha-bad", "3.0.0", "sounds/k.ogg")),
        (EWA, entry("e1", "1.0", "sounds/e.ogg")),
    ]);

    let plan = super::plan(&target, &live, &|sha| sha != "sha-good");

    assert_eq!(plan.remove, vec![EWA.to_string()], "EWA still goes");
    assert_eq!(plan.put_back.len(), 1, "polish still comes back");
    assert_eq!(plan.put_back[0].key, POLISH);
    assert_eq!(plan.unavailable.len(), 1);
}

/// A half-written entry is never "already correct", even when the digest
/// matches: its files may be partial, which is what the write-ahead state
/// exists to record.
#[test]
fn an_unfinished_install_is_redone_even_at_the_same_version() {
    let target = snapshot(&[(KAMIL, entry("k1", "1.0", "sounds/k.ogg"))]);
    let mut half = entry("k1", "1.0", "sounds/k.ogg");
    half.state = crate::ledger::EntryState::Installing;
    let live = snapshot(&[(KAMIL, half)]);

    let plan = super::plan(&target, &live, &all_present);

    assert_eq!(plan.remove, vec![KAMIL.to_string()]);
    assert_eq!(plan.put_back.len(), 1);
    assert!(plan.keep.is_empty());
}

/// "Uninstall everything" is restore with an empty target — the same code
/// path, which is why the scene offers it as the oldest entry in the list.
#[test]
fn an_empty_target_removes_everything() {
    let live = snapshot(&[
        (KAMIL, entry("k1", "1.0", "sounds/k.ogg")),
        (POLISH, entry("p1", "1.0", "language/p.ini")),
    ]);

    let plan = super::plan(&super::Snapshot::new(), &live, &all_present);

    assert_eq!(plan.remove.len(), 2);
    assert!(plan.put_back.is_empty());
    assert!(plan.keep.is_empty());
}

/// RULE 3. The snapshot knows the file replaced something of the user's;
/// a fresh install would record `absent` and a later uninstall would then
/// DELETE a file the user had before we ever arrived.
#[test]
fn the_ledger_afterwards_keeps_the_snapshots_provenance() {
    let target = snapshot(&[(
        KAMIL,
        entry_replacing(
            "sha-good",
            "2.1.0",
            "Plugins/Pacenote/PaceNote.ini",
            crate::ledger::Prior::Backup {
                point: "2026-10-01T00-00-00Z".into(),
            },
        ),
    )]);
    let live = snapshot(&[(
        KAMIL,
        entry("sha-bad", "3.0.0", "Plugins/Pacenote/PaceNote.ini"),
    )]);

    let plan = super::plan(&target, &live, &all_present);
    let after = super::ledger_after(&target, &live, &plan, &[]);

    let file = after[KAMIL]
        .files
        .values()
        .next()
        .expect("the entry keeps its file");
    assert_eq!(
        file.prior,
        crate::ledger::Prior::Backup {
            point: "2026-10-01T00-00-00Z".into()
        },
        "the user's own copy must still be findable after a restore"
    );
    assert_eq!(after[KAMIL].sha256, "sha-good");
}

/// A package the executor could not put back must not be left in the
/// ledger as though it had been.
#[test]
fn a_failed_put_back_is_not_recorded_as_installed() {
    let target = snapshot(&[
        (KAMIL, entry("k1", "1.0", "sounds/k.ogg")),
        (POLISH, entry("p1", "1.0", "language/p.ini")),
    ]);
    let live = super::Snapshot::new();

    let plan = super::plan(&target, &live, &all_present);
    let after = super::ledger_after(&target, &live, &plan, &[KAMIL.to_string()]);

    assert!(
        !after.contains_key(KAMIL),
        "it is not on disk, so not in the ledger"
    );
    assert!(after.contains_key(POLISH));
}

/// What could not come back stays described by its OWN entry: it is the
/// version actually on disk.
#[test]
fn an_unavailable_package_keeps_its_live_entry() {
    let target = snapshot(&[(KAMIL, entry("sha-good", "2.1.0", "sounds/k.ogg"))]);
    let live = snapshot(&[(KAMIL, entry("sha-bad", "3.0.0", "sounds/k.ogg"))]);

    let plan = super::plan(&target, &live, &|sha| sha != "sha-good");
    let after = super::ledger_after(&target, &live, &plan, &[]);

    assert_eq!(
        after[KAMIL].sha256, "sha-bad",
        "the ledger has to describe what is actually there"
    );
}

/// The list needs more than a date to choose from, and nothing records
/// what a run was about, so it is derived.
#[test]
fn a_points_line_says_what_the_run_after_it_did() {
    let before = snapshot(&[
        (KAMIL, entry("k1", "1.0", "sounds/k.ogg")),
        (POLISH, entry("p1", "1.0", "language/p.ini")),
    ]);
    let after = snapshot(&[
        (KAMIL, entry("k2", "2.0", "sounds/k.ogg")), // updated
        (EWA, entry("e1", "1.0", "sounds/e.ogg")),   // added
                                                     // POLISH removed
    ]);

    let changes = super::changes(&before, &after);

    assert_eq!(changes.added, 1);
    assert_eq!(changes.removed, 1);
    assert_eq!(changes.updated, 1);
    assert!(!changes.is_empty());
}

/// The list shows a date, not a directory name.
#[test]
fn a_point_reads_as_a_date_and_time() {
    assert_eq!(
        super::human_time("2026-10-27T12-11-31Z"),
        "2026-10-27 12:11"
    );
    // Two runs in one second: the suffix is not part of the time.
    assert_eq!(
        super::human_time("2026-10-27T12-11-31Z-2"),
        "2026-10-27 12:11"
    );
    // Something nobody recognises is shown as it is, never as a blank row.
    assert_eq!(super::human_time("backup-old"), "backup-old");
}
