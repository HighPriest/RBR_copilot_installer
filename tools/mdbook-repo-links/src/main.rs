//! `mdbook-repo-links`: an mdBook preprocessor for a book whose chapters
//! include documents from elsewhere in the repository.
//!
//! The book mirrors the repository's layout — `docs/src/installer/DESIGN.md`
//! includes `installer/DESIGN.md` — so a relative link written for the
//! original resolves the same way in the book. What the mirror cannot hold
//! are the targets that are not chapters: source files, scripts, the licence.
//! In the book those would be dead links; this points each one at the file
//! on GitHub instead, and leaves every other link as it is.
//!
//! Configured in `book.toml`:
//!
//! ```toml
//! [preprocessor.repo-links]
//! command = "cargo run --quiet --locked -p mdbook-repo-links --"
//! repository = "https://github.com/<owner>/<repo>"
//! branch = "master"     # default
//! repo-root = ".."      # from the book's root; default
//! ```

mod links;
mod resolver;

fn main() -> std::process::ExitCode {
    // mdBook asks first whether a renderer is supported; a plain link works in all of them.
    if std::env::args().nth(1).as_deref() == Some("supports") {
        return std::process::ExitCode::SUCCESS;
    }
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("mdbook-repo-links: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Reads `[context, book]` from stdin and writes the book back to stdout.
fn run() -> Result<(), String> {
    let input: serde_json::Value = serde_json::from_reader(std::io::stdin().lock())
        .map_err(|e| format!("cannot read mdBook's input: {e}"))?;
    let serde_json::Value::Array(mut parts) = input else {
        return Err("mdBook's input is not [context, book]".into());
    };
    if parts.len() != 2 {
        return Err("mdBook's input is not [context, book]".into());
    }
    let mut book = parts.pop().unwrap_or_default();
    let context = parts.pop().unwrap_or_default();
    let config = resolver::Config::from_context(&context)?;
    rewrite_book(&mut book, &config);
    serde_json::to_writer(std::io::stdout().lock(), &book)
        .map_err(|e| format!("cannot write the book back: {e}"))
}

/// Every chapter, wherever it sits: the walk is over the JSON itself, so a
/// change in how mdBook nests sections between versions does not matter.
fn rewrite_book(value: &mut serde_json::Value, config: &resolver::Config) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(chapter)) = map.get_mut("Chapter") {
                rewrite_chapter(chapter, config);
            }
            for child in map.values_mut() {
                rewrite_book(child, config);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                rewrite_book(item, config);
            }
        }
        _ => {}
    }
}

fn rewrite_chapter(
    chapter: &mut serde_json::Map<String, serde_json::Value>,
    config: &resolver::Config,
) {
    // A draft chapter has no file, and nothing to rewrite.
    let Some(source) = chapter
        .get("source_path")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let resolver = resolver::Resolver::for_chapter(config, std::path::Path::new(source));
    if let Some(serde_json::Value::String(content)) = chapter.get_mut("content") {
        *content = links::rewrite_links(content, &|target| resolver.resolve(target));
    }
}
