//! Integration tests (task S8): a real server on an ephemeral port, a real
//! data tree in a tempdir and a real (in-memory) auth DB.
//!
//! The tree is the shipped layout — `<data>/<game>/packages/<directory>/…`
//! with a second game present — so these tests also pin down that one
//! server serves every game without the games bleeding into each other.
//!
//! The matrix that matters is {anonymous, no-grants, dirA, dirA+dirB} ×
//! {public, dirA, dirB} for BOTH `/packages` filtering and `/download`
//! status codes, because those two must never disagree: whatever a caller
//! can see, they can fetch, and whatever they cannot see, they cannot fetch.
//!
//! CONVENTION (root CLAUDE.md): full paths at the point of use.

struct Server {
    base_url: String,
    connection: libsql::Connection,
    _data: tempfile::TempDir,
}

impl Server {
    /// RBR with one package per access directory (`public`, `alpha`, `beta`)
    /// plus a deeply nested one; a second game sharing the same
    /// directory AND name; a MOTD beside the games.
    async fn start() -> Server {
        let data = tempfile::tempdir().unwrap();
        write(
            &zip(data.path(), "rbr", "public/basic.zip"),
            b"public-bytes",
        );
        write(&zip(data.path(), "rbr", "alpha/secret.zip"), b"alpha-bytes");
        write(&zip(data.path(), "rbr", "beta/other.zip"), b"beta-bytes");
        // The nested layout the presentation model depends on.
        write(
            &zip(data.path(), "rbr", "public/codriver/polish/male/Kamil.zip"),
            b"kamil-bytes",
        );
        // Same directory and same name under another game: only the game
        // keeps these two apart.
        write(
            &zip(data.path(), "beamng", "public/basic.zip"),
            b"beamng-bytes",
        );
        write(&data.path().join("motd/en.txt"), b"English news");
        regenerate(data.path());

        let connection = pacenotes_server::db::open(":memory:").await.unwrap();
        let state = std::sync::Arc::new(pacenotes_server::routes::AppState {
            store: pacenotes_server::store::ManifestStore::new(data.path()),
            db: connection.clone(),
        });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(listener, pacenotes_server::routes::router(state))
                .await
                .unwrap();
        });

        Server {
            base_url,
            connection,
            _data: data,
        }
    }

    async fn mint(&self, grants: &[&str]) -> String {
        let grants: Vec<String> = grants.iter().map(|g| g.to_string()).collect();
        pacenotes_server::admin::mint(&self.connection, Some("test"), &grants, None)
            .await
            .unwrap()
    }

    async fn get(&self, path: &str, token: Option<&str>) -> reqwest::Response {
        let request = reqwest::Client::new().get(format!("{}{path}", self.base_url));
        let request = match token {
            Some(t) => request.bearer_auth(t),
            None => request,
        };
        request.send().await.unwrap()
    }

    /// Every package `/packages` returns, as "directory/name" refs.
    async fn listed(&self, token: Option<&str>) -> Vec<String> {
        let manifest: pacenotes_shared::Manifest =
            self.get("/packages", token).await.json().await.unwrap();
        let mut refs: Vec<String> = manifest
            .packages
            .iter()
            .map(|p| p.package_ref().to_string())
            .collect();
        refs.sort();
        refs
    }

    /// Which access directories `/packages` reveals, deduplicated and sorted
    /// — the listing half of the matrix. Deduplicated because a directory
    /// holds many packages (the fixture's `public` holds two) and what the
    /// matrix is about is which DIRECTORIES leak, not how many files sit in
    /// them.
    async fn visible(&self, token: Option<&str>) -> Vec<String> {
        let manifest: pacenotes_shared::Manifest =
            self.get("/packages", token).await.json().await.unwrap();
        let mut directories: Vec<String> =
            manifest.packages.into_iter().map(|p| p.directory).collect();
        directories.sort();
        directories.dedup();
        directories
    }

    /// Status of downloading each directory's single package — the access
    /// half of the matrix.
    async fn download_statuses(&self, token: Option<&str>) -> [u16; 3] {
        [
            self.get("/download/rbr/public/basic", token)
                .await
                .status()
                .as_u16(),
            self.get("/download/rbr/alpha/secret", token)
                .await
                .status()
                .as_u16(),
            self.get("/download/rbr/beta/other", token)
                .await
                .status()
                .as_u16(),
        ]
    }
}

/// A package name with `/` in it has to survive the URL as segments, which
/// is the whole point of the wildcard route.
const NESTED: &str = "codriver/polish/male/Kamil";

/// `<data>/<game>/packages/<rest>`.
fn zip(data: &std::path::Path, game: &str, rest: &str) -> std::path::PathBuf {
    data.join(game)
        .join(pacenotes_server::store::PACKAGES_DIR)
        .join(rest)
}

