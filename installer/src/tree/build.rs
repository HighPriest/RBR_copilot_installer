impl super::Tree {
    /// The tree for ONE game: what the server listed, what the cache holds and
    /// what the game folder's ledger has `installed`. A leaf starts checked when
    /// it is installed or cached, because Apply uninstalls what is unchecked.
    pub fn build(
        game: &str,
        packages: &[pacenotes_shared::PackageInfo],
        cached: &[super::CachedPackage],
        installed: &[String],
    ) -> super::Tree {
        let mut rows = listed_rows(game, packages, cached, installed);
        rows.extend(local_only_rows(game, cached, installed, &rows));
        rows.extend(unavailable_rows(game, installed, &rows));
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
            installed: row.installed,
            checked: row.installed || row.state.has_payload(),
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
            installed: false,
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
    installed: bool,
}

/// The server's packages for `game`, with the cache deciding each one's state;
/// a name that could widen into a path is dropped, since a manifest can come
/// from a server the user does not run.
fn listed_rows(
    game: &str,
    packages: &[pacenotes_shared::PackageInfo],
    cached: &[super::CachedPackage],
    installed: &[String],
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
                installed: installed.contains(&key),
                key,
                state,
            }
        })
        .collect()
}

/// Cached packages for `game` that the server did not list.
fn local_only_rows(
    game: &str,
    cached: &[super::CachedPackage],
    installed: &[String],
    listed: &[Row],
) -> Vec<Row> {
    cached
        .iter()
        .filter(|entry| entry.game == game && pacenotes_shared::valid_name(&entry.name))
        .filter(|entry| !listed.iter().any(|row| row.key == entry.key()))
        .map(|entry| {
            let key = entry.key();
            Row {
                segments: entry.name.split('/').map(str::to_string).collect(),
                label: entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| leaf_of(&entry.name).to_string()),
                installed: installed.contains(&key),
                key,
                state: super::LeafState::LocalOnly,
            }
        })
        .collect()
}

/// Installed packages for `game` that neither the server nor the cache knows.
fn unavailable_rows(game: &str, installed: &[String], known: &[Row]) -> Vec<Row> {
    installed
        .iter()
        .filter(|key| !known.iter().any(|row| &row.key == *key))
        .filter_map(|key| {
            let (package_game, _, name) = pacenotes_shared::split_download_path(key)?;
            (package_game == game).then(|| Row {
                segments: name.split('/').map(str::to_string).collect(),
                label: leaf_of(name).to_string(),
                key: key.clone(),
                state: super::LeafState::Unavailable,
                installed: true,
            })
        })
        .collect()
}

fn leaf_of(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
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
