pub fn check_health(base_url: String) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let online = crate::api::Api::new(&base_url, None).health(&cancel);
    super::Message::Health { online }
}

pub fn fetch_motd(base_url: String, lang: &'static str) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    super::Message::Motd(crate::api::Api::new(&base_url, None).fetch_motd(lang, &cancel))
}

/// Only a 401 stops the user, so a flaky network never strands someone whose
/// packages are already cached.
pub fn check_secret(base_url: String, secret: String) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let api = crate::api::Api::new(&base_url, Some(secret));
    super::Message::SecretChecked(api.whoami(&cancel))
}

pub fn fetch_packages(
    base_url: String,
    token: Option<String>,
    game: &'static str,
) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match crate::api::Api::new(&base_url, token).fetch_manifest(game, &cancel) {
        Ok(manifest) => super::Message::Packages(Ok(manifest)),
        Err(e) => super::Message::Packages(Err(e.message)),
    }
}
