//! The log exists for the run that goes wrong, so these are about the file.

#[test]
fn every_line_is_on_disk_before_the_next_one_is_written() {
    let dir = crate::scratch_dir::ScratchDir::new("log-flush");
    let path = dir.0.join("install.log");
    let mut log = super::Log::create(Some(path.clone()));

    log.line("downloading codriver/polish/male/Kamil");
    let on_disk = std::fs::read_to_string(&path).expect("log file");
    assert!(
        on_disk.contains("downloading codriver/polish/male/Kamil"),
        "{on_disk:?}"
    );

    log.line("verified");
    let on_disk = std::fs::read_to_string(&path).expect("log file");
    assert!(on_disk.contains("verified"));
}

/// Reopening the installer to read what happened cannot erase the run being read.
#[test]
fn each_run_writes_its_own_file() {
    let dir = crate::scratch_dir::ScratchDir::new("log-per-run");
    let mut first = super::Log::create(Some(super::log_path(&dir.0, "2026-10-25T10-00-00Z")));
    first.line("yesterday: installed Kamil");
    drop(first);
    let mut second = super::Log::create(Some(super::log_path(&dir.0, "2026-10-26T10-00-00Z")));
    second.line("today: installed Ewa");
    drop(second);

    let logs = super::log_dir(&dir.0);
    let names = sorted_names(&logs);
    assert_eq!(
        names,
        vec![
            "install-2026-10-25T10-00-00Z.log",
            "install-2026-10-26T10-00-00Z.log"
        ]
    );
    let yesterday = std::fs::read_to_string(logs.join(&names[0])).expect("yesterday's log");
    assert!(yesterday.contains("yesterday"));
}

#[test]
fn the_log_directory_is_created() {
    let dir = crate::scratch_dir::ScratchDir::new("log-mkdir");
    let path = dir.0.join("never-created").join("install.log");
    let mut log = super::Log::create(Some(path.clone()));
    log.line("first run");
    assert!(path.is_file());
    assert_eq!(log.path(), Some(path.as_path()));
}

#[test]
fn a_log_with_nowhere_to_write_still_records() {
    let mut log = super::Log::create(None);
    log.line("downloading");
    log.line("verified");
    assert_eq!(log.lines().len(), 2);
    assert_eq!(log.path(), None);
}

#[test]
fn the_pane_text_is_crlf_separated() {
    let mut log = super::Log::create(None);
    log.line("downloading");
    log.line("verified");
    assert_eq!(log.text(), "downloading\r\nverified");
}

#[test]
fn the_log_lives_in_the_games_state_directory() {
    let game_dir = std::path::Path::new("C:\\Games\\RBR");
    assert_eq!(
        super::log_path(game_dir, "2026-10-26T10-00-00Z"),
        game_dir
            .join(".pacenotes")
            .join("logs")
            .join("install-2026-10-26T10-00-00Z.log")
    );
}

/// By name, so a file restored from a backup cannot pass for the newest run.
#[test]
fn only_the_newest_logs_are_kept() {
    let dir = crate::scratch_dir::ScratchDir::new("log-prune");
    for day in 1..=7 {
        let name = format!("install-2026-10-0{day}T00-00-00Z.log");
        std::fs::write(dir.0.join(name), "x").expect("write");
    }
    std::fs::write(dir.0.join("notes.txt"), "keep me").expect("write");

    super::prune(&dir.0, 3);

    assert_eq!(
        sorted_names(&dir.0),
        vec![
            "install-2026-10-05T00-00-00Z.log",
            "install-2026-10-06T00-00-00Z.log",
            "install-2026-10-07T00-00-00Z.log",
            "notes.txt",
        ]
    );
}

/// The normal case: the log starts before the user has chosen a game folder.
#[test]
fn attaching_writes_out_what_was_already_said() {
    let dir = crate::scratch_dir::ScratchDir::new("log-attach");
    let mut log = super::Log::create(None);
    log.line("language: English");
    log.line("server: online");
    assert_eq!(log.path(), None);

    let path = super::log_path(&dir.0, "2026-10-26T10-00-00Z");
    log.attach(path.clone());
    log.line("game: Richard Burns Rally");

    let on_disk = std::fs::read_to_string(&path).expect("log file");
    for line in [
        "language: English",
        "server: online",
        "game: Richard Burns Rally",
    ] {
        assert!(on_disk.contains(line), "{on_disk:?}");
    }
    assert_eq!(log.path(), Some(path.as_path()));
}

fn sorted_names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
