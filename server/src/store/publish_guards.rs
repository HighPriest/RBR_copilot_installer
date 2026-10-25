//! What publishing a new manifest would take away from the live one without
//! anyone deciding it, which `manifest gen` refuses unless told otherwise.

/// Games the current manifest serves that `next` would take off the server
/// entirely because their schema is missing or broken, typically after an
/// upgrade that did not copy `schema.toml` in; a game whose packages all left
/// on purpose keeps its schema and is not listed.
pub fn unpublished_games(
    current: &pacenotes_shared::Manifest,
    next: &pacenotes_shared::Manifest,
) -> Vec<String> {
    let mut games: Vec<String> = current
        .packages
        .iter()
        .map(|package| package.game.clone())
        .filter(|game| !next.schemas.contains_key(game))
        .filter(|game| !next.packages.iter().any(|package| &package.game == game))
        .collect();
    games.sort();
    games.dedup();
    games
}

/// Values the current manifest carries that publishing `next` would drop, one
/// line per package and field.
///
/// `manifest gen` once carried every value forward, so a catalogue can hold
/// metadata no sidecar has. A value a sidecar changed is not a loss, nor is a
/// package whose zip is gone, nor a version nobody chose.
pub fn metadata_losses(
    current: &pacenotes_shared::Manifest,
    next: &pacenotes_shared::Manifest,
) -> Vec<String> {
    let mut losses = Vec::new();
    for old in &current.packages {
        let Some(new) = next.packages.iter().find(|package| {
            package.game == old.game
                && package.directory == old.directory
                && package.name == old.name
        }) else {
            continue;
        };
        let id = old.download_path();
        if crate::store::is_chosen_version(&old.version)
            && new
                .version
                .starts_with(crate::store::DERIVED_VERSION_PREFIX)
        {
            losses.push(format!("{id}: version {:?}", old.version));
        }
        let optional_fields = [
            ("display_name", &old.display_name, &new.display_name),
            ("description", &old.description, &new.description),
            (
                "exclusive_group",
                &old.exclusive_group,
                &new.exclusive_group,
            ),
        ];
        for (field, before, after) in optional_fields {
            if let (Some(value), None) = (before, after) {
                losses.push(format!("{id}: {field} {value:?}"));
            }
        }
        if !old.requires.is_empty() && new.requires.is_empty() {
            losses.push(format!("{id}: requires {:?}", old.requires));
        }
    }
    losses
}

#[cfg(test)]
mod tests {
    /// What a server upgraded without copying `schema.toml` in would publish.
    #[test]
    fn a_live_game_losing_its_schema_is_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        crate::store::fixture_tree::write(
            &crate::store::fixture_tree::package_path(dir, "rbr", "public/language/polish.zip"),
            b"x",
        );
        let next = crate::store::fixture_tree::generate(dir).manifest;
        assert!(next.packages.is_empty());
        let live = crate::store::fixture_tree::legacy_manifest();
        assert_eq!(super::unpublished_games(&live, &next), vec!["rbr"]);
    }

    /// A game whose packages all left on purpose still has its schema.
    #[test]
    fn a_game_emptied_on_purpose_is_not_unpublished() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        crate::store::fixture_tree::declare(dir, "rbr", &["language"]);
        std::fs::create_dir_all(dir.join("rbr").join(crate::store::PACKAGES_DIR)).unwrap();
        let next = crate::store::fixture_tree::generate(dir).manifest;
        let live = crate::store::fixture_tree::legacy_manifest();
        assert!(super::unpublished_games(&live, &next).is_empty());
    }

    fn basic_on_disk(dir: &std::path::Path, sidecar: Option<&[u8]>) -> pacenotes_shared::Manifest {
        let zip = crate::store::fixture_tree::package_path(dir, "rbr", "public/basic.zip");
        crate::store::fixture_tree::write_package(dir, "rbr", "public/basic.zip", b"hello");
        let _ = std::fs::remove_file(zip.with_extension("toml"));
        if let Some(sidecar) = sidecar {
            crate::store::fixture_tree::write(&zip.with_extension("toml"), sidecar);
        }
        crate::store::fixture_tree::generate(dir).manifest
    }

    #[test]
    fn dropping_manifest_only_metadata_is_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let next = basic_on_disk(tmp.path(), None);

        let losses = super::metadata_losses(&crate::store::fixture_tree::legacy_manifest(), &next)
            .join("\n");
        for field in [
            "version",
            "display_name",
            "description",
            "exclusive_group",
            "requires",
        ] {
            assert!(losses.contains(field), "{field} missing from {losses}");
        }
        assert!(losses.contains("rbr/public/basic"), "{losses}");
    }

    /// The operator speaking, not a loss.
    #[test]
    fn a_value_a_sidecar_changed_is_not_a_loss() {
        let tmp = tempfile::tempdir().unwrap();
        let next = basic_on_disk(
            tmp.path(),
            Some(
                br#"
                    version = "3.0.0"
                    display_name = "Other"
                    description = "Other"
                    exclusive_group = "other"
                    requires = ["public/language/english"]
                "#,
            ),
        );
        let current = crate::store::fixture_tree::legacy_manifest();
        assert!(super::metadata_losses(&current, &next).is_empty());
    }

    #[test]
    fn the_legacy_default_version_is_not_a_loss() {
        let tmp = tempfile::tempdir().unwrap();
        let next = basic_on_disk(tmp.path(), None);
        let mut defaulted = crate::store::fixture_tree::legacy_manifest();
        let package = &mut defaulted.packages[0];
        package.version = "0.0.0".into();
        package.display_name = None;
        package.description = None;
        package.exclusive_group = None;
        package.requires.clear();
        assert!(super::metadata_losses(&defaulted, &next).is_empty());
    }

    /// Deleting a zip unpublishes the package, which is its own deliberate act.
    #[test]
    fn a_package_whose_zip_is_gone_is_not_a_loss() {
        let current = crate::store::fixture_tree::legacy_manifest();
        assert!(super::metadata_losses(&current, &Default::default()).is_empty());
    }
}
