/// What support asks for first, at the top of every run log: which build, when,
/// which server and which Windows.
pub fn run_header(base_url: &str) -> Vec<String> {
    let started =
        pacenotes_installer::utc_time::iso_stamp(pacenotes_installer::utc_time::unix_now());
    vec![
        format!("Pacenotes Installer {}", env!("PACENOTES_VERSION")),
        format!("started  {started}"),
        format!("server   {base_url}"),
        format!("windows  {}", windows_version()),
    ]
}

/// `RtlGetVersion` rather than `GetVersionExW`, which reports Windows 8 to any
/// program whose manifest does not list a newer one, as ours does not.
fn windows_version() -> String {
    let mut info: windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW =
        unsafe { core::mem::zeroed() };
    info.dwOSVersionInfoSize = core::mem::size_of::<
        windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW,
    >() as u32;
    let status = unsafe { windows_sys::Wdk::System::SystemServices::RtlGetVersion(&mut info) };
    if status != 0 {
        return "unknown".to_string();
    }
    format!(
        "{}.{}.{}",
        info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
    )
}
