/// The client a request comes from, for rate limiting.
///
/// Behind Caddy every peer is Caddy, so `X-Forwarded-For` names the client, but
/// only a trusted proxy's is believed: its right-most entry that is not itself a
/// trusted proxy. An unparsable entry there falls back to the peer rather than
/// reading further left, where the client writes.
pub fn client_ip(
    peer: std::net::IpAddr,
    headers: &axum::http::HeaderMap,
    trusted: &[super::Cidr],
) -> std::net::IpAddr {
    let peer = peer.to_canonical();
    let is_trusted = |ip: std::net::IpAddr| trusted.iter().any(|network| network.contains(ip));
    if !is_trusted(peer) {
        return peer;
    }
    let hops: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .collect();
    for hop in hops.into_iter().rev() {
        match hop.parse::<std::net::IpAddr>() {
            Ok(ip) if is_trusted(ip.to_canonical()) => continue,
            Ok(ip) => return ip.to_canonical(),
            Err(_) => return peer,
        }
    }
    peer
}

#[cfg(test)]
mod tests {
    fn forwarded_for(value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_str(value).unwrap(),
        );
        headers
    }

    fn ip(text: &str) -> std::net::IpAddr {
        text.parse().unwrap()
    }

    fn trusted() -> Vec<super::super::Cidr> {
        vec![super::super::Cidr::parse("10.0.0.0/24").unwrap()]
    }

    const CADDY: &str = "10.0.0.90";

    #[test]
    fn through_a_trusted_proxy_the_client_is_the_entry_it_appended() {
        let client = super::client_ip(ip(CADDY), &forwarded_for("198.51.100.1"), &trusted());
        assert_eq!(client, ip("198.51.100.1"));
    }

    #[test]
    fn a_forged_entry_left_of_the_proxys_is_ignored() {
        let headers = forwarded_for("1.1.1.1, 198.51.100.1");
        assert_eq!(
            super::client_ip(ip(CADDY), &headers, &trusted()),
            ip("198.51.100.1")
        );
    }

    #[test]
    fn the_header_from_anyone_else_is_ignored() {
        let stranger = ip("203.0.113.7");
        let headers = forwarded_for("1.1.1.1");
        assert_eq!(super::client_ip(stranger, &headers, &trusted()), stranger);
        assert_eq!(
            super::client_ip(ip(CADDY), &forwarded_for("198.51.100.1"), &[]),
            ip(CADDY)
        );
    }

    #[test]
    fn garbage_where_the_client_belongs_falls_back_to_the_peer() {
        let headers = forwarded_for("1.1.1.1, unknown");
        assert_eq!(super::client_ip(ip(CADDY), &headers, &trusted()), ip(CADDY));
    }
}
