fn basic_zip(dir: &std::path::Path) -> std::path::PathBuf {
    crate::store::fixture_tree::package_path(dir, "rbr", "public/basic.zip")
}

#[test]
fn migration_moves_manifest_only_values_into_sidecars_once() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    crate::store::fixture_tree::write_package(dir, "rbr", "public/basic.zip", b"hello");
    crate::store::fixture_tree::write(
        &basic_zip(dir).with_extension("toml"),
        b"# hand-written\ndisplay_name = \"Kept\"",
    );
    crate::store::write_manifest(dir, &crate::store::fixture_tree::legacy_manifest()).unwrap();

    let migration = super::migrate_to_sidecars(dir).unwrap();
    assert!(migration.errors.is_empty(), "{:?}", migration.errors);
    let written = migration.written.join("\n");
    assert!(
        !written.contains("display_name"),
        "never overwrite: {written}"
    );
    for key in ["version", "description", "exclusive_group", "requires"] {
        assert!(written.contains(key), "{key} not migrated: {written}");
    }

    let text = std::fs::read_to_string(basic_zip(dir).with_extension("toml")).unwrap();
    assert!(text.starts_with("# hand-written\n"), "{text}");
    let regenerated = crate::store::fixture_tree::generate(dir);
    assert!(regenerated.errors.is_empty(), "{:?}", regenerated.errors);
    let basic = &regenerated.manifest.packages[0];
    assert_eq!(basic.version, "2.3.4");
    assert_eq!(basic.label(), "Kept");
    assert_eq!(basic.requires, vec!["public/language/polish".to_string()]);
    assert!(crate::store::metadata_losses(
        &crate::store::fixture_tree::legacy_manifest(),
        &regenerated.manifest
    )
    .is_empty());

    let again = super::migrate_to_sidecars(dir).unwrap();
    assert!(again.written.is_empty(), "{:?}", again.written);
}

/// Pinning either would freeze a version nobody chose.
#[test]
fn migration_never_pins_a_default_or_derived_version() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    crate::store::fixture_tree::write_package(dir, "rbr", "public/basic.zip", b"hello");
    for version in ["0.0.0", "sha-2cf24dba"] {
        let mut manifest = crate::store::fixture_tree::legacy_manifest();
        manifest.packages[0].version = version.into();
        crate::store::write_manifest(dir, &manifest).unwrap();
        let _ = std::fs::remove_file(basic_zip(dir).with_extension("toml"));

        let migration = super::migrate_to_sidecars(dir).unwrap();
        assert!(
            !migration
                .written
                .iter()
                .any(|line| line.ends_with("version")),
            "{version} was pinned: {:?}",
            migration.written
        );
    }
}
