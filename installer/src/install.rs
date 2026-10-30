//! RBR-specific install knowledge: `write_rbr_config`, the one built-in step,
//! ported from the legacy `CreateRBRConfiguration`. No Win32.

/// `(percent, status)`.
pub type Report<'a> = &'a mut dyn FnMut(u8, &str);

/// Windows separators are intentional: RBR expects them.
pub const PACENOTE_CONFIG_DIR: &str = "Plugins\\Pacenote\\config";

/// Writes `PaceNote.ini` and the `Rbr*.ini` files byte-for-byte as the C# did,
/// and returns the summary shown in the UI; `Err` carries the localized
/// "install the Pacenote plugin first" text when `PACENOTE_CONFIG_DIR` is missing.
///
/// TODO(I7): implement.
pub fn write_rbr_config(
    _game_dir: &std::path::Path,
    _copilot: &str,
    _style: &str,
    _language: &str,
) -> Result<String, String> {
    Err("not implemented".into())
}
