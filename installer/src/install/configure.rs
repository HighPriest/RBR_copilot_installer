#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigureError {
    /// The game is there without its plugin; the caller shows this localized.
    PluginMissing,
    Io {
        doing: String,
        reason: String,
    },
}

impl std::fmt::Display for ConfigureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigureError::PluginMissing => write!(
                formatter,
                "the Pacenote plugin is not installed in this folder, so there is \
                 nothing to configure. Install it from the RSF installer first."
            ),
            ConfigureError::Io { doing, reason } => write!(formatter, "{doing}: {reason}"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Configured {
    /// Written or changed, `/`-separated, for the ledger.
    pub files: Vec<String>,
    /// For the run log; none of them fails the step.
    pub notes: Vec<String>,
}

/// Runs the game's own step; `picks` maps a category to the chosen package's
/// `game/directory/name`, as scene 5 produces it.
pub fn configure(
    game: crate::gamedir::Game,
    game_dir: &std::path::Path,
    picks: &std::collections::BTreeMap<String, String>,
) -> Result<Configured, ConfigureError> {
    match game {
        crate::gamedir::Game::Rbr => super::rbr::configure(game_dir, picks),
        crate::gamedir::Game::DirtRally2
        | crate::gamedir::Game::AssettoCorsa
        | crate::gamedir::Game::BeamNg => Ok(Configured::default()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_game_with_no_step_leaves_its_folder_untouched() {
        for game in [
            crate::gamedir::Game::DirtRally2,
            crate::gamedir::Game::AssettoCorsa,
            crate::gamedir::Game::BeamNg,
        ] {
            let folder = crate::scratch_dir::ScratchDir::new("configure-other");
            let picks = [(
                pacenotes_shared::CATEGORY_LANGUAGE.to_string(),
                "rbr/public/language/english".to_string(),
            )]
            .into();
            let done = super::configure(game, &folder.0, &picks).expect("no step is not an error");
            assert_eq!(done, super::Configured::default());
            assert_eq!(
                std::fs::read_dir(&folder.0).expect("scratch").count(),
                0,
                "{game:?}"
            );
        }
    }
}
