//! `SHIPPED` is byte for byte the `PaceNote.ini` the live
//! `codriver/polish/male/KamilFilipek` package puts in a game folder.

const SHIPPED: &str = "[SETTINGS]\r\n\
     sounds=polish/male/KamilFilipek\r\n\
     ;language=english\r\n\
     language=polish\r\n\
     replaySpeeds=0.005 0.01 0.03 0.06 0.125 0.25 0.5 0.75 1 1.25 1.5 1.75 2 4 6 8 10\r\n\
     enableGUI=1\r\n\
     showReplayHUD=1\r\n\
     muteReplayPacenotes=1\r\n";

const KAMIL: &str = "rbr/public/codriver/polish/male/KamilFilipek";
const KAMIL_AUDIO: &str = "polish/male/KamilFilipek";
const SOMEONE: &str = "rbr/public/codriver/czech/male/Someone";
const ENGLISH: &str = "rbr/public/language/english";

/// A game folder with the plugin installed, and the voice's audio folder when given.
fn game_with_plugin(label: &str, audio: Option<&str>) -> crate::scratch_dir::ScratchDir {
    let game = crate::scratch_dir::ScratchDir::new(label);
    std::fs::create_dir_all(crate::relative_path::join(
        &game.0,
        super::PACENOTE_CONFIG_DIR,
    ))
    .expect("plugin config dir");
    if let Some(audio) = audio {
        let sounds = format!("{}/{audio}", super::PACENOTE_SOUNDS);
        std::fs::create_dir_all(crate::relative_path::join(&game.0, &sounds)).expect("audio dir");
    }
    game
}

fn settings_path(game: &crate::scratch_dir::ScratchDir) -> std::path::PathBuf {
    crate::relative_path::join(&game.0, super::PACENOTE_SETTINGS)
}

fn ship_settings(game: &crate::scratch_dir::ScratchDir) {
    std::fs::write(settings_path(game), SHIPPED).expect("write the shipped settings");
}

fn settings(game: &crate::scratch_dir::ScratchDir) -> String {
    std::fs::read_to_string(settings_path(game)).expect("the settings file")
}

fn picks(entries: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    entries
        .iter()
        .map(|(category, key)| (category.to_string(), key.to_string()))
        .collect()
}

fn english_with_kamil() -> std::collections::BTreeMap<String, String> {
    picks(&[
        (pacenotes_shared::CATEGORY_LANGUAGE, ENGLISH),
        (pacenotes_shared::CATEGORY_CODRIVER, KAMIL),
    ])
}

/// English picked with a Polish voice, whose own file says Polish.
#[test]
fn the_language_pick_changes_one_line_and_nothing_else() {
    let game = game_with_plugin("rbr-language", Some(KAMIL_AUDIO));
    ship_settings(&game);

    let done = super::configure(&game.0, &english_with_kamil()).expect("configured");

    assert_eq!(done.files, vec![super::PACENOTE_SETTINGS.to_string()]);
    assert_eq!(
        settings(&game),
        SHIPPED.replace("language=polish", "language=english")
    );
}

/// What a regenerated file would have lost, which is why the step edits instead.
#[test]
fn what_the_package_author_wrote_survives() {
    let game = game_with_plugin("rbr-author", Some(KAMIL_AUDIO));
    ship_settings(&game);
    super::configure(
        &game.0,
        &picks(&[(pacenotes_shared::CATEGORY_LANGUAGE, ENGLISH)]),
    )
    .expect("configured");

    let after = settings(&game);
    assert!(
        after.contains("muteReplayPacenotes=1"),
        "not the plugin default"
    );
    assert!(after.contains(";language=english"));
    assert!(after.contains("replaySpeeds=0.005 0.01 0.03"));
    assert!(after.ends_with("\r\n"));
    assert_eq!(after.matches("language=").count(), 2);
}

#[test]
fn the_voice_pick_sets_the_sounds_path() {
    let game = game_with_plugin("rbr-voice", Some("czech/male/Someone"));
    ship_settings(&game);

    super::configure(
        &game.0,
        &picks(&[(pacenotes_shared::CATEGORY_CODRIVER, SOMEONE)]),
    )
    .expect("configured");

    assert!(settings(&game).contains("sounds=czech/male/Someone"));
}

#[test]
fn a_sounds_path_with_no_audio_folder_is_not_written() {
    let game = game_with_plugin("rbr-no-audio", None);
    ship_settings(&game);

    let done = super::configure(
        &game.0,
        &picks(&[(pacenotes_shared::CATEGORY_CODRIVER, SOMEONE)]),
    )
    .expect("configured");

    assert!(settings(&game).contains("sounds=polish/male/KamilFilipek"));
    assert!(done.files.is_empty());
    assert!(
        done.notes
            .iter()
            .any(|note| note.contains("no audio folder")),
        "{:?}",
        done.notes
    );
}

#[test]
fn a_missing_settings_file_is_written_from_the_template() {
    let game = game_with_plugin("rbr-template", Some(KAMIL_AUDIO));

    let done = super::configure(&game.0, &english_with_kamil()).expect("configured");

    assert_eq!(done.files, vec![super::PACENOTE_SETTINGS.to_string()]);
    let written = settings(&game);
    assert!(written.contains("language=english"));
    assert!(written.contains("sounds=polish/male/KamilFilipek"));
    assert!(written.contains("enableGUI=1"));
    assert!(
        done.notes.iter().any(|note| note.contains("was missing")),
        "{:?}",
        done.notes
    );
}

#[test]
fn no_plugin_means_nothing_is_written_and_the_reason_is_named() {
    let game = crate::scratch_dir::ScratchDir::new("rbr-no-plugin");

    let error = super::configure(
        &game.0,
        &picks(&[(pacenotes_shared::CATEGORY_LANGUAGE, ENGLISH)]),
    )
    .expect_err("there is no plugin here");

    assert_eq!(error, crate::install::ConfigureError::PluginMissing);
    assert!(error.to_string().contains("RSF"), "{error}");
    assert!(!game.0.join("Plugins").exists());
}

/// A second write would take a restore point and churn the ledger for nothing.
#[test]
fn configuring_twice_writes_nothing_the_second_time() {
    let game = game_with_plugin("rbr-twice", Some(KAMIL_AUDIO));
    ship_settings(&game);

    super::configure(&game.0, &english_with_kamil()).expect("first");
    let after_first = settings(&game);
    let second = super::configure(&game.0, &english_with_kamil()).expect("second");

    assert!(second.files.is_empty());
    assert_eq!(settings(&game), after_first);
}
