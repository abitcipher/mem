//! Bounded, deterministic CBOR primitives shared by all MEM decoders.
//!
//! [`scan`] runs before any typed decoding. It checks well-formedness without recursion and
//! rejects everything outside the deterministic subset MEM uses:
//!
//! - definite lengths only;
//! - shortest-form integers and lengths;
//! - nesting depth at most [`MAX_DEPTH`];
//! - bounded container lengths;
//! - no tags, floats or `undefined`.
//!
//! The typed decoders then read fields strictly in canonical key order. Together this
//! guarantees that every accepted input is the unique deterministic encoding of its value.

use alloc::vec::Vec;
use core::convert::Infallible;

use minicbor::{Decoder, Encoder, encode};

use crate::error::{CborError, ProtoError, SchemaError};

/// Maximum container nesting depth.
pub const MAX_DEPTH: usize = 8;

/// Bounds applied to untrusted input before decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum total input length in bytes.
    pub max_input_len: usize,
    /// Maximum number of entries in any single array or map.
    pub max_container_len: u64,
}

const MAJOR_UINT: u8 = 0;
const MAJOR_NINT: u8 = 1;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const MAJOR_TAG: u8 = 6;
const MAJOR_SIMPLE: u8 = 7;

const SIMPLE_FALSE: u8 = 20;
const SIMPLE_TRUE: u8 = 21;
const SIMPLE_NULL: u8 = 22;

/// Checks that `input` is exactly one well-formed CBOR item within `limits` and the
/// deterministic subset described in the module docs.
///
/// Does not check `max_input_len`; callers check that first so that oversized input
/// costs nothing.
///
/// # Errors
///
/// Returns the first [`CborError`] found, scanning front to back.
///
/// # Security
///
/// Runs in `O(input.len())` time and constant memory, never recurses and never panics.
/// Declared lengths are checked against the remaining input before use, so a hostile header
/// cannot cause a large allocation in later decoding.
pub fn scan(input: &[u8], limits: &DecodeLimits) -> Result<(), CborError> {
    let mut pos = 0usize;
    // `pending[i]` = items still expected in the container open at depth `i + 1`.
    let mut pending = [0u64; MAX_DEPTH];
    let mut depth = 0usize;

    loop {
        let (major, info, arg) = read_head(input, &mut pos)?;
        let remaining = input.len().saturating_sub(pos);
        let children = match major {
            MAJOR_UINT | MAJOR_NINT => 0,
            MAJOR_BYTES | MAJOR_TEXT => {
                let len = usize::try_from(arg).map_err(|_| CborError::Truncated)?;
                if len > remaining {
                    return Err(CborError::Truncated);
                }
                let end = pos + len;
                if major == MAJOR_TEXT {
                    let text = input.get(pos..end).ok_or(CborError::Truncated)?;
                    core::str::from_utf8(text).map_err(|_| CborError::InvalidUtf8)?;
                }
                pos = end;
                0
            }
            MAJOR_ARRAY | MAJOR_MAP => {
                if arg > limits.max_container_len {
                    return Err(CborError::ContainerTooLong {
                        max: limits.max_container_len,
                    });
                }
                let items = if major == MAJOR_MAP {
                    arg.saturating_mul(2)
                } else {
                    arg
                };
                // Every item takes at least one byte; reject impossible counts early.
                if items > u64::try_from(remaining).unwrap_or(u64::MAX) {
                    return Err(CborError::Truncated);
                }
                items
            }
            MAJOR_TAG => return Err(CborError::TagNotAllowed),
            // Only the one-byte forms of false/true/null; `f814` etc. are not well-formed.
            _ => match info {
                SIMPLE_FALSE | SIMPLE_TRUE | SIMPLE_NULL => 0,
                _ => return Err(CborError::SimpleNotAllowed),
            },
        };

        if children > 0 {
            let slot = pending
                .get_mut(depth)
                .ok_or(CborError::DepthExceeded { max: MAX_DEPTH })?;
            *slot = children;
            depth += 1;
            continue;
        }

        // A complete item: account for it in its parents, closing finished containers.
        loop {
            let Some(parent) = depth.checked_sub(1) else {
                // The top-level item is complete.
                return if pos == input.len() {
                    Ok(())
                } else {
                    Err(CborError::TrailingBytes)
                };
            };
            let slot = pending.get_mut(parent).ok_or(CborError::Truncated)?;
            *slot -= 1;
            if *slot > 0 {
                break;
            }
            depth = parent;
        }
    }
}

