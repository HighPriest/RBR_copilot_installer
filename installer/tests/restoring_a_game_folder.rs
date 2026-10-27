//! A game folder taken back to a restore point: a user had a working install, a
//! run replaced a package with a broken version, and they want the working one
//! back. The assertions go past "the bytes are back" to what a careless restore
//! destroys, the user's own files and the record of which files were theirs.

mod common;

const VOICE: &str = "Plugins/Pacenote/sounds/Kamil.ogg";
const EWA_VOICE: &str = "Plugins/Pacenote/sounds/Ewa.ogg";
/// A file the user already had, which the package replaces.
const THEIRS: &str = "Plugins/Pacenote/PaceNote.ini";

/// Plans and runs a restore to `target`, as the restore scene does.
fn restore(
    world: &common::World,
    target: &pacenotes_installer::restore::Snapshot,
) -> (
    pacenotes_installer::restore::Plan,
    pacenotes_installer::engine::Outcome,
) {
    let cache = world.cache();
    let mut ledger = world.ledger();
    let plan = pacenotes_installer::restore::plan(target, ledger.packages(), &|sha256| {
        cache.has_payload(sha256)
    });
    let prepared = pacenotes_installer::restore::prepare(
        &plan,
        &cache,
        pacenotes_installer::engine::HostFacts::default(),
        &world.game(),
        &mut pacenotes_installer::engine::Silent,
    )
    .expect("pre-flight");
    let outcome = pacenotes_installer::restore::execute(
        target,
        &plan,
        &prepared,
        &world.game(),
        &cache,
        &mut ledger,
        &mut pacenotes_installer::engine::Silent,
    )
    .expect("restore");
    (plan, outcome)
}

fn restore_to_point(
    world: &common::World,
    point: &str,
) -> (
    pacenotes_installer::restore::Plan,
    pacenotes_installer::engine::Outcome,
) {
    let target = pacenotes_installer::restore::read(&world.game(), point)
        .expect("the point is readable")
        .snapshot;
    restore(world, &target)
}

#[test]
fn a_bad_update_is_undone_down_to_the_bytes() {
    let world = common::World::new("rollback");
    world.publish_version(common::KAMIL, "2.1.0", &[(VOICE, b"good audio")]);
    world.apply(&[common::KAMIL]);
    world.publish_version(common::KAMIL, "3.0.0", &[(VOICE, b"BROKEN")]);
    let update = world.apply(&[common::KAMIL]);
    let point = update.restore_point.expect("an update takes a point");
    assert_eq!(world.read_game_file(VOICE).as_deref(), Some(&b"BROKEN"[..]));

    let (plan, outcome) = restore_to_point(&world, &point);

    assert_eq!(plan.put_back.len(), 1);
    assert!(plan.unavailable.is_empty());
    assert_eq!(
        world.read_game_file(VOICE).as_deref(),
        Some(&b"good audio"[..]),
        "the working version is back on disk"
    );
    assert_eq!(
        world
            .ledger()
            .get(common::KAMIL)
            .expect("installed")
            .version,
        "2.1.0",
        "and the ledger agrees with the disk"
    );
    assert!(
        outcome.restore_point.is_some(),
        "a restore is itself undoable"
    );
}

/// Otherwise the user keeps the thing they are trying to escape, which is the
/// bug the legacy installer had.
#[test]
fn what_was_installed_after_the_point_is_removed() {
    let world = common::World::new("removes");
    world.publish(common::KAMIL, &[(VOICE, b"kamil")]);
    let first = world.apply(&[common::KAMIL]);
    let point = first.restore_point.expect("the first run takes a point");
    world.publish(common::EWA, &[(EWA_VOICE, b"ewa")]);
    world.apply(&[common::KAMIL, common::EWA]);

    restore_to_point(&world, &point);

    assert!(
        world.read_game_file(EWA_VOICE).is_none(),
        "a package added after the point must not survive it"
    );
    assert!(world.read_game_file(VOICE).is_none());
    assert!(world.ledger().packages().is_empty());
}

/// Uninstalling after a restore gives the user their own file back only if the
/// restore kept the record of whose file it was.
#[test]
fn a_restore_keeps_the_users_own_file_recoverable() {
    let world = common::World::new("provenance");
    world.write_game_file(THEIRS, b"the user's own settings");
    world.publish_version(common::KAMIL, "2.1.0", &[(THEIRS, b"shipped by 2.1.0")]);
    world.apply(&[common::KAMIL]);
    world.publish_version(common::KAMIL, "3.0.0", &[(THEIRS, b"shipped by 3.0.0")]);
    let update = world.apply(&[common::KAMIL]);
    let point = update.restore_point.expect("a point");

    restore_to_point(&world, &point);
    assert_eq!(
        world.read_game_file(THEIRS).as_deref(),
        Some(&b"shipped by 2.1.0"[..]),
        "2.1.0's file is back"
    );
    let ledger = world.ledger();
    let file = ledger
        .get(common::KAMIL)
        .expect("installed")
        .files
        .values()
        .find(|file| file.path == THEIRS)
        .expect("the ini is recorded");
    assert!(
        matches!(
            file.prior,
            pacenotes_installer::ledger::Prior::Backup { .. }
        ),
        "prior was {:?}",
        file.prior
    );

    world.apply(&[]);
    assert_eq!(
        world.read_game_file(THEIRS).as_deref(),
        Some(&b"the user's own settings"[..]),
        "uninstalling after a restore gives the user their own file back"
    );
}

