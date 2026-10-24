/// Is this a usable game id or access directory name: `[a-z0-9_-]+`?
///
/// Both end up in a URL and on disk, and this alphabet can neither widen into
/// a path nor collide case-insensitively on Windows.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Is this a usable `PackageInfo::name`: `/`-separated segments, none empty,
/// none `.` or `..`, and no `\` or control character anywhere?
///
/// Both sides call it, because a manifest can come from a server the user does not run.
pub fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\\') {
        return false;
    }
    // A name becomes text the installer writes, such as `sounds=` in an ini,
    // so a control character would let a server inject whole lines.
    if name.chars().any(|c| c.is_ascii_control()) {
        return false;
    }
    name.split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    #[test]
    fn valid_id_is_the_rule_for_games_and_directories() {
        assert!(super::valid_id("public"));
        assert!(super::valid_id("rbr"));
        assert!(super::valid_id("premium-voices_2"));
        assert!(!super::valid_id(""));
        assert!(!super::valid_id("Public"));
        assert!(!super::valid_id(".."));
        assert!(!super::valid_id("a/b"));
        assert!(!super::valid_id("a\\b"));
        assert!(!super::valid_id("a.b"));
    }

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
            "",
            "..",
            "codriver/../../etc",
            "codriver/./Kamil",
            "/codriver/Kamil",
            "codriver/Kamil/",
            "codriver//Kamil",
            "codriver\\polish",
        ] {
            assert!(!super::valid_name(bad), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn a_name_can_never_carry_a_control_character() {
        for bad in [
            "codriver/Kamil\nmuteReplayPacenotes=1",
            "codriver/Kamil\r\nenableGUI=0",
            "codriver/\tKamil",
            "codriver/Ka\0mil",
            "codriver/Kamil\u{7f}",
        ] {
            assert!(!super::valid_name(bad), "{bad:?} should be rejected");
        }
        assert!(super::valid_name("style/Numeric Standard"));
        assert!(super::valid_name("codriver/polish/male/Miłosz"));
    }
}
