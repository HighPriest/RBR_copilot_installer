//! `--selftest-http <url> [--token <bearer>] [--out <file>]`: one real GET
//! through `http`, reported to a file or a MessageBox, since a GUI-subsystem
//! binary has no stdout. `tests/Test-HttpClient.ps1` drives it.

const PREVIEW_CHARS: usize = 400;

pub struct SelfTest {
    url: String,
    /// Never written into the report, which may become a test artifact.
    token: Option<String>,
    out: Option<std::path::PathBuf>,
}

impl SelfTest {
    pub fn from_args() -> Option<SelfTest> {
        let mut arguments = std::env::args().skip(1);
        let mut url = None;
        let mut token = None;
        let mut out = None;
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--selftest-http" => url = arguments.next(),
                "--token" => token = arguments.next(),
                "--out" => out = arguments.next().map(std::path::PathBuf::from),
                _ => {}
            }
        }
        url.map(|url| SelfTest { url, token, out })
    }

    /// The process exit code: 0 when the GET succeeded, 1 when it failed, 2
    /// when the report could not be written.
    pub fn run(&self) -> i32 {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut content_length = None;
        let mut progress = |_: u64, total: Option<u64>| content_length = total;

        let (exit_code, report) =
            match crate::http::get(&self.url, self.token.as_deref(), &mut progress, &cancel) {
                Ok(body) => (0, self.success_report(&body, content_length)),
                Err(e) => (1, self.failure_report(&e)),
            };

        match &self.out {
            Some(path) => {
                if std::fs::write(path, &report).is_err() {
                    crate::ui::message_box_error(
                        core::ptr::null_mut(),
                        &format!("Cannot write {}", path.display()),
                    );
                    return 2;
                }
            }
            None => crate::ui::message_box_info(core::ptr::null_mut(), &report),
        }
        exit_code
    }

    fn success_report(&self, body: &[u8], content_length: Option<u64>) -> String {
        let preview: String = String::from_utf8_lossy(body)
            .chars()
            .take(PREVIEW_CHARS)
            .collect();
        let content_length = content_length
            .map(|length| length.to_string())
            .unwrap_or_else(|| "unknown".into());
        format!(
            "RESULT: OK\nURL: {}\nBYTES: {}\nCONTENT_LENGTH: {content_length}\nBODY: {preview}\n",
            self.url,
            body.len(),
        )
    }

    fn failure_report(&self, error: &crate::http::HttpError) -> String {
        let status = error
            .status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "-".into());
        format!(
            "RESULT: FAIL\nURL: {}\nSTATUS: {status}\nCODE: {}\nMESSAGE: {}\n",
            self.url, error.code, error.message,
        )
    }
}
