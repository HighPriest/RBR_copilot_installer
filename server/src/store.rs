//! Package store: manifest.json (source of truth for metadata) + zips on disk
//! under per-access-directory subdirs: <packages>/<directory>/<name>.zip.
//! Task S3/S4. The DB never knows about packages; grants reference directory
//! names only.
//!
//! CONVENTION (root CLAUDE.md): no `use` statements — types and functions are
//! written with their full paths at the point of use.

/// Directory names are `[a-z0-9_-]+`. Enforced by `manifest gen` and
/// re-checked on load, so a name can never carry a path separator, `..`,
/// a drive letter or a leading `/`.
pub fn valid_directory(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Package names are file stems, so they allow mixed case and `.` on top of
/// the directory alphabet — but never a separator or `..`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
        && !name.contains("..")
}

pub struct ManifestStore {
    packages_dir: std::path::PathBuf,
    cache: std::sync::RwLock<Cached>,
}

struct Cached {
    manifest: pacenotes_shared::Manifest,
    mtime: Option<std::time::SystemTime>,
    /// Distinguishes "never loaded" from "loaded an empty manifest".
    loaded: bool,
}

impl ManifestStore {
    pub fn new(packages_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            packages_dir: packages_dir.into(),
            cache: std::sync::RwLock::new(Cached {
                manifest: pacenotes_shared::Manifest::default(),
                mtime: None,
                loaded: false,
            }),
        }
    }

    pub fn manifest_path(&self) -> std::path::PathBuf {
        self.packages_dir.join("manifest.json")
    }

    /// Lazy reload: compare manifest.json mtime; reload under write lock if
    /// changed. Called at the top of each handler. A missing manifest.json is
    /// not an error — it means "nothing published yet".
    pub fn current(&self) -> pacenotes_shared::Manifest {
        let disk_mtime = std::fs::metadata(self.manifest_path())
            .and_then(|m| m.modified())
            .ok();

        {
            let cached = self.cache.read().unwrap_or_else(|e| e.into_inner());
            if cached.loaded && cached.mtime == disk_mtime {
                return cached.manifest.clone();
            }
        }

        let manifest = match read_manifest(&self.manifest_path()) {
            Ok(m) => m,
            Err(e) => {
                // Never fail a request over a bad manifest: log and serve the
                // last good copy (empty on first load).
                tracing::error!(
                    error = %e,
                    path = %self.manifest_path().display(),
                    "manifest load failed"
                );
                return self
                    .cache
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .manifest
                    .clone();
            }
        };

        let mut cached = self.cache.write().unwrap_or_else(|e| e.into_inner());
        cached.manifest = manifest.clone();
        cached.mtime = disk_mtime;
        cached.loaded = true;
        manifest
    }

    /// SECURITY: `directory` and `name` are lookup keys into the manifest,
    /// never path fragments from the URL. Returns the on-disk zip path only
    /// for known (directory, name) pairs — this is the structural
    /// path-traversal defense (CLAUDE.md). The manifest generator (below) is
    /// the only place directory/name strings are created, and it validates
    /// them against [a-z0-9_-]+ / no separators.
    pub fn zip_path(
        &self,
        directory: &str,
        name: &str,
    ) -> Option<(pacenotes_shared::PackageInfo, std::path::PathBuf)> {
        let pkg = self
            .current()
            .packages
            .into_iter()
            .find(|p| p.directory == directory && p.name == name)?;
        // Defense in depth: an entry with an invalid key never yields a path
        // (read_manifest already rejects such manifests wholesale).
        if !valid_directory(&pkg.directory) || !valid_name(&pkg.name) {
            return None;
        }
        let path = self
            .packages_dir
            .join(&pkg.directory)
            .join(format!("{}.zip", pkg.name));
        Some((pkg, path))
    }

    /// `manifest gen` (task S3): scan <dir>/<subdir>/*.zip — subdir name =
    /// access directory (validated `[a-z0-9_-]+`; skipped with a warning
    /// otherwise). Computes size/sha256/updated_at, merges each zip's
    /// `<name>.toml` sidecar (install metadata, protocol v2), and returns the
    /// manifest plus the warnings the publisher should see.
    /// Top-level zips (no subdir) are an error: every package must live in a
    /// directory; `public/` is where anonymous packages go.
    pub fn generate(dir: &std::path::Path) -> anyhow::Result<Generated> {
        let previous = read_manifest(&dir.join("manifest.json")).unwrap_or_default();
        let mut generated = Generated {
            manifest: pacenotes_shared::Manifest::default(),
            warnings: Vec::new(),
        };

        for entry in read_dir_sorted(dir)? {
            let path = entry.path();
            let dir_name = entry.file_name().to_string_lossy().into_owned();

            if path.is_file() {
                if path.extension().is_some_and(|e| e == "zip") {
                    anyhow::bail!(
                        "top-level zip {}: every package must live in a directory \
                         (put anonymous packages in {}/)",
                        path.display(),
                        pacenotes_shared::PUBLIC_DIR
                    );
                }
                continue;
            }
            if !path.is_dir() {
                continue;
            }
            if !valid_directory(&dir_name) {
                generated.warn(format!(
                    "skipping directory {dir_name:?}: not a valid directory name [a-z0-9_-]+"
                ));
                continue;
            }

            // Sidecars are matched against the zips found in the same pass, so
            // a sidecar whose zip is missing can be reported afterwards.
            let mut sidecars_seen: Vec<String> = Vec::new();
            let mut names_seen: Vec<String> = Vec::new();

            for file in read_dir_sorted(&path)? {
                let file_path = file.path();
                if !file_path.is_file() {
                    continue;
                }
                let stem = file_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                match file_path.extension().and_then(|e| e.to_str()) {
                    Some("toml") => {
                        sidecars_seen.push(stem);
                        continue;
                    }
                    Some("zip") => {}
                    _ => continue,
                }
                let name = stem;
                if !valid_name(&name) {
                    generated.warn(format!(
                        "skipping {dir_name}/{name}.zip: invalid package name"
                    ));
                    continue;
                }
                names_seen.push(name.clone());

                let meta = std::fs::metadata(&file_path)
                    .map_err(|e| anyhow::anyhow!("stat {}: {e}", file_path.display()))?;
                let updated_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let previous_entry = previous
                    .packages
                    .iter()
                    .find(|p| p.directory == dir_name && p.name == name);
                let sidecar = match read_sidecar(&file_path.with_extension("toml")) {
                    Ok(s) => s,
                    Err(e) => {
                        // A broken sidecar must not silently publish a package
                        // with default metadata.
                        anyhow::bail!("{dir_name}/{name}.toml: {e}");
                    }
                };

                let mut package = pacenotes_shared::PackageInfo {
                    directory: dir_name.clone(),
                    name,
                    // Version precedence: sidecar wins, else the version the
                    // previous manifest carried, else "0.0.0".
                    version: sidecar
                        .version
                        .clone()
                        .or_else(|| previous_entry.map(|p| p.version.clone()))
                        .unwrap_or_else(|| "0.0.0".to_string()),
                    size: meta.len(),
                    sha256: sha256_file(&file_path)?,
                    updated_at,
                    game: sidecar
                        .game
                        .clone()
                        .or_else(|| previous_entry.map(|p| p.game.clone()))
                        .unwrap_or_else(pacenotes_shared::default_game),
                    display_name: sidecar
                        .display_name
                        .clone()
                        .or_else(|| previous_entry.and_then(|p| p.display_name.clone())),
                    description: sidecar
                        .description
                        .clone()
                        .or_else(|| previous_entry.and_then(|p| p.description.clone())),
                    exclusive_group: sidecar
                        .exclusive_group
                        .clone()
                        .or_else(|| previous_entry.and_then(|p| p.exclusive_group.clone())),
                    requires: sidecar
                        .requires
                        .clone()
                        .or_else(|| previous_entry.map(|p| p.requires.clone()))
                        .unwrap_or_default(),
                };
                package.requires.sort();
                package.requires.dedup();

                generated.manifest.packages.push(package);
            }

            for sidecar in sidecars_seen {
                if !names_seen.contains(&sidecar) {
                    generated.warn(format!(
                        "{dir_name}/{sidecar}.toml has no {sidecar}.zip next to it \
                         — the metadata is ignored"
                    ));
                }
            }
        }

        generated.validate_requires();
        Ok(generated)
    }
}

