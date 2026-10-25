/// One row of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    /// `game/directory/name`, the key the cache and the ledger use.
    pub key: String,
    /// The path below the category, e.g. `polish / male / Kamil`.
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceList {
    /// The schema's category id, e.g. `pacenotes_shared::CATEGORY_CODRIVER`.
    pub category: &'static str,
    pub options: Vec<ChoiceOption>,
    pub selected: Option<usize>,
}

impl ChoiceList {
    pub fn selected_key(&self) -> Option<&str> {
        self.selected.map(|index| self.options[index].key.as_str())
    }

    /// An empty list asks nothing, as a category the server does not offer must not block Install.
    pub fn is_satisfied(&self) -> bool {
        self.options.is_empty() || self.selected.is_some()
    }
}
