//! Long-term identity keys.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use data_encoding::BASE32_NOPAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::CryptoRng;
use x25519_dalek::{PublicKey as XPublic, StaticSecret};
use zeroize::Zeroizing;

use crate::error::CryptoError;

/// Ed25519 identity signing key. Zeroized on drop; never printed.
pub struct IdentityKey(SigningKey);

impl IdentityKey {
    /// Generates a new identity from the injected CSPRNG.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut seed = Zeroizing::new([0u8; 32]);
        rng.fill_bytes(seed.as_mut());
        Self(SigningKey::from_bytes(&seed))
    }

    /// Restores an identity from its 32-byte seed (storage only).
    #[must_use]
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(seed))
    }

    /// The 32-byte seed, for encrypted local storage only.
    ///
    /// # Security
    ///
    /// The result is secret. It must only be written to encrypted storage and must never
    /// cross the FFI boundary.
    #[must_use]
    pub fn to_seed(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0.to_bytes())
    }

    /// The public half.
    #[must_use]
    pub fn public(&self) -> IdentityPublic {
        IdentityPublic(self.0.verifying_key().to_bytes())
    }

    /// Signs the concatenation `domain || parts[0] || parts[1] || ...`.
    ///
    /// # Security
    ///
    /// `domain` must be a fixed, unique `"MEM/v1/..."` label so that a signature from one
    /// context can never be accepted in another, and no domain may be a prefix of another.
    /// The parts are joined without lengths, so every part except the last must have a fixed
    /// length; otherwise `["a", "bc"]` and `["ab", "c"]` would sign the same bytes.
    #[must_use]
    pub fn sign(&self, domain: &[u8], parts: &[&[u8]]) -> [u8; 64] {
        self.0.sign(&concat(domain, parts)).to_bytes()
    }

    /// X25519 static secret derived from this identity.
    pub(crate) fn x25519_secret(&self) -> StaticSecret {
        StaticSecret::from(self.0.to_scalar_bytes())
    }
}

impl fmt::Debug for IdentityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityKey(<secret>)")
    }
}

/// A validated Ed25519 identity public key.
///
/// Ordered by its bytes; the order defines the `lo`/`hi` roles of the contact secret and the tags.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdentityPublic([u8; 32]);

impl IdentityPublic {
    /// Parses and validates a public key.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] if the bytes are not a valid point, or the point is weak
    /// (small order).
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let key = VerifyingKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidPublicKey)?;
        if key.is_weak() {
            return Err(CryptoError::InvalidPublicKey);
        }
        Ok(Self(*bytes))
    }

    /// Raw key bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verifies a signature made by [`IdentityKey::sign`] with the same `domain` and `parts`.
    ///
    /// Uses strict verification (rejects malleable and small-order signatures).
    ///
    /// # Errors
    ///
    /// [`CryptoError::BadSignature`] if verification fails.
    pub fn verify(
        &self,
        domain: &[u8],
        parts: &[&[u8]],
        signature: &[u8; 64],
    ) -> Result<(), CryptoError> {
        let key = self.verifying_key()?;
        key.verify_strict(&concat(domain, parts), &Signature::from_bytes(signature))
            .map_err(|_| CryptoError::BadSignature)
    }

    /// The device id of this key when it is a device key:
    /// `BLAKE3(device_pk)[0..8]`. Only names a device; never a secret.
    #[must_use]
    pub fn device_id(&self) -> [u8; 8] {
        let hash = blake3::hash(&self.0);
        let mut id = [0u8; 8];
        id.copy_from_slice(&hash.as_bytes()[..8]);
        id
    }

    /// Human-verifiable fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        let hash = blake3::hash(&self.0);
        let mut out = [0u8; 20];
        out.copy_from_slice(&hash.as_bytes()[..20]);
        Fingerprint(out)
    }

    /// X25519 public key derived from this identity.
    pub(crate) fn x25519_public(&self) -> Result<XPublic, CryptoError> {
        Ok(XPublic::from(
            self.verifying_key()?.to_montgomery().to_bytes(),
        ))
    }

    fn verifying_key(&self) -> Result<VerifyingKey, CryptoError> {
        VerifyingKey::from_bytes(&self.0).map_err(|_| CryptoError::InvalidPublicKey)
    }
}

impl fmt::Debug for IdentityPublic {
    /// Prints the fingerprint prefix only; public keys are identifiers.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fp = self.fingerprint().to_string_grouped();
        write!(f, "IdentityPublic({}…)", fp.get(..4).unwrap_or(""))
    }
}

/// First 20 bytes of `BLAKE3(identity_pk)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 20]);

impl Fingerprint {
    /// Raw fingerprint bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Base32 (RFC 4648, no padding) in groups of four characters, e.g. `ABCD EFGH ...`.
    #[must_use]
    pub fn to_string_grouped(&self) -> String {
        let raw = BASE32_NOPAD.encode(&self.0);
        let mut out = String::with_capacity(raw.len() + raw.len() / 4);
        for (i, c) in raw.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                out.push(' ');
            }
            out.push(c);
        }
        out
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.to_string_grouped())
    }
}

fn concat(domain: &[u8], parts: &[&[u8]]) -> Vec<u8> {
    let len = domain.len() + parts.iter().map(|p| p.len()).sum::<usize>();
    let mut data = Vec::with_capacity(len);
    data.extend_from_slice(domain);
    for part in parts {
        data.extend_from_slice(part);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    /// The device id is BLAKE3(pk)[0..8], so the first 8 bytes of the same
    /// hash the fingerprint truncates to 20.
    #[test]
    fn device_id_is_the_hash_prefix() {
        let key = IdentityKey::from_seed(&[3; 32]).public();
        assert_eq!(key.device_id()[..], key.fingerprint().as_bytes()[..8]);
        let other = IdentityKey::from_seed(&[4; 32]).public();
        assert_ne!(key.device_id(), other.device_id());
    }

    #[test]
    fn sign_verify_and_domain_separation() {
        let mut rng = ChaCha20Rng::from_seed([7; 32]);
        let key = IdentityKey::generate(&mut rng);
        let public = key.public();
        let sig = key.sign(b"MEM/v1/test", &[b"a", b"bc"]);
        assert_eq!(public.verify(b"MEM/v1/test", &[b"ab", b"c"], &sig), Ok(()));
        assert_eq!(
            public.verify(b"MEM/v1/other", &[b"abc"], &sig),
            Err(CryptoError::BadSignature)
        );
    }

    #[test]
    fn seed_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([1; 32]);
        let key = IdentityKey::generate(&mut rng);
        let restored = IdentityKey::from_seed(&key.to_seed());
        assert_eq!(restored.public(), key.public());
    }

    #[test]
    fn rejects_weak_public_key() {
        // The identity point (small order) encodes as 0x01 followed by zeros.
        let mut weak = [0u8; 32];
        weak[0] = 1;
        assert_eq!(
            IdentityPublic::from_bytes(&weak),
            Err(CryptoError::InvalidPublicKey)
        );
    }

    #[test]
    fn fingerprint_format() {
        let mut rng = ChaCha20Rng::from_seed([2; 32]);
        let fp = IdentityKey::generate(&mut rng).public().fingerprint();
        let s = fp.to_string_grouped();
        assert_eq!(s.len(), 32 + 7);
        assert!(s.split(' ').all(|g| g.len() == 4));
    }

    #[test]
    fn debug_never_prints_secrets() {
        let mut rng = ChaCha20Rng::from_seed([3; 32]);
        let key = IdentityKey::generate(&mut rng);
        assert_eq!(alloc::format!("{key:?}"), "IdentityKey(<secret>)");
    }
}
