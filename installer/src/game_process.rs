/// Whether `game` is running from `folder`, which holds its files open so an
/// install would fail partway (P12). Another copy of the game elsewhere does
/// not lock this folder, so the process's own path decides; one whose path
/// cannot be read — say, running elevated while we are not — counts as
/// running, since refusing is the safe answer.
pub fn is_running_from(game: pacenotes_installer::gamedir::Game, folder: &str) -> bool {
    let marker = game.marker().to_lowercase();
    // Windows reports a process's path with `\` and no doubled separator; the
    // folder field may hold `/` or a trailing `\`. Done by hand: std's
    // canonicalize cost 3.5 kB.
    let folder = folder.replace('/', "\\");
    let expected = format!("{}\\{marker}", folder.trim_end_matches('\\').to_lowercase());
    unsafe {
        let snapshot = windows_sys::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot(
            windows_sys::Win32::System::Diagnostics::ToolHelp::TH32CS_SNAPPROCESS,
            0,
        );
        if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: windows_sys::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W =
            core::mem::zeroed();
        entry.dwSize = core::mem::size_of_val(&entry) as u32;
        let mut found = false;
        let mut more = windows_sys::Win32::System::Diagnostics::ToolHelp::Process32FirstW(
            snapshot, &mut entry,
        );
        while more != 0 && !found {
            if wide_text(&entry.szExeFile).to_lowercase() == marker {
                found = image_path(entry.th32ProcessID).is_none_or(|path| path == expected);
            }
            more = windows_sys::Win32::System::Diagnostics::ToolHelp::Process32NextW(
                snapshot, &mut entry,
            );
        }
        windows_sys::Win32::Foundation::CloseHandle(snapshot);
        found
    }
}

/// The full path of a process's executable, lowercased, or `None` when it
/// cannot be opened.
unsafe fn image_path(process_id: u32) -> Option<String> {
    let process = windows_sys::Win32::System::Threading::OpenProcess(
        windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
        0,
        process_id,
    );
    if process.is_null() {
        return None;
    }
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    let read = windows_sys::Win32::System::Threading::QueryFullProcessImageNameW(
        process,
        0,
        buffer.as_mut_ptr(),
        &mut length,
    );
    windows_sys::Win32::Foundation::CloseHandle(process);
    (read != 0).then(|| wide_text(&buffer[..length as usize]).to_lowercase())
}

fn wide_text(wide: &[u16]) -> String {
    let end = wide
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}
