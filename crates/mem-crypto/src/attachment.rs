//! Attachment encryption: one blob per file, sealed once with a random file key.
//!
//! The file is padded (ISO/IEC 7816-4: `0x80` then zeros) to a power-of-two bucket of at least
//! [`CHUNK_LEN`] bytes, so anyone holding the blob learns only the bucket. The padded file is
//! split into [`CHUNK_LEN`]-byte chunks, each sealed with ChaCha20-Poly1305 under the STREAM
//! construction as used by age v1: the 12-byte nonce is an 11-byte big-endian chunk counter
//! followed by a flag byte, `0x01` on the last chunk and `0x00` otherwise. Reordered, dropped,
//! appended or truncated chunks therefore fail to open.
//!
//! The payload key is `HKDF-SHA256(ikm = file key, salt = "", info = "MEM/v1/attachment")`.
//! The blob's content address is BLAKE3 of the ciphertext, so it reveals nothing about the file
//! and cannot be matched against known files.
//!
//! # Security
//!
//! The file key must be fresh and random for every attachment ([`FileKey::generate`]); it travels
//! only inside end-to-end encrypted payloads. Each (key, counter) pair is used once because a key
//! seals exactly one file.

use alloc::vec::Vec;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hkdf::Hkdf;
use rand_core::CryptoRng;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::CryptoError;

/// Plaintext bytes per chunk.
pub const CHUNK_LEN: usize = 64 * 1024;
/// Authentication tag bytes per chunk.
pub const TAG_LEN: usize = 16;
/// Ciphertext bytes per chunk.
pub const SEALED_CHUNK_LEN: usize = CHUNK_LEN + TAG_LEN;
/// Largest file accepted, in bytes (100 MiB).
pub const MAX_FILE_LEN: u64 = 100 * 1024 * 1024;

const INFO: &[u8] = b"MEM/v1/attachment";

/// The random key of one attachment.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct FileKey([u8; 32]);

impl FileKey {
    /// A fresh random key.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut k = [0u8; 32];
        rng.fill_bytes(&mut k);
        Self(k)
    }

    /// From the 32 bytes carried in an attachment reference.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The 32 bytes, for the attachment reference inside an encrypted payload only.
    ///
    /// # Security
    ///
    /// Never log these or put them anywhere outside an end-to-end encrypted payload or local
    /// encrypted storage.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn cipher(&self) -> Result<ChaCha20Poly1305, CryptoError> {
        let mut okm = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(None, &self.0)
            .expand(INFO, okm.as_mut())
            .map_err(|_| CryptoError::Seal)?;
        ChaCha20Poly1305::new_from_slice(okm.as_ref()).map_err(|_| CryptoError::Seal)
    }
}

impl core::fmt::Debug for FileKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("FileKey(..)")
    }
}

/// BLAKE3 of a sealed blob: how carriers, the hub and storage name it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ContentAddress(pub [u8; 32]);

impl ContentAddress {
    /// The address of `sealed`.
    #[must_use]
    pub fn of(sealed: &[u8]) -> Self {
        Self(*blake3::hash(sealed).as_bytes())
    }
}

/// Padded length of a file of `len` bytes: the smallest power of two that holds the file plus
/// the `0x80` marker, at least [`CHUNK_LEN`]; `None` above [`MAX_FILE_LEN`].
#[must_use]
pub fn padded_len(len: u64) -> Option<u64> {
    if len > MAX_FILE_LEN {
        return None;
    }
    Some((len + 1).next_power_of_two().max(CHUNK_LEN as u64))
}

/// Sealed length of a file of `len` bytes; `None` above [`MAX_FILE_LEN`].
#[must_use]
pub fn sealed_len(len: u64) -> Option<u64> {
    let padded = padded_len(len)?;
    Some(padded / CHUNK_LEN as u64 * SEALED_CHUNK_LEN as u64)
}

