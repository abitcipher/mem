//! Inner payload, v1 (`spec/inner.cddl`).
//!
//! Only ever exists in plaintext after HPKE decryption, or before sealing. Attachments (field 5)
//! and key updates (field 6) are not supported yet; inputs carrying them are rejected. Field 10
//! carries piggybacked delivery acks, and content type 2 is a standalone ack.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Encoder;
use minicbor::encode::Error as EncodeError;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{InvalidBundle, ProtoError, SchemaError};

/// Domain separation label for the inner signature.
pub const SIGNATURE_DOMAIN: &[u8] = b"MEM/v1/inner-sig";
/// Padding buckets in bytes (v0.1 stops at 16 KiB).
pub const PAD_BUCKETS: [usize; 5] = [256, 512, 1024, 4096, 16_384];
/// Content type: UTF-8 text.
pub const CONTENT_TYPE_TEXT: u64 = 1;
/// Maximum UTF-8 text length in bytes in v0.1.
pub const MAX_TEXT_LEN: usize = 4000;
/// Content type: delivery ack, content is [`encode_ack_ids`].
pub const CONTENT_TYPE_ACK: u64 = 2;
/// Content type: standalone key update, empty content; the key is in field 6.
pub const CONTENT_TYPE_KEY_UPDATE: u64 = 3;
/// Content type 4: a contact request in an intro bundle.
pub const CONTENT_TYPE_INTRO: u64 = 4;
/// Longest note in a contact request, in bytes.
pub const MAX_INTRO_NOTE_LEN: usize = 280;
/// Longest card inside a contact request (cards are at most about 220 bytes).
const MAX_INTRO_CARD_LEN: usize = 512;
/// Most message ids in one ack, piggybacked (field 10) or standalone.
pub const MAX_ACK_IDS: usize = 64;

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: 16_384,
    // The map has at most 10 entries; field 10 holds up to 64 ids.
    max_container_len: MAX_ACK_IDS as u64,
};

/// Signed inner payload fields: keys 0..=7, 9 and 10 (everything except the signature, key 8).
#[derive(Clone, PartialEq, Eq)]
pub struct InnerFields {
    /// Sender identity public key (Ed25519).
    pub sender: [u8; 32],
    /// UUIDv7 message id (timestamp is fine inside E2E).
    pub message_id: [u8; 16],
    /// Conversation id.
    pub conversation_id: [u8; 16],
    /// Content type.
    pub content_type: u64,
    /// Content, CBOR per content type.
    pub content: Vec<u8>,
    /// Whether the sender asks for a delivery ack.
    pub ack_requested: Option<bool>,
    /// Purge token, released by the receiver after delivery (key 9).
    pub purge_token: [u8; 16],
    /// Piggybacked acks: message ids received from the recipient (key 10). Empty means
    /// the field is absent; at most [`MAX_ACK_IDS`].
    pub acks: Vec<[u8; 16]>,
    /// The sender's current mailbox key and its serial (key 6), until the recipient
    /// has confirmed a bundle that carried it.
    pub key_update: Option<KeyUpdate>,
    /// The signing device's certificate (key 11), present when a secondary
    /// device signed (field 8 is then by that device key).
    pub device_cert: Option<crate::device::DeviceCert>,
}

/// A mailbox key announcement; authenticated by the inner signature.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyUpdate {
    /// X25519 mailbox public key.
    pub mailbox_pk: [u8; 32],
    /// Increasing serial; receivers keep only the highest.
    pub serial: u32,
    /// The same mailbox key's X-Wing public half, 1 216 bytes (key 2).
    pub xwing_pk: Option<Vec<u8>>,
    /// Whose mailbox key this is (key 3); `None` for the identity's only
    /// device.
    pub device_id: Option<[u8; 8]>,
    /// The newest device list the sender knows (key 4).
    pub device_list: Option<crate::device::DeviceList>,
    /// Feature bits (key 5), absent when zero; bit 0 is [`crate::device::FEATURE_DEVICES`].
    pub features: u32,
}

/// Length of an X-Wing public key in a key update.
pub const XWING_PK_LEN: usize = 1216;

impl fmt::Debug for InnerFields {
    /// Never prints content.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerFields")
            .field("content_type", &self.content_type)
            .field("content_len", &self.content.len())
            .finish_non_exhaustive()
    }
}

