//! Package store: manifest.json (source of truth for metadata) + zips on disk
//! under per-access-directory subtrees: <packages>/<directory>/<name>.zip,
//! where `name` is the zip's path under its access directory WITHOUT the
//! extension, and may be nested any number of levels deep:
//!
//! ```text
//! packages/public/codriver/polish/male/Kamil.zip
//!          ^dir   ^------------ name -----^
//! ```
//!
//! That path is the whole presentation (root CLAUDE.md "Install model"):
//! `shared` derives category/tree levels/leaf from it, so the operator's
//! folder layout IS the installer's Download tree and sidecars carry no
//! positional metadata.
//!
//! The DB never knows about packages; grants reference directory names only.
//!
//! CONVENTION (root CLAUDE.md): no `use` statements — types and functions are
//! written with their full paths at the point of use.

/// Subdirectory of the packages root holding message-of-the-day texts
/// (`<packages>/motd/<lang>.txt`). It sits alongside the access directories
/// but is NOT one: it holds no packages and `manifest gen` skips it.
pub const MOTD_DIR: &str = "motd";

/// Directory names are `[a-z0-9_-]+`. Enforced by `manifest gen` and
/// re-checked on load, so a name can never carry a path separator, `..`,
/// a drive letter or a leading `/`.
pub fn valid_directory(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
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
    /// path-traversal defense (CLAUDE.md), and it is what makes a nested
    /// `name` safe: the URL cannot name a file the manifest does not already
    /// list. The manifest generator (below) is the only place directory/name
    /// strings are created; it validates the directory against
    /// `[a-z0-9_-]+` and the name against `shared::valid_name`, which bans
    /// `\\`, `.`/`..` segments and empty segments.
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
        if !valid_directory(&pkg.directory) || !pacenotes_shared::valid_name(&pkg.name) {
            return None;
        }
        let path = self
            .packages_dir
            .join(&pkg.directory)
            .join(format!("{}.zip", pkg.name));
        Some((pkg, path))
    }

    /// Message of the day for `lang`, falling back to English, or `None`
    /// when neither exists (the installer treats that exactly like being
    /// offline).
    ///
    /// SECURITY: `lang` arrives in a query string, so unlike `name` it is not
    /// a manifest key and cannot be validated by lookup. It is therefore
    /// restricted to `[a-z]{2,8}` BEFORE it touches the filesystem — no
    /// separator, no dot, no `..` can survive that.
    pub fn motd(&self, lang: &str) -> Option<String> {
        let mut tried: Vec<&str> = Vec::new();
        if valid_lang(lang) {
            tried.push(lang);
        } else if !lang.is_empty() {
            tracing::debug!("ignoring motd lang request: not [a-z]{{2,8}}");
        }
        if !tried.contains(&MOTD_FALLBACK_LANG) {
            tried.push(MOTD_FALLBACK_LANG);
        }

        for candidate in tried {
            let path = self
                .packages_dir
                .join(MOTD_DIR)
                .join(format!("{candidate}.txt"));
            match std::fs::read_to_string(&path) {
                Ok(text) => return Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "reading motd failed");
                    continue;
                }
            }
        }
        None
    }

    /// `manifest gen` (task S3): walk `<dir>/<directory>/**/*.zip`. The top
    /// level is the access directory (validated `[a-z0-9_-]+`; skipped with a
    /// warning otherwise); EVERYTHING below it is the package's `name` — its
    /// path under that directory, `/`-separated, without the `.zip`. Computes
    /// size/sha256/updated_at and merges each zip's `<name>.toml` sidecar
    /// (which sits next to the zip, at whatever depth it lives).
    ///
    /// Returns the manifest plus the warnings the publisher should see.
    /// Top-level zips (no access directory) are an error: every package must
    /// live in one; `public/` is where anonymous packages go.
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
            if dir_name == MOTD_DIR {
                // Message-of-the-day texts, not packages. Served by /motd.
                continue;
            }
            if !valid_directory(&dir_name) {
                generated.warn(format!(
                    "skipping directory {dir_name:?}: not a valid directory name [a-z0-9_-]+"
                ));
                continue;
            }
            if pacenotes_shared::KNOWN_CATEGORIES.contains(&dir_name.as_str()) {
                // The commonest filing mistake: the access-directory level
                // left out, so a category sits where `public` should. Every
                // package would then report its SECOND folder as its
                // category, and the whole tree would be shifted up a level.
                generated.warn(format!(
                    "{dir_name:?} is the top level, where an access directory \
                     (e.g. {:?}) belongs — categories go one level down, as \
                     {}/{dir_name}/…. Everything under it is being published \
                     with {dir_name:?} as its access directory.",
                    pacenotes_shared::PUBLIC_DIR,
                    pacenotes_shared::PUBLIC_DIR
                ));
            }

            // One recursive pass per access directory collects zips and
            // sidecars together, so a sidecar whose zip is missing can be
            // reported afterwards.
            let mut found = Found::default();
            collect(&path, "", &mut found, &mut generated);

            for (name, zip_path) in &found.zips {
                let meta = std::fs::metadata(zip_path)
                    .map_err(|e| anyhow::anyhow!("stat {}: {e}", zip_path.display()))?;
                let updated_at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let previous_entry = previous
                    .packages
                    .iter()
                    .find(|p| p.directory == dir_name && &p.name == name);
                let sidecar = match read_sidecar(&zip_path.with_extension("toml")) {
                    Ok(s) => s,
                    Err(e) => {
                        // A broken sidecar must not silently publish a package
                        // with default metadata.
                        anyhow::bail!("{dir_name}/{name}.toml: {e}");
                    }
                };

                let mut package = pacenotes_shared::PackageInfo {
                    directory: dir_name.clone(),
                    name: name.clone(),
                    // Version precedence: sidecar wins, else the version the
                    // previous manifest carried, else "0.0.0".
                    version: sidecar
                        .version
                        .clone()
                        .or_else(|| previous_entry.map(|p| p.version.clone()))
                        .unwrap_or_else(|| "0.0.0".to_string()),
                    size: meta.len(),
                    sha256: sha256_file(zip_path)?,
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

            for sidecar in &found.sidecars {
                if !found.zips.iter().any(|(name, _)| name == sidecar) {
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

/// Language the MOTD falls back to when the requested one has no file.
pub const MOTD_FALLBACK_LANG: &str = "en";

/// Language tags accepted by `/motd`: `[a-z]{2,8}`, which covers `en`, `pl`
/// and longer ISO tags while excluding every character that could reach
/// outside the motd directory.
pub fn valid_lang(lang: &str) -> bool {
    (2..=8).contains(&lang.len()) && lang.bytes().all(|b| b.is_ascii_lowercase())
}

/// What one access directory's subtree holds, keyed by package `name`
/// (the path under that directory, without the extension).
#[derive(Default)]
struct Found {
    zips: Vec<(String, std::path::PathBuf)>,
    sidecars: Vec<String>,
}

/// Recursively collects the zips and sidecars under an access directory.
/// `prefix` is the path walked so far, so a zip's key is exactly the `name`
/// the protocol wants. Depth-first in name order, which keeps a regenerated
/// manifest byte-stable.
fn collect(dir: &std::path::Path, prefix: &str, found: &mut Found, generated: &mut Generated) {
    let entries = match read_dir_sorted(dir) {
        Ok(entries) => entries,
        Err(e) => {
            generated.warn(format!("skipping {}: {e}", dir.display()));
            return;
        }
    };

    for entry in entries {
        let path = entry.path();
        // A name that does not round-trip through UTF-8 cannot be a lookup
        // key: the manifest would name a file that cannot be reopened.
        let Some(file_name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            generated.warn(format!(
                "skipping {}: name is not valid UTF-8",
                path.display()
            ));
            continue;
        };

        if path.is_dir() {
            collect(&path, &join_name(prefix, &file_name), found, generated);
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let extension = path.extension().and_then(|e| e.to_str());
        if !matches!(extension, Some("zip") | Some("toml")) {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(stem) => stem,
            None => continue,
        };
        let name = join_name(prefix, stem);

        // The choke point: a name that `shared::valid_name` rejects never
        // reaches the manifest, so it can never be turned back into a path.
        if !pacenotes_shared::valid_name(&name) {
            generated.warn(format!(
                "skipping {}: {name:?} is not a usable package name",
                path.display()
            ));
            continue;
        }

        match extension {
            Some("zip") => found.zips.push((name, path)),
            _ => found.sidecars.push(name),
        }
    }
}

/// Appends one segment to a package name. Segments are joined with `/`
/// whatever the host's path separator is: `name` is a protocol string, not a
/// filesystem path.
fn join_name(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}/{segment}")
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
        if !pacenotes_shared::valid_name(&p.name) {
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
        // Names are validated by `shared::valid_name` now: a name is a path
        // under the access directory, so `/` is legal and `\\`, `.`/`..`
        // segments and empty segments are not.
        assert!(pacenotes_shared::valid_name("codriver/polish/male/Kamil"));
        assert!(pacenotes_shared::valid_name("FilipekMod_v1.2"));
        assert!(!pacenotes_shared::valid_name("../secret"));
        assert!(!pacenotes_shared::valid_name("a/../b"));
        assert!(!pacenotes_shared::valid_name("a\\b"));
        assert!(!pacenotes_shared::valid_name("/a"));
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
        write(&dir.join("public/english.toml"), b"game = \"dirtrally2\"\n");

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
    fn generate_recurses_and_names_packages_by_their_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/codriver/polish/male/Kamil.zip"), b"kamil");
        write(
            &dir.join("public/codriver/polish/female/Magda.zip"),
            b"magda",
        );
        write(&dir.join("public/language/polish.zip"), b"polish");
        write(&dir.join("private/style/Descriptive.zip"), b"descriptive");

        let generated = generate(dir);
        assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);

        let mut found: Vec<String> = generated
            .manifest
            .packages
            .iter()
            .map(|p| format!("{}/{}", p.directory, p.name))
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                "private/style/Descriptive",
                "public/codriver/polish/female/Magda",
                "public/codriver/polish/male/Kamil",
                "public/language/polish",
            ]
        );

        // The path IS the presentation: shared derives all three from `name`.
        let kamil = generated
            .manifest
            .get(&pacenotes_shared::PackageRef::parse("public/codriver/polish/male/Kamil").unwrap())
            .unwrap();
        assert_eq!(kamil.category(), pacenotes_shared::CATEGORY_CODRIVER);
        assert_eq!(kamil.tree_levels(), vec!["polish", "male"]);
        assert_eq!(kamil.leaf(), "Kamil");
        assert_eq!(kamil.label(), "Kamil");

        // A package hanging directly off its category still works.
        let polish = generated
            .manifest
            .get(&pacenotes_shared::PackageRef::parse("public/language/polish").unwrap())
            .unwrap();
        assert_eq!(polish.category(), pacenotes_shared::CATEGORY_LANGUAGE);
        assert!(polish.tree_levels().is_empty());
        assert_eq!(polish.leaf(), "polish");
    }

    #[test]
    fn nested_names_resolve_back_to_their_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/codriver/polish/male/Kamil.zip"), b"kamil");
        write_manifest(dir, &generate(dir).manifest);

        let store = super::ManifestStore::new(dir);
        let (package, path) = store
            .zip_path("public", "codriver/polish/male/Kamil")
            .unwrap();
        assert_eq!(package.leaf(), "Kamil");
        assert_eq!(path, dir.join("public/codriver/polish/male/Kamil.zip"));

        // Partial paths are not packages, and nothing outside the manifest
        // resolves at all.
        assert!(store.zip_path("public", "codriver/polish/male").is_none());
        assert!(store.zip_path("public", "codriver").is_none());
        assert!(store
            .zip_path("public", "codriver/../../manifest")
            .is_none());
    }

    #[test]
    fn sidecars_are_read_next_to_a_nested_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/language/polish.zip"), b"lang");
        write(&dir.join("public/codriver/polish/male/Kamil.zip"), b"kamil");
        write(
            &dir.join("public/codriver/polish/male/Kamil.toml"),
            br#"
                version = "3.0.0"
                display_name = "Kamil Filipek"
                requires = ["public/language/polish"]
            "#,
        );

        let generated = generate(dir);
        assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
        let kamil = generated
            .manifest
            .get(&pacenotes_shared::PackageRef::parse("public/codriver/polish/male/Kamil").unwrap())
            .unwrap();
        assert_eq!(kamil.version, "3.0.0");
        // display_name renames the LEAF only; the levels still come from the path.
        assert_eq!(kamil.label(), "Kamil Filipek");
        assert_eq!(kamil.tree_levels(), vec!["polish", "male"]);
        // A nested ref resolves: it splits on the FIRST separator only.
        assert_eq!(kamil.required_refs().len(), 1);
    }

    #[test]
    fn sidecar_keys_that_carried_positional_metadata_are_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("public/language/polish.zip"), b"lang");
        write(
            &dir.join("public/language/polish.toml"),
            b"category = \"language\"\n",
        );

        // `category`/`tree_path` are derived from the path now, so a sidecar
        // still carrying them is a filing mistake worth failing on.
        let err = super::ManifestStore::generate(dir).unwrap_err().to_string();
        assert!(err.contains("polish.toml"), "{err}");
    }

    #[test]
    fn motd_falls_back_to_english_and_refuses_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let store = super::ManifestStore::new(dir);

        // Nothing published: the installer treats this like being offline.
        assert!(store.motd("pl").is_none());

        write(&dir.join("motd/en.txt"), b"English news");
        assert_eq!(store.motd("en").as_deref(), Some("English news"));
        // No pl.txt yet -> English.
        assert_eq!(store.motd("pl").as_deref(), Some("English news"));
        // Absent/odd lang -> English, never a path.
        assert_eq!(store.motd("").as_deref(), Some("English news"));
        assert_eq!(store.motd("../manifest").as_deref(), Some("English news"));
        assert_eq!(store.motd("en/../en").as_deref(), Some("English news"));

        write(&dir.join("motd/pl.txt"), "Polskie wieści".as_bytes());
        assert_eq!(store.motd("pl").as_deref(), Some("Polskie wieści"));
        assert_eq!(store.motd("en").as_deref(), Some("English news"));
    }

    #[test]
    fn lang_validation() {
        assert!(super::valid_lang("en"));
        assert!(super::valid_lang("pt"));
        // Longer ISO tags are fine; the installer only sends two-letter ones
        // (`i18n::Language::tag`).
        assert!(super::valid_lang("ptbr"));
        assert!(!super::valid_lang("e"));
        assert!(!super::valid_lang(""));
        assert!(!super::valid_lang("EN"));
        assert!(!super::valid_lang("en-GB"));
        assert!(!super::valid_lang(".."));
        assert!(!super::valid_lang("toolonglanguagetag"));
    }

    #[test]
    fn motd_directory_is_not_an_access_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join("motd/en.txt"), b"news");
        write(&dir.join("public/language/polish.zip"), b"lang");

        let generated = generate(dir);
        assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
        assert_eq!(generated.manifest.packages.len(), 1);
        assert_eq!(generated.manifest.packages[0].directory, "public");
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
