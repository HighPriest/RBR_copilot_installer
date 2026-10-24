//! The fixtures are real `Compress-Archive` output (see `fixtures/README.md`):
//! a miniature overlay holding `RBR_INI`, `SOUND` and `aa/evil.txt`, the last
//! exactly as long as `../evil.txt` so it can be patched into a hostile name.

const DEFLATED: &[u8] = include_bytes!("fixtures/overlay-deflate.zip");
const STORED: &[u8] = include_bytes!("fixtures/overlay-stored.zip");

const RBR_INI: &str = "Plugins/Pacenote/config/pacenote/Rbr.ini";
const SOUND: &str = "Plugins/Pacenote/sounds/polish/male/Kamil/001.ogg";

fn under(dir: &crate::scratch_dir::ScratchDir, relative: &str) -> std::path::PathBuf {
    let mut path = dir.0.clone();
    for segment in relative.split('/') {
        path.push(segment);
    }
    path
}

#[test]
fn entries_lists_the_files_a_package_owns_and_no_directories() {
    let listed = super::entries(DEFLATED).expect("read the fixture");
    assert_eq!(
        listed,
        vec![
            RBR_INI.to_string(),
            SOUND.to_string(),
            "aa/evil.txt".to_string()
        ]
    );
}

/// The paths are ledger keys, so they read the same on every machine.
#[test]
fn paths_are_slash_separated_on_every_host() {
    for path in super::entries(DEFLATED).expect("read the fixture") {
        assert!(!path.contains('\\'), "{path}");
        assert!(!path.starts_with('/'), "{path}");
    }
}

#[test]
fn extracting_puts_every_file_where_the_package_says() {
    let dir = crate::scratch_dir::ScratchDir::new("zip-extract");
    let written = super::extract(DEFLATED, &dir.0).expect("extract");
    assert_eq!(written, super::entries(DEFLATED).expect("list"));
    assert_eq!(
        std::fs::read(under(&dir, RBR_INI)).expect("the ini"),
        b"[PACENOTE]\r\nname=Kamil\r\n"
    );
    assert_eq!(
        std::fs::read(under(&dir, SOUND)).expect("the sound").len(),
        75
    );
}

#[test]
fn a_stored_entry_reads_the_same_as_a_deflated_one() {
    let deflated = crate::scratch_dir::ScratchDir::new("zip-deflated");
    let stored = crate::scratch_dir::ScratchDir::new("zip-stored");
    super::extract(DEFLATED, &deflated.0).expect("extract deflated");
    super::extract(STORED, &stored.0).expect("extract stored");
    for path in [RBR_INI, SOUND] {
        assert_eq!(
            std::fs::read(under(&deflated, path)).expect("deflated file"),
            std::fs::read(under(&stored, path)).expect("stored file"),
            "{path}"
        );
    }
}

/// .NET's zips have no entry for `Plugins/`, nor for the folder holding a nested file.
#[test]
fn directories_that_are_not_entries_are_created_anyway() {
    let dir = crate::scratch_dir::ScratchDir::new("zip-mkdirs");
    super::extract(DEFLATED, &dir.0).expect("extract");
    for directory in [
        "Plugins",
        "Plugins/Pacenote/config/pacenote",
        "Plugins/Pacenote/sounds/polish/male/Kamil",
    ] {
        assert!(under(&dir, directory).is_dir(), "{directory}");
    }
}

#[test]
fn extracting_twice_overwrites_rather_than_failing() {
    let dir = crate::scratch_dir::ScratchDir::new("zip-overwrite");
    super::extract(DEFLATED, &dir.0).expect("first extract");
    std::fs::write(under(&dir, RBR_INI), b"edited by hand").expect("edit");
    super::extract(DEFLATED, &dir.0).expect("second extract");
    assert_eq!(
        std::fs::read(under(&dir, RBR_INI)).expect("the ini"),
        b"[PACENOTE]\r\nname=Kamil\r\n"
    );
}

/// Zip slip: nothing may be written, not even the entries before the bad one.
#[test]
fn an_entry_that_escapes_the_target_is_refused() {
    let hostile = rename_entry(DEFLATED, "aa/evil.txt", "../evil.txt");
    let dir = crate::scratch_dir::ScratchDir::new("zip-slip");
    let inside = dir.0.join("game");
    std::fs::create_dir_all(&inside).expect("create target");

    let error = super::extract(&hostile, &inside).expect_err("extraction must be refused");
    assert!(error.0.contains("outside"), "{}", error.0);
    assert!(!dir.0.join("evil.txt").exists());
    assert!(!inside
        .join(RBR_INI.replace('/', std::path::MAIN_SEPARATOR_STR))
        .exists());
    assert!(super::entries(&hostile).is_err());
}