/// A complete inner payload: fields plus the sender's signature (field 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Inner {
    /// Signed fields.
    pub fields: InnerFields,
    /// Ed25519 signature over `SIGNATURE_DOMAIN || recipient_pk || signed_bytes()`.
    pub signature: [u8; 64],
}

impl InnerFields {
    /// Canonical CBOR of the inner map without key 8: the message part of the signature.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        encode_with(self.content.len() + 112, |e| {
            e.map(self.entries())?;
            self.write_head(e)?;
            e.u8(9)?.bytes(&self.purge_token)?;
            self.write_tail(e)?;
            Ok(())
        })
    }

    /// Number of map entries excluding the signature.
    fn entries(&self) -> u64 {
        6 + u64::from(self.ack_requested.is_some())
            + u64::from(!self.acks.is_empty())
            + u64::from(self.key_update.is_some())
            + u64::from(self.device_cert.is_some())
    }

    /// Writes keys 10 and 11 (after the purge token in canonical order), if present.
    fn write_tail(
        &self,
        e: &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), EncodeError<core::convert::Infallible>> {
        if !self.acks.is_empty() {
            e.u8(10)?.array(self.acks.len() as u64)?;
            for id in &self.acks {
                e.bytes(id)?;
            }
        }
        if let Some(c) = &self.device_cert {
            e.u8(11)?.bytes(&c.encode())?;
        }
        Ok(())
    }

    /// Writes keys 0..=7 (everything that precedes the signature in canonical order).
    fn write_head(
        &self,
        e: &mut Encoder<&mut Vec<u8>>,
    ) -> Result<(), EncodeError<core::convert::Infallible>> {
        e.u8(0)?
            .bytes(&self.sender)?
            .u8(1)?
            .bytes(&self.message_id)?
            .u8(2)?
            .bytes(&self.conversation_id)?
            .u8(3)?
            .u64(self.content_type)?
            .u8(4)?
            .bytes(&self.content)?;
        if let Some(k) = &self.key_update {
            let entries = 2
                + u64::from(k.xwing_pk.is_some())
                + u64::from(k.device_id.is_some())
                + u64::from(k.device_list.is_some())
                + u64::from(k.features != 0);
            e.u8(6)?
                .map(entries)?
                .u8(0)?
                .bytes(&k.mailbox_pk)?
                .u8(1)?
                .u32(k.serial)?;
            if let Some(x) = &k.xwing_pk {
                e.u8(2)?.bytes(x)?;
            }
            if let Some(id) = &k.device_id {
                e.u8(3)?.bytes(id)?;
            }
            if let Some(l) = &k.device_list {
                // Checked when the list was built or decoded.
                e.u8(4)?.bytes(&l.encode().unwrap_or_default())?;
            }
            if k.features != 0 {
                e.u8(5)?.u32(k.features)?;
            }
        }
        if let Some(ack) = self.ack_requested {
            e.u8(7)?.bool(ack)?;
        }
        Ok(())
    }
}

impl Inner {
    /// Canonical CBOR of the whole inner map including the signature.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(self.fields.content.len() + 160, |e| {
            e.map(self.fields.entries() + 1)?;
            self.fields.write_head(e)?;
            e.u8(8)?
                .bytes(&self.signature)?
                .u8(9)?
                .bytes(&self.fields.purge_token)?;
            self.fields.write_tail(e)?;
            Ok(())
        })
    }

    /// Decodes an untrusted inner payload (after unpadding).
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or unsupported input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let mut r = MapReader::new(input, &LIMITS)?;
        let sender = r.array::<32>(0)?;
        let message_id = r.array::<16>(1)?;
        let conversation_id = r.array::<16>(2)?;
        let content_type = r.u64(3)?;
        let content = r.bytes(4)?.to_vec();
        let key_update = if r.has(6) {
            Some(r.key_update(6)?)
        } else {
            None
        };
        let ack_requested = if r.has(7) { Some(r.bool(7)?) } else { None };
        let signature = r.array::<64>(8)?;
        let purge_token = r.array::<16>(9)?;
        let acks = if r.has(10) {
            r.id_list(10, MAX_ACK_IDS)?
        } else {
            Vec::new()
        };
        let device_cert = if r.has(11) {
            Some(crate::device::DeviceCert::decode(r.bytes(11)?)?)
        } else {
            None
        };
        r.finish()?;
        Ok(Self {
            fields: InnerFields {
                sender,
                message_id,
                conversation_id,
                content_type,
                content,
                ack_requested,
                purge_token,
                acks,
                key_update,
                device_cert,
            },
            signature,
        })
    }
}

