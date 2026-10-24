impl super::Tree {
    /// A leaf's own checkbox, or a grouping row's derived from its leaves.
    pub fn check_of(&self, index: usize) -> super::Check {
        let node = &self.nodes[index];
        if node.is_leaf() {
            return if node.checked {
                super::Check::Checked
            } else {
                super::Check::Unchecked
            };
        }
        let leaves = self.leaves_under(Some(index));
        let checked_count = leaves
            .iter()
            .filter(|&&leaf| self.nodes[leaf].checked)
            .count();
        match checked_count {
            0 => super::Check::Unchecked,
            count if count == leaves.len() => super::Check::Checked,
            _ => super::Check::Partial,
        }
    }

    /// Checks or unchecks `index` and everything under it; the cache is never touched.
    pub fn set_checked(&mut self, index: usize, checked: bool) {
        for leaf in self.leaves_under(Some(index)) {
            self.nodes[leaf].checked = checked;
        }
    }

    pub fn checked_leaves(&self) -> Vec<usize> {
        self.leaves_under(None)
            .into_iter()
            .filter(|&leaf| self.nodes[leaf].checked)
            .collect()
    }

    /// The keys of every checked leaf, in display order.
    pub fn checked_keys(&self) -> Vec<String> {
        self.checked_leaves()
            .into_iter()
            .filter_map(|leaf| self.nodes[leaf].key.clone())
            .collect()
    }

    /// The categories present in the tree with nothing checked under them.
    pub fn categories_missing_a_pick(&self) -> Vec<String> {
        self.roots
            .iter()
            .filter(|&&root| self.check_of(root) == super::Check::Unchecked)
            .map(|&root| self.nodes[root].label.clone())
            .collect()
    }

    /// The checked leaves scene 6 fetches, updates included.
    pub fn checked_needing_download(&self) -> Vec<String> {
        self.checked_leaves()
            .into_iter()
            .filter(|&leaf| {
                self.nodes[leaf]
                    .state
                    .is_some_and(super::LeafState::needs_download)
            })
            .filter_map(|leaf| self.nodes[leaf].key.clone())
            .collect()
    }
}
