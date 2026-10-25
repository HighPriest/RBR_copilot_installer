/// How far back `manifest gen` looks.
pub const SUMMARY_DAYS: i64 = 30;

/// Per `(game, package, package)`: the reports naming the pair, and the files.
type PairTally = std::collections::BTreeMap<
    (String, String, String),
    (usize, std::collections::BTreeSet<String>),
>;

/// One warning per pair of packages reported overlapping in the last
/// `SUMMARY_DAYS`, so a user's bad day becomes something the operator sees
/// when publishing; unreadable files and lines are skipped, since this is advice.
pub fn overlap_warnings(reports_dir: &std::path::Path, now: i64) -> Vec<String> {
    let mut pairs = PairTally::new();
    for record in recent_records(reports_dir, now) {
        tally(&record, &mut pairs);
    }
    pairs
        .into_iter()
        .map(|((game, a, b), (reports, files))| {
            let example = files.iter().next().cloned().unwrap_or_default();
            format!(
                "{game}: {a} and {b} were reported overlapping by {reports} installer \
                 run(s) in the last {SUMMARY_DAYS} days, on {} file(s) (e.g. {example}) — \
                 give them an exclusive_group, or move the shared file into one package",
                files.len()
            )
        })
        .collect()
}

fn recent_records(reports_dir: &std::path::Path, now: i64) -> Vec<super::Record> {
    let Ok(entries) = std::fs::read_dir(reports_dir) else {
        return Vec::new();
    };
    let oldest = super::file_name(now - SUMMARY_DAYS * 86_400);
    let mut records = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("conflicts-") || !name.ends_with(".jsonl") || name < oldest {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        records.extend(
            text.lines()
                .filter_map(|line| serde_json::from_str::<super::Record>(line).ok()),
        );
    }
    records
}

/// Counts a report once per pair however many files it names them on.
fn tally(record: &super::Record, pairs: &mut PairTally) {
    let mut in_this_report = std::collections::BTreeSet::new();
    for conflict in &record.conflicts {
        let mut references: Vec<&String> = conflict.packages.iter().collect();
        references.sort();
        references.dedup();
        for (index, a) in references.iter().enumerate() {
            for b in &references[index + 1..] {
                let key = (record.game.clone(), (*a).clone(), (*b).clone());
                pairs
                    .entry(key.clone())
                    .or_default()
                    .1
                    .insert(conflict.file.clone());
                in_this_report.insert(key);
            }
        }
    }
    for key in in_this_report {
        pairs.entry(key).or_default().0 += 1;
    }
}

#[cfg(test)]
mod tests {
    const OCTOBER_25: i64 = 1_792_886_400;

    fn line(file: &str) -> String {
        serde_json::to_string(&super::super::Record {
            received_at: OCTOBER_25,
            game: "rbr".into(),
            installer: "0.2.0".into(),
            conflicts: vec![pacenotes_shared::Conflict {
                file: file.into(),
                packages: vec![
                    "public/language/polish".into(),
                    "public/codriver/polish/male/Kamil".into(),
                ],
            }],
            dropped: 0,
        })
        .unwrap()
    }

    #[test]
    fn overlaps_are_summarised_per_pair_for_recent_days_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let recent = format!("{}\n{}\nnot json\n", line("a.ini"), line("b.ini"));
        std::fs::write(dir.join("conflicts-2026-10-24.jsonl"), recent).unwrap();
        std::fs::write(
            dir.join("conflicts-2026-01-01.jsonl"),
            line("old.ini") + "\n",
        )
        .unwrap();

        let warnings = super::overlap_warnings(dir, OCTOBER_25);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        let warning = &warnings[0];
        assert!(
            warning
                .starts_with("rbr: public/codriver/polish/male/Kamil and public/language/polish"),
            "{warning}"
        );
        assert!(warning.contains("by 2 installer run(s)"), "{warning}");
        assert!(warning.contains("on 2 file(s)"), "{warning}");
    }

    #[test]
    fn no_reports_directory_means_nothing_to_say() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(super::overlap_warnings(&tmp.path().join("absent"), OCTOBER_25).is_empty());
    }
}
