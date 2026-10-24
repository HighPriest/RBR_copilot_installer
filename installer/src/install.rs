//! The per-game configuration step: what a game needs done to its own files
//! once the packages are on disk. No shared abstraction on purpose, since the
//! games share no file layout; the one contract is that a step reports every
//! file it wrote, so the engine ledgers and backs those up like extracted ones.
//!
//! RBR's step edits `language` and `sounds` in the co-driver's own
//! `PaceNote.ini` and keeps every other line, because the packages now ship
//! the files the legacy `CreateRBRConfiguration` generated.
//!
//! TODO(I7):
//! - `configure(game, game_dir, &Configuration) -> Result<Vec<String>, String>`,
//!   dispatching on the game and returning `/`-separated paths for the ledger
//! - `mod rbr`: set the two keys, write from a template when the file is
//!   absent, and report "install the Pacenote plugin first" when
//!   `PACENOTE_CONFIG_DIR` is missing
//! - `sounds` is the voice's path below its category
//!   (`codriver/polish/male/KamilFilipek` → `polish/male/KamilFilipek`), kept
//!   as shipped and logged when `Plugins\Pacenote\sounds\<that path>` is absent

/// `(percent, status)`.
pub type Report<'a> = &'a mut dyn FnMut(u8, &str);

/// Windows separators are intentional: RBR expects them.
pub const PACENOTE_CONFIG_DIR: &str = "Plugins\\Pacenote\\config";

/// The plugin's settings, shipped by the co-driver packages and edited in place.
pub const PACENOTE_SETTINGS: &str = "Plugins\\Pacenote\\PaceNote.ini";
