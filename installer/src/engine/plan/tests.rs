//! Each case is a real sequence a user puts the installer through, with the
//! live catalogue's shape: three styles writing the same paths, co-drivers two
//! levels deep, and `pacenotes/*` as no-choice extras.

const RBR: &str = pacenotes_shared::GAME_RBR;
const CHOICES: [&str; 3] = [
    pacenotes_shared::CATEGORY_LANGUAGE,
    pacenotes_shared::CATEGORY_CODRIVER,
    pacenotes_shared::CATEGORY_STYLE,
];

const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
const EWA: &str = "rbr/public/codriver/polish/female/Ewa";
const DESCRIPTIVE: &str = "rbr/public/style/Descriptive";
const NUMERIC: &str = "rbr/public/style/Numeric Standard";
const POLISH: &str = "rbr/public/language/polish";
const ENGLISH: &str = "rbr/public/language/english";
const LUPPIS: &str = "rbr/public/pacenotes/LuppisV3";

type Available = std::collections::BTreeMap<String, crate::tree::CachedPackage>;
type Installed = std::collections::BTreeMap<String, crate::ledger::LedgerEntry>;

fn cached(key: &str, sha: &str) -> crate::tree::CachedPackage {
    let (game, directory, name) =
        pacenotes_shared::split_download_path(key).expect("a well-formed key");
    crate::tree::CachedPackage {
        game: game.into(),
        directory: directory.into(),
        name: name.into(),
        sha256: sha.into(),
        version: "1.0.0".into(),
        display_name: None,
        exclusive_group: None,
        requires: Vec::new(),
        size: 100,
        updated_at: 1,
    }
}

fn available(entries: &[(&str, &str)]) -> Available {
    entries
        .iter()
        .map(|(key, sha)| (key.to_string(), cached(key, sha)))
        .collect()
}

fn installed(entries: &[(&str, &str)]) -> Installed {
    entries
        .iter()
        .map(|(key, sha)| {
            let entry = crate::ledger::LedgerEntry {
                sha256: sha.to_string(),
                version: "1.0.0".into(),
                exclusive_group: None,
                state: crate::ledger::EntryState::Installed,
                files: std::collections::BTreeMap::new(),
            };
            (key.to_string(), entry)
        })
        .collect()
}

fn try_plan(
    selection: &[&str],
    installed: &Installed,
    available: &Available,
) -> Result<super::Plan, super::PlanError> {
    let selection: Vec<String> = selection.iter().map(|key| key.to_string()).collect();
    super::plan(&super::PlanInput {
        game: RBR,
        selection: &selection,
        choice_categories: &CHOICES,
        installed,
        available,
    })
}

fn actions(selection: &[&str], installed: &Installed, available: &Available) -> Vec<super::Action> {
    try_plan(selection, installed, available)
        .expect("a plan")
        .actions
}

fn uninstall(key: &str) -> super::Action {
    super::Action::Uninstall(key.to_string())
}

fn install(key: &str) -> super::Action {
    super::Action::Install(key.to_string())
}

/// The selection's order decides who wins an overlap, so planning keeps it.
#[test]
fn a_first_install_installs_the_selection_in_order() {
    let available = available(&[(POLISH, "p1"), (KAMIL, "k1"), (DESCRIPTIVE, "d1")]);
    assert_eq!(
        actions(&[POLISH, KAMIL, DESCRIPTIVE], &installed(&[]), &available),
        vec![install(POLISH), install(KAMIL), install(DESCRIPTIVE)]
    );
}

#[test]
fn re_running_with_nothing_changed_is_a_no_op() {
    let available = available(&[(POLISH, "p1"), (KAMIL, "k1")]);
    let installed = installed(&[(POLISH, "p1"), (KAMIL, "k1")]);
    assert!(actions(&[POLISH, KAMIL], &installed, &available).is_empty());
}

#[test]
fn switching_style_uninstalls_before_it_installs() {
    let available = available(&[(DESCRIPTIVE, "d1"), (NUMERIC, "n1")]);
    let installed = installed(&[(DESCRIPTIVE, "d1")]);
    assert_eq!(
        actions(&[NUMERIC], &installed, &available),
        vec![uninstall(DESCRIPTIVE), install(NUMERIC)]
    );
}

#[test]
fn an_exclusive_group_supersedes_its_installed_member() {
    let other = "rbr/public/pacenotes/LuppisV2";
    let mut available = available(&[(LUPPIS, "l3"), (other, "l2")]);
    for key in [LUPPIS, other] {
        available.get_mut(key).unwrap().exclusive_group = Some("luppis".into());
    }
    let mut installed = installed(&[(other, "l2")]);
    installed.get_mut(other).unwrap().exclusive_group = Some("luppis".into());

    assert_eq!(
        actions(&[LUPPIS], &installed, &available),
        vec![uninstall(other), install(LUPPIS)]
    );
}

#[test]
fn a_pick_supersedes_only_within_its_own_category() {
    let available = available(&[(KAMIL, "k1"), (EWA, "e1"), (POLISH, "p1")]);
    let installed = installed(&[(KAMIL, "k1"), (POLISH, "p1")]);
    assert_eq!(
        actions(&[EWA, POLISH], &installed, &available),
        vec![uninstall(KAMIL), install(EWA)]
    );
}

