//! Each game's `<data>/<game>/schema.toml`: which categories its catalogue
//! has, published in the manifest with the hash installers compare.

/// The repository's `schemas/<game>.toml` byte for byte, which installers of
/// the same release embed too.
pub const BUNDLED_SCHEMAS: &[(&str, &[u8])] = &[(
    pacenotes_shared::GAME_RBR,
    include_bytes!("../../../schemas/rbr.toml"),
)];

/// Strict, so a typo cannot silently change what a category means.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaFile {
    #[serde(default)]
    category: Vec<SchemaCategoryFile>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaCategoryFile {
    id: String,
    select: String,
    install_root: String,
}

/// Validates a schema and hashes its raw bytes, comments included, since that
/// is the hash an installer compares.
pub fn parse_schema(bytes: &[u8]) -> anyhow::Result<pacenotes_shared::GameSchema> {
    let text = std::str::from_utf8(bytes).map_err(|_| anyhow::anyhow!("not UTF-8"))?;
    let file: SchemaFile = toml::from_str(text).map_err(|e| anyhow::anyhow!("{e}"))?;
    if file.category.is_empty() {
        anyhow::bail!("declares no [[category]] — a game with no categories can publish nothing");
    }
    let mut categories: Vec<pacenotes_shared::SchemaCategory> = Vec::new();
    for category in file.category {
        if categories.iter().any(|seen| seen.id == category.id) {
            anyhow::bail!("category {:?} is declared twice", category.id);
        }
        categories.push(validated(category)?);
    }
    Ok(pacenotes_shared::GameSchema {
        sha256: crate::store::sha256_bytes(bytes),
        categories,
    })
}

fn validated(category: SchemaCategoryFile) -> anyhow::Result<pacenotes_shared::SchemaCategory> {
    if !pacenotes_shared::valid_id(&category.id) {
        anyhow::bail!(
            "category id {:?} must be [a-z0-9_-]+ (it is a folder name)",
            category.id
        );
    }
    if category.select != pacenotes_shared::SELECT_ONE
        && category.select != pacenotes_shared::SELECT_ANY
    {
        anyhow::bail!(
            "category {:?}: select must be {:?} or {:?}, not {:?}",
            category.id,
            pacenotes_shared::SELECT_ONE,
            pacenotes_shared::SELECT_ANY,
            category.select
        );
    }
    // A token the installer maps to a place it already knows, so nothing here steers writes.
    let is_token = !category.install_root.is_empty()
        && category
            .install_root
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !is_token {
        anyhow::bail!(
            "category {:?}: install_root {:?} must be a token like {:?}, never a path",
            category.id,
            category.install_root,
            pacenotes_shared::INSTALL_ROOT_GAME
        );
    }
    Ok(pacenotes_shared::SchemaCategory {
        id: category.id,
        select: category.select,
        install_root: category.install_root,
    })
}

/// The game's schema, or the error that withholds the whole game.
pub fn load_schema(
    game_dir: &std::path::Path,
    game: &str,
) -> Result<pacenotes_shared::GameSchema, String> {
    let schema_file = pacenotes_shared::SCHEMA_FILE;
    let path = game_dir.join(schema_file);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "{game}/{schema_file} is missing — NOTHING is published for {game} until it \
                 exists (copy schemas/{game}.toml from the repository if there is one)"
            ));
        }
        Err(e) => {
            return Err(format!(
                "reading {}: {e} — {game} not published",
                path.display()
            ))
        }
    };
    parse_schema(&bytes)
        .map_err(|e| format!("{game}/{schema_file}: {e} — nothing published for {game}"))
}

/// Why a game's schema will stop installers of this release from downloading
/// it, if it will; a warning, since the operator may be mid-rollout.
pub fn bundled_schema_warning(
    game: &str,
    schema: &pacenotes_shared::GameSchema,
    bundled: &[(&str, &[u8])],
) -> Option<String> {
    match bundled.iter().find(|(id, _)| *id == game) {
        Some((_, expected)) => {
            let expected = crate::store::sha256_bytes(expected);
            (expected != schema.sha256).then(|| {
                format!(
                    "{game}/{} differs from the schema this server release was built with \
                     (sha256 {} vs {expected}). Installers from this release will REFUSE to \
                     download {game} packages until they are updated — intended only as part \
                     of a schema rollout",
                    pacenotes_shared::SCHEMA_FILE,
                    schema.sha256
                )
            })
        }
        None if !bundled.is_empty() => Some(format!(
            "this server release bundles no schema for {game}: installers built alongside it \
             will refuse to download {game} packages"
        )),
        None => None,
    }
}

#[cfg(test)]
mod tests;
