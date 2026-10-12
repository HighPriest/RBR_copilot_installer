//! The Download scene's tree: manifest + cache → nodes, states and checks.
//!
//! This is the model only. It holds no Win32 anything and no display strings
//! of its own — it reports a `LeafState` and the UI appends the localized
//! "(downloaded)" / "(update)" / "(local only)" suffix. That split is what
//! lets the whole thing be tested on any host.
//!
//! Shape comes from where a package SITS on the server: `PackageInfo.name` is
//! its path under the access directory, so `codriver/polish/male/Kamil` is
//! category `codriver`, levels `polish` → `male`, leaf `Kamil`.
//! See `installer/DESIGN.md` scene 4.

#![allow(dead_code)]

/// What the cache holds for one package, as the tree needs to see it.
///
/// Identity is `game` + `directory` + `name`: one server serves every game, so
/// two games can both hold `public/style/Numeric Standard` and a key of
/// "directory/name" would let one shadow the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedPackage {
    pub game: String,
    pub directory: String,
    pub name: String,
    pub sha256: String,
    pub version: String,
    pub display_name: Option<String>,
}

impl CachedPackage {
    /// `game/directory/name` — the same string `PackageInfo::download_path()`
    /// produces, and the key both the cache index and the ledger use.
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.game, self.directory, self.name)
    }
}

/// Where a leaf stands between the server and this computer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeafState {
    /// Listed by the server, not in the cache. Needs downloading.
    NotCached,
    /// In the cache at exactly the sha the server lists.
    Downloaded,
    /// In the cache, but the server lists a different sha.
    UpdateAvailable,
    /// In the cache and NOT listed by the server — an older package the
    /// operator has withdrawn, or one downloaded while a credential granted
    /// access that it no longer does. Still installable from the cache.
    LocalOnly,
}

impl LeafState {
    /// Is the payload already on this computer? Drives whether scene 6 has to
    /// fetch anything for this leaf.
    pub fn is_present(self) -> bool {
        matches!(
            self,
            LeafState::Downloaded | LeafState::UpdateAvailable | LeafState::LocalOnly
        )
    }
}

/// A parent's checkbox, derived from its descendants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    Unchecked,
    Checked,
    /// Some descendants checked, some not — `TVIS_STATEIMAGEMASK` partial.
    Partial,
}

/// One row. Grouping rows carry no package; leaves carry everything.
#[derive(Clone, Debug)]
pub struct Node {
    /// What to show, WITHOUT any state suffix — the UI adds that, localized.
    pub label: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Leaves only: the package's `game/directory/name`.
    pub key: Option<String>,
    /// Leaves only.
    pub state: Option<LeafState>,
    /// Leaves only. A parent's check is computed; see [`Tree::check_of`].
    checked: bool,
}

impl Node {
    pub fn is_leaf(&self) -> bool {
        self.key.is_some()
    }
}

/// Nodes in a flat arena: a Win32 tree view wants an integer per item to put
/// in `TVITEM::lParam`, and an index is exactly that.
#[derive(Clone, Debug, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    /// Indices of the top-level category rows, in display order.
    roots: Vec<usize>,
}

impl Tree {
    /// Build the tree for ONE game.
    ///
    /// `packages` is what the server listed (already its answer for this game;
    /// anything for another game is ignored rather than trusted). `cached` is
    /// what this computer holds. Leaves present in the cache start checked, as
    /// DESIGN.md asks — the common case is re-running the installer and
    /// keeping what you already have.
    pub fn build(
        game: &str,
        packages: &[pacenotes_shared::PackageInfo],
        cached: &[CachedPackage],
    ) -> Tree {
        let mut tree = Tree::default();

        // Server packages first, so a leaf that exists on both sides is the
        // server's (with the cache only deciding its state).
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut rows: Vec<Row> = Vec::new();

        for package in packages.iter().filter(|p| p.game == game) {
            if !pacenotes_shared::valid_name(&package.name) {
                continue; // a manifest can come from a server we do not run
            }
            let key = package.download_path();
            let state = match cached.iter().find(|c| c.key() == key) {
                Some(hit) if hit.sha256 == package.sha256 => LeafState::Downloaded,
                Some(_) => LeafState::UpdateAvailable,
                None => LeafState::NotCached,
            };
            seen.insert(key.clone());
            rows.push(Row {
                segments: package.name.split('/').map(str::to_string).collect(),
                label: package.label().to_string(),
                key,
                state,
            });
        }

        // Anything cached for this game that the server did not list.
        for entry in cached.iter().filter(|c| c.game == game) {
            let key = entry.key();
            if seen.contains(&key) || !pacenotes_shared::valid_name(&entry.name) {
                continue;
            }
            let leaf = entry.name.rsplit('/').next().unwrap_or(&entry.name);
            rows.push(Row {
                segments: entry.name.split('/').map(str::to_string).collect(),
                label: entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| leaf.to_string()),
                key,
                state: LeafState::LocalOnly,
            });
        }

        rows.sort_by(|a, b| {
            category_rank(&a.segments[0])
                .cmp(&category_rank(&b.segments[0]))
                .then_with(|| a.segments.cmp(&b.segments))
        });

