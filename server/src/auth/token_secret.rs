/// Marks a Pacenotes access key, for users and for secret scanners.
pub const TOKEN_PREFIX: &str = "pcn_";

/// `pcn_` + 32 random bytes as unpadded base64url; the caller shows it once
/// and stores only `hash_token` of it.
pub fn mint_token() -> String {
    let mut bytes = [0u8; 32];
    <rand::rngs::OsRng as rand::RngCore>::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    format!("{TOKEN_PREFIX}{}", base64url(&bytes))
}

/// Lowercase hex sha256: the only form of a credential the server keeps.
pub fn hash_token(raw: &str) -> String {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(raw.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let padded = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let group =
            (u32::from(padded[0]) << 16) | (u32::from(padded[1]) << 8) | u32::from(padded[2]);
        let sextets = [
            group >> 18 & 63,
            group >> 12 & 63,
            group >> 6 & 63,
            group & 63,
        ];
        let unpadded_length = chunk.len() + 1;
        for sextet in sextets.iter().take(unpadded_length) {
            encoded.push(ALPHABET[*sextet as usize] as char);
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    #[test]
    fn hashing_is_stable_and_hides_the_token() {
        assert_eq!(
            super::hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let token = super::mint_token();
        assert!(!super::hash_token(&token).contains(&token));
    }

    #[test]
    fn minted_tokens_are_prefixed_unique_and_url_safe() {
        let first = super::mint_token();
        let second = super::mint_token();
        assert_ne!(first, second);
        assert!(first.starts_with(super::TOKEN_PREFIX), "{first}");
        let encoded_32_bytes = 43;
        assert_eq!(
            first.len(),
            super::TOKEN_PREFIX.len() + encoded_32_bytes,
            "{first}"
        );
        assert!(
            first[super::TOKEN_PREFIX.len()..]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "{first}"
        );
    }

    #[test]
    fn base64url_matches_known_vectors() {
        let vectors: [(&[u8], &str); 6] = [
            (b"", ""),
            (b"f", "Zg"),
            (b"fo", "Zm8"),
            (b"foo", "Zm9v"),
            (b"foobar", "Zm9vYmFy"),
            (&[0xfb, 0xff], "-_8"),
        ];
        for (input, expected) in vectors {
            assert_eq!(super::base64url(input), expected, "{input:?}");
        }
    }
}
