//! The restart is only seamless if what one instance writes the next one reads
//! back exactly, so the command line is checked against Windows' own splitter.

/// How Windows itself splits a command line, the source of truth `quote` must
/// agree with.
fn split_by_windows(command_line: &str) -> Vec<String> {
    let wide = crate::i18n::to_wide(&format!("pacenotes-installer.exe {command_line}"));
    let mut count = 0;
    unsafe {
        let parts = windows_sys::Win32::UI::Shell::CommandLineToArgvW(wide.as_ptr(), &mut count);
        assert!(!parts.is_null(), "CommandLineToArgvW failed");
        let split = (1..count as usize)
            .map(|index| {
                let part = *parts.add(index);
                let length = (0..).take_while(|&at| *part.add(at) != 0).count();
                String::from_utf16_lossy(core::slice::from_raw_parts(part, length))
            })
            .collect();
        windows_sys::Win32::Foundation::LocalFree(parts as _);
        split
    }
}

fn restart(folder: &str, secret: &str) -> super::Restart {
    super::Restart {
        language: crate::i18n::Language::Pl,
        folder: folder.to_string(),
        secret: secret.to_string(),
    }
}

/// The shapes a real game folder takes: spaces, a trailing backslash (which
/// would otherwise escape the closing quote), and quotes in the odd case.
#[test]
fn a_restart_reads_back_exactly_what_was_written() {
    let folders = [
        r"C:\Program Files (x86)\SCS\Richard Burns Rally",
        r"C:\Program Files\RBR\",
        r"D:\Games\RBR",
        r#"D:\Odd "quoted" \\ name\"#,
    ];
    for folder in folders {
        let written = restart(folder, "pcn_0123456789abcdefghijklmnopqrstuvwxyzABCDEFG");
        let arguments = split_by_windows(&written.command_line(&[]));
        assert_eq!(super::Restart::parse(&arguments), Some(written), "{folder}");
    }
}

/// Without these a restarted test run would quietly talk to the real server.
#[test]
fn the_developer_overrides_survive_a_restart() {
    let written = restart(r"C:\Games\RBR", "");
    let passed_on = [
        ("--server", "http://127.0.0.1:8080".to_string()),
        ("--cache", r"C:\scratch cache\".to_string()),
    ];
    let arguments = split_by_windows(&written.command_line(&passed_on));

    let after = |flag: &str| {
        let at = arguments.iter().position(|argument| argument == flag)?;
        arguments.get(at + 1).cloned()
    };
    assert_eq!(after("--server").as_deref(), Some("http://127.0.0.1:8080"));
    assert_eq!(after("--cache").as_deref(), Some(r"C:\scratch cache\"));
    assert_eq!(super::Restart::parse(&arguments), Some(written));
}

/// Only a restart opens on scene 3; an ordinary launch never does.
#[test]
fn an_ordinary_launch_is_not_a_restart() {
    let arguments: Vec<String> = ["--server", "http://127.0.0.1:8080", "--folder", r"C:\RBR"]
        .iter()
        .map(|argument| argument.to_string())
        .collect();
    assert_eq!(super::Restart::parse(&arguments), None);
}
