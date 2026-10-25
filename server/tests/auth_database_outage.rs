//! A failing auth database must not take the free catalogue down, and must
//! never hand out protected bytes.

mod common;

#[tokio::test]
async fn a_database_outage_never_takes_the_public_catalogue_down() {
    let server = common::Server::start().await;
    let live = server.mint(&["alpha"]).await;
    server
        .connection
        .execute_batch("DROP TABLE grants; DROP TABLE tokens;")
        .await
        .unwrap();

    assert_eq!(server.status("/packages", Some(&live)).await, 200);
    assert_eq!(
        server.visible_directories(Some(&live)).await,
        vec!["public"]
    );
    assert_eq!(
        server
            .status("/download/rbr/public/basic", Some(&live))
            .await,
        200
    );

    assert_eq!(
        server
            .status("/download/rbr/alpha/secret", Some(&live))
            .await,
        500,
        "a protected download fails closed"
    );
    assert_eq!(
        server
            .status("/download/rbr/alpha/secret", Some("not-a-token"))
            .await,
        401,
        "a malformed key is turned away before any query"
    );
}
