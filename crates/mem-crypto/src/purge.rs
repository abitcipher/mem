//! Courier purge tokens and commitments.

use core::fmt;

use rand_core::CryptoRng;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Length of a purge token.
pub const PURGE_TOKEN_LEN: usize = 16;
/// Length of a purge commitment.
pub const PURGE_COMMITMENT_LEN: usize = 16;

const TOKEN_DOMAIN: &[u8] = b"MEM/v1/purge-token";
const COMMIT_CONTEXT: &str = "MEM v1 2026-10 purge-commitment";

/// Releasing this token lets any node delete bundles carrying its commitment.
///
/// Secret until the sender or recipient releases it. Not `Copy`: every duplicate is an explicit
/// `clone()`, and each one is zeroized on drop.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Zeroize, ZeroizeOnDrop)]
pub struct PurgeToken([u8; PURGE_TOKEN_LEN]);

impl PurgeToken {
    /// Wraps bytes from an inner payload or a gossip message.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; PURGE_TOKEN_LEN]) -> Self {
        Self(bytes)
    }

    /// Raw token bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; PURGE_TOKEN_LEN] {
        &self.0
    }
}

impl fmt::Debug for PurgeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PurgeToken(<redacted>)")
    }
}

/// Public commitment to a [`PurgeToken`], carried in the envelope (field 10).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PurgeCommitment([u8; PURGE_COMMITMENT_LEN]);

impl PurgeCommitment {
    /// Wraps bytes from an envelope.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; PURGE_COMMITMENT_LEN]) -> Self {
        Self(bytes)
    }

    /// Raw commitment bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; PURGE_COMMITMENT_LEN] {
        &self.0
    }
}

impl fmt::Debug for PurgeCommitment {
    /// Identifier-like: never print it in full.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PurgeCommitment({:02x}…)", self.0[0])
    }
}

/// The sender's per-device key for deriving purge tokens. Zeroized on drop; never printed.
pub struct SenderPurgeKey(Zeroizing<[u8; 32]>);

impl SenderPurgeKey {
    /// Generates a fresh key from the injected CSPRNG.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut key = Zeroizing::new([0u8; 32]);
        rng.fill_bytes(key.as_mut());
        Self(key)
    }

    /// Restores a stored key.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(Zeroizing::new(*bytes))
    }

    /// Raw key bytes, for encrypted storage only.
    ///
    /// # Security
    ///
    /// Secret. Never log it and never pass it across FFI.
    #[must_use]
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    /// The token for `bundle_id`. Deterministic, so the sender never stores tokens.
    #[must_use]
    pub fn derive_token(&self, bundle_id: &[u8; 16]) -> PurgeToken {
        // A keyed hasher holds the key itself as its chaining value: wipe it and its reader.
        let mut hasher = Zeroizing::new(blake3::Hasher::new_keyed(&self.0));
        hasher.update(TOKEN_DOMAIN);
        hasher.update(bundle_id);
        let mut out = [0u8; PURGE_TOKEN_LEN];
        Zeroizing::new(hasher.finalize_xof()).fill(&mut out);
        PurgeToken(out)
    }
}

impl fmt::Debug for SenderPurgeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SenderPurgeKey(<secret>)")
    }
}

/// The commitment for `token`.
#[must_use]
pub fn commit(token: &PurgeToken) -> PurgeCommitment {
    let mut full = Zeroizing::new(blake3::derive_key(COMMIT_CONTEXT, &token.0));
    let mut out = [0u8; PURGE_COMMITMENT_LEN];
    out.copy_from_slice(&full[..PURGE_COMMITMENT_LEN]);
    full.zeroize();
    PurgeCommitment(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn deterministic_and_separated() {
        let mut rng = ChaCha20Rng::from_seed([5; 32]);
        let k1 = SenderPurgeKey::generate(&mut rng);
        let k2 = SenderPurgeKey::generate(&mut rng);
        let t = k1.derive_token(&[1; 16]);
        assert_eq!(t, k1.derive_token(&[1; 16]));
        assert_ne!(t, k1.derive_token(&[2; 16]));
        assert_ne!(t, k2.derive_token(&[1; 16]));
        assert_eq!(commit(&t), commit(&t.clone()));
        assert_ne!(commit(&t), commit(&k1.derive_token(&[2; 16])));
        // The commitment is not the token.
        assert_ne!(commit(&t).as_bytes(), t.as_bytes());
    }

    #[test]
    fn matches_independent_blake3_construction() {
        // Recompute with the blake3 crate's one-shot APIs, independent of the code above.
        let key = SenderPurgeKey::from_bytes(&[7; 32]);
        let mut msg = alloc::vec::Vec::from(&b"MEM/v1/purge-token"[..]);
        msg.extend_from_slice(&[9; 16]);
        let expected_hash = blake3::keyed_hash(&[7; 32], &msg);
        let expected_token = &expected_hash.as_bytes()[..16];
        let token = key.derive_token(&[9; 16]);
        assert_eq!(token.as_bytes(), expected_token);
        let expected_commit =
            &blake3::derive_key("MEM v1 2026-10 purge-commitment", expected_token)[..16];
        assert_eq!(commit(&token).as_bytes(), expected_commit);
    }

    #[test]
    fn debug_redacts() {
        let t = PurgeToken::from_bytes([0xab; 16]);
        assert_eq!(alloc::format!("{t:?}"), "PurgeToken(<redacted>)");
        assert_eq!(
            alloc::format!("{:?}", SenderPurgeKey::from_bytes(&[1; 32])),
            "SenderPurgeKey(<secret>)"
        );
    }
}
