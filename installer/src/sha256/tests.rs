//! Expected digests are published vectors (FIPS 180-4, NIST CAVP) or another
//! implementation's output, never values this implementation produced.

#[test]
fn published_vectors() {
    for (input, digest) in [
        (
            &b""[..],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
        ),
        (
            &[0u8; 64],
            "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b",
        ),
    ] {
        assert_eq!(super::hex_digest(input), digest, "{input:?}");
    }
}

/// A million 'a's catches a length counter kept in the wrong unit or overflowing.
#[test]
fn the_long_vector() {
    let mut hasher = super::Sha256::new();
    let chunk = vec![b'a'; 1000];
    for _ in 0..1000 {
        hasher.update(&chunk);
    }
    assert_eq!(
        super::hex(&hasher.finalize()),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

/// A download's chunk boundaries are not ours to choose.
#[test]
fn chunking_never_changes_the_digest() {
    let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    let whole = super::hex_digest(&data);
    for chunk_size in [1usize, 63, 64, 65, 127, 128, 129, 500, 999] {
        let mut hasher = super::Sha256::new();
        for part in data.chunks(chunk_size) {
            hasher.update(part);
        }
        assert_eq!(
            super::hex(&hasher.finalize()),
            whole,
            "{chunk_size}-byte chunks"
        );
    }

    let mut hasher = super::Sha256::new();
    for part in [
        &[][..],
        &data[..1],
        &data[1..64],
        &[],
        &data[64..65],
        &data[65..],
    ] {
        hasher.update(part);
    }
    assert_eq!(super::hex(&hasher.finalize()), whole);
}

#[test]
fn block_boundaries() {
    for length in [55usize, 56, 57, 63, 64, 65, 119, 120, 121, 128] {
        let data = vec![b'x'; length];
        let mut hasher = super::Sha256::new();
        hasher.update(&data[..length / 2]);
        hasher.update(&data[length / 2..]);
        assert_eq!(
            super::hex(&hasher.finalize()),
            super::hex_digest(&data),
            "length {length}"
        );
    }
}

#[test]
fn hex_is_lowercase_and_padded() {
    assert_eq!(super::hex(&[0u8; 32]), "0".repeat(64));

    let mut digest = [0u8; 32];
    digest[0] = 0x0f;
    digest[31] = 0xa0;
    let text = super::hex(&digest);
    assert!(text.starts_with("0f"), "{text}");
    assert!(text.ends_with("a0"), "{text}");
    assert_eq!(text, text.to_lowercase());
}

/// Cross-checked against .NET's `System.Security.Cryptography.SHA256`.
#[test]
fn another_implementation_agrees() {
    assert_eq!(
        super::hex_digest(b"pacenotes overlay payload\n"),
        "8de8d45aeae6897a49d317137ba5fa8b66f26f3bf9c5e3c8033fa1e6ada978d6"
    );
}
