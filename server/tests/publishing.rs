//! Publishing: drop files in, run `manifest gen`, and the running server serves the result.

mod common;

#[tokio::test]
async fn manifest_changes_are_picked_up_without_a_restart() {
    let server = common::Server::start().await;
    let before = server.listed(None).await.len();
    assert_eq!(server.status("/download/rbr/public/extra", None).await, 404);

    common::write(
        &common::package_path(server.data.path(), "rbr", "public/extra.zip"),
        b"extra-bytes",
    );
    common::regenerate(server.data.path());

    assert_eq!(server.listed(None).await.len(), before + 1);
    assert_eq!(server.status("/download/rbr/public/extra", None).await, 200);
}

#[tokio::test]
async fn packages_lists_every_game_and_filters_on_request() {
    let server = common::Server::start().await;

    let everything = server.manifest("/packages", None).await;
    assert!(everything
        .packages
        .iter()
        .any(|package| package.game == "rbr"));
    assert!(everything
        .packages
        .iter()
        .any(|package| package.game == "beamng"));
    assert!(everything
        .packages
        .iter()
        .all(|package| package.is_public()));

    let rbr = server.manifest("/packages?game=rbr", None).await;
    assert!(!rbr.packages.is_empty());
    assert!(rbr.packages.iter().all(|package| package.game == "rbr"));

    let beamng = server.manifest("/packages?game=beamng", None).await;
    assert_eq!(beamng.packages.len(), 1);
    assert_eq!(beamng.packages[0].download_path(), "beamng/public/basic");

    let unknown = server.manifest("/packages?game=nosuchgame", None).await;
    assert!(unknown.packages.is_empty());
}

#[tokio::test]
async fn the_game_comes_from_the_layout_not_a_sidecar() {
    let server = common::Server::start().await;
    common::write(
        &common::package_path(server.data.path(), "rbr", "public/basic.toml"),
        b"game = \"beamng\"\n",
    );
    let error = pacenotes_server::store::generate(server.data.path())
        .unwrap_err()
        .to_string();
    assert!(error.contains("basic.toml"), "{error}");
    assert!(error.contains("game"), "{error}");
}
