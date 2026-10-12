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
//! INSTALL MODEL: every package zip is a game-root overlay — extracting it at
//! the game folder puts every file in its final place, and that is the ONLY
//! install mechanism. The installer never reads instruction files out of a
//! package and never interprets paths found INSIDE a zip.
//!
//! PRESENTATION MODEL: where a package SITS on the server is how it is shown.
//! `name` is the package's path under its access directory, so
//! `codriver/polish/male/Kamil` is category `codriver`, tree levels
//! `polish` → `male`, leaf `Kamil`. One source of truth: organising the files
//! on disk organises the UI, and sidecars carry no positional metadata.
//!
//! That is a reversal of the earlier rule "metadata, NOT paths" — see the
//! Presentation model section of the root CLAUDE.md. The security reasoning
//! behind the old rule is untouched: this is about the SERVER's own storage
//! layout, which the operator controls, not about anything inside a package.
//! `directory`, `name` and refs built from them stay LOOKUP KEYS — the server
//! must still never join them onto a filesystem path (see server/CLAUDE.md).

/// Directory that is always accessible, token or not.
pub const PUBLIC_DIR: &str = "public";

/// Games the installer can target (`PackageInfo::game`). Open string: adding
/// a game must not require a protocol bump, but these are the known values
/// and they match `installer/src/gamedir.rs`, which defers to them.
///
/// The ids are those in `installer/DESIGN.md` rev. 2.
pub const GAME_RBR: &str = "rbr";
pub const GAME_DIRT_RALLY2: &str = "dirtrally2";
pub const GAME_ASSETTOCORSA: &str = "assettocorsa";
pub const GAME_BEAMNG: &str = "beamng";

/// Default for `PackageInfo::game`: manifests written before the installer
/// went multi-game listed Richard Burns Rally packages only.
pub fn default_game() -> String {
    GAME_RBR.to_string()
}

/// UI grouping: the FIRST segment of `PackageInfo::name`, i.e. the top folder
/// a package sits in under its access directory. Open string; these are the
/// known values. Category is orthogonal to the access `directory` —
/// `directory` is auth only, and the two are separate levels of the layout:
/// `packages/<directory>/<category>/.../<package>.zip`.
///
/// The first three are what the wizard's Configuration scene offers, one
/// single-select list each (`installer/DESIGN.md` rev. 2 scene 5), so a
/// package in one of them is a CHOICE. `pacenotes` is the odd one out:
/// those are installed as-is with nothing to pick.
pub const CATEGORY_LANGUAGE: &str = "language";
pub const CATEGORY_CODRIVER: &str = "codriver";
pub const CATEGORY_STYLE: &str = "style";
pub const CATEGORY_PACENOTES: &str = "pacenotes";

/// Every category this protocol knows about. `category` is an open string,
/// so this is not a whitelist — it is what lets the server recognise a
/// category name appearing where an access directory belongs, which is the
/// easiest way to mis-file a package tree.
pub const KNOWN_CATEGORIES: [&str; 4] = [
    CATEGORY_LANGUAGE,
    CATEGORY_CODRIVER,
    CATEGORY_STYLE,
    CATEGORY_PACENOTES,
];

/// The three categories the Configuration scene requires exactly one pick
/// from, in the order the scene lists them.
pub const CHOICE_CATEGORIES: [&str; 3] = [CATEGORY_LANGUAGE, CATEGORY_CODRIVER, CATEGORY_STYLE];

