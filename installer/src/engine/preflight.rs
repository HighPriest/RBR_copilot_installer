#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreflightError {
    PayloadMissing {
        package: String,
    },
    /// The cache file rotted after it was verified at download time.
    PayloadCorrupt {
        package: String,
    },
    PayloadUnreadable {
        package: String,
        reason: String,
    },
    /// `needed` counts what is written plus the copies of what it replaces.
    NotEnoughSpace {
        needed: u64,
        available: u64,
    },
    NotWritable {
        reason: String,
    },
    /// The game holds its files open, so a write would fail partway.
    GameRunning,
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflightError::PayloadMissing { package } => {
                write!(formatter, "{package} is not in the download cache")
            }
            PreflightError::PayloadCorrupt { package } => write!(
                formatter,
                "{package} is damaged in the download cache; delete it and download it again"
            ),
            PreflightError::PayloadUnreadable { package, reason } => {
                write!(formatter, "{package} could not be read: {reason}")
            }
            PreflightError::NotEnoughSpace { needed, available } => write!(
                formatter,
                "this needs {} and the drive has {}",
                crate::size::human(*needed),
                crate::size::human(*available)
            ),
            PreflightError::NotWritable { reason } => {
                write!(formatter, "the game folder cannot be written to: {reason}")
            }
            PreflightError::GameRunning => write!(
                formatter,
                "the game is running. Close it first — its files are open, and the \
                 installation would stop partway through"
            ),
        }
    }
}

/// What the caller measures through Win32, so this module stays testable off Windows.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostFacts {
    /// Free bytes on the game folder's volume; `None` skips the space check.
    pub free_bytes: Option<u64>,
    pub game_running: bool,
}

/// What `execute` reuses, so no archive is opened twice.
#[derive(Debug)]
pub struct Preflight {
    /// Per package to install, in archive order.
    pub files: std::collections::BTreeMap<String, Vec<crate::zip::FileEntry>>,
    pub install_bytes: u64,
}

impl Preflight {
    /// In the shape `overlaps` takes.
    pub fn paths(&self) -> std::collections::BTreeMap<String, Vec<String>> {
        self.files
            .iter()
            .map(|(key, files)| {
                let paths = files.iter().map(|file| file.path.clone()).collect();
                (key.clone(), paths)
            })
            .collect()
    }
}

/// Everything that can fail is checked before the first file is touched, since
/// a failure after an uninstall leaves neither the old content nor the new.
///
/// Each payload is verified by its sha256, which reads the file once instead of
/// decompressing every entry; `zip` still checks each entry's CRC on extraction.
pub fn preflight(
    plan: &super::Plan,
    game_dir: &std::path::Path,
    cache: &crate::cache::Cache,
    facts: HostFacts,
    watcher: &mut dyn super::Watcher,
) -> Result<Preflight, PreflightError> {
    if facts.game_running {
        return Err(PreflightError::GameRunning);
    }
    probe_writable(game_dir)?;

    let mut files = std::collections::BTreeMap::new();
    let mut install_bytes = 0u64;
    let installs = plan.installs();
    for (index, key) in installs.iter().enumerate() {
        watcher.phase(
            super::Phase::Checking,
            key,
            index as u64,
            installs.len() as u64,
        );
        let missing = || PreflightError::PayloadMissing {
            package: key.to_string(),
        };
        let sha256 = &cache.get(key).ok_or_else(missing)?.sha256;
        let listed = verified_files(cache, key, sha256)?;
        install_bytes += listed.iter().map(|file| file.size).sum::<u64>();
        files.insert(key.to_string(), listed);
    }

    check_space(install_bytes, facts)?;
    Ok(Preflight {
        files,
        install_bytes,
    })
}

/// The files of the payload named by `sha256`, checked against it; by digest
/// rather than through the index, which holds only the newest version, so a
/// restore can name an older one.
pub(crate) fn verified_files(
    cache: &crate::cache::Cache,
    key: &str,
    sha256: &str,
) -> Result<Vec<crate::zip::FileEntry>, PreflightError> {
    let bytes =
        std::fs::read(cache.payload_path(sha256)).map_err(|_| PreflightError::PayloadMissing {
            package: key.to_string(),
        })?;
    if crate::sha256::hex_digest(&bytes) != sha256 {
        return Err(PreflightError::PayloadCorrupt {
            package: key.to_string(),
        });
    }
    crate::zip::entries(&bytes).map_err(|e| PreflightError::PayloadUnreadable {
        package: key.to_string(),
        reason: e.0,
    })
}

/// Room for what is written plus copies of what it replaces, doubled as if every
/// file replaced one its size, which errs on the safe side.
pub(crate) fn check_space(install_bytes: u64, facts: HostFacts) -> Result<(), PreflightError> {
    if let Some(available) = facts.free_bytes {
        let needed = install_bytes.saturating_mul(2);
        if needed > available {
            return Err(PreflightError::NotEnoughSpace { needed, available });
        }
    }
    Ok(())
}

/// Creates the state directory and a probe file in it.
pub(crate) fn probe_writable(game_dir: &std::path::Path) -> Result<(), PreflightError> {
    let not_writable = |e: std::io::Error| PreflightError::NotWritable {
        reason: e.to_string(),
    };
    let dir = game_dir.join(crate::ledger::STATE_DIR);
    std::fs::create_dir_all(&dir).map_err(not_writable)?;
    let probe = dir.join(".probe");
    std::fs::write(&probe, b"pacenotes").map_err(not_writable)?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}