/// Result of `manifest gen`: the manifest plus everything the publisher
/// should look at. Warnings are collected rather than logged directly so the
/// CLI can print them together and tests can assert on them.
#[derive(Debug)]
pub struct Generated {
    pub manifest: pacenotes_shared::Manifest,
    pub warnings: Vec<String>,
}

impl Generated {
    fn warn(&mut self, message: String) {
        tracing::warn!("{message}");
        self.warnings.push(message);
    }

    /// `requires` checks that can only run once every package is known:
    /// a dep that does not exist, and a public package depending on a
    /// protected one (anonymous users could list it but never resolve the
    /// dependency, so the UI would block on a package they cannot see).
    fn validate_requires(&mut self) {
        let mut problems: Vec<String> = Vec::new();
        for package in &self.manifest.packages {
            for raw in &package.requires {
                let Some(reference) = pacenotes_shared::PackageRef::parse(raw) else {
                    problems.push(format!(
                        "{}/{} requires {raw:?}, which is not a \"directory/name\" reference",
                        package.directory, package.name
                    ));
                    continue;
                };
                let Some(dependency) = self.manifest.get(&reference) else {
                    problems.push(format!(
                        "{}/{} requires {reference}, which does not exist",
                        package.directory, package.name
                    ));
                    continue;
                };
                if package.is_public() && !dependency.is_public() {
                    problems.push(format!(
                        "{}/{} is public but requires {reference}, which is not — \
                         anonymous users would see a package they cannot complete",
                        package.directory, package.name
                    ));
                }
                if dependency.game != package.game {
                    problems.push(format!(
                        "{}/{} (game {}) requires {reference} (game {})",
                        package.directory, package.name, package.game, dependency.game
                    ));
                }
            }
        }
        for problem in problems {
            self.warn(problem);
        }
    }
}

