/// What was at a path before the installer first wrote it, which decides what uninstall does.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Prior {
    /// Nothing: uninstall deletes the file.
    Absent,
    /// Another file, copied into this restore point, named per file because a
    /// path replaced in several runs has a copy in each.
    Backup { point: String },
    /// Exactly these bytes, so nothing was copied: uninstall puts the same
    /// bytes back from the package's payload, or leaves the file when that is gone.
    Identical,
}

/// One file an installed package owns.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LedgerFile {
    /// As written, `/`-separated in its original case; the map key is this folded.
    pub path: String,
    /// What the installer wrote, compared before uninstall deletes anything.
    pub sha256: String,
    pub prior: Prior,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LedgerEntry {
    /// The package zip's, which is also its cache payload's name.
    pub sha256: String,
    pub version: String,
    /// As installed, so uninstall decisions work offline without a manifest.
    #[serde(default)]
    pub exclusive_group: Option<String>,
    #[serde(default)]
    pub state: EntryState,
    /// By folded path.
    pub files: std::collections::BTreeMap<String, LedgerFile>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryState {
    #[default]
    Installed,
    /// Recorded before extraction began, so the next run knows a half-written
    /// file is ours and never backs it up as the user's original.
    Installing,
}