/// What a publisher runs after dropping files in.
fn regenerate(data: &std::path::Path) {
    let generated = pacenotes_server::store::ManifestStore::generate(data).unwrap();
    write(
        &data.join("manifest.json"),
        serde_json::to_string(&generated.manifest)
            .unwrap()
            .as_bytes(),
    );
}

fn write(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[tokio::test]
async fn health_needs_no_auth() {
    let server = Server::start().await;
    let response = server.get("/health", None).await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn listing_and_download_agree_across_the_grant_matrix() {
    let server = Server::start().await;
    let no_grants = server.mint(&[]).await;
    let alpha_only = server.mint(&["alpha"]).await;
    let both = server.mint(&["alpha", "beta"]).await;

    // Anonymous: public only. Protected packages exist but need a credential.
    assert_eq!(server.visible(None).await, vec!["public"]);
    assert_eq!(server.download_statuses(None).await, [200, 401, 401]);

    // A credential with no grants is valid — and sees exactly what anonymous
    // sees. The difference is the status: it is a known credential without
    // the grant, which is 403, not 401.
    assert_eq!(server.visible(Some(&no_grants)).await, vec!["public"]);
    assert_eq!(
        server.download_statuses(Some(&no_grants)).await,
        [200, 403, 403]
    );

    assert_eq!(
        server.visible(Some(&alpha_only)).await,
        vec!["alpha", "public"]
    );
    assert_eq!(
        server.download_statuses(Some(&alpha_only)).await,
        [200, 200, 403]
    );

    assert_eq!(
        server.visible(Some(&both)).await,
        vec!["alpha", "beta", "public"]
    );
    assert_eq!(server.download_statuses(Some(&both)).await, [200, 200, 200]);
}

#[tokio::test]
async fn download_streams_the_exact_bytes() {
    let server = Server::start().await;
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
    let server = Server::start().await;
    let token = server.mint(&["alpha"]).await;

    // A package that does not exist is 404 whether or not the caller could
    // have downloaded it — the status must not reveal which directories
    // exist.
    assert_eq!(
        server
            .get("/download/rbr/alpha/nope", Some(&token))
            .await
            .status(),
        404
    );
    assert_eq!(
        server
            .get("/download/rbr/nosuchdir/x", Some(&token))
            .await
            .status(),
        404
    );
    assert_eq!(
        server.get("/download/rbr/nosuchdir/x", None).await.status(),
        404
    );

    // directory/name are manifest keys, never path fragments.
    for path in [
        "/download/rbr/..%2F..%2Fmanifest",
        "/download/rbr/public/..%2F..%2Fmanifest",
        "/download/rbr/public/%2Fetc%2Fpasswd",
    ] {
        assert_eq!(server.get(path, None).await.status(), 404, "{path}");
    }
}

#[tokio::test]
async fn a_bad_credential_is_never_more_than_anonymous() {
    let server = Server::start().await;

    // Garbage, and a well-formed key that was never minted.
    for token in [
        "not-a-token",
        "pcn_0000000000000000000000000000000000000000000",
    ] {
        assert_eq!(server.visible(Some(token)).await, vec!["public"]);
        assert_eq!(server.download_statuses(Some(token)).await, [200, 401, 401]);
    }
}

#[tokio::test]
async fn expired_and_revoked_credentials_lose_access() {
    let server = Server::start().await;

    let expired = pacenotes_server::admin::mint(
        &server.connection,
        Some("expired"),
        &["alpha".to_string()],
        Some(-1), // deadline a day in the past
    )
    .await
    .unwrap();
    // /packages never 4xxs: an expired credential silently sees the
    // anonymous listing...
    assert_eq!(server.visible(Some(&expired)).await, vec!["public"]);
    // ...and /download reports 401, like a missing credential.
    assert_eq!(
        server.download_statuses(Some(&expired)).await,
        [200, 401, 401]
    );

    let revoked = server.mint(&["alpha"]).await;
    assert_eq!(
        server.visible(Some(&revoked)).await,
        vec!["alpha", "public"]
    );
    pacenotes_server::admin::revoke(&server.connection, &revoked)
        .await
        .unwrap();
    assert_eq!(server.visible(Some(&revoked)).await, vec!["public"]);
    assert_eq!(
        server.download_statuses(Some(&revoked)).await,
        [200, 401, 401]
    );
}

#[tokio::test]
async fn ungrant_takes_effect_without_a_restart() {
    let server = Server::start().await;
    let token = server.mint(&["alpha", "beta"]).await;
    assert_eq!(
        server.download_statuses(Some(&token)).await,
        [200, 200, 200]
    );

    pacenotes_server::admin::ungrant(&server.connection, &token, "beta")
        .await
        .unwrap();

    // Grants are read per request, never cached in the process.
    assert_eq!(
        server.download_statuses(Some(&token)).await,
        [200, 200, 403]
    );
    assert_eq!(server.visible(Some(&token)).await, vec!["alpha", "public"]);
}

#[tokio::test]
async fn error_bodies_are_api_errors() {
    let server = Server::start().await;
    let no_grants = server.mint(&[]).await;

    let unauthorized: pacenotes_shared::ApiError = server
        .get("/download/rbr/alpha/secret", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(unauthorized.code, "unauthorized");
    assert!(!unauthorized.message.is_empty());

    let forbidden: pacenotes_shared::ApiError = server
        .get("/download/rbr/alpha/secret", Some(&no_grants))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(forbidden.code, "forbidden");

    let not_found: pacenotes_shared::ApiError = server
        .get("/download/rbr/alpha/nope", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(not_found.code, "not_found");
}

#[tokio::test]
async fn manifest_changes_are_picked_up_without_a_restart() {
    let server = Server::start().await;
    let before = server.listed(None).await.len();
    assert_eq!(
        server
            .get("/download/rbr/public/extra", None)
            .await
            .status(),
        404
    );

    // Publish another public package the way a publisher would: drop the zip
    // in, regenerate the manifest.
    write(
        &zip(server._data.path(), "rbr", "public/extra.zip"),
        b"extra-bytes",
    );
    regenerate(server._data.path());

    // The store notices the new manifest mtime on the next request.
    assert_eq!(server.listed(None).await.len(), before + 1);
    assert_eq!(
        server
            .get("/download/rbr/public/extra", None)
            .await
            .status(),
        200
    );
}

#[tokio::test]
async fn nested_names_download_through_the_wildcard_route() {
    let server = Server::start().await;

    // The nested package is public, so anonymous gets it...
    let response = server
        .get(&format!("/download/rbr/public/{NESTED}"), None)
        .await;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-length"], "11");
    // ...with the LEAF as the filename, not the whole path.
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("Kamil.zip"),
        "{:?}",
        response.headers()["content-disposition"]
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"kamil-bytes");

    // ...and it is listed with its path as `name`, which is what lets the
    // installer build the tree.
    let manifest: pacenotes_shared::Manifest =
        server.get("/packages", None).await.json().await.unwrap();
    let nested = manifest
        .get(&pacenotes_shared::PackageRef::parse(&format!("public/{NESTED}")).unwrap())
        .expect("nested package listed");
    assert_eq!(nested.category(), pacenotes_shared::CATEGORY_CODRIVER);
    assert_eq!(nested.tree_levels(), vec!["polish", "male"]);
    assert_eq!(nested.leaf(), "Kamil");
}

#[tokio::test]
async fn a_nested_name_is_still_only_a_lookup_key() {
    let server = Server::start().await;

    // Each of these names a real file on disk, or a prefix of one, and none
    // is a manifest entry — so each is 404. The `..` cases are
    // percent-encoded on purpose: an HTTP client collapses dot segments
    // before sending, so a literal `../` never reaches the server as one,
    // and `%2F`/`%2E` is how a traversal attempt actually arrives.
    for path in [
        // Interior nodes of the tree are not packages.
        "/download/rbr/public/codriver/polish/male",
        "/download/rbr/public/codriver",
        // Reaching out of the access directory.
        "/download/rbr/public/..%2Fmotd%2Fen",
        "/download/rbr/public/codriver%2F..%2F..%2Falpha%2Fsecret",
        "/download/rbr/public/%2E%2E%2Fmanifest",
        // The MOTD directory is not an access directory.
        "/download/rbr/motd/en",
        // Absolute-looking names.
        "/download/rbr/public/%2Fetc%2Fpasswd",
    ] {
        assert_eq!(server.get(path, None).await.status(), 404, "{path}");
    }

    // Filing a package deeper does not make it public: `alpha` has no nested
    // package, so this is 404 (not 401) — the status must not reveal which
    // names exist inside a directory the caller cannot see.
    assert_eq!(
        server
            .get(&format!("/download/rbr/alpha/{NESTED}"), None)
            .await
            .status(),
        404
    );
}

#[tokio::test]
async fn motd_serves_text_falls_back_and_404s_when_absent() {
    let server = Server::start().await;

    // Explicit language that exists.
    let response = server.get("/motd?lang=en", None).await;
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain"),
        "{:?}",
        response.headers()["content-type"]
    );
    assert_eq!(response.text().await.unwrap(), "English news");

    // A language with no file falls back to English; so does a missing or
    // malformed one. None of them reaches the filesystem as a path.
    for query in ["?lang=pl", "", "?lang=", "?lang=../manifest", "?lang=EN"] {
        let response = server.get(&format!("/motd{query}"), None).await;
        assert_eq!(response.status(), 200, "{query}");
        assert_eq!(response.text().await.unwrap(), "English news", "{query}");
    }

    // Once a translation exists it wins.
    write(
        &server._data.path().join("motd/pl.txt"),
        "Polskie wieści".as_bytes(),
    );
    let response = server.get("/motd?lang=pl", None).await;
    assert_eq!(response.text().await.unwrap(), "Polskie wieści");

    // No MOTD at all = 404, which the installer treats like being offline.
    std::fs::remove_dir_all(server._data.path().join("motd")).unwrap();
    assert_eq!(server.get("/motd?lang=en", None).await.status(), 404);
}

#[tokio::test]
async fn motd_needs_no_credential() {
    let server = Server::start().await;
    // Scene 2 calls /motd before the user has entered an access key.
    assert_eq!(server.get("/motd?lang=en", None).await.status(), 200);
    let bogus = server.get("/motd?lang=en", Some("not-a-token")).await;
    assert_eq!(bogus.status(), 200);
}

#[tokio::test]
async fn games_share_one_server_without_bleeding_into_each_other() {
    let server = Server::start().await;

    // The fixture has rbr/public/basic AND beamng/public/basic: same access
    // directory, same name. Only the game tells them apart, which is why it
    // leads the download address.
    let rbr = server.get("/download/rbr/public/basic", None).await;
    assert_eq!(rbr.status(), 200);
    assert_eq!(rbr.bytes().await.unwrap().as_ref(), b"public-bytes");

    let beamng = server.get("/download/beamng/public/basic", None).await;
    assert_eq!(beamng.status(), 200);
    assert_eq!(beamng.bytes().await.unwrap().as_ref(), b"beamng-bytes");

    // A game with nothing published is a 404, not someone else's bytes.
    assert_eq!(
        server
            .get("/download/dirtrally2/public/basic", None)
            .await
            .status(),
        404
    );
    // And the game segment is a lookup key like the others.
    for path in [
        "/download/..%2Fmotd/public/basic",
        "/download/RBR/public/basic",
        "/download/rbr%2F..%2Fbeamng/public/basic",
    ] {
        assert_eq!(server.get(path, None).await.status(), 404, "{path}");
    }
}

#[tokio::test]
async fn packages_lists_every_game_and_filters_on_request() {
    let server = Server::start().await;

    // No filter: one document covering every game, which is what a client
    // managing several games wants.
    let everything = server.listed(None).await;
    assert!(
        everything.iter().any(|r| r.starts_with("public/")),
        "{everything:?}"
    );
    let manifest: pacenotes_shared::Manifest =
        server.get("/packages", None).await.json().await.unwrap();
    assert!(manifest.packages.iter().any(|p| p.game == "rbr"));
    assert!(manifest.packages.iter().any(|p| p.game == "beamng"));

    // ?game= narrows it to one tab's worth.
    let rbr: pacenotes_shared::Manifest = server
        .get("/packages?game=rbr", None)
        .await
        .json()
        .await
        .unwrap();
    assert!(!rbr.packages.is_empty());
    assert!(rbr.packages.iter().all(|p| p.game == "rbr"));

    let beamng: pacenotes_shared::Manifest = server
        .get("/packages?game=beamng", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(beamng.packages.len(), 1);
    assert_eq!(beamng.packages[0].download_path(), "beamng/public/basic");

    // An unknown game is an empty list, not an error: the installer's tab for
    // a game this server carries nothing for just shows nothing.
    let unknown: pacenotes_shared::Manifest = server
        .get("/packages?game=nosuchgame", None)
        .await
        .json()
        .await
        .unwrap();
    assert!(unknown.packages.is_empty());

    // Filtering by game does not widen access: beamng's protected
    // directories would be filtered the same way.
    let anonymous_games: std::collections::BTreeSet<String> = manifest
        .packages
        .iter()
        .map(|p| p.directory.clone())
        .collect();
    assert_eq!(
        anonymous_games,
        std::collections::BTreeSet::from(["public".to_string()])
    );
}

#[tokio::test]
async fn the_game_comes_from_the_layout_not_a_sidecar() {
    let server = Server::start().await;

    // A sidecar that tries to claim another game is a hard error, because
    // `game` is no longer a sidecar key at all.
    write(
        &zip(server._data.path(), "rbr", "public/basic.toml"),
        b"game = \"beamng\"\n",
    );
    let err = pacenotes_server::store::ManifestStore::generate(server._data.path())
        .unwrap_err()
        .to_string();
    assert!(err.contains("basic.toml"), "{err}");
    assert!(err.contains("game"), "{err}");
}