/// Encodes with `f` into a fresh buffer.
///
/// Writing to a `Vec` cannot fail (`Error = Infallible`) and MEM encoders emit no custom
/// messages, so the result carries no information and is discarded.
pub(crate) fn encode_with(
    capacity: usize,
    f: impl FnOnce(&mut Encoder<&mut Vec<u8>>) -> Result<(), encode::Error<Infallible>>,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(capacity);
    let _: Result<(), encode::Error<Infallible>> = f(&mut Encoder::new(&mut buf));
    buf
}

/// Strict reader for a top-level CBOR map with integer keys in canonical (ascending) order.
///
/// Callers read fields in key order; optional fields use [`MapReader::has`]. [`MapReader::finish`]
/// verifies that every declared entry was consumed, which rejects unknown keys.
pub(crate) struct MapReader<'b> {
    d: Decoder<'b>,
    entries: u64,
    read: u64,
}

impl<'b> MapReader<'b> {
    /// Checks size and structure with [`scan`], then opens the top-level map.
    pub(crate) fn new(input: &'b [u8], limits: &DecodeLimits) -> Result<Self, ProtoError> {
        if input.len() > limits.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: limits.max_input_len,
            });
        }
        scan(input, limits)?;
        let mut d = Decoder::new(input);
        let Ok(Some(entries)) = d.map() else {
            return Err(SchemaError::MapShape.into());
        };
        Ok(Self {
            d,
            entries,
            read: 0,
        })
    }

    /// Whether the next key is `key` (for optional fields).
    pub(crate) fn has(&mut self, key: u64) -> bool {
        self.read < self.entries && self.d.probe().u64().ok() == Some(key)
    }

    fn key(&mut self, key: u64) -> Result<(), SchemaError> {
        if self.read >= self.entries {
            return Err(SchemaError::UnexpectedKey { expected: key });
        }
        match self.d.u64() {
            Ok(k) if k == key => {
                self.read += 1;
                Ok(())
            }
            _ => Err(SchemaError::UnexpectedKey { expected: key }),
        }
    }

    pub(crate) fn u8(&mut self, key: u64) -> Result<u8, SchemaError> {
        self.key(key)?;
        self.d.u8().map_err(|_| SchemaError::WrongType { key })
    }

    pub(crate) fn u64(&mut self, key: u64) -> Result<u64, SchemaError> {
        self.key(key)?;
        self.d.u64().map_err(|_| SchemaError::WrongType { key })
    }

    pub(crate) fn bool(&mut self, key: u64) -> Result<bool, SchemaError> {
        self.key(key)?;
        self.d.bool().map_err(|_| SchemaError::WrongType { key })
    }

    pub(crate) fn bytes(&mut self, key: u64) -> Result<&'b [u8], SchemaError> {
        self.key(key)?;
        self.d.bytes().map_err(|_| SchemaError::WrongType { key })
    }

    pub(crate) fn array<const N: usize>(&mut self, key: u64) -> Result<[u8; N], SchemaError> {
        let b = self.bytes(key)?;
        <[u8; N]>::try_from(b).map_err(|_| SchemaError::WrongLength { key })
    }

    /// A definite-length array of 1..=`max` byte strings of exactly 16 bytes (message ids).
    pub(crate) fn id_list(&mut self, key: u64, max: usize) -> Result<Vec<[u8; 16]>, SchemaError> {
        self.key(key)?;
        read_id_list(&mut self.d, key, max)
    }

    /// `{0: bstr .size 32, 1: uint32, ? 2: bstr .size 1216, ? 3: bstr .size 8,
    /// ? 4: bstr .cbor device_list, ? 5: uint32 (non-zero)}` exactly, in that order
    /// (`key_update`, X-Wing key, devices).
    pub(crate) fn key_update(&mut self, key: u64) -> Result<crate::inner::KeyUpdate, ProtoError> {
        self.key(key)?;
        let entries = self.d.map().ok().flatten().unwrap_or(0);
        if !(2..=6).contains(&entries) || self.d.u64().ok() != Some(0) {
            return Err(SchemaError::WrongType { key }.into());
        }
        let pk = self
            .d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key })?;
        if self.d.u64().ok() != Some(1) {
            return Err(SchemaError::WrongType { key }.into());
        }
        let serial = self.d.u32().map_err(|_| SchemaError::WrongType { key })?;
        let mut k = crate::inner::KeyUpdate {
            mailbox_pk: pk,
            serial,
            ..Default::default()
        };
        // Optional keys 2..=5, each at most once, ascending.
        let mut last = 1;
        for _ in 2..entries {
            let sub = self
                .d
                .u64()
                .ok()
                .filter(|s| *s > last && *s <= 5)
                .ok_or(SchemaError::WrongType { key })?;
            last = sub;
            match sub {
                2 => {
                    let b = self.d.bytes().map_err(|_| SchemaError::WrongType { key })?;
                    if b.len() != crate::inner::XWING_PK_LEN {
                        return Err(SchemaError::WrongLength { key }.into());
                    }
                    k.xwing_pk = Some(b.to_vec());
                }
                3 => {
                    let b = self.d.bytes().map_err(|_| SchemaError::WrongType { key })?;
                    k.device_id =
                        Some(<[u8; 8]>::try_from(b).map_err(|_| SchemaError::WrongLength { key })?);
                }
                4 => {
                    let b = self.d.bytes().map_err(|_| SchemaError::WrongType { key })?;
                    k.device_list = Some(crate::device::DeviceList::decode(b)?);
                }
                _ => {
                    let f = self.d.u32().map_err(|_| SchemaError::WrongType { key })?;
                    if f == 0 {
                        // Zero is written as absent (canonical).
                        return Err(SchemaError::UnknownValue { key }.into());
                    }
                    k.features = f;
                }
            }
        }
        Ok(k)
    }

    /// A definite-length array of 1..=`max` byte strings, each 1..=`max_len` bytes
    /// (bounded before allocating).
    pub(crate) fn bytes_list(
        &mut self,
        key: u64,
        max: usize,
        max_len: usize,
    ) -> Result<Vec<&'b [u8]>, SchemaError> {
        self.key(key)?;
        let n = match self.d.array() {
            Ok(Some(n)) => usize::try_from(n).map_err(|_| SchemaError::WrongLength { key })?,
            _ => return Err(SchemaError::WrongType { key }),
        };
        if n == 0 || n > max {
            return Err(SchemaError::WrongLength { key });
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let b = self.d.bytes().map_err(|_| SchemaError::WrongType { key })?;
            if b.is_empty() || b.len() > max_len {
                return Err(SchemaError::WrongLength { key });
            }
            out.push(b);
        }
        Ok(out)
    }

    /// Inner field 5: the attachment references ([`crate::attachment`]).
    pub(crate) fn attachments(
        &mut self,
        key: u64,
    ) -> Result<Vec<crate::attachment::AttachmentRef>, SchemaError> {
        self.key(key)?;
        crate::attachment::read_list(&mut self.d, key)
    }

    pub(crate) fn text(&mut self, key: u64) -> Result<&'b str, SchemaError> {
        self.key(key)?;
        self.d.str().map_err(|_| SchemaError::WrongType { key })
    }

    /// Fails unless every declared entry was read.
    pub(crate) fn finish(self) -> Result<(), SchemaError> {
        if self.read == self.entries {
            Ok(())
        } else {
            Err(SchemaError::MapShape)
        }
    }
}

