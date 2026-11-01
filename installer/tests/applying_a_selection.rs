//! A selection applied to a real game folder: each case builds a cache holding
//! a real archive and runs plan, pre-flight and execute over a scratch directory,
//! with the facts only Win32 can measure injected.

mod common;

const INI: &str = "Plugins/Pacenote/PaceNote.ini";

fn prior_of(world: &common::World, key: &str, path: &str) -> pacenotes_installer::ledger::Prior {
    let ledger = world.ledger();
    let entry = ledger.get(key).expect("ledgered");
    entry.files[&pacenotes_installer::ledger::fold(path)]
        .prior
        .clone()
}

fn files_copied_into(point: &std::path::Path) -> usize {
    std::fs::read_dir(point.join("files")).map_or(0, |files| files.count())
}

#[test]
fn installing_puts_the_files_down_and_records_them() {
    let world = common::World::new("install");
    world.publish(
        common::KAMIL,
        &[
            (INI, b"sounds=polish/male/Kamil\n"),
            ("Audio/Go.wav", b"wav"),
        ],
    );

    let outcome = world.apply(&[common::KAMIL]);
    assert_eq!(outcome.installed, vec![common::KAMIL.to_string()]);
    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"sounds=polish/male/Kamil\n"[..])
    );

    let ledger = world.ledger();
    let entry = ledger.get(common::KAMIL).expect("ledgered");
    assert_eq!(
        entry.state,
        pacenotes_installer::ledger::EntryState::Installed
    );
    assert_eq!(entry.files.len(), 2);
    assert!(ledger.owns(INI));
    assert_eq!(
        prior_of(&world, common::KAMIL, INI),
        pacenotes_installer::ledger::Prior::Absent
    );
}

/// An empty tree left behind would make the folder look installed still.
#[test]
fn uninstalling_removes_what_it_added_and_the_directories_it_emptied() {
    let world = common::World::new("uninstall");
    world.publish(common::KAMIL, &[(INI, b"ours"), ("Audio/Go.wav", b"wav")]);
    world.apply(&[common::KAMIL]);

    let outcome = world.apply(&[]);
    assert_eq!(outcome.uninstalled, vec![common::KAMIL.to_string()]);
    assert!(world.read_game_file(INI).is_none());
    assert!(!world.path("Plugins").exists());
    assert!(world.ledger().is_empty());
}

#[test]
fn a_file_we_replaced_is_put_back_on_uninstall() {
    let world = common::World::new("restore");
    world.write_game_file(INI, b"the user's own settings");
    world.publish(common::KAMIL, &[(INI, b"ours")]);

    world.apply(&[common::KAMIL]);
    assert_eq!(world.read_game_file(INI).as_deref(), Some(&b"ours"[..]));
    assert!(matches!(
        prior_of(&world, common::KAMIL, INI),
        pacenotes_installer::ledger::Prior::Backup { .. }
    ));

    world.apply(&[]);
    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"the user's own settings"[..])
    );
}

/// The user had these bytes before us, so uninstall restores them from the
/// cached payload instead of deleting them.
#[test]
fn a_file_that_already_matched_is_not_copied_but_is_still_put_back() {
    let world = common::World::new("identical");
    world.write_game_file(INI, b"exactly what the package ships");
    world.publish(common::KAMIL, &[(INI, b"exactly what the package ships")]);

    world.apply(&[common::KAMIL]);
    assert_eq!(
        prior_of(&world, common::KAMIL, INI),
        pacenotes_installer::ledger::Prior::Identical
    );
    let points = world.restore_points();
    assert_eq!(points.len(), 1);
    assert_eq!(files_copied_into(&points[0]), 0);

    world.apply(&[]);
    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"exactly what the package ships"[..])
    );
}

/// Rewriting a file with the bytes it already holds costs a write per file and
/// changes nothing, and on a first run over a manual install that is every file.
#[test]
fn a_file_that_already_matched_is_not_written_again() {
    let world = common::World::new("identical-untouched");
    world.write_game_file(INI, b"exactly what the package ships");
    let long_ago =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    std::fs::File::options()
        .write(true)
        .open(world.path(INI))
        .and_then(|file| file.set_modified(long_ago))
        .expect("backdate the file");
    world.publish(common::KAMIL, &[(INI, b"exactly what the package ships")]);

    world.apply(&[common::KAMIL]);
    let modified = std::fs::metadata(world.path(INI)).and_then(|meta| meta.modified());
    assert_eq!(modified.expect("read the time"), long_ago);
}

