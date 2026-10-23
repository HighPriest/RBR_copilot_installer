/// Exit code when the report itself could not be written.
const REPORT_UNWRITABLE: i32 = 2;

/// Writes `report` to `out`, or shows it when there is none, and returns
/// `exit_code` unless the file could not be written.
pub fn deliver(out: Option<&std::path::Path>, report: &str, exit_code: i32) -> i32 {
    match out {
        Some(path) => {
            if std::fs::write(path, report).is_err() {
                crate::ui::message_box_error(
                    core::ptr::null_mut(),
                    &format!("Cannot write {}", path.display()),
                );
                return REPORT_UNWRITABLE;
            }
        }
        None => crate::ui::message_box_info(core::ptr::null_mut(), report),
    }
    exit_code
}
