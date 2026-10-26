/// Creates `.pacenotes\restore\<now>\` holding the ledger as it is before the
/// run, which is what lets a restore also remove what was installed afterwards.
pub fn open_restore_point(
    game_dir: &std::path::Path,
    ledger: &crate::ledger::Ledger,
    watcher: &mut dyn crate::engine::Watcher,
) -> Result<String, super::ExecuteError> {
    let name = utc_stamp(unix_now());
    let dir = restore_dir(game_dir, &name);
    std::fs::create_dir_all(dir.join("files"))
        .map_err(|e| super::ExecuteError::io(format!("creating the restore point {name}"), e))?;
    ledger
        .save_snapshot(&dir.join("ledger.json"))
        .map_err(|e| super::ExecuteError::io("recording what is installed", e))?;
    watcher.log(&format!("restore point {name}"));
    Ok(name)
}

pub fn restore_dir(game_dir: &std::path::Path, point: &str) -> std::path::PathBuf {
    game_dir
        .join(crate::ledger::STATE_DIR)
        .join("restore")
        .join(point)
}

/// Where `point` keeps its copy of the file at `relative`.
pub fn saved_copy(game_dir: &std::path::Path, point: &str, relative: &str) -> std::path::PathBuf {
    crate::relative_path::join(&restore_dir(game_dir, point).join("files"), relative)
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-10-25T18-15-00Z`: sortable, and without ISO 8601's colons, which
/// Windows forbids in a directory name.
///
/// Howard Hinnant's civil-from-days arithmetic, hand-rolled to spare a date crate.
pub fn utc_stamp(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds = unix_seconds.rem_euclid(86_400);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}Z",
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    /// Expected values come from .NET's `DateTimeOffset.FromUnixTimeSeconds`,
    /// since a hand-worked expectation passes however broken the code is.
    #[test]
    fn restore_point_names_match_the_utc_calendar() {
        assert_eq!(super::utc_stamp(0), "1970-01-01T00-00-00Z");
        assert_eq!(super::utc_stamp(1_761_000_000), "2025-10-20T22-40-00Z");
        assert_eq!(super::utc_stamp(1_709_164_800), "2024-02-29T00-00-00Z");
    }

    #[test]
    fn restore_point_names_sort_chronologically_and_are_legal_on_windows() {
        let earlier = super::utc_stamp(1_761_000_000);
        let later = super::utc_stamp(1_761_000_001);
        assert!(earlier < later);
        for forbidden in [':', '/', '\\', '*', '?', '"', '<', '>', '|'] {
            assert!(!later.contains(forbidden), "{later}");
        }
    }
}
