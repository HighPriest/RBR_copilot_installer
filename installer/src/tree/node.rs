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
}

impl LeafState {
    pub fn is_present(self) -> bool {
        matches!(
            self,
            LeafState::Downloaded | LeafState::UpdateAvailable | LeafState::LocalOnly
        )
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
    /// Leaves only.
    pub state: Option<LeafState>,
    /// Leaves only; see `Tree::check_of` for grouping rows.
    pub(super) checked: bool,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.key.is_some()
    }
}
