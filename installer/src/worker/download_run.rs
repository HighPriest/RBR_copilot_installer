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
/// game folder, so the cache can be filled now and installed from later. A
/// package that still fails after its retries is recorded and the rest carry
/// on. The worker opens its own handle on the cache; the UI reopens its own
/// afterwards.
pub fn run_download(job: DownloadJob, reporter: &super::Reporter) -> super::Message {
    let api = crate::api::Api::new(&job.base_url, job.token.clone());
    let mut cache = pacenotes_installer::cache::Cache::open(&job.cache_root);
    let missing = &job.downloads;
    let total: u64 = missing.iter().map(|package| package.size).sum();
    let mut done: u64 = 0;
    let mut downloaded = 0;
    let mut bytes: u64 = 0;
    let mut failed = Vec::new();

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
        let mut on_retry = |error: &crate::http::HttpError, delay_ms: u64| {
            let line = format!(
                "retrying {} in {} s: {}",
                package.download_path(),
                delay_ms / 1000,
                error.message
            );
            report(0, Some(line));
        };
        match crate::package_download::download_retrying(
            &api,
            package,
            &mut cache,
            &mut progress,
            &job.cancel,
            &mut on_retry,
        ) {
            Ok(()) => {
                downloaded += 1;
                bytes += package.size;
            }
            Err(e) if e.is_cancelled() => {
                return super::Message::Downloaded(super::Fetched::Cancelled)
            }
            Err(e) => failed.push(super::NotDownloaded {
                key: package.download_path(),
                label: package.label().to_string(),
                reason: e.message,
            }),
        }
        // Counted either way, so the bar reaches its end.
        done += package.size;
    }

    super::Message::Downloaded(super::Fetched::Done {
        downloaded,
        bytes,
        failed,
    })
}
