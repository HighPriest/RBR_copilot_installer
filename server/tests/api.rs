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
    /// public/basic.zip, alpha/secret.zip, beta/other.zip + a generated
    /// manifest, served by the real router.
    async fn start() -> Server {
        let packages = tempfile::tempdir().unwrap();
        write(&packages.path().join("public/basic.zip"), b"public-bytes");
        write(&packages.path().join("alpha/secret.zip"), b"alpha-bytes");
        write(&packages.path().join("beta/other.zip"), b"beta-bytes");
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

    /// Directories visible in `/packages`, sorted — the listing half of the
    /// matrix.
    async fn visible(&self, token: Option<&str>) -> Vec<String> {
        let manifest: pacenotes_shared::Manifest =
            self.get("/packages", token).await.json().await.unwrap();
        let mut directories: Vec<String> =
            manifest.packages.into_iter().map(|p| p.directory).collect();
        directories.sort();
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
    assert_eq!(server.visible(None).await, vec!["public"]);

    // Publish a second public package the way a publisher would.
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

    assert_eq!(server.visible(None).await, vec!["public", "public"]);
    assert_eq!(
        server.get("/download/public/extra", None).await.status(),
        200
    );
}
