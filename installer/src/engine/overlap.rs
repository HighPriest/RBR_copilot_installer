/// A file more than one package in a plan writes. Not an error: the later
/// package wins, the log says so, and the server is told (`installer/CLAUDE.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlap {
    /// As the packages spell it.
    pub path: String,
    /// In install order; the last one wins.
    pub packages: Vec<String>,
}

/// Compared folded, as the filesystem compares names; the file lists come from
/// the caller, who reads each zip once.
pub fn overlaps(
    install_order: &[String],
    files: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<Overlap> {
    let mut writers: std::collections::BTreeMap<String, (String, Vec<String>)> =
        std::collections::BTreeMap::new();
    for package in install_order {
        for path in files.get(package).into_iter().flatten() {
            let (_, packages) = writers
                .entry(crate::ledger::fold(path))
                .or_insert_with(|| (path.clone(), Vec::new()));
            packages.push(package.clone());
        }
    }
    writers
        .into_values()
        .filter(|(_, packages)| packages.len() > 1)
        .map(|(path, packages)| Overlap { path, packages })
        .collect()
}

#[cfg(test)]
mod tests {
    const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
    const POLISH: &str = "rbr/public/language/polish";

    fn files_of(entries: &[(&str, &[&str])]) -> std::collections::BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(package, paths)| {
                (
                    package.to_string(),
                    paths.iter().map(|path| path.to_string()).collect(),
                )
            })
            .collect()
    }

    fn install_order() -> [String; 2] {
        [POLISH.to_string(), KAMIL.to_string()]
    }

    #[test]
    fn overlapping_writers_are_reported_in_install_order() {
        let files = files_of(&[
            (
                POLISH,
                &[
                    "Plugins/Pacenote/PaceNote.ini",
                    "Plugins/Pacenote/language/polish/strings.ini",
                ],
            ),
            (
                KAMIL,
                &[
                    "Plugins/Pacenote/PaceNote.ini",
                    "Plugins/Pacenote/sounds/polish/male/Kamil/1-1.ogg",
                ],
            ),
        ]);
        let found = super::overlaps(&install_order(), &files);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "Plugins/Pacenote/PaceNote.ini");
        assert_eq!(found[0].packages, install_order().to_vec());
    }

    #[test]
    fn an_overlap_is_found_across_spellings() {
        let files = files_of(&[
            (POLISH, &["Plugins/Pacenote/X.ini"]),
            (KAMIL, &["plugins/pacenote/x.INI"]),
        ]);
        assert_eq!(super::overlaps(&install_order(), &files).len(), 1);
    }

    #[test]
    fn packages_that_share_nothing_report_nothing() {
        let files = files_of(&[(POLISH, &["Plugins/a.ini"]), (KAMIL, &["Plugins/b.ini"])]);
        assert!(super::overlaps(&install_order(), &files).is_empty());
    }
}