/// One installable package as listed by `GET /packages`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PackageInfo {
    /// Access directory this package belongs to (also its on-disk subdir).
    /// Lowercase [a-z0-9_-]+; validated server-side.
    pub directory: String,
    /// The package's path under its access directory, without the `.zip`:
    /// `<packages>/<directory>/<name>.zip` on disk, and the tail of the
    /// download URL. Segments are separated by `/` and carry the whole
    /// presentation: first = `category`, middle = tree levels, last = the
    /// package itself.
    ///
    /// `codriver/polish/male/Kamil` →
    /// CODRIVER → polish → male → Kamil.
    ///
    /// Validate with [`valid_name`] before trusting one. It is a LOOKUP KEY,
    /// never to be joined onto a filesystem path.
    pub name: String,
    pub version: String,
    /// Size of the package zip in bytes (for progress bars / free-space checks).
    pub size: u64,
    /// Lowercase hex sha256 of the zip. Installer MUST verify after download.
    pub sha256: String,
    /// Unix seconds of last modification (replaces Supabase `UpdatedAt`).
    pub updated_at: i64,
    /// Which game this package overlays. See `GAME_*`.
    #[serde(default = "default_game")]
    pub game: String,
    /// Human name shown for the LEAF. The tree levels above it come from
    /// `name`, so this renames only the package itself.
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

    /// The segments of `name`, i.e. the package's position in the tree.
    pub fn segments(&self) -> std::str::Split<'_, char> {
        self.name.split('/')
    }

    /// Top-level grouping: the first segment. See `CATEGORY_*`.
    ///
    /// A `name` with no separator is its own category, which is what a flat
    /// `packages/<directory>/<package>.zip` layout means.
    pub fn category(&self) -> &str {
        self.name.split('/').next().unwrap_or(&self.name)
    }

    /// The levels BETWEEN the category and the package — `["polish", "male"]`
    /// for `codriver/polish/male/Kamil`. Empty when the package hangs
    /// directly off its category.
    pub fn tree_levels(&self) -> Vec<&str> {
        let mut segments: Vec<&str> = self.segments().collect();
        if segments.len() < 2 {
            return Vec::new();
        }
        segments.pop(); // the package itself
        segments.remove(0); // the category
        segments
    }

    /// The last segment: the package on its own, without its tree position.
    pub fn leaf(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    /// What the UI shows for this package: `display_name` when the publisher
    /// set one, otherwise the LEAF — never the whole path, which the tree is
    /// already showing level by level.
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or_else(|| self.leaf())
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
    /// Parses `<directory>/<name>`, where `name` may itself contain `/`
    /// because it is the package's path under that directory. The FIRST
    /// separator divides them; the rest belong to the name.
    ///
    /// Rejects anything `valid_name` rejects, so a ref can never widen into a
    /// path: no `\`, no `..`, no empty segment, no leading or trailing `/`.
    pub fn parse(text: &str) -> Option<PackageRef> {
        let (directory, name) = text.split_once('/')?;
        if directory.is_empty() || directory.contains('\\') || !valid_name(name) {
            return None;
        }
        Some(PackageRef {
            directory: directory.to_string(),
            name: name.to_string(),
        })
    }
}

