pub struct Log {
    header: Vec<String>,
    entries: Vec<Entry>,
    clock: fn() -> i64,
    path: Option<std::path::PathBuf>,
    file: Option<std::fs::File>,
}

/// A line, and the moment it was said, which a line replayed into the file
/// later must keep.
struct Entry {
    said_at: i64,
    text: String,
}

impl Log {
    /// `header` heads the file and not the pane: it is what support asks for
    /// first, which a user reading along does not need. A file that cannot be
    /// opened leaves a log without one.
    pub fn create(header: Vec<String>, path: Option<std::path::PathBuf>) -> Log {
        Log::with_clock(header, path, crate::utc_time::unix_now)
    }

    /// `create`, telling the time by `clock`, so a test can name the moment.
    pub fn with_clock(
        header: Vec<String>,
        path: Option<std::path::PathBuf>,
        clock: fn() -> i64,
    ) -> Log {
        let mut log = Log {
            header,
            entries: Vec::new(),
            clock,
            path: None,
            file: None,
        };
        if let Some(path) = path {
            log.attach(path);
        }
        log
    }

    /// Points the log at a file, writing out the header and everything said so
    /// far, since the game folder it belongs in is only known partway through
    /// the run; a second call follows the run to another folder.
    pub fn attach(&mut self, path: std::path::PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
            super::prune(parent, super::KEEP_LOGS.saturating_sub(1));
        }
        let Ok(mut file) = std::fs::File::create(&path) else {
            return;
        };
        let lines = self
            .header
            .iter()
            .cloned()
            .chain(std::iter::once(String::new()))
            .chain(self.entries.iter().map(stamped));
        for line in lines {
            let _ = std::io::Write::write_all(&mut file, format!("{line}\n").as_bytes());
        }
        let _ = std::io::Write::flush(&mut file);
        self.file = Some(file);
        self.path = Some(path);
    }

    /// Flushed at once, so a run that dies still leaves the line saying what it was doing.
    pub fn line(&mut self, text: impl Into<String>) {
        let entry = Entry {
            said_at: (self.clock)(),
            text: text.into(),
        };
        if let Some(file) = self.file.as_mut() {
            let _ = std::io::Write::write_all(file, format!("{}\n", stamped(&entry)).as_bytes());
            let _ = std::io::Write::flush(file);
        }
        self.entries.push(entry);
    }

    /// As the pane shows them: without the times, which only the file needs.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.text.as_str())
    }

    /// CRLF-separated, because a Win32 multiline edit ignores a lone LF.
    pub fn text(&self) -> String {
        self.lines().collect::<Vec<_>>().join("\r\n")
    }

    /// `None` when no file was written, so the summary never points at one that does not exist.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// UTC in the server log's own shape, so a user's log lines up with the server's.
fn stamped(entry: &Entry) -> String {
    format!(
        "{}  {}",
        crate::utc_time::iso_stamp(entry.said_at),
        entry.text
    )
}
