//! SHA-1 (FIPS 180-4), for looking dumps up in MAME's software list by the
//! digest it records. Written from the standard; a dump's identity, not a
//! security use.

pub fn digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 80];
        for (t, word) in block.chunks(4).enumerate() {
            w[t] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for t in 16..80 {
            w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (t, &word) in w.iter().enumerate() {
            let (f, k) = match t {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The digest as lowercase hex, as MAME's list writes it.
pub fn hex(data: &[u8]) -> String {
    digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_digests() {
        assert_eq!(super::hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            super::hex(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            super::hex(&million),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }
}