/// After an install, an identical file still holding what was written is the
/// original, so emptying the cache costs it nothing.
#[test]
fn an_identical_original_survives_an_emptied_cache() {
    let world = common::World::new("identical-no-cache");
    world.write_game_file(INI, b"exactly what the package ships");
    let sha = world.publish(common::KAMIL, &[(INI, b"exactly what the package ships")]);
    world.apply(&[common::KAMIL]);

    std::fs::remove_file(world.payload(&sha)).expect("empty the cache");
    let outcome = world.apply(&[]);
    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"exactly what the package ships"[..])
    );
    assert!(outcome.left_alone.is_empty(), "{:?}", outcome.left_alone);
}

#[test]
fn a_file_changed_since_we_installed_it_is_left_alone() {
    let world = common::World::new("changed");
    world.publish(common::KAMIL, &[(INI, b"ours"), ("Audio/Go.wav", b"wav")]);
    world.apply(&[common::KAMIL]);

    world.write_game_file(INI, b"edited by the RSF installer");
    let outcome = world.apply(&[]);

    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"edited by the RSF installer"[..])
    );
    assert_eq!(outcome.left_alone, vec![INI.to_string()]);
    assert!(world.read_game_file("Audio/Go.wav").is_none());
}

#[test]
fn a_file_two_packages_own_outlives_the_first_uninstall() {
    let world = common::World::new("shared");
    world.publish(common::KAMIL, &[(INI, b"shared")]);
    world.publish(common::EWA, &[(INI, b"shared")]);

    world.apply(&[common::KAMIL, common::EWA]);
    assert_eq!(world.ledger().owners_of(INI).len(), 2);

    world.apply(&[common::EWA]);
    assert!(world.read_game_file(INI).is_some(), "Ewa still owns it");
    world.apply(&[]);
    assert!(world.read_game_file(INI).is_none(), "nobody owns it");
}

/// The uninstall runs first, so it cannot delete what the new pick writes.
#[test]
fn switching_a_pick_leaves_the_new_files_in_place() {
    let world = common::World::new("switch");
    world.publish(common::KAMIL, &[(INI, b"kamil")]);
    world.publish(common::EWA, &[(INI, b"ewa")]);
    world.apply(&[common::KAMIL]);

    let plan = world.plan_for(&[common::EWA]);
    assert_eq!(
        plan.actions,
        vec![
            pacenotes_installer::engine::Action::Uninstall(common::KAMIL.into()),
            pacenotes_installer::engine::Action::Install(common::EWA.into()),
        ]
    );
    world.run(&plan);

    assert_eq!(world.read_game_file(INI).as_deref(), Some(&b"ewa"[..]));
}

/// The half-written files are ours, so none of them may be saved as the user's original.
#[test]
fn a_crashed_install_is_cleaned_up_rather_than_mistaken_for_the_users() {
    let world = common::World::new("crash");
    world.publish(common::KAMIL, &[(INI, b"ours")]);
    world.apply(&[common::KAMIL]);

    let mut ledger = world.ledger();
    let mut entry = ledger.get(common::KAMIL).cloned().expect("ledgered");
    entry.state = pacenotes_installer::ledger::EntryState::Installing;
    ledger
        .put(common::KAMIL, entry)
        .expect("simulate the crash");

    world.apply(&[common::KAMIL]);

    assert_eq!(
        world
            .ledger()
            .get(common::KAMIL)
            .expect("installed again")
            .state,
        pacenotes_installer::ledger::EntryState::Installed
    );
    assert_eq!(
        prior_of(&world, common::KAMIL, INI),
        pacenotes_installer::ledger::Prior::Absent
    );
    let copied: usize = world
        .restore_points()
        .iter()
        .map(|point| files_copied_into(point))
        .sum();
    assert_eq!(copied, 0);
}

/// Each snapshot is the ledger from BEFORE its run, which is what lets a
/// restore remove what was installed afterwards.
#[test]
fn a_run_that_changes_anything_leaves_a_restore_point() {
    let world = common::World::new("point");
    world.publish(common::KAMIL, &[(INI, b"ours")]);
    let snapshot_of = |point: &str| {
        let dir = world.game().join(".pacenotes").join("restore").join(point);
        std::fs::read_to_string(dir.join("ledger.json")).expect("a ledger snapshot")
    };

    let first = world
        .apply(&[common::KAMIL])
        .restore_point
        .expect("a point");
    assert_eq!(snapshot_of(&first).trim(), "{}");

    world.publish(common::EWA, &[("Audio/Ewa.wav", b"ewa")]);
    let second = world
        .apply(&[common::KAMIL, common::EWA])
        .restore_point
        .expect("a point");
    let snapshot = snapshot_of(&second);
    assert!(snapshot.contains(common::KAMIL));
    assert!(!snapshot.contains(common::EWA));
}

