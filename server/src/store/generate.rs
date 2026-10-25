//! `manifest gen`: walk `<data>/<game>/packages/<directory>/**/*.zip`, hash
//! every zip, merge its sidecar, and collect what the publisher should fix.

/// A package without a sidecar `version` and without a previous manifest entry.
const DEFAULT_VERSION: &str = "0.0.0";

/// A zip outside an access directory or with a broken sidecar is withheld as an
/// error rather than published with silently wrong metadata; the rest publishes.
pub fn generate(data_dir: &std::path::Path) -> anyhow::Result<super::Generated> {
    let previous =
        crate::store::manifest_file::read_manifest(&data_dir.join(crate::store::MANIFEST_FILE))
            .unwrap_or_default();
    let mut generated = super::Generated {
        manifest: pacenotes_shared::Manifest::default(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    for entry in read_dir_sorted(data_dir)? {
        let path = entry.path();
        if path.is_file() {
            if is_zip(&path) {
                generated.error(format!(
                    "top-level zip {}: a package belongs under <game>/{}/<directory>/ — \
                     not published",
                    path.display(),
                    crate::store::PACKAGES_DIR
                ));
            }
            continue;
        }
        let game = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() && game != crate::store::MOTD_DIR {
            scan_game(&path, &game, &previous, &mut generated)?;
        }
    }

    for problem in crate::store::requires_check::requires_problems(&generated.manifest) {
        generated.warn(problem);
    }
    Ok(generated)
}

fn scan_game(
    game_dir: &std::path::Path,
    game: &str,
    previous: &pacenotes_shared::Manifest,
    generated: &mut super::Generated,
) -> anyhow::Result<()> {
    let packages_dir = crate::store::PACKAGES_DIR;
    if !pacenotes_shared::valid_id(game) {
        generated.warn(format!("skipping {game:?}: a game id must be [a-z0-9_-]+"));
        return Ok(());
    }
    if pacenotes_shared::KNOWN_CATEGORIES.contains(&game) || game == pacenotes_shared::PUBLIC_DIR {
        generated.warn(format!(
            "{game:?} is at the top level, where a GAME id belongs (e.g. {:?}). \
             The layout is <game>/{packages_dir}/<directory>/<name>.zip.",
            pacenotes_shared::GAME_RBR
        ));
    }
    if !pacenotes_shared::KNOWN_GAMES.contains(&game) {
        generated.warn(format!(
            "{game:?} is not a game this protocol knows ({:?}) — publishing it anyway",
            pacenotes_shared::KNOWN_GAMES
        ));
    }

    let packages_root = game_dir.join(packages_dir);
    if !packages_root.is_dir() {
        generated.warn(format!(
            "{game}/ has no {packages_dir}/ folder — nothing published for it"
        ));
        return Ok(());
    }

    for entry in read_dir_sorted(&packages_root)? {
        let path = entry.path();
        if path.is_file() {
            if is_zip(&path) {
                generated.error(format!(
                    "zip outside an access directory: {} — every package must live in one \
                     (put anonymous packages in {}/); not published",
                    path.display(),
                    pacenotes_shared::PUBLIC_DIR
                ));
            }
            continue;
        }
        let directory = entry.file_name().to_string_lossy().into_owned();
        if !path.is_dir() {
            continue;
        }
        if !pacenotes_shared::valid_id(&directory) {
            generated.warn(format!(
                "skipping {game}/{packages_dir}/{directory:?}: an access directory must be \
                 [a-z0-9_-]+"
            ));
            continue;
        }
        if pacenotes_shared::KNOWN_CATEGORIES.contains(&directory.as_str()) {
            generated.warn(format!(
                "{game}/{packages_dir}/{directory}/ is a CATEGORY name where an access \
                 directory (e.g. {public:?}) belongs — categories go one level down, as \
                 {public}/{directory}/…",
                public = pacenotes_shared::PUBLIC_DIR
            ));
        }
        scan_access_directory(&path, game, &directory, previous, generated)?;
    }
    Ok(())
}

fn scan_access_directory(
    directory_path: &std::path::Path,
    game: &str,
    directory: &str,
    previous: &pacenotes_shared::Manifest,
    generated: &mut super::Generated,
) -> anyhow::Result<()> {
    let mut found = FoundFiles::default();
    collect(directory_path, "", &mut found, generated);

    for (name, zip_path) in &found.zips {
        let previous_entry = previous.packages.iter().find(|package| {
            package.game == game && package.directory == directory && &package.name == name
        });
        let sidecar = match crate::store::sidecar::read_sidecar(&zip_path.with_extension("toml")) {
            Ok(sidecar) => sidecar,
            Err(e) => {
                generated.error(format!(
                    "{game}/{directory}/{name}.toml: {e} — package not published"
                ));
                continue;
            }
        };
        let package = package_entry(game, directory, name, zip_path, sidecar, previous_entry)?;
        generated.manifest.packages.push(package);
    }

    for sidecar in &found.sidecars {
        if !found.zips.iter().any(|(name, _)| name == sidecar) {
            generated.warn(format!(
                "{game}/{directory}/{sidecar}.toml has no {sidecar}.zip next to it — \
                 the metadata is ignored"
            ));
        }
    }
    Ok(())
}

/// A sidecar value wins, then the previous manifest's, then the default;
/// size, hash and mtime always come from the zip.
fn package_entry(
    game: &str,
    directory: &str,
    name: &str,
    zip_path: &std::path::Path,
    sidecar: crate::store::sidecar::Sidecar,
    previous: Option<&pacenotes_shared::PackageInfo>,
) -> anyhow::Result<pacenotes_shared::PackageInfo> {
    let metadata = std::fs::metadata(zip_path)
        .map_err(|e| anyhow::anyhow!("stat {}: {e}", zip_path.display()))?;
    let updated_at = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since_epoch| since_epoch.as_secs() as i64)
        .unwrap_or(0);

    let mut requires = sidecar
        .requires
        .or_else(|| previous.map(|package| package.requires.clone()))
        .unwrap_or_default();
    requires.sort();
    requires.dedup();

    Ok(pacenotes_shared::PackageInfo {
        directory: directory.to_string(),
        name: name.to_string(),
        version: sidecar
            .version
            .or_else(|| previous.map(|package| package.version.clone()))
            .unwrap_or_else(|| DEFAULT_VERSION.to_string()),
        size: metadata.len(),
        sha256: crate::store::sha256_file(zip_path)?,
        updated_at,
        game: game.to_string(),
        display_name: sidecar
            .display_name
            .or_else(|| previous.and_then(|package| package.display_name.clone())),
        description: sidecar
            .description
            .or_else(|| previous.and_then(|package| package.description.clone())),
        exclusive_group: sidecar
            .exclusive_group
            .or_else(|| previous.and_then(|package| package.exclusive_group.clone())),
        requires,
    })
}

/// The zips and sidecars under one access directory, keyed by package `name`.
#[derive(Default)]
struct FoundFiles {
    zips: Vec<(String, std::path::PathBuf)>,
    sidecars: Vec<String>,
}

/// Depth-first in name order, which keeps a regenerated manifest byte-stable.
fn collect(
    dir: &std::path::Path,
    prefix: &str,
    found: &mut FoundFiles,
    generated: &mut super::Generated,
) {
    let entries = match read_dir_sorted(dir) {
        Ok(entries) => entries,
        Err(e) => {
            generated.warn(format!("skipping {}: {e}", dir.display()));
            return;
        }
    };

    for entry in entries {
        let path = entry.path();
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
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

        let extension = path.extension().and_then(|extension| extension.to_str());
        if !path.is_file() || !matches!(extension, Some("zip" | "toml")) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let name = join_name(prefix, stem);
        if !pacenotes_shared::valid_name(&name) {
            generated.warn(format!(
                "skipping {}: {name:?} is not a usable package name",
                path.display()
            ));
            continue;
        }

        if extension == Some("zip") {
            if !url_clean(&name) {
                generated.warn(format!(
                    "{name:?} contains characters a client must percent-encode \
                     (space, #, ?, %, +, …) — consider renaming the file"
                ));
            }
            found.zips.push((name, path));
        } else {
            found.sidecars.push(name);
        }
    }
}

/// Does every byte pass through a URL unescaped, `/` included?
fn url_clean(name: &str) -> bool {
    name.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/'))
}

/// Joins with `/` on every host: a name is a protocol string, not a filesystem path.
fn join_name(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}/{segment}")
    }
}

fn is_zip(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|extension| extension == "zip")
}

fn read_dir_sorted(dir: &std::path::Path) -> anyhow::Result<Vec<std::fs::DirEntry>> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", dir.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

#[cfg(test)]
mod tests;
