const LANG_ENGLISH: u16 = 0x09;
const SUBLANG_ENGLISH_US: u16 = 0x01;
const LANG_POLISH: u16 = 0x15;
const SUBLANG_DEFAULT: u16 = 0x01;
const PRIMARY_LANGUAGE_MASK: u16 = 0x3FF;

/// `MAKELANGID`.
const fn make_langid(primary: u16, sublanguage: u16) -> u16 {
    (sublanguage << 10) | primary
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    En,
    Pl,
}

impl Language {
    /// In picker order, so an index is also the combo item index.
    pub const ALL: [Language; 2] = [Language::En, Language::Pl];

    /// The endonym, never translated, so a user in the wrong language still recognises theirs.
    pub fn label(self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Pl => "Polski",
        }
    }

    pub fn index(self) -> usize {
        Language::ALL
            .iter()
            .position(|&language| language == self)
            .unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Language {
        Language::ALL.get(index).copied().unwrap_or(Language::En)
    }

    /// The `lang` of `GET /motd`, which names the server's `<lang>.txt`.
    pub fn tag(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Pl => "pl",
        }
    }

    fn langid(self) -> u16 {
        match self {
            Language::En => make_langid(LANG_ENGLISH, SUBLANG_ENGLISH_US),
            Language::Pl => make_langid(LANG_POLISH, SUBLANG_DEFAULT),
        }
    }

    fn primary(self) -> u16 {
        self.langid() & PRIMARY_LANGUAGE_MASK
    }
}

/// The OS UI language when the installer ships it, matched on the primary
/// language so every Polish locale lands on Polish.
pub fn os_language() -> Option<Language> {
    let ui_language = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    let primary = ui_language & PRIMARY_LANGUAGE_MASK;
    Language::ALL
        .iter()
        .copied()
        .find(|language| language.primary() == primary)
}

/// The language scene 1 chose, or 0 before it has.
static CHOSEN: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Per thread, so workers get already-loaded strings from the UI thread; it is
/// also remembered for the panic hook, which can run on any thread.
pub fn set_language(language: Language) {
    CHOSEN.store(language.langid(), std::sync::atomic::Ordering::Relaxed);
    unsafe { windows_sys::Win32::Globalization::SetThreadUILanguage(language.langid()) };
}

/// Applies the chosen language to this thread; before scene 1 answers the OS default stands.
pub fn use_chosen_language() {
    let langid = CHOSEN.load(std::sync::atomic::Ordering::Relaxed);
    if langid != 0 {
        unsafe { windows_sys::Win32::Globalization::SetThreadUILanguage(langid) };
    }
}
