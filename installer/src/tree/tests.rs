const RBR: &str = pacenotes_shared::GAME_RBR;
const DR2: &str = pacenotes_shared::GAME_DIRT_RALLY2;

fn package(game: &str, directory: &str, name: &str, sha: &str) -> pacenotes_shared::PackageInfo {
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

fn cached(game: &str, directory: &str, name: &str, sha: &str) -> super::CachedPackage {
    super::CachedPackage {
        game: game.to_string(),
        directory: directory.to_string(),
        name: name.to_string(),
        sha256: sha.to_string(),
        version: "1.0.0".into(),
        display_name: None,
        exclusive_group: None,
        requires: Vec::new(),
        size: 0,
        updated_at: 0,
    }
}

/// The real catalogue's shape: three single-pick categories plus pacenotes,
/// with the co-driver voices two levels deep.
fn catalogue() -> Vec<pacenotes_shared::PackageInfo> {
    vec![
        package(RBR, "public", "style/Descriptive", "s1"),
        package(RBR, "public", "style/Numeric Standard", "s2"),
        package(RBR, "public", "language/english", "l1"),
        package(RBR, "public", "pacenotes/RBR", "p1"),
        package(RBR, "public", "codriver/polish/male/Kamil", "c1"),
        package(RBR, "public", "codriver/polish/female/Ewa", "c2"),
        package(RBR, "public", "codriver/english/male/John", "c3"),
    ]
}

/// The node reached by following these labels down from the roots.
fn find(tree: &super::Tree, path: &[&str]) -> usize {
    let mut level: Vec<usize> = tree.roots().to_vec();
    let mut found = usize::MAX;
    for label in path {
        found = *level
            .iter()
            .find(|&&index| tree.node(index).label == *label)
            .unwrap_or_else(|| panic!("no node {label:?} among {:?}", labels(tree, &level)));
        level = tree.children(found).to_vec();
    }
    found
}

fn labels(tree: &super::Tree, indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .map(|&index| tree.node(index).label.clone())
        .collect()
}

#[test]
fn the_storage_path_becomes_the_tree() {
    let tree = super::Tree::build(RBR, &catalogue(), &[]);
    assert_eq!(
        labels(&tree, tree.roots()),
        vec!["language", "codriver", "style", "pacenotes"]
    );

    let kamil = find(&tree, &["codriver", "polish", "male", "Kamil"]);
    assert!(tree.node(kamil).is_leaf());
    assert_eq!(
        tree.node(kamil).key.as_deref(),
        Some("rbr/public/codriver/polish/male/Kamil")
    );
    assert!(tree.node(find(&tree, &["style", "Descriptive"])).is_leaf());
}

/// The server may reorder the manifest; the tree must not follow.
#[test]
fn category_order_is_deterministic_and_matches_the_configuration_scene() {
    let mut shuffled = catalogue();
    shuffled.reverse();
    let tree = super::Tree::build(RBR, &shuffled, &[]);
    assert_eq!(
        labels(&tree, tree.roots()),
        vec!["language", "codriver", "style", "pacenotes"]
    );
    assert_eq!(
        labels(&tree, tree.roots())[..3],
        pacenotes_shared::CHOICE_CATEGORIES
    );
}

#[test]
fn packages_sharing_a_level_share_its_row() {
    let tree = super::Tree::build(RBR, &catalogue(), &[]);
    let polish = find(&tree, &["codriver", "polish"]);
    assert_eq!(labels(&tree, tree.children(polish)), vec!["female", "male"]);
    assert_eq!(tree.leaves_under(Some(polish)).len(), 2);
}

#[test]
fn cache_decides_each_leafs_state() {
    let cache = vec![
        cached(RBR, "public", "style/Descriptive", "s1"),
        cached(RBR, "public", "language/english", "OLD"),
    ];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);

    for (path, state) in [
        (&["style", "Descriptive"][..], super::LeafState::Downloaded),
        (&["language", "english"], super::LeafState::UpdateAvailable),
        (&["pacenotes", "RBR"], super::LeafState::NotCached),
    ] {
        assert_eq!(tree.node(find(&tree, path)).state, Some(state), "{path:?}");
    }
}

