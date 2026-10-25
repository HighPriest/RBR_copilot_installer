//! An installer telling the operator that selected packages wrote the same
//! file: stored without identifying anyone, validated as hostile input, rate
//! limited, and summarised at the next publish.

mod common;

/// Two packages that both ship PaceNote.ini.
fn conflict_body() -> serde_json::Value {
    serde_json::json!({
        "game": "rbr",
        "installer": "0.2.0",
        "conflicts": [{
            "file": "Plugins/Pacenote/PaceNote.ini",
            "packages": ["public/basic", "public/codriver/polish/male/Kamil"]
        }]
    })
}

async fn report(
    server: &common::Server,
    body: &serde_json::Value,
    forwarded_for: Option<&str>,
) -> reqwest::Response {
    let request = reqwest::Client::new()
        .post(format!("{}/report/conflict", server.base_url))
        .json(body);
    let request = match forwarded_for {
        Some(ip) => request.header("X-Forwarded-For", ip),
        None => request,
    };
    request.send().await.unwrap()
}

async fn report_status(
    server: &common::Server,
    body: &serde_json::Value,
    forwarded_for: Option<&str>,
) -> u16 {
    report(server, body, forwarded_for).await.status().as_u16()
}

fn stored_text(server: &common::Server) -> String {
    let dir = server
        .data
        .path()
        .join(pacenotes_server::store::REPORTS_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return String::new();
    };
    entries
        .flatten()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .collect()
}

fn stored_reports(server: &common::Server) -> Vec<pacenotes_server::reports::Record> {
    stored_text(server)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn a_conflict_report_is_accepted_stored_and_never_echoed() {
    let server = common::Server::start().await;
    let before = pacenotes_server::auth::now();

    let response = report(&server, &conflict_body(), None).await;
    assert_eq!(response.status(), 202);
    assert!(response.bytes().await.unwrap().is_empty());

    let stored = stored_reports(&server);
    assert_eq!(stored.len(), 1);
    let record = &stored[0];
    assert_eq!(record.game, "rbr");
    assert_eq!(record.installer, "0.2.0");
    assert_eq!(record.conflicts[0].file, "Plugins/Pacenote/PaceNote.ini");
    assert_eq!(record.dropped, 0);
    assert!(
        (before..=pacenotes_server::auth::now()).contains(&record.received_at),
        "the server's clock, not the client's"
    );
    let text = stored_text(&server);
    assert!(!text.contains("127.0.0.1"), "no address is stored: {text}");
}

#[tokio::test]
async fn invalid_reports_are_refused_without_echoing_them() {
    let server = common::Server::start().await;
    let mut bad_game = conflict_body();
    bad_game["game"] = "../../etc".into();
    let mut no_valid_conflict = conflict_body();
    no_valid_conflict["conflicts"][0]["file"] = "..\\..\\boot.ini".into();

    for (case, body) in [
        ("bad game", bad_game),
        ("no valid conflict", no_valid_conflict),
        ("not a report", serde_json::json!({"hello": "<script>"})),
    ] {
        let response = report(&server, &body, None).await;
        assert_eq!(response.status(), 400, "{case}");
        let text = response.text().await.unwrap();
        let error: pacenotes_shared::ApiError = serde_json::from_str(&text).unwrap();
        assert_eq!(error.code, "invalid_report", "{case}");
        assert!(
            !text.contains("<script>") && !text.contains("etc"),
            "{case}: {text}"
        );
    }
    assert!(stored_reports(&server).is_empty());
}

/// Refused before anything parses it.
#[tokio::test]
async fn a_report_over_the_body_cap_is_413() {
    let server = common::Server::start().await;
    let mut huge = conflict_body();
    huge["installer"] = "x".repeat(pacenotes_server::reports::MAX_BODY_BYTES).into();
    assert_eq!(report_status(&server, &huge, None).await, 413);
}

/// Without a trusted proxy the header is ignored, so rotating it is useless.
#[tokio::test]
async fn reports_are_rate_limited_and_a_forged_header_does_not_help() {
    let limits = pacenotes_server::reports::Limits {
        client_burst: 2.0,
        client_per_hour: 2.0,
        ..Default::default()
    };
    let server = common::Server::start_with(limits, Vec::new()).await;
    for forged in ["198.51.100.1", "198.51.100.2"] {
        assert_eq!(
            report_status(&server, &conflict_body(), Some(forged)).await,
            202
        );
    }

    let limited = report(&server, &conflict_body(), Some("198.51.100.3")).await;
    assert_eq!(limited.status(), 429);
    assert!(limited.headers().contains_key("retry-after"));
    let error: pacenotes_shared::ApiError = limited.json().await.unwrap();
    assert_eq!(error.code, "rate_limited");
    assert_eq!(stored_reports(&server).len(), 2);
}

/// The test client connects from 127.0.0.1, standing in for Caddy.
#[tokio::test]
async fn behind_a_trusted_proxy_each_client_gets_its_own_bucket() {
    let limits = pacenotes_server::reports::Limits {
        client_burst: 1.0,
        client_per_hour: 1.0,
        ..Default::default()
    };
    let trusted = vec![pacenotes_server::reports::Cidr::parse("127.0.0.1").unwrap()];
    let server = common::Server::start_with(limits, trusted).await;

    let first = Some("198.51.100.1");
    assert_eq!(report_status(&server, &conflict_body(), first).await, 202);
    assert_eq!(
        report_status(&server, &conflict_body(), Some("198.51.100.2")).await,
        202
    );
    assert_eq!(report_status(&server, &conflict_body(), first).await, 429);
}

/// Advice only: the manifest itself is unchanged by reports.
#[tokio::test]
async fn reported_overlaps_reach_the_publisher_at_manifest_gen() {
    let server = common::Server::start().await;
    assert_eq!(report_status(&server, &conflict_body(), None).await, 202);

    let generated = pacenotes_server::store::generate(server.data.path()).unwrap();
    assert!(
        generated.warnings.iter().any(|warning| {
            warning.contains("public/basic")
                && warning.contains("public/codriver/polish/male/Kamil")
                && warning.contains("reported overlapping")
        }),
        "{:?}",
        generated.warnings
    );
    assert!(generated.errors.is_empty(), "{:?}", generated.errors);
}
