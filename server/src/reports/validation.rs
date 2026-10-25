/// A real report is a few hundred bytes.
pub const MAX_BODY_BYTES: usize = 8 * 1024;
/// Kept from one report; the rest count as dropped.
pub const MAX_CONFLICTS: usize = 32;
/// A conflict needs two writers, and nobody sends nine by accident.
pub const MIN_PACKAGES: usize = 2;
pub const MAX_PACKAGES: usize = 8;
/// About a Windows path: anything longer is not a game file.
pub const MAX_FILE_BYTES: usize = 260;
pub const MAX_INSTALLER_LEN: usize = 32;

/// What a report that passed validation stores.
#[derive(Debug, PartialEq, Eq)]
pub struct Validated {
    pub game: String,
    pub installer: String,
    pub conflicts: Vec<pacenotes_shared::Conflict>,
    /// Invalid or over the cap.
    pub dropped: usize,
}

/// A bad game, installer or an empty result rejects the report; a bad conflict
/// is dropped and counted, so one malformed entry does not cost the rest.
pub fn validate(report: pacenotes_shared::ConflictReport) -> Result<Validated, &'static str> {
    if !pacenotes_shared::valid_id(&report.game) {
        return Err("game is not a valid game id");
    }
    let installer_ok = (1..=MAX_INSTALLER_LEN).contains(&report.installer.len())
        && report
            .installer
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'));
    if !installer_ok {
        return Err("installer must be 1-32 characters of [0-9A-Za-z.+-]");
    }

    let mut conflicts = Vec::new();
    let mut dropped = 0;
    for conflict in report.conflicts {
        if conflicts.len() < MAX_CONFLICTS && is_usable(&conflict) {
            conflicts.push(conflict);
        } else {
            dropped += 1;
        }
    }
    if conflicts.is_empty() {
        return Err("the report holds no valid conflict");
    }
    Ok(Validated {
        game: report.game,
        installer: report.installer,
        conflicts,
        dropped,
    })
}

fn is_usable(conflict: &pacenotes_shared::Conflict) -> bool {
    conflict.file.len() <= MAX_FILE_BYTES
        && pacenotes_shared::valid_name(&conflict.file)
        && (MIN_PACKAGES..=MAX_PACKAGES).contains(&conflict.packages.len())
        && conflict
            .packages
            .iter()
            .all(|reference| pacenotes_shared::PackageRef::parse(reference).is_some())
}

#[cfg(test)]
mod tests {
    const PAIR: [&str; 2] = [
        "public/language/polish",
        "public/codriver/polish/male/Kamil",
    ];

    fn report(
        game: &str,
        installer: &str,
        conflicts: Vec<(&str, Vec<&str>)>,
    ) -> pacenotes_shared::ConflictReport {
        pacenotes_shared::ConflictReport {
            game: game.into(),
            installer: installer.into(),
            conflicts: conflicts
                .into_iter()
                .map(|(file, packages)| pacenotes_shared::Conflict {
                    file: file.into(),
                    packages: packages.into_iter().map(String::from).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_good_report_passes_untouched() {
        let validated = super::validate(report(
            "rbr",
            "0.2.0",
            vec![("Plugins/Pacenote/PaceNote.ini", PAIR.to_vec())],
        ))
        .unwrap();
        assert_eq!(validated.game, "rbr");
        assert_eq!(validated.conflicts.len(), 1);
        assert_eq!(validated.dropped, 0);
    }

    #[test]
    fn whole_report_problems_reject_it() {
        let one_conflict = || vec![("a/b.ini", PAIR.to_vec())];
        for (game, installer, conflicts) in [
            ("../etc", "0.2.0", one_conflict()),
            ("rbr", "", one_conflict()),
            ("rbr", "0.2.0; rm -rf", one_conflict()),
            ("rbr", &"9".repeat(33), one_conflict()),
            ("rbr", "0.2.0", vec![]),
        ] {
            assert!(
                super::validate(report(game, installer, conflicts)).is_err(),
                "{game} {installer}"
            );
        }
    }

    #[test]
    fn bad_entries_are_dropped_and_counted_not_fatal() {
        let long = "a/".repeat(140) + "x.ini";
        let nine: Vec<&str> = std::iter::repeat_n(PAIR[0], 9).collect();
        let validated = super::validate(report(
            "rbr",
            "0.2.0",
            vec![
                ("Plugins/Pacenote/PaceNote.ini", PAIR.to_vec()),
                ("C:\\Windows\\system.ini", PAIR.to_vec()),
                ("../escape.ini", PAIR.to_vec()),
                (long.as_str(), PAIR.to_vec()),
                ("a.ini", vec![PAIR[0]]),
                ("a.ini", nine),
                ("a.ini", vec![PAIR[0], "no-separator"]),
            ],
        ))
        .unwrap();
        assert_eq!(validated.conflicts.len(), 1);
        assert_eq!(validated.dropped, 6);
    }

    #[test]
    fn conflicts_beyond_the_cap_are_dropped() {
        let many: Vec<(&str, Vec<&str>)> = (0..40).map(|_| ("a.ini", PAIR.to_vec())).collect();
        let validated = super::validate(report("rbr", "0.2.0", many)).unwrap();
        assert_eq!(validated.conflicts.len(), super::MAX_CONFLICTS);
        assert_eq!(validated.dropped, 40 - super::MAX_CONFLICTS);
    }
}
