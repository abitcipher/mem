//! At-rest encryption for the local state snapshot.

use alloc::vec::Vec;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

use crate::error::CryptoError;

const AAD: &[u8] = b"MEM/v1/state";
const NONCE_LEN: usize = 24;

/// Encrypts `plaintext` as `nonce(24) || XChaCha20-Poly1305(key, nonce, plaintext, aad)`.
///
/// # Errors
///
/// [`CryptoError::Seal`] if the AEAD rejects the input (only for absurdly large plaintexts).
pub fn seal_state(
    key: &[u8; 32],
    plaintext: &[u8],
    rng: &mut impl CryptoRng,
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    let mut nonce = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| CryptoError::Seal)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypts the output of [`seal_state`].
///
/// # Errors
///
/// [`CryptoError::Open`] if the input is too short, the key is wrong, or the data was modified.
pub fn open_state(key: &[u8; 32], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (nonce, ct) = sealed
        .split_at_checked(NONCE_LEN)
        .ok_or(CryptoError::Open)?;
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Open)?;
    let cipher = XChaCha20Poly1305::new(&(*key).into());
    cipher
        .decrypt(&XNonce::from(nonce), Payload { msg: ct, aad: AAD })
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Open)
}

/// The SQLCipher key for the courier database, derived from the state key so the same
/// key is never used with two ciphers.
#[must_use]
pub fn courier_db_key(state_key: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(blake3::derive_key(
        "MEM v1 2026-10 courier-db-key",
        state_key,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn roundtrip_and_tamper() {
        let mut rng = ChaCha20Rng::from_seed([4; 32]);
        let key = [7u8; 32];
        let sealed = seal_state(&key, b"state", &mut rng).unwrap();
        assert_eq!(open_state(&key, &sealed).unwrap().as_slice(), b"state");
        let mut bad = sealed.clone();
        bad[30] ^= 1;
        assert_eq!(open_state(&key, &bad), Err(CryptoError::Open));
        assert_eq!(open_state(&[8u8; 32], &sealed), Err(CryptoError::Open));
        assert_eq!(open_state(&key, &sealed[..10]), Err(CryptoError::Open));
    }
}
