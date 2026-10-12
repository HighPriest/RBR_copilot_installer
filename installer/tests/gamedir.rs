//! AUTOMATED TEST for `pacenotes_installer::gamedir`.
//!
//! Cross-platform: pure std, no Win32, so this runs in CI on any host.
//!
//! Every case below builds a fixture that represents a REAL situation the user
//! can be in — a folder holding a specific game, a folder holding a different
//! game, a folder holding neither — and asserts the exact status for it. The
//! point of the game-folder field is to tell those apart, so "some directory
//! exists" is not a meaningful input and does not appear here.

use pacenotes_installer::gamedir::{detect, validate, DirStatus, Game};

/// A scratch directory that deletes itself. Hand-rolled rather than pulling in
/// `tempfile`, to keep the installer's dependency list as short as CLAUDE.md
/// asks (a dev-dependency would not reach the binary, but it would still be
/// one more pinned crate to keep current for ~15 lines of work).
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("pacenotes-{label}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create scratch dir");
        Scratch(path)
    }

    /// Place a game's marker executable, making this a folder for that game.
    /// Contents are irrelevant — the installer identifies a game by the
    /// presence of the file, exactly as the legacy C# did.
    fn with_marker(self, game: Game) -> Scratch {
        std::fs::write(self.0.join(game.marker()), b"").expect("write marker");
        self
    }

    fn path(&self) -> &str {
        self.0.to_str().expect("scratch path is valid utf-8")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The marker names are ported behavior, not free choices: a folder that the
/// legacy installer accepted must still be accepted here. `InstallationForm.cs`
/// checked `RichardBurnsRally_SSE.exe` and `dirtrally2.exe`, so renaming these
/// would silently reject every existing user's folder.
#[test]
fn markers_match_the_legacy_installer() {
    assert_eq!(Game::Rbr.marker(), "RichardBurnsRally_SSE.exe");
    assert_eq!(Game::DirtRally2.marker(), "dirtrally2.exe");
    assert_eq!(Game::AssettoCorsa.marker(), "assettocorsa.exe");
    assert_eq!(Game::BeamNg.marker(), "BeamNG.drive.exe");
}

/// The case the whole module exists for: a folder holding one game must be
/// accepted for that game and refused for the others. Without this, scene 3's
/// dropdown would be decorative and a user could overlay RBR pacenotes into
/// BeamNG.
#[test]
fn a_game_folder_is_accepted_only_for_that_game() {
    for owner in Game::ALL {
        let dir = Scratch::new("owned").with_marker(owner);
        for candidate in Game::ALL {
            let expected = if candidate == owner {
                DirStatus::Ok
            } else {
                DirStatus::NotGame
            };
            assert_eq!(
                validate(candidate, dir.path()),
                expected,
                "folder containing {} judged as {:?}'s folder",
                owner.marker(),
                candidate
            );
        }
    }
}

/// A real directory that simply is not a game install. This is the regression
/// guard: an existence-only check reports Ok here, which would let the
/// installer write a game overlay into an arbitrary folder.
#[test]
fn an_ordinary_directory_is_not_a_game_folder() {
    let dir = Scratch::new("ordinary");
    std::fs::write(dir.0.join("readme.txt"), b"not a game").expect("write decoy");
    for game in Game::ALL {
        assert_eq!(validate(game, dir.path()), DirStatus::NotGame);
    }
}

/// A folder holding a file *named like* another game's marker still is not
/// this game's folder — the check must be per-game, not "any .exe present".
#[test]
fn a_neighbouring_games_marker_does_not_qualify() {
    let dir = Scratch::new("neighbour").with_marker(Game::DirtRally2);
    assert_eq!(validate(Game::Rbr, dir.path()), DirStatus::NotGame);
    assert_eq!(validate(Game::DirtRally2, dir.path()), DirStatus::Ok);
}

/// Paths pasted from Explorer or a chat message carry whitespace; the folder
/// is still the right folder, so it must be accepted.
#[test]
fn surrounding_whitespace_is_tolerated() {
    let dir = Scratch::new("padded").with_marker(Game::Rbr);
    let padded = format!("  {}\t\r\n", dir.path());
    assert_eq!(validate(Game::Rbr, &padded), DirStatus::Ok);
}

/// A blank field is the app's start-up state, and needs a "tell me where it
/// is" prompt rather than a "that is wrong" error.
#[test]
fn a_blank_field_is_distinct_from_a_wrong_path() {
    for blank in ["", "   ", "\t", "\r\n"] {
        assert_eq!(validate(Game::Rbr, blank), DirStatus::Empty);
    }
}

/// A path the user mistyped, versus one that exists. These need different
/// messages, so they are different statuses.
#[test]
fn a_missing_path_reports_not_found() {
    let dir = Scratch::new("missing");
    let absent = dir.0.join("no-such-subfolder");
    assert_eq!(
        validate(Game::Rbr, absent.to_str().expect("utf-8")),
        DirStatus::NotFound
    );
}

/// Pointing at the game's executable instead of its folder is a natural
/// mistake; a file is not a folder and must not be treated as one.
#[test]
fn a_file_path_is_not_a_folder() {
    let dir = Scratch::new("filepath").with_marker(Game::Rbr);
    let exe = dir.0.join(Game::Rbr.marker());
    assert_eq!(
        validate(Game::Rbr, exe.to_str().expect("utf-8")),
        DirStatus::NotFound
    );
}

/// Scene 3's dropdown addresses games by index, so index/from_index must
/// agree for every game or picking one would select another.
#[test]
fn dropdown_indices_round_trip() {
    for game in Game::ALL {
        assert_eq!(Game::from_index(game.index()), game);
    }
    assert_eq!(Game::ALL[0], Game::Rbr, "RBR is listed first");
    // An out-of-range index must not panic in a dialog proc.
    assert_eq!(Game::from_index(Game::ALL.len()), Game::Rbr);
}

/// The game ids are the SAME values the server writes into
/// `PackageInfo::game`. If they drift, the Download scene filters every
/// package out of the tree and it silently comes up empty — no error.
#[test]
fn game_ids_are_the_shared_protocol_values() {
    assert_eq!(Game::Rbr.id(), pacenotes_shared::GAME_RBR);
    assert_eq!(Game::DirtRally2.id(), pacenotes_shared::GAME_DIRT_RALLY2);
    assert_eq!(Game::AssettoCorsa.id(), pacenotes_shared::GAME_ASSETTOCORSA);
    assert_eq!(Game::BeamNg.id(), pacenotes_shared::GAME_BEAMNG);

    // A manifest entry carrying that id must resolve to the matching game.
    for game in Game::ALL {
        assert_eq!(Game::from_id(game.id()), Some(game));
    }
}

/// `game` is an open string: a newer server may list a game this build does
/// not know. Those packages must be ignored, not shown under another game.
#[test]
fn an_unknown_game_id_matches_no_game() {
    for unknown in ["", "wrc", "rbr2", "RBR", " rbr"] {
        assert_eq!(
            Game::from_id(unknown),
            None,
            "id {unknown:?} matched a game"
        );
    }
}

/// A manifest written before the server went multi-game carries no `game`
/// field; shared defaults it to RBR, which must be a game we actually have.
#[test]
fn the_protocol_default_game_is_a_known_game() {
    assert_eq!(
        Game::from_id(&pacenotes_shared::default_game()),
        Some(Game::Rbr)
    );
}

/// Dropdown labels are what the user picks; they are proper nouns and must
/// not be translated or abbreviated.
#[test]
fn dropdown_labels_name_the_games() {
    assert_eq!(Game::Rbr.title(), "Richard Burns Rally");
    assert_eq!(Game::DirtRally2.title(), "Dirt Rally 2.0");
    assert_eq!(Game::AssettoCorsa.title(), "Assetto Corsa");
    assert_eq!(Game::BeamNg.title(), "BeamNG");
}

// ---- detect(): what drives the wizard's game dropdown (scene 3) ----

/// No marker: the dropdown stays on "(Detecting)" and Next stays disabled.
/// A real folder that is not a game is the common case here — the user has
/// browsed somewhere plausible but wrong.
#[test]
fn detect_finds_nothing_in_a_folder_that_is_not_a_game() {
    let dir = Scratch::new("detect-none");
    std::fs::write(dir.0.join("readme.txt"), b"not a game").expect("write decoy");
    assert_eq!(detect(dir.path()), Vec::new());
}

/// Exactly one marker: the dropdown shows that game and is disabled, because
/// there is nothing to choose.
#[test]
fn detect_finds_the_one_game_a_folder_holds() {
    for game in Game::ALL {
        let dir = Scratch::new("detect-one").with_marker(game);
        assert_eq!(
            detect(dir.path()),
            vec![game],
            "folder held {}",
            game.marker()
        );
    }
}

/// Several markers in one folder: the dropdown is enabled and lists exactly
/// the matches, in Game::ALL order so the list is stable between runs.
#[test]
fn detect_lists_every_game_present_in_all_order() {
    let dir = Scratch::new("detect-many")
        .with_marker(Game::BeamNg)
        .with_marker(Game::Rbr)
        .with_marker(Game::AssettoCorsa);
    assert_eq!(
        detect(dir.path()),
        vec![Game::Rbr, Game::AssettoCorsa, Game::BeamNg]
    );
}

/// Marker names come from publishers and third-party installers that do not
/// agree on case. NTFS would hide this on Windows, so an exact-case check
/// would pass the tests here and still fail a user whose folder holds
/// `DirtRally2.exe`.
#[test]
fn detect_ignores_the_case_of_the_executable_name() {
    for spelling in ["dirtrally2.exe", "DirtRally2.exe", "DIRTRALLY2.EXE"] {
        let dir = Scratch::new("detect-case");
        std::fs::write(dir.0.join(spelling), b"").expect("write marker");
        assert_eq!(
            detect(dir.path()),
            vec![Game::DirtRally2],
            "spelling {spelling} was not recognised"
        );
    }
}

/// A DIRECTORY named like a marker is not an executable. Game installs really
/// do contain folders with program-ish names, and treating one as a match
/// would send the whole run at the wrong game.
#[test]
fn detect_ignores_a_directory_named_like_a_marker() {
    let dir = Scratch::new("detect-dir");
    std::fs::create_dir(dir.0.join(Game::Rbr.marker())).expect("create decoy dir");
    assert_eq!(detect(dir.path()), Vec::new());
}

/// A path that is not there at all must report nothing rather than fail: the
/// field is scanned while the user is still typing it.
#[test]
fn detect_on_a_missing_or_blank_path_finds_nothing() {
    assert_eq!(detect(""), Vec::new());
    assert_eq!(detect("   "), Vec::new());
    let dir = Scratch::new("detect-missing");
    let absent = dir.0.join("no-such-subfolder");
    assert_eq!(detect(absent.to_str().expect("utf-8")), Vec::new());
}

/// detect() and validate() must agree, or scene 3 could offer a game that the
/// folder check then rejects.
#[test]
fn detect_and_validate_agree() {
    let dir = Scratch::new("agree").with_marker(Game::AssettoCorsa);
    let found = detect(dir.path());
    for game in Game::ALL {
        let expected = if found.contains(&game) {
            DirStatus::Ok
        } else {
            DirStatus::NotGame
        };
        assert_eq!(validate(game, dir.path()), expected, "{game:?}");
    }
}

/// Only `Ok` may unlock Install; every failure mode must keep it blocked.
#[test]
fn only_ok_permits_installing() {
    assert!(DirStatus::Ok.is_ok());
    for blocked in [DirStatus::Empty, DirStatus::NotFound, DirStatus::NotGame] {
        assert!(!blocked.is_ok(), "{blocked:?} must not permit installing");
    }
}
