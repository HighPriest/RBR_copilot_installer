//! Pacenotes Installer — entry point.
//! Flow mirrors legacy Program.cs: language picker (Cancel = exit) → main dialog.
//! See installer/CLAUDE.md before editing. Size gate: tests/Test-BinarySize.ps1.
//!
//! Windows-only. The pure logic lives in the sibling lib target (src/lib.rs);
//! only the modules declared here may touch windows-sys.
#![windows_subsystem = "windows"]
#![cfg(windows)]

mod api;
mod http;
mod i18n;
mod ui;

fn main() {
    // Task I3's manual check, before any UI is created.
    if let Some(selftest) = SelfTest::from_args() {
        std::process::exit(selftest.run());
    }

    unsafe {
        let icc = windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX {
            dwSize: core::mem::size_of::<windows_sys::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
                as u32,
            dwICC: windows_sys::Win32::UI::Controls::ICC_STANDARD_CLASSES
                | windows_sys::Win32::UI::Controls::ICC_PROGRESS_CLASS
                | windows_sys::Win32::UI::Controls::ICC_LISTVIEW_CLASSES
                | windows_sys::Win32::UI::Controls::ICC_TAB_CLASSES,
        };
        windows_sys::Win32::UI::Controls::InitCommonControlsEx(&icc);
    }

    // Task I2: language picker dialog; on Cancel, exit (like Program.cs).
    let Some(lang) = ui::show_language_picker() else {
        return;
    };
    i18n::set_language(lang);

    // Task I2/I4: main dialog owns the app lifetime; worker thread posts
    // WM_APP progress messages back to it.
    ui::run_main_dialog();
}

/// `--selftest-http <url> [--token <bearer>] [--out <file>]`
///
/// Performs one real GET through `http.rs` and reports what came back. This is
/// the only way to exercise the WinHTTP path without the rest of the app, and
/// it is what `tests/ui/Test-HttpClient.ps1` drives against a live server.
///
/// Output goes to `--out` when given and to a MessageBox otherwise. This is a
/// GUI-subsystem binary, so it has no stdout to print to; attaching to the
/// parent console would mean pulling in console APIs and reopening CONOUT$ for
/// a developer-only flag, whereas a file is both scriptable and free.
struct SelfTest {
    url: String,
    /// Access key, so the authenticated paths can be exercised too. The token
    /// is never written into the report: it would end up in a test artifact or
    /// a pasted MessageBox, and server/CLAUDE.md forbids logging credentials.
    token: Option<String>,
    out: Option<std::path::PathBuf>,
}

impl SelfTest {
    fn from_args() -> Option<SelfTest> {
        let mut args = std::env::args().skip(1);
        let mut url = None;
        let mut token = None;
        let mut out = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--selftest-http" => url = args.next(),
                "--token" => token = args.next(),
                "--out" => out = args.next().map(std::path::PathBuf::from),
                _ => {}
            }
        }
        url.map(|url| SelfTest { url, token, out })
    }

    /// Returns the process exit code: 0 when the GET succeeded.
    fn run(&self) -> i32 {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut last_progress = (0u64, None);
        let mut progress = |read: u64, total: Option<u64>| last_progress = (read, total);

        let (code, report) =
            match http::get(&self.url, self.token.as_deref(), &mut progress, &cancel) {
                Ok(body) => {
                    let preview: String = String::from_utf8_lossy(&body)
                        .chars()
                        .take(PREVIEW_CHARS)
                        .collect();
                    (
                        0,
                        format!(
                            "RESULT: OK\nURL: {}\nBYTES: {}\nCONTENT_LENGTH: {}\nBODY: {preview}\n",
                            self.url,
                            body.len(),
                            last_progress
                                .1
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "unknown".into()),
                        ),
                    )
                }
                Err(e) => (
                    1,
                    format!(
                        "RESULT: FAIL\nURL: {}\nSTATUS: {}\nCODE: {}\nMESSAGE: {}\n",
                        self.url,
                        e.status
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".into()),
                        e.code,
                        e.message,
                    ),
                ),
            };

        match &self.out {
            Some(path) => {
                if std::fs::write(path, &report).is_err() {
                    ui::message_box_error(&format!("Cannot write {}", path.display()));
                    return 2;
                }
            }
            None => ui::message_box_info(&report),
        }
        code
    }
}

const PREVIEW_CHARS: usize = 400;
