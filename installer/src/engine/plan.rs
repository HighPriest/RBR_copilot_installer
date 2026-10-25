/// One step of a plan, for a `game/directory/name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Uninstall(String),
    Install(String),
}

impl Action {
    pub fn key(&self) -> &str {
        match self {
            Action::Uninstall(key) | Action::Install(key) => key,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// Selected without a verified payload, so the download phase failed or the cache lost it.
    NothingToInstallFrom { package: String },
    /// A dependency that is not selected, which the user can fix by ticking it.
    MissingRequirement { package: String, requires: String },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::NothingToInstallFrom { package } => write!(
                formatter,
                "{package} was selected but has not been downloaded, so there is \
                 nothing to install from"
            ),
            PlanError::MissingRequirement { package, requires } => write!(
                formatter,
                "{package} needs {requires}, which is neither selected nor already \
                 installed"
            ),
        }
    }
}

pub struct PlanInput<'a> {
    /// Dependency refs resolve within this game.
    pub game: &'a str,
    /// Everything the folder should hold afterwards, kept packages included,
    /// in install order, which decides who wins an overlap.
    pub selection: &'a [String],
    /// This game's single-pick categories, where a new pick supersedes the installed one.
    pub choice_categories: &'a [&'a str],
    pub installed: &'a std::collections::BTreeMap<String, crate::ledger::LedgerEntry>,
    /// Cache entries whose payload is present.
    pub available: &'a std::collections::BTreeMap<String, crate::tree::CachedPackage>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Every uninstall, then every install.
    pub actions: Vec<Action>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn uninstalls(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                Action::Uninstall(key) => Some(key.as_str()),
                Action::Install(_) => None,
            })
            .collect()
    }

    pub fn installs(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                Action::Install(key) => Some(key.as_str()),
                Action::Uninstall(_) => None,
            })
            .collect()
    }
}

/// Uninstalls always come first, because the style packages ship identical
/// paths and the other order would delete what was just written.
pub fn plan(input: &PlanInput) -> Result<Plan, PlanError> {
    let selected: std::collections::BTreeSet<&str> =
        input.selection.iter().map(String::as_str).collect();
    let uninstall = packages_to_uninstall(input, &selected);
    let install = packages_to_install(input)?;
    check_requirements(input, &selected)?;

    let mut actions: Vec<Action> = uninstall.into_iter().map(Action::Uninstall).collect();
    actions.extend(install.into_iter().map(Action::Install));
    Ok(Plan { actions })
}

/// Installed packages that are unwanted, half-written by a dead run, outdated,
/// or superseded by a new pick in the same group or choice category.
fn packages_to_uninstall(
    input: &PlanInput,
    selected: &std::collections::BTreeSet<&str>,
) -> std::collections::BTreeSet<String> {
    let mut uninstall = std::collections::BTreeSet::new();
    for (key, entry) in input.installed {
        let unwanted = !selected.contains(key.as_str());
        let unfinished = entry.state == crate::ledger::EntryState::Installing;
        let outdated = input
            .available
            .get(key)
            .is_some_and(|cached| cached.sha256 != entry.sha256);
        if unwanted || unfinished || outdated {
            uninstall.insert(key.clone());
        }
    }

    for key in input.selection {
        let Some(incoming) = input.available.get(key.as_str()) else {
            continue;
        };
        for (installed_key, entry) in input.installed {
            if installed_key != key
                && !selected.contains(installed_key.as_str())
                && supersedes(key, incoming, installed_key, entry, input.choice_categories)
            {
                uninstall.insert(installed_key.clone());
            }
        }
    }
    uninstall
}

/// The selection, in order, minus what is already installed at the same sha.
fn packages_to_install(input: &PlanInput) -> Result<Vec<String>, PlanError> {
    let mut install = Vec::new();
    for key in input.selection {
        let Some(cached) = input.available.get(key.as_str()) else {
            return Err(PlanError::NothingToInstallFrom {
                package: key.clone(),
            });
        };
        let already_installed = input.installed.get(key.as_str()).is_some_and(|entry| {
            entry.sha256 == cached.sha256 && entry.state == crate::ledger::EntryState::Installed
        });
        if !already_installed {
            install.push(key.clone());
        }
    }
    Ok(install)
}

/// A requirement counts only when selected: anything installed but unselected
/// is uninstalled by this same plan.
fn check_requirements(
    input: &PlanInput,
    selected: &std::collections::BTreeSet<&str>,
) -> Result<(), PlanError> {
    for key in input.selection {
        let Some(cached) = input.available.get(key.as_str()) else {
            continue;
        };
        for reference in &cached.requires {
            let needed = format!("{}/{}", input.game, reference);
            if !selected.contains(needed.as_str()) {
                return Err(PlanError::MissingRequirement {
                    package: key.clone(),
                    requires: needed,
                });
            }
        }
    }
    Ok(())
}

/// Whether installing `incoming` means `existing` goes: the same
/// `exclusive_group`, or the same choice category, read from the keys so it
/// works offline.
fn supersedes(
    incoming_key: &str,
    incoming: &crate::tree::CachedPackage,
    existing_key: &str,
    existing: &crate::ledger::LedgerEntry,
    choice_categories: &[&str],
) -> bool {
    if let (Some(incoming_group), Some(existing_group)) =
        (&incoming.exclusive_group, &existing.exclusive_group)
    {
        if incoming_group == existing_group {
            return true;
        }
    }
    match (
        pacenotes_shared::category_of_download_path(incoming_key),
        pacenotes_shared::category_of_download_path(existing_key),
    ) {
        (Some(incoming_category), Some(existing_category)) => {
            incoming_category == existing_category && choice_categories.contains(&incoming_category)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
