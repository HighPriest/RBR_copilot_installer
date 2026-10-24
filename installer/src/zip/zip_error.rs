/// Readable as-is by the user.
#[derive(Debug)]
pub struct ZipError(pub String);

impl std::fmt::Display for ZipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl ZipError {
    pub(super) fn new(message: impl Into<String>) -> ZipError {
        ZipError(message.into())
    }

    pub(super) fn truncated() -> ZipError {
        ZipError::new("this package is damaged (it ends in the middle of a record)")
    }
}
