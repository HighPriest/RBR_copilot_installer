//! A selection applied to a real game folder: each case builds a cache holding
//! a real archive and runs plan, pre-flight and execute over a scratch directory,
//! with the facts only Win32 can measure injected.

const RBR: &str = pacenotes_shared::GAME_RBR;
const CHOICES: [&str; 3] = [
    pacenotes_shared::CATEGORY_LANGUAGE,
    pacenotes_shared::CATEGORY_CODRIVER,
    pacenotes_shared::CATEGORY_STYLE,
];
const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
const EWA: &str = "rbr/public/codriver/polish/female/Ewa";
const INI: &str = "Plugins/Pacenote/PaceNote.ini";

/// A scratch root holding a game folder and a cache, deleted on drop.
struct World {
    root: std::path::PathBuf,
}

impl World {
    fn new(label: &str) -> World {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pacenotes-engine-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("game")).expect("create game dir");
        std::fs::create_dir_all(root.join("cache")).expect("create cache dir");
        World { root }
    }

    fn game(&self) -> std::path::PathBuf {
        self.root.join("game")
    }

    fn cache(&self) -> pacenotes_installer::cache::Cache {
        pacenotes_installer::cache::Cache::open(self.root.join("cache"))
    }

    fn payload(&self, sha: &str) -> std::path::PathBuf {
        self.root.join("cache").join(format!("{sha}.zip"))
    }

    /// Stores an archive of `files` under its digest and indexes it, as a download would.
    fn publish(&self, key: &str, files: &[(&str, &[u8])]) -> String {
        let zip = build_zip(files);
        let sha = pacenotes_installer::sha256::hex_digest(&zip);
        std::fs::write(self.payload(&sha), &zip).expect("write payload");

        let (game, directory, name) =
            pacenotes_shared::split_download_path(key).expect("a well-formed key");
        self.cache()
            .insert(pacenotes_installer::tree::CachedPackage {
                game: game.into(),
                directory: directory.into(),
                name: name.into(),
                sha256: sha.clone(),
                version: "1.0.0".into(),
                display_name: None,
                exclusive_group: None,
                requires: Vec::new(),
                size: zip.len() as u64,
                updated_at: 1,
            })
            .expect("index it");
        sha
    }

    fn path(&self, relative: &str) -> std::path::PathBuf {
        let mut path = self.game();
        for segment in relative.split('/') {
            path.push(segment);
        }
        path
    }

    fn write_game_file(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
        std::fs::write(path, bytes).expect("write");
    }

    fn read_game_file(&self, relative: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(relative)).ok()
    }

    fn ledger(&self) -> pacenotes_installer::ledger::Ledger {
        pacenotes_installer::ledger::Ledger::open(self.game())
    }

    fn restore_points(&self) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(self.game().join(".pacenotes").join("restore"))
            .expect("the restore directory")
            .map(|point| point.expect("readable").path())
            .collect()
    }

    fn apply(&self, selection: &[&str]) -> pacenotes_installer::engine::Outcome {
        self.run(&self.plan_for(selection))
    }

    fn plan_for(&self, selection: &[&str]) -> pacenotes_installer::engine::Plan {
        let selection: Vec<String> = selection.iter().map(|key| key.to_string()).collect();
        let available = self
            .cache()
            .entries()
            .into_iter()
            .map(|entry| (entry.key(), entry))
            .collect();
        pacenotes_installer::engine::plan(&pacenotes_installer::engine::PlanInput {
            game: RBR,
            selection: &selection,
            choice_categories: &CHOICES,
            installed: self.ledger().packages(),
            available: &available,
        })
        .expect("a plan")
    }

    fn preflight(
        &self,
        plan: &pacenotes_installer::engine::Plan,
        facts: pacenotes_installer::engine::HostFacts,
    ) -> Result<pacenotes_installer::engine::Preflight, pacenotes_installer::engine::PreflightError>
    {
        pacenotes_installer::engine::preflight(
            plan,
            &self.game(),
            &self.cache(),
            facts,
            &mut pacenotes_installer::engine::Silent,
        )
    }

    fn run(
        &self,
        plan: &pacenotes_installer::engine::Plan,
    ) -> pacenotes_installer::engine::Outcome {
        let prepared = self
            .preflight(plan, pacenotes_installer::engine::HostFacts::default())
            .expect("pre-flight");
        pacenotes_installer::engine::execute(
            plan,
            &prepared,
            &self.game(),
            &self.cache(),
            &mut self.ledger(),
            &mut pacenotes_installer::engine::Silent,
        )
        .expect("execute")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A minimal STORED zip, built by hand to spare a writer dependency.
fn build_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        // Version, flags, method 0 (stored), time, date.
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        directory.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        // Extra, comment, disk, internal attributes.
        directory.extend_from_slice(&[0; 8]);
        directory.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let directory_at = out.len() as u32;
    let directory_len = directory.len() as u32;
    out.extend_from_slice(&directory);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    // This disk, and the disk holding the directory.
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&directory_len.to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment length
    out
}

