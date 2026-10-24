use sha2::{Digest, Sha256};

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub fn feature_index(token: &str, buckets: usize) -> usize {
    assert!(buckets > 0);
    let digest = Sha256::digest(token.as_bytes());
    let mut n = [0u8; 8];
    n.copy_from_slice(&digest[..8]);
    (u64::from_le_bytes(n) as usize) % buckets
}
pub fn simhash(tokens: &[&str]) -> u64 {
    let mut acc = [0i32; 64];
    for token in tokens {
        let h = Sha256::digest(token.as_bytes());
        for i in 0..64 {
            let bit = (h[i / 8] >> (i % 8)) & 1;
            acc[i] += if bit == 1 { 1 } else { -1 };
        }
    }
    acc.iter()
        .enumerate()
        .fold(0u64, |r, (i, v)| r | (((*v > 0) as u64) << i))
}
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_hash() {
        assert_eq!(feature_index("neuron", 100), feature_index("neuron", 100));
        assert_eq!(sha256_hex(b"a").len(), 64);
    }
}
