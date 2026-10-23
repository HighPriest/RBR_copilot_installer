/// `%LOCALAPPDATA%\PacenotesInstaller\cache`, or `None` when the variable is unset.
pub fn default_root() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        std::path::PathBuf::from(base)
            .join("PacenotesInstaller")
            .join("cache")
    })
}
