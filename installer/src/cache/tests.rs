//! The cases that matter are the ones where the index and the disk disagree.

fn entry(game: &str, name: &str, sha: &str) -> crate::tree::CachedPackage {
    crate::tree::CachedPackage {
        game: game.into(),
        directory: "public".into(),
        name: name.into(),
        sha256: sha.into(),
        version: "1.0.0".into(),
        display_name: None,
        exclusive_group: None,
        requires: Vec::new(),
        size: 0,
        updated_at: 0,
    }
}

fn put_payload(root: &std::path::Path, sha: &str) {
    std::fs::create_dir_all(root).expect("create root");
    std::fs::write(root.join(format!("{sha}.zip")), b"payload").expect("write payload");
}

#[test]
fn what_is_written_is_still_there_next_run() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-roundtrip");
    put_payload(&dir.0, "aa");
    let mut cache = super::Cache::open(&dir.0);
    cache
        .insert(entry("rbr", "style/Descriptive", "aa"))
        .expect("insert");

    let reopened = super::Cache::open(&dir.0);
    let found = reopened
        .get("rbr/public/style/Descriptive")
        .expect("entry survives");
    assert_eq!(found.sha256, "aa");
    assert_eq!(found.version, "1.0.0");
    assert_eq!(reopened.entries().len(), 1);
}

#[test]
fn two_games_can_hold_the_same_directory_and_name() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-twogames");
    put_payload(&dir.0, "aa");
    put_payload(&dir.0, "bb");
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/Numeric", "aa")).unwrap();
    cache
        .insert(entry("dirtrally2", "style/Numeric", "bb"))
        .unwrap();

    assert_eq!(cache.entries().len(), 2, "one shadowed the other");
    assert_eq!(cache.get("rbr/public/style/Numeric").unwrap().sha256, "aa");
    assert_eq!(
        cache.get("dirtrally2/public/style/Numeric").unwrap().sha256,
        "bb"
    );
}

#[test]
fn identical_content_is_stored_once() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-dedupe");
    put_payload(&dir.0, "same");
    let mut cache = super::Cache::open(&dir.0);
    cache
        .insert(entry("rbr", "language/english", "same"))
        .unwrap();
    cache
        .insert(entry("beamng", "language/english", "same"))
        .unwrap();

    assert_eq!(cache.entries().len(), 2);
    let zip_count = std::fs::read_dir(&dir.0)
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "zip")
        })
        .count();
    assert_eq!(zip_count, 1);
}

#[test]
fn an_entry_whose_payload_vanished_is_not_offered() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-vanished");
    put_payload(&dir.0, "here");
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/Here", "here")).unwrap();
    cache.insert(entry("rbr", "style/Gone", "gone")).unwrap();

    let offered: Vec<String> = cache
        .entries()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(offered, vec!["style/Here".to_string()]);
    assert!(cache.get("rbr/public/style/Gone").is_some());
    assert!(!cache.has_payload("gone"));
}

/// Losing the index costs a re-download; deleting the payloads over a parse error would cost more.
#[test]
fn a_corrupt_index_reads_as_empty_and_keeps_the_payloads() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-corrupt");
    put_payload(&dir.0, "aa");
    std::fs::write(dir.0.join(super::INDEX_FILE), b"{ this is not json").unwrap();

    let cache = super::Cache::open(&dir.0);
    assert!(cache.entries().is_empty());
    assert!(dir.0.join("aa.zip").is_file());
    assert_eq!(cache.index_state(), super::IndexState::Unreadable);
}

/// A sweep trusting an unreadable index would delete every download the user has.
#[test]
fn nothing_is_an_orphan_when_the_index_could_not_be_read() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-no-sweep");
    put_payload(&dir.0, "aa");
    put_payload(&dir.0, "bb");
    std::fs::write(dir.0.join(super::INDEX_FILE), b"{ truncated").unwrap();

    assert!(super::Cache::open(&dir.0).orphans().is_empty());
}

/// A newer build may give the same keys another meaning.
#[test]
fn an_index_from_a_newer_build_is_not_trusted() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-future");
    put_payload(&dir.0, "aa");
    std::fs::write(
        dir.0.join(super::INDEX_FILE),
        br#"{"version": 9999, "packages": {}}"#,
    )
    .unwrap();

    let cache = super::Cache::open(&dir.0);
    assert_eq!(cache.index_state(), super::IndexState::Unreadable);
    assert!(cache.orphans().is_empty());
    assert!(dir.0.join("aa.zip").is_file());
}

