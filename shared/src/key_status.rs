/// The body of `GET /whoami` for a live access key; every other key is a 401,
/// so the status alone tells a mistyped key from a good one, which `/packages`
/// cannot, since it falls back to the public listing.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KeyStatus {
    /// Unix seconds; `None` never expires. Lets the installer warn before the key stops working.
    pub expires_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn key_status_round_trips_with_and_without_expiry() {
        for status in [
            super::KeyStatus { expires_at: None },
            super::KeyStatus {
                expires_at: Some(1_900_000_000),
            },
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: super::KeyStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    /// `null` is the wire form of "never", not an absent field.
    #[test]
    fn a_key_that_never_expires_says_so_explicitly() {
        assert_eq!(
            serde_json::to_string(&super::KeyStatus { expires_at: None }).unwrap(),
            r#"{"expires_at":null}"#
        );
    }
}
