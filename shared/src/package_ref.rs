/// A package's `directory/name`: its download path, its cache and ledger key,
/// and the wire form of a `requires` entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageRef {
    pub directory: String,
    pub name: String,
}

impl PackageRef {
    /// Splits on the first `/`, since the name is itself a path, and rejects
    /// anything `valid_name` rejects.
    pub fn parse(text: &str) -> Option<PackageRef> {
        let (directory, name) = text.split_once('/')?;
        if directory.is_empty() || directory.contains('\\') || !crate::valid_name(name) {
            return None;
        }
        Some(PackageRef {
            directory: directory.to_string(),
            name: name.to_string(),
        })
    }
}

impl std::fmt::Display for PackageRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}/{}", self.directory, self.name)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_ref_splits_directory_from_a_path_shaped_name() {
        let reference = super::PackageRef::parse("public/codriver/polish/male/Kamil").unwrap();
        assert_eq!(reference.directory, "public");
        assert_eq!(reference.name, "codriver/polish/male/Kamil");
        assert_eq!(reference.to_string(), "public/codriver/polish/male/Kamil");
    }

    #[test]
    fn a_ref_round_trips_through_its_text_form() {
        let reference = super::PackageRef::parse("premium-voices/pro").unwrap();
        assert_eq!(reference.directory, "premium-voices");
        assert_eq!(reference.name, "pro");
        assert_eq!(reference.to_string(), "premium-voices/pro");
    }

    #[test]
    fn a_ref_can_never_widen_into_a_filesystem_path() {
        for bad in [
            "pro",
            "public",
            "/pro",
            "/codriver/Kamil",
            "public/",
            "public/../secrets",
            "public\\pro",
            "public\\codriver/Kamil",
            "..\\..\\data/auth.db",
        ] {
            assert!(
                super::PackageRef::parse(bad).is_none(),
                "{bad:?} should be rejected"
            );
        }
    }
}
