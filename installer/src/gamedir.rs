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

/// A game the installer can target. The ids are `pacenotes_shared::GAME_*`
/// and the markers are from `installer/DESIGN.md` rev. 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Game {
    Rbr,
    DirtRally2,
    AssettoCorsa,
    BeamNg,
}

impl Game {
    pub const ALL: [Game; 4] = [
        Game::Rbr,
        Game::DirtRally2,
        Game::AssettoCorsa,
        Game::BeamNg,
    ];

    /// Shown in the wizard's game dropdown. A proper noun, untranslated.
    pub fn title(self) -> &'static str {
        match self {
            Game::Rbr => "Richard Burns Rally",
            Game::DirtRally2 => "Dirt Rally 2.0",
            Game::AssettoCorsa => "Assetto Corsa",
            Game::BeamNg => "BeamNG",
        }
    }

    /// The executable whose presence identifies this game's install folder.
    /// RBR and Dirt Rally keep the names the legacy installer checked, so a
    /// folder that worked there still works here.
    pub fn marker(self) -> &'static str {
        match self {
            Game::Rbr => "RichardBurnsRally_SSE.exe",
            Game::DirtRally2 => "dirtrally2.exe",
            Game::AssettoCorsa => "assettocorsa.exe",
            Game::BeamNg => "BeamNG.drive.exe",
        }
    }

    /// Wire identifier: the value the server puts in `PackageInfo::game`.
    ///
    /// These ARE `pacenotes_shared::GAME_*` rather than copies of them. The
    /// shared crate documents that the two sides match; deferring to it is
    /// what makes that true. The Download scene filters the tree by this
    /// value, so a drift would silently show an empty tree.
    pub fn id(self) -> &'static str {
        match self {
            Game::Rbr => pacenotes_shared::GAME_RBR,
            Game::DirtRally2 => pacenotes_shared::GAME_DIRT_RALLY2,
            Game::AssettoCorsa => pacenotes_shared::GAME_ASSETTOCORSA,
            Game::BeamNg => pacenotes_shared::GAME_BEAMNG,
        }
    }

    /// Game for a manifest's `game` value. `None` for an id this build does
    /// not know: `game` is an open string, so a newer server may list packages
    /// for a game with no entry here, and those must be ignored rather than
    /// shown under the wrong one.
    pub fn from_id(id: &str) -> Option<Game> {
        Game::ALL.iter().copied().find(|game| game.id() == id)
    }

    /// Index in `ALL`, i.e. the order the dropdown lists games in.
    pub fn index(self) -> usize {
        Game::ALL.iter().position(|&g| g == self).unwrap_or(0)
    }

    /// Game for an index into `ALL`; out of range falls back to the first.
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
    /// executable.
    NotGame,
    /// The directory holds this game.
    Ok,
}

impl DirStatus {
    pub fn is_ok(self) -> bool {
        self == DirStatus::Ok
    }
}

/// Which games `dir` looks like an install of, in `Game::ALL` order.
///
/// This drives the wizard's game dropdown (DESIGN.md rev. 2 scene 3): empty →
/// "(Detecting)" placeholder and Next stays disabled, one → that game, shown
/// disabled, several → the dropdown is enabled listing exactly these.
///
/// Matching is case-insensitive and done by listing the directory once, not by
/// probing four paths. NTFS would hide a case mismatch on Windows, but the
/// marker names come from publishers and installers that do not all agree on
/// case, and the cross-platform tests would otherwise be testing something
/// weaker than what ships.
pub fn detect(dir: &str) -> Vec<Game> {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let entries = match std::fs::read_dir(trimmed) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut present: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        // Only files: a DIRECTORY called dirtrally2.exe is not an executable.
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            present.push(entry.file_name().to_string_lossy().to_lowercase());
        }
    }

    Game::ALL
        .iter()
        .copied()
        .filter(|game| present.iter().any(|f| f == &game.marker().to_lowercase()))
        .collect()
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
    if !std::path::Path::new(trimmed).is_dir() {
        return DirStatus::NotFound;
    }
    if detect(trimmed).contains(&game) {
        DirStatus::Ok
    } else {
        DirStatus::NotGame
    }
}
