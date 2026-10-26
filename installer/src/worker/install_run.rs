/// Everything scene 6's run needs, owned, because it crosses a thread. No token
/// and no package list: everything it installs from is in the cache, and the
/// conflict report it may send is unauthenticated.
pub struct InstallJob {
    pub base_url: String,
    pub cache_root: std::path::PathBuf,
    pub game_dir: std::path::PathBuf,
    pub game: pacenotes_installer::gamedir::Game,
    /// What the folder holds afterwards, in install order.
    pub selection: Vec<String>,
    /// Category to picked key, for the configuration step.
    pub picks: std::collections::BTreeMap<String, String>,
    pub choice_categories: Vec<String>,
    /// Measured by the UI thread, the side that may call Win32.
    pub facts: pacenotes_installer::engine::HostFacts,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Scene 6's run: apply the selection to the game folder from the cache, which
/// is what lets this screen work offline.
pub fn run_install(job: InstallJob, reporter: &super::Reporter) -> super::Message {
    let cache = pacenotes_installer::cache::Cache::open(&job.cache_root);
    match super::apply_selection::apply_selection(&job, &cache, reporter) {
        Ok(applied) => super::Message::Installed(super::Outcome::Done { applied }),
        Err(message) => super::Message::Installed(super::Outcome::Failed(message)),
    }
}
