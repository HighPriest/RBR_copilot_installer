/// The folder beside the games holding `<lang>.txt` message-of-the-day texts.
pub const MOTD_DIR: &str = "motd";

pub const MOTD_FALLBACK_LANG: &str = "en";

/// `[a-z]{2,8}`: `lang` names a file no manifest lists, so this alphabet is
/// what keeps it inside the motd folder.
pub fn valid_lang(lang: &str) -> bool {
    (2..=8).contains(&lang.len()) && lang.bytes().all(|b| b.is_ascii_lowercase())
}

/// The text for `lang`, else the fallback language's, else `None`.
pub fn read_motd(data_dir: &std::path::Path, lang: &str) -> Option<String> {
    let mut candidates: Vec<&str> = Vec::new();
    if valid_lang(lang) {
        candidates.push(lang);
    } else if !lang.is_empty() {
        tracing::debug!("ignoring motd lang request: not [a-z]{{2,8}}");
    }
    if !candidates.contains(&MOTD_FALLBACK_LANG) {
        candidates.push(MOTD_FALLBACK_LANG);
    }

    candidates.into_iter().find_map(|candidate| {
        let path = data_dir.join(MOTD_DIR).join(format!("{candidate}.txt"));
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "reading motd failed");
                None
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn motd_falls_back_to_english_and_refuses_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert!(super::read_motd(dir, "pl").is_none());

        crate::store::fixture_tree::write(&dir.join("motd/en.txt"), b"English news");
        assert_eq!(super::read_motd(dir, "en").as_deref(), Some("English news"));
        assert_eq!(super::read_motd(dir, "pl").as_deref(), Some("English news"));
        for odd in ["", "../manifest", "en/../en"] {
            assert_eq!(
                super::read_motd(dir, odd).as_deref(),
                Some("English news"),
                "{odd:?}"
            );
        }

        crate::store::fixture_tree::write(&dir.join("motd/pl.txt"), "Polskie wieści".as_bytes());
        assert_eq!(
            super::read_motd(dir, "pl").as_deref(),
            Some("Polskie wieści")
        );
        assert_eq!(super::read_motd(dir, "en").as_deref(), Some("English news"));
    }

    #[test]
    fn lang_validation() {
        for good in ["en", "pt", "ptbr"] {
            assert!(super::valid_lang(good), "{good:?}");
        }
        for bad in ["e", "", "EN", "en-GB", "..", "toolonglanguagetag"] {
            assert!(!super::valid_lang(bad), "{bad:?}");
        }
    }
}