        for row in rows {
            tree.insert(row);
        }
        tree
    }

    /// Walk/create the grouping rows for `row`, then hang the leaf off them.
    fn insert(&mut self, row: Row) {
        let mut parent: Option<usize> = None;
        // Every segment but the last is a grouping level.
        for segment in &row.segments[..row.segments.len() - 1] {
            parent = Some(self.child_named(parent, segment));
        }
        let checked = row.state.is_present();
        let node = Node {
            label: row.label,
            parent,
            children: Vec::new(),
            key: Some(row.key),
            state: Some(row.state),
            checked,
        };
        let index = self.push(node, parent);
        let _ = index;
    }

    /// Existing grouping child with this label, or a new one.
    fn child_named(&mut self, parent: Option<usize>, label: &str) -> usize {
        let siblings = match parent {
            Some(p) => self.nodes[p].children.clone(),
            None => self.roots.clone(),
        };
        if let Some(found) = siblings
            .into_iter()
            .find(|&i| !self.nodes[i].is_leaf() && self.nodes[i].label == label)
        {
            return found;
        }
        let node = Node {
            label: label.to_string(),
            parent,
            children: Vec::new(),
            key: None,
            state: None,
            checked: false,
        };
        self.push(node, parent)
    }

    fn push(&mut self, node: Node, parent: Option<usize>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        match parent {
            Some(p) => self.nodes[p].children.push(index),
            None => self.roots.push(index),
        }
        index
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    /// Top-level category rows, in display order.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    pub fn children(&self, index: usize) -> &[usize] {
        &self.nodes[index].children
    }

    /// Every leaf under `index` (or the whole tree for `None`), in order.
    pub fn leaves_under(&self, index: Option<usize>) -> Vec<usize> {
        let mut out = Vec::new();
        match index {
            Some(i) => self.collect_leaves(i, &mut out),
            None => {
                for &root in &self.roots {
                    self.collect_leaves(root, &mut out);
                }
            }
        }
        out
    }

    fn collect_leaves(&self, index: usize, out: &mut Vec<usize>) {
        if self.nodes[index].is_leaf() {
            out.push(index);
            return;
        }
        for &child in &self.nodes[index].children {
            self.collect_leaves(child, out);
        }
    }

    /// A leaf's own checkbox; a parent's, derived from its descendants.
    ///
    /// A grouping row with no leaves under it cannot happen from `build`, but
    /// is reported Unchecked rather than panicking.
    pub fn check_of(&self, index: usize) -> Check {
        if self.nodes[index].is_leaf() {
            return if self.nodes[index].checked {
                Check::Checked
            } else {
                Check::Unchecked
            };
        }
        let leaves = self.leaves_under(Some(index));
        let checked = leaves.iter().filter(|&&l| self.nodes[l].checked).count();
        match checked {
            0 => Check::Unchecked,
            n if n == leaves.len() => Check::Checked,
            _ => Check::Partial,
        }
    }

    /// Check or uncheck `index` and everything under it.
    ///
    /// DESIGN.md: checking a parent checks all its descendants. Unchecking a
    /// downloaded leaf only drops it from THIS run; nothing is removed from
    /// the cache, which is why this touches no state but the checkbox.
    pub fn set_checked(&mut self, index: usize, checked: bool) {
        for leaf in self.leaves_under(Some(index)) {
            self.nodes[leaf].checked = checked;
        }
        if self.nodes[index].is_leaf() {
            self.nodes[index].checked = checked;
        }
    }

    /// Keys of every checked leaf, in display order — what scene 4 commits.
    pub fn checked_keys(&self) -> Vec<String> {
        self.leaves_under(None)
            .into_iter()
            .filter(|&l| self.nodes[l].checked)
            .filter_map(|l| self.nodes[l].key.clone())
            .collect()
    }

    pub fn checked_leaves(&self) -> Vec<usize> {
        self.leaves_under(None)
            .into_iter()
            .filter(|&l| self.nodes[l].checked)
            .collect()
    }

    /// Top-level categories that have nothing checked under them.
    ///
    /// DESIGN.md: Download requires at least one checked leaf in EVERY
    /// category present in the tree, and the warning modal names the ones
    /// missing. A category only counts if it is in the tree at all, so a
    /// server offering no `pacenotes` does not block the user.
    pub fn categories_missing_a_pick(&self) -> Vec<String> {
        self.roots
            .iter()
            .filter(|&&root| self.check_of(root) == Check::Unchecked)
            .map(|&root| self.nodes[root].label.clone())
            .collect()
    }

    /// Checked leaves whose payload is not on this computer — what scene 6's
    /// download phase has to fetch.
    pub fn checked_needing_download(&self) -> Vec<String> {
        self.checked_leaves()
            .into_iter()
            .filter(|&l| !self.nodes[l].state.is_some_and(LeafState::is_present))
            .filter_map(|l| self.nodes[l].key.clone())
            .collect()
    }
}

/// A leaf on its way into the tree.
struct Row {
    segments: Vec<String>,
    label: String,
    key: String,
    state: LeafState,
}

/// Display order for the top level: the three the Configuration scene asks a
/// pick from, in the order it lists them, then `pacenotes`, then anything the
/// server invented, alphabetically. Deterministic ordering matters because the
/// tree is rebuilt whenever the manifest is refetched.
fn category_rank(category: &str) -> usize {
    pacenotes_shared::CHOICE_CATEGORIES
        .iter()
        .position(|c| *c == category)
        .or_else(|| {
            if category == pacenotes_shared::CATEGORY_PACENOTES {
                Some(pacenotes_shared::CHOICE_CATEGORIES.len())
            } else {
                None
            }
        })
        .unwrap_or(pacenotes_shared::CHOICE_CATEGORIES.len() + 1)
}
