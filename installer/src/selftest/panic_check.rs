/// `--selftest-panic [--out FILE]`: panics on purpose, quietly, so a suite can
/// prove the hook writes a report, a path using the installer correctly never takes.
pub fn panic_if_requested() {
    let mut arguments = std::env::args().skip(1);
    let mut requested = false;
    let mut out = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--selftest-panic" => requested = true,
            "--out" => out = arguments.next().map(std::path::PathBuf::from),
            _ => {}
        }
    }
    if !requested {
        return;
    }
    if let Some(path) = out {
        crate::crash::log_to(path);
    }
    crate::crash::be_quiet();
    panic!("deliberate panic from --selftest-panic");
}
