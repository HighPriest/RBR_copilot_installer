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

#[test]
fn a_new_run_replaces_the_previous_log() {
    let dir = crate::scratch_dir::ScratchDir::new("log-truncate");
    let path = dir.0.join("install.log");
    let mut previous = super::Log::create(Some(path.clone()));
    previous.line("yesterday: installed Kamil");
    drop(previous);

    let mut current = super::Log::create(Some(path.clone()));
    current.line("today: installed Ewa");
    let on_disk = std::fs::read_to_string(&path).expect("log file");
    assert!(on_disk.contains("today: installed Ewa"));
    assert!(!on_disk.contains("yesterday"), "{on_disk:?}");
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
