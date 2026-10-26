pub struct Log {
    lines: Vec<String>,
    path: Option<std::path::PathBuf>,
    file: Option<std::fs::File>,
}

impl Log {
    /// A file that cannot be opened leaves a log without one.
    pub fn create(path: Option<std::path::PathBuf>) -> Log {
        let mut log = Log {
            lines: Vec::new(),
            path: None,
            file: None,
        };
        if let Some(path) = path {
            log.attach(path);
        }
        log
    }

    /// Points the log at a file, writing out everything said so far, since the
    /// game folder it belongs in is only known partway through the run; a
    /// second call follows the run to another folder.
    pub fn attach(&mut self, path: std::path::PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
            super::prune(parent, super::KEEP_LOGS.saturating_sub(1));
        }
        let Ok(mut file) = std::fs::File::create(&path) else {
            return;
        };
        for line in &self.lines {
            let _ = std::io::Write::write_all(&mut file, format!("{line}\n").as_bytes());
        }
        let _ = std::io::Write::flush(&mut file);
        self.file = Some(file);
        self.path = Some(path);
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
