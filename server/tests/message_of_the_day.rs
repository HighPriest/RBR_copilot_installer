//! `/motd`: plain text, an English fallback, no credential, and a 404 the installer treats as offline.

mod common;

#[tokio::test]
async fn motd_serves_text_falls_back_and_404s_when_absent() {
    let server = common::Server::start().await;

    let response = server.get("/motd?lang=en", None).await;
    assert_eq!(response.status(), 200);
    let content_type = response.headers()["content-type"].clone();
    assert!(
        content_type.to_str().unwrap().starts_with("text/plain"),
        "{content_type:?}"
    );
    assert_eq!(response.text().await.unwrap(), "English news");

    for query in ["?lang=pl", "", "?lang=", "?lang=../manifest", "?lang=EN"] {
        let response = server.get(&format!("/motd{query}"), None).await;
        assert_eq!(response.status(), 200, "{query}");
        assert_eq!(response.text().await.unwrap(), "English news", "{query}");
    }

    common::write(
        &server.data.path().join("motd/pl.txt"),
        "Polskie wieści".as_bytes(),
    );
    let response = server.get("/motd?lang=pl", None).await;
    assert_eq!(response.text().await.unwrap(), "Polskie wieści");

    std::fs::remove_dir_all(server.data.path().join("motd")).unwrap();
    assert_eq!(server.status("/motd?lang=en", None).await, 404);
}

#[tokio::test]
async fn motd_needs_no_credential() {
    let server = common::Server::start().await;
    assert_eq!(server.status("/motd?lang=en", None).await, 200);
    assert_eq!(
        server.status("/motd?lang=en", Some("not-a-token")).await,
        200
    );
}
