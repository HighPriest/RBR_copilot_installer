/// Whether another copy is running for this user, since two would share the
/// download temp files, `cache.json` and a game folder's ledger unguarded.
///
/// A named mutex needs no cleanup after a crash. `Local\` scopes it to the
/// logon session, because `Global\` needs a privilege a standard user may lack.
/// The handle is never closed, so the name lives as long as the process.
pub fn another_instance_running() -> bool {
    let name = crate::i18n::to_wide("Local\\PacenotesInstaller.SingleInstance");
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
        windows_sys::Win32::Foundation::GetLastError()
            == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
    }
}
