//! Whatever a caller can list they can download, and nothing else.

mod common;

#[tokio::test]
async fn listing_and_download_agree_across_the_grant_matrix() {
    let server = common::Server::start().await;
    let no_grants = server.mint(&[]).await;
    let alpha_only = server.mint(&["alpha"]).await;
    let both = server.mint(&["alpha", "beta"]).await;

    assert_eq!(server.visible_directories(None).await, vec!["public"]);
    assert_eq!(server.download_statuses(None).await, [200, 401, 401]);

    assert_eq!(
        server.visible_directories(Some(&no_grants)).await,
        vec!["public"]
    );
    assert_eq!(
        server.download_statuses(Some(&no_grants)).await,
        [200, 403, 403]
    );

    assert_eq!(
        server.visible_directories(Some(&alpha_only)).await,
        vec!["alpha", "public"]
    );
    assert_eq!(
        server.download_statuses(Some(&alpha_only)).await,
        [200, 200, 403]
    );

    assert_eq!(
        server.visible_directories(Some(&both)).await,
        vec!["alpha", "beta", "public"]
    );
    assert_eq!(server.download_statuses(Some(&both)).await, [200, 200, 200]);
}

#[tokio::test]
async fn a_bad_credential_is_never_more_than_anonymous() {
    let server = common::Server::start().await;
    let never_minted = "pcn_0000000000000000000000000000000000000000000";
    for token in ["not-a-token", never_minted] {
        assert_eq!(
            server.visible_directories(Some(token)).await,
            vec!["public"]
        );
        assert_eq!(server.download_statuses(Some(token)).await, [200, 401, 401]);
    }
}

#[tokio::test]
async fn expired_and_revoked_credentials_lose_access() {
    let server = common::Server::start().await;
    let a_day_ago = Some(-1);
    let expired = pacenotes_server::admin::mint(
        &server.connection,
        Some("expired"),
        &["alpha".to_string()],
        a_day_ago,
    )
    .await
    .unwrap();
    assert_eq!(
        server.visible_directories(Some(&expired)).await,
        vec!["public"]
    );
    assert_eq!(
        server.download_statuses(Some(&expired)).await,
        [200, 401, 401]
    );

    let revoked = server.mint(&["alpha"]).await;
    assert_eq!(
        server.visible_directories(Some(&revoked)).await,
        vec!["alpha", "public"]
    );
    pacenotes_server::admin::revoke(&server.connection, &revoked)
        .await
        .unwrap();
    assert_eq!(
        server.visible_directories(Some(&revoked)).await,
        vec!["public"]
    );
    assert_eq!(
        server.download_statuses(Some(&revoked)).await,
        [200, 401, 401]
    );
}

#[tokio::test]
async fn ungrant_takes_effect_without_a_restart() {
    let server = common::Server::start().await;
    let token = server.mint(&["alpha", "beta"]).await;
    assert_eq!(
        server.download_statuses(Some(&token)).await,
        [200, 200, 200]
    );

    pacenotes_server::admin::ungrant(&server.connection, &token, "beta")
        .await
        .unwrap();

    assert_eq!(
        server.download_statuses(Some(&token)).await,
        [200, 200, 403]
    );
    assert_eq!(
        server.visible_directories(Some(&token)).await,
        vec!["alpha", "public"]
    );
}

#[tokio::test]
async fn error_bodies_are_api_errors() {
    let server = common::Server::start().await;
    let no_grants = server.mint(&[]).await;

    for (path, token, code) in [
        ("/download/rbr/alpha/secret", None, "unauthorized"),
        (
            "/download/rbr/alpha/secret",
            Some(no_grants.as_str()),
            "forbidden",
        ),
        ("/download/rbr/alpha/nope", None, "not_found"),
    ] {
        let error: pacenotes_shared::ApiError = server.get(path, token).await.json().await.unwrap();
        assert_eq!(error.code, code);
        assert!(!error.message.is_empty());
    }
}

#[tokio::test]
async fn health_needs_no_auth() {
    let server = common::Server::start().await;
    let response = server.get("/health", None).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
}
