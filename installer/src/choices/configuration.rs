#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Configuration {
    /// In `CHOICE_CATEGORIES` order, which is the page's order.
    pub lists: Vec<super::ChoiceList>,
    /// Checked packages that take no choice: `pacenotes` and any other category.
    pub extras: Vec<String>,
}

impl Configuration {
    /// The lists for the checked leaves of `tree`. A key in `keep` that is
    /// still offered stays picked across a trip back to scene 4, and a list
    /// with a single option comes preselected.
    pub fn from_tree(tree: &crate::tree::Tree, keep: &[String]) -> Configuration {
        let checked = tree.checked_leaves();
        let lists = pacenotes_shared::CHOICE_CATEGORIES
            .iter()
            .map(|&category| {
                let options: Vec<super::ChoiceOption> = checked
                    .iter()
                    .copied()
                    .filter(|&leaf| tree.category_of(leaf) == category)
                    .filter_map(|leaf| {
                        let key = tree.node(leaf).key.clone()?;
                        Some(super::ChoiceOption {
                            key,
                            label: option_label(&tree.path_of(leaf)),
                        })
                    })
                    .collect();
                let selected = keep
                    .iter()
                    .find_map(|kept| options.iter().position(|option| &option.key == kept))
                    .or(if options.len() == 1 { Some(0) } else { None });
                super::ChoiceList {
                    category,
                    options,
                    selected,
                }
            })
            .collect();

        let extras = checked
            .iter()
            .copied()
            .filter(|&leaf| !pacenotes_shared::CHOICE_CATEGORIES.contains(&tree.category_of(leaf)))
            .filter_map(|leaf| tree.node(leaf).key.clone())
            .collect();

        Configuration { lists, extras }
    }

    pub fn is_complete(&self) -> bool {
        self.lists.iter().all(super::ChoiceList::is_satisfied)
    }

    /// The categories still waiting for a pick, as the warning names them.
    pub fn missing(&self) -> Vec<&'static str> {
        self.lists
            .iter()
            .filter(|list| !list.is_satisfied())
            .map(|list| list.category)
            .collect()
    }

    pub fn select(&mut self, list: usize, option: Option<usize>) {
        if let Some(list) = self.lists.get_mut(list) {
            list.selected = option.filter(|&index| index < list.options.len());
        }
    }

    /// The pick for one `CATEGORY_*`, by name, so a reordered `CHOICE_CATEGORIES` cannot misdirect it.
    pub fn selected_key(&self, category: &str) -> Option<&str> {
        self.lists
            .iter()
            .find(|list| list.category == category)
            .and_then(super::ChoiceList::selected_key)
    }

    /// In list order, skipping empty lists.
    pub fn selected_keys(&self) -> Vec<String> {
        self.lists
            .iter()
            .filter_map(|list| list.selected_key().map(str::to_string))
            .collect()
    }

    /// Category to picked key, for the game's configuration step to take what it understands.
    pub fn picks_by_category(&self) -> std::collections::BTreeMap<String, String> {
        self.lists
            .iter()
            .filter_map(|list| {
                let key = list.selected_key()?;
                Some((list.category.to_string(), key.to_string()))
            })
            .collect()
    }

    /// The picks, then the extras, each key once.
    pub fn install_keys(&self) -> Vec<String> {
        let mut keys = self.selected_keys();
        for extra in &self.extras {
            if !keys.contains(extra) {
                keys.push(extra.clone());
            }
        }
        keys
    }
}

/// The path below the category, which the list caption already names; a
/// package that is its own category keeps its label.
fn option_label(path: &[&str]) -> String {
    match path {
        [_category, below @ ..] if !below.is_empty() => below.join(" / "),
        _ => path.join(" / "),
    }
}
