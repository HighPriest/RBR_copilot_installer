const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0000_0040;
const MAX_PATH: usize = 260;

/// The folder the user picked, or `None` on cancel.
pub unsafe fn browse_for_folder(
    owner: windows_sys::Win32::Foundation::HWND,
    prompt: &str,
) -> Option<String> {
    let title = crate::i18n::to_wide(prompt);
    let mut browse_info: windows_sys::Win32::UI::Shell::BROWSEINFOW = core::mem::zeroed();
    browse_info.hwndOwner = owner;
    browse_info.lpszTitle = title.as_ptr();
    browse_info.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;

    let item_list = windows_sys::Win32::UI::Shell::SHBrowseForFolderW(&browse_info);
    if item_list.is_null() {
        return None;
    }
    let mut buffer = [0u16; MAX_PATH];
    let found = windows_sys::Win32::UI::Shell::SHGetPathFromIDListW(item_list, buffer.as_mut_ptr());
    windows_sys::Win32::System::Com::CoTaskMemFree(item_list as *const core::ffi::c_void);
    if found == 0 {
        return None;
    }
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}
