const APP_ICON_ID: u16 = 1;

/// `MAKEINTRESOURCEW`.
pub fn make_int_resource(id: u16) -> windows_sys::core::PCWSTR {
    id as usize as windows_sys::core::PCWSTR
}

pub fn loword(value: windows_sys::Win32::Foundation::WPARAM) -> u16 {
    (value & 0xFFFF) as u16
}

pub fn hiword(value: windows_sys::Win32::Foundation::WPARAM) -> u16 {
    ((value >> 16) & 0xFFFF) as u16
}

pub unsafe fn load_app_icon() -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
    windows_sys::Win32::UI::WindowsAndMessaging::LoadIconW(
        crate::i18n::instance(),
        make_int_resource(APP_ICON_ID),
    )
}
