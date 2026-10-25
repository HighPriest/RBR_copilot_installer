//! Each case is a way uninstall could delete the wrong file, which nothing undoes.

const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
const DESCRIPTIVE: &str = "rbr/public/style/Descriptive";
const INI: &str = "Plugins/Pacenote/PaceNote.ini";

/// An entry owning `paths`, each recorded as having replaced nothing.
fn entry(sha: &str, paths: &[&str]) -> super::LedgerEntry {
    super::LedgerEntry {
        sha256: sha.into(),
        version: "1.0.0".into(),
        exclusive_group: None,
        state: super::EntryState::Installed,
        files: paths
            .iter()
            .map(|path| {
                (
                    super::fold(path),
                    super::LedgerFile {
                        path: path.to_string(),
                        sha256: format!("sha-of-{path}"),
                        prior: super::Prior::Absent,
                    },
                )
            })
            .collect(),
    }
}

fn ledger_file(game: &crate::scratch_dir::ScratchDir) -> std::path::PathBuf {
    game.0.join(super::STATE_DIR).join(super::LEDGER_FILE)
}

#[test]
fn what_was_installed_is_still_known_next_run() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-roundtrip");
    let mut ledger = super::Ledger::open(&game.0);
    assert_eq!(ledger.state(), super::LedgerState::Missing);
    ledger.put(KAMIL, entry("zip-sha", &[INI])).expect("write");

    let reopened = super::Ledger::open(&game.0);
    assert_eq!(reopened.state(), super::LedgerState::Loaded);
    let found = reopened.get(KAMIL).expect("the entry survives");
    assert_eq!(found.sha256, "zip-sha");
    assert_eq!(found.files.len(), 1);
    assert_eq!(found.files.values().next().unwrap().path, INI);
}

#[test]
fn the_ledger_lives_in_the_state_directory() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-where");
    let mut ledger = super::Ledger::open(&game.0);
    ledger.put(KAMIL, entry("zip-sha", &[INI])).expect("write");
    assert!(ledger_file(&game).is_file());
    assert!(!game.0.join("Plugins").exists());
}

#[test]
fn ownership_ignores_case_because_the_filesystem_does() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-case");
    let mut ledger = super::Ledger::open(&game.0);
    ledger
        .put(KAMIL, entry("a", &["Plugins/Pacenote/PaceNote.ini"]))
        .unwrap();
    ledger
        .put(DESCRIPTIVE, entry("b", &["plugins/pacenote/pacenote.INI"]))
        .unwrap();

    let mut owners = ledger.owners_of("PLUGINS/PACENOTE/PACENOTE.INI");
    owners.sort();
    let mut expected = vec![KAMIL, DESCRIPTIVE];
    expected.sort();
    assert_eq!(owners, expected);
    assert!(ledger.owns("Plugins/Pacenote/PaceNote.ini"));
}

#[test]
fn a_file_no_package_owns_is_the_users() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-unowned");
    let mut ledger = super::Ledger::open(&game.0);
    ledger.put(KAMIL, entry("a", &[INI])).unwrap();
    assert!(ledger.owns(INI));
    assert!(!ledger.owns("Plugins/Pacenote/Something.ini"));
    assert!(ledger
        .owners_of("Plugins/Pacenote/Something.ini")
        .is_empty());
}

/// All three style packages own the same 49 paths.
#[test]
fn a_shared_file_keeps_an_owner_until_the_last_one_leaves() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-shared");
    let mut ledger = super::Ledger::open(&game.0);
    ledger
        .put(KAMIL, entry("a", &[INI, "Audio/Go.wav"]))
        .unwrap();
    ledger.put(DESCRIPTIVE, entry("b", &[INI])).unwrap();
    assert_eq!(ledger.owners_of(INI).len(), 2);

    ledger.remove(KAMIL).unwrap();
    assert_eq!(ledger.owners_of(INI), vec![DESCRIPTIVE]);
    assert!(ledger.owners_of("Audio/Go.wav").is_empty());
}