/// Encodes UTF-8 text as content for [`CONTENT_TYPE_TEXT`].
///
/// # Errors
///
/// [`InvalidBundle::TooLarge`] if the text exceeds [`MAX_TEXT_LEN`] bytes.
pub fn encode_text(text: &str) -> Result<Vec<u8>, ProtoError> {
    if text.len() > MAX_TEXT_LEN {
        return Err(InvalidBundle::TooLarge {
            len: text.len(),
            max: MAX_TEXT_LEN,
        }
        .into());
    }
    Ok(encode_with(text.len() + 4, |e| {
        e.str(text)?;
        Ok(())
    }))
}

/// Decodes [`CONTENT_TYPE_TEXT`] content.
///
/// # Errors
///
/// [`ProtoError`] if the content is not exactly one canonical text string within limits.
pub fn decode_text(content: &[u8]) -> Result<String, ProtoError> {
    let limits = DecodeLimits {
        max_input_len: MAX_TEXT_LEN + 3,
        max_container_len: 0,
    };
    if content.len() > limits.max_input_len {
        return Err(ProtoError::InputTooLarge {
            len: content.len(),
            max: limits.max_input_len,
        });
    }
    crate::cbor::scan(content, &limits)?;
    let mut d = minicbor::Decoder::new(content);
    let text = d.str().map_err(|_| SchemaError::WrongType { key: 4 })?;
    Ok(String::from(text))
}

/// Encodes standalone-ack content for [`CONTENT_TYPE_ACK`]: an array of 1..=[`MAX_ACK_IDS`]
/// message ids.
///
/// # Errors
///
/// [`SchemaError::WrongLength`] for an empty or oversized list.
pub fn encode_ack_ids(ids: &[[u8; 16]]) -> Result<Vec<u8>, ProtoError> {
    if ids.is_empty() || ids.len() > MAX_ACK_IDS {
        return Err(SchemaError::WrongLength { key: 4 }.into());
    }
    Ok(encode_with(ids.len() * 17 + 3, |e| {
        e.array(ids.len() as u64)?;
        for id in ids {
            e.bytes(id)?;
        }
        Ok(())
    }))
}

/// Decodes [`CONTENT_TYPE_ACK`] content (untrusted).
///
/// # Errors
///
/// [`ProtoError`] unless the content is exactly one canonical array of 1..=[`MAX_ACK_IDS`]
/// 16-byte strings.
pub fn decode_ack_ids(content: &[u8]) -> Result<Vec<[u8; 16]>, ProtoError> {
    let limits = DecodeLimits {
        max_input_len: MAX_ACK_IDS * 17 + 3,
        max_container_len: MAX_ACK_IDS as u64,
    };
    if content.len() > limits.max_input_len {
        return Err(ProtoError::InputTooLarge {
            len: content.len(),
            max: limits.max_input_len,
        });
    }
    crate::cbor::scan(content, &limits)?;
    let mut d = minicbor::Decoder::new(content);
    Ok(crate::cbor::read_id_list(&mut d, 4, MAX_ACK_IDS)?)
}

/// Content of a contact request (content type 4).
#[derive(Clone, PartialEq, Eq)]
pub struct IntroContent {
    /// The sender's signed card (`spec/card.cddl`).
    pub card: Vec<u8>,
    /// The pairing nonce of the recipient's card that the sender used.
    pub recipient_nonce: [u8; 16],
    /// A short note shown with the request, 1..=[`MAX_INTRO_NOTE_LEN`] bytes.
    pub note: Option<String>,
}

impl fmt::Debug for IntroContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntroContent")
            .field("card_len", &self.card.len())
            .field("note_len", &self.note.as_ref().map(String::len))
            .finish_non_exhaustive()
    }
}

