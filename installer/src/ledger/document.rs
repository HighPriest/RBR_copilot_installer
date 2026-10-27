/// Bumped when the shape changes in a way an older installer must not read.
const LEDGER_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(super) struct Document {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    pub(super) packages: std::collections::BTreeMap<String, super::LedgerEntry>,
}

/// How `installed.json` was found; only `Missing` means the folder is untouched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LedgerState {
    Missing,
    /// Unparseable, or written by a newer build: read as empty, never written over.
    Unreadable,
    Loaded,
}

pub struct Ledger {
    dir: std::path::PathBuf,
    pub(super) document: Document,
    state: LedgerState,
}

impl Ledger {
    pub fn open(game_dir: impl AsRef<std::path::Path>) -> Ledger {
        let dir = game_dir.as_ref().join(super::STATE_DIR);
        let (document, state) = match std::fs::read(dir.join(super::LEDGER_FILE)) {
            Err(_) => (Document::default(), LedgerState::Missing),
            Ok(bytes) => match serde_json::from_slice::<Document>(&bytes) {
                Ok(document) if document.version <= LEDGER_VERSION => {
                    (document, LedgerState::Loaded)
                }
                Ok(_) | Err(_) => (Document::default(), LedgerState::Unreadable),
            },
        };
        Ledger {
            dir,
            document,
            state,
        }
    }

    pub fn state(&self) -> LedgerState {
        self.state
    }

    /// `.pacenotes` inside the game folder.
    pub fn state_dir(&self) -> &std::path::Path {
        &self.dir
    }

    pub fn put(&mut self, key: &str, entry: super::LedgerEntry) -> std::io::Result<()> {
        self.document.packages.insert(key.to_string(), entry);
        self.save()
    }

    /// Separate from `put`, so a write-ahead entry and a finished one cannot be confused.
    pub fn mark_installed(&mut self, key: &str) -> std::io::Result<()> {
        if let Some(entry) = self.document.packages.get_mut(key) {
            entry.state = super::EntryState::Installed;
        }
        self.save()
    }

    pub fn remove(&mut self, key: &str) -> std::io::Result<()> {
        self.document.packages.remove(key);
        self.save()
    }

    /// What restoring a point does.
    pub fn replace_all(
        &mut self,
        packages: std::collections::BTreeMap<String, super::LedgerEntry>,
    ) -> std::io::Result<()> {
        self.document.packages = packages;
        self.save()
    }

    /// The packages alone, as `replace_all` takes them back.
    pub fn save_snapshot(&self, path: &std::path::Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.document.packages)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// What `save_snapshot` wrote, or `None` when it is missing or unreadable.
    pub fn read_snapshot(
        path: &std::path::Path,
    ) -> Option<std::collections::BTreeMap<String, super::LedgerEntry>> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// After each package, as a temp file renamed over the old one, so a run
    /// that dies leaves a ledger that still describes the disk. Refuses to
    /// replace a ledger it could not read, which may record a newer build's packages.
    pub fn save(&mut self) -> std::io::Result<()> {
        if self.state == LedgerState::Unreadable {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "this game folder has an installation record this version cannot read; \
                 it was probably written by a newer installer",
            ));
        }
        std::fs::create_dir_all(&self.dir)?;
        self.document.version = LEDGER_VERSION;
        let json = serde_json::to_vec_pretty(&self.document)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let temp = self.dir.join(format!("{}.tmp", super::LEDGER_FILE));
        std::fs::write(&temp, &json)?;
        std::fs::rename(&temp, self.dir.join(super::LEDGER_FILE))?;
        self.state = LedgerState::Loaded;
        Ok(())
    }
}
