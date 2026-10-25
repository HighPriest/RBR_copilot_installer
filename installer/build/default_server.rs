const ENV_FILE: &str = ".env";
const DEFAULT_SERVER_KEY: &str = "PACENOTES_DEFAULT_SERVER";

/// The server address lives in the gitignored `.env`, so a deployment's
/// address never enters the repository; without it the build fails.
pub fn export_default_server() {
    println!("cargo:rerun-if-changed={ENV_FILE}");
    let text = std::fs::read_to_string(ENV_FILE).unwrap_or_else(|_| {
        panic!(
            "installer/{ENV_FILE} is missing: copy installer/.env.example to it and set \
             {DEFAULT_SERVER_KEY} (see DEVELOPMENT.md)"
        )
    });
    let server = env_value(&text, DEFAULT_SERVER_KEY)
        .filter(|server| !server.is_empty())
        .unwrap_or_else(|| panic!("installer/{ENV_FILE} does not set {DEFAULT_SERVER_KEY}"));
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
