//! Game ids, as used in `installer/DESIGN.md` rev. 2 and `installer/src/gamedir.rs`.
//! The set is open: a new game needs no protocol bump.

pub const GAME_RBR: &str = "rbr";
pub const GAME_DIRT_RALLY2: &str = "dirtrally2";
pub const GAME_ASSETTOCORSA: &str = "assettocorsa";
pub const GAME_BEAMNG: &str = "beamng";

/// Lets the server spot a game id filed where something else belongs.
pub const KNOWN_GAMES: [&str; 4] = [GAME_RBR, GAME_DIRT_RALLY2, GAME_ASSETTOCORSA, GAME_BEAMNG];

/// The game of a package from a manifest written before games existed, which listed RBR only.
pub fn default_game() -> String {
    GAME_RBR.to_string()
}
