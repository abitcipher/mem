//! Pairing card.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Domain separation label for the card signature.
pub const CARD_SIGNATURE_DOMAIN: &[u8] = b"MEM/v1/card";
/// Maximum display name length in UTF-8 bytes.
pub const MAX_DISPLAY_NAME_LEN: usize = 64;

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: 512,
    max_container_len: 6,
};

/// Unsigned card fields.
#[derive(Clone, PartialEq, Eq)]
pub struct CardFields {
    /// Ed25519 identity public key.
    pub identity_pk: [u8; 32],
    /// X25519 mailbox public key.
    pub mailbox_pk: [u8; 32],
    /// Pairing nonce.
    pub pairing_nonce: [u8; 16],
    /// Optional self-chosen display name (untrusted).
    pub display_name: Option<String>,
}

impl fmt::Debug for CardFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardFields").finish_non_exhaustive()
    }
}

/// A signed pairing card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PairingCard {
    /// Signed fields.
    pub fields: CardFields,
    /// Signature over `CARD_SIGNATURE_DOMAIN || signed_bytes()` by `identity_pk`.
    pub signature: [u8; 64],
}

impl CardFields {
    /// Canonical CBOR of the card without the signature.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the display name is empty or too long.
    pub fn signed_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(self.encode_inner(None))
    }

    fn check(&self) -> Result<(), SchemaError> {
        match &self.display_name {
            Some(n) if n.is_empty() || n.len() > MAX_DISPLAY_NAME_LEN => {
                Err(SchemaError::WrongLength { key: 4 })
            }
            _ => Ok(()),
        }
    }

    fn encode_inner(&self, signature: Option<&[u8; 64]>) -> Vec<u8> {
        let entries = 4 + u64::from(self.display_name.is_some()) + u64::from(signature.is_some());
        encode_with(256, |e| {
            e.map(entries)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&self.identity_pk)?
                .u8(2)?
                .bytes(&self.mailbox_pk)?
                .u8(3)?
                .bytes(&self.pairing_nonce)?;
            if let Some(name) = &self.display_name {
                e.u8(4)?.str(name)?;
            }
            if let Some(sig) = signature {
                e.u8(5)?.bytes(sig)?;
            }
            Ok(())
        })
    }
}

impl PairingCard {
    /// Canonical CBOR of the signed card.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the display name is empty or too long.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.fields.check()?;
        Ok(self.fields.encode_inner(Some(&self.signature)))
    }

    /// Decodes an untrusted card. The signature is *not* verified here; that is
    /// `mem-crypto`'s job and must happen before the card is used.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or unsupported input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let mut r = MapReader::new(input, &LIMITS)?;
        if r.u8(0)? != 1 {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let identity_pk = r.array::<32>(1)?;
        let mailbox_pk = r.array::<32>(2)?;
        let pairing_nonce = r.array::<16>(3)?;
        let display_name = if r.has(4) {
            Some(String::from(r.text(4)?))
        } else {
            None
        };
        let signature = r.array::<64>(5)?;
        r.finish()?;
        let fields = CardFields {
            identity_pk,
            mailbox_pk,
            pairing_nonce,
            display_name,
        };
        fields.check()?;
        Ok(Self { fields, signature })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: Option<&str>) -> PairingCard {
        PairingCard {
            fields: CardFields {
                identity_pk: [1; 32],
                mailbox_pk: [2; 32],
                pairing_nonce: [3; 16],
                display_name: name.map(String::from),
            },
            signature: [4; 64],
        }
    }

    #[test]
    fn roundtrip() {
        for n in [None, Some("Олена")] {
            let c = card(n);
            assert_eq!(PairingCard::decode(&c.encode().unwrap()).unwrap(), c);
        }
    }

    #[test]
    fn rejects_bad_names() {
        assert!(card(Some("")).encode().is_err());
        assert!(card(Some(&"x".repeat(65))).encode().is_err());
    }
}
