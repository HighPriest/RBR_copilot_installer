//! Localization: strings live in the .rc STRINGTABLE; this module selects the
//! active language and loads strings via LoadStringW. Mirrors the C#
//! LocalizationChangeForm + culture setup in Program.cs. Task I2.

#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    // TODO(I2): add languages present in the original resx resources.
}

static mut CURRENT: Language = Language::En;

pub fn set_language(lang: Language) {
    // Single-threaded UI init; worker threads only read via load_string.
    unsafe { CURRENT = lang }
}

/// Load string `id` from the STRINGTABLE for the current language.
/// (Per-language tables use id offsets or SetThreadUILanguage — decide in I2.)
pub fn load_string(_id: u16) -> String {
    // TODO(I2): LoadStringW
    String::new()
}
