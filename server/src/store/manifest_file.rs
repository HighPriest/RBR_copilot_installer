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
