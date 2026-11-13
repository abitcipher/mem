//! Editing sent messages (`spec/inner.cddl`): an edit of an own text (content type
//! [`CONTENT_TYPE_EDIT`]) and the edit limit a recipient announces to a contact (content type
//! [`CONTENT_TYPE_EDIT_LIMIT`]).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};
use crate::inner::MAX_TEXT_LEN;

/// Content type: the new text of an own earlier message.
pub const CONTENT_TYPE_EDIT: u64 = 11;
/// Content type: how late the sender accepts edits from this contact.
pub const CONTENT_TYPE_EDIT_LIMIT: u64 = 12;
/// Content type: the sender withdraws an earlier text (deleting counts as editing: the same
/// limit applies).
pub const CONTENT_TYPE_DELETE: u64 = 13;
/// Positions of the edit-limit scale: 0 always, 1..=11 a duration, 12 never.
pub const EDIT_LIMIT_POSITIONS: u8 = 13;

/// The new text of an earlier message.
#[derive(Clone, PartialEq, Eq)]
pub struct Edit {
    /// The message edited (sent by the same sender to the same recipient).
    pub message_id: [u8; 16],
    /// Its whole new text, 1..=[`MAX_TEXT_LEN`] bytes.
    pub text: String,
}

impl fmt::Debug for Edit {
    /// Never prints the text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edit")
            .field("len", &self.text.len())
            .finish_non_exhaustive()
    }
}

impl Edit {
    /// Canonical CBOR `{0: message_id, 1: text}`.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for an empty or too long text.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        if !(1..=MAX_TEXT_LEN).contains(&self.text.len()) {
            return Err(SchemaError::WrongLength { key: 1 }.into());
        }
        Ok(encode_with(self.text.len() + 32, |e| {
            e.map(2)?
                .u8(0)?
                .bytes(&self.message_id)?
                .u8(1)?
                .str(&self.text)?;
            Ok(())
        }))
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_TEXT_LEN + 32,
            max_container_len: 2,
        };
        let mut r = MapReader::new(input, &limits)?;
        let message_id = r.array::<16>(0)?;
        let text = String::from(r.text(1)?);
        r.finish()?;
        if !(1..=MAX_TEXT_LEN).contains(&text.len()) {
            return Err(SchemaError::WrongLength { key: 1 }.into());
        }
        Ok(Self { message_id, text })
    }
}

/// Encodes a deletion `{0: message_id}`.
#[must_use]
pub fn encode_delete(message_id: &[u8; 16]) -> Vec<u8> {
    encode_with(20, |e| {
        e.map(1)?.u8(0)?.bytes(message_id)?;
        Ok(())
    })
}

/// Decodes an untrusted deletion.
///
/// # Errors
///
/// [`ProtoError`] for malformed input.
pub fn decode_delete(input: &[u8]) -> Result<[u8; 16], ProtoError> {
    let limits = DecodeLimits {
        max_input_len: 24,
        max_container_len: 1,
    };
    let mut r = MapReader::new(input, &limits)?;
    let id = r.array::<16>(0)?;
    r.finish()?;
    Ok(id)
}

/// Encodes an edit limit `{0: position}`.
///
/// # Errors
///
/// [`SchemaError::UnknownValue`] for a position outside the scale.
pub fn encode_edit_limit(position: u8) -> Result<Vec<u8>, ProtoError> {
    if position >= EDIT_LIMIT_POSITIONS {
        return Err(SchemaError::UnknownValue { key: 0 }.into());
    }
    Ok(encode_with(4, |e| {
        e.map(1)?.u8(0)?.u8(position)?;
        Ok(())
    }))
}

/// Decodes an untrusted edit limit.
///
/// # Errors
///
/// [`ProtoError`] for malformed input or a position outside the scale.
pub fn decode_edit_limit(input: &[u8]) -> Result<u8, ProtoError> {
    let limits = DecodeLimits {
        max_input_len: 8,
        max_container_len: 1,
    };
    let mut r = MapReader::new(input, &limits)?;
    let position = r.u8(0)?;
    r.finish()?;
    if position >= EDIT_LIMIT_POSITIONS {
        return Err(SchemaError::UnknownValue { key: 0 }.into());
    }
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_and_limits_round_trip_within_bounds() {
        let e = Edit {
            message_id: [3; 16],
            text: "fixed a typo".into(),
        };
        assert_eq!(Edit::decode(&e.encode().unwrap()).unwrap(), e);
        let empty = Edit {
            message_id: [3; 16],
            text: String::new(),
        };
        assert!(empty.encode().is_err());
        for p in 0..EDIT_LIMIT_POSITIONS {
            assert_eq!(
                decode_edit_limit(&encode_edit_limit(p).unwrap()).unwrap(),
                p
            );
        }
        assert!(encode_edit_limit(EDIT_LIMIT_POSITIONS).is_err());
        assert!(decode_edit_limit(&[0xa1, 0x00, 0x0d]).is_err());
        assert_eq!(decode_delete(&encode_delete(&[9; 16])).unwrap(), [9; 16]);
    }
}
