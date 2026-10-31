//! A GUI binary with `panic = "abort"` has nowhere to print, so a panic would
//! be a vanished window and an offset into a stripped exe. The hook runs before
//! the abort: it writes where and why into the run log, and tells the user in
//! their language where that is, offering to show the file.

/// Until the wizard knows the run log's file; a panic before that goes beside the exe.
static LOG: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Writes the report but shows no window, for `--selftest-panic`, which has nobody to click.
static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().map_or_else(
            || "unknown location".to_string(),
            |location| format!("{}:{}", location.file(), location.line()),
        );
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_string());
        let text = format!("pacenotes-installer panicked at {location}: {message}");
        let written = record(&text);

        // The modal pumps messages and can redeliver the one that panicked, so only the first shows.
        static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if QUIET.load(std::sync::atomic::Ordering::SeqCst)
            || SHOWN.swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        // The worker thread never had the chosen language applied.
        crate::i18n::use_chosen_language();
        tell_the_user(written.as_deref(), &text);
    }));
}

/// Points later panics at this run's log file.
pub fn log_to(path: std::path::PathBuf) {
    if let Ok(mut slot) = LOG.lock() {
        *slot = Some(path);
    }
}

pub fn be_quiet() {
    QUIET.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// A plain message and the file to send; the technical line is only shown
/// when there is no file to hold it.
fn tell_the_user(written: Option<&std::path::Path>, text: &str) {
    match written {
        Some(path) => {
            let ask =
                crate::i18n::load_string_arg(crate::i18n::IDS_CRASH, &path.display().to_string());
            if crate::ui::ask_error(core::ptr::null_mut(), &ask) {
                crate::ui::reveal_in_explorer(path);
            }
        }
        None => {
            let nowhere = crate::i18n::load_string(crate::i18n::IDS_CRASH_NOFILE);
            crate::ui::message_box_error(
                core::ptr::null_mut(),
                &format!("{nowhere}\r\n\r\n{text}"),
            );
        }
    }
}

/// Where `text` was appended, if anywhere; best effort, since failing to
/// write a panic must not become a second one.
fn record(text: &str) -> Option<std::path::PathBuf> {
    let path = LOG
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .or_else(fallback)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    // Timed like every other line of the run log it usually lands in.
    let now = pacenotes_installer::utc_time::iso_stamp(pacenotes_installer::utc_time::unix_now());
    std::io::Write::write_all(&mut file, format!("{now}  {text}\n").as_bytes()).ok()?;
    Some(path)
}

/// `pacenotes-installer-crash.log` next to the exe.
fn fallback() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_file_name("pacenotes-installer-crash.log"))
}