fn nonce(index: u64, last: bool) -> Result<Nonce, CryptoError> {
    // 11-byte counter: indexes beyond 2^88 cannot occur for files of at most 100 MiB, but are
    // refused rather than wrapped.
    if index >= 1 << 63 {
        return Err(CryptoError::Seal);
    }
    let mut n = [0u8; 12];
    n[3..11].copy_from_slice(&index.to_be_bytes());
    n[11] = u8::from(last);
    Ok(Nonce::from(n))
}

/// Seals one padded chunk of exactly [`CHUNK_LEN`] bytes; `last` marks the final chunk.
///
/// # Errors
///
/// [`CryptoError::Seal`] for a chunk of another length or an impossible index.
pub fn seal_chunk(
    key: &FileKey,
    index: u64,
    last: bool,
    chunk: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if chunk.len() != CHUNK_LEN {
        return Err(CryptoError::Seal);
    }
    key.cipher()?
        .encrypt(&nonce(index, last)?, chunk)
        .map_err(|_| CryptoError::Seal)
}

/// Opens one sealed chunk of exactly [`SEALED_CHUNK_LEN`] bytes.
///
/// # Errors
///
/// [`CryptoError::Open`] if the chunk is not the `index`-th one of this key's file, its `last`
/// flag differs, or it was altered.
pub fn open_chunk(
    key: &FileKey,
    index: u64,
    last: bool,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if sealed.len() != SEALED_CHUNK_LEN {
        return Err(CryptoError::Open);
    }
    let cipher = key.cipher().map_err(|_| CryptoError::Open)?;
    let nonce = nonce(index, last).map_err(|_| CryptoError::Open)?;
    cipher
        .decrypt(&nonce, sealed)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Open)
}

/// Pads and seals a whole file.
///
/// # Errors
///
/// [`CryptoError::Seal`] for a file above [`MAX_FILE_LEN`].
pub fn seal(key: &FileKey, file: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let padded_len = padded_len(file.len() as u64).ok_or(CryptoError::Seal)?;
    let padded_len = usize::try_from(padded_len).map_err(|_| CryptoError::Seal)?;
    let mut padded = Zeroizing::new(Vec::with_capacity(padded_len));
    padded.extend_from_slice(file);
    padded.push(0x80);
    padded.resize(padded_len, 0);
    let chunks = padded_len / CHUNK_LEN;
    let mut out = Vec::with_capacity(chunks * SEALED_CHUNK_LEN);
    for (i, chunk) in padded.chunks(CHUNK_LEN).enumerate() {
        out.extend_from_slice(&seal_chunk(key, i as u64, i + 1 == chunks, chunk)?);
    }
    Ok(out)
}

