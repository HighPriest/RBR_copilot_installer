impl super::Tree {
    /// The tree for ONE game: what the server listed plus what the cache holds,
    /// with every leaf already on this computer starting checked.
    pub fn build(
        game: &str,
        packages: &[pacenotes_shared::PackageInfo],
        cached: &[super::CachedPackage],
    ) -> super::Tree {
        let mut rows = listed_rows(game, packages, cached);
        rows.extend(local_only_rows(game, cached, &rows));
        rows.sort_by(|a, b| {
            category_rank(&a.segments[0])
                .cmp(&category_rank(&b.segments[0]))
                .then_with(|| a.segments.cmp(&b.segments))
        });

        let mut tree = super::Tree::default();
        for row in rows {
            tree.insert(row);
        }
        tree
    }

    fn insert(&mut self, row: Row) {
        let mut parent: Option<usize> = None;
        let levels = &row.segments[..row.segments.len().saturating_sub(1)];
        for level in levels {
            parent = Some(self.child_named(parent, level));
        }
        let node = super::Node {
            label: row.label,
            parent,
            children: Vec::new(),
            key: Some(row.key),
            state: Some(row.state),
            checked: row.state.has_payload(),
        };
        self.push(node, parent);
    }

    /// The grouping child of `parent` with this label, created when missing.
    fn child_named(&mut self, parent: Option<usize>, label: &str) -> usize {
        let siblings = match parent {
            Some(parent) => &self.nodes[parent].children,
            None => &self.roots,
        };
        let existing = siblings
            .iter()
            .copied()
            .find(|&index| !self.nodes[index].is_leaf() && self.nodes[index].label == label);
        if let Some(existing) = existing {
            return existing;
        }
        let node = super::Node {
            label: label.to_string(),
            parent,
            children: Vec::new(),
            key: None,
            state: None,
            checked: false,
        };
        self.push(node, parent)
    }
}

/// A leaf on its way into the tree.
struct Row {
    segments: Vec<String>,
    label: String,
    key: String,
    state: super::LeafState,
}

/// The server's packages for `game`, with the cache deciding each one's state;
/// a name that could widen into a path is dropped, since a manifest can come
/// from a server the user does not run.
fn listed_rows(
    game: &str,
    packages: &[pacenotes_shared::PackageInfo],
    cached: &[super::CachedPackage],
) -> Vec<Row> {
    packages
        .iter()
        .filter(|package| package.game == game && pacenotes_shared::valid_name(&package.name))
        .map(|package| {
            let key = package.download_path();
            let state = match cached.iter().find(|entry| entry.key() == key) {
                Some(entry) if entry.sha256 == package.sha256 => super::LeafState::Downloaded,
                Some(_) => super::LeafState::UpdateAvailable,
                None => super::LeafState::NotCached,
            };
            Row {
                segments: package.segments().map(str::to_string).collect(),
                label: package.label().to_string(),
                key,
                state,
            }
        })
        .collect()
}

/// Cached packages for `game` that the server did not list.
fn local_only_rows(game: &str, cached: &[super::CachedPackage], listed: &[Row]) -> Vec<Row> {
    cached
        .iter()
        .filter(|entry| entry.game == game && pacenotes_shared::valid_name(&entry.name))
        .filter(|entry| !listed.iter().any(|row| row.key == entry.key()))
        .map(|entry| {
            let leaf = entry.name.rsplit('/').next().unwrap_or(&entry.name);
            Row {
                segments: entry.name.split('/').map(str::to_string).collect(),
                label: entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| leaf.to_string()),
                key: entry.key(),
                state: super::LeafState::LocalOnly,
            }
        })
        .collect()
}

/// The Configuration scene's categories in its order, then `pacenotes`, then
/// any other category alphabetically, so a refetch never reorders the tree.
fn category_rank(category: &str) -> usize {
    let choices = &pacenotes_shared::CHOICE_CATEGORIES;
    match choices.iter().position(|choice| *choice == category) {
        Some(rank) => rank,
        None if category == pacenotes_shared::CATEGORY_PACENOTES => choices.len(),
        None => choices.len() + 1,
    }
}
