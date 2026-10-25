/// Lowercase hex sha256 of a file's contents, streamed rather than read whole.
pub fn sha256_file(path: &std::path::Path) -> anyhow::Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| anyhow::anyhow!("hash {}: {e}", path.display()))?;
    Ok(hex(&sha2::Digest::finalize(hasher)))
}

/// Lowercase hex sha256 of bytes already in memory.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex(&<sha2::Sha256 as sha2::Digest>::digest(bytes))
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
