//! Fixtures follow the live `rbr` catalogue's shape, such as
//! `codriver/polish/male/PiotrSielilo`.

const RBR: &str = pacenotes_shared::GAME_RBR;

/// Stated here, so a schema change fails a test instead of silently changing the UI.
const CHOICES: [&str; 3] = [
    pacenotes_shared::CATEGORY_LANGUAGE,
    pacenotes_shared::CATEGORY_CODRIVER,
    pacenotes_shared::CATEGORY_STYLE,
];

fn package(name: &str) -> pacenotes_shared::PackageInfo {
    pacenotes_shared::PackageInfo {
        directory: "public".into(),
        name: name.to_string(),
        version: "1.0.0".into(),
        size: 10,
        sha256: format!("sha-of-{name}"),
        updated_at: 1,
        game: RBR.to_string(),
        display_name: None,
        description: None,
        exclusive_group: None,
        requires: Vec::new(),
    }
}

/// Nothing cached, so nothing starts ticked.
fn tree_of(names: &[&str]) -> crate::tree::Tree {
    let packages: Vec<_> = names.iter().map(|name| package(name)).collect();
    crate::tree::Tree::build(RBR, &packages, &[], &[])
}

fn leaf_of(tree: &crate::tree::Tree, key: &str) -> usize {
    tree.leaves_under(None)
        .into_iter()
        .find(|&leaf| tree.node(leaf).key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no leaf for {key}"))
}

fn tick(tree: &mut crate::tree::Tree, name: &str) {
    let leaf = leaf_of(tree, &format!("{RBR}/public/{name}"));
    tree.set_checked(leaf, true);
}

fn tree_with_ticked(names: &[&str]) -> crate::tree::Tree {
    let mut tree = tree_of(names);
    for name in names {
        tick(&mut tree, name);
    }
    tree
}

fn labels(configuration: &super::Configuration, list: usize) -> Vec<String> {
    configuration.lists[list]
        .options
        .iter()
        .map(|option| option.label.clone())
        .collect()
}

#[test]
fn three_lists_in_order_with_the_codriver_shown_by_its_path() {
    let tree = tree_with_ticked(&[
        "language/czech",
        "codriver/polish/male/PiotrSielilo",
        "style/Numeric Standard",
    ]);
    let configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);

    let categories: Vec<&str> = configuration
        .lists
        .iter()
        .map(|list| list.category)
        .collect();
    assert_eq!(categories, vec!["language", "codriver", "style"]);
    assert_eq!(labels(&configuration, 0), vec!["czech".to_string()]);
    assert_eq!(
        labels(&configuration, 1),
        vec!["polish / male / PiotrSielilo".to_string()]
    );
    assert_eq!(
        labels(&configuration, 2),
        vec!["Numeric Standard".to_string()]
    );
}

/// The same voice name exists under several languages.
#[test]
fn two_voices_with_the_same_name_are_told_apart() {
    let mut tree = tree_of(&[
        "codriver/polish/male/Kamil",
        "codriver/czech/male/Kamil",
        "language/czech",
        "style/Numeric Standard",
    ]);
    tick(&mut tree, "codriver/polish/male/Kamil");
    tick(&mut tree, "codriver/czech/male/Kamil");
    let voices = labels(&super::Configuration::from_tree(&tree, &[], &CHOICES), 1);

    assert_eq!(voices.len(), 2);
    assert!(voices.contains(&"czech / male / Kamil".to_string()));
    assert!(voices.contains(&"polish / male / Kamil".to_string()));
}

#[test]
fn a_single_option_is_preselected_and_several_are_not() {
    let tree = tree_with_ticked(&[
        "language/czech",
        "language/polish",
        "style/Numeric Standard",
    ]);
    let configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);

    assert_eq!(configuration.lists[0].selected, None);
    assert_eq!(
        configuration.lists[2].selected_key(),
        Some(format!("{RBR}/public/style/Numeric Standard").as_str())
    );
}

#[test]
fn install_waits_for_the_lists_that_have_options() {
    let tree = tree_with_ticked(&[
        "language/czech",
        "language/polish",
        "codriver/polish/male/Kamil",
    ]);
    let mut configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);

    assert!(!configuration.is_complete());
    assert_eq!(configuration.missing(), vec!["language"]);
    configuration.select(0, Some(1));
    assert!(configuration.is_complete());
    assert!(configuration.missing().is_empty());
}

