//! Protocol types shared between installer and server.
//!
//! RULES (see root CLAUDE.md):
//! - serde structs/enums only; no I/O, no async, no HTTP.
//! - Timestamps are unix seconds (i64), never chrono types.
//! - Any change here is a protocol change: update both sides in the same PR.
//! - Full paths at the point of use instead of `use` statements.
//!
//! ACCESS MODEL: packages live in named directories. The directory named
//! `PUBLIC_DIR` is visible to everyone; every other directory is visible only
//! to credentials granted that directory. One credential can hold grants for
//! any number of directories. There are no tiers.
//!
//! INSTALL MODEL (protocol v2, milestone M1a): every package zip is a
//! game-root overlay — extracting it at the game folder puts every file in
//! its final place, and that is the ONLY install mechanism. Everything that
//! varies between packages is metadata carried HERE, never paths inside the
//! zip and never instruction files the installer would have to interpret.

/// Directory that is always accessible, token or not.
pub const PUBLIC_DIR: &str = "public";

/// Games the installer can target (`PackageInfo::game`). Open string: adding
/// a game must not require a protocol bump, but these are the known values
/// and they match `installer/src/gamedir.rs`.
pub const GAME_RBR: &str = "rbr";
pub const GAME_DIRT_RALLY: &str = "dirtrally";
pub const GAME_BEAMNG: &str = "beamng";

/// Default for `PackageInfo::game`: manifests written before the installer
/// went multi-game listed Richard Burns Rally packages only.
pub fn default_game() -> String {
    GAME_RBR.to_string()
}

/// UI grouping (`PackageInfo::category`). Open string; these are the known
/// values. Category is orthogonal to the access `directory` — `directory` is
/// auth only.
pub const CATEGORY_SOUNDS: &str = "sounds";
pub const CATEGORY_CONFIGS: &str = "configs";
pub const CATEGORY_LANGUAGES: &str = "languages";
pub const CATEGORY_PACENOTES: &str = "pacenotes";

/// Default for `PackageInfo::category`, for manifests written before the
/// field existed.
pub fn default_category() -> String {
    CATEGORY_PACENOTES.to_string()
}

/// One installable package as listed by `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PackageInfo {
    /// Access directory this package belongs to (also its on-disk subdir).
    /// Lowercase [a-z0-9_-]+; validated server-side.
    pub directory: String,
    /// Name unique WITHIN its directory; also the download path segment and
    /// the on-disk filename stem (`<packages>/<directory>/<name>.zip`).
    pub name: String,
    pub version: String,
    /// Size of the package zip in bytes (for progress bars / free-space checks).
    pub size: u64,
    /// Lowercase hex sha256 of the zip. Installer MUST verify after download.
    pub sha256: String,
    /// Unix seconds of last modification (replaces Supabase `UpdatedAt`).
    pub updated_at: i64,
    /// Which game this package overlays — the installer's ribbon tab.
    /// See `GAME_*`.
    #[serde(default = "default_game")]
    pub game: String,
    /// UI grouping within a game tab. See `CATEGORY_*`.
    #[serde(default = "default_category")]
    pub category: String,
    /// Human name shown in the UI. Replaces the legacy scheme of encoding a
    /// hierarchy in the storage path (`sounds\lang\gender\Voice.zip`).
    /// Falls back to `name` when absent — see `PackageInfo::label`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional human description shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Packages sharing a group overwrite the same files (e.g. the
    /// pacenote-config variants), so at most one may be installed: the UI
    /// radio-selects within a group and Apply uninstalls the previous member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_group: Option<String>,
    /// Dependencies as "directory/name" refs (see `PackageRef`), e.g. a voice
    /// pack requiring its language pack. Empty when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
}

impl PackageInfo {
    pub fn is_public(&self) -> bool {
        self.directory == PUBLIC_DIR
    }

    /// Stable identity across directories: the download path and the key both
    /// the cache index and the install ledger use.
    pub fn package_ref(&self) -> PackageRef {
        PackageRef {
            directory: self.directory.clone(),
            name: self.name.clone(),
        }
    }

