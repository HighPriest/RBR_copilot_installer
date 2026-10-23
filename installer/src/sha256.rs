//! A std-only SHA-256, because the `sha2` crate's generic stack costs tens of kB.
//! Every download is verified with it and the cache is keyed by it.
//! Names inside `compress` follow FIPS 180-4, so the code reads against the spec.

/// The first 32 bits of the fractional parts of the cube roots of the first
/// 64 primes (FIPS 180-4 §4.2.2).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The first 32 bits of the fractional parts of the square roots of the first
/// 8 primes (FIPS 180-4 §5.3.3).
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const BLOCK_SIZE: usize = 64;
/// Where the message length starts inside the final padded block.
const LENGTH_OFFSET: usize = 56;

pub struct Sha256 {
    hash: [u32; 8],
    /// Bytes not yet part of a whole block.
    buffer: [u8; BLOCK_SIZE],
    buffered: usize,
    total_bytes: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Sha256 {
        Sha256 {
            hash: H0,
            buffer: [0u8; BLOCK_SIZE],
            buffered: 0,
            total_bytes: 0,
        }
    }

    /// Callable any number of times, since a download arrives in chunks.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(data.len() as u64);

        if self.buffered > 0 {
            let taken = (BLOCK_SIZE - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + taken].copy_from_slice(&data[..taken]);
            self.buffered += taken;
            data = &data[taken..];
            if self.buffered < BLOCK_SIZE {
                // Falling through would overwrite `buffered` and lose these bytes.
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffered = 0;
        }

        let (blocks, rest) = data.as_chunks::<BLOCK_SIZE>();
        for block in blocks {
            self.compress(block);
        }
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.buffered = rest.len();
    }

    /// Pads with 0x80, zeros and the bit length as FIPS 180-4 §5.1.1 says.
    pub fn finalize(mut self) -> [u8; 32] {
        let length_in_bits = self.total_bytes.wrapping_mul(8);
        let mut padding = [0u8; BLOCK_SIZE + 8];
        padding[0] = 0x80;
        let zeros = if self.buffered < LENGTH_OFFSET {
            LENGTH_OFFSET - self.buffered
        } else {
            BLOCK_SIZE + LENGTH_OFFSET - self.buffered
        };
        padding[zeros..zeros + 8].copy_from_slice(&length_in_bits.to_be_bytes());
        self.update(&padding[..zeros + 8]);
        debug_assert_eq!(self.buffered, 0, "padding must end on a block boundary");

        let mut digest = [0u8; 32];
        for (index, word) in self.hash.iter().enumerate() {
            digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    /// FIPS 180-4 §6.2.2.
    fn compress(&mut self, block: &[u8; BLOCK_SIZE]) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.hash;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in self.hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
}

/// Lowercase hex, the form the manifest and the cache use.
pub fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The lowercase hex digest of data already in memory.
pub fn hex_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex(&hasher.finalize())
}

#[cfg(test)]
mod tests;
