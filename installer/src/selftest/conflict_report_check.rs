/// `--selftest-report <game> [--server URL] [--out FILE]`: posts a fabricated
/// conflict report, the only way to exercise the POST path, since a real one
/// needs a catalogue where two packages write one file. The overlap is built as
/// `engine::overlaps` builds one, so the keys, refs and trim are the real ones.
pub struct ConflictReportSelfTest {
    game: String,
    server: String,
    out: Option<std::path::PathBuf>,
}

impl ConflictReportSelfTest {
    pub fn from_args() -> Option<ConflictReportSelfTest> {
        let mut arguments = std::env::args().skip(1);
        let mut game = None;
        let mut server = None;
        let mut out = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--selftest-report" => game = arguments.next(),
                "--server" => server = arguments.next(),
                "--out" => out = arguments.next().map(std::path::PathBuf::from),
                _ => {}
            }
        }
        game.map(|game| ConflictReportSelfTest {
            game,
            server: server.unwrap_or_else(|| crate::DEFAULT_SERVER.to_string()),
            out,
        })
    }

    /// The process exit code: 0 when the server took the report.
    pub fn run(&self) -> i32 {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let api = crate::api::Api::new(&self.server, None);
        let overlaps = vec![pacenotes_installer::engine::Overlap {
            path: "Plugins/Pacenote/PaceNote.ini".to_string(),
            packages: vec![
                format!("{}/public/language/selftest", self.game),
                format!("{}/public/codriver/selftest", self.game),
            ],
        }];

        let (exit_code, report) = match api.report_conflicts(&self.game, &overlaps, &cancel) {
            Ok(()) => (
                0,
                format!(
                    "RESULT: OK\nGAME: {}\nCONFLICTS: {}\n",
                    self.game,
                    overlaps.len()
                ),
            ),
            Err(e) => (
                1,
                format!(
                    "RESULT: FAIL\nGAME: {}\nSTATUS: {}\nMESSAGE: {}\n",
                    self.game,
                    e.status
                        .map_or_else(|| "-".to_string(), |status| status.to_string()),
                    e.message
                ),
            ),
        };
        super::report::deliver(self.out.as_deref(), &report, exit_code)
    }
}
