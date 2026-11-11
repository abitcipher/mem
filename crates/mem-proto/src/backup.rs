//! Backup file and contents (`spec/backup.cddl`).
//!
//! The file is public-looking data: a header (KDF parameters, salt, nonce) and a ciphertext. Only
//! the recovery secret opens it. Everything is decoded with bounds before use.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// First element of every backup file.
pub const MAGIC: &str = "MEM-BACKUP";
/// File format version.
pub const BACKUP_VERSION: u8 = 1;
/// Contents format version.
pub const CONTENTS_VERSION: u8 = 1;
/// Largest ciphertext accepted.
pub const MAX_CIPHERTEXT_LEN: usize = 64 * 1024 * 1024;
/// Text armor prefix for small backups sent through the share sheet.
pub const ARMOR_PREFIX: &str = "membackup1:";
/// Largest app-version string.
pub const MAX_APP_VERSION_LEN: usize = 32;

const FILE_LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_CIPHERTEXT_LEN + 128,
    max_container_len: 6,
};

/// Argon2id parameters as stored (the ranges are enforced by `mem-crypto`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfFields {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Passes.
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

/// The authenticated header of a backup file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupHeader {
    /// Key derivation parameters.
    pub kdf: KdfFields,
    /// Argon2id salt, fresh per backup.
    pub salt: [u8; 16],
    /// XChaCha20-Poly1305 nonce, fresh per backup.
    pub nonce: [u8; 24],
}

impl BackupHeader {
    /// The associated data for the AEAD: the first five elements as a CBOR array.
    #[must_use]
    pub fn aad(&self) -> Vec<u8> {
        encode_with(80, |e| {
            e.array(5)?;
            self.write(e)?;
            Ok(())
        })
    }

    fn write(
        &self,
        e: &mut minicbor::Encoder<&mut Vec<u8>>,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>> {
        e.str(MAGIC)?
            .u8(BACKUP_VERSION)?
            .array(3)?
            .u32(self.kdf.m_kib)?
            .u32(self.kdf.t)?
            .u32(self.kdf.p)?
            .bytes(&self.salt)?
            .bytes(&self.nonce)?;
        Ok(())
    }
}

/// A backup file: header plus ciphertext.
#[derive(Clone, PartialEq, Eq)]
pub struct BackupFile {
    /// Header (authenticated).
    pub header: BackupHeader,
    /// Ciphertext of the padded [`BackupContents`].
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for BackupFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackupFile")
            .field("header", &self.header)
            .field("ciphertext_len", &self.ciphertext.len())
            .finish()
    }
}

impl BackupFile {
    /// Canonical CBOR encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(self.ciphertext.len() + 96, |e| {
            e.array(6)?;
            self.header.write(e)?;
            e.bytes(&self.ciphertext)?;
            Ok(())
        })
    }

    /// Decodes an untrusted backup file. KDF parameter ranges are checked by `mem-crypto`
    /// before any key derivation.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        if input.len() > FILE_LIMITS.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: FILE_LIMITS.max_input_len,
            });
        }
        cbor::scan(input, &FILE_LIMITS)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(6)) {
            return Err(SchemaError::MapShape.into());
        }
        if d.str().ok() != Some(MAGIC) {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        if d.u8().ok() != Some(BACKUP_VERSION) {
            return Err(SchemaError::UnknownValue { key: 1 }.into());
        }
        if d.array().ok() != Some(Some(3)) {
            return Err(SchemaError::WrongType { key: 2 }.into());
        }
        let kdf = KdfFields {
            m_kib: d.u32().map_err(|_| SchemaError::WrongType { key: 2 })?,
            t: d.u32().map_err(|_| SchemaError::WrongType { key: 2 })?,
            p: d.u32().map_err(|_| SchemaError::WrongType { key: 2 })?,
        };
        let salt = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 3 })?;
        let nonce = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 24]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 4 })?;
        let ciphertext = d.bytes().map_err(|_| SchemaError::WrongType { key: 5 })?;
        if ciphertext.len() > MAX_CIPHERTEXT_LEN {
            return Err(SchemaError::WrongLength { key: 5 }.into());
        }
        if d.position() != input.len() {
            return Err(SchemaError::MapShape.into());
        }
        Ok(Self {
            header: BackupHeader { kdf, salt, nonce },
            ciphertext: ciphertext.to_vec(),
        })
    }
}

/// What a backup holds once decrypted.
#[derive(Clone, PartialEq, Eq)]
pub struct BackupContents {
    /// When the backup was made, Unix ms.
    pub created_unix_ms: u64,
    /// The app version that wrote it.
    pub app_version: String,
    /// The core state snapshot (secret: never print, never cross FFI).
    pub snapshot: Vec<u8>,
    /// Whether message history is included.
    pub history: bool,
}

