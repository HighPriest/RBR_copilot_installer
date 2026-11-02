const ENV_FILE: &str = ".env";
/// Names another file to read instead. The documentation build sets it to
/// `.env.example`: rustdoc prints the address it finds, and a published page
/// must never carry a deployment's own.
const ENV_FILE_OVERRIDE: &str = "PACENOTES_ENV_FILE";
const DEFAULT_SERVER_KEY: &str = "PACENOTES_DEFAULT_SERVER";

/// The server address lives in the gitignored `.env`, so a deployment's
/// address never enters the repository; without it the build fails.
pub fn export_default_server() {
    println!("cargo:rerun-if-env-changed={ENV_FILE_OVERRIDE}");
    let file = std::env::var(ENV_FILE_OVERRIDE).unwrap_or_else(|_| ENV_FILE.to_string());
    println!("cargo:rerun-if-changed={file}");
    let text = std::fs::read_to_string(&file).unwrap_or_else(|_| {
        panic!(
            "installer/{file} is missing: copy installer/.env.example to installer/{ENV_FILE} \
             and set {DEFAULT_SERVER_KEY} (see DEVELOPMENT.md)"
        )
    });
    let server = env_value(&text, DEFAULT_SERVER_KEY)
        .filter(|server| !server.is_empty())
        .unwrap_or_else(|| panic!("installer/{file} does not set {DEFAULT_SERVER_KEY}"));
    println!("cargo:rustc-env={DEFAULT_SERVER_KEY}={server}");
}

/// The value of `KEY=value` in dotenv text; `#` starts a comment line.
fn env_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| line.strip_prefix(key)?.trim_start().strip_prefix('='))
        .map(|value| value.trim().trim_matches('"'))
}