/// Publisher-authored metadata next to a package zip: `<name>.toml`.
/// Every field is optional — an absent field keeps whatever the previous
/// manifest had, and failing that the protocol default.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    version: Option<String>,
    game: Option<String>,

    display_name: Option<String>,
    description: Option<String>,
    exclusive_group: Option<String>,
    requires: Option<Vec<String>>,
}

/// Reads `<name>.toml`. A missing sidecar is the normal case, not an error.
fn read_sidecar(path: &std::path::Path) -> anyhow::Result<Sidecar> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Sidecar::default()),
        Err(e) => anyhow::bail!("reading {}: {e}", path.display()),
    };
    toml::from_str(&text).map_err(|e| anyhow::anyhow!("{e}"))
}

/// Directory listing in a stable (name-sorted) order, so generated manifests
/// don't churn between runs.
fn read_dir_sorted(dir: &std::path::Path) -> anyhow::Result<Vec<std::fs::DirEntry>> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", dir.display()))?;
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

/// Read + validate manifest.json. Missing file = empty manifest.
fn read_manifest(path: &std::path::Path) -> anyhow::Result<pacenotes_shared::Manifest> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(pacenotes_shared::Manifest::default())
        }
        Err(e) => anyhow::bail!("reading {}: {e}", path.display()),
    };
    let manifest: pacenotes_shared::Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;

    for p in &manifest.packages {
        if !valid_directory(&p.directory) {
            anyhow::bail!("invalid directory name {:?} in manifest", p.directory);
        }
        if !valid_name(&p.name) {
            anyhow::bail!("invalid package name {:?} in manifest", p.name);
        }
    }
    Ok(manifest)
}

