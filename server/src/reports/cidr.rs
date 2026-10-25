/// An IP network written as `10.0.0.0/24`, or a single address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cidr {
    network: std::net::IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(text: &str) -> Result<Cidr, String> {
        let text = text.trim();
        let (address, prefix) = match text.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (text, None),
        };
        let network: std::net::IpAddr = address
            .parse()
            .map_err(|_| format!("{text:?} is not an IP address or CIDR"))?;
        let network = network.to_canonical();
        let max = if network.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            None => max,
            Some(prefix) => prefix
                .parse::<u8>()
                .ok()
                .filter(|prefix| *prefix <= max)
                .ok_or_else(|| format!("{text:?}: prefix must be 0-{max}"))?,
        };
        Ok(Cidr { network, prefix })
    }

    /// An IPv4-mapped IPv6 address, as a dual-stack listener reports it, counts as IPv4.
    pub fn contains(&self, ip: std::net::IpAddr) -> bool {
        match (self.network, ip.to_canonical()) {
            (std::net::IpAddr::V4(network), std::net::IpAddr::V4(ip)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(network) & mask == u32::from(ip) & mask
            }
            (std::net::IpAddr::V6(network), std::net::IpAddr::V6(ip)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(network) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    fn ip(text: &str) -> std::net::IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn a_network_matches_its_addresses_and_nothing_else() {
        let network = super::Cidr::parse("10.0.0.0/24").unwrap();
        assert!(network.contains(ip("10.0.0.90")));
        assert!(!network.contains(ip("10.0.1.1")));
        assert!(network.contains(ip("::ffff:10.0.0.90")));
        assert!(!network.contains(ip("2001:db8::1")));

        let v6 = super::Cidr::parse("fd00::/8").unwrap();
        assert!(v6.contains(ip("fd12::1")));
    }

    #[test]
    fn a_bare_address_matches_only_itself() {
        let host = super::Cidr::parse("10.0.0.90").unwrap();
        assert!(host.contains(ip("10.0.0.90")));
        assert!(!host.contains(ip("10.0.0.91")));
    }

    #[test]
    fn malformed_networks_are_refused() {
        for bad in ["10.0.0.0/33", "caddy", "10.0.0.0/x"] {
            assert!(super::Cidr::parse(bad).is_err(), "{bad}");
        }
    }
}
