//! Every fixture is a folder a user could really point at: one holding a
//! game's executable, another game's, or neither.

fn scratch_with_markers(label: &str, games: &[super::Game]) -> crate::scratch_dir::ScratchDir {
    let dir = crate::scratch_dir::ScratchDir::new(label);
    for game in games {
        std::fs::write(dir.0.join(game.marker()), b"").expect("write marker");
    }
    dir
}

/// Renaming a marker would reject every folder the legacy installer accepted.
#[test]
fn markers_match_the_legacy_installer() {
    assert_eq!(super::Game::Rbr.marker(), "RichardBurnsRally_SSE.exe");
    assert_eq!(super::Game::DirtRally2.marker(), "dirtrally2.exe");
    assert_eq!(super::Game::AssettoCorsa.marker(), "assettocorsa.exe");
    assert_eq!(super::Game::BeamNg.marker(), "BeamNG.drive.exe");
}

#[test]
fn a_game_folder_is_accepted_only_for_that_game() {
    for owner in super::Game::ALL {
        let dir = scratch_with_markers("owned", &[owner]);
        for candidate in super::Game::ALL {
            let expected = if candidate == owner {
                super::DirStatus::Ok
            } else {
                super::DirStatus::NotGame
            };
            assert_eq!(
                super::validate(candidate, dir.path()),
                expected,
                "folder containing {} judged as {candidate:?}'s folder",
                owner.marker()
            );
        }
    }
}

/// An existence-only check would accept this and overlay a game into an arbitrary folder.
#[test]
fn an_ordinary_directory_is_not_a_game_folder() {
    let dir = crate::scratch_dir::ScratchDir::new("ordinary");
    std::fs::write(dir.0.join("readme.txt"), b"not a game").expect("write decoy");
    for game in super::Game::ALL {
        assert_eq!(super::validate(game, dir.path()), super::DirStatus::NotGame);
    }
}

#[test]
fn a_neighbouring_games_marker_does_not_qualify() {
    let dir = scratch_with_markers("neighbour", &[super::Game::DirtRally2]);
    assert_eq!(
        super::validate(super::Game::Rbr, dir.path()),
        super::DirStatus::NotGame
    );
    assert_eq!(
        super::validate(super::Game::DirtRally2, dir.path()),
        super::DirStatus::Ok
    );
}

#[test]
fn surrounding_whitespace_is_tolerated() {
    let dir = scratch_with_markers("padded", &[super::Game::Rbr]);
    let padded = format!("  {}\t\r\n", dir.path());
    assert_eq!(
        super::validate(super::Game::Rbr, &padded),
        super::DirStatus::Ok
    );
}

#[test]
fn a_blank_field_is_distinct_from_a_wrong_path() {
    for blank in ["", "   ", "\t", "\r\n"] {
        assert_eq!(
            super::validate(super::Game::Rbr, blank),
            super::DirStatus::Empty
        );
    }
}

#[test]
fn a_missing_path_reports_not_found() {
    let dir = crate::scratch_dir::ScratchDir::new("missing");
    let absent = dir.0.join("no-such-subfolder");
    assert_eq!(
        super::validate(super::Game::Rbr, absent.to_str().expect("utf-8")),
        super::DirStatus::NotFound
    );
}

#[test]
fn a_file_path_is_not_a_folder() {
    let dir = scratch_with_markers("filepath", &[super::Game::Rbr]);
    let executable = dir.0.join(super::Game::Rbr.marker());
    assert_eq!(
        super::validate(super::Game::Rbr, executable.to_str().expect("utf-8")),
        super::DirStatus::NotFound
    );
}

#[test]
fn dropdown_indices_round_trip() {
    for game in super::Game::ALL {
        assert_eq!(super::Game::from_index(game.index()), game);
    }
    assert_eq!(super::Game::ALL[0], super::Game::Rbr);
    assert_eq!(
        super::Game::from_index(super::Game::ALL.len()),
        super::Game::Rbr
    );
}

