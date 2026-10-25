/// A 64 KiB chunk arrives hundreds of times a second, and a post per chunk
/// would keep the UI thread redrawing instead of responding.
const REPORT_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Everything scene 6's run needs, owned, because it crosses a thread.
pub struct InstallJob {
    pub base_url: String,
    pub token: Option<String>,
    pub cache_root: std::path::PathBuf,
    pub game_dir: std::path::PathBuf,
    pub game: pacenotes_installer::gamedir::Game,
    /// Checked in scene 4 and not on this computer yet.
    pub downloads: Vec<pacenotes_shared::PackageInfo>,
    /// What the folder holds afterwards, in install order.
    pub selection: Vec<String>,
    /// Category to picked key, for the configuration step.
    pub picks: std::collections::BTreeMap<String, String>,
    pub choice_categories: Vec<String>,
    /// Measured by the UI thread, the side that may call Win32.
    pub facts: pacenotes_installer::engine::HostFacts,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Scene 6's run: download what is missing, then apply the selection to the
/// game folder. The worker opens its own handle on the cache; the UI reopens
/// its own afterwards.
pub fn run_install(job: InstallJob, reporter: &super::Reporter) -> super::Message {
    let api = crate::api::Api::new(&job.base_url, job.token.clone());
    let mut cache = pacenotes_installer::cache::Cache::open(&job.cache_root);
    let missing = &job.downloads;
    let total: u64 = missing.iter().map(|package| package.size).sum();
    let mut done: u64 = 0;

    for package in missing {
        let report = |item_done: u64, log: Option<String>| {
            reporter.send(super::Progress {
                phase: super::Phase::Download,
                item: package.label().to_string(),
                item_done,
                item_size: package.size,
                done: done + item_done,
                total,
                log,
            });
        };
        report(
            0,
            Some(format!(
                "downloading {} ({})",
                package.download_path(),
                pacenotes_installer::size::human(package.size)
            )),
        );

        let mut last_post = std::time::Instant::now();
        let mut progress = |read: u64, _content_length: Option<u64>| {
            if last_post.elapsed() >= REPORT_EVERY {
                last_post = std::time::Instant::now();
                report(read, None);
            }
        };
        match crate::package_download::download_to_cache(
            &api,
            package,
            &mut cache,
            &mut progress,
            &job.cancel,
        ) {
            Ok(()) => done += package.size,
            Err(e) if e.is_cancelled() => {
                return super::Message::Installed(super::Outcome::Cancelled)
            }
            Err(e) => return super::Message::Installed(super::Outcome::Failed(e.message)),
        }
    }

    match super::apply_selection::apply_selection(&job, &cache, reporter) {
        Ok(applied) => super::Message::Installed(super::Outcome::Done {
            downloaded: missing.len(),
            bytes: total,
            applied,
        }),
        Err(message) => super::Message::Installed(super::Outcome::Failed(message)),
    }
}
