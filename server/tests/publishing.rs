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
    let generated = pacenotes_server::store::generate(server.data.path()).unwrap();
    let error = generated
        .errors
        .iter()
        .find(|error| error.contains("basic.toml"))
        .expect("the sidecar is reported");
    assert!(error.contains("game"), "{error}");
    assert!(!generated
        .manifest
        .packages
        .iter()
        .any(|package| package.game == "rbr" && package.name == "basic"));
}

/// Anonymous callers get the schemas too, since an installer checks one before it has a key.
#[tokio::test]
async fn the_listing_carries_each_games_install_schema() {
    let server = common::Server::start().await;
    let rbr_schema = std::fs::read(
        server
            .data
            .path()
            .join("rbr")
            .join(pacenotes_shared::SCHEMA_FILE),
    )
    .unwrap();

    let manifest = server.manifest("/packages", None).await;
    assert_eq!(
        manifest.schemas.keys().collect::<Vec<_>>(),
        vec!["beamng", "rbr"]
    );
    assert_eq!(
        manifest.schemas["rbr"].sha256,
        pacenotes_server::store::sha256_bytes(&rbr_schema)
    );

    let rbr = server.manifest("/packages?game=rbr", None).await;
    assert_eq!(rbr.schemas.keys().collect::<Vec<_>>(), vec!["rbr"]);
}

#[tokio::test]
async fn publishing_is_strict_about_the_schema() {
    let server = common::Server::start().await;
    let data = server.data.path();
    common::write(
        &common::package_path(data, "rbr", "public/extras/Bonus.zip"),
        b"bonus",
    );
    common::write(
        &common::package_path(data, "dirtrally2", "public/codriver/x.zip"),
        b"dr2",
    );
    let generated = pacenotes_server::store::generate(data).unwrap();
    assert!(generated
        .errors
        .iter()
        .any(|error| error.contains("dirtrally2/schema.toml")));
    pacenotes_server::store::write_manifest(data, &generated.manifest).unwrap();

    assert_eq!(
        server
            .status("/download/rbr/public/extras/Bonus", None)
            .await,
        404
    );
    let dirt_rally = server.manifest("/packages?game=dirtrally2", None).await;
    assert!(dirt_rally.packages.is_empty());
    assert!(dirt_rally.schemas.is_empty());
}
