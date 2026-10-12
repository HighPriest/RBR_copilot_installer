//! AUTOMATED TEST for `pacenotes_installer::tree`, the Download scene's model.
//!
//! Cross-platform: pure std, no Win32, so this runs in CI on any host.
//!
//! Fixtures are shaped like the real package layout — `style/Descriptive`,
//! `language/english`, `codriver/polish/male/Kamil` — because the whole point
//! of the model is turning THOSE paths into the tree the user clicks.

use pacenotes_installer::tree::{CachedPackage, Check, LeafState, Tree};

const RBR: &str = pacenotes_shared::GAME_RBR;
const DR2: &str = pacenotes_shared::GAME_DIRT_RALLY2;

/// A manifest entry. `name` is the package's path under its access directory,
/// which is the only thing deciding where it lands in the tree.
fn pkg(game: &str, directory: &str, name: &str, sha: &str) -> pacenotes_shared::PackageInfo {
    pacenotes_shared::PackageInfo {
        directory: directory.to_string(),
        name: name.to_string(),
        version: "1.0.0".into(),
        size: 10,
        sha256: sha.to_string(),
        updated_at: 1,
        game: game.to_string(),
        display_name: None,
        description: None,
        exclusive_group: None,
        requires: Vec::new(),
    }
}

fn cached(game: &str, directory: &str, name: &str, sha: &str) -> CachedPackage {
    CachedPackage {
        game: game.to_string(),
        directory: directory.to_string(),
        name: name.to_string(),
        sha256: sha.to_string(),
        version: "1.0.0".into(),
        display_name: None,
    }
}

/// The real-world shape: three single-pick categories plus pacenotes, with the
/// co-driver voices nested two levels deep.
fn catalogue() -> Vec<pacenotes_shared::PackageInfo> {
    vec![
        pkg(RBR, "public", "style/Descriptive", "s1"),
        pkg(RBR, "public", "style/Numeric Standard", "s2"),
        pkg(RBR, "public", "language/english", "l1"),
        pkg(RBR, "public", "pacenotes/RBR", "p1"),
        pkg(RBR, "public", "codriver/polish/male/Kamil", "c1"),
        pkg(RBR, "public", "codriver/polish/female/Ewa", "c2"),
        pkg(RBR, "public", "codriver/english/male/John", "c3"),
    ]
}

/// Walk from the roots by label; returns the node index.
fn find(tree: &Tree, path: &[&str]) -> usize {
    let mut level: Vec<usize> = tree.roots().to_vec();
    let mut found = usize::MAX;
    for label in path {
        found = *level
            .iter()
            .find(|&&i| tree.node(i).label == *label)
            .unwrap_or_else(|| panic!("no node {label:?} among {:?}", labels(tree, &level)));
        level = tree.children(found).to_vec();
    }
    found
}

fn labels(tree: &Tree, indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .map(|&i| tree.node(i).label.clone())
        .collect()
}

/// A package's path becomes its place in the tree: the folders the operator
/// made ARE the levels the user clicks through.
#[test]
fn the_storage_path_becomes_the_tree() {
    let tree = Tree::build(RBR, &catalogue(), &[]);

    // Top level is the categories, not the packages.
    assert_eq!(
        labels(&tree, tree.roots()),
        vec!["language", "codriver", "style", "pacenotes"]
    );

    // A two-level-deep voice hangs off codriver → polish → male.
    let kamil = find(&tree, &["codriver", "polish", "male", "Kamil"]);
    assert!(tree.node(kamil).is_leaf());
    assert_eq!(
        tree.node(kamil).key.as_deref(),
        Some("rbr/public/codriver/polish/male/Kamil")
    );

    // A package directly under its category has no middle levels.
    let descriptive = find(&tree, &["style", "Descriptive"]);
    assert!(tree.node(descriptive).is_leaf());
}

/// Categories lead with the three the Configuration scene asks a pick from, in
/// the order it lists them, so the tree and that scene agree. The order must
/// not depend on manifest order, which the server is free to change.
#[test]
fn category_order_is_deterministic_and_matches_the_configuration_scene() {
    let mut shuffled = catalogue();
    shuffled.reverse();
    let tree = Tree::build(RBR, &shuffled, &[]);
    assert_eq!(
        labels(&tree, tree.roots()),
        vec!["language", "codriver", "style", "pacenotes"]
    );
    // And the first three are exactly the scene-5 lists, in its order.
    assert_eq!(
        labels(&tree, tree.roots())[..3],
        pacenotes_shared::CHOICE_CATEGORIES
    );
}

