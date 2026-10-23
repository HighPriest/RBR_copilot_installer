pub fn check_health(base_url: String) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let online = crate::api::Api::new(&base_url, None).health(&cancel);
    super::Message::Health { online }
}

pub fn fetch_motd(base_url: String, lang: &'static str) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    super::Message::Motd(crate::api::Api::new(&base_url, None).fetch_motd(lang, &cancel))
}

/// Asks for a listing with the secret; only a 401 stops the user, so a flaky
/// network never strands someone whose packages are already cached.
pub fn check_secret(base_url: String, secret: String) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let api = crate::api::Api::new(&base_url, Some(secret));
    let any_game = pacenotes_shared::GAME_RBR;
    let outcome = match api.fetch_manifest(any_game, &cancel) {
        Ok(_) => super::SecretOutcome::Accepted,
        Err(e) if e.status == Some(401) => super::SecretOutcome::Rejected,
        Err(_) => super::SecretOutcome::Unreachable,
    };
    super::Message::SecretChecked(outcome)
}

pub fn fetch_packages(
    base_url: String,
    token: Option<String>,
    game: &'static str,
) -> super::Message {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match crate::api::Api::new(&base_url, token).fetch_manifest(game, &cancel) {
        Ok(manifest) => super::Message::Packages(Ok(manifest.packages)),
        Err(e) => super::Message::Packages(Err(e.message)),
    }
}