    /// What the UI shows: `display_name` when the publisher set one, the raw
    /// `name` otherwise.
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    /// Parsed `requires` entries. Malformed refs are dropped rather than
    /// failing the whole manifest — the server warns about them at
    /// `manifest gen` time, where the publisher can still fix them.
    pub fn required_refs(&self) -> Vec<PackageRef> {
        self.requires
            .iter()
            .filter_map(|r| PackageRef::parse(r))
            .collect()
    }
}

/// A "directory/name" reference to a package: the download path, the cache
/// index key, the ledger key and the `requires` wire format, all one shape.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageRef {
    pub directory: String,
    pub name: String,
}

impl PackageRef {
    /// Parses exactly `<directory>/<name>`. Rejects anything with a second
    /// separator, an empty half, or a `\` (the installer runs on Windows,
    /// where a backslash would read as a path), so a ref can never widen into
    /// a path.
    pub fn parse(text: &str) -> Option<PackageRef> {
        if text.contains('\\') {
            return None;
        }
        let (directory, name) = text.split_once('/')?;
        if directory.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        Some(PackageRef {
            directory: directory.to_string(),
            name: name.to_string(),
        })
    }
}

impl std::fmt::Display for PackageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.directory, self.name)
    }
}

/// Response body of `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub packages: Vec<PackageInfo>,
}

impl Manifest {
    /// Keep only packages visible to a caller holding `grants`
    /// (directory names). `public` is always kept. Used by the server;
    /// exercised by tests here.
    pub fn visible_to(mut self, grants: &[String]) -> Self {
        self.packages
            .retain(|p| p.is_public() || grants.iter().any(|g| g == &p.directory));
        self
    }

    /// Anonymous view.
    pub fn public_only(self) -> Self {
        self.visible_to(&[])
    }

    /// Lookup by ref — the manifest is small (tens of entries), so a scan is
    /// the right data structure.
    pub fn get(&self, reference: &PackageRef) -> Option<&PackageInfo> {
        self.packages
            .iter()
            .find(|p| p.directory == reference.directory && p.name == reference.name)
    }
}

/// Uniform error body for non-2xx responses.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ApiError {
    pub code: String,    // e.g. "unauthorized", "forbidden", "not_found"
    pub message: String, // human-readable, safe to show in a MessageBox
}

/// Header name carrying the bearer token ("Authorization: Bearer <token>").
pub const AUTH_HEADER: &str = "Authorization";
pub const BEARER_PREFIX: &str = "Bearer ";

#[cfg(test)]
mod tests {
    fn pkg(dir: &str, name: &str) -> super::PackageInfo {
        super::PackageInfo {
            directory: dir.into(),
            name: name.into(),
            version: "1.0.0".into(),
            size: 123,
            sha256: "ab".repeat(32),
            updated_at: 1_700_000_000,
            game: super::default_game(),
            category: super::default_category(),
            display_name: None,
            description: None,
            exclusive_group: None,
            requires: Vec::new(),
        }
    }

    fn sample() -> super::Manifest {
        super::Manifest {
            packages: vec![
                pkg(super::PUBLIC_DIR, "FilipekMod_numeric_standard"),
                pkg("premium-voices", "FilipekMod_premium_pack"),
                pkg("beta", "FilipekMod_beta_pack"),
            ],
        }
    }

