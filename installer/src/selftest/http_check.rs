const PREVIEW_CHARS: usize = 400;

/// `--selftest-http <url> [--token <bearer>] [--out <file>]`: one GET through
/// `http`, which `tests/Test-HttpClient.ps1` drives against a live server.
pub struct HttpSelfTest {
    url: String,
    /// Never written into the report, which may become a test artifact.
    token: Option<String>,
    out: Option<std::path::PathBuf>,
}

impl HttpSelfTest {
    pub fn from_args() -> Option<HttpSelfTest> {
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
        url.map(|url| HttpSelfTest { url, token, out })
    }

    /// The process exit code: 0 when the GET succeeded.
    pub fn run(&self) -> i32 {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut content_length = None;
        let mut progress = |_: u64, total: Option<u64>| content_length = total;

        let (exit_code, report) =
            match crate::http::get(&self.url, self.token.as_deref(), &mut progress, &cancel) {
                Ok(body) => (0, self.success_report(&body, content_length)),
                Err(e) => (1, self.failure_report(&e)),
            };
        super::report::deliver(self.out.as_deref(), &report, exit_code)
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