impl IntroContent {
    fn check(&self) -> Result<(), SchemaError> {
        if self.card.is_empty() || self.card.len() > MAX_INTRO_CARD_LEN {
            return Err(SchemaError::WrongLength { key: 0 });
        }
        match &self.note {
            Some(n) if n.is_empty() || n.len() > MAX_INTRO_NOTE_LEN => {
                Err(SchemaError::WrongLength { key: 2 })
            }
            _ => Ok(()),
        }
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for an empty or oversized card or note.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        let entries = 2 + u64::from(self.note.is_some());
        Ok(encode_with(self.card.len() + 320, |e| {
            e.map(entries)?
                .u8(0)?
                .bytes(&self.card)?
                .u8(1)?
                .bytes(&self.recipient_nonce)?;
            if let Some(n) = &self.note {
                e.u8(2)?.str(n)?;
            }
            Ok(())
        }))
    }

    /// Decodes untrusted content. The card inside is decoded and verified separately.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_INTRO_CARD_LEN + MAX_INTRO_NOTE_LEN + 32,
            max_container_len: 3,
        };
        let mut r = MapReader::new(input, &limits)?;
        let card = r.bytes(0)?.to_vec();
        let recipient_nonce = r.array::<16>(1)?;
        let note = if r.has(2) {
            Some(String::from(r.text(2)?))
        } else {
            None
        };
        r.finish()?;
        let c = Self {
            card,
            recipient_nonce,
            note,
        };
        c.check()?;
        Ok(c)
    }
}

/// ISO/IEC 7816-4 padding to the next bucket.
///
/// # Errors
///
/// [`InvalidBundle::TooLarge`] if `data.len() + 1` exceeds the largest bucket.
pub fn pad(data: &[u8]) -> Result<Vec<u8>, ProtoError> {
    let needed = data.len() + 1;
    let Some(&bucket) = PAD_BUCKETS.iter().find(|b| **b >= needed) else {
        return Err(InvalidBundle::TooLarge {
            len: needed,
            max: PAD_BUCKETS[PAD_BUCKETS.len() - 1],
        }
        .into());
    };
    let mut out = Vec::with_capacity(bucket);
    out.extend_from_slice(data);
    out.push(0x80);
    out.resize(bucket, 0);
    Ok(out)
}

