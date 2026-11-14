//! Files shared with a contact or a group (`spec/inner.cddl`): the list of what is offered
//! (content type [`CONTENT_TYPE_SHARE_CATALOG`]), a request to download one item
//! ([`CONTENT_TYPE_SHARE_REQUEST`]) and the answer carrying the file
//! ([`CONTENT_TYPE_SHARE_FILE`], with one attachment reference in inner field 5).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::attachment::{MAX_FILE_LEN, MAX_MEDIA_TYPE_LEN, MAX_NAME_LEN};
use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Content type: what a sender offers to download, replacing the list it sent before.
pub const CONTENT_TYPE_SHARE_CATALOG: u64 = 14;
/// Content type: a request to download one offered item.
pub const CONTENT_TYPE_SHARE_REQUEST: u64 = 15;
/// Content type: the answer to a request, with the file as the one attachment.
pub const CONTENT_TYPE_SHARE_FILE: u64 = 16;
/// At most this many items in one list.
pub const MAX_SHARE_ITEMS: usize = 512;
/// Longest share name, in bytes.
pub const MAX_SHARE_NAME_LEN: usize = 64;
/// Largest encoded list: it must fit the largest padding bucket with the rest of the inner.
pub const MAX_CATALOG_LEN: usize = 30 * 1024;

/// One offered file.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareItem {
    /// Random per share; says nothing about the file.
    pub item_id: [u8; 16],
    /// File name, 1..=[`MAX_NAME_LEN`] bytes.
    pub name: String,
    /// Size in bytes, at most [`MAX_FILE_LEN`].
    pub size: u64,
    /// Media type, 1..=[`MAX_MEDIA_TYPE_LEN`] bytes.
    pub media_type: String,
    /// Last change, Unix seconds, if known.
    pub modified_s: Option<u64>,
}

impl fmt::Debug for ShareItem {
    /// Never prints the file name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareItem")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// The list of what a sender offers in one share. An empty list withdraws the share.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareCatalog {
    /// Random, chosen by the sender.
    pub share_id: [u8; 16],
    /// The share's name, 1..=[`MAX_SHARE_NAME_LEN`] bytes.
    pub name: String,
    /// At most [`MAX_SHARE_ITEMS`].
    pub items: Vec<ShareItem>,
    /// The group the share is offered to, if it is not for one contact.
    pub group: Option<[u8; 16]>,
}

impl fmt::Debug for ShareCatalog {
    /// Never prints names.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareCatalog")
            .field("items", &self.items.len())
            .field("group", &self.group.is_some())
            .finish_non_exhaustive()
    }
}

fn check_text(s: &str, max: usize, key: u64) -> Result<(), SchemaError> {
    if (1..=max).contains(&s.len()) {
        Ok(())
    } else {
        Err(SchemaError::WrongLength { key })
    }
}

impl ShareItem {
    fn check(&self) -> Result<(), SchemaError> {
        check_text(&self.name, MAX_NAME_LEN, 2)?;
        check_text(&self.media_type, MAX_MEDIA_TYPE_LEN, 2)?;
        if self.size > MAX_FILE_LEN {
            return Err(SchemaError::WrongLength { key: 2 });
        }
        Ok(())
    }
}

impl ShareCatalog {
    fn check(&self) -> Result<(), SchemaError> {
        check_text(&self.name, MAX_SHARE_NAME_LEN, 1)?;
        if self.items.len() > MAX_SHARE_ITEMS {
            return Err(SchemaError::WrongLength { key: 2 });
        }
        self.items.iter().try_for_each(ShareItem::check)
    }

    /// Canonical CBOR `{0: share_id, 1: name, 2: [* item], ? 3: group}`.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] for a field out of bounds, or a list longer than [`MAX_CATALOG_LEN`]
    /// once encoded (split it into several shares).
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        let bytes = encode_with(256 + self.items.len() * 64, |e| {
            e.map(3 + u64::from(self.group.is_some()))?
                .u8(0)?
                .bytes(&self.share_id)?
                .u8(1)?
                .str(&self.name)?
                .u8(2)?
                .array(self.items.len() as u64)?;
            for i in &self.items {
                e.map(4 + u64::from(i.modified_s.is_some()))?
                    .u8(0)?
                    .bytes(&i.item_id)?
                    .u8(1)?
                    .str(&i.name)?
                    .u8(2)?
                    .u64(i.size)?
                    .u8(3)?
                    .str(&i.media_type)?;
                if let Some(m) = i.modified_s {
                    e.u8(4)?.u64(m)?;
                }
            }
            if let Some(g) = &self.group {
                e.u8(3)?.bytes(g)?;
            }
            Ok(())
        });
        if bytes.len() > MAX_CATALOG_LEN {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        Ok(bytes)
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_CATALOG_LEN,
            max_container_len: MAX_SHARE_ITEMS as u64,
        };
        let mut r = MapReader::new(input, &limits)?;
        let share_id = r.array::<16>(0)?;
        let name = String::from(r.text(1)?);
        let items = read_items(r.nested(2)?)?;
        let group = if r.has(3) {
            Some(r.array::<16>(3)?)
        } else {
            None
        };
        r.finish()?;
        let c = Self {
            share_id,
            name,
            items,
            group,
        };
        c.check()?;
        Ok(c)
    }
}