#[test]
fn pacenotes_packages_are_installed_without_being_chosen() {
    let tree = tree_with_ticked(&[
        "pacenotes/Monte Carlo",
        "pacenotes/Sweden",
        "language/czech",
        "codriver/polish/male/Kamil",
        "style/Numeric Standard",
    ]);
    let configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);

    assert_eq!(configuration.lists.len(), 3);
    assert!(configuration.is_complete());
    assert_eq!(
        configuration.extras,
        vec![
            format!("{RBR}/public/pacenotes/Monte Carlo"),
            format!("{RBR}/public/pacenotes/Sweden"),
        ]
    );
    assert_eq!(
        configuration.install_keys(),
        vec![
            format!("{RBR}/public/language/czech"),
            format!("{RBR}/public/codriver/polish/male/Kamil"),
            format!("{RBR}/public/style/Numeric Standard"),
            format!("{RBR}/public/pacenotes/Monte Carlo"),
            format!("{RBR}/public/pacenotes/Sweden"),
        ]
    );
}

#[test]
fn a_pick_survives_a_trip_back_to_the_tree() {
    let mut tree = tree_of(&[
        "language/czech",
        "language/polish",
        "codriver/polish/male/Kamil",
        "codriver/polish/female/Ania",
        "style/Numeric Standard",
    ]);
    for name in [
        "language/czech",
        "language/polish",
        "codriver/polish/male/Kamil",
        "style/Numeric Standard",
    ] {
        tick(&mut tree, name);
    }
    let mut configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);
    configuration.select(0, Some(1));
    let picked = configuration.selected_keys();
    assert_eq!(picked[0], format!("{RBR}/public/language/polish"));

    tick(&mut tree, "codriver/polish/female/Ania");
    let configuration = super::Configuration::from_tree(&tree, &picked, &CHOICES);
    assert_eq!(
        configuration.selected_key("language"),
        Some(format!("{RBR}/public/language/polish").as_str())
    );
}

#[test]
fn unticking_the_picked_package_reopens_the_question() {
    let mut tree = tree_with_ticked(&["language/czech", "language/polish", "language/english"]);
    let mut configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);
    let polish_in_sorted_order = 2;
    configuration.select(0, Some(polish_in_sorted_order));
    let picked = configuration.selected_keys();
    assert_eq!(picked, vec![format!("{RBR}/public/language/polish")]);

    let leaf = leaf_of(&tree, &picked[0]);
    tree.set_checked(leaf, false);
    let configuration = super::Configuration::from_tree(&tree, &picked, &CHOICES);
    assert_eq!(configuration.lists[0].options.len(), 2);
    assert_eq!(configuration.lists[0].selected, None);
    assert!(!configuration.is_complete());
}

#[test]
fn an_unknown_category_is_installed_as_is() {
    let tree = tree_with_ticked(&["language/czech", "hud/Overlay"]);
    let configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);
    assert_eq!(configuration.lists.len(), 3);
    assert_eq!(
        configuration.extras,
        vec![format!("{RBR}/public/hud/Overlay")]
    );
}

#[test]
fn unticked_packages_are_not_offered() {
    let mut tree = tree_of(&[
        "language/czech",
        "language/polish",
        "style/Numeric Standard",
        "style/Descriptive",
    ]);
    tick(&mut tree, "language/czech");
    tick(&mut tree, "style/Descriptive");
    let configuration = super::Configuration::from_tree(&tree, &[], &CHOICES);

    assert_eq!(labels(&configuration, 0), vec!["czech".to_string()]);
    assert_eq!(labels(&configuration, 2), vec!["Descriptive".to_string()]);
    assert!(configuration.is_complete());
}

fn czech_and_installed_polish() -> crate::tree::Tree {
    let packages: Vec<_> = ["language/czech", "language/polish"]
        .iter()
        .map(|name| package(name))
        .collect();
    let installed = [format!("{RBR}/public/language/polish")];
    let mut tree = crate::tree::Tree::build(RBR, &packages, &[], &installed);
    let czech = leaf_of(&tree, &format!("{RBR}/public/language/czech"));
    tree.set_checked(czech, true);
    tree
}

/// The Configure/Install route changes an install, and an omitted package would be uninstalled.
#[test]
fn what_is_already_installed_is_preselected() {
    let configuration =
        super::Configuration::from_tree(&czech_and_installed_polish(), &[], &CHOICES);
    assert_eq!(
        configuration.lists[0].selected_key(),
        Some(format!("{RBR}/public/language/polish").as_str())
    );
}

/// They may have come back from the download scene precisely to change it.
#[test]
fn a_users_own_pick_beats_what_is_installed() {
    let czech = format!("{RBR}/public/language/czech");
    let configuration = super::Configuration::from_tree(
        &czech_and_installed_polish(),
        std::slice::from_ref(&czech),
        &CHOICES,
    );
    assert_eq!(configuration.lists[0].selected_key(), Some(czech.as_str()));
}
