//! Which games the installer targets, and what makes a folder actually be one
//! of them.
//!
//! A "game folder" is NOT just any directory that happens to exist. It is the
//! directory holding that specific game's executable, and each game has a
//! different one. The legacy C# checked exactly this
//! (InstallationForm.cs: `File.Exists(Path.Combine(selected, "RichardBurnsRally_SSE.exe"))`
//! and siblings), and users depend on being told when they have pointed the
//! installer at the wrong folder — an existence-only check would happily
//! accept `C:\Windows` and then overlay pacenotes into it.
//!
//! Pure std, no Win32: this is the half of the folder field that is worth
//! testing, so it lives in the lib target and is exercised cross-platform by
//! tests/gamedir.rs.

/// A game the installer can target. Ribbon order is `ALL`'s order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Game {
    Rbr,
    DirtRally,
    BeamNg,
}

impl Game {
    pub const ALL: [Game; 3] = [Game::Rbr, Game::DirtRally, Game::BeamNg];

    /// Ribbon label. A proper noun, deliberately untranslated.
    pub fn title(self) -> &'static str {
        match self {
            Game::Rbr => "Richard Burns Rally",
            Game::DirtRally => "Dirt Rally",
            Game::BeamNg => "BeamNG",
        }
    }

    /// The executable whose presence identifies this game's install folder.
    /// RBR and Dirt Rally keep the names the legacy installer checked, so a
    /// folder that worked there still works here.
    pub fn marker(self) -> &'static str {
        match self {
            Game::Rbr => "RichardBurnsRally_SSE.exe",
            Game::DirtRally => "dirtrally2.exe",
            Game::BeamNg => "BeamNG.drive.exe",
        }
    }

    /// Wire identifier: the value the server puts in `PackageInfo::game`.
    ///
    /// These ARE `pacenotes_shared::GAME_*` rather than copies of them. The
    /// shared crate documents that the two sides match; deferring to it is
    /// what makes that true. Task I5 filters each ribbon tab's lists by this
    /// value, so a drift would silently empty a tab.
    pub fn id(self) -> &'static str {
        match self {
            Game::Rbr => pacenotes_shared::GAME_RBR,
            Game::DirtRally => pacenotes_shared::GAME_DIRT_RALLY,
            Game::BeamNg => pacenotes_shared::GAME_BEAMNG,
        }
    }

    /// Game for a manifest's `game` value. `None` for an id this build does
    /// not know: `game` is an open string, so a newer server may list packages
    /// for a game with no tab here, and those must be ignored rather than
    /// dumped into the wrong one.
    pub fn from_id(id: &str) -> Option<Game> {
        Game::ALL.iter().copied().find(|game| game.id() == id)
    }

    /// Index in `ALL`, i.e. the ribbon tab index.
    pub fn index(self) -> usize {
        Game::ALL.iter().position(|&g| g == self).unwrap_or(0)
    }

    /// Game for a ribbon tab index; out-of-range falls back to the first.
    pub fn from_index(index: usize) -> Game {
        Game::ALL.get(index).copied().unwrap_or(Game::Rbr)
    }
}

/// Outcome of checking the game-folder field. Each variant maps to a distinct
/// message, because "you typed nothing", "that path is not there" and "that is
/// a real folder but not this game" need different fixes from the user.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirStatus {
    /// Field is blank.
    Empty,
    /// The path does not exist, or is a file rather than a directory.
    NotFound,
    /// The directory exists but does not contain this game's marker
    /// executable. Carries the marker so the message can name it.
    NotGame,
    /// The directory holds this game.
    Ok,
}

impl DirStatus {
    pub fn is_ok(self) -> bool {
        self == DirStatus::Ok
    }
}

/// Validate `dir` as `game`'s install folder.
///
/// Surrounding whitespace is trimmed: paths pasted from a file manager or a
/// chat message routinely carry it, and failing on that would be unhelpful.
pub fn validate(game: Game, dir: &str) -> DirStatus {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return DirStatus::Empty;
    }
    let path = std::path::Path::new(trimmed);
    if !path.is_dir() {
        return DirStatus::NotFound;
    }
    if !path.join(game.marker()).is_file() {
        return DirStatus::NotGame;
    }
    DirStatus::Ok
}