/// Is this a usable `PackageInfo::name`?
///
/// `name` is a path now, so this is the choke point that keeps it from
/// becoming a *filesystem* path. Both sides validate: the server when it
/// generates a manifest and serves a download, the installer when it reads
/// one, because a manifest can arrive from a server the user does not run.
///
/// Rejected: empty, a `\` (Windows would read it as a separator), any `.` or
/// `..` segment, an empty segment — which covers a leading `/`, a trailing
/// `/`, and a `//` in the middle.
pub fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\\') {
        return false;
    }
    name.split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
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
        m.packages[1].name = "codriver/polish/male/Kamil".into();
        m.packages[1].game = super::GAME_DIRT_RALLY2.into();
        m.packages[1].display_name = Some("Kamil Filipek".into());
        m.packages[1].exclusive_group = Some("pacenote-config".into());
        m.packages[1].requires = vec!["public/language/english".into()];

        let back: super::Manifest =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        let p = &back.packages[1];
        assert_eq!(p.game, super::GAME_DIRT_RALLY2);
        assert_eq!(p.label(), "Kamil Filipek");
        assert_eq!(p.exclusive_group.as_deref(), Some("pacenote-config"));
        // A requires ref carries a path on its name side now.
        assert_eq!(
            p.required_refs(),
            vec![super::PackageRef {
                directory: "public".into(),
                name: "language/english".into()
            }]
        );
    }

    /// An older manifest must still LOAD, because an installer cache written
    /// by an earlier build is read by a newer one.
    ///
    /// Note what it does NOT do: a flat `name` carries no tree, so such a
    /// package becomes its own category. Fields the manifest no longer has
    /// (`category`, `tree_path`) are ignored rather than rejected. A server
    /// upgrading to this protocol must re-run `manifest gen` over the new
    /// folder layout; there is no way to invent a tree for a flat name.
    #[test]
    fn an_older_manifest_still_deserializes() {
        let old = r#"{"packages":[{"directory":"public","name":"basic",
            "version":"1.0.0","size":5,"sha256":"ab","updated_at":1,
            "category":"pacenotes","tree_path":["ignored"]}]}"#;
        let m: super::Manifest = serde_json::from_str(old).unwrap();
        let p = &m.packages[0];
        assert_eq!(p.game, super::GAME_RBR);
        assert_eq!(p.label(), "basic");
        assert!(p.display_name.is_none());
        assert!(p.exclusive_group.is_none());
        assert!(p.requires.is_empty());
        // Retired fields are ignored, and the flat name is its own category.
        assert_eq!(p.category(), "basic");
        assert!(p.tree_levels().is_empty());
        assert_eq!(p.leaf(), "basic");
    }

    /// The whole presentation comes out of `name`, so this is the behaviour
    /// the Download scene's tree and the Configuration scene's lists rest on.
    #[test]
    fn a_name_is_the_packages_place_in_the_tree() {
        let p = pkg(super::PUBLIC_DIR, "codriver/polish/male/Kamil");
        assert_eq!(p.category(), super::CATEGORY_CODRIVER);
        assert_eq!(p.tree_levels(), vec!["polish", "male"]);
        assert_eq!(p.leaf(), "Kamil");
        // No display_name: the UI shows the leaf, never the whole path, which
        // the tree is already showing one level at a time.
        assert_eq!(p.label(), "Kamil");

        // One level under a category: no middle levels.
        let p = pkg(super::PUBLIC_DIR, "style/Descriptive");
        assert_eq!(p.category(), super::CATEGORY_STYLE);
        assert!(p.tree_levels().is_empty());
        assert_eq!(p.leaf(), "Descriptive");

        // A flat name is its own category and its own leaf.
        let p = pkg(super::PUBLIC_DIR, "basic");
        assert_eq!(p.category(), "basic");
        assert!(p.tree_levels().is_empty());
        assert_eq!(p.leaf(), "basic");
    }

    /// `name` is a lookup key that now looks like a path, so the thing that
    /// stops it BECOMING one is this validator. Every rejected case here is a
    /// way to escape the packages directory or to confuse the server's own
    /// path handling.
    #[test]
    fn a_name_can_never_widen_into_a_filesystem_path() {
        for good in [
            "basic",
            "style/Descriptive",
            "codriver/polish/male/Kamil",
            "language/english",
        ] {
            assert!(super::valid_name(good), "{good:?} should be valid");
        }
        for bad in [
            "",                   // nothing at all
            "..",                 // the parent directory
            "codriver/../../etc", // climbing out
            "codriver/./Kamil",   // a no-op segment
            "/codriver/Kamil",    // absolute
            "codriver/Kamil/",    // trailing separator
            "codriver//Kamil",    // empty segment
            "codriver\\polish",   // Windows separator
        ] {
            assert!(!super::valid_name(bad), "{bad:?} should be rejected");
        }
    }

    /// A ref splits on the FIRST separator: everything after it is the name,
    /// which is itself a path. Without that, no `requires` entry could point
    /// at a nested package.
    #[test]
    fn a_ref_splits_directory_from_a_path_shaped_name() {
        let r = super::PackageRef::parse("public/codriver/polish/male/Kamil").unwrap();
        assert_eq!(r.directory, "public");
        assert_eq!(r.name, "codriver/polish/male/Kamil");
        // Round-trips, so it can be used as a cache and ledger key.
        assert_eq!(r.to_string(), "public/codriver/polish/male/Kamil");

        for bad in [
            "public",                 // no name half
            "/codriver/Kamil",        // no directory half
            "public/",                // empty name
            "public/../secrets",      // climbing out
            "public\\codriver/Kamil", // Windows separator
        ] {
            assert!(
                super::PackageRef::parse(bad).is_none(),
                "{bad:?} should be rejected"
            );
        }
    }

    /// Absent optionals stay absent on the wire, so a v1 reader sees a v1
    /// document and the manifest stays readable by hand.
    #[test]
    fn empty_metadata_is_not_serialized() {
        let json = serde_json::to_string(&pkg(super::PUBLIC_DIR, "basic")).unwrap();
        assert!(!json.contains("display_name"), "{json}");
        assert!(!json.contains("exclusive_group"), "{json}");
        assert!(!json.contains("requires"), "{json}");
        // game is not optional: it always ships. `category` is NOT a field at
        // all any more — it is the first segment of `name`.
        assert!(json.contains("\"game\":\"rbr\""), "{json}");
        assert!(!json.contains("category"), "{json}");
        assert!(!json.contains("tree_path"), "{json}");
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
        assert!(super::PackageRef::parse("public\\pro").is_none());
        assert!(super::PackageRef::parse("..\\..\\data/auth.db").is_none());
        // `public/sub/pro` is now VALID and means a nested package; see
        // a_ref_splits_directory_from_a_path_shaped_name.
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
