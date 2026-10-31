/// Which of scene 3's three buttons started the run. It decides only what is
/// skipped: every route ends in the same progress page and summary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    /// Download/Update: scene 4 picks from the catalogue, scene 4b fetches.
    Download,
    /// Configure/Install: straight to the questions, against the cache and the ledger.
    Configure,
    /// Restore/Uninstall: pick a state to go back to, read what that would do, do it.
    Restore,
}

/// What the run accumulates, owned by the sheet and shared by every page.
pub struct WizardState {
    pub language: crate::i18n::Language,
    /// The access key from scene 2; empty is anonymous.
    pub secret: String,
    /// `None` until `GET /health` answers.
    pub online: Option<bool>,
    /// Set once `secret` is accepted, so re-entering scene 2 does not re-check it.
    pub secret_checked: bool,
    pub folder: String,
    /// In `Game::ALL` order.
    pub detected: Vec<pacenotes_installer::gamedir::Game>,
    /// The single detection, or the user's pick when the folder holds several games.
    pub game: Option<pacenotes_installer::gamedir::Game>,
    pub route: Route,
    pub base_url: String,
    /// The game folder's own cache, opened when scene 3 settles the folder.
    pub cache: pacenotes_installer::cache::Cache,
    /// `--cache <dir>`, which wins over the game folder's cache when given.
    pub cache_override: Option<std::path::PathBuf>,

    /// The catalogue behind the tree, whose sizes and digests scene 6 needs.
    pub packages: Vec<pacenotes_shared::PackageInfo>,
    pub tree: pacenotes_installer::tree::Tree,
    /// `HTREEITEM` per tree node, by node index.
    pub tree_items: Vec<isize>,
    /// Set while the model is pushed into the control, whose change notifications would recurse.
    pub tree_syncing: bool,
    pub catalogue_loaded: bool,
    /// The server publishes a catalogue from a schema this build does not know,
    /// which blocks downloading for this game and nothing else.
    pub schema_mismatch: bool,
    /// Set when scene 4b starts its run, since the sheet activates the page again afterwards.
    pub download_started: bool,
    /// What scene 4b fetched, for the summary, since the install outcome comes later.
    pub downloaded: usize,
    pub downloaded_bytes: u64,
    /// The keys committed with Download, in tree order.
    pub chosen: Vec<String>,
    /// Rebuilt from the tree each time scene 5 activates.
    pub config: pacenotes_installer::choices::Configuration,

    /// The points offered, newest first; the list's last row is "remove everything".
    pub points: Vec<pacenotes_installer::restore::Point>,
    /// The chosen point and what it would do, handed to the worker unchanged,
    /// so the preview approved and the work done cannot disagree.
    pub restore_to: Option<pacenotes_installer::restore::Point>,
    pub restore_plan: pacenotes_installer::restore::Plan,

    /// Shown on scenes 6 and 7, and written to disk as it grows.
    pub log: pacenotes_installer::runlog::Log,
    /// Raised by the UI thread, read by the worker between chunks.
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set when scene 6 starts its run, since the sheet activates the page again afterwards.
    pub install_started: bool,
    /// `None` while the run is still going.
    pub outcome: Option<crate::worker::Outcome>,

    request: usize,
}

impl WizardState {
    pub fn new(base_url: String, cache_override: Option<std::path::PathBuf>) -> WizardState {
        let header = crate::run_header::run_header(&base_url);
        WizardState {
            language: crate::i18n::os_language().unwrap_or(crate::i18n::Language::En),
            secret: String::new(),
            online: None,
            secret_checked: false,
            folder: String::new(),
            detected: Vec::new(),
            game: None,
            route: Route::Download,
            base_url,
            // Reads as empty until a route is taken, and nothing downloads before that.
            cache: pacenotes_installer::cache::Cache::open(
                cache_override.clone().unwrap_or_default(),
            ),
            cache_override,
            packages: Vec::new(),
            tree: pacenotes_installer::tree::Tree::default(),
            tree_items: Vec::new(),
            tree_syncing: false,
            catalogue_loaded: false,
            schema_mismatch: false,
            download_started: false,
            downloaded: 0,
            downloaded_bytes: 0,
            chosen: Vec::new(),
            config: pacenotes_installer::choices::Configuration::default(),
            points: Vec::new(),
            restore_to: None,
            restore_plan: pacenotes_installer::restore::Plan::default(),
            // Attached to the game folder once scene 3 settles it.
            log: pacenotes_installer::runlog::Log::create(header, None),
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            install_started: false,
            outcome: None,
            request: 0,
        }
    }

    /// A fresh id for a worker request; answers to older ids are dropped.
    pub fn next_request(&mut self) -> usize {
        self.request += 1;
        self.request
    }

    pub fn is_current_request(&self, request: usize) -> bool {
        request == self.request
    }

    /// The access key for a request, or `None` for anonymous.
    pub fn token(&self) -> Option<String> {
        Some(self.secret.clone()).filter(|secret| !secret.is_empty())
    }

    /// Takes up where an instance restarted as administrator left off: its
    /// language, folder and key, the key already past scene 2.
    pub fn resume(&mut self, restart: crate::elevation::Restart) {
        self.language = restart.language;
        self.folder = restart.folder;
        self.secret = restart.secret;
        self.secret_checked = true;
    }

    /// Scene 3 settles the game before anything needs it; RBR only spares the lookups a `None`.
    pub fn game_id(&self) -> &'static str {
        self.game
            .map(|game| game.id())
            .unwrap_or(pacenotes_shared::GAME_RBR)
    }
}