#[test]
fn a_plan_with_no_actions_leaves_no_restore_point() {
    let world = common::World::new("noop");
    world.publish(common::KAMIL, &[(INI, b"ours")]);
    world.apply(&[common::KAMIL]);
    let before = world.restore_points().len();

    let outcome = world.apply(&[common::KAMIL]);
    assert!(outcome.restore_point.is_none());
    assert_eq!(world.restore_points().len(), before);
}

#[test]
fn a_corrupt_payload_is_caught_before_anything_is_touched() {
    let world = common::World::new("corrupt");
    let sha = world.publish(common::KAMIL, &[(INI, b"ours")]);
    let mut bytes = std::fs::read(world.payload(&sha)).expect("read payload");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xFF;
    std::fs::write(world.payload(&sha), &bytes).expect("rot the payload");

    let error = world
        .preflight(
            &world.plan_for(&[common::KAMIL]),
            pacenotes_installer::engine::HostFacts::default(),
        )
        .expect_err("the payload no longer hashes to its name");
    assert_eq!(
        error,
        pacenotes_installer::engine::PreflightError::PayloadCorrupt {
            package: common::KAMIL.into()
        }
    );
    assert!(world.read_game_file(INI).is_none());
}

#[test]
fn not_enough_space_stops_the_run_before_it_starts() {
    let world = common::World::new("space");
    world.publish(common::KAMIL, &[(INI, &[b'x'; 4096])]);

    let error = world
        .preflight(
            &world.plan_for(&[common::KAMIL]),
            pacenotes_installer::engine::HostFacts {
                free_bytes: Some(1024),
                game_running: false,
            },
        )
        .expect_err("1 kB is not enough for 4 kB plus its backups");
    assert!(matches!(
        error,
        pacenotes_installer::engine::PreflightError::NotEnoughSpace { .. }
    ));
    assert!(error.to_string().contains("kB"), "{error}");
}

#[test]
fn a_running_game_stops_the_run() {
    let world = common::World::new("running");
    world.publish(common::KAMIL, &[(INI, b"ours")]);

    let error = world
        .preflight(
            &world.plan_for(&[common::KAMIL]),
            pacenotes_installer::engine::HostFacts {
                free_bytes: None,
                game_running: true,
            },
        )
        .expect_err("the game is open");
    assert_eq!(
        error,
        pacenotes_installer::engine::PreflightError::GameRunning
    );
}

/// Records what the per-game step would, after editing an installed file.
fn record_configured(world: &common::World, path: &str) -> pacenotes_installer::ledger::Ledger {
    let mut ledger = world.ledger();
    pacenotes_installer::engine::record_configured(
        &world.game(),
        &[path.to_string()],
        &mut ledger,
        &mut pacenotes_installer::engine::Silent,
    )
    .expect("record");
    ledger
}

/// Without the new hash, uninstall would take our own edit for someone else's.
#[test]
fn a_file_the_configuration_step_edited_is_still_ours() {
    let world = common::World::new("configured");
    world.publish(common::KAMIL, &[(INI, b"[SETTINGS]\nlanguage=polish\n")]);
    world.apply(&[common::KAMIL]);

    world.write_game_file(INI, b"[SETTINGS]\nlanguage=english\n");
    record_configured(&world, INI);

    let outcome = world.apply(&[]);
    assert!(outcome.left_alone.is_empty(), "{:?}", outcome.left_alone);
    assert!(world.read_game_file(INI).is_none());
}

/// The step cannot know whether the user had the file first.
#[test]
fn a_configured_file_no_package_owns_is_not_adopted() {
    let world = common::World::new("orphan-config");
    world.publish(common::KAMIL, &[("Audio/Go.wav", b"wav")]);
    world.apply(&[common::KAMIL]);

    world.write_game_file(INI, b"written by the configuration step");
    assert!(!record_configured(&world, INI).owns(INI));

    world.apply(&[]);
    assert!(world.read_game_file(INI).is_some());
}