/// Independent of the reader's own, so a broken CRC there cannot agree with itself.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn prior_of(world: &World, key: &str, path: &str) -> pacenotes_installer::ledger::Prior {
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
    let world = World::new("install");
    world.publish(
        KAMIL,
        &[
            (INI, b"sounds=polish/male/Kamil\n"),
            ("Audio/Go.wav", b"wav"),
        ],
    );

    let outcome = world.apply(&[KAMIL]);
    assert_eq!(outcome.installed, vec![KAMIL.to_string()]);
    assert_eq!(
        world.read_game_file(INI).as_deref(),
        Some(&b"sounds=polish/male/Kamil\n"[..])
    );

    let ledger = world.ledger();
    let entry = ledger.get(KAMIL).expect("ledgered");
    assert_eq!(
        entry.state,
        pacenotes_installer::ledger::EntryState::Installed
    );
    assert_eq!(entry.files.len(), 2);
    assert!(ledger.owns(INI));
    assert_eq!(
        prior_of(&world, KAMIL, INI),
        pacenotes_installer::ledger::Prior::Absent
    );
}

/// An empty tree left behind would make the folder look installed still.
#[test]
fn uninstalling_removes_what_it_added_and_the_directories_it_emptied() {
    let world = World::new("uninstall");
    world.publish(KAMIL, &[(INI, b"ours"), ("Audio/Go.wav", b"wav")]);
    world.apply(&[KAMIL]);

    let outcome = world.apply(&[]);
    assert_eq!(outcome.uninstalled, vec![KAMIL.to_string()]);
    assert!(world.read_game_file(INI).is_none());
    assert!(!world.path("Plugins").exists());
    assert!(world.ledger().is_empty());
}

