const RELEASE_TAG_KEY: &str = "PACENOTES_RELEASE_TAG";
const RELEASE_TAG_PREFIX: &str = "v/";
const VERSION_KEY: &str = "PACENOTES_VERSION";
const DEVELOPMENT_VERSION: &str = "0.0.0-dev";
const HEADER_FILE: &str = "pacenotes_version.h";

/// The version every place shows comes from the release tag CI builds from, so
/// none is typed by hand; without a tag the build is a development one.
///
/// Exports `PACENOTES_VERSION` to the code and writes the header the `.rc`
/// includes for VERSIONINFO, so the two cannot disagree. Must run before the
/// `.rc` is compiled.
pub fn export_release_version() {
    println!("cargo:rerun-if-env-changed={RELEASE_TAG_KEY}");
    let (numbers, product_version) = match std::env::var(RELEASE_TAG_KEY) {
        Ok(tag) => {
            let numbers = release_numbers(&tag).unwrap_or_else(|| {
                panic!(
                    "{RELEASE_TAG_KEY} is \"{tag}\"; a release tag is {RELEASE_TAG_PREFIX}X.Y.Z \
                     with each part 0..=65535"
                )
            });
            let [major, minor, patch] = numbers;
            (numbers, format!("{major}.{minor}.{patch}"))
        }
        Err(_) => ([0; 3], DEVELOPMENT_VERSION.to_string()),
    };
    println!("cargo:rustc-env={VERSION_KEY}={product_version}");
    write_header(numbers, &product_version);
}

/// `v/1.2.3` as its three numbers; each is a 16-bit word in VERSIONINFO.
fn release_numbers(tag: &str) -> Option<[u16; 3]> {
    let mut parts = tag.strip_prefix(RELEASE_TAG_PREFIX)?.split('.');
    let mut numbers = [0u16; 3];
    for number in &mut numbers {
        let part = parts.next()?;
        // `parse` alone would accept a leading `+`.
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(numbers)
}

fn write_header(numbers: [u16; 3], product_version: &str) {
    let [major, minor, patch] = numbers;
    let header = format!(
        "#define PACENOTES_VERSION_NUMBERS {major},{minor},{patch},0\n\
         #define PACENOTES_FILE_VERSION \"{major}.{minor}.{patch}.0\"\n\
         #define PACENOTES_PRODUCT_VERSION \"{product_version}\"\n"
    );
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let path = std::path::Path::new(&out_dir).join(HEADER_FILE);
    std::fs::write(&path, header)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}