pub fn sha256_file(path: &std::path::Path) -> anyhow::Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("open {}: {e}", path.display()))?;
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| anyhow::anyhow!("hash {}: {e}", path.display()))?;
    Ok(sha2::Digest::finalize(hasher)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    fn write(path: &std::path::Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn write_manifest(dir: &std::path::Path, manifest: &pacenotes_shared::Manifest) {
        write(
            &dir.join("manifest.json"),
            serde_json::to_string(manifest).unwrap().as_bytes(),
        );
    }

    fn generate(dir: &std::path::Path) -> super::Generated {
        super::ManifestStore::generate(dir).unwrap()
    }

    #[test]
    fn name_validation_rejects_traversal() {
        assert!(super::valid_directory("public"));
        assert!(super::valid_directory("premium-voices_2"));
        assert!(!super::valid_directory(""));
        assert!(!super::valid_directory(".."));
        assert!(!super::valid_directory("a/b"));
        assert!(!super::valid_directory("a\\b"));
        assert!(!super::valid_directory("Premium"));
        assert!(super::valid_name("FilipekMod_v1.2"));
        assert!(!super::valid_name("../secret"));
        assert!(!super::valid_name("a..b"));
    }

    #[test]
    fn generate_scans_subdirs_and_hashes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        write(&dir.join("premium-voices/pro.zip"), b"world!");
        write(&dir.join("public/notes.txt"), b"ignored");

        let generated = generate(dir);
        assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
        assert_eq!(generated.manifest.packages.len(), 2);

        let basic = generated
            .manifest
            .packages
            .iter()
            .find(|p| p.name == "basic")
            .unwrap();
        assert_eq!(basic.directory, "public");
        assert_eq!(basic.size, 5);
        assert_eq!(
            basic.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(basic.version, "0.0.0");
        // No sidecar, no previous manifest: protocol defaults.
        assert_eq!(basic.game, pacenotes_shared::GAME_RBR);
        assert_eq!(basic.category(), "basic"); // flat name: its own category
        assert!(generated
            .manifest
            .packages
            .iter()
            .any(|p| p.directory == "premium-voices"));
    }

    #[test]
    fn sidecar_supplies_install_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/english.zip"), b"lang");
        write(&dir.join("public/eva.zip"), b"voice");
        write(
            &dir.join("public/eva.toml"),
            br#"
                version = "2.1.0"
                game = "dirtrally2"
                display_name = "English / Female / Eva"
                description = "Calm delivery"
                exclusive_group = "voice"
                requires = ["public/english"]
            "#,
        );
        write(
            &dir.join("public/english.toml"),
            b"game = \"dirtrally2\"\n",
        );

        let generated = generate(dir);
        assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
        let eva = generated
            .manifest
            .get(&pacenotes_shared::PackageRef::parse("public/eva").unwrap())
            .unwrap();
        assert_eq!(eva.version, "2.1.0");
        assert_eq!(eva.game, pacenotes_shared::GAME_DIRT_RALLY2);
        assert_eq!(eva.category(), "eva"); // flat name: its own category
        assert_eq!(eva.label(), "English / Female / Eva");
        assert_eq!(eva.description.as_deref(), Some("Calm delivery"));
        assert_eq!(eva.exclusive_group.as_deref(), Some("voice"));
        assert_eq!(eva.requires, vec!["public/english".to_string()]);
    }

    #[test]
    fn sidecar_wins_over_previous_manifest_but_absent_fields_are_preserved() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        write_manifest(
            dir,
            &pacenotes_shared::Manifest {
                packages: vec![pacenotes_shared::PackageInfo {
                    directory: "public".into(),
                    name: "basic".into(),
                    version: "2.3.4".into(),
                    size: 0,
                    sha256: "stale".into(),
                    updated_at: 1,
                    game: pacenotes_shared::GAME_BEAMNG.into(),
                    display_name: Some("Numeric".into()),
                    description: Some("Numeric pacenotes".into()),
                    exclusive_group: Some("pacenote-config".into()),
                    requires: Vec::new(),
                }],
            },
        );
        write(
            &dir.join("public/basic.toml"),
            b"display_name = \"Numeric v2\"\n",
        );

        let basic = generate(dir).manifest.packages.remove(0);
        // Sidecar field wins...
        assert_eq!(basic.label(), "Numeric v2");
        // ...everything it omits keeps what the manifest already published...
        assert_eq!(basic.version, "2.3.4");
        assert_eq!(basic.game, pacenotes_shared::GAME_BEAMNG);
        assert_eq!(basic.category(), "basic");
        assert_eq!(basic.description.as_deref(), Some("Numeric pacenotes"));
        assert_eq!(basic.exclusive_group.as_deref(), Some("pacenote-config"));
        // ...while size/sha/updated_at always come from the file on disk.
        assert_eq!(basic.size, 5);
        assert_ne!(basic.sha256, "stale");
    }

    #[test]
    fn broken_sidecar_fails_the_run() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        write(&dir.join("public/basic.toml"), b"categoy = \"sounds\"\n");

        // A typo must not publish a package with silently-default metadata.
        let err = super::ManifestStore::generate(dir).unwrap_err().to_string();
        assert!(err.contains("public/basic.toml"), "{err}");
    }

    #[test]
    fn sidecar_without_zip_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        write(&dir.join("public/ghost.toml"), b"game = \"rbr\"\n");

        let generated = generate(dir);
        assert_eq!(generated.manifest.packages.len(), 1);
        assert!(
            generated
                .warnings
                .iter()
                .any(|w| w.contains("ghost.toml") && w.contains("no ghost.zip")),
            "{:?}",
            generated.warnings
        );
    }

    #[test]
    fn dangling_and_cross_boundary_requires_warn() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/voice.zip"), b"voice");
        write(
            &dir.join("public/voice.toml"),
            b"requires = [\"public/missing\", \"not-a-ref\", \"premium/lang\"]\n",
        );
        write(&dir.join("premium/lang.zip"), b"lang");

        let generated = generate(dir);
        let joined = generated.warnings.join("\n");
        assert!(joined.contains("public/missing"), "{joined}");
        assert!(joined.contains("not-a-ref"), "{joined}");
        // public -> premium: anonymous users can see the voice but never the
        // language pack it needs.
        assert!(joined.contains("is public but requires"), "{joined}");
    }

    #[test]
    fn requires_across_games_warn() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/voice.zip"), b"voice");
        write(
            &dir.join("public/voice.toml"),
            b"game = \"rbr\"\nrequires = [\"public/lang\"]\n",
        );
        write(&dir.join("public/lang.zip"), b"lang");
        write(&dir.join("public/lang.toml"), b"game = \"beamng\"\n");

        let joined = generate(dir).warnings.join("\n");
        assert!(joined.contains("game rbr"), "{joined}");
        assert!(joined.contains("game beamng"), "{joined}");
    }

    #[test]
    fn generate_rejects_top_level_zip() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("loose.zip"), b"x");
        let err = super::ManifestStore::generate(tmp.path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("top-level zip"), "{err}");
    }

    #[test]
    fn generate_skips_invalid_directory_names() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("Bad Dir/x.zip"), b"x");
        write(&tmp.path().join("public/ok.zip"), b"x");
        let generated = generate(tmp.path());
        assert_eq!(generated.manifest.packages.len(), 1);
        assert_eq!(generated.manifest.packages[0].directory, "public");
        assert!(generated.warnings.iter().any(|w| w.contains("Bad Dir")));
    }

    #[test]
    fn store_reloads_on_mtime_change_and_resolves_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        let store = super::ManifestStore::new(dir);
        assert!(store.current().packages.is_empty()); // no manifest yet

        write_manifest(dir, &generate(dir).manifest);

        assert_eq!(store.current().packages.len(), 1);
        let (pkg, path) = store.zip_path("public", "basic").unwrap();
        assert_eq!(pkg.name, "basic");
        assert_eq!(path, dir.join("public").join("basic.zip"));
        assert!(store.zip_path("public", "nope").is_none());
        assert!(store.zip_path("../data", "auth").is_none());
    }

    #[test]
    fn bad_manifest_json_serves_last_good_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/basic.zip"), b"hello");
        write_manifest(dir, &generate(dir).manifest);
        let store = super::ManifestStore::new(dir);
        assert_eq!(store.current().packages.len(), 1);

        write(&dir.join("manifest.json"), b"{ not json");
        assert_eq!(store.current().packages.len(), 1);
    }
}