#[test]
fn a_file_we_replaced_is_put_back_on_uninstall() {
    let world = World::new("restore");
    world.write_game_file(INI, b"the user's own settings");
    world.publish(KAMIL, &[(INI, b"ours")]);

    world.apply(&[KAMIL]);
    assert_eq!(world.read_game_file(INI).as_deref(), Some(&b"ours"[..]));
    assert!(matches!(
        prior_of(&world, KAMIL, INI),
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
    let world = World::new("identical");
    world.write_game_file(INI, b"exactly what the package ships");
    world.publish(KAMIL, &[(INI, b"exactly what the package ships")]);

    world.apply(&[KAMIL]);
    assert_eq!(
        prior_of(&world, KAMIL, INI),
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

#[test]
fn a_file_changed_since_we_installed_it_is_left_alone() {
    let world = World::new("changed");
    world.publish(KAMIL, &[(INI, b"ours"), ("Audio/Go.wav", b"wav")]);
    world.apply(&[KAMIL]);

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
    let world = World::new("shared");
    world.publish(KAMIL, &[(INI, b"shared")]);
    world.publish(EWA, &[(INI, b"shared")]);

    world.apply(&[KAMIL, EWA]);
    assert_eq!(world.ledger().owners_of(INI).len(), 2);

    world.apply(&[EWA]);
    assert!(world.read_game_file(INI).is_some(), "Ewa still owns it");
    world.apply(&[]);
    assert!(world.read_game_file(INI).is_none(), "nobody owns it");
}

/// The uninstall runs first, so it cannot delete what the new pick writes.
#[test]
fn switching_a_pick_leaves_the_new_files_in_place() {
    let world = World::new("switch");
    world.publish(KAMIL, &[(INI, b"kamil")]);
    world.publish(EWA, &[(INI, b"ewa")]);
    world.apply(&[KAMIL]);

    let plan = world.plan_for(&[EWA]);
    assert_eq!(
        plan.actions,
        vec![
            pacenotes_installer::engine::Action::Uninstall(KAMIL.into()),
            pacenotes_installer::engine::Action::Install(EWA.into()),
        ]
    );
    world.run(&plan);

    assert_eq!(world.read_game_file(INI).as_deref(), Some(&b"ewa"[..]));
}

/// The half-written files are ours, so none of them may be saved as the user's original.
#[test]
fn a_crashed_install_is_cleaned_up_rather_than_mistaken_for_the_users() {
    let world = World::new("crash");
    world.publish(KAMIL, &[(INI, b"ours")]);
    world.apply(&[KAMIL]);

    let mut ledger = world.ledger();
    let mut entry = ledger.get(KAMIL).cloned().expect("ledgered");
    entry.state = pacenotes_installer::ledger::EntryState::Installing;
    ledger.put(KAMIL, entry).expect("simulate the crash");

    world.apply(&[KAMIL]);

    assert_eq!(
        world.ledger().get(KAMIL).expect("installed again").state,
        pacenotes_installer::ledger::EntryState::Installed
    );
    assert_eq!(
        prior_of(&world, KAMIL, INI),
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
    let world = World::new("point");
    world.publish(KAMIL, &[(INI, b"ours")]);
    let snapshot_of = |point: &str| {
        let dir = world.game().join(".pacenotes").join("restore").join(point);
        std::fs::read_to_string(dir.join("ledger.json")).expect("a ledger snapshot")
    };

    let first = world.apply(&[KAMIL]).restore_point.expect("a point");
    assert_eq!(snapshot_of(&first).trim(), "{}");

    world.publish(EWA, &[("Audio/Ewa.wav", b"ewa")]);
    let second = world.apply(&[KAMIL, EWA]).restore_point.expect("a point");
    let snapshot = snapshot_of(&second);
    assert!(snapshot.contains(KAMIL));
    assert!(!snapshot.contains(EWA));
}

#[test]
fn a_plan_with_no_actions_leaves_no_restore_point() {
    let world = World::new("noop");
    world.publish(KAMIL, &[(INI, b"ours")]);
    world.apply(&[KAMIL]);
    let before = world.restore_points().len();

    let outcome = world.apply(&[KAMIL]);
    assert!(outcome.restore_point.is_none());
    assert_eq!(world.restore_points().len(), before);
}

#[test]
fn a_corrupt_payload_is_caught_before_anything_is_touched() {
    let world = World::new("corrupt");
    let sha = world.publish(KAMIL, &[(INI, b"ours")]);
    let mut bytes = std::fs::read(world.payload(&sha)).expect("read payload");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xFF;
    std::fs::write(world.payload(&sha), &bytes).expect("rot the payload");

    let error = world
        .preflight(
            &world.plan_for(&[KAMIL]),
            pacenotes_installer::engine::HostFacts::default(),
        )
        .expect_err("the payload no longer hashes to its name");
    assert_eq!(
        error,
        pacenotes_installer::engine::PreflightError::PayloadCorrupt {
            package: KAMIL.into()
        }
    );
    assert!(world.read_game_file(INI).is_none());
}

#[test]
fn not_enough_space_stops_the_run_before_it_starts() {
    let world = World::new("space");
    world.publish(KAMIL, &[(INI, &[b'x'; 4096])]);

    let error = world
        .preflight(
            &world.plan_for(&[KAMIL]),
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
    let world = World::new("running");
    world.publish(KAMIL, &[(INI, b"ours")]);

    let error = world
        .preflight(
            &world.plan_for(&[KAMIL]),
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
fn record_configured(world: &World, path: &str) -> pacenotes_installer::ledger::Ledger {
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
    let world = World::new("configured");
    world.publish(KAMIL, &[(INI, b"[SETTINGS]\nlanguage=polish\n")]);
    world.apply(&[KAMIL]);

    world.write_game_file(INI, b"[SETTINGS]\nlanguage=english\n");
    record_configured(&world, INI);

    let outcome = world.apply(&[]);
    assert!(outcome.left_alone.is_empty(), "{:?}", outcome.left_alone);
    assert!(world.read_game_file(INI).is_none());
}

/// The step cannot know whether the user had the file first.
#[test]
fn a_configured_file_no_package_owns_is_not_adopted() {
    let world = World::new("orphan-config");
    world.publish(KAMIL, &[("Audio/Go.wav", b"wav")]);
    world.apply(&[KAMIL]);

    world.write_game_file(INI, b"written by the configuration step");
    assert!(!record_configured(&world, INI).owns(INI));

    world.apply(&[]);
    assert!(world.read_game_file(INI).is_some());
}