#[test]
fn what_was_there_before_is_recorded_per_file() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-prior");
    let mut ledger = super::Ledger::open(&game.0);
    let mut recorded = entry("a", &[]);
    for (path, prior) in [
        ("Plugins/new.ini", super::Prior::Absent),
        ("Plugins/replaced.ini", super::Prior::Backup),
        ("Plugins/same.ini", super::Prior::Identical),
    ] {
        recorded.files.insert(
            super::fold(path),
            super::LedgerFile {
                path: path.into(),
                sha256: "written".into(),
                prior,
            },
        );
    }
    ledger.put(KAMIL, recorded).unwrap();

    let reopened = super::Ledger::open(&game.0);
    let files = &reopened.get(KAMIL).unwrap().files;
    assert_eq!(files["plugins/new.ini"].prior, super::Prior::Absent);
    assert_eq!(files["plugins/replaced.ini"].prior, super::Prior::Backup);
    assert_eq!(files["plugins/same.ini"].prior, super::Prior::Identical);
}

#[test]
fn an_unfinished_install_is_recognisable_afterwards() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-crash");
    let mut ledger = super::Ledger::open(&game.0);
    let mut planned = entry("a", &[INI]);
    planned.state = super::EntryState::Installing;
    ledger.put(KAMIL, planned).unwrap();

    let next_run = super::Ledger::open(&game.0);
    assert_eq!(next_run.unfinished(), vec![KAMIL.to_string()]);
    assert!(next_run.owns(INI), "the half-written file is ours");

    let mut ledger = super::Ledger::open(&game.0);
    ledger.mark_installed(KAMIL).unwrap();
    assert!(super::Ledger::open(&game.0).unfinished().is_empty());
}

#[test]
fn the_whole_record_can_be_replaced_at_once() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-replace");
    let mut ledger = super::Ledger::open(&game.0);
    ledger.put(KAMIL, entry("a", &[INI])).unwrap();
    ledger
        .put(DESCRIPTIVE, entry("b", &["Plugins/x.ini"]))
        .unwrap();

    let snapshot = [(KAMIL.to_string(), entry("a", &[INI]))]
        .into_iter()
        .collect();
    ledger.replace_all(snapshot).unwrap();

    let reopened = super::Ledger::open(&game.0);
    assert_eq!(reopened.len(), 1);
    assert!(reopened.get(DESCRIPTIVE).is_none());
}

#[test]
fn an_unreadable_ledger_is_never_overwritten() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-future");
    std::fs::create_dir_all(game.0.join(super::STATE_DIR)).unwrap();
    std::fs::write(ledger_file(&game), br#"{"version": 9999, "packages": {}}"#).unwrap();

    let mut ledger = super::Ledger::open(&game.0);
    assert_eq!(ledger.state(), super::LedgerState::Unreadable);
    assert!(ledger.put(KAMIL, entry("a", &[INI])).is_err());
    let still = std::fs::read_to_string(ledger_file(&game)).unwrap();
    assert!(still.contains("9999"));
}

/// A truncated file may be any build's crash, and its packages are still on disk.
#[test]
fn a_corrupt_ledger_is_also_refused() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-corrupt");
    std::fs::create_dir_all(game.0.join(super::STATE_DIR)).unwrap();
    std::fs::write(ledger_file(&game), b"{ truncated").unwrap();

    let mut ledger = super::Ledger::open(&game.0);
    assert_eq!(ledger.state(), super::LedgerState::Unreadable);
    assert!(ledger.put(KAMIL, entry("a", &[INI])).is_err());
}

#[test]
fn saving_leaves_no_temp_file_and_no_gap() {
    let game = crate::scratch_dir::ScratchDir::new("ledger-atomic");
    let mut ledger = super::Ledger::open(&game.0);
    ledger.put(KAMIL, entry("a", &[INI])).unwrap();
    let first = std::fs::read(ledger_file(&game)).expect("written");

    ledger
        .put(DESCRIPTIVE, entry("b", &["Plugins/x.ini"]))
        .unwrap();
    let second = std::fs::read(ledger_file(&game)).expect("still there");
    assert_ne!(first, second);
    assert!(!ledger_file(&game).with_extension("json.tmp").exists());
    assert_eq!(super::Ledger::open(&game.0).len(), 2);
}

#[test]
fn each_game_folder_has_its_own_ledger() {
    let first = crate::scratch_dir::ScratchDir::new("ledger-folder-a");
    let second = crate::scratch_dir::ScratchDir::new("ledger-folder-b");
    super::Ledger::open(&first.0)
        .put(KAMIL, entry("a", &[INI]))
        .unwrap();

    assert_eq!(super::Ledger::open(&first.0).len(), 1);
    assert_eq!(super::Ledger::open(&second.0).len(), 0);
    assert_eq!(
        super::Ledger::open(&second.0).state(),
        super::LedgerState::Missing
    );
}
