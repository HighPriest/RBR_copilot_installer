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
    pub base_url: String,
    pub cache: pacenotes_installer::cache::Cache,

    pub tree: pacenotes_installer::tree::Tree,
    /// `HTREEITEM` per tree node, by node index.
    pub tree_items: Vec<isize>,
    /// Set while the model is pushed into the control, whose change notifications would recurse.
    pub tree_syncing: bool,
    pub catalogue_loaded: bool,
    /// The keys committed with Download, in tree order.
    pub chosen: Vec<String>,
    /// Rebuilt from the tree each time scene 5 activates.
    pub config: pacenotes_installer::choices::Configuration,

    request: usize,
}

impl WizardState {
    pub fn new(base_url: String) -> WizardState {
        WizardState {
            language: crate::i18n::Language::En,
            secret: String::new(),
            online: None,
            secret_checked: false,
            folder: String::new(),
            detected: Vec::new(),
            game: None,
            base_url,
            cache: open_cache(),
            tree: pacenotes_installer::tree::Tree::default(),
            tree_items: Vec::new(),
            tree_syncing: false,
            catalogue_loaded: false,
            chosen: Vec::new(),
            config: pacenotes_installer::choices::Configuration::default(),
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
}

/// A cache that cannot be located behaves as an empty one in the temp folder.
fn open_cache() -> pacenotes_installer::cache::Cache {
    let root = pacenotes_installer::cache::default_root().unwrap_or_else(|| {
        std::env::temp_dir()
            .join("PacenotesInstaller")
            .join("cache")
    });
    pacenotes_installer::cache::Cache::open(root)
}
