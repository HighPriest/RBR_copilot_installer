//! `/download/{game}/{directory}/{*name}`: the bytes, and the 404 for anything the manifest does not list.

mod common;

#[tokio::test]
async fn download_streams_the_exact_bytes() {
    let server = common::Server::start().await;
    let token = server.mint(&["alpha"]).await;
    let response = server.get("/download/rbr/alpha/secret", Some(&token)).await;

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/zip");
    assert_eq!(response.headers()["content-length"], "11");
    assert!(response.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .contains("secret.zip"));
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"alpha-bytes");
}

#[tokio::test]
async fn unknown_packages_and_traversal_attempts_are_404() {
    let server = common::Server::start().await;
    let token = server.mint(&["alpha"]).await;

    assert_eq!(
        server
            .status("/download/rbr/alpha/nope", Some(&token))
            .await,
        404
    );
    assert_eq!(
        server
            .status("/download/rbr/nosuchdir/x", Some(&token))
            .await,
        404
    );
    assert_eq!(server.status("/download/rbr/nosuchdir/x", None).await, 404);
    for path in [
        "/download/rbr/..%2F..%2Fmanifest",
        "/download/rbr/public/..%2F..%2Fmanifest",
        "/download/rbr/public/%2Fetc%2Fpasswd",
    ] {
        assert_eq!(server.status(path, None).await, 404, "{path}");
    }
}

#[tokio::test]
async fn nested_names_download_through_the_wildcard_route() {
    let server = common::Server::start().await;

    let response = server
        .get(
            &format!("/download/rbr/public/{}", common::NESTED_NAME),
            None,
        )
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-length"], "11");
    let disposition = response.headers()["content-disposition"].clone();
    assert!(
        disposition.to_str().unwrap().contains("Kamil.zip"),
        "{disposition:?}"
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"kamil-bytes");

    let manifest = server.manifest("/packages", None).await;
    let reference = format!("public/{}", common::NESTED_NAME);
    let nested = manifest
        .get(
            pacenotes_shared::GAME_RBR,
            &pacenotes_shared::PackageRef::parse(&reference).unwrap(),
        )
        .expect("nested package listed");
    assert_eq!(nested.category(), pacenotes_shared::CATEGORY_CODRIVER);
    assert_eq!(nested.tree_levels(), vec!["polish", "male"]);
    assert_eq!(nested.leaf(), "Kamil");
}

/// Dot segments arrive percent-encoded, because an HTTP client collapses a literal `../` before sending.
#[tokio::test]
async fn a_nested_name_is_still_only_a_lookup_key() {
    let server = common::Server::start().await;
    for path in [
        "/download/rbr/public/codriver/polish/male",
        "/download/rbr/public/codriver",
        "/download/rbr/public/..%2Fmotd%2Fen",
        "/download/rbr/public/codriver%2F..%2F..%2Falpha%2Fsecret",
        "/download/rbr/public/%2E%2E%2Fmanifest",
        "/download/rbr/motd/en",
        "/download/rbr/public/%2Fetc%2Fpasswd",
    ] {
        assert_eq!(server.status(path, None).await, 404, "{path}");
    }

    let nested_in_alpha = format!("/download/rbr/alpha/{}", common::NESTED_NAME);
    assert_eq!(server.status(&nested_in_alpha, None).await, 404);
}

#[tokio::test]
async fn games_share_one_server_without_bleeding_into_each_other() {
    let server = common::Server::start().await;

    let rbr = server.get("/download/rbr/public/basic", None).await;
    assert_eq!(rbr.status(), 200);
    assert_eq!(rbr.bytes().await.unwrap().as_ref(), b"public-bytes");

    let beamng = server.get("/download/beamng/public/basic", None).await;
    assert_eq!(beamng.status(), 200);
    assert_eq!(beamng.bytes().await.unwrap().as_ref(), b"beamng-bytes");

    for path in [
        "/download/dirtrally2/public/basic",
        "/download/..%2Fmotd/public/basic",
        "/download/RBR/public/basic",
        "/download/rbr%2F..%2Fbeamng/public/basic",
    ] {
        assert_eq!(server.status(path, None).await, 404, "{path}");
    }
}
