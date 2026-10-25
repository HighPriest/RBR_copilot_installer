/// One stored line, stamped with the server's clock since a client's is never trusted.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct Record {
    /// Unix seconds.
    pub received_at: i64,
    pub game: String,
    pub installer: String,
    pub conflicts: Vec<pacenotes_shared::Conflict>,
    pub dropped: usize,
}

/// Where reports go, whose `X-Forwarded-For` to believe, and the limiter.
pub struct Reports {
    dir: std::path::PathBuf,
    trusted: Vec<super::Cidr>,
    limiter: super::RateLimiter,
    /// So two reports never interleave in one line.
    write_lock: tokio::sync::Mutex<()>,
}

impl Reports {
    pub fn new(
        dir: impl Into<std::path::PathBuf>,
        trusted: Vec<super::Cidr>,
        limits: super::Limits,
    ) -> Reports {
        Reports {
            dir: dir.into(),
            trusted,
            limiter: super::RateLimiter::new(limits),
            write_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn client_ip(
        &self,
        peer: std::net::IpAddr,
        headers: &axum::http::HeaderMap,
    ) -> std::net::IpAddr {
        super::client_ip(peer, headers, &self.trusted)
    }

    pub fn check_rate(&self, client: std::net::IpAddr) -> Result<(), u64> {
        self.limiter.check(client, std::time::Instant::now())
    }

    /// Appends one JSON line to the day's file.
    pub async fn append(&self, record: &Record) -> anyhow::Result<std::path::PathBuf> {
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        let _guard = self.write_lock.lock().await;
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| anyhow::anyhow!("creating {}: {e}", self.dir.display()))?;
        let path = self.dir.join(super::file_name(record.received_at));
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| anyhow::anyhow!("opening {}: {e}", path.display()))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("writing {}: {e}", path.display()))?;
        // tokio's `write_all` can return before the bytes reach the OS, so the 202 would precede them.
        tokio::io::AsyncWriteExt::flush(&mut file)
            .await
            .map_err(|e| anyhow::anyhow!("flushing {}: {e}", path.display()))?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn records_append_one_line_each_to_the_days_file() {
        let tmp = tempfile::tempdir().unwrap();
        let reports = super::Reports::new(
            tmp.path().join("reports"),
            vec![],
            super::super::Limits::default(),
        );
        let record = super::Record {
            received_at: 1_792_886_400,
            game: "rbr".into(),
            installer: "0.2.0".into(),
            conflicts: vec![pacenotes_shared::Conflict {
                file: "Plugins/Pacenote/PaceNote.ini".into(),
                packages: vec![
                    "public/language/polish".into(),
                    "public/codriver/polish/male/Kamil".into(),
                ],
            }],
            dropped: 0,
        };
        let path = reports.append(&record).await.unwrap();
        reports.append(&record).await.unwrap();
        assert!(path.ends_with("reports/conflicts-2026-10-25.jsonl"));

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let back: super::Record = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(back, record);
    }
}
