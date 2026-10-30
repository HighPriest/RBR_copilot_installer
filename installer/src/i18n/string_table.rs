/// The string `id` in the calling thread's UI language, or `<id>` when the
/// table lacks it, so a forgotten translation shows instead of a blank control.
pub fn load_string(id: u16) -> String {
    unsafe {
        let mut resource: *mut u16 = core::ptr::null_mut();
        let point_into_resource = 0;
        let length = windows_sys::Win32::UI::WindowsAndMessaging::LoadStringW(
            instance(),
            id as u32,
            &mut resource as *mut *mut u16 as windows_sys::core::PWSTR,
            point_into_resource,
        );
        if length <= 0 || resource.is_null() {
            return format!("<{id}>");
        }
        String::from_utf16_lossy(core::slice::from_raw_parts(resource, length as usize))
    }
}

/// The string `id` with its first `%s` replaced by `argument`; a template
/// without one comes back unchanged.
pub fn load_string_arg(id: u16, argument: &str) -> String {
    let template = load_string(id);
    match template.find("%s") {
        Some(at) => format!("{}{argument}{}", &template[..at], &template[at + 2..]),
        None => template,
    }
}

/// This executable's module handle, where the resources live.
pub fn instance() -> windows_sys::Win32::Foundation::HMODULE {
    unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(core::ptr::null()) }
}