/// Removes ISO/IEC 7816-4 padding.
///
/// # Errors
///
/// [`SchemaError::MapShape`] if the length is not a bucket size or the padding is malformed.
pub fn unpad(padded: &[u8]) -> Result<&[u8], ProtoError> {
    if !PAD_BUCKETS.contains(&padded.len()) {
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

    fn inner(ack: Option<bool>) -> Inner {
        Inner {
            fields: InnerFields {
                sender: [1; 32],
                message_id: [2; 16],
                conversation_id: [3; 16],
                content_type: CONTENT_TYPE_TEXT,
                content: encode_text("привіт").unwrap(),
                ack_requested: ack,
                purge_token: [5; 16],
                acks: Vec::new(),
                key_update: None,
                device_cert: None,
            },
            signature: [4; 64],
        }
    }

    /// A secondary device's certificate (key 11) and a key update naming the
    /// device, carrying the device list and the feature bit.
    #[test]
    fn device_fields_roundtrip_signed_and_canonical() {
        use crate::device::{DeviceCert, DeviceList, FEATURE_DEVICES};
        let cert = |d: u8| DeviceCert {
            identity_pk: [1; 32],
            device_pk: [d; 32],
            signature: [9; 64],
        };
        let mut i = inner(Some(true));
        i.fields.acks = alloc::vec![[6; 16]];
        i.fields.device_cert = Some(cert(11));
        i.fields.key_update = Some(KeyUpdate {
            mailbox_pk: [7; 32],
            serial: 3,
            xwing_pk: Some(alloc::vec![8; XWING_PK_LEN]),
            device_id: Some([12; 8]),
            device_list: Some(DeviceList {
                identity_pk: [1; 32],
                serial: 2,
                certs: alloc::vec![cert(10), cert(11)],
                signature: [13; 64],
            }),
            features: FEATURE_DEVICES,
        });
        let bytes = i.encode();
        assert_eq!(Inner::decode(&bytes).unwrap(), i);
        // The signature covers the certificate.
        let mut other = i.fields.clone();
        other.device_cert = Some(cert(14));
        assert_ne!(other.signed_bytes(), i.fields.signed_bytes());
        // Each optional key-update field roundtrips on its own.
        for k in [
            KeyUpdate {
                device_id: Some([1; 8]),
                ..KeyUpdate::default()
            },
            KeyUpdate {
                features: FEATURE_DEVICES,
                ..KeyUpdate::default()
            },
        ] {
            let mut j = inner(None);
            j.fields.key_update = Some(k);
            assert_eq!(Inner::decode(&j.encode()).unwrap(), j);
        }
        // Features 0 must be absent, not written.
        let mut j = inner(None);
        j.fields.key_update = Some(KeyUpdate {
            features: 1,
            ..KeyUpdate::default()
        });
        let mut b = j.encode();
        let at = b.windows(2).position(|w| w == [0x05, 0x01]).unwrap();
        b[at + 1] = 0x00;
        assert!(Inner::decode(&b).is_err());
    }

    #[test]
    fn key_update_roundtrips_in_canonical_order_and_is_signed() {
        let mut i = inner(Some(true));
        i.fields.key_update = Some(KeyUpdate {
            mailbox_pk: [7; 32],
            serial: 3,
            xwing_pk: None,
            ..KeyUpdate::default()
        });
        let bytes = i.encode();
        assert_eq!(Inner::decode(&bytes).unwrap(), i);
        // Field 6 sits between 4 and 7, and the signature covers it.
        assert!(
            i.fields
                .signed_bytes()
                .windows(4)
                .any(|w| w == [0x06, 0xa2, 0x00, 0x58])
        );
        // A wrong shape is refused: the serial as a one-byte byte string instead of a uint.
        let mut broken = bytes.clone();
        let at = broken.windows(2).position(|w| w == [0x01, 0x03]).unwrap();
        broken[at + 1] = 0x41;
        broken.insert(at + 2, 0x03);
        assert!(Inner::decode(&broken).is_err());
    }

    /// A key update may carry the X-Wing public half (exactly 1 216 bytes), covered by
    /// the signature.
    #[test]
    fn key_update_carries_the_xwing_key() {
        let mut i = inner(None);
        i.fields.key_update = Some(KeyUpdate {
            mailbox_pk: [7; 32],
            serial: 4,
            xwing_pk: Some(alloc::vec![9; XWING_PK_LEN]),
            ..KeyUpdate::default()
        });
        let bytes = i.encode();
        assert_eq!(Inner::decode(&bytes).unwrap(), i);
        assert!(
            i.fields
                .signed_bytes()
                .windows(3)
                .any(|w| w == [0x06, 0xa3, 0x00])
        );
        // Any other length is refused.
        let mut short = i.clone();
        if let Some(k) = short.fields.key_update.as_mut() {
            k.xwing_pk = Some(alloc::vec![9; 32]);
        }
        assert!(Inner::decode(&short.encode()).is_err());
    }

    #[test]
    fn roundtrip() {
        for ack in [None, Some(true), Some(false)] {
            let i = inner(ack);
            assert_eq!(Inner::decode(&i.encode()).unwrap(), i);
        }
        assert_eq!(decode_text(&inner(None).fields.content).unwrap(), "привіт");
    }

    #[test]
    fn signed_bytes_are_the_map_without_signature() {
        let i = inner(Some(true));
        let full = i.encode();
        let signed = i.fields.signed_bytes();
        // Remove the key-8 entry (key byte, 2-byte bstr header, 64-byte signature) from the full
        // encoding and fix the map header: that must be exactly the signed bytes.
        let sig_entry = [&[0x08, 0x58, 0x40][..], &[4; 64][..]].concat();
        let at = full
            .windows(sig_entry.len())
            .position(|w| w == sig_entry.as_slice())
            .unwrap();
        let mut expected = full.clone();
        expected.drain(at..at + sig_entry.len());
        expected[0] -= 1;
        assert_eq!(signed, expected);
        assert_eq!(full[0], 0xa8);
        // The purge token is covered by the signature.
        let mut other = i.fields.clone();
        other.purge_token[0] ^= 1;
        assert_ne!(other.signed_bytes(), signed);
    }

    #[test]
    fn rejects_unknown_and_unsupported_fields() {
        let mut bytes = inner(None).encode();
        // Add one entry to the map header and append key 12 => 0.
        bytes[0] += 1;
        bytes.extend_from_slice(&[0x0c, 0x00]);
        assert_eq!(Inner::decode(&bytes), Err(SchemaError::MapShape.into()));
        // Key 11 must be a device certificate, not a number.
        let mut bad = inner(None).encode();
        bad[0] += 1;
        bad.extend_from_slice(&[0x0b, 0x00]);
        assert!(Inner::decode(&bad).is_err());
        // Key 10 must be a list of ids, not a number.
        let mut bad = inner(None).encode();
        bad[0] += 1;
        bad.extend_from_slice(&[0x0a, 0x00]);
        assert!(Inner::decode(&bad).is_err());
    }

    #[test]
    fn piggybacked_acks_roundtrip_signed_and_bounded() {
        let mut i = inner(Some(true));
        i.fields.acks = vec![[7; 16], [8; 16]];
        let bytes = i.encode();
        assert_eq!(Inner::decode(&bytes).unwrap(), i);
        // Covered by the signature.
        let mut other = i.fields.clone();
        other.acks[1][0] ^= 1;
        assert_ne!(other.signed_bytes(), i.fields.signed_bytes());
        // The most ids allowed roundtrip; one more is refused.
        i.fields.acks = vec![[9; 16]; MAX_ACK_IDS];
        assert_eq!(Inner::decode(&i.encode()).unwrap(), i);
        i.fields.acks = vec![[9; 16]; MAX_ACK_IDS + 1];
        assert!(Inner::decode(&i.encode()).is_err());
        // An empty list on the wire is refused (absent is the only "no acks").
        let mut empty = inner(None).encode();
        empty[0] += 1;
        empty.extend_from_slice(&[0x0a, 0x80]);
        assert!(Inner::decode(&empty).is_err());
    }

    #[test]
    fn standalone_ack_content() {
        let ids = vec![[1; 16], [2; 16]];
        assert_eq!(decode_ack_ids(&encode_ack_ids(&ids).unwrap()).unwrap(), ids);
        assert!(encode_ack_ids(&[]).is_err());
        assert!(encode_ack_ids(&vec![[0; 16]; MAX_ACK_IDS + 1]).is_err());
        // Wrong id length, trailing bytes, and non-array content are refused.
        assert!(decode_ack_ids(&[0x81, 0x41, 0x00]).is_err());
        let mut trailing = encode_ack_ids(&ids).unwrap();
        trailing.push(0);
        assert!(decode_ack_ids(&trailing).is_err());
        assert!(decode_ack_ids(&encode_text("x").unwrap()).is_err());
    }

    #[test]
    fn padding_roundtrip_and_buckets() {
        for len in [0usize, 1, 254, 255, 256, 511, 4095, 16_383] {
            let data = vec![0xAB; len];
            let padded = pad(&data).unwrap();
            assert!(PAD_BUCKETS.contains(&padded.len()), "{len}");
            assert_eq!(unpad(&padded).unwrap(), &data[..]);
        }
        assert!(pad(&vec![0; 16_384]).is_err());
    }

    #[test]
    fn unpad_rejects_malformed() {
        assert!(unpad(&[0u8; 256]).is_err());
        let mut p = pad(b"x").unwrap();
        p[1] = 0x81;
        assert!(unpad(&p).is_err());
        assert!(unpad(&[0x80; 100]).is_err());
    }

    #[test]
    fn text_limits() {
        assert!(encode_text(&"a".repeat(MAX_TEXT_LEN)).is_ok());
        assert!(encode_text(&"a".repeat(MAX_TEXT_LEN + 1)).is_err());
    }

    #[test]
    fn intro_content_roundtrips_and_bounds_the_note() {
        let c = IntroContent {
            card: vec![0xa5; 200],
            recipient_nonce: [7; 16],
            note: Some(String::from("hi, it's Ann from the market")),
        };
        assert_eq!(IntroContent::decode(&c.encode().unwrap()).unwrap(), c);
        let bare = IntroContent {
            note: None,
            ..c.clone()
        };
        assert_eq!(IntroContent::decode(&bare.encode().unwrap()).unwrap(), bare);
        let long = IntroContent {
            note: Some("x".repeat(MAX_INTRO_NOTE_LEN + 1)),
            ..c
        };
        assert!(long.encode().is_err());
    }
}
