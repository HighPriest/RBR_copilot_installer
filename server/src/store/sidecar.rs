/// The optional `<name>.toml` a publisher writes next to a zip.
///
/// Unknown keys are an error, so a sidecar still carrying a retired
/// positional key (`category`, `tree_path`, `game`) fails the run.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Sidecar {
    pub version: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub exclusive_group: Option<String>,
    pub requires: Option<Vec<String>>,
}

/// A missing sidecar is the normal case and reads as an empty one.
pub fn read_sidecar(path: &std::path::Path) -> anyhow::Result<Sidecar> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Sidecar::default()),
        Err(e) => anyhow::bail!("reading {}: {e}", path.display()),
    };
    parse_sidecar(&text)
}

pub fn parse_sidecar(text: &str) -> anyhow::Result<Sidecar> {
    toml::from_str(text).map_err(|e| anyhow::anyhow!("{e}"))
}
