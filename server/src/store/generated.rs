/// A generated manifest and what the publisher should look at, collected so
/// the CLI prints them together and tests can assert on them.
#[derive(Debug)]
pub struct Generated {
    pub manifest: pacenotes_shared::Manifest,
    /// Published anyway, but worth a look: a mis-filed level, a name that needs encoding.
    pub warnings: Vec<String>,
    /// Withheld from the manifest, such as a broken sidecar's package; the rest
    /// still publishes, but the run exits non-zero.
    pub errors: Vec<String>,
}

impl Generated {
    pub(super) fn warn(&mut self, message: String) {
        tracing::warn!("{message}");
        self.warnings.push(message);
    }

    pub(super) fn error(&mut self, message: String) {
        tracing::error!("{message}");
        self.errors.push(message);
    }
}
