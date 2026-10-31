//! Restarting as administrator, for a game folder this account cannot write
//! (P11): what the new instance carries over so nothing is asked twice, and
//! the UAC launch itself.

const RESTARTED: &str = "--restarted";
const LANGUAGE: &str = "--language";
const FOLDER: &str = "--folder";
const SECRET: &str = "--secret";
/// Developer overrides a restart must keep, or it would talk to another server.
const PASSED_ON: [&str; 2] = ["--server", "--cache"];

/// What a restarted instance resumes from; it opens on scene 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restart {
    pub language: crate::i18n::Language,
    pub folder: String,
    /// On the elevated process's command line, readable by other programs
    /// running as this user: accepted by the maintainer for a seamless restart.
    pub secret: String,
}

impl Restart {
    /// This process's own restart, when it is one.
    pub fn from_args() -> Option<Restart> {
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        Restart::parse(&arguments)
    }

    fn parse(arguments: &[String]) -> Option<Restart> {
        if !arguments.iter().any(|argument| argument == RESTARTED) {
            return None;
        }
        let value = |flag: &str| {
            let at = arguments.iter().position(|argument| argument == flag)?;
            arguments.get(at + 1).cloned()
        };
        Some(Restart {
            language: crate::i18n::Language::from_tag(&value(LANGUAGE)?)?,
            folder: value(FOLDER)?,
            secret: value(SECRET).unwrap_or_default(),
        })
    }

    /// The command line of the restarted instance, with the developer
    /// overrides this one was given.
    fn command_line(&self, passed_on: &[(&str, String)]) -> String {
        let mut arguments = vec![
            RESTARTED.to_string(),
            LANGUAGE.to_string(),
            self.language.tag().to_string(),
            FOLDER.to_string(),
            self.folder.clone(),
            SECRET.to_string(),
            self.secret.clone(),
        ];
        for (flag, value) in passed_on {
            arguments.push(flag.to_string());
            arguments.push(value.clone());
        }
        arguments
            .iter()
            .map(|argument| quote(argument))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Whether this process already runs elevated, when offering elevation again
/// would only loop.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: windows_sys::Win32::Foundation::HANDLE = core::ptr::null_mut();
        if windows_sys::Win32::System::Threading::OpenProcessToken(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            windows_sys::Win32::Security::TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return false;
        }
        let mut elevation = windows_sys::Win32::Security::TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut written = 0u32;
        let read = windows_sys::Win32::Security::GetTokenInformation(
            token,
            windows_sys::Win32::Security::TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            core::mem::size_of::<windows_sys::Win32::Security::TOKEN_ELEVATION>() as u32,
            &mut written,
        );
        windows_sys::Win32::Foundation::CloseHandle(token);
        read != 0 && elevation.TokenIsElevated != 0
    }
}

/// Starts this executable again through UAC; false when the user cancels the
/// prompt or it cannot start, which leaves this instance where it was.
pub fn restart_elevated(owner: windows_sys::Win32::Foundation::HWND, restart: &Restart) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let passed_on: Vec<(&str, String)> = PASSED_ON
        .iter()
        .filter_map(|flag| Some((*flag, crate::argument_after(flag)?)))
        .collect();
    let verb = crate::i18n::to_wide("runas");
    let file = crate::i18n::to_wide(&exe.to_string_lossy());
    let parameters = crate::i18n::to_wide(&restart.command_line(&passed_on));
    let started = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            owner,
            verb.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            core::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    // Anything above 32 is success, by ShellExecute's own convention.
    started as isize > 32
}

/// One argument as `CommandLineToArgvW` and Rust's `std::env::args` split it
/// back: quoted, with the backslashes before a quote doubled.
fn quote(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '"']) {
        return argument.to_string();
    }
    let mut quoted = String::from('"');
    // Backslashes count only before a quote, including the closing one.
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '"' {
            quoted.push_str(&"\\".repeat(backslashes + 1));
        }
        backslashes = if character == '\\' {
            backslashes + 1
        } else {
            0
        };
        quoted.push(character);
    }
    quoted.push_str(&"\\".repeat(backslashes));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests;
