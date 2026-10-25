//! A real server on an ephemeral port over a tempdir data tree and an in-memory auth DB.
#![allow(dead_code)]

pub struct Server {
    pub base_url: String,
    pub connection: libsql::Connection,
    pub data: tempfile::TempDir,
}

pub const NESTED_NAME: &str = "codriver/polish/male/Kamil";

impl Server {
    /// RBR with one package in each of `public`, `alpha` and `beta` plus a
    /// nested one; BeamNG with the same `public/basic`; an English MOTD.
    pub async fn start() -> Server {
        Server::start_with(pacenotes_server::reports::Limits::default(), Vec::new()).await
    }

    /// `start`, with conflict-report limits and trusted proxies of the test's choosing.
    pub async fn start_with(
        limits: pacenotes_server::reports::Limits,
        trusted: Vec<pacenotes_server::reports::Cidr>,
    ) -> Server {
        let data = tempfile::tempdir().unwrap();
        write(
            &package_path(data.path(), "rbr", "public/basic.zip"),
            b"public-bytes",
        );
        write(
            &package_path(data.path(), "rbr", "alpha/secret.zip"),
            b"alpha-bytes",
        );
        write(
            &package_path(data.path(), "rbr", "beta/other.zip"),
            b"beta-bytes",
        );
        write(
            &package_path(data.path(), "rbr", &format!("public/{NESTED_NAME}.zip")),
            b"kamil-bytes",
        );
        write(
            &package_path(data.path(), "beamng", "public/basic.zip"),
            b"beamng-bytes",
        );
        write(&data.path().join("motd/en.txt"), b"English news");
        // Includes the categories tests add later, since a game publishes only what its schema declares.
        declare(
            data.path(),
            "rbr",
            &["basic", "secret", "other", "codriver", "style", "extra"],
        );
        declare(data.path(), "beamng", &["basic"]);
        regenerate(data.path());

        let connection = pacenotes_server::db::open(":memory:").await.unwrap();
        let state = std::sync::Arc::new(pacenotes_server::routes::AppState {
            store: pacenotes_server::store::ManifestStore::new(data.path()),
            db: connection.clone(),
            reports: pacenotes_server::reports::Reports::new(
                data.path().join(pacenotes_server::store::REPORTS_DIR),
                trusted,
                limits,
            ),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let service = pacenotes_server::routes::router(state)
                .into_make_service_with_connect_info::<std::net::SocketAddr>();
            axum::serve(listener, service).await.unwrap();
        });

        Server {
            base_url,
            connection,
            data,
        }
    }

    pub async fn mint(&self, grants: &[&str]) -> String {
        let grants: Vec<String> = grants.iter().map(|grant| grant.to_string()).collect();
        pacenotes_server::admin::mint(&self.connection, Some("test"), &grants, None)
            .await
            .unwrap()
    }

    pub async fn get(&self, path: &str, token: Option<&str>) -> reqwest::Response {
        let request = reqwest::Client::new().get(format!("{}{path}", self.base_url));
        let request = match token {
            Some(token) => request.bearer_auth(token),
            None => request,
        };
        request.send().await.unwrap()
    }

    pub async fn manifest(&self, path: &str, token: Option<&str>) -> pacenotes_shared::Manifest {
        self.get(path, token).await.json().await.unwrap()
    }

    /// Every listed package as a sorted `directory/name`.
    pub async fn listed(&self, token: Option<&str>) -> Vec<String> {
        let mut references: Vec<String> = self
            .manifest("/packages", token)
            .await
            .packages
            .iter()
            .map(|package| package.package_ref().to_string())
            .collect();
        references.sort();
        references
    }

    /// The access directories `/packages` reveals, sorted and deduplicated.
    pub async fn visible_directories(&self, token: Option<&str>) -> Vec<String> {
        let mut directories: Vec<String> = self
            .manifest("/packages", token)
            .await
            .packages
            .into_iter()
            .map(|package| package.directory)
            .collect();
        directories.sort();
        directories.dedup();
        directories
    }

    /// The status of downloading the one RBR package in `public`, `alpha` and `beta`.
    pub async fn download_statuses(&self, token: Option<&str>) -> [u16; 3] {
        let mut statuses = [0; 3];
        for (status, path) in statuses.iter_mut().zip([
            "/download/rbr/public/basic",
            "/download/rbr/alpha/secret",
            "/download/rbr/beta/other",
        ]) {
            *status = self.get(path, token).await.status().as_u16();
        }
        statuses
    }

    pub async fn status(&self, path: &str, token: Option<&str>) -> u16 {
        self.get(path, token).await.status().as_u16()
    }
}

pub fn package_path(data: &std::path::Path, game: &str, rest: &str) -> std::path::PathBuf {
    data.join(game)
        .join(pacenotes_server::store::PACKAGES_DIR)
        .join(rest)
}

/// What a publisher runs after dropping files in.
pub fn regenerate(data: &std::path::Path) {
    let generated = pacenotes_server::store::generate(data).unwrap();
    pacenotes_server::store::write_manifest(data, &generated.manifest).unwrap();
}

pub fn write(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Writes `<data>/<game>/schema.toml` declaring `categories` as optional extras.
pub fn declare(data: &std::path::Path, game: &str, categories: &[&str]) {
    let text: String = categories
        .iter()
        .map(|id| {
            format!("[[category]]\nid = \"{id}\"\nselect = \"any\"\ninstall_root = \"game\"\n")
        })
        .collect();
    write(
        &data.join(game).join(pacenotes_shared::SCHEMA_FILE),
        text.as_bytes(),
    );
}
