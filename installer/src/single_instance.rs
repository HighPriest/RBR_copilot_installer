const MUTEX_NAME: &str = "Local\\PacenotesInstaller.SingleInstance";
/// How long a restarted instance waits for the one that launched it to close.
const RESTART_WAIT_MS: u32 = 10_000;
const RESTART_POLL_MS: u32 = 100;

/// Whether another copy is running for this user, since two would share the
/// download temp files, `cache.json` and a game folder's ledger unguarded.
///
/// A named mutex needs no cleanup after a crash. `Local\` scopes it to the
/// logon session, because `Global\` needs a privilege a standard user may lack.
/// The handle that wins is never closed, so the name lives as long as the
/// process; one that finds the name taken is, or it would keep the mutex alive
/// after its owner exits.
pub fn another_instance_running() -> bool {
    let name = crate::i18n::to_wide(MUTEX_NAME);
    let take_ownership = 1;
    unsafe {
        let handle = windows_sys::Win32::System::Threading::CreateMutexW(
            core::ptr::null(),
            take_ownership,
            name.as_ptr(),
        );
        // A failed check must not block the run on a machine with an unusual policy.
        if handle.is_null() {
            return false;
        }
        let taken = windows_sys::Win32::Foundation::GetLastError()
            == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
        if taken {
            windows_sys::Win32::Foundation::CloseHandle(handle);
        }
        taken
    }
}

/// `another_instance_running` for an instance restarted as administrator, which
/// starts while the one that launched it is still closing.
pub fn another_instance_outlasts_restart() -> bool {
    let mut waited = 0;
    while another_instance_running() {
        if waited >= RESTART_WAIT_MS {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(u64::from(RESTART_POLL_MS)));
        waited += RESTART_POLL_MS;
    }
    false
}
