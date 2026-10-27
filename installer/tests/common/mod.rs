//! A scratch game folder beside a scratch cache, holding real archives, for the
//! scenarios that drive plan, pre-flight and execute end to end.
#![allow(dead_code)]

pub const RBR: &str = pacenotes_shared::GAME_RBR;
pub const CHOICES: [&str; 3] = [
    pacenotes_shared::CATEGORY_LANGUAGE,
    pacenotes_shared::CATEGORY_CODRIVER,
    pacenotes_shared::CATEGORY_STYLE,
];
pub const KAMIL: &str = "rbr/public/codriver/polish/male/Kamil";
pub const EWA: &str = "rbr/public/codriver/polish/female/Ewa";

/// A scratch root holding a game folder and a cache, deleted on drop.
pub struct World {
    pub root: std::path::PathBuf,
}

impl World {
    pub fn new(label: &str) -> World {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("pacenotes-{label}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(root.join("game")).expect("create game dir");
        std::fs::create_dir_all(root.join("cache")).expect("create cache dir");
        World { root }
    }

    pub fn game(&self) -> std::path::PathBuf {
        self.root.join("game")
    }

    pub fn cache(&self) -> pacenotes_installer::cache::Cache {
        pacenotes_installer::cache::Cache::open(self.root.join("cache"))
    }

    pub fn payload(&self, sha: &str) -> std::path::PathBuf {
        self.root.join("cache").join(format!("{sha}.zip"))
    }

    /// Version 1.0.0 of `key`; see `publish_version`.
    pub fn publish(&self, key: &str, files: &[(&str, &[u8])]) -> String {
        self.publish_version(key, "1.0.0", files)
    }

    /// Stores an archive of `files` under its digest and points the index at it, as
    /// a download would; the index keeps only the newest version, the payloads all.
    pub fn publish_version(&self, key: &str, version: &str, files: &[(&str, &[u8])]) -> String {
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
                version: version.into(),
                display_name: None,
                exclusive_group: None,
                requires: Vec::new(),
                size: zip.len() as u64,
                updated_at: 1,
            })
            .expect("index it");
        sha
    }

    pub fn path(&self, relative: &str) -> std::path::PathBuf {
        let mut path = self.game();
        for segment in relative.split('/') {
            path.push(segment);
        }
        path
    }

    pub fn write_game_file(&self, relative: &str, bytes: &[u8]) {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
        std::fs::write(path, bytes).expect("write");
    }

    pub fn read_game_file(&self, relative: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(relative)).ok()
    }

    pub fn ledger(&self) -> pacenotes_installer::ledger::Ledger {
        pacenotes_installer::ledger::Ledger::open(self.game())
    }

    pub fn restore_points(&self) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(self.game().join(".pacenotes").join("restore"))
            .expect("the restore directory")
            .map(|point| point.expect("readable").path())
            .collect()
    }

    pub fn apply(&self, selection: &[&str]) -> pacenotes_installer::engine::Outcome {
        self.run(&self.plan_for(selection))
    }

    pub fn plan_for(&self, selection: &[&str]) -> pacenotes_installer::engine::Plan {
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

    pub fn preflight(
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

    pub fn run(
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