#[test]
fn a_cached_package_the_server_dropped_still_appears() {
    let cache = vec![cached(RBR, "public", "codriver/polish/male/Retired", "r1")];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);

    let retired = find(&tree, &["codriver", "polish", "male", "Retired"]);
    assert_eq!(tree.node(retired).state, Some(super::LeafState::LocalOnly));
    let male = find(&tree, &["codriver", "polish", "male"]);
    assert_eq!(tree.leaves_under(Some(male)).len(), 2);
}

/// Two games can hold the same `directory/name`, so only the game keeps them apart.
#[test]
fn another_games_packages_are_not_in_this_games_tree() {
    let mut packages = catalogue();
    packages.push(package(DR2, "public", "style/Descriptive", "other"));
    let cache = vec![cached(DR2, "public", "codriver/polish/male/Nobody", "n1")];

    let tree = super::Tree::build(RBR, &packages, &cache);

    assert_eq!(tree.leaves_under(Some(find(&tree, &["style"]))).len(), 2);
    assert!(!tree
        .checked_keys()
        .iter()
        .any(|key| key.starts_with(&format!("{DR2}/"))));
    assert_eq!(tree.leaves_under(None).len(), catalogue().len());
}

#[test]
fn what_is_already_downloaded_starts_checked() {
    let cache = vec![
        cached(RBR, "public", "style/Descriptive", "s1"),
        cached(RBR, "public", "codriver/polish/male/Retired", "r1"),
    ];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);

    assert_eq!(
        tree.checked_keys(),
        vec![
            "rbr/public/codriver/polish/male/Retired".to_string(),
            "rbr/public/style/Descriptive".to_string(),
        ]
    );
    assert!(tree.checked_needing_download().is_empty());
}

#[test]
fn checking_a_parent_takes_its_whole_subtree() {
    let mut tree = super::Tree::build(RBR, &catalogue(), &[]);
    let codriver = find(&tree, &["codriver"]);
    assert_eq!(tree.check_of(codriver), super::Check::Unchecked);

    tree.set_checked(codriver, true);
    assert_eq!(tree.check_of(codriver), super::Check::Checked);
    assert_eq!(tree.leaves_under(Some(codriver)).len(), 3);
    assert_eq!(tree.checked_keys().len(), 3);

    tree.set_checked(find(&tree, &["codriver", "polish", "male", "Kamil"]), false);
    assert_eq!(tree.check_of(codriver), super::Check::Partial);
    assert_eq!(
        tree.check_of(find(&tree, &["codriver", "polish"])),
        super::Check::Partial
    );
    assert_eq!(
        tree.check_of(find(&tree, &["codriver", "english"])),
        super::Check::Checked
    );

    tree.set_checked(codriver, false);
    assert_eq!(tree.check_of(codriver), super::Check::Unchecked);
    assert!(tree.checked_keys().is_empty());
}

#[test]
fn unchecking_a_downloaded_leaf_leaves_it_in_the_cache() {
    let cache = vec![cached(RBR, "public", "style/Descriptive", "s1")];
    let mut tree = super::Tree::build(RBR, &catalogue(), &cache);
    let leaf = find(&tree, &["style", "Descriptive"]);

    tree.set_checked(leaf, false);
    assert_eq!(tree.check_of(leaf), super::Check::Unchecked);
    assert_eq!(tree.node(leaf).state, Some(super::LeafState::Downloaded));
}

