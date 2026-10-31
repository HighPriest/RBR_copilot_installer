/// `--selftest-download <game> [--server URL] [--token T] [--out FILE] [--largest]`:
/// downloads that game's smallest package (or `--largest`) into a throwaway
/// cache, exercising stream, hash, verify, promote and index without the wizard.
pub struct DownloadSelfTest {
    game: String,
    server: String,
    token: Option<String>,
    out: Option<std::path::PathBuf>,
    largest: bool,
}

impl DownloadSelfTest {
    pub fn from_args() -> Option<DownloadSelfTest> {
        let mut arguments = std::env::args().skip(1);
        let mut game = None;
        let mut server = None;
        let mut token = None;
        let mut out = None;
        let mut largest = false;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--selftest-download" => game = arguments.next(),
                "--server" => server = arguments.next(),
                "--token" => token = arguments.next(),
                "--out" => out = arguments.next().map(std::path::PathBuf::from),
                "--largest" => largest = true,
                _ => {}
            }
        }
        game.map(|game| DownloadSelfTest {
            game,
            server: server.unwrap_or_else(|| crate::DEFAULT_SERVER.to_string()),
            token,
            out,
            largest,
        })
    }

    /// The process exit code: 0 when the package arrived verified and indexed.
    pub fn run(&self) -> i32 {
        let (exit_code, report) = match self.pick_package() {
            Ok(package) => self.download(&package),
            Err(message) => (
                1,
                format!("RESULT: FAIL\nSTAGE: manifest\nMESSAGE: {message}\n"),
            ),
        };
        super::report::deliver(self.out.as_deref(), &report, exit_code)
    }

    fn pick_package(&self) -> Result<pacenotes_shared::PackageInfo, String> {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let api = crate::api::Api::new(&self.server, self.token.clone());
        let manifest = api
            .fetch_manifest(&self.game, &cancel)
            .map_err(|e| e.message)?;
        let mut packages: Vec<_> = manifest
            .packages
            .into_iter()
            .filter(|package| package.game == self.game)
            .collect();
        packages.sort_by_key(|package| package.size);
        let picked = if self.largest {
            packages.pop()
        } else {
            packages.into_iter().next()
        };
        picked.ok_or_else(|| format!("no packages for game {}", self.game))
    }

    /// Downloads into a throwaway cache, so a check never disturbs the real one.
    fn download(&self, package: &pacenotes_shared::PackageInfo) -> (i32, String) {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let api = crate::api::Api::new(&self.server, self.token.clone());
        let root = std::env::temp_dir().join(format!("pacenotes-selftest-{}", std::process::id()));
        let mut cache = pacenotes_installer::cache::Cache::open(&root);

        let started = std::time::Instant::now();
        let mut bytes_read = 0u64;
        let mut progress = |read: u64, _: Option<u64>| bytes_read = read;
        let mut retries = 0;
        let outcome = crate::package_download::download_retrying(
            &api,
            package,
            &mut cache,
            &mut progress,
            &cancel,
            &mut |_, _| retries += 1,
        );
        // Whole milliseconds, since formatting a float links ~16 kB.
        let millis = started.elapsed().as_millis();

        let result = match outcome {
            Ok(()) => (
                0,
                format!(
                    "RESULT: OK\nPACKAGE: {}\nSIZE: {}\nREAD: {bytes_read}\nSHA256: {}\nVERIFIED: yes\n\
                     PAYLOAD_ON_DISK: {}\nINDEXED: {}\nRETRIES: {retries}\nMILLIS: {millis}\n",
                    package.download_path(),
                    package.size,
                    package.sha256,
                    cache.has_payload(&package.sha256),
                    cache.get(&package.download_path()).is_some(),
                ),
            ),
            Err(e) => (
                1,
                format!(
                    "RESULT: FAIL\nSTAGE: download\nPACKAGE: {}\nSIZE: {}\nRETRIES: {retries}\nMESSAGE: {}\n",
                    package.download_path(),
                    package.size,
                    e.message
                ),
            ),
        };
        let _ = std::fs::remove_dir_all(&root);
        result
    }
}