/// Reads a definite-length array of 1..=`max` 16-byte byte strings (bounded before allocating).
pub(crate) fn read_id_list(
    d: &mut Decoder<'_>,
    key: u64,
    max: usize,
) -> Result<Vec<[u8; 16]>, SchemaError> {
    let n = match d.array() {
        Ok(Some(n)) => usize::try_from(n).map_err(|_| SchemaError::WrongLength { key })?,
        _ => return Err(SchemaError::WrongType { key }),
    };
    if n == 0 || n > max {
        return Err(SchemaError::WrongLength { key });
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let b = d.bytes().map_err(|_| SchemaError::WrongType { key })?;
        out.push(<[u8; 16]>::try_from(b).map_err(|_| SchemaError::WrongLength { key })?);
    }
    Ok(out)
}

/// Reads one item header as `(major, additional_info, argument)`, enforcing shortest-form
/// arguments. Advances `pos` past the header.
fn read_head(input: &[u8], pos: &mut usize) -> Result<(u8, u8, u64), CborError> {
    let initial = *input.get(*pos).ok_or(CborError::Truncated)?;
    *pos += 1;
    let major = initial >> 5;
    let info = initial & 0x1f;

    let (width, min) = match info {
        0..=23 => return Ok((major, info, u64::from(info))),
        24 => (1, 24),
        25 => (2, 1 << 8),
        26 => (4, 1 << 16),
        27 => (8, 1 << 32),
        28..=30 => return Err(CborError::Reserved),
        _ => return Err(CborError::IndefiniteLength),
    };
    let end = pos.checked_add(width).ok_or(CborError::Truncated)?;
    let bytes = input.get(*pos..end).ok_or(CborError::Truncated)?;
    *pos = end;
    let arg = bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));

    // Simple values 24..=31 use a one-byte argument; floats use 2/4/8 bytes and are rejected
    // by the caller, so the shortest-form rule only applies to the other major types here.
    if major != MAJOR_SIMPLE && arg < min {
        return Err(CborError::NonMinimal);
    }
    Ok((major, info, arg))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: DecodeLimits = DecodeLimits {
        max_input_len: 1024,
        max_container_len: 16,
    };

    fn check(hex_input: &str) -> Result<(), CborError> {
        scan(&hex::decode(hex_input).unwrap(), &LIMITS)
    }

    #[test]
    fn accepts_canonical_items() {
        for ok in [
            "00",
            "17",
            "1818",
            "190100",
            "1a00010000",
            "1b0000000100000000",
            "20",
            "40",
            "4401020304",
            "6161",
            "80",
            "a0",
            "a201020304",
            "f4",
            "f5",
            "f6",
        ] {
            assert_eq!(check(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn rejects_non_minimal() {
        for bad in [
            "1817",
            "1900ff",
            "1a0000ffff",
            "1b00000000ffffffff",
            "5800",
            "9800",
        ] {
            assert_eq!(check(bad), Err(CborError::NonMinimal), "{bad}");
        }
    }

    #[test]
    fn rejects_indefinite_and_break() {
        for bad in ["5f40ff", "9fff", "bfff", "ff"] {
            assert_eq!(check(bad), Err(CborError::IndefiniteLength), "{bad}");
        }
    }

    #[test]
    fn rejects_forbidden_types() {
        assert_eq!(check("c000"), Err(CborError::TagNotAllowed));
        assert_eq!(check("f7"), Err(CborError::SimpleNotAllowed)); // undefined
        assert_eq!(check("f93c00"), Err(CborError::SimpleNotAllowed)); // f16 1.0
        assert_eq!(check("f820"), Err(CborError::SimpleNotAllowed)); // simple(32)
        assert_eq!(check("f814"), Err(CborError::SimpleNotAllowed)); // two-byte `false`
        assert_eq!(check("1c"), Err(CborError::Reserved));
    }

    #[test]
    fn rejects_truncation_and_trailing() {
        assert_eq!(check(""), Err(CborError::Truncated));
        assert_eq!(check("18"), Err(CborError::Truncated));
        assert_eq!(check("4301"), Err(CborError::Truncated));
        assert_eq!(check("8201"), Err(CborError::Truncated));
        assert_eq!(check("a101"), Err(CborError::Truncated));
        assert_eq!(check("0000"), Err(CborError::TrailingBytes));
        // Declared length far beyond the input must fail without allocating.
        assert_eq!(check("5bffffffffffffffff"), Err(CborError::Truncated));
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert_eq!(check("61ff"), Err(CborError::InvalidUtf8));
    }

    #[test]
    fn enforces_depth_limit() {
        // MAX_DEPTH nested single-element arrays around a 0 are fine; one more is not.
        let ok = "81".repeat(MAX_DEPTH) + "00";
        assert_eq!(check(&ok), Ok(()));
        let bad = "81".repeat(MAX_DEPTH + 1) + "00";
        assert_eq!(
            check(&bad),
            Err(CborError::DepthExceeded { max: MAX_DEPTH })
        );
        // Empty containers do not open a level.
        let empty = "81".repeat(MAX_DEPTH) + "80";
        assert_eq!(check(&empty), Ok(()));
    }

    #[test]
    fn enforces_container_limit() {
        assert_eq!(check("90"), Err(CborError::Truncated)); // 16 entries declared, none present
        assert_eq!(
            check("910000000000000000000000000000000000"),
            Err(CborError::ContainerTooLong { max: 16 })
        );
    }

    #[test]
    fn closes_nested_containers() {
        // [[1, [2]], {3: [4, 5]}, 6]
        assert_eq!(check("8382018102a10382040506"), Ok(()));
    }
}