/// Opens a whole sealed blob and removes the padding. The blob must be exactly the sealed size
/// of a power-of-two bucket, and the file inside must be `expected_len` bytes long (the size
/// the attachment reference states).
///
/// # Errors
///
/// [`CryptoError::Open`] for a malformed, altered, truncated or extended blob, a wrong key, bad
/// padding, or a length other than `expected_len`.
pub fn open(
    key: &FileKey,
    sealed: &[u8],
    expected_len: u64,
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let want = sealed_len(expected_len).ok_or(CryptoError::Open)?;
    if sealed.len() as u64 != want {
        return Err(CryptoError::Open);
    }
    let chunks = sealed.len() / SEALED_CHUNK_LEN;
    let mut out = Zeroizing::new(Vec::with_capacity(chunks * CHUNK_LEN));
    for (i, chunk) in sealed.chunks(SEALED_CHUNK_LEN).enumerate() {
        out.extend_from_slice(&open_chunk(key, i as u64, i + 1 == chunks, chunk)?);
    }
    // Strip ISO/IEC 7816-4 padding: zeros, then exactly one 0x80.
    let end = out.iter().rposition(|&b| b != 0).ok_or(CryptoError::Open)?;
    if out[end] != 0x80 || end as u64 != expected_len {
        return Err(CryptoError::Open);
    }
    out.truncate(end);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn key() -> FileKey {
        FileKey::generate(&mut ChaCha20Rng::seed_from_u64(7))
    }

    #[test]
    fn round_trip_at_bucket_edges() {
        let k = key();
        for len in [0, 1, CHUNK_LEN - 1, CHUNK_LEN, CHUNK_LEN + 1, 300 * 1024] {
            let file: Vec<u8> = (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect();
            let sealed = seal(&k, &file).unwrap();
            assert_eq!(sealed.len() as u64, sealed_len(len as u64).unwrap());
            assert_eq!(&*open(&k, &sealed, len as u64).unwrap(), &file[..]);
        }
    }

    #[test]
    fn buckets_are_powers_of_two_from_one_chunk() {
        assert_eq!(padded_len(0), Some(CHUNK_LEN as u64));
        assert_eq!(padded_len(CHUNK_LEN as u64 - 1), Some(CHUNK_LEN as u64));
        assert_eq!(padded_len(CHUNK_LEN as u64), Some(2 * CHUNK_LEN as u64));
        assert_eq!(padded_len(MAX_FILE_LEN), Some(128 * 1024 * 1024));
        assert_eq!(padded_len(MAX_FILE_LEN + 1), None);
        // The small-attachment limit: up to 192 KiB of file fits a 256 KiB bucket.
        assert_eq!(padded_len(192 * 1024), Some(256 * 1024));
    }

    #[test]
    fn tampering_reordering_and_truncation_fail() {
        let k = key();
        let file = alloc::vec![9u8; 3 * CHUNK_LEN];
        let sealed = seal(&k, &file).unwrap(); // 4 chunks
        let len = file.len() as u64;

        let mut flipped = sealed.clone();
        flipped[10] ^= 1;
        assert!(open(&k, &flipped, len).is_err());

        let mut swapped = sealed.clone();
        let (a, b) = swapped.split_at_mut(SEALED_CHUNK_LEN);
        a.swap_with_slice(&mut b[..SEALED_CHUNK_LEN]);
        assert!(open(&k, &swapped, len).is_err());

        // Dropping the last chunk leaves no chunk with the last flag (and the wrong size).
        let short = &sealed[..2 * SEALED_CHUNK_LEN];
        assert!(open(&k, short, CHUNK_LEN as u64).is_err());
        assert!(open_chunk(&k, 1, true, &sealed[SEALED_CHUNK_LEN..2 * SEALED_CHUNK_LEN]).is_err());

        let other = FileKey::generate(&mut ChaCha20Rng::seed_from_u64(8));
        assert!(open(&other, &sealed, len).is_err());
        assert!(open(&k, &sealed, len - 1).is_err());
    }

    #[test]
    fn content_address_names_the_ciphertext() {
        let k = key();
        let sealed = seal(&k, b"hello").unwrap();
        let addr = ContentAddress::of(&sealed);
        // A fresh key gives a different blob and address for the same file.
        let other = seal(
            &FileKey::generate(&mut ChaCha20Rng::seed_from_u64(9)),
            b"hello",
        )
        .unwrap();
        assert_ne!(addr, ContentAddress::of(&other));
    }

    /// Fixed key and file. The SHA-256 of the sealed blob was computed independently (Python
    /// `cryptography`: HKDF-SHA256, ChaCha20-Poly1305, nonce `00 * 11 || 01`, 7816-4 padding);
    /// the BLAKE3 content address pins the rest.
    #[test]
    fn known_answer() {
        use sha2::Digest;
        let k = FileKey::from_bytes([0x42; 32]);
        let sealed = seal(&k, b"MEM attachment").unwrap();
        assert_eq!(sealed.len(), SEALED_CHUNK_LEN);
        assert_eq!(
            hex::encode(Sha256::digest(&sealed)),
            "c9330b64611ef75129810b3a6bbfb56d7fb07054e4b5cd3d073a32f7c25806fc"
        );
        assert_eq!(
            hex::encode(ContentAddress::of(&sealed).0),
            "0fc8d7148b0530bf7783cd27ae32d6f6fd2c4f4686aa2db77f800433bf28d387"
        );
    }
}
