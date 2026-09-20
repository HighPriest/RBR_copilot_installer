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
pub const IDS_BADKEY: u16 = 7;
pub const IDS_NOPLUGIN: u16 = 8;
pub const IDS_READY: u16 = 9;
pub const IDS_NOT_IMPL: u16 = 10;
pub const IDS_APP_TITLE: u16 = 20;
pub const IDS_TAB_LIBRARY: u16 = 21;
pub const IDS_TAB_INSTALL: u16 = 22;
pub const IDS_COL_NAME: u16 = 23;
pub const IDS_COL_CATEGORY: u16 = 24;
pub const IDS_COL_VERSION: u16 = 25;
pub const IDS_COL_SIZE: u16 = 26;
pub const IDS_COL_STATE: u16 = 27;
pub const IDS_BTN_DOWNLOAD: u16 = 28;
pub const IDS_BTN_UNLOCK: u16 = 29;
pub const IDS_BTN_APPLY: u16 = 30;
pub const IDS_BTN_RESTORE: u16 = 31;
pub const IDS_BTN_CLOSE: u16 = 32;
pub const IDS_BTN_BROWSE: u16 = 33;
pub const IDS_LBL_ACCESSKEY: u16 = 34;
pub const IDS_LBL_GAMEDIR: u16 = 35;
pub const IDS_LANG_TITLE: u16 = 36;
pub const IDS_LANG_PROMPT: u16 = 37;
pub const IDS_STATE_NOTCACHED: u16 = 38;
pub const IDS_STATE_CACHED: u16 = 39;
pub const IDS_STATE_UPDATE: u16 = 40;
pub const IDS_BROWSE_PROMPT: u16 = 41;
