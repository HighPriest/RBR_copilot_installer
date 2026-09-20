//! AUTOMATED TEST for `pacenotes_installer::gamedir`.
//!
//! Cross-platform: pure std, no Win32, so this runs in CI on any host.
//!
//! Every case below builds a fixture that represents a REAL situation the user
//! can be in — a folder holding a specific game, a folder holding a different
//! game, a folder holding neither — and asserts the exact status for it. The
//! point of the game-folder field is to tell those apart, so "some directory
//! exists" is not a meaningful input and does not appear here.

use pacenotes_installer::gamedir::{validate, DirStatus, Game};

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
    assert_eq!(Game::DirtRally.marker(), "dirtrally2.exe");
    assert_eq!(Game::BeamNg.marker(), "BeamNG.drive.exe");
}

/// The case the whole module exists for: a folder holding one game must be
/// accepted for that game and refused for the others. Without this, the ribbon
/// tab would be decorative and a user could overlay RBR pacenotes into BeamNG.
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
    let dir = Scratch::new("neighbour").with_marker(Game::DirtRally);
    assert_eq!(validate(Game::Rbr, dir.path()), DirStatus::NotGame);
    assert_eq!(validate(Game::DirtRally, dir.path()), DirStatus::Ok);
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

/// The ribbon addresses games by tab index, so index/from_index must agree for
/// every game or a tab switch would edit another game's folder.
#[test]
fn ribbon_indices_round_trip() {
    for game in Game::ALL {
        assert_eq!(Game::from_index(game.index()), game);
    }
    assert_eq!(Game::ALL[0], Game::Rbr, "ribbon opens on RBR");
    // An out-of-range tab index must not panic in a dialog proc.
    assert_eq!(Game::from_index(Game::ALL.len()), Game::Rbr);
}

/// The ribbon's game ids are the SAME values the server writes into
/// `PackageInfo::game`. If they drift, task I5 filters every package out of
/// its tab and the lists silently come up empty — no error, just nothing.
#[test]
fn game_ids_are_the_shared_protocol_values() {
    assert_eq!(Game::Rbr.id(), pacenotes_shared::GAME_RBR);
    assert_eq!(Game::DirtRally.id(), pacenotes_shared::GAME_DIRT_RALLY);
    assert_eq!(Game::BeamNg.id(), pacenotes_shared::GAME_BEAMNG);

    // A manifest entry carrying that id must land on the matching tab.
    for game in Game::ALL {
        assert_eq!(Game::from_id(game.id()), Some(game));
    }
}

/// `game` is an open string: a newer server may list a game this build has no
/// tab for. Those packages must be ignored, not shown under some other game.
#[test]
fn an_unknown_game_id_belongs_to_no_tab() {
    for unknown in ["", "wrc", "rbr2", "RBR", " rbr"] {
        assert_eq!(Game::from_id(unknown), None, "id {unknown:?} matched a tab");
    }
}

/// A manifest written before the server went multi-game carries no `game`
/// field; shared defaults it to RBR, which must be a tab we actually have.
#[test]
fn the_protocol_default_game_has_a_tab() {
    assert_eq!(
        Game::from_id(&pacenotes_shared::default_game()),
        Some(Game::Rbr)
    );
}

/// Ribbon labels are what the user clicks; they are proper nouns and must not
/// be translated or abbreviated.
#[test]
fn ribbon_labels_name_the_games() {
    assert_eq!(Game::Rbr.title(), "Richard Burns Rally");
    assert_eq!(Game::DirtRally.title(), "Dirt Rally");
    assert_eq!(Game::BeamNg.title(), "BeamNG");
}

/// Only `Ok` may unlock Install; every failure mode must keep it blocked.
#[test]
fn only_ok_permits_installing() {
    assert!(DirStatus::Ok.is_ok());
    for blocked in [DirStatus::Empty, DirStatus::NotFound, DirStatus::NotGame] {
        assert!(!blocked.is_ok(), "{blocked:?} must not permit installing");
    }
}
