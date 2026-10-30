//! The games the installer targets, and the executable that makes a folder one
//! of them, as the legacy `InstallationForm.cs` checked.

/// A game the installer can target, as listed in `installer/DESIGN.md` rev. 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Game {
    Rbr,
    DirtRally2,
    AssettoCorsa,
    BeamNg,
}

impl Game {
    /// In dropdown order.
    pub const ALL: [Game; 4] = [
        Game::Rbr,
        Game::DirtRally2,
        Game::AssettoCorsa,
        Game::BeamNg,
    ];

    /// A proper noun, never translated.
    pub fn title(self) -> &'static str {
        match self {
            Game::Rbr => "Richard Burns Rally",
            Game::DirtRally2 => "Dirt Rally 2.0",
            Game::AssettoCorsa => "Assetto Corsa",
            Game::BeamNg => "BeamNG",
        }
    }

    /// The executable identifying this game's folder; RBR and Dirt Rally keep
    /// the legacy names so every folder that worked there still works.
    pub fn marker(self) -> &'static str {
        match self {
            Game::Rbr => "RichardBurnsRally_SSE.exe",
            Game::DirtRally2 => "dirtrally2.exe",
            Game::AssettoCorsa => "assettocorsa.exe",
            Game::BeamNg => "BeamNG.drive.exe",
        }
    }

    /// The `PackageInfo::game` value the server writes for this game.
    pub fn id(self) -> &'static str {
        match self {
            Game::Rbr => pacenotes_shared::GAME_RBR,
            Game::DirtRally2 => pacenotes_shared::GAME_DIRT_RALLY2,
            Game::AssettoCorsa => pacenotes_shared::GAME_ASSETTOCORSA,
            Game::BeamNg => pacenotes_shared::GAME_BEAMNG,
        }
    }

    /// `None` for an id this build does not know, whose packages are then ignored.
    pub fn from_id(id: &str) -> Option<Game> {
        Game::ALL.iter().copied().find(|game| game.id() == id)
    }

    pub fn index(self) -> usize {
        Game::ALL.iter().position(|&game| game == self).unwrap_or(0)
    }

    /// Out of range falls back to the first game.
    pub fn from_index(index: usize) -> Game {
        Game::ALL.get(index).copied().unwrap_or(Game::Rbr)
    }
}

/// The game-folder check's outcome; each needs a different fix from the user.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirStatus {
    Empty,
    /// Missing, or a file rather than a directory.
    NotFound,
    /// A directory without this game's executable.
    NotGame,
    Ok,
}

impl DirStatus {
    pub fn is_ok(self) -> bool {
        self == DirStatus::Ok
    }
}

/// The games `dir` holds, in `Game::ALL` order.
///
/// Matches executable names case-insensitively, because publishers disagree
/// on case and NTFS would hide a mismatch only on Windows.
pub fn detect(dir: &str) -> Vec<Game> {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(trimmed) else {
        return Vec::new();
    };

    let file_names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect();

    Game::ALL
        .iter()
        .copied()
        .filter(|game| file_names.contains(&game.marker().to_lowercase()))
        .collect()
}

/// Ignores surrounding whitespace, which pasted paths routinely carry.
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

#[cfg(test)]
mod tests;
