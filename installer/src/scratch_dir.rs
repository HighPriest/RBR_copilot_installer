/// A temporary directory that deletes itself, kept in-crate to spare a `tempfile` dependency.
pub struct ScratchDir(pub std::path::PathBuf);

impl ScratchDir {
    pub fn new(label: &str) -> ScratchDir {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("pacenotes-{label}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create scratch dir");
        ScratchDir(path)
    }

    pub fn path(&self) -> &str {
        self.0.to_str().expect("scratch path is valid utf-8")
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
