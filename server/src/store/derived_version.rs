/// Marks a version derived from a package's bytes rather than written in a sidecar.
pub const DERIVED_VERSION_PREFIX: &str = "sha-";

/// What every unversioned package got before the manifest became a pure
/// function of the files on disk.
const LEGACY_DEFAULT_VERSION: &str = "0.0.0";

/// `sha-` + the first 8 hex chars of the sha256: the same on every rebuild and
/// replica, and different for every payload, unlike the old fixed `0.0.0`.
pub fn derived_version(sha256: &str) -> String {
    format!("{DERIVED_VERSION_PREFIX}{}", &sha256[..sha256.len().min(8)])
}

/// A version a person chose, as opposed to the legacy default or a derived one.
pub fn is_chosen_version(version: &str) -> bool {
    version != LEGACY_DEFAULT_VERSION && !version.starts_with(DERIVED_VERSION_PREFIX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_derived_version_is_the_sha_prefix() {
        assert_eq!(
            super::derived_version(
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
            ),
            "sha-2cf24dba"
        );
    }

    #[test]
    fn only_a_version_someone_wrote_counts_as_chosen() {
        assert!(super::is_chosen_version("2.3.4"));
        assert!(!super::is_chosen_version("0.0.0"));
        assert!(!super::is_chosen_version("sha-2cf24dba"));
    }
}