    #[test]
    fn json_round_trip() {
        let m = sample();
        let s = serde_json::to_string(&m).unwrap();
        let back: super::Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.packages.len(), 3);
        assert_eq!(back.packages[1].directory, "premium-voices");
    }

    #[test]
    fn round_trip_keeps_install_metadata() {
        let mut m = sample();
        m.packages[1].game = super::GAME_DIRT_RALLY.into();
        m.packages[1].category = super::CATEGORY_SOUNDS.into();
        m.packages[1].display_name = Some("English / Female / Eva".into());
        m.packages[1].exclusive_group = Some("pacenote-config".into());
        m.packages[1].requires = vec!["public/english-language".into()];

        let back: super::Manifest =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        let p = &back.packages[1];
        assert_eq!(p.game, super::GAME_DIRT_RALLY);
        assert_eq!(p.category, super::CATEGORY_SOUNDS);
        assert_eq!(p.label(), "English / Female / Eva");
        assert_eq!(p.exclusive_group.as_deref(), Some("pacenote-config"));
        assert_eq!(
            p.required_refs(),
            vec![super::PackageRef {
                directory: "public".into(),
                name: "english-language".into()
            }]
        );
    }

    /// A v1 manifest (no install metadata) must still load: the server may be
    /// upgraded before every manifest.json is regenerated, and an installer
    /// cache written by an older build is read by a newer one.
    #[test]
    fn v1_manifest_still_deserializes() {
        let v1 = r#"{"packages":[{"directory":"public","name":"basic",
            "version":"1.0.0","size":5,"sha256":"ab","updated_at":1}]}"#;
        let m: super::Manifest = serde_json::from_str(v1).unwrap();
        let p = &m.packages[0];
        assert_eq!(p.game, super::GAME_RBR);
        assert_eq!(p.category, super::CATEGORY_PACENOTES);
        assert_eq!(p.label(), "basic");
        assert!(p.display_name.is_none());
        assert!(p.exclusive_group.is_none());
        assert!(p.requires.is_empty());
    }

    /// Absent optionals stay absent on the wire, so a v1 reader sees a v1
    /// document and the manifest stays readable by hand.
    #[test]
    fn empty_metadata_is_not_serialized() {
        let json = serde_json::to_string(&pkg(super::PUBLIC_DIR, "basic")).unwrap();
        assert!(!json.contains("display_name"), "{json}");
        assert!(!json.contains("exclusive_group"), "{json}");
        assert!(!json.contains("requires"), "{json}");
        // game/category are not optional: they always ship.
        assert!(json.contains("\"game\":\"rbr\""), "{json}");
        assert!(json.contains("\"category\":\"pacenotes\""), "{json}");
    }

    #[test]
    fn package_ref_round_trips_and_rejects_paths() {
        let r = super::PackageRef::parse("premium-voices/pro").unwrap();
        assert_eq!(r.directory, "premium-voices");
        assert_eq!(r.name, "pro");
        assert_eq!(r.to_string(), "premium-voices/pro");

        assert!(super::PackageRef::parse("pro").is_none());
        assert!(super::PackageRef::parse("/pro").is_none());
        assert!(super::PackageRef::parse("public/").is_none());
        assert!(super::PackageRef::parse("public/sub/pro").is_none());
        assert!(super::PackageRef::parse("public\\pro").is_none());
        assert!(super::PackageRef::parse("..\\..\\data/auth.db").is_none());
    }

    #[test]
    fn malformed_requires_are_dropped_not_fatal() {
        let mut p = pkg(super::PUBLIC_DIR, "voice");
        p.requires = vec!["public/lang".into(), "garbage".into()];
        assert_eq!(p.required_refs().len(), 1);
    }

    #[test]
    fn manifest_get_finds_by_ref() {
        let m = sample();
        let r = super::PackageRef::parse("beta/FilipekMod_beta_pack").unwrap();
        assert_eq!(m.get(&r).unwrap().name, "FilipekMod_beta_pack");
        assert!(m
            .get(&super::PackageRef::parse("beta/nope").unwrap())
            .is_none());
    }

    #[test]
    fn public_only_keeps_public_dir() {
        let m = sample().public_only();
        assert_eq!(m.packages.len(), 1);
        assert!(m.packages[0].is_public());
    }

    #[test]
    fn visible_to_unions_grants_with_public() {
        let m = sample().visible_to(&["beta".to_string()]);
        let dirs: Vec<_> = m.packages.iter().map(|p| p.directory.as_str()).collect();
        assert_eq!(dirs, vec![super::PUBLIC_DIR, "beta"]);
    }

    #[test]
    fn multiple_grants() {
        let m = sample().visible_to(&["beta".to_string(), "premium-voices".to_string()]);
        assert_eq!(m.packages.len(), 3);
    }
}
