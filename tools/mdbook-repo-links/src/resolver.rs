//! Where a link target written in a chapter lands: in the book, which leaves
//! it alone, or in the repository, which points it at GitHub.

pub struct Config {
    /// The book's `src`, where the chapters are.
    book_src: std::path::PathBuf,
    repo_root: std::path::PathBuf,
    repository: String,
    branch: String,
}

impl Config {
    /// From mdBook's context: its `root`, `book.src`, and this preprocessor's table.
    pub fn from_context(context: &serde_json::Value) -> Result<Config, String> {
        let root = context
            .get("root")
            .and_then(serde_json::Value::as_str)
            .ok_or("mdBook's context names no root")?;
        let root = std::path::Path::new(root);
        let config = context.get("config");
        let src = config
            .and_then(|config| config.pointer("/book/src"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("src");
        let table = config.and_then(|config| config.pointer("/preprocessor/repo-links"));
        let setting = |key: &str| {
            table
                .and_then(|table| table.get(key))
                .and_then(serde_json::Value::as_str)
        };
        let repository = setting("repository")
            .ok_or("book.toml sets no [preprocessor.repo-links] repository")?
            .trim_end_matches('/')
            .to_string();
        let repo_root = root.join(setting("repo-root").unwrap_or(".."));
        Ok(Config {
            book_src: canonical(&root.join(src))?,
            repo_root: canonical(&repo_root)?,
            repository,
            branch: setting("branch").unwrap_or("master").to_string(),
        })
    }
}

fn canonical(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The two places one chapter's relative links can mean.
pub struct Resolver<'a> {
    config: &'a Config,
    /// The chapter's directory in the book.
    in_book: std::path::PathBuf,
    /// The same directory as a path from the repository root: the original's
    /// directory when the chapter includes a file from there, otherwise the
    /// chapter's own place under the book.
    in_repo: std::path::PathBuf,
}

impl<'a> Resolver<'a> {
    pub fn for_chapter(config: &'a Config, source: &std::path::Path) -> Resolver<'a> {
        let directory = source.parent().unwrap_or(std::path::Path::new(""));
        let in_book = directory.to_path_buf();
        let mirrors_the_repository = config.repo_root.join(source).is_file();
        let in_repo = if mirrors_the_repository {
            directory.to_path_buf()
        } else {
            let book = config
                .book_src
                .strip_prefix(&config.repo_root)
                .unwrap_or(std::path::Path::new(""));
            book.join(directory)
        };
        Resolver {
            config,
            in_book,
            in_repo,
        }
    }

    /// The GitHub address for a target that is in the repository but not in
    /// the book; `None` for anything else, which stays as written.
    pub fn resolve(&self, target: &str) -> Option<String> {
        let external = target.contains("://") || target.starts_with("mailto:");
        if target.is_empty() || external || target.starts_with(['#', '/']) {
            return None;
        }
        let (path, anchor) = match target.split_once('#') {
            Some((path, anchor)) => (path, format!("#{anchor}")),
            None => (target, String::new()),
        };
        // Lexically, so a path that climbs out of the book into the
        // repository is not taken for a page of the book.
        let in_book = normalize(&self.in_book.join(path))
            .is_some_and(|page| self.config.book_src.join(page).exists());
        if path.is_empty() || in_book {
            return None;
        }
        let relative = normalize(&self.in_repo.join(path))?;
        let file = self.config.repo_root.join(&relative);
        let kind = if file.is_dir() {
            "tree"
        } else if file.is_file() {
            "blob"
        } else {
            return None;
        };
        Some(format!(
            "{}/{kind}/{}/{relative}{anchor}",
            self.config.repository, self.config.branch
        ))
    }
}

/// `path` with `.` and `..` resolved, `/`-separated; `None` when it climbs
/// out of the repository.
fn normalize(path: &std::path::Path) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            std::path::Component::ParentDir => {
                parts.pop()?;
            }
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    /// A repository with a book under `docs/src` mirroring `installer/DESIGN.md`.
    struct Fixture {
        root: std::path::PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Fixture {
            let root =
                std::env::temp_dir().join(format!("repo-links-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for file in [
                "installer/DESIGN.md",
                "installer/src/zip.rs",
                "LICENSE",
                "docs/src/installer/DESIGN.md",
                "docs/src/architecture/README.md",
                "docs/src/architecture/diagrams/er.svg",
            ] {
                let path = root.join(file);
                std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
                std::fs::write(&path, "x").expect("write");
            }
            Fixture { root }
        }

        fn config(&self) -> super::Config {
            let context = serde_json::json!({
                "root": self.root.join("docs"),
                "config": { "preprocessor": { "repo-links": {
                    "repository": "https://github.com/o/r/",
                } } }
            });
            super::Config::from_context(&context).expect("config")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_mirrored_chapter_resolves_against_the_original() {
        let fixture = Fixture::new("mirror");
        let config = fixture.config();
        let chapter = super::Resolver::for_chapter(&config, "installer/DESIGN.md".as_ref());
        assert_eq!(
            chapter.resolve("src/zip.rs#L3").as_deref(),
            Some("https://github.com/o/r/blob/master/installer/src/zip.rs#L3")
        );
        assert_eq!(
            chapter.resolve("../LICENSE").as_deref(),
            Some("https://github.com/o/r/blob/master/LICENSE")
        );
        assert_eq!(
            chapter.resolve("src/").as_deref(),
            Some("https://github.com/o/r/tree/master/installer/src")
        );
        // Chapters, external links and anchors stay as written.
        assert_eq!(chapter.resolve("DESIGN.md"), None);
        assert_eq!(chapter.resolve("https://example.test/x"), None);
        assert_eq!(chapter.resolve("#scene-4"), None);
        assert_eq!(chapter.resolve("missing.rs"), None);
        assert_eq!(chapter.resolve("../../../outside"), None);
    }

    #[test]
    fn a_chapter_written_for_the_book_resolves_from_its_own_place() {
        let fixture = Fixture::new("authored");
        let config = fixture.config();
        let chapter = super::Resolver::for_chapter(&config, "architecture/README.md".as_ref());
        assert_eq!(chapter.resolve("diagrams/er.svg"), None);
        assert_eq!(
            chapter.resolve("../../../installer/src/zip.rs").as_deref(),
            Some("https://github.com/o/r/blob/master/installer/src/zip.rs")
        );
    }
}
