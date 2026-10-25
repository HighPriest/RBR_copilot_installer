/// The body of `POST /report/conflict`: packages the user selected wrote the
/// same file, and the later one won.
///
/// Carries nothing identifying, only the game, the installer's version and
/// catalogue data the server already has; the server stamps the time itself.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConflictReport {
    /// A `GAME_*` id, which the packages below belong to.
    pub game: String,
    /// E.g. `"0.2.0"`.
    pub installer: String,
    pub conflicts: Vec<Conflict>,
}

/// One file several packages wrote.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// Relative to the install root and `/`-separated, as the ledger records it.
    pub file: String,
    /// `directory/name` refs within the report's game, in install order: the
    /// last one's bytes are on disk.
    pub packages: Vec<String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn conflict_reports_round_trip_in_the_documented_shape() {
        let wire = r#"{"game":"rbr","installer":"0.2.0","conflicts":[
            {"file":"Plugins/Pacenote/PaceNote.ini",
             "packages":["public/language/polish","public/codriver/polish/male/Kamil"]}]}"#;
        let report: super::ConflictReport = serde_json::from_str(wire).unwrap();
        assert_eq!(report.game, crate::GAME_RBR);
        for reference in &report.conflicts[0].packages {
            assert!(crate::PackageRef::parse(reference).is_some(), "{reference}");
        }
        let back: super::ConflictReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(back, report);
    }
}