/// Grouping levels are shared, not repeated per package: two Polish voices
/// produce ONE `polish` row.
#[test]
fn packages_sharing_a_level_share_its_row() {
    let tree = Tree::build(RBR, &catalogue(), &[]);
    let polish = find(&tree, &["codriver", "polish"]);
    assert_eq!(labels(&tree, tree.children(polish)), vec!["female", "male"]);
    assert_eq!(tree.leaves_under(Some(polish)).len(), 2);
}

/// The three states a server package can be in, which drive the suffix the UI
/// appends and whether scene 6 has to fetch anything.
#[test]
fn cache_decides_each_leafs_state() {
    let cache = vec![
        cached(RBR, "public", "style/Descriptive", "s1"), // same sha
        cached(RBR, "public", "language/english", "OLD"), // server moved on
    ];
    let tree = Tree::build(RBR, &catalogue(), &cache);

    let same = find(&tree, &["style", "Descriptive"]);
    assert_eq!(tree.node(same).state, Some(LeafState::Downloaded));

    let stale = find(&tree, &["language", "english"]);
    assert_eq!(tree.node(stale).state, Some(LeafState::UpdateAvailable));

    let absent = find(&tree, &["pacenotes", "RBR"]);
    assert_eq!(tree.node(absent).state, Some(LeafState::NotCached));
}

/// A package in the cache that the server no longer lists is still installable
/// — the operator may have withdrawn it, or a credential may have lapsed. It
/// must appear, in its proper place, marked local-only.
#[test]
fn a_cached_package_the_server_dropped_still_appears() {
    let cache = vec![cached(RBR, "public", "codriver/polish/male/Retired", "r1")];
    let tree = Tree::build(RBR, &catalogue(), &cache);

    let retired = find(&tree, &["codriver", "polish", "male", "Retired"]);
    assert_eq!(tree.node(retired).state, Some(LeafState::LocalOnly));
    // Nested under the same rows as the server's own Polish voices.
    assert_eq!(
        tree.leaves_under(Some(find(&tree, &["codriver", "polish", "male"])))
            .len(),
        2
    );
}

/// One server serves every game. A manifest or cache entry for another game
/// must not leak into this game's tree — it would offer the user content that
/// cannot work, and the two can share a directory+name.
#[test]
fn another_games_packages_are_not_in_this_games_tree() {
    let mut packages = catalogue();
    packages.push(pkg(DR2, "public", "style/Descriptive", "other"));
    let cache = vec![cached(DR2, "public", "codriver/polish/male/Nobody", "n1")];

    let tree = Tree::build(RBR, &packages, &cache);

    // Same directory+name as an RBR package, but the other game's: one leaf.
    assert_eq!(tree.leaves_under(Some(find(&tree, &["style"]))).len(), 2);
    assert!(!tree
        .checked_keys()
        .iter()
        .any(|k| k.starts_with(&format!("{DR2}/"))));
    assert_eq!(tree.leaves_under(None).len(), catalogue().len());
}

/// Re-running the installer should keep what you already have, so anything on
/// this computer starts checked and anything that would need fetching does not.
#[test]
fn what_is_already_downloaded_starts_checked() {
    let cache = vec![
        cached(RBR, "public", "style/Descriptive", "s1"),
        cached(RBR, "public", "codriver/polish/male/Retired", "r1"),
    ];
    let tree = Tree::build(RBR, &catalogue(), &cache);

    assert_eq!(
        tree.checked_keys(),
        vec![
            "rbr/public/codriver/polish/male/Retired".to_string(),
            "rbr/public/style/Descriptive".to_string(),
        ]
    );
    // Nothing checked needs fetching, so scene 6 would have no download phase.
    assert!(tree.checked_needing_download().is_empty());
}

/// Checking a parent takes everything under it, and a parent whose children
/// disagree is partial — the tri-state the Win32 checkbox renders.
#[test]
fn checking_a_parent_takes_its_whole_subtree() {
    let mut tree = Tree::build(RBR, &catalogue(), &[]);
    let codriver = find(&tree, &["codriver"]);
    assert_eq!(tree.check_of(codriver), Check::Unchecked);

    tree.set_checked(codriver, true);
    assert_eq!(tree.check_of(codriver), Check::Checked);
    assert_eq!(tree.leaves_under(Some(codriver)).len(), 3);
    assert_eq!(tree.checked_keys().len(), 3);

    // Drop one grandchild: every ancestor becomes partial, none unchecked.
    tree.set_checked(find(&tree, &["codriver", "polish", "male", "Kamil"]), false);
    assert_eq!(tree.check_of(codriver), Check::Partial);
    assert_eq!(
        tree.check_of(find(&tree, &["codriver", "polish"])),
        Check::Partial
    );
    assert_eq!(
        tree.check_of(find(&tree, &["codriver", "english"])),
        Check::Checked
    );

    tree.set_checked(codriver, false);
    assert_eq!(tree.check_of(codriver), Check::Unchecked);
    assert!(tree.checked_keys().is_empty());
}