#[test]
fn extras_outside_a_choice_category_coexist() {
    let second = "rbr/public/pacenotes/Monte";
    let available = available(&[(LUPPIS, "l1"), (second, "m1")]);
    let installed = installed(&[(LUPPIS, "l1")]);
    assert_eq!(
        actions(&[LUPPIS, second], &installed, &available),
        vec![install(second)]
    );
}

/// The old files go first, or uninstalling them afterwards would delete the new ones.
#[test]
fn a_changed_sha_is_a_reinstall_not_an_overwrite() {
    let available = available(&[(KAMIL, "k2")]);
    let installed = installed(&[(KAMIL, "k1")]);
    assert_eq!(
        actions(&[KAMIL], &installed, &available),
        vec![uninstall(KAMIL), install(KAMIL)]
    );
}

#[test]
fn something_installed_and_not_selected_is_uninstalled() {
    let available = available(&[(POLISH, "p1"), (KAMIL, "k1")]);
    let installed = installed(&[(POLISH, "p1"), (KAMIL, "k1")]);
    assert_eq!(
        actions(&[POLISH], &installed, &available),
        vec![uninstall(KAMIL)]
    );
}

/// A server-side withdrawal must not uninstall anything by itself.
#[test]
fn a_withdrawn_package_that_is_still_selected_is_left_alone() {
    let available = available(&[(POLISH, "p1")]);
    let installed = installed(&[(POLISH, "p1"), (KAMIL, "k1")]);

    let error = try_plan(&[POLISH, KAMIL], &installed, &available)
        .expect_err("there is nothing to install from");
    assert_eq!(
        error,
        super::PlanError::NothingToInstallFrom {
            package: KAMIL.into()
        }
    );
    assert_eq!(
        actions(&[POLISH], &installed, &available),
        vec![uninstall(KAMIL)]
    );
}

#[test]
fn an_unfinished_install_is_redone_from_scratch() {
    let available = available(&[(KAMIL, "k1")]);
    let mut installed = installed(&[(KAMIL, "k1")]);
    installed.get_mut(KAMIL).unwrap().state = crate::ledger::EntryState::Installing;
    assert_eq!(
        actions(&[KAMIL], &installed, &available),
        vec![uninstall(KAMIL), install(KAMIL)]
    );
}

#[test]
fn an_unfinished_install_that_is_no_longer_wanted_is_removed() {
    let available = available(&[(POLISH, "p1")]);
    let mut installed = installed(&[(POLISH, "p1"), (KAMIL, "k1")]);
    installed.get_mut(KAMIL).unwrap().state = crate::ledger::EntryState::Installing;
    assert_eq!(
        actions(&[POLISH], &installed, &available),
        vec![uninstall(KAMIL)]
    );
}

fn kamil_requiring_polish(entries: &[(&str, &str)]) -> Available {
    let mut available = available(entries);
    available.get_mut(KAMIL).unwrap().requires = vec!["public/language/polish".into()];
    available
}

fn missing_polish() -> super::PlanError {
    super::PlanError::MissingRequirement {
        package: KAMIL.into(),
        requires: POLISH.into(),
    }
}

#[test]
fn a_missing_requirement_refuses_the_plan() {
    let available = kamil_requiring_polish(&[(KAMIL, "k1"), (POLISH, "p1")]);
    let error = try_plan(&[KAMIL], &installed(&[]), &available)
        .expect_err("the dependency is not selected");
    assert_eq!(error, missing_polish());
    assert!(error.to_string().contains("language/polish"), "{error}");
}

/// Installed but unselected means uninstalled by the same plan.
#[test]
fn a_requirement_must_be_selected_even_when_it_is_already_installed() {
    let available = kamil_requiring_polish(&[(KAMIL, "k1"), (POLISH, "p1")]);
    let installed = installed(&[(POLISH, "p1")]);

    let error = try_plan(&[KAMIL], &installed, &available)
        .expect_err("unselecting polish would uninstall it");
    assert_eq!(error, missing_polish());
    assert_eq!(
        actions(&[POLISH, KAMIL], &installed, &available),
        vec![install(KAMIL)]
    );
}

#[test]
fn a_requirement_being_uninstalled_does_not_count() {
    let available = kamil_requiring_polish(&[(KAMIL, "k1"), (POLISH, "p1"), (ENGLISH, "e1")]);
    let installed = installed(&[(POLISH, "p1"), (KAMIL, "k1")]);
    let error = try_plan(&[ENGLISH, KAMIL], &installed, &available)
        .expect_err("polish is being superseded by english");
    assert_eq!(error, missing_polish());
}

#[test]
fn a_requirement_resolves_within_this_game() {
    let available = kamil_requiring_polish(&[(KAMIL, "k1"), (POLISH, "p1")]);
    assert_eq!(
        actions(&[POLISH, KAMIL], &installed(&[]), &available).len(),
        2
    );
}

#[test]
fn a_ledger_entry_with_files_plans_the_same() {
    let available = available(&[(KAMIL, "k2")]);
    let mut installed = installed(&[(KAMIL, "k1")]);
    installed.get_mut(KAMIL).unwrap().files.insert(
        "plugins/pacenote/pacenote.ini".into(),
        crate::ledger::LedgerFile {
            path: "Plugins/Pacenote/PaceNote.ini".into(),
            sha256: "written".into(),
            prior: crate::ledger::Prior::Backup { point: "p".into() },
        },
    );
    assert_eq!(
        actions(&[KAMIL], &installed, &available),
        vec![uninstall(KAMIL), install(KAMIL)]
    );
}
