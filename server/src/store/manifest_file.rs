/// Reads and validates `manifest.json`; a missing file is an empty manifest.
///
/// Rejects the whole document when any game, directory or name is not a valid
/// lookup key, so a hand-edited manifest can never yield a path outside the data root.
pub fn read_manifest(path: &std::path::Path) -> anyhow::Result<pacenotes_shared::Manifest> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(pacenotes_shared::Manifest::default())
        }
        Err(e) => anyhow::bail!("reading {}: {e}", path.display()),
    };
    let manifest: pacenotes_shared::Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;

    for package in &manifest.packages {
        if !pacenotes_shared::valid_id(&package.game) {
            anyhow::bail!("invalid game id {:?} in manifest", package.game);
        }
        if !pacenotes_shared::valid_id(&package.directory) {
            anyhow::bail!("invalid directory name {:?} in manifest", package.directory);
        }
        if !pacenotes_shared::valid_name(&package.name) {
            anyhow::bail!("invalid package name {:?} in manifest", package.name);
        }
    }
    Ok(manifest)
}

/// Writes `<data>/manifest.json` through a temp file renamed over it in the same
/// directory, so a crash or a full disk leaves the previous manifest whole and
/// a reloading server never reads half a file.
pub fn write_manifest(
    data_dir: &std::path::Path,
    manifest: &pacenotes_shared::Manifest,
) -> anyhow::Result<std::path::PathBuf> {
    let path = data_dir.join(crate::store::MANIFEST_FILE);
    let temp = data_dir.join(format!(".{}.tmp", crate::store::MANIFEST_FILE));
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(&temp, format!("{json}\n"))
        .map_err(|e| anyhow::anyhow!("writing {}: {e}", temp.display()))?;
    std::fs::rename(&temp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        anyhow::anyhow!("replacing {}: {e}", path.display())
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_manifest_is_replaced_without_leaving_a_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join(crate::store::MANIFEST_FILE), b"old").unwrap();

        let path = super::write_manifest(dir, &pacenotes_shared::Manifest::default()).unwrap();
        assert_eq!(path, dir.join(crate::store::MANIFEST_FILE));
        assert!(super::read_manifest(&path).unwrap().packages.is_empty());
        let leftovers: Vec<_> = std::fs::read_dir(dir).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    }
}
