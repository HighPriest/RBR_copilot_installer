//! `/whoami` answers "is this key good?", which `/packages` never can, since it
//! falls back to the public listing for any key it does not accept.

mod common;

#[tokio::test]
async fn a_live_key_reports_its_expiry() {
    let server = common::Server::start().await;
    let live = server.mint(&["alpha"]).await;

    let response = server.get("/whoami", Some(&live)).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status: pacenotes_shared::KeyStatus = response.json().await.unwrap();
    assert_eq!(status.expires_at, None);

    let expiring =
        pacenotes_server::admin::mint(&server.connection, Some("expiring"), &[], Some(30))
            .await
            .unwrap();
    let status: pacenotes_shared::KeyStatus = server
        .get("/whoami", Some(&expiring))
        .await
        .json()
        .await
        .unwrap();
    assert!(status
        .expires_at
        .is_some_and(|deadline| deadline > pacenotes_server::auth::now()));
}

#[tokio::test]
async fn every_kind_of_bad_key_is_the_same_401() {
    let server = common::Server::start().await;
    let a_day_ago = Some(-1);
    let expired =
        pacenotes_server::admin::mint(&server.connection, None, &["alpha".to_string()], a_day_ago)
            .await
            .unwrap();
    let revoked = server.mint(&["alpha"]).await;
    pacenotes_server::admin::revoke(&server.connection, &revoked)
        .await
        .unwrap();
    let never_minted = format!("pcn_{}", "A".repeat(43));

    for (case, token) in [
        ("no key", None),
        ("malformed", Some("not-a-token")),
        ("never minted", Some(never_minted.as_str())),
        ("expired", Some(expired.as_str())),
        ("revoked", Some(revoked.as_str())),
    ] {
        let response = server.get("/whoami", token).await;
        assert_eq!(response.status(), 401, "{case}");
        let body: pacenotes_shared::ApiError = response.json().await.unwrap();
        assert_eq!(body.code, "unauthorized", "{case}");
    }
}
