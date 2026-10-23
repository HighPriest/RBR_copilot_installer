/// A 64 KiB chunk arrives hundreds of times a second, and a post per chunk
/// would keep the UI thread redrawing instead of responding.
const REPORT_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Scene 6's run: download every package in `missing` (what scene 4 ticked
/// and the cache lacks), then stop and say so, since backup and install wait
/// for tasks I6 and I7.
///
/// The worker opens its own handle on the cache; the UI reopens its own afterwards.
pub fn run_install(
    base_url: String,
    token: Option<String>,
    cache_root: std::path::PathBuf,
    missing: Vec<pacenotes_shared::PackageInfo>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    reporter: &super::Reporter,
) -> super::Message {
    let api = crate::api::Api::new(&base_url, token);
    let mut cache = pacenotes_installer::cache::Cache::open(&cache_root);
    let total: u64 = missing.iter().map(|package| package.size).sum();
    let mut done: u64 = 0;

    for package in &missing {
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
            &cancel,
        ) {
            Ok(()) => done += package.size,
            Err(e) if e.is_cancelled() => {
                return super::Message::Installed(super::Outcome::Cancelled)
            }
            Err(e) => return super::Message::Installed(super::Outcome::Failed(e.message)),
        }
    }

    reporter.send(super::Progress {
        phase: super::Phase::Install,
        item: String::new(),
        item_done: 0,
        item_size: 0,
        done: total,
        total,
        log: Some(
            "backup and install are not implemented yet (tasks I6 and I7); \
             nothing in the game folder was touched"
                .to_string(),
        ),
    });
    super::Message::Installed(super::Outcome::Done {
        downloaded: missing.len(),
        bytes: total,
    })
}
