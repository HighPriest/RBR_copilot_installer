//! Fetching a package into the cache. A payload enters the cache only after its
//! sha256 matched the manifest, so nothing downstream verifies it again.

/// Streams `package` to a temp file while hashing it, then promotes the file
/// to `<sha>.zip` and records the entry; any failure deletes the temp file.
pub fn download_to_cache(
    api: &crate::api::Api,
    package: &pacenotes_shared::PackageInfo,
    cache: &mut pacenotes_installer::cache::Cache,
    progress: crate::http::ProgressFn,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(), crate::http::HttpError> {
    std::fs::create_dir_all(cache.root())
        .map_err(|e| local_failure("cannot create the download cache", e))?;
    // Named per package, so two concurrent runs never share a temp file.
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
) -> Result<(), crate::http::HttpError> {
    let mut file =
        std::fs::File::create(temp).map_err(|e| local_failure("cannot write to the cache", e))?;
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
        return Err(local_failure("cannot write to the cache", e));
    }
    transfer?;

    let received = pacenotes_installer::sha256::hex(&hasher.finalize());
    if received != package.sha256 {
        return Err(crate::http::HttpError::parse(format!(
            "{} arrived corrupted: expected sha256 {}, got {received}",
            package.label(),
            package.sha256
        )));
    }
    Ok(())
}

/// Identical bytes another package already stored are kept and the temp file dropped.
fn promote(
    cache: &mut pacenotes_installer::cache::Cache,
    package: &pacenotes_shared::PackageInfo,
    temp: &std::path::Path,
) -> Result<(), crate::http::HttpError> {
    let payload = cache.payload_path(&package.sha256);
    if payload.exists() {
        let _ = std::fs::remove_file(temp);
    } else {
        std::fs::rename(temp, &payload)
            .map_err(|e| local_failure("cannot move the download into the cache", e))?;
    }
    cache
        .insert(pacenotes_installer::tree::CachedPackage {
            game: package.game.clone(),
            directory: package.directory.clone(),
            name: package.name.clone(),
            sha256: package.sha256.clone(),
            version: package.version.clone(),
            display_name: package.display_name.clone(),
        })
        .map_err(|e| local_failure("cannot update the cache index", e))
}

fn local_failure(context: &str, error: std::io::Error) -> crate::http::HttpError {
    crate::http::HttpError::parse(format!("{context}: {error}"))
}
