//! The single error type of this crate.

/// Cryptographic failures. Variants carry no key material or plaintext.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// A public key is not a valid curve point or is a weak/low-order key.
    #[error("invalid public key")]
    InvalidPublicKey,
    /// A Diffie-Hellman result was not contributory (low-order input).
    #[error("non-contributory key agreement")]
    NonContributory,
    /// A signature did not verify.
    #[error("bad signature")]
    BadSignature,
    /// HPKE sealing failed.
    #[error("encryption failed")]
    Seal,
    /// HPKE or AEAD opening failed: wrong key, tampered ciphertext or associated data.
    #[error("decryption failed")]
    Open,
}
