//! Integration tests (task S8): a real server on an ephemeral port, a real
//! packages tree in a tempdir and a real (in-memory) auth DB.
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
    _packages: tempfile::TempDir,
}

impl Server {
    /// One package per access directory (`public`, `alpha`, `beta`) plus a
    /// deeply nested one and a MOTD, generated into a manifest and served by
    /// the real router.
    async fn start() -> Server {
        let packages = tempfile::tempdir().unwrap();
        write(&packages.path().join("public/basic.zip"), b"public-bytes");
        write(&packages.path().join("alpha/secret.zip"), b"alpha-bytes");
        write(&packages.path().join("beta/other.zip"), b"beta-bytes");
        // The nested layout the presentation model depends on.
        write(
            &packages
                .path()
                .join("public/codriver/polish/male/Kamil.zip"),
            b"kamil-bytes",
        );
        write(&packages.path().join("motd/en.txt"), b"English news");
        let generated = pacenotes_server::store::ManifestStore::generate(packages.path()).unwrap();
        write(
            &packages.path().join("manifest.json"),
            serde_json::to_string(&generated.manifest)
                .unwrap()
                .as_bytes(),
        );

        let connection = pacenotes_server::db::open(":memory:").await.unwrap();
        let state = std::sync::Arc::new(pacenotes_server::routes::AppState {
            store: pacenotes_server::store::ManifestStore::new(packages.path()),
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
            _packages: packages,
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
            self.get("/download/public/basic", token)
                .await
                .status()
                .as_u16(),
            self.get("/download/alpha/secret", token)
                .await
                .status()
                .as_u16(),
            self.get("/download/beta/other", token)
                .await
                .status()
                .as_u16(),
        ]
    }
}

/// A package name with `/` in it has to survive the URL as segments, which
/// is the whole point of the wildcard route.
const NESTED: &str = "codriver/polish/male/Kamil";

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
    let response = server.get("/download/alpha/secret", Some(&token)).await;

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
            .get("/download/alpha/nope", Some(&token))
            .await
            .status(),
        404
    );
    assert_eq!(
        server
            .get("/download/nosuchdir/x", Some(&token))
            .await
            .status(),
        404
    );
    assert_eq!(
        server.get("/download/nosuchdir/x", None).await.status(),
        404
    );

    // directory/name are manifest keys, never path fragments.
    for path in [
        "/download/..%2F..%2Fdata/auth.db",
        "/download/public/..%2F..%2Fmanifest",
        "/download/%2Fetc/passwd",
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
        .get("/download/alpha/secret", None)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(unauthorized.code, "unauthorized");
    assert!(!unauthorized.message.is_empty());

    let forbidden: pacenotes_shared::ApiError = server
        .get("/download/alpha/secret", Some(&no_grants))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(forbidden.code, "forbidden");

    let not_found: pacenotes_shared::ApiError = server
        .get("/download/alpha/nope", None)
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
        server.get("/download/public/extra", None).await.status(),
        404
    );

    // Publish another public package the way a publisher would: drop the zip
    // in, regenerate the manifest.
    write(
        &server._packages.path().join("public/extra.zip"),
        b"extra-bytes",
    );
    let generated =
        pacenotes_server::store::ManifestStore::generate(server._packages.path()).unwrap();
    write(
        &server._packages.path().join("manifest.json"),
        serde_json::to_string(&generated.manifest)
            .unwrap()
            .as_bytes(),
    );

    // The store notices the new manifest mtime on the next request.
    assert_eq!(server.listed(None).await.len(), before + 1);
    assert_eq!(
        server.get("/download/public/extra", None).await.status(),
        200
    );
}

#[tokio::test]
async fn nested_names_download_through_the_wildcard_route() {
    let server = Server::start().await;

    // The nested package is public, so anonymous gets it...
    let response = server
        .get(&format!("/download/public/{NESTED}"), None)
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
        "/download/public/codriver/polish/male",
        "/download/public/codriver",
        // Reaching out of the access directory.
        "/download/public/..%2Fmotd%2Fen",
        "/download/public/codriver%2F..%2F..%2Falpha%2Fsecret",
        "/download/public/%2E%2E%2Fmanifest",
        // The MOTD directory is not an access directory.
        "/download/motd/en",
        // Absolute-looking names.
        "/download/public/%2Fetc%2Fpasswd",
    ] {
        assert_eq!(server.get(path, None).await.status(), 404, "{path}");
    }

    // Filing a package deeper does not make it public: `alpha` has no nested
    // package, so this is 404 (not 401) — the status must not reveal which
    // names exist inside a directory the caller cannot see.
    assert_eq!(
        server
            .get(&format!("/download/alpha/{NESTED}"), None)
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
        &server._packages.path().join("motd/pl.txt"),
        "Polskie wieści".as_bytes(),
    );
    let response = server.get("/motd?lang=pl", None).await;
    assert_eq!(response.text().await.unwrap(), "Polskie wieści");

    // No MOTD at all = 404, which the installer treats like being offline.
    std::fs::remove_dir_all(server._packages.path().join("motd")).unwrap();
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
