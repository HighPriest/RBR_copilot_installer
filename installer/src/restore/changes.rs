/// What the run that followed a point did, counted for the list of points.
///
/// Derived, because nothing records what a run was about and a date alone does
/// not help a user choose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub added: usize,
    pub removed: usize,
    pub updated: usize,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added == 0 && self.removed == 0 && self.updated == 0
    }
}

/// `after` is the next point's snapshot, or the live ledger for the newest point.
pub fn changes(before: &super::Snapshot, after: &super::Snapshot) -> Changes {
    let mut counted = Changes::default();
    for (key, then) in before {
        match after.get(key) {
            None => counted.removed += 1,
            Some(now) if now.sha256 != then.sha256 => counted.updated += 1,
            Some(_) => {}
        }
    }
    counted.added = after
        .keys()
        .filter(|key| !before.contains_key(*key))
        .count();
    counted
}
