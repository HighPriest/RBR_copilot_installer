/// A 64 KiB chunk arrives hundreds of times a second, and a post per chunk
/// would keep the UI thread redrawing instead of responding.
const REPORT_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Everything scene 4b's run needs, owned, because it crosses a thread. It
/// knows no game folder: the download only fills the cache.
pub struct DownloadJob {
    pub base_url: String,
    pub token: Option<String>,
    pub cache_root: std::path::PathBuf,
    /// Checked in scene 4 and not on this computer yet, which is the scene 4
    /// selection rather than the scene 5 picks.
    pub downloads: Vec<pacenotes_shared::PackageInfo>,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Scene 4b's run: fetch what is missing and verify it, writing nothing to the
/// game folder, so the cache can be filled now and installed from later. The
/// worker opens its own handle on the cache; the UI reopens its own afterwards.
pub fn run_download(job: DownloadJob, reporter: &super::Reporter) -> super::Message {
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
                return super::Message::Downloaded(super::Fetched::Cancelled)
            }
            Err(e) => return super::Message::Downloaded(super::Fetched::Failed(e.message)),
        }
    }

    super::Message::Downloaded(super::Fetched::Done {
        downloaded: missing.len(),
        bytes: total,
    })
}
