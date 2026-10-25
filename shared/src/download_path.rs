/// Takes a `game/directory/name` apart, the inverse of
/// `PackageInfo::download_path`; only the first two `/` separate, since the
/// name is itself a path. `None` unless every part is valid.
pub fn split_download_path(path: &str) -> Option<(&str, &str, &str)> {
    let (game, rest) = path.split_once('/')?;
    let (directory, name) = rest.split_once('/')?;
    if !crate::valid_id(game) || !crate::valid_id(directory) || !crate::valid_name(name) {
        return None;
    }
    Some((game, directory, name))
}

/// The first segment of the name in a `game/directory/name`.
pub fn category_of_download_path(path: &str) -> Option<&str> {
    let (_, _, name) = split_download_path(path)?;
    Some(name.split('/').next().unwrap_or(name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_download_path_comes_apart_the_way_it_went_together() {
        let path = crate::fixtures::package(crate::PUBLIC_DIR, "codriver/polish/male/Kamil")
            .download_path();
        assert_eq!(
            super::split_download_path(&path),
            Some(("rbr", "public", "codriver/polish/male/Kamil"))
        );
        assert_eq!(
            super::category_of_download_path(&path),
            Some(crate::CATEGORY_CODRIVER)
        );
        assert_eq!(
            super::category_of_download_path("rbr/public/language/polish"),
            Some(crate::CATEGORY_LANGUAGE)
        );
    }

    /// Half a package would have a caller look up, delete or download the wrong thing.
    #[test]
    fn a_malformed_download_path_parses_to_nothing() {
        for bad in [
            "",
            "rbr",
            "rbr/public",
            "rbr/public/",
            "rbr/public/../escape",
            "RBR/public/x",
            "rbr/Public/x",
            "rbr/public/a\\b",
            "rbr/public/x\nkey=evil",
        ] {
            assert_eq!(super::split_download_path(bad), None, "{bad:?}");
        }
    }
}