/// Reads the item list: a definite array of maps with keys 0..=3 and optional 4, in order.
fn read_items(d: &mut Decoder<'_>) -> Result<Vec<ShareItem>, SchemaError> {
    const KEY: u64 = 2;
    let wrong = SchemaError::WrongType { key: KEY };
    let n = match d.array() {
        Ok(Some(n)) => usize::try_from(n).map_err(|_| SchemaError::WrongLength { key: KEY })?,
        _ => return Err(wrong),
    };
    if n > MAX_SHARE_ITEMS {
        return Err(SchemaError::WrongLength { key: KEY });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let Ok(Some(entries @ (4 | 5))) = d.map() else {
            return Err(wrong);
        };
        expect_key(d, 0)?;
        let item_id = fixed16(d)?;
        expect_key(d, 1)?;
        let name = String::from(d.str().map_err(|_| wrong)?);
        expect_key(d, 2)?;
        let size = d.u64().map_err(|_| wrong)?;
        expect_key(d, 3)?;
        let media_type = String::from(d.str().map_err(|_| wrong)?);
        let modified_s = if entries == 5 {
            expect_key(d, 4)?;
            Some(d.u64().map_err(|_| wrong)?)
        } else {
            None
        };
        out.push(ShareItem {
            item_id,
            name,
            size,
            media_type,
            modified_s,
        });
    }
    Ok(out)
}

fn expect_key(d: &mut Decoder<'_>, want: u64) -> Result<(), SchemaError> {
    match d.u64() {
        Ok(k) if k == want => Ok(()),
        _ => Err(SchemaError::UnexpectedKey { expected: want }),
    }
}

fn fixed16(d: &mut Decoder<'_>) -> Result<[u8; 16], SchemaError> {
    let b = d.bytes().map_err(|_| SchemaError::WrongType { key: 2 })?;
    <[u8; 16]>::try_from(b).map_err(|_| SchemaError::WrongLength { key: 2 })
}

/// One item of one share: the content of a request and of the answer carrying the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShareRef {
    /// The share.
    pub share_id: [u8; 16],
    /// The item in it.
    pub item_id: [u8; 16],
}

impl ShareRef {
    /// Canonical CBOR `{0: share_id, 1: item_id}`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(40, |e| {
            e.map(2)?
                .u8(0)?
                .bytes(&self.share_id)?
                .u8(1)?
                .bytes(&self.item_id)?;
            Ok(())
        })
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: 48,
            max_container_len: 2,
        };
        let mut r = MapReader::new(input, &limits)?;
        let share_id = r.array::<16>(0)?;
        let item_id = r.array::<16>(1)?;
        r.finish()?;
        Ok(Self { share_id, item_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(i: u8) -> ShareItem {
        ShareItem {
            item_id: [i; 16],
            name: alloc::format!("photo-{i}.jpg"),
            size: 1000 + u64::from(i),
            media_type: "image/jpeg".into(),
            modified_s: i.is_multiple_of(2).then_some(1_790_000_000),
        }
    }

    #[test]
    fn catalogs_and_refs_round_trip() {
        let c = ShareCatalog {
            share_id: [1; 16],
            name: "Holiday".into(),
            items: (0..5).map(item).collect(),
            group: Some([9; 16]),
        };
        assert_eq!(ShareCatalog::decode(&c.encode().unwrap()).unwrap(), c);
        let withdrawn = ShareCatalog {
            items: Vec::new(),
            group: None,
            ..c
        };
        assert_eq!(
            ShareCatalog::decode(&withdrawn.encode().unwrap()).unwrap(),
            withdrawn
        );
        let r = ShareRef {
            share_id: [1; 16],
            item_id: [2; 16],
        };
        assert_eq!(ShareRef::decode(&r.encode()).unwrap(), r);
    }

    #[test]
    fn out_of_bounds_lists_are_refused() {
        let mut c = ShareCatalog {
            share_id: [1; 16],
            name: String::new(),
            items: Vec::new(),
            group: None,
        };
        assert!(c.encode().is_err(), "empty name");
        c.name = "x".into();
        c.items = (0..=u8::MAX)
            .chain(0..=u8::MAX)
            .chain([0])
            .map(item)
            .collect();
        assert!(c.encode().is_err(), "more than 512 items");
        c.items = alloc::vec![ShareItem {
            size: MAX_FILE_LEN + 1,
            ..item(1)
        }];
        assert!(c.encode().is_err(), "file too large");
        // Long names hit the encoded size limit before the item limit.
        c.items = (0..200)
            .map(|i| ShareItem {
                name: "n".repeat(MAX_NAME_LEN),
                ..item(i)
            })
            .collect();
        assert!(c.encode().is_err(), "over the encoded size");
    }

    #[test]
    fn hostile_input_is_refused() {
        let good = ShareCatalog {
            share_id: [1; 16],
            name: "a".into(),
            items: alloc::vec![item(2)],
            group: None,
        }
        .encode()
        .unwrap();
        for cut in 0..good.len() {
            assert!(ShareCatalog::decode(&good[..cut]).is_err());
        }
        // An unknown extra key in the map.
        let mut extra = good.clone();
        extra[0] += 1;
        extra.extend([0x05, 0x00]);
        assert!(ShareCatalog::decode(&extra).is_err());
        assert!(ShareRef::decode(&[0xa1, 0x00, 0x40]).is_err());
    }
}