#[test]
fn a_missing_index_is_told_apart_from_an_unreadable_one() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-first-run");
    put_payload(&dir.0, "stray");

    let cache = super::Cache::open(&dir.0);
    assert_eq!(cache.index_state(), super::IndexState::Missing);
    assert!(cache.orphans().is_empty());

    let mut cache = super::Cache::open(&dir.0);
    put_payload(&dir.0, "kept");
    cache.insert(entry("rbr", "style/Kept", "kept")).unwrap();
    assert_eq!(cache.index_state(), super::IndexState::Loaded);
    assert_eq!(cache.orphans().len(), 1);
}

#[test]
fn saving_never_removes_the_old_index_first() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-replace");
    put_payload(&dir.0, "aa");
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/A", "aa")).unwrap();

    let before = std::fs::read(dir.0.join(super::INDEX_FILE)).expect("first index");
    cache.insert(entry("rbr", "style/B", "aa")).unwrap();
    let after = std::fs::read(dir.0.join(super::INDEX_FILE)).expect("index still there");

    assert_ne!(before, after);
    assert_eq!(super::Cache::open(&dir.0).entries().len(), 2);
}

#[test]
fn a_missing_cache_is_an_empty_one() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-missing");
    let cache = super::Cache::open(dir.0.join("not-created-yet"));
    assert!(cache.entries().is_empty());
    assert!(cache.orphans().is_empty());
    assert!(cache.get("rbr/public/anything").is_none());
}

#[test]
fn saving_leaves_no_temp_file() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-atomic");
    put_payload(&dir.0, "aa");
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/A", "aa")).unwrap();
    cache.insert(entry("rbr", "style/B", "aa")).unwrap();

    assert!(dir.0.join(super::INDEX_FILE).is_file());
    assert!(!dir.0.join(format!("{}.tmp", super::INDEX_FILE)).exists());
    assert_eq!(super::Cache::open(&dir.0).entries().len(), 2);
}

#[test]
fn orphans_are_payloads_with_no_entry() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-orphans");
    put_payload(&dir.0, "kept");
    put_payload(&dir.0, "stray");
    std::fs::write(dir.0.join("notes.txt"), b"not a payload").unwrap();
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/Kept", "kept")).unwrap();

    let orphans: Vec<String> = cache
        .orphans()
        .into_iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(orphans, vec!["stray.zip".to_string()]);
}

#[test]
fn removing_an_entry_leaves_the_payload_for_the_sweep() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-remove");
    put_payload(&dir.0, "aa");
    let mut cache = super::Cache::open(&dir.0);
    cache.insert(entry("rbr", "style/A", "aa")).unwrap();
    cache.remove("rbr/public/style/A").unwrap();

    assert!(cache.entries().is_empty());
    assert!(dir.0.join("aa.zip").is_file());
    assert_eq!(cache.orphans().len(), 1);
}

/// Kept: what the index names and what `in_use` names. Gone: the rest, and a
/// download a killed run left half-written.
#[test]
fn a_sweep_removes_only_what_nothing_needs() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-sweep");
    for sha in ["indexed", "rollback", "stray"] {
        put_payload(&dir.0, sha);
    }
    std::fs::write(dir.0.join("0123.part"), b"half").expect("partial");
    let mut cache = super::Cache::open(&dir.0);
    cache
        .insert(entry("rbr", "style/Kept", "indexed"))
        .expect("index");
    let in_use = ["rollback".to_string()];

    let swept = cache.sweep(Some(&in_use));

    assert_eq!(swept.files, 2);
    assert_eq!(swept.bytes, "payload".len() as u64 + "half".len() as u64);
    assert!(dir.0.join("indexed.zip").is_file());
    assert!(dir.0.join("rollback.zip").is_file());
    assert!(!dir.0.join("stray.zip").exists());
    assert!(!dir.0.join("0123.part").exists());
}

/// What is needed is unknown, so no payload may go; a half-written download still does.
#[test]
fn without_knowing_what_is_needed_only_partial_downloads_go() {
    let dir = crate::scratch_dir::ScratchDir::new("cache-sweep-unknown");
    put_payload(&dir.0, "stray");
    std::fs::write(dir.0.join("0123.part"), b"half").expect("partial");
    let mut cache = super::Cache::open(&dir.0);
    cache
        .insert(entry("rbr", "style/Kept", "indexed"))
        .expect("index");

    let swept = cache.sweep(None);

    assert_eq!(swept.files, 1);
    assert!(dir.0.join("stray.zip").is_file());
    assert!(!dir.0.join("0123.part").exists());
}

/// Deleting the game has to take the downloads with it.
#[test]
fn a_cache_lives_beside_the_install_it_belongs_to() {
    let game = std::path::Path::new(r"C:\Games\RBR");

    assert_eq!(
        super::dir_for(game),
        game.join(".pacenotes").join("cache"),
        "same state directory as the ledger, the backups and the logs"
    );
}
