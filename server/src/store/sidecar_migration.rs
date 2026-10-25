/// What `migrate_to_sidecars` did.
#[derive(Debug, Default)]
pub struct Migration {
    /// One line per key written, e.g. `rbr/public/style/Descriptive: version`.
    pub written: Vec<String>,
    /// Packages whose sidecar is unreadable or whose zip is gone.
    pub errors: Vec<String>,
}

/// `manifest migrate-to-sidecars`: appends each value the current manifest
/// carries and its package's sidecar lacks, so a pure regeneration keeps it.
///
/// Never overwrites a key, never pins the legacy or a derived version, and a
/// second run writes nothing; appending keeps a sidecar's comments, and a
/// top-level key is valid at the end of a flat TOML file.
pub fn migrate_to_sidecars(data_dir: &std::path::Path) -> anyhow::Result<Migration> {
    let current = crate::store::read_manifest(&data_dir.join(crate::store::MANIFEST_FILE))?;
    let mut migration = Migration::default();
    for package in &current.packages {
        let id = package.download_path();
        let Some(zip) = crate::store::zip_file(data_dir, package).filter(|zip| zip.is_file())
        else {
            migration.errors.push(format!(
                "{id}: its zip is gone — nothing to attach the values to"
            ));
            continue;
        };
        let sidecar_path = zip.with_extension("toml");
        let text = match std::fs::read_to_string(&sidecar_path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                migration
                    .errors
                    .push(format!("{id}: reading {}: {e}", sidecar_path.display()));
                continue;
            }
        };
        let existing = match crate::store::sidecar::parse_sidecar(&text) {
            Ok(existing) => existing,
            Err(e) => {
                migration.errors.push(format!(
                    "{id}: {} does not parse ({e}) — fix it first",
                    sidecar_path.display()
                ));
                continue;
            }
        };

        let missing = missing_keys(&existing, package);
        if missing.is_empty() {
            continue;
        }
        migration
            .written
            .extend(missing.iter().map(|(key, _)| format!("{id}: {key}")));
        std::fs::write(&sidecar_path, with_keys_appended(text, &missing))
            .map_err(|e| anyhow::anyhow!("writing {}: {e}", sidecar_path.display()))?;
    }
    Ok(migration)
}

/// The package's values the sidecar does not set, as TOML.
fn missing_keys(
    existing: &crate::store::sidecar::Sidecar,
    package: &pacenotes_shared::PackageInfo,
) -> Vec<(&'static str, toml::Value)> {
    let text = |value: &str| toml::Value::String(value.to_string());
    let mut missing = Vec::new();
    if existing.version.is_none() && crate::store::is_chosen_version(&package.version) {
        missing.push(("version", text(&package.version)));
    }
    let optional_fields = [
        (
            "display_name",
            &existing.display_name,
            &package.display_name,
        ),
        ("description", &existing.description, &package.description),
        (
            "exclusive_group",
            &existing.exclusive_group,
            &package.exclusive_group,
        ),
    ];
    for (key, in_sidecar, in_manifest) in optional_fields {
        if let (None, Some(value)) = (in_sidecar, in_manifest) {
            missing.push((key, text(value)));
        }
    }
    if existing.requires.is_none() && !package.requires.is_empty() {
        let references = package.requires.iter().map(|reference| text(reference));
        missing.push(("requires", toml::Value::Array(references.collect())));
    }
    missing
}

fn with_keys_appended(mut text: String, keys: &[(&'static str, toml::Value)]) -> String {
    if text.is_empty() {
        text.push_str("# Moved here from manifest.json by `manifest migrate-to-sidecars`.\n");
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    for (key, value) in keys {
        text.push_str(&format!("{key} = {value}\n"));
    }
    text
}

#[cfg(test)]
mod tests;
