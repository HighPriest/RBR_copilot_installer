//! What a cache may keep, and a client that already holds the bytes
//! downloading nothing.

mod common;

const BASIC: &str = "/download/rbr/public/basic";

async fn basic_etag(server: &common::Server) -> String {
    let manifest = server.manifest("/packages", None).await;
    let basic = manifest
        .packages
        .iter()
        .find(|package| package.game == "rbr" && package.name == "basic")
        .expect("rbr/public/basic is listed");
    format!("\"{}\"", basic.sha256)
}

async fn get_if_none_match(server: &common::Server, path: &str, etag: &str) -> u16 {
    reqwest::Client::new()
        .get(format!("{}{path}", server.base_url))
        .header("If-None-Match", etag)
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn caches_are_told_what_they_may_keep() {
    let server = common::Server::start().await;
    let key = server.mint(&["alpha"]).await;

    let listing = server.get("/packages", Some(&key)).await;
    assert_eq!(listing.headers()["cache-control"], "private, no-store");
    assert_eq!(listing.headers()["vary"], "Authorization");

    let protected = server.get("/download/rbr/alpha/secret", Some(&key)).await;
    assert_eq!(protected.status(), 200);
    assert_eq!(protected.headers()["cache-control"], "private, no-store");

    let public = server.get(BASIC, None).await;
    assert_eq!(public.headers()["cache-control"], "no-cache");
    assert_eq!(public.headers()["etag"], basic_etag(&server).await.as_str());
}

#[tokio::test]
async fn a_client_that_already_has_the_bytes_gets_304() {
    let server = common::Server::start().await;
    let etag = basic_etag(&server).await;
    assert_eq!(get_if_none_match(&server, BASIC, &etag).await, 304);
    assert_eq!(get_if_none_match(&server, BASIC, "\"stale\"").await, 200);
}

/// Authorization comes first, so a stranger learns nothing from a validator.
#[tokio::test]
async fn a_protected_validator_is_never_confirmed_to_a_stranger() {
    let server = common::Server::start().await;
    assert_eq!(
        get_if_none_match(&server, "/download/rbr/alpha/secret", "*").await,
        401
    );
}
