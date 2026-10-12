//! Localization. Strings live in per-language STRINGTABLEs in res/installer.rc;
//! selecting a language is just `SetThreadUILanguage`, after which `LoadStringW`
//! resolves against the thread UI language on its own. That means zero lookup
//! code and zero per-language tables of our own — the smallest thing that works.
//! Mirrors the C# LocalizationChangeForm + culture setup in Program.cs. Task I2.

#![allow(dead_code)]

/// MAKELANGID(p, s) = (s << 10) | p
const fn make_langid(primary: u16, sublang: u16) -> u16 {
    (sublang << 10) | primary
}

const LANG_ENGLISH: u16 = 0x09;
const SUBLANG_ENGLISH_US: u16 = 0x01;
const LANG_POLISH: u16 = 0x15;
const SUBLANG_DEFAULT: u16 = 0x01;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    En,
    Pl,
}

impl Language {
    /// Order shown in the picker combo; index doubles as the combo item index.
    pub const ALL: [Language; 2] = [Language::En, Language::Pl];

    /// Endonym — deliberately NOT localized, so a user who booted into the
    /// wrong language can still recognize their own.
    pub fn label(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Pl => "Polski",
        }
    }

    fn langid(self) -> u16 {
        match self {
            Language::En => make_langid(LANG_ENGLISH, SUBLANG_ENGLISH_US),
            Language::Pl => make_langid(LANG_POLISH, SUBLANG_DEFAULT),
        }
    }

    /// Index in `ALL` — also the combo item index on scene 1.
    pub fn index(self) -> usize {
        Language::ALL.iter().position(|&l| l == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Language {
        Language::ALL.get(index).copied().unwrap_or(Language::En)
    }

    /// Short tag for `GET /motd?lang=`. Matches the `<lang>.txt` filenames the
    /// server serves MOTD text from.
    pub fn tag(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Pl => "pl",
        }
    }

    /// Primary language id of this langid, i.e. ignoring the sublanguage, so
    /// any Polish locale matches our Polish and any English matches English.
    fn primary(self) -> u16 {
        self.langid() & 0x3FF
    }
}

/// The language the OS UI is in, if the installer ships it.
///
/// DESIGN.md scene 1: this is the combo's default. Compared on the PRIMARY
/// language only — someone running pl-PL and someone running a Polish build
/// with a different sublanguage should both land on Polish.
pub fn os_language() -> Option<Language> {
    let ui = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    let primary = ui & 0x3FF;
    Language::ALL
        .iter()
        .copied()
        .find(|l| l.primary() == primary)
}

/// Set the UI language for the calling thread. Resource lookups made by
/// `load_string` afterwards resolve against it.
///
/// NOTE: this is per-thread. Worker threads (task I4) that need localized
/// status text must either call this too or receive already-loaded strings
/// from the UI thread. We do the latter.
pub fn set_language(lang: Language) {
    unsafe { windows_sys::Win32::Globalization::SetThreadUILanguage(lang.langid()) };
}

/// Load string `id` from the STRINGTABLE for the current thread UI language.
///
/// Uses the zero-buffer form of LoadStringW: passing cchBufferMax = 0 makes it
/// write a pointer to the read-only resource data into the buffer argument and
/// return its length, so there is no copy into a stack buffer and no guess at
/// a maximum length.
pub fn load_string(id: u16) -> String {
    unsafe {
        let mut ptr: *mut u16 = core::ptr::null_mut();
        let len = windows_sys::Win32::UI::WindowsAndMessaging::LoadStringW(
            instance(),
            id as u32,
            // The "buffer" is where LoadStringW stores the resource pointer.
            &mut ptr as *mut *mut u16 as windows_sys::core::PWSTR,
            0,
        );
        if len <= 0 || ptr.is_null() {
            // Missing id: show the id rather than an empty control, so a
            // forgotten translation is visible instead of silently blank.
            return format!("<{id}>");
        }
        String::from_utf16_lossy(core::slice::from_raw_parts(ptr, len as usize))
    }
}

/// Load string `id` and substitute the FIRST `%s` with `arg`.
///
/// Deliberately not a printf: the only placeholder any of our strings use is a
/// single `%s`, and hand-rolling this keeps a formatting crate out of the size
/// budget. A string with no `%s` comes back unchanged rather than silently
/// dropping `arg`, so a translation that forgets the placeholder degrades to
/// a still-readable message.
pub fn load_string_arg(id: u16, arg: &str) -> String {
    let template = load_string(id);
    match template.find("%s") {
        Some(at) => format!("{}{arg}{}", &template[..at], &template[at + 2..]),
        None => template,
    }
}

/// HINSTANCE of this executable (where the resources live).
pub fn instance() -> windows_sys::Win32::Foundation::HMODULE {
    unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(core::ptr::null()) }
}

/// UTF-16, NUL-terminated — what every -W API wants.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

// ---- String table ids (must match the #defines in res/installer.rc) ----
pub const IDS_CONNECTING: u16 = 1;
pub const IDS_DOWNLOADING: u16 = 2;
pub const IDS_VERIFYING: u16 = 3;
pub const IDS_BACKUP: u16 = 4;
pub const IDS_INSTALLING: u16 = 5;
pub const IDS_DONE: u16 = 6;
pub const IDS_READY: u16 = 9;
pub const IDS_NOT_IMPL: u16 = 10;
pub const IDS_APP_TITLE: u16 = 20;

// Per-scene primary button labels + shared chrome
pub const IDS_BTN_CONTINUE: u16 = 30;
pub const IDS_BTN_ACCEPT: u16 = 31;
pub const IDS_BTN_NEXT: u16 = 32;
pub const IDS_BTN_DOWNLOAD: u16 = 33;
pub const IDS_BTN_INSTALL: u16 = 34;
pub const IDS_BTN_FINISH: u16 = 35;
pub const IDS_BTN_BROWSE: u16 = 36;
pub const IDS_CANCEL_CONFIRM: u16 = 37;
pub const IDS_BTN_BACK: u16 = 38;
pub const IDS_BTN_CANCEL: u16 = 39;

// Scene 1 — Language
pub const IDS_S1_TITLE: u16 = 50;
pub const IDS_S1_PROMPT: u16 = 51;
pub const IDS_S1_RESTART: u16 = 52;

// Scene 2 — Message of the day
pub const IDS_S2_TITLE: u16 = 60;
pub const IDS_S2_SECRET: u16 = 61;
pub const IDS_S2_CHECKING: u16 = 62;
pub const IDS_S2_ONLINE: u16 = 63;
pub const IDS_S2_OFFLINE: u16 = 64;
pub const IDS_S2_OFFLINE_MOTD: u16 = 65;
pub const IDS_S2_BADSECRET: u16 = 66;

// Scene 3 — Game folder
pub const IDS_S3_TITLE: u16 = 70;
pub const IDS_S3_PATH: u16 = 71;
pub const IDS_S3_GAME: u16 = 72;
pub const IDS_S3_DETECTING: u16 = 73;
pub const IDS_S3_NOGAME: u16 = 74;