impl fmt::Debug for BackupContents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackupContents")
            .field("created_unix_ms", &self.created_unix_ms)
            .field("history", &self.history)
            .finish_non_exhaustive()
    }
}

impl BackupContents {
    /// Canonical CBOR encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(self.snapshot.len() + 64, |e| {
            e.map(5)?
                .u8(0)?
                .u8(CONTENTS_VERSION)?
                .u8(1)?
                .u64(self.created_unix_ms)?
                .u8(2)?
                .str(&self.app_version)?
                .u8(3)?
                .bytes(&self.snapshot)?
                .u8(4)?
                .bool(self.history)?;
            Ok(())
        })
    }

    /// Decodes decrypted contents. They were authenticated, but are still checked.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed or unsupported contents.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_CIPHERTEXT_LEN,
            max_container_len: 5,
        };
        let mut r = MapReader::new(input, &limits)?;
        if r.u8(0)? != CONTENTS_VERSION {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let created_unix_ms = r.u64(1)?;
        let app_version = r.text(2)?;
        if app_version.is_empty() || app_version.len() > MAX_APP_VERSION_LEN {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        let app_version = String::from(app_version);
        let snapshot = r.bytes(3)?.to_vec();
        let history = r.bool(4)?;
        r.finish()?;
        Ok(Self {
            created_unix_ms,
            app_version,
            snapshot,
            history,
        })
    }
}

/// Pads backup plaintext (ISO/IEC 7816-4) to the next power of two of at least 4 KiB, so the
/// file size reveals only a coarse size class.
#[must_use]
pub fn pad(data: &[u8]) -> Vec<u8> {
    let target = (data.len() + 1).max(4096).next_power_of_two();
    let mut out = Vec::with_capacity(target);
    out.extend_from_slice(data);
    out.push(0x80);
    out.resize(target, 0);
    out
}

/// Removes [`pad`] padding.
///
/// # Errors
///
/// [`SchemaError::MapShape`] if the length is not a valid size class or the padding is malformed.
pub fn unpad(padded: &[u8]) -> Result<&[u8], ProtoError> {
    if padded.len() < 4096 || !padded.len().is_power_of_two() {
        return Err(SchemaError::MapShape.into());
    }
    let end = padded
        .iter()
        .rposition(|b| *b != 0)
        .ok_or(SchemaError::MapShape)?;
    match padded.get(end) {
        Some(0x80) => padded
            .get(..end)
            .ok_or_else(|| SchemaError::MapShape.into()),
        _ => Err(SchemaError::MapShape.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn file() -> BackupFile {
        BackupFile {
            header: BackupHeader {
                kdf: KdfFields {
                    m_kib: 65_536,
                    t: 3,
                    p: 1,
                },
                salt: [1; 16],
                nonce: [2; 24],
            },
            ciphertext: vec![3; 100],
        }
    }

    #[test]
    fn file_roundtrip_and_aad_is_the_header_prefix() {
        let f = file();
        let bytes = f.encode();
        assert_eq!(BackupFile::decode(&bytes).unwrap(), f);
        let aad = f.header.aad();
        // The AAD is the same five elements, under an array(5) head instead of array(6).
        assert_eq!(aad[0], 0x85);
        assert_eq!(bytes[0], 0x86);
        assert_eq!(&aad[1..], &bytes[1..aad.len()]);
    }

    #[test]
    fn file_rejects_bad_input() {
        let good = file().encode();
        let mut magic = good.clone();
        magic[2] ^= 1;
        assert!(BackupFile::decode(&magic).is_err());
        let mut trailing = good.clone();
        trailing.push(0);
        assert!(BackupFile::decode(&trailing).is_err());
        assert!(BackupFile::decode(&good[..good.len() - 1]).is_err());
        assert!(BackupFile::decode(&[0x80]).is_err());
    }

    #[test]
    fn contents_roundtrip_and_bounds() {
        let c = BackupContents {
            created_unix_ms: 1_792_022_400_000,
            app_version: "0.2.0".into(),
            snapshot: vec![9; 50],
            history: true,
        };
        assert_eq!(BackupContents::decode(&c.encode()).unwrap(), c);
        let long = BackupContents {
            app_version: "x".repeat(MAX_APP_VERSION_LEN + 1),
            ..c.clone()
        };
        assert!(BackupContents::decode(&long.encode()).is_err());
    }

    #[test]
    fn padding_size_classes() {
        for len in [0usize, 1, 4095, 4096, 10_000, 70_000] {
            let data = vec![0xAB; len];
            let p = pad(&data);
            assert!(p.len() >= 4096 && p.len().is_power_of_two(), "{len}");
            assert_eq!(unpad(&p).unwrap(), &data[..]);
        }
        assert!(unpad(&[0u8; 4096]).is_err());
        assert!(unpad(&[0x80; 5000]).is_err());
    }
}