#[test]
fn download_requires_one_pick_per_category_present() {
    let mut tree = super::Tree::build(RBR, &catalogue(), &[]);
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

    let without_pacenotes: Vec<_> = catalogue()
        .into_iter()
        .filter(|package| package.category() != pacenotes_shared::CATEGORY_PACENOTES)
        .collect();
    let tree = super::Tree::build(RBR, &without_pacenotes, &[]);
    assert_eq!(
        tree.categories_missing_a_pick(),
        vec!["language", "codriver", "style"]
    );
}

#[test]
fn only_checked_leaves_without_a_payload_need_downloading() {
    let cache = vec![cached(RBR, "public", "style/Descriptive", "s1")];
    let mut tree = super::Tree::build(RBR, &catalogue(), &cache);

    tree.set_checked(find(&tree, &["language", "english"]), true);
    assert_eq!(
        tree.checked_needing_download(),
        vec!["rbr/public/language/english".to_string()]
    );
}

/// A republished package must reach whoever already has the old one.
#[test]
fn an_update_is_ticked_by_default_and_still_downloaded() {
    let cache = vec![cached(RBR, "public", "language/english", "OLD-SHA")];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);
    let stale = find(&tree, &["language", "english"]);

    assert_eq!(
        tree.node(stale).state,
        Some(super::LeafState::UpdateAvailable)
    );
    assert_eq!(tree.check_of(stale), super::Check::Checked);
    assert!(super::LeafState::UpdateAvailable.has_payload());
    assert!(tree
        .checked_needing_download()
        .contains(&"rbr/public/language/english".to_string()));
}

#[test]
fn a_leaf_already_at_the_right_sha_is_not_refetched() {
    let cache = vec![cached(RBR, "public", "style/Descriptive", "s1")];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);
    let leaf = find(&tree, &["style", "Descriptive"]);

    assert_eq!(tree.node(leaf).state, Some(super::LeafState::Downloaded));
    assert_eq!(tree.check_of(leaf), super::Check::Checked);
    assert!(tree.checked_needing_download().is_empty());
}

/// The server no longer has it, so there is nothing to fetch.
#[test]
fn a_local_only_leaf_is_never_queued_for_download() {
    let cache = vec![cached(RBR, "public", "style/Withdrawn", "w1")];
    let tree = super::Tree::build(RBR, &catalogue(), &cache);
    let leaf = find(&tree, &["style", "Withdrawn"]);

    assert_eq!(tree.node(leaf).state, Some(super::LeafState::LocalOnly));
    assert!(super::LeafState::LocalOnly.has_payload());
    assert!(!super::LeafState::LocalOnly.needs_download());
    assert!(tree.checked_needing_download().is_empty());
}

#[test]
fn display_name_renames_only_the_leaf() {
    let mut packages = catalogue();
    packages[4].display_name = Some("Kamil Filipek".into());
    let tree = super::Tree::build(RBR, &packages, &[]);

    let renamed = find(&tree, &["codriver", "polish", "male", "Kamil Filipek"]);
    assert_eq!(
        tree.node(renamed).key.as_deref(),
        Some("rbr/public/codriver/polish/male/Kamil")
    );
}

#[test]
fn a_malformed_name_never_enters_the_tree() {
    let packages = vec![
        package(RBR, "public", "../../etc/passwd", "x"),
        package(RBR, "public", "style/../../escape", "y"),
        package(RBR, "public", "style/Descriptive", "ok"),
    ];
    let tree = super::Tree::build(RBR, &packages, &[]);
    let leaves = tree.leaves_under(None);
    assert_eq!(leaves.len(), 1);
    assert_eq!(
        tree.node(leaves[0]).key.as_deref(),
        Some("rbr/public/style/Descriptive")
    );
}

/// Offline with an empty cache is a normal state.
#[test]
fn an_empty_catalogue_is_handled() {
    let tree = super::Tree::build(RBR, &[], &[]);
    assert!(tree.is_empty());
    assert!(tree.roots().is_empty());
    assert!(tree.checked_keys().is_empty());
    assert!(tree.categories_missing_a_pick().is_empty());
    assert!(tree.checked_needing_download().is_empty());
}
