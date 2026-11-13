//! Attachments (`spec/inner.cddl`): the reference inside a message (inner field 5) and the
//! parts that carry a sealed blob (content type [`CONTENT_TYPE_ATTACHMENT_PART`]).
//!
//! The file itself is sealed elsewhere (`mem-crypto`); this module only encodes and strictly
//! decodes the structures, with every length bounded before allocation.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::encode::Error as EncodeError;
use minicbor::{Decoder, Encoder};

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Content type: one part of a sealed attachment blob.
pub const CONTENT_TYPE_ATTACHMENT_PART: u64 = 9;
/// Most attachments in one message.
pub const MAX_ATTACHMENTS: usize = 16;
/// Largest file, in bytes (100 MiB).
pub const MAX_FILE_LEN: u64 = 100 * 1024 * 1024;
/// Longest file name, in bytes.
pub const MAX_NAME_LEN: usize = 255;
/// Longest media type, in bytes.
pub const MAX_MEDIA_TYPE_LEN: usize = 127;
/// Largest inline preview, in bytes.
pub const MAX_PREVIEW_LEN: usize = 8192;
/// Bytes of sealed blob per part (the last part may be shorter).
pub const PART_LEN: usize = 30 * 1024;
/// Largest sealed blob a part may belong to: a 100 MiB file padded to 128 MiB, plus one
/// 16-byte tag per 64 KiB chunk.
pub const MAX_SEALED_LEN: u64 = 128 * 1024 * 1024 + 2048 * 16;

/// One attached file, as named inside a message.
#[derive(Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    /// BLAKE3 of the sealed blob.
    pub address: [u8; 32],
    /// File size in bytes, at most [`MAX_FILE_LEN`].
    pub size: u64,
    /// The file key.
    pub key: [u8; 32],
    /// File name, 1..=[`MAX_NAME_LEN`] bytes.
    pub name: String,
    /// Media type, 1..=[`MAX_MEDIA_TYPE_LEN`] bytes.
    pub media_type: String,
    /// Inline preview, 1..=[`MAX_PREVIEW_LEN`] bytes.
    pub preview: Option<Vec<u8>>,
}

impl fmt::Debug for AttachmentRef {
    /// Never prints the key, name or preview.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentRef")
            .field("size", &self.size)
            .field("media_type", &self.media_type)
            .finish_non_exhaustive()
    }
}

impl AttachmentRef {
    fn check(&self, key: u64) -> Result<(), SchemaError> {
        let ok = self.size <= MAX_FILE_LEN
            && (1..=MAX_NAME_LEN).contains(&self.name.len())
            && (1..=MAX_MEDIA_TYPE_LEN).contains(&self.media_type.len())
            && self
                .preview
                .as_ref()
                .is_none_or(|p| (1..=MAX_PREVIEW_LEN).contains(&p.len()));
        if ok {
            Ok(())
        } else {
            Err(SchemaError::WrongLength { key })
        }
    }
}

/// Checks a list before encoding: 1..=[`MAX_ATTACHMENTS`] valid references.
///
/// # Errors
///
/// [`SchemaError::WrongLength`] for an empty or oversized list or an invalid reference.
pub fn check_list(list: &[AttachmentRef]) -> Result<(), SchemaError> {
    if list.is_empty() || list.len() > MAX_ATTACHMENTS {
        return Err(SchemaError::WrongLength { key: 5 });
    }
    list.iter().try_for_each(|a| a.check(5))
}

/// Writes inner field 5's value (callers check the list with [`check_list`] first).
pub(crate) fn write_list(
    e: &mut Encoder<&mut Vec<u8>>,
    list: &[AttachmentRef],
) -> Result<(), EncodeError<core::convert::Infallible>> {
    e.array(list.len() as u64)?;
    for a in list {
        e.map(5 + u64::from(a.preview.is_some()))?
            .u8(0)?
            .bytes(&a.address)?
            .u8(1)?
            .u64(a.size)?
            .u8(2)?
            .bytes(&a.key)?
            .u8(3)?
            .str(&a.name)?
            .u8(4)?
            .str(&a.media_type)?;
        if let Some(p) = &a.preview {
            e.u8(5)?.bytes(p)?;
        }
    }
    Ok(())
}

/// Reads inner field 5's value: a definite array of 1..=[`MAX_ATTACHMENTS`] references, each a
/// map with keys 0..=4 and optional 5, in canonical order, every field within its bounds.
pub(crate) fn read_list(d: &mut Decoder<'_>, key: u64) -> Result<Vec<AttachmentRef>, SchemaError> {
    let wrong = SchemaError::WrongType { key };
    let n = match d.array() {
        Ok(Some(n)) => usize::try_from(n).map_err(|_| SchemaError::WrongLength { key })?,
        _ => return Err(wrong),
    };
    if n == 0 || n > MAX_ATTACHMENTS {
        return Err(SchemaError::WrongLength { key });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let Ok(Some(entries @ (5 | 6))) = d.map() else {
            return Err(wrong);
        };
        expect_key(d, 0, key)?;
        let address = fixed::<32>(d, key)?;
        expect_key(d, 1, key)?;
        let size = d.u64().map_err(|_| wrong)?;
        expect_key(d, 2, key)?;
        let file_key = fixed::<32>(d, key)?;
        expect_key(d, 3, key)?;
        let name = String::from(d.str().map_err(|_| wrong)?);
        expect_key(d, 4, key)?;
        let media_type = String::from(d.str().map_err(|_| wrong)?);
        let preview = if entries == 6 {
            expect_key(d, 5, key)?;
            Some(d.bytes().map_err(|_| wrong)?.to_vec())
        } else {
            None
        };
        let a = AttachmentRef {
            address,
            size,
            key: file_key,
            name,
            media_type,
            preview,
        };
        a.check(key)?;
        out.push(a);
    }
    Ok(out)
}

