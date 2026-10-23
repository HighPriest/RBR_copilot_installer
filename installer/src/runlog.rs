//! The run log (DESIGN.md "Logging"): in memory for scene 6's pane, and on
//! disk line by line for after a run that died. Only the UI thread writes it;
//! workers post their lines.

/// `%LOCALAPPDATA%\PacenotesInstaller\install.log`, or `None` when the variable is unset.
pub fn default_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        std::path::PathBuf::from(base)
            .join("PacenotesInstaller")
            .join("install.log")
    })
}

pub struct Log {
    lines: Vec<String>,
    path: Option<std::path::PathBuf>,
    file: Option<std::fs::File>,
}

impl Log {
    /// Truncates the previous run's file, since the interesting run is always
    /// the last; a file that cannot be opened leaves a log without one.
    pub fn create(path: Option<std::path::PathBuf>) -> Log {
        let file = path.as_ref().and_then(|path| {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::File::create(path).ok()
        });
        Log {
            lines: Vec::new(),
            path: path.filter(|_| file.is_some()),
            file,
        }
    }

    /// Flushed at once, so a run that dies still leaves the line saying what it was doing.
    pub fn line(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(file) = self.file.as_mut() {
            let _ = std::io::Write::write_all(file, format!("{text}\n").as_bytes());
            let _ = std::io::Write::flush(file);
        }
        self.lines.push(text);
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// CRLF-separated, because a Win32 multiline edit ignores a lone LF.
    pub fn text(&self) -> String {
        self.lines.join("\r\n")
    }

    /// `None` when no file was written, so the summary never points at one that does not exist.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests;
