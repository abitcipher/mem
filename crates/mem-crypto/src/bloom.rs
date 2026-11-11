//! Salted Bloom filters for encounter summaries.
//!
//! A node tells a peer what it holds without resending bundles. Each summary has a fresh random
//! salt, so two summaries from the same node cannot be compared bit for bit.

use alloc::vec;
use alloc::vec::Vec;

/// Hash functions per item.
pub const BLOOM_K: usize = 7;
/// Largest filter in bytes (`spec/encounter.cddl`).
pub const MAX_BLOOM_BYTES: usize = 32 * 1024;
/// Bits per item for about 1 % false positives with `k = 7` (rounded up).
const BITS_PER_ITEM: usize = 10;

const CONTEXT: &str = "MEM v1 2026-10 summary-bloom";

/// A Bloom filter keyed by a per-summary salt.
#[derive(Clone, PartialEq, Eq)]
pub struct SaltedBloom {
    key: [u8; 32],
    bits: Vec<u8>,
}

impl core::fmt::Debug for SaltedBloom {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SaltedBloom({} B)", self.bits.len())
    }
}

impl SaltedBloom {
    /// An empty filter sized for `items`, capped at [`MAX_BLOOM_BYTES`].
    #[must_use]
    pub fn with_capacity(salt: &[u8; 16], items: usize) -> Self {
        let bytes = items
            .saturating_mul(BITS_PER_ITEM)
            .div_ceil(8)
            .clamp(8, MAX_BLOOM_BYTES);
        Self {
            key: blake3::derive_key(CONTEXT, salt),
            bits: vec![0; bytes],
        }
    }

    /// A received filter. `None` if it is empty or over the size limit.
    #[must_use]
    pub fn from_bytes(salt: &[u8; 16], bits: Vec<u8>) -> Option<Self> {
        (!bits.is_empty() && bits.len() <= MAX_BLOOM_BYTES).then(|| Self {
            key: blake3::derive_key(CONTEXT, salt),
            bits,
        })
    }

    /// The bit array, for the wire.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bits
    }

    /// Adds `item`.
    pub fn insert(&mut self, item: &[u8]) {
        for i in self.indexes(item) {
            if let Some(byte) = self.bits.get_mut(i / 8) {
                *byte |= 1 << (i % 8);
            }
        }
    }

    /// Whether `item` may be in the set (false positives about 1 % at design load; never false
    /// negatives).
    #[must_use]
    pub fn contains(&self, item: &[u8]) -> bool {
        self.indexes(item).into_iter().all(|i| {
            self.bits
                .get(i / 8)
                .is_some_and(|b| b & (1 << (i % 8)) != 0)
        })
    }

    fn indexes(&self, item: &[u8]) -> [usize; BLOOM_K] {
        let mut out = [0u8; BLOOM_K * 4];
        blake3::Hasher::new_keyed(&self.key)
            .update(item)
            .finalize_xof()
            .fill(&mut out);
        let m = self.bits.len() * 8;
        let mut idx = [0usize; BLOOM_K];
        for (i, chunk) in out.as_chunks::<4>().0.iter().enumerate() {
            let w = u32::from_be_bytes(*chunk);
            if let Some(slot) = idx.get_mut(i) {
                *slot = usize::try_from(w).unwrap_or(0) % m;
            }
        }
        idx
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    #[test]
    fn members_are_found_and_false_positives_are_rare() {
        let mut b = SaltedBloom::with_capacity(&[1; 16], 1000);
        for i in 0u32..1000 {
            b.insert(&i.to_be_bytes());
        }
        assert!((0u32..1000).all(|i| b.contains(&i.to_be_bytes())));
        let fp = (1000u32..11_000)
            .filter(|i| b.contains(&i.to_be_bytes()))
            .count();
        assert!(fp < 250, "false positives {fp} / 10000");
    }

    #[test]
    fn salts_make_filters_incomparable_and_receivers_rebuild_them() {
        let (mut a, mut b) = (
            SaltedBloom::with_capacity(&[1; 16], 10),
            SaltedBloom::with_capacity(&[2; 16], 10),
        );
        a.insert(b"same");
        b.insert(b"same");
        assert_ne!(a.as_bytes(), b.as_bytes());
        let received = SaltedBloom::from_bytes(&[1; 16], a.as_bytes().to_vec()).unwrap();
        assert!(received.contains(b"same"));
        assert!(SaltedBloom::from_bytes(&[1; 16], vec![]).is_none());
        assert!(SaltedBloom::from_bytes(&[1; 16], vec![0; MAX_BLOOM_BYTES + 1]).is_none());
    }
}
