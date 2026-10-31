/// The packages a ledger held at one moment, keyed as the ledger keys them.
pub type Snapshot = std::collections::BTreeMap<String, crate::ledger::LedgerEntry>;

/// One restore point, named by the UTC stamp of the run that followed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub name: String,
    /// What was installed before that run.
    pub snapshot: Snapshot,
}

/// Every readable restore point in a game folder, newest first.
///
/// Sorted by name, not modification time, because the names are UTC stamps and
/// a point copied back from a backup would otherwise sort as the newest run. A
/// point whose snapshot cannot be read is skipped: it cannot be described, so
/// it cannot be offered.
pub fn points(game_dir: &std::path::Path) -> Vec<Point> {
    let dir = game_dir
        .join(crate::ledger::STATE_DIR)
        .join(crate::engine::RESTORE_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<Point> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| read(game_dir, &entry.file_name().to_string_lossy()))
        .collect();
    found.sort_by(|a, b| b.name.cmp(&a.name));
    found
}

/// The point called `name`, or `None` when it has no readable snapshot.
pub fn read(game_dir: &std::path::Path, name: &str) -> Option<Point> {
    let path = crate::engine::snapshot_path(game_dir, name);
    let snapshot = crate::ledger::Ledger::read_snapshot(&path)?;
    Some(Point {
        name: name.to_string(),
        snapshot,
    })
}

/// A point's name as a person reads a date, `2026-10-27 12:11`.
///
/// Seconds and the `-2` suffix of two runs in one second are dropped, since the
/// user is choosing between runs, not timing them; a name of any other shape is
/// shown as it is rather than as a blank row.
pub fn human_time(point: &str) -> String {
    let stem = point.split('Z').next().unwrap_or(point);
    let Some((date, time)) = stem.split_once('T') else {
        return point.to_string();
    };
    let mut parts = time.split('-');
    match (parts.next(), parts.next()) {
        (Some(hours), Some(minutes)) if date.len() == 10 => format!("{date} {hours}:{minutes}"),
        _ => point.to_string(),
    }
}

/// The payload digests this folder's ledger and every restore point name: what
/// a sweep of the cache must keep beyond what its index names, since a restore
/// rolls back to an older payload the index has moved past.
///
/// `None` when any of them cannot be read — a ledger or point written by a
/// newer build, a damaged file — since what it needs is then unknown, and a
/// wrong guess would delete someone's way back.
pub fn payloads_in_use(game_dir: &std::path::Path) -> Option<Vec<String>> {
    let ledger = crate::ledger::Ledger::open(game_dir);
    if ledger.state() == crate::ledger::LedgerState::Unreadable {
        return None;
    }
    let points = points(game_dir);
    // `points` skips a point it cannot read, which here must stop the sweep.
    let directories = std::fs::read_dir(
        game_dir
            .join(crate::ledger::STATE_DIR)
            .join(crate::engine::RESTORE_DIR),
    )
    .map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .count()
    });
    if points.len() != directories {
        return None;
    }
    let snapshots = points.iter().flat_map(|point| point.snapshot.values());
    Some(
        ledger
            .packages()
            .values()
            .chain(snapshots)
            .map(|entry| entry.sha256.clone())
            .collect(),
    )
}
