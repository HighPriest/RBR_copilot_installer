fn write_package(data_dir: &std::path::Path, game: &str, rest: &str, bytes: &[u8]) {
    crate::store::fixture_tree::write(
        &crate::store::fixture_tree::package_path(data_dir, game, rest),
        bytes,
    );
}

fn find<'a>(
    generated: &'a crate::store::Generated,
    reference: &str,
) -> &'a pacenotes_shared::PackageInfo {
    generated
        .manifest
        .get(
            pacenotes_shared::GAME_RBR,
            &pacenotes_shared::PackageRef::parse(reference).unwrap(),
        )
        .unwrap()
}

#[test]
fn scans_subdirs_and_hashes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/basic.zip", b"hello");
    write_package(dir, "rbr", "premium-voices/pro.zip", b"world!");
    write_package(dir, "rbr", "public/notes.txt", b"ignored");

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    assert_eq!(generated.manifest.packages.len(), 2);

    let basic = find(&generated, "public/basic");
    assert_eq!(basic.size, 5);
    assert_eq!(
        basic.sha256,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert_eq!(basic.version, super::DEFAULT_VERSION);
    assert_eq!(basic.game, pacenotes_shared::GAME_RBR);
    assert_eq!(find(&generated, "premium-voices/pro").size, 6);
}

#[test]
fn sidecar_supplies_install_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/english.zip", b"lang");
    write_package(dir, "rbr", "public/eva.zip", b"voice");
    write_package(
        dir,
        "rbr",
        "public/eva.toml",
        br#"
            version = "2.1.0"
            display_name = "English / Female / Eva"
            description = "Calm delivery"
            exclusive_group = "voice"
            requires = ["public/english"]
        "#,
    );
    write_package(
        dir,
        "rbr",
        "public/english.toml",
        b"description = \"UI language\"\n",
    );

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    let eva = find(&generated, "public/eva");
    assert_eq!(eva.version, "2.1.0");
    assert_eq!(eva.game, pacenotes_shared::GAME_RBR);
    assert_eq!(eva.label(), "English / Female / Eva");
    assert_eq!(eva.description.as_deref(), Some("Calm delivery"));
    assert_eq!(eva.exclusive_group.as_deref(), Some("voice"));
    assert_eq!(eva.requires, vec!["public/english".to_string()]);
}

#[test]
fn sidecar_wins_over_previous_manifest_but_absent_fields_are_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/basic.zip", b"hello");
    let mut previous = crate::store::fixture_tree::generate(dir).manifest;
    let entry = &mut previous.packages[0];
    entry.version = "2.3.4".into();
    entry.sha256 = "stale".into();
    entry.display_name = Some("Numeric".into());
    entry.description = Some("Numeric pacenotes".into());
    entry.exclusive_group = Some("pacenote-config".into());
    crate::store::write_manifest(dir, &previous).unwrap();
    write_package(
        dir,
        "rbr",
        "public/basic.toml",
        b"display_name = \"Numeric v2\"\n",
    );

    let basic = crate::store::fixture_tree::generate(dir)
        .manifest
        .packages
        .remove(0);
    assert_eq!(basic.label(), "Numeric v2");
    assert_eq!(basic.version, "2.3.4");
    assert_eq!(basic.description.as_deref(), Some("Numeric pacenotes"));
    assert_eq!(basic.exclusive_group.as_deref(), Some("pacenote-config"));
    assert_eq!(basic.size, 5);
    assert_ne!(basic.sha256, "stale");
}

fn has_error(generated: &crate::store::Generated, fragment: &str) -> bool {
    generated
        .errors
        .iter()
        .any(|error| error.contains(fragment))
}

/// One typo must neither publish its package with default metadata nor take the rest down.
#[test]
fn a_broken_sidecar_withholds_its_package_and_nothing_else() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/basic.zip", b"hello");
    write_package(dir, "rbr", "public/basic.toml", b"categoy = \"sounds\"\n");
    write_package(dir, "rbr", "public/other.zip", b"fine");

    let generated = crate::store::fixture_tree::generate(dir);
    let names: Vec<&str> = generated
        .manifest
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(names, vec!["other"]);
    assert!(
        has_error(&generated, "public/basic.toml"),
        "{:?}",
        generated.errors
    );
}

#[test]
fn sidecar_keys_that_carried_positional_metadata_withhold_the_package() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/language/polish.zip", b"lang");
    write_package(
        dir,
        "rbr",
        "public/language/polish.toml",
        b"category = \"language\"\n",
    );

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.manifest.packages.is_empty());
    assert!(
        has_error(&generated, "polish.toml"),
        "{:?}",
        generated.errors
    );
}

#[test]
fn sidecar_without_zip_warns() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/basic.zip", b"hello");
    write_package(dir, "rbr", "public/ghost.toml", b"version = \"1.0.0\"\n");

    let generated = crate::store::fixture_tree::generate(dir);
    assert_eq!(generated.manifest.packages.len(), 1);
    assert!(
        generated
            .warnings
            .iter()
            .any(|warning| warning.contains("ghost.toml") && warning.contains("no ghost.zip")),
        "{:?}",
        generated.warnings
    );
}