/// "Uninstall everything" goes through the same code, with the same guarantees.
#[test]
fn restoring_to_an_empty_target_uninstalls_everything() {
    let world = common::World::new("empty");
    world.write_game_file(THEIRS, b"the user's own settings");
    world.publish(common::KAMIL, &[(THEIRS, b"shipped"), (VOICE, b"audio")]);
    world.apply(&[common::KAMIL]);

    restore(&world, &pacenotes_installer::restore::Snapshot::new());

    assert!(world.read_game_file(VOICE).is_none(), "ours is gone");
    assert_eq!(
        world.read_game_file(THEIRS).as_deref(),
        Some(&b"the user's own settings"[..]),
        "theirs came back"
    );
    assert!(world.ledger().packages().is_empty());
}

/// Removing the newer version when the old one cannot come back would leave the
/// user with nothing at all.
#[test]
fn a_missing_payload_leaves_what_is_installed_alone() {
    let world = common::World::new("missing");
    let old = world.publish_version(common::KAMIL, "2.1.0", &[(VOICE, b"good audio")]);
    world.apply(&[common::KAMIL]);
    world.publish_version(common::KAMIL, "3.0.0", &[(VOICE, b"BROKEN")]);
    let update = world.apply(&[common::KAMIL]);
    let point = update.restore_point.expect("a point");
    std::fs::remove_file(world.payload(&old)).expect("drop the old payload");

    let (plan, _) = restore_to_point(&world, &point);

    assert_eq!(plan.unavailable.len(), 1);
    assert!(plan.remove.is_empty());
    assert_eq!(
        world.read_game_file(VOICE).as_deref(),
        Some(&b"BROKEN"[..]),
        "the wrong version beats an empty folder"
    );
    assert_eq!(
        world
            .ledger()
            .get(common::KAMIL)
            .expect("installed")
            .version,
        "3.0.0",
        "and the ledger still describes what is actually there"
    );
}

/// Reusing the directory had the second run overwrite the first one's snapshot,
/// destroying the way back the first run had promised.
#[test]
fn two_runs_in_the_same_second_get_their_own_points() {
    let world = common::World::new("collide");
    world.publish(common::KAMIL, &[(VOICE, b"kamil")]);
    let first = world
        .apply(&[common::KAMIL])
        .restore_point
        .expect("a point");
    world.publish(common::EWA, &[(EWA_VOICE, b"ewa")]);
    let second = world
        .apply(&[common::KAMIL, common::EWA])
        .restore_point
        .expect("a point");

    assert_ne!(first, second, "one run must not take another's point");
    assert!(
        second.starts_with(&first),
        "the suffixed name still sorts after the plain one: {second}"
    );
    let snapshot = |point: &str| {
        pacenotes_installer::restore::read(&world.game(), point)
            .expect("readable")
            .snapshot
    };
    assert!(
        snapshot(&first).is_empty(),
        "the first point predates everything"
    );
    assert_eq!(
        snapshot(&second).keys().collect::<Vec<_>>(),
        vec![common::KAMIL],
        "the second point knows Kamil was already there"
    );
}

/// The order the scene offers them in, since a user looking for a way back
/// wants the most recent working state first.
#[test]
fn points_are_listed_newest_first() {
    let world = common::World::new("order");
    world.publish(common::KAMIL, &[(VOICE, b"kamil")]);
    let first = world
        .apply(&[common::KAMIL])
        .restore_point
        .expect("a point");
    world.publish(common::EWA, &[(EWA_VOICE, b"ewa")]);
    let second = world
        .apply(&[common::KAMIL, common::EWA])
        .restore_point
        .expect("a point");

    let listed: Vec<String> = pacenotes_installer::restore::points(&world.game())
        .into_iter()
        .map(|point| point.name)
        .collect();

    assert_eq!(listed, vec![second, first]);
}

/// The user's chosen voice has to come back with the files.
#[test]
fn the_picks_come_from_the_restored_state() {
    let world = common::World::new("picks");
    world.publish(common::KAMIL, &[(VOICE, b"kamil")]);
    world.apply(&[common::KAMIL]);

    let picks = pacenotes_installer::restore::picks(world.ledger().packages(), &common::CHOICES);

    assert_eq!(
        picks
            .get(pacenotes_shared::CATEGORY_CODRIVER)
            .map(String::as_str),
        Some(common::KAMIL)
    );
    assert!(!picks.contains_key(pacenotes_shared::CATEGORY_LANGUAGE));
}