#[test]
fn the_guard_rejects_the_names_that_would_escape() {
    for name in [
        "../evil.txt",
        "a/../../evil.txt",
        "..\\evil.txt",
        "/etc/passwd",
        "\\Windows\\System32\\drivers\\etc\\hosts",
        "C:/Windows/System32/x.dll",
        "C:x.dll",
        "Rbr.ini:hidden",
        "..",
        "",
        "/",
    ] {
        assert_eq!(super::safe_relative(name), None, "{name:?}");
    }
}

#[test]
fn the_guard_keeps_the_names_packages_really_use() {
    for (name, expected) in [
        (RBR_INI, RBR_INI),
        (
            "Plugins\\Pacenote\\config\\Rbr.ini",
            "Plugins/Pacenote/config/Rbr.ini",
        ),
        ("Plugins//Pacenote/./config/", "Plugins/Pacenote/config"),
        ("style/Numeric Standard.ini", "style/Numeric Standard.ini"),
    ] {
        assert_eq!(
            super::safe_relative(name),
            Some(expected.to_string()),
            "{name:?}"
        );
    }
}

/// One flipped byte of stored text leaves sizes intact, so only the CRC can tell.
#[test]
fn a_damaged_entry_is_refused_rather_than_installed() {
    let mut damaged = STORED.to_vec();
    let at = find(&damaged, b"[PACENOTE]").expect("the ini text is stored verbatim");
    damaged[at + 1] ^= 0x20;
    let dir = crate::scratch_dir::ScratchDir::new("zip-damaged");

    let error = super::extract(&damaged, &dir.0).expect_err("damaged data must be refused");
    assert!(error.0.contains("damaged"), "{}", error.0);
    assert!(error.0.contains("download it again"), "{}", error.0);
}

/// A short list would become a short ledger, leaving files nothing owns.
#[test]
fn a_truncated_central_directory_is_refused_not_half_read() {
    let mut damaged = DEFLATED.to_vec();
    let last_record = damaged
        .windows(4)
        .rposition(|window| window == [0x50, 0x4b, 0x01, 0x02])
        .expect("a central directory record");
    damaged[last_record..last_record + 4].copy_from_slice(b"XXXX");

    let error = super::entries(&damaged).expect_err("a short directory must be refused");
    assert!(error.0.contains("entries but only"), "{}", error.0);
}

#[test]
fn something_that_is_not_a_zip_is_reported_as_such() {
    assert!(super::entries(b"").is_err());
    assert!(super::entries(b"<html>404 Not Found</html>").is_err());
    let without_end_record = &DEFLATED[..DEFLATED.len() - 40];
    assert!(super::entries(without_end_record).is_err());
}

/// MANUAL CHECK against `PACENOTES_ZIP_CORPUS`, a directory of real package zips
/// such as a download cache; see `fixtures/README.md`.
#[test]
#[ignore]
fn every_real_package_in_a_corpus_reads() {
    let Some(corpus) = std::env::var_os("PACENOTES_ZIP_CORPUS") else {
        panic!("set PACENOTES_ZIP_CORPUS to a directory of package zips");
    };
    let dir = crate::scratch_dir::ScratchDir::new("zip-corpus");
    let mut packages = 0;
    let mut files = 0;
    for entry in std::fs::read_dir(&corpus).expect("read the corpus") {
        let path = entry.expect("corpus entry").path();
        if path.extension().is_none_or(|extension| extension != "zip") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read a package");
        let listed =
            super::entries(&bytes).unwrap_or_else(|e| panic!("{}: {}", path.display(), e.0));
        let into = dir.0.join(format!("p{packages}"));
        let written =
            super::extract(&bytes, &into).unwrap_or_else(|e| panic!("{}: {}", path.display(), e.0));
        assert_eq!(listed, written, "{}", path.display());
        assert!(!written.is_empty(), "{} is empty", path.display());
        packages += 1;
        files += written.len();
        let _ = std::fs::remove_dir_all(&into);
    }
    assert!(packages > 0, "no .zip files in {corpus:?}");
    println!("read {packages} packages, {files} files");
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Renames an entry in both its local header and the central directory; the
/// names must be the same length so no offset moves.
fn rename_entry(zip: &[u8], from: &str, to: &str) -> Vec<u8> {
    assert_eq!(from.len(), to.len());
    let mut renamed = zip.to_vec();
    let needle = from.as_bytes();
    let mut replaced = 0;
    for at in 0..renamed.len().saturating_sub(needle.len()) {
        if &renamed[at..at + needle.len()] == needle {
            renamed[at..at + needle.len()].copy_from_slice(to.as_bytes());
            replaced += 1;
        }
    }
    assert_eq!(
        replaced, 2,
        "expected the local header and the central directory"
    );
    renamed
}