/// Unchecking a downloaded leaf drops it from this run only. Its state must
/// not change — the payload stays in the cache, as DESIGN.md requires.
#[test]
fn unchecking_a_downloaded_leaf_leaves_it_in_the_cache() {
    let cache = vec![cached(RBR, "public", "style/Descriptive", "s1")];
    let mut tree = Tree::build(RBR, &catalogue(), &cache);
    let leaf = find(&tree, &["style", "Descriptive"]);

    tree.set_checked(leaf, false);
    assert_eq!(tree.check_of(leaf), Check::Unchecked);
    assert_eq!(tree.node(leaf).state, Some(LeafState::Downloaded));
}

/// Download needs a pick in every category the tree actually offers, and the
/// warning names the ones missing. A category the server does not offer must
/// not be demanded.
#[test]
fn download_requires_one_pick_per_category_present() {
    let mut tree = Tree::build(RBR, &catalogue(), &[]);
    assert_eq!(
        tree.categories_missing_a_pick(),
        vec!["language", "codriver", "style", "pacenotes"]
    );

    tree.set_checked(find(&tree, &["language", "english"]), true);
    tree.set_checked(find(&tree, &["codriver", "polish", "male", "Kamil"]), true);
    assert_eq!(tree.categories_missing_a_pick(), vec!["style", "pacenotes"]);

    tree.set_checked(find(&tree, &["style", "Descriptive"]), true);
    tree.set_checked(find(&tree, &["pacenotes", "RBR"]), true);
    assert!(tree.categories_missing_a_pick().is_empty());

    // A catalogue without `pacenotes` must not demand one.
    let partial: Vec<_> = catalogue()
        .into_iter()
        .filter(|p| p.category() != "pacenotes")
        .collect();
    let tree = Tree::build(RBR, &partial, &[]);
    assert_eq!(
        tree.categories_missing_a_pick(),
        vec!["language", "codriver", "style"]
    );
}

/// Scene 6's download phase gets exactly the checked leaves whose payload is
/// missing — not every checked leaf, or it would refetch the whole cache.
#[test]
fn only_checked_leaves_without_a_payload_need_downloading() {
    let cache = vec![cached(RBR, "public", "style/Descriptive", "s1")];
    let mut tree = Tree::build(RBR, &catalogue(), &cache);

    tree.set_checked(find(&tree, &["language", "english"]), true);
    assert_eq!(
        tree.checked_needing_download(),
        vec!["rbr/public/language/english".to_string()]
    );

    // An update IS present locally, but at the wrong sha, so it must refetch.
    let cache = vec![cached(RBR, "public", "language/english", "OLD")];
    let tree = Tree::build(RBR, &catalogue(), &cache);
    let stale = find(&tree, &["language", "english"]);
    assert_eq!(tree.node(stale).state, Some(LeafState::UpdateAvailable));
    assert!(LeafState::UpdateAvailable.is_present());
}

/// `display_name` renames the LEAF only; the levels above it come from the
/// path and must be untouched by it.
#[test]
fn display_name_renames_only_the_leaf() {
    let mut packages = catalogue();
    packages[4].display_name = Some("Kamil Filipek".into());
    let tree = Tree::build(RBR, &packages, &[]);

    let renamed = find(&tree, &["codriver", "polish", "male", "Kamil Filipek"]);
    assert_eq!(
        tree.node(renamed).key.as_deref(),
        Some("rbr/public/codriver/polish/male/Kamil")
    );
}

/// A manifest can arrive from a server we do not run, so a name that could
/// widen into a path must never reach the tree.
#[test]
fn a_malformed_name_never_enters_the_tree() {
    let packages = vec![
        pkg(RBR, "public", "../../etc/passwd", "x"),
        pkg(RBR, "public", "style/../../escape", "y"),
        pkg(RBR, "public", "style/Descriptive", "ok"),
    ];
    let tree = Tree::build(RBR, &packages, &[]);
    assert_eq!(tree.leaves_under(None).len(), 1);
    assert_eq!(
        tree.node(tree.leaves_under(None)[0]).key.as_deref(),
        Some("rbr/public/style/Descriptive")
    );
}

/// An empty catalogue is a normal state — offline with an empty cache — and
/// must not panic anything the scene calls.
#[test]
fn an_empty_catalogue_is_handled() {
    let tree = Tree::build(RBR, &[], &[]);
    assert!(tree.is_empty());
    assert!(tree.roots().is_empty());
    assert!(tree.checked_keys().is_empty());
    assert!(tree.categories_missing_a_pick().is_empty());
    assert!(tree.checked_needing_download().is_empty());
}
