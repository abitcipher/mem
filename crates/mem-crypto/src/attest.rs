//! Attestation keys and signatures.
//!
//! An attestation key is derived from the identity seed for one relying party, so it is the
//! same at that party every time, different at every other one, and cannot be linked to the
//! identity. It signs attestations only.

use core::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::identity::IdentityKey;

/// Signature domain of an attestation.
pub const ATTESTATION_DOMAIN: &[u8] = b"MEM/v1/attestation";
/// HKDF info prefix of the per-relying-party key.
pub const KEY_INFO: &[u8] = b"MEM/v1/attest-key";
/// HKDF info of the emoji check.
pub const SAS_INFO: &[u8] = b"MEM/v1/attest-sas";
/// Emoji in the check, as in pairing.
pub const SAS_LEN: usize = 6;

/// Longest relying-party origin accepted for key derivation (as `mem_proto::attest`).
const MAX_ORIGIN_LEN: usize = 270;

/// The attestation key for one relying party. Zeroized on drop; never printed.
pub struct AttestKey(SigningKey);

impl AttestKey {
    /// Derives the key for `rp_origin` from the identity:
    /// seed = `HKDF-SHA256(salt = "", ikm = identity seed, info = KEY_INFO || rp_origin, L = 32)`.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] if the origin is empty or longer than an origin can be
    /// (the caller passes an origin already checked by `mem_proto::attest`).
    pub fn derive(identity: &IdentityKey, rp_origin: &str) -> Result<Self, CryptoError> {
        if rp_origin.is_empty() || rp_origin.len() > MAX_ORIGIN_LEN {
            return Err(CryptoError::InvalidPublicKey);
        }
        let ikm = identity.to_seed();
        let hk = Hkdf::<Sha256>::new(None, ikm.as_ref());
        let mut seed = Zeroizing::new([0u8; 32]);
        hk.expand_multi_info(&[KEY_INFO, rp_origin.as_bytes()], seed.as_mut())
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    /// The public key, which goes into offers and statements.
    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    /// Signs `ATTESTATION_DOMAIN || rp_challenge || statement` (the statement's canonical CBOR).
    #[must_use]
    pub fn sign(&self, rp_challenge: &[u8; 16], statement: &[u8]) -> [u8; 64] {
        self.0
            .sign(&signed_bytes(rp_challenge, statement))
            .to_bytes()
    }
}

impl fmt::Debug for AttestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AttestKey(<secret>)")
    }
}

/// Checks an attestation signature, strictly (rejects weak keys and malleable signatures).
///
/// # Errors
///
/// [`CryptoError::InvalidPublicKey`] for a bad or weak key, [`CryptoError::BadSignature`] if
/// the signature does not verify.
pub fn verify(
    attest_pk: &[u8; 32],
    rp_challenge: &[u8; 16],
    statement: &[u8],
    signature: &[u8; 64],
) -> Result<(), CryptoError> {
    let key = VerifyingKey::from_bytes(attest_pk).map_err(|_| CryptoError::InvalidPublicKey)?;
    if key.is_weak() {
        return Err(CryptoError::InvalidPublicKey);
    }
    key.verify_strict(
        &signed_bytes(rp_challenge, statement),
        &Signature::from_bytes(signature),
    )
    .map_err(|_| CryptoError::BadSignature)
}

/// The emoji check both phones show: indexes into the 64-entry pairing
/// table, `HKDF-SHA256(ikm = BLAKE3(statement), info = SAS_INFO, L = 6) & 63`.
#[must_use]
pub fn sas(statement: &[u8]) -> [u8; SAS_LEN] {
    let ikm = blake3::hash(statement);
    let hk = Hkdf::<Sha256>::new(None, ikm.as_bytes());
    let mut out = [0u8; SAS_LEN];
    if hk.expand(SAS_INFO, &mut out).is_err() {
        out.fill(0);
    }
    for b in &mut out {
        *b &= 63;
    }
    out
}

fn signed_bytes(rp_challenge: &[u8; 16], statement: &[u8]) -> alloc::vec::Vec<u8> {
    let mut m = alloc::vec::Vec::with_capacity(ATTESTATION_DOMAIN.len() + 16 + statement.len());
    m.extend_from_slice(ATTESTATION_DOMAIN);
    m.extend_from_slice(rp_challenge);
    m.extend_from_slice(statement);
    m
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    fn identity(n: u8) -> IdentityKey {
        IdentityKey::from_seed(&[n; 32])
    }

    #[test]
    fn one_key_per_relying_party_unlinkable_to_the_identity() {
        let id = identity(1);
        let a = AttestKey::derive(&id, "https://basa.ua").unwrap();
        let again = AttestKey::derive(&id, "https://basa.ua").unwrap();
        let other = AttestKey::derive(&id, "https://example.org").unwrap();
        assert_eq!(a.public(), again.public());
        assert_ne!(a.public(), other.public());
        assert_ne!(a.public(), *id.public().as_bytes());
        let someone = AttestKey::derive(&identity(2), "https://basa.ua").unwrap();
        assert_ne!(a.public(), someone.public());
        assert!(AttestKey::derive(&id, "").is_err());
    }

    #[test]
    fn sign_and_verify() {
        let k = AttestKey::derive(&identity(1), "https://basa.ua").unwrap();
        let sig = k.sign(&[5; 16], b"statement");
        verify(&k.public(), &[5; 16], b"statement", &sig).unwrap();
        assert_eq!(
            verify(&k.public(), &[6; 16], b"statement", &sig),
            Err(CryptoError::BadSignature)
        );
        assert_eq!(
            verify(&k.public(), &[5; 16], b"statemenT", &sig),
            Err(CryptoError::BadSignature)
        );
        // An identity signature over the same bytes under another domain does not pass.
        let id = identity(1);
        let forged = id.sign(b"MEM/v1/card", &[&[5; 16], b"statement"]);
        assert!(verify(id.public().as_bytes(), &[5; 16], b"statement", &forged).is_err());
    }

    #[test]
    fn sas_is_deterministic_and_in_range() {
        assert_eq!(sas(b"x"), sas(b"x"));
        assert_ne!(sas(b"x"), sas(b"y"));
        assert!(sas(b"x").iter().all(|i| *i < 64));
    }
}
