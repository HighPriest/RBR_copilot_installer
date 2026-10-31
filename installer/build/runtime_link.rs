/// The "hybrid CRT": the VC runtime linked into the exe, the Universal CRT left
/// to Windows, which ships it since Windows 10. Without it the installer needs
/// the Visual C++ redistributable, and on a PC without one Windows refuses to
/// start it with an unlocalized "VCRUNTIME140.dll was not found". A fully
/// static CRT measured +97 kB; this one +22 kB (installer/CLAUDE.md).
///
/// `+crt-static` itself comes from `.cargo/config.toml`, the one place a target
/// feature can be set; a RUSTFLAGS variable silently replaces it, so this
/// refuses to build without it rather than ship the dependency again.
pub fn link_hybrid_crt() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let features = std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    if !features.split(',').any(|feature| feature == "crt-static") {
        panic!(
            "the installer must link the VC runtime statically (+crt-static, set in \
             installer/.cargo/config.toml); a RUSTFLAGS variable replaces that file's \
             flags, so unset it, or the exe would need the Visual C++ redistributable"
        );
    }
    // Static VC runtime, but the Universal CRT from Windows rather than linked in.
    println!("cargo:rustc-link-arg=/NODEFAULTLIB:libucrt.lib");
    println!("cargo:rustc-link-arg=/DEFAULTLIB:ucrt.lib");
}
