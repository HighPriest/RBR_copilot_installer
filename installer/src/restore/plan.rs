/// One package the restore has to put back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PutBack {
    pub key: String,
    /// The digest the snapshot recorded, which is why restore cannot reuse
    /// `engine::plan`: the cache index holds only the newest version of a key,
    /// while the older payload is still on disk under its own digest.
    pub sha256: String,
    pub version: String,
}

/// What restoring to a point would do, shown as the preview and then carried
/// out unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Installed now and not wanted at that point, or wanted in another
    /// version, in which case it is also in `put_back`.
    pub remove: Vec<String>,
    /// Installed from the cache, in key order.
    pub put_back: Vec<PutBack>,
    /// Wanted at that point but no longer in the cache: neither removed nor
    /// reinstalled, and named in the preview before the user commits.
    pub unavailable: Vec<PutBack>,
    /// Already as the point had it.
    pub keep: Vec<String>,
}

impl Plan {
    /// The folder already matches the point.
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.put_back.is_empty()
    }

    /// Some of what the point held cannot come back.
    pub fn is_partial(&self) -> bool {
        !self.unavailable.is_empty()
    }
}

/// What it takes to make `live` look like `target`.
///
/// `payload_present` says whether a digest is in the cache, so the decision is
/// testable without one. An empty `target` is a complete uninstall: the same
/// operation, deliberately not a second code path.
pub fn plan(
    target: &super::Snapshot,
    live: &super::Snapshot,
    payload_present: &dyn Fn(&str) -> bool,
) -> Plan {
    let mut plan = Plan::default();
    for (key, now) in live {
        let Some(then) = target.get(key) else {
            plan.remove.push(key.clone());
            continue;
        };
        // A half-written entry is never already correct: its files may be partial.
        if then.sha256 == now.sha256 && now.state == crate::ledger::EntryState::Installed {
            plan.keep.push(key.clone());
        } else if payload_present(&then.sha256) {
            plan.remove.push(key.clone());
            plan.put_back.push(put_back(key, then));
        } else {
            plan.unavailable.push(put_back(key, then));
        }
    }
    for (key, then) in target.iter().filter(|(key, _)| !live.contains_key(*key)) {
        if payload_present(&then.sha256) {
            plan.put_back.push(put_back(key, then));
        } else {
            plan.unavailable.push(put_back(key, then));
        }
    }
    plan
}

fn put_back(key: &str, then: &crate::ledger::LedgerEntry) -> PutBack {
    PutBack {
        key: key.to_string(),
        sha256: then.sha256.clone(),
        version: then.version.clone(),
    }
}

/// The ledger once `plan` has been carried out, `failed` naming what could
/// not be put back after all.
///
/// A restored package gets the snapshot's entry, because its per-file `prior`
/// records what the user had before this installer first wrote there; a fresh
/// install would record `absent` and a later uninstall would delete the
/// user's file. A package left alone keeps its live entry, which describes
/// what is actually on disk.
pub fn ledger_after(
    target: &super::Snapshot,
    live: &super::Snapshot,
    plan: &Plan,
    failed: &[String],
) -> super::Snapshot {
    let restored = plan
        .put_back
        .iter()
        .filter(|wanted| !failed.contains(&wanted.key))
        .filter_map(|wanted| Some((wanted.key.clone(), target.get(&wanted.key)?.clone())));
    let untouched = plan
        .keep
        .iter()
        .chain(plan.unavailable.iter().map(|wanted| &wanted.key))
        .filter_map(|key| Some((key.clone(), live.get(key)?.clone())));
    untouched.chain(restored).collect()
}

/// Per single-pick category, the package the restored state holds in it, which
/// is what the game's configuration step needs to make it the user's pick again.
pub fn picks(
    target: &super::Snapshot,
    choice_categories: &[&str],
) -> std::collections::BTreeMap<String, String> {
    choice_categories
        .iter()
        .filter_map(|category| {
            let key = target
                .keys()
                .find(|key| pacenotes_shared::category_of_download_path(key) == Some(category))?;
            Some((category.to_string(), key.clone()))
        })
        .collect()
}