/// Reads one map key and fails unless it is `want` (keys come in canonical order).
fn expect_key(d: &mut Decoder<'_>, want: u64, key: u64) -> Result<(), SchemaError> {
    if d.u64().ok() == Some(want) {
        Ok(())
    } else {
        Err(SchemaError::WrongType { key })
    }
}

fn fixed<const N: usize>(d: &mut Decoder<'_>, key: u64) -> Result<[u8; N], SchemaError> {
    let b = d.bytes().map_err(|_| SchemaError::WrongType { key })?;
    <[u8; N]>::try_from(b).map_err(|_| SchemaError::WrongLength { key })
}

/// One part of a sealed blob: `{0: address, 1: offset, 2: total, 3: bytes}`.
#[derive(Clone, PartialEq, Eq)]
pub struct AttachmentPart {
    /// Content address of the whole sealed blob.
    pub address: [u8; 32],
    /// Byte offset of this part, a multiple of [`PART_LEN`].
    pub offset: u64,
    /// Length of the whole sealed blob, at most [`MAX_SEALED_LEN`].
    pub total: u64,
    /// The part: [`PART_LEN`] bytes, or the rest of the blob for the last part.
    pub bytes: Vec<u8>,
}

impl fmt::Debug for AttachmentPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentPart")
            .field("offset", &self.offset)
            .field("total", &self.total)
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl AttachmentPart {
    /// Whether offset, total and length fit together.
    fn check(&self) -> Result<(), SchemaError> {
        let part = PART_LEN as u64;
        let ok = self.total > 0
            && self.total <= MAX_SEALED_LEN
            && self.offset.is_multiple_of(part)
            && self.offset < self.total
            && self.bytes.len() as u64 == part.min(self.total - self.offset);
        if ok {
            Ok(())
        } else {
            Err(SchemaError::WrongLength { key: 3 })
        }
    }

    /// The parts of `sealed`, in order.
    #[must_use]
    pub fn split(address: [u8; 32], sealed: &[u8]) -> Vec<Self> {
        sealed
            .chunks(PART_LEN)
            .enumerate()
            .map(|(i, bytes)| Self {
                address,
                offset: (i * PART_LEN) as u64,
                total: sealed.len() as u64,
                bytes: bytes.to_vec(),
            })
            .collect()
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if offset, total and length do not fit together.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(self.bytes.len() + 64, |e| {
            e.map(4)?
                .u8(0)?
                .bytes(&self.address)?
                .u8(1)?
                .u64(self.offset)?
                .u8(2)?
                .u64(self.total)?
                .u8(3)?
                .bytes(&self.bytes)?;
            Ok(())
        }))
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or inconsistent input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: PART_LEN + 64,
            max_container_len: 4,
        };
        let mut r = MapReader::new(input, &limits)?;
        let p = Self {
            address: r.array::<32>(0)?,
            offset: r.u64(1)?,
            total: r.u64(2)?,
            bytes: r.bytes(3)?.to_vec(),
        };
        r.finish()?;
        p.check()?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(preview: bool) -> AttachmentRef {
        AttachmentRef {
            address: [1; 32],
            size: 1234,
            key: [2; 32],
            name: "photo.jpg".into(),
            media_type: "image/jpeg".into(),
            preview: preview.then(|| alloc::vec![7; 100]),
        }
    }

    fn round_trip(list: &[AttachmentRef]) -> Result<Vec<AttachmentRef>, SchemaError> {
        let bytes = encode_with(64, |e| write_list(e, list));
        let mut d = Decoder::new(&bytes);
        let out = read_list(&mut d, 5)?;
        assert_eq!(d.position(), bytes.len());
        Ok(out)
    }

    #[test]
    fn references_round_trip() {
        let list = [sample(false), sample(true)];
        check_list(&list).unwrap();
        assert_eq!(round_trip(&list).unwrap(), list);
    }

    #[test]
    fn references_out_of_bounds_are_refused() {
        assert!(check_list(&[]).is_err());
        let mut long = sample(false);
        long.name = "x".repeat(MAX_NAME_LEN + 1);
        assert!(check_list(&[long.clone()]).is_err());
        assert!(round_trip(&[long]).is_err());
        let mut big = sample(false);
        big.size = MAX_FILE_LEN + 1;
        assert!(round_trip(&[big]).is_err());
        let mut empty_preview = sample(false);
        empty_preview.preview = Some(Vec::new());
        assert!(round_trip(&[empty_preview]).is_err());
        let many = alloc::vec![sample(false); MAX_ATTACHMENTS + 1];
        assert!(round_trip(&many).is_err());
    }

    #[test]
    fn parts_split_and_round_trip() {
        let sealed = alloc::vec![5u8; 2 * PART_LEN + 10];
        let parts = AttachmentPart::split([3; 32], &sealed);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2].bytes.len(), 10);
        for p in &parts {
            assert_eq!(&AttachmentPart::decode(&p.encode().unwrap()).unwrap(), p);
        }
    }

    #[test]
    fn inconsistent_parts_are_refused() {
        let good = AttachmentPart::split([3; 32], &[9u8; 100]).remove(0);
        let mut bad = good.clone();
        bad.offset = 1;
        assert!(bad.encode().is_err());
        let mut bad = good.clone();
        bad.total = 99;
        assert!(bad.encode().is_err());
        let mut bad = good;
        bad.total = MAX_SEALED_LEN + 1;
        assert!(bad.encode().is_err());
    }
}
