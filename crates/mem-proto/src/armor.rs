//! Text armor for bundles and pairing cards.
//!
//! Armor is how MEM data travels through channels that only carry text: the OS share sheet,
//! the clipboard, and QR codes.

use alloc::string::String;
use alloc::vec::Vec;

use data_encoding::BASE64URL_NOPAD;

use crate::envelope::MAX_BUNDLE_LEN;
use crate::error::{ProtoError, SchemaError};

/// Prefix of an armored bundle.
pub const BUNDLE_PREFIX: &str = "mem1:";
/// Prefix of an armored pairing card.
pub const CARD_PREFIX: &str = "memcard1:";

/// What an armored string contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Armored {
    /// CBOR of a bundle envelope.
    Bundle(Vec<u8>),
    /// CBOR of a pairing card.
    Card(Vec<u8>),
    /// CBOR of a device request (`memdevice1:`).
    DeviceRequest(Vec<u8>),
}

/// Armors bundle bytes.
#[must_use]
pub fn armor_bundle(bundle: &[u8]) -> String {
    armor(BUNDLE_PREFIX, bundle)
}

/// Armors pairing card bytes.
#[must_use]
pub fn armor_card(card: &[u8]) -> String {
    armor(CARD_PREFIX, card)
}

/// Armors a device request.
#[must_use]
pub fn armor_device_request(request: &[u8]) -> String {
    armor(crate::device::DEVICE_REQUEST_PREFIX, request)
}

fn armor(prefix: &str, data: &[u8]) -> String {
    let mut out = String::with_capacity(prefix.len() + data.len() * 4 / 3 + 4);
    out.push_str(prefix);
    out.push_str(&BASE64URL_NOPAD.encode(data));
    out
}

/// Parses armored text pasted or scanned by the user.
///
/// Leading/trailing whitespace is ignored, and so is any text before the first prefix (users
/// often paste a whole chat message). Everything after the prefix up to the next whitespace is
/// the payload.
///
/// # Errors
///
/// [`ProtoError::InputTooLarge`] for oversized input, [`SchemaError::MapShape`] if no prefix is
/// found or the base64 is invalid.
pub fn dearmor(text: &str) -> Result<Armored, ProtoError> {
    let max_chars = MAX_BUNDLE_LEN * 4 / 3 + 64;
    if text.len() > max_chars * 2 {
        return Err(ProtoError::InputTooLarge {
            len: text.len(),
            max: max_chars * 2,
        });
    }
    // Check the longer prefixes first: neither contains "mem1:", but be explicit.
    for (prefix, kind) in [
        (crate::device::DEVICE_REQUEST_PREFIX, 2),
        (CARD_PREFIX, 1),
        (BUNDLE_PREFIX, 0),
    ] {
        if let Some(start) = text.find(prefix) {
            let rest = text.get(start + prefix.len()..).unwrap_or("");
            let body = rest.split_whitespace().next().unwrap_or("");
            if body.len() > max_chars {
                return Err(ProtoError::InputTooLarge {
                    len: body.len(),
                    max: max_chars,
                });
            }
            let data = BASE64URL_NOPAD
                .decode(body.as_bytes())
                .map_err(|_| SchemaError::MapShape)?;
            return Ok(match kind {
                2 => Armored::DeviceRequest(data),
                1 => Armored::Card(data),
                _ => Armored::Bundle(data),
            });
        }
    }
    Err(SchemaError::MapShape.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn roundtrip_and_embedded_text() {
        let data = vec![0, 1, 2, 250, 251, 255];
        let s = armor_bundle(&data);
        assert!(s.starts_with("mem1:"));
        assert_eq!(dearmor(&s).unwrap(), Armored::Bundle(data.clone()));
        let pasted = alloc::format!("Hi! here it is:\n{s}\nbye");
        assert_eq!(dearmor(&pasted).unwrap(), Armored::Bundle(data.clone()));
        assert_eq!(dearmor(&armor_card(&data)).unwrap(), Armored::Card(data));
    }

    #[test]
    fn rejects_garbage() {
        assert!(dearmor("hello").is_err());
        assert!(dearmor("mem1:!!!").is_err());
        assert!(dearmor(&"a".repeat(1_000_000)).is_err());
    }
}
