//! Fetching a package into the cache. A payload enters the cache only after its
//! sha256 matched the manifest, so nothing downstream verifies it again.

/// The waits before the second and third attempts at one package.
const RETRY_DELAYS_MS: [u64; 2] = [1_000, 3_000];
/// How often a wait looks at Cancel.
const CANCEL_POLL_MS: u64 = 100;

/// One attempt's failure, and whether another attempt could succeed.
struct Failure {
    error: crate::http::HttpError,
    retry: bool,
}

impl Failure {
    fn transfer(error: crate::http::HttpError) -> Failure {
        let retry = error.is_transient();
        Failure { error, retry }
    }

    /// A full or locked disk is not the network's fault, and waiting will not fix it.
    fn local(context: &str, error: std::io::Error) -> Failure {
        Failure {
            error: crate::http::HttpError::parse(format!("{context}: {error}")),
            retry: false,
        }
    }
}

/// `download_to_cache`, tried again after a failure a retry could fix (see
/// `HttpError::is_transient`, plus a payload that arrived corrupted), waiting
/// between attempts; `on_retry` hears why, for the run log.
pub fn download_retrying(
    api: &crate::api::Api,
    package: &pacenotes_shared::PackageInfo,
    cache: &mut pacenotes_installer::cache::Cache,
    progress: crate::http::ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
    on_retry: &mut dyn FnMut(&crate::http::HttpError, u64),
) -> Result<(), crate::http::HttpError> {
    let mut delays = RETRY_DELAYS_MS.iter();
    loop {
        let failure = match download_to_cache(api, package, cache, &mut *progress, cancel) {
            Ok(()) => return Ok(()),
            Err(failure) => failure,
        };
        let delay = match delays.next() {
            Some(&delay) if failure.retry && !failure.error.is_cancelled() => delay,
            _ => return Err(failure.error),
        };
        on_retry(&failure.error, delay);
        wait(delay, cancel)?;
    }
}

/// Sleeps `milliseconds`, giving up as soon as Cancel is pressed.
fn wait(
    milliseconds: u64,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), crate::http::HttpError> {
    let mut waited = 0;
    while waited < milliseconds {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::http::HttpError::cancelled());
        }
        std::thread::sleep(std::time::Duration::from_millis(CANCEL_POLL_MS));
        waited += CANCEL_POLL_MS;
    }
    Ok(())
}

/// Streams `package` to a temp file while hashing it, then promotes the file
/// to `<sha>.zip` and records the entry; any failure deletes the temp file.
fn download_to_cache(
    api: &crate::api::Api,
    package: &pacenotes_shared::PackageInfo,
    cache: &mut pacenotes_installer::cache::Cache,
    progress: crate::http::ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), Failure> {
    std::fs::create_dir_all(cache.root())
        .map_err(|e| Failure::local("cannot create the download cache", e))?;
    // Named per package; two installer instances are kept apart by the single-instance check.
    let temp = cache.root().join(format!(
        "{}.part",
        pacenotes_installer::sha256::hex_digest(package.download_path().as_bytes())
    ));

    let outcome = fetch_verified(api, package, &temp, progress, cancel)
        .and_then(|()| promote(cache, package, &temp));
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    outcome
}

fn fetch_verified(
    api: &crate::api::Api,
    package: &pacenotes_shared::PackageInfo,
    temp: &std::path::Path,
    progress: crate::http::ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), Failure> {
    let mut file =
        std::fs::File::create(temp).map_err(|e| Failure::local("cannot write to the cache", e))?;
    let mut hasher = pacenotes_installer::sha256::Sha256::new();
    let mut write_error: Option<std::io::Error> = None;

    let transfer = crate::http::get_streaming(
        &api.download_url(package),
        api.token.as_deref(),
        &mut |chunk| {
            hasher.update(chunk);
            std::io::Write::write_all(&mut file, chunk)
                .map_err(|e| write_error = Some(e))
                .is_ok()
        },
        progress,
        cancel,
    );
    // A full disk must not be reported as a network failure.
    if let Some(e) = write_error {
        return Err(Failure::local("cannot write to the cache", e));
    }
    transfer.map_err(Failure::transfer)?;

    let received = pacenotes_installer::sha256::hex(&hasher.finalize());
    if received != package.sha256 {
        // Damaged on the way, which the next attempt may well not be.
        return Err(Failure {
            error: crate::http::HttpError::parse(format!(
                "{} arrived corrupted: expected sha256 {}, got {received}",
                package.label(),
                package.sha256
            )),
            retry: true,
        });
    }
    Ok(())
}

/// Identical bytes another package already stored are kept and the temp file dropped.
fn promote(
    cache: &mut pacenotes_installer::cache::Cache,
    package: &pacenotes_shared::PackageInfo,
    temp: &std::path::Path,
) -> Result<(), Failure> {
    let payload = cache.payload_path(&package.sha256);
    if payload.exists() {
        let _ = std::fs::remove_file(temp);
    } else {
        std::fs::rename(temp, &payload)
            .map_err(|e| Failure::local("cannot move the download into the cache", e))?;
    }
    cache
        .insert(pacenotes_installer::tree::CachedPackage::of(package))
        .map_err(|e| Failure::local("cannot update the cache index", e))
}
