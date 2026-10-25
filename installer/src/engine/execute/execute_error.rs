#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecuteError {
    Io {
        doing: String,
        reason: String,
    },
    /// Usually a CRC mismatch; the package stays `installing` in the ledger,
    /// so the next run cleans up after it.
    Extract {
        package: String,
        reason: String,
    },
    Ledger {
        reason: String,
    },
}

impl ExecuteError {
    pub(super) fn io(doing: impl Into<String>, error: impl std::fmt::Display) -> ExecuteError {
        ExecuteError::Io {
            doing: doing.into(),
            reason: error.to_string(),
        }
    }

    pub(super) fn extract(package: &str, reason: impl std::fmt::Display) -> ExecuteError {
        ExecuteError::Extract {
            package: package.to_string(),
            reason: reason.to_string(),
        }
    }

    pub(super) fn ledger(error: std::io::Error) -> ExecuteError {
        ExecuteError::Ledger {
            reason: error.to_string(),
        }
    }
}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecuteError::Io { doing, reason } => write!(formatter, "{doing}: {reason}"),
            ExecuteError::Extract { package, reason } => {
                write!(formatter, "{package} could not be installed: {reason}")
            }
            ExecuteError::Ledger { reason } => write!(
                formatter,
                "the installation record could not be updated: {reason}"
            ),
        }
    }
}