/// A drift here would filter every package out of the Download tree without an error.
#[test]
fn game_ids_are_the_shared_protocol_values() {
    assert_eq!(super::Game::Rbr.id(), pacenotes_shared::GAME_RBR);
    assert_eq!(
        super::Game::DirtRally2.id(),
        pacenotes_shared::GAME_DIRT_RALLY2
    );
    assert_eq!(
        super::Game::AssettoCorsa.id(),
        pacenotes_shared::GAME_ASSETTOCORSA
    );
    assert_eq!(super::Game::BeamNg.id(), pacenotes_shared::GAME_BEAMNG);
    for game in super::Game::ALL {
        assert_eq!(super::Game::from_id(game.id()), Some(game));
    }
}

#[test]
fn an_unknown_game_id_matches_no_game() {
    for unknown in ["", "wrc", "rbr2", "RBR", " rbr"] {
        assert_eq!(super::Game::from_id(unknown), None, "{unknown:?}");
    }
}

#[test]
fn the_protocol_default_game_is_a_known_game() {
    assert_eq!(
        super::Game::from_id(&pacenotes_shared::default_game()),
        Some(super::Game::Rbr)
    );
}

#[test]
fn dropdown_labels_name_the_games() {
    assert_eq!(super::Game::Rbr.title(), "Richard Burns Rally");
    assert_eq!(super::Game::DirtRally2.title(), "Dirt Rally 2.0");
    assert_eq!(super::Game::AssettoCorsa.title(), "Assetto Corsa");
    assert_eq!(super::Game::BeamNg.title(), "BeamNG");
}

#[test]
fn detect_finds_nothing_in_a_folder_that_is_not_a_game() {
    let dir = crate::scratch_dir::ScratchDir::new("detect-none");
    std::fs::write(dir.0.join("readme.txt"), b"not a game").expect("write decoy");
    assert_eq!(super::detect(dir.path()), Vec::new());
}

#[test]
fn detect_finds_the_one_game_a_folder_holds() {
    for game in super::Game::ALL {
        let dir = scratch_with_markers("detect-one", &[game]);
        assert_eq!(super::detect(dir.path()), vec![game], "{}", game.marker());
    }
}

#[test]
fn detect_lists_every_game_present_in_all_order() {
    let dir = scratch_with_markers(
        "detect-many",
        &[
            super::Game::BeamNg,
            super::Game::Rbr,
            super::Game::AssettoCorsa,
        ],
    );
    assert_eq!(
        super::detect(dir.path()),
        vec![
            super::Game::Rbr,
            super::Game::AssettoCorsa,
            super::Game::BeamNg
        ]
    );
}

#[test]
fn detect_ignores_the_case_of_the_executable_name() {
    for spelling in ["dirtrally2.exe", "DirtRally2.exe", "DIRTRALLY2.EXE"] {
        let dir = crate::scratch_dir::ScratchDir::new("detect-case");
        std::fs::write(dir.0.join(spelling), b"").expect("write marker");
        assert_eq!(
            super::detect(dir.path()),
            vec![super::Game::DirtRally2],
            "{spelling}"
        );
    }
}

#[test]
fn detect_ignores_a_directory_named_like_a_marker() {
    let dir = crate::scratch_dir::ScratchDir::new("detect-dir");
    std::fs::create_dir(dir.0.join(super::Game::Rbr.marker())).expect("create decoy dir");
    assert_eq!(super::detect(dir.path()), Vec::new());
}

/// The field is scanned while the user is still typing it.
#[test]
fn detect_on_a_missing_or_blank_path_finds_nothing() {
    assert_eq!(super::detect(""), Vec::new());
    assert_eq!(super::detect("   "), Vec::new());
    let dir = crate::scratch_dir::ScratchDir::new("detect-missing");
    let absent = dir.0.join("no-such-subfolder");
    assert_eq!(super::detect(absent.to_str().expect("utf-8")), Vec::new());
}

#[test]
fn detect_and_validate_agree() {
    let dir = scratch_with_markers("agree", &[super::Game::AssettoCorsa]);
    let found = super::detect(dir.path());
    for game in super::Game::ALL {
        let expected = if found.contains(&game) {
            super::DirStatus::Ok
        } else {
            super::DirStatus::NotGame
        };
        assert_eq!(super::validate(game, dir.path()), expected, "{game:?}");
    }
}

#[test]
fn only_ok_permits_installing() {
    assert!(super::DirStatus::Ok.is_ok());
    for blocked in [
        super::DirStatus::Empty,
        super::DirStatus::NotFound,
        super::DirStatus::NotGame,
    ] {
        assert!(!blocked.is_ok(), "{blocked:?}");
    }
}