#[test]
fn dangling_and_cross_boundary_requires_warn() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/voice.zip", b"voice");
    write_package(
        dir,
        "rbr",
        "public/voice.toml",
        b"requires = [\"public/missing\", \"not-a-ref\", \"premium/lang\"]\n",
    );
    write_package(dir, "rbr", "premium/lang.zip", b"lang");

    let warnings = crate::store::fixture_tree::generate(dir)
        .warnings
        .join("\n");
    assert!(warnings.contains("public/missing"), "{warnings}");
    assert!(warnings.contains("not-a-ref"), "{warnings}");
    assert!(warnings.contains("is public but requires"), "{warnings}");
}

/// The only `public/lang` is beamng's, so the rbr package's dependency is missing.
#[test]
fn requires_resolve_within_one_game_only() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/voice.zip", b"voice");
    write_package(
        dir,
        "rbr",
        "public/voice.toml",
        b"requires = [\"public/lang\"]\n",
    );
    write_package(dir, "beamng", "public/lang.zip", b"lang");

    let warnings = crate::store::fixture_tree::generate(dir)
        .warnings
        .join("\n");
    assert!(
        warnings.contains("does not exist in game rbr"),
        "{warnings}"
    );
}

/// `beamng` sorts first, so a game-blind lookup would find its package instead.
#[test]
fn a_same_named_package_in_another_game_does_not_shadow_the_real_one() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/voice.zip", b"voice");
    write_package(
        dir,
        "rbr",
        "public/voice.toml",
        b"requires = [\"public/lang\"]\n",
    );
    write_package(dir, "rbr", "public/lang.zip", b"lang");
    write_package(dir, "beamng", "public/lang.zip", b"lang");

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
}

#[test]
fn recurses_and_names_packages_by_their_path() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(
        dir,
        "rbr",
        "public/codriver/polish/male/Kamil.zip",
        b"kamil",
    );
    write_package(
        dir,
        "rbr",
        "public/codriver/polish/female/Magda.zip",
        b"magda",
    );
    write_package(dir, "rbr", "public/language/polish.zip", b"polish");
    write_package(dir, "rbr", "private/style/Descriptive.zip", b"descriptive");

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    let mut references: Vec<String> = generated
        .manifest
        .packages
        .iter()
        .map(|package| package.package_ref().to_string())
        .collect();
    references.sort();
    assert_eq!(
        references,
        vec![
            "private/style/Descriptive",
            "public/codriver/polish/female/Magda",
            "public/codriver/polish/male/Kamil",
            "public/language/polish",
        ]
    );

    let kamil = find(&generated, "public/codriver/polish/male/Kamil");
    assert_eq!(kamil.category(), pacenotes_shared::CATEGORY_CODRIVER);
    assert_eq!(kamil.tree_levels(), vec!["polish", "male"]);
    assert_eq!(kamil.leaf(), "Kamil");
}

#[test]
fn sidecars_are_read_next_to_a_nested_zip() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_package(dir, "rbr", "public/language/polish.zip", b"lang");
    write_package(
        dir,
        "rbr",
        "public/codriver/polish/male/Kamil.zip",
        b"kamil",
    );
    write_package(
        dir,
        "rbr",
        "public/codriver/polish/male/Kamil.toml",
        br#"
            version = "3.0.0"
            display_name = "Kamil Filipek"
            requires = ["public/language/polish"]
        "#,
    );

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    let kamil = find(&generated, "public/codriver/polish/male/Kamil");
    assert_eq!(kamil.version, "3.0.0");
    assert_eq!(kamil.label(), "Kamil Filipek");
    assert_eq!(kamil.tree_levels(), vec!["polish", "male"]);
    assert_eq!(kamil.required_refs().len(), 1);
}

#[test]
fn motd_directory_is_not_a_game() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    crate::store::fixture_tree::write(&dir.join("motd/en.txt"), b"news");
    write_package(dir, "rbr", "public/language/polish.zip", b"lang");

    let generated = crate::store::fixture_tree::generate(dir);
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
    assert_eq!(generated.manifest.packages.len(), 1);
}

#[test]
fn names_needing_url_encoding_warn_but_publish() {
    let tmp = tempfile::tempdir().unwrap();
    write_package(
        tmp.path(),
        "rbr",
        "public/style/Numeric Standard.zip",
        b"style",
    );

    let generated = crate::store::fixture_tree::generate(tmp.path());
    assert_eq!(
        generated.manifest.packages[0].name,
        "style/Numeric Standard"
    );
    assert!(
        generated
            .warnings
            .iter()
            .any(|warning| warning.contains("percent-encode")),
        "{:?}",
        generated.warnings
    );
}

#[test]
fn a_top_level_zip_is_withheld_as_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    crate::store::fixture_tree::write(&tmp.path().join("loose.zip"), b"x");
    let generated = crate::store::fixture_tree::generate(tmp.path());
    assert!(generated.manifest.packages.is_empty());
    assert!(
        has_error(&generated, "top-level zip"),
        "{:?}",
        generated.errors
    );
}

#[test]
fn skips_invalid_directory_names() {
    let tmp = tempfile::tempdir().unwrap();
    write_package(tmp.path(), "rbr", "Bad Dir/x.zip", b"x");
    write_package(tmp.path(), "rbr", "public/ok.zip", b"x");

    let generated = crate::store::fixture_tree::generate(tmp.path());
    assert_eq!(generated.manifest.packages.len(), 1);
    assert_eq!(generated.manifest.packages[0].directory, "public");
    assert!(generated
        .warnings
        .iter()
        .any(|warning| warning.contains("Bad Dir")));
}
