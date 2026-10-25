//! Richard Burns Rally: the Pacenote plugin's settings file.

/// `/`-separated like a package path, since it is also what the ledger records.
pub const PACENOTE_SETTINGS: &str = "Plugins/Pacenote/PaceNote.ini";

/// Present only when the plugin is installed.
const PACENOTE_CONFIG_DIR: &str = "Plugins/Pacenote/config";

/// Each co-driver's audio sits under here at its path below its category.
const PACENOTE_SOUNDS: &str = "Plugins/Pacenote/sounds";

/// The plugin's documented defaults, written only when the file is absent and
/// then edited like a shipped one.
const TEMPLATE: &str = "[SETTINGS]\r\n\
     sounds=\r\n\
     language=english\r\n\
     replaySpeeds=0.005 0.01 0.03 0.06 0.125 0.25 0.5 0.75 1 1.25 1.5 1.75 2 4 6 8 10\r\n\
     enableGUI=1\r\n\
     showReplayHUD=1\r\n\
     muteReplayPacenotes=0\r\n";

const SECTION: &str = "SETTINGS";

pub fn configure(
    game_dir: &std::path::Path,
    picks: &std::collections::BTreeMap<String, String>,
) -> Result<super::Configured, super::ConfigureError> {
    if !crate::relative_path::join(game_dir, PACENOTE_CONFIG_DIR).is_dir() {
        return Err(super::ConfigureError::PluginMissing);
    }

    let mut outcome = super::Configured::default();
    let path = crate::relative_path::join(game_dir, PACENOTE_SETTINGS);
    let shipped = std::fs::read_to_string(&path).ok();
    if shipped.is_none() {
        outcome.notes.push(format!(
            "{PACENOTE_SETTINGS} was missing; writing a default one"
        ));
    }
    let original = shipped.as_deref().unwrap_or(TEMPLATE);

    let mut text = original.to_string();
    if let Some(language) = picks
        .get(pacenotes_shared::CATEGORY_LANGUAGE)
        .and_then(|key| leaf_of(key))
    {
        text = super::ini::set_ini_key(&text, SECTION, "language", language);
    }
    if let Some(sounds) = picks
        .get(pacenotes_shared::CATEGORY_CODRIVER)
        .and_then(|key| below_category(key))
    {
        // A value naming a missing folder silences the co-driver, so the package's own stays.
        let audio = crate::relative_path::join(game_dir, &format!("{PACENOTE_SOUNDS}/{sounds}"));
        if audio.is_dir() {
            text = super::ini::set_ini_key(&text, SECTION, "sounds", sounds);
        } else {
            outcome.notes.push(format!(
                "no audio folder at {PACENOTE_SOUNDS}/{sounds}; leaving the sounds setting \
                 as the package shipped it"
            ));
        }
    }

    if shipped.as_deref() == Some(text.as_str()) {
        return Ok(outcome);
    }
    write_settings(&path, &text)?;
    outcome.files.push(PACENOTE_SETTINGS.to_string());
    Ok(outcome)
}

fn write_settings(path: &std::path::Path, text: &str) -> Result<(), super::ConfigureError> {
    let io_error = |doing: String, e: std::io::Error| super::ConfigureError::Io {
        doing,
        reason: e.to_string(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| io_error(format!("creating {}", parent.display()), e))?;
    }
    std::fs::write(path, text.as_bytes())
        .map_err(|e| io_error(format!("writing {}", path.display()), e))
}

/// `rbr/public/language/polish` → `polish`, the plugin's own folder name.
fn leaf_of(key: &str) -> Option<&str> {
    let (_, _, name) = pacenotes_shared::split_download_path(key)?;
    name.rsplit('/').next()
}

/// `rbr/public/codriver/polish/male/Kamil` → `polish/male/Kamil`, the value the
/// voice packages write themselves.
fn below_category(key: &str) -> Option<&str> {
    let (_, _, name) = pacenotes_shared::split_download_path(key)?;
    let (_, rest) = name.split_once('/')?;
    Some(rest)
}

#[cfg(test)]
mod tests;
