/// Where a leaf stands between the server and this computer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeafState {
    NotCached,
    /// Cached at exactly the sha the server lists.
    Downloaded,
    /// Cached, but the server lists a different sha.
    UpdateAvailable,
    /// Cached and no longer listed: withdrawn, or no longer granted. Still installable.
    LocalOnly,
    /// Installed, and in neither the catalogue nor the cache, so nothing can
    /// reinstall it; shown so the user can keep it, since Apply removes what is unselected.
    Unavailable,
}

impl LeafState {
    /// Whether some payload is on this computer, an outdated one included, so the leaf installs offline.
    pub fn has_payload(self) -> bool {
        matches!(
            self,
            LeafState::Downloaded | LeafState::UpdateAvailable | LeafState::LocalOnly
        )
    }

    /// Not the negation of `has_payload`: an update has old bytes yet still needs the new ones.
    pub fn needs_download(self) -> bool {
        matches!(self, LeafState::NotCached | LeafState::UpdateAvailable)
    }
}

/// A row's checkbox; a grouping row's is derived from the leaves under it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    Unchecked,
    Checked,
    Partial,
}

/// One row. Grouping rows carry no package.
#[derive(Clone, Debug)]
pub struct Node {
    /// Without the state suffix, which the UI adds localized.
    pub label: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Leaves only: the package's `game/directory/name`.
    pub key: Option<String>,
    /// Leaves only: where the package stands against the cache.
    pub state: Option<LeafState>,
    /// In this game folder's ledger, which is independent of the per-user cache.
    pub installed: bool,
    /// Leaves only; see `Tree::check_of` for grouping rows.
    pub(super) checked: bool,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.key.is_some()
    }
}
