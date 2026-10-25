/// Joins a `/`-separated relative path segment by segment, so the platform
/// never re-parses the string as a path of its own.
pub fn join(base: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let mut path = base.to_path_buf();
    for segment in relative.split('/') {
        path.push(segment);
    }
    path
}
