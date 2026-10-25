/// The server's 8 KiB body cap for `/report/conflict`, less room for the fields
/// around the list; an oversized report is answered 413 and tells nobody anything.
const REPORT_BUDGET: usize = 8 * 1024 - 256;

impl super::Api {
    /// `POST /report/conflict`: only the operator can fix packages that fight
    /// over a file, and the user is never blocked by them, so this is how it
    /// reaches them. No token, no paths, no machine name.
    pub fn report_conflicts(
        &self,
        game: &str,
        overlaps: &[pacenotes_installer::engine::Overlap],
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), crate::http::HttpError> {
        let report = pacenotes_shared::ConflictReport {
            game: game.to_string(),
            installer: env!("CARGO_PKG_VERSION").to_string(),
            conflicts: fitting_conflicts(overlaps),
        };
        if report.conflicts.is_empty() {
            return Ok(());
        }
        let body = serde_json::to_vec(&report).map_err(|e| {
            crate::http::HttpError::parse(format!("cannot build the conflict report: {e}"))
        })?;
        crate::http::post_json(&format!("{}/report/conflict", self.base_url), &body, cancel)
    }
}

/// As many conflicts as fit under the cap, measured rather than guessed.
fn fitting_conflicts(
    overlaps: &[pacenotes_installer::engine::Overlap],
) -> Vec<pacenotes_shared::Conflict> {
    let mut conflicts = Vec::new();
    let mut used = 0;
    for overlap in overlaps {
        let packages = overlap.refs();
        // One writer left after dropping unsplittable keys is no conflict.
        if packages.len() < 2 {
            continue;
        }
        let conflict = pacenotes_shared::Conflict {
            file: overlap.path.clone(),
            packages,
        };
        used += serde_json::to_string(&conflict).map_or(0, |json| json.len()) + 1;
        if used > REPORT_BUDGET {
            break;
        }
        conflicts.push(conflict);
    }
    conflicts
}
