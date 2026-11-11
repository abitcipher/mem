//! The operator-signed hub list (`spec/hublist.cddl`).
//!
//! Decoding here checks only shape and bounds; the signature is checked by the core against the
//! operator keys built into the app.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Domain separation for the operator's signature.
pub const HUB_LIST_DOMAIN: &[u8] = b"MEM/v1/hub-list";
/// Most hubs in one list.
pub const MAX_HUBS: usize = 64;
/// Longest hub URL.
pub const MAX_HUB_URL_LEN: usize = 256;
/// Longest region label.
pub const MAX_REGION_LEN: usize = 32;
/// Most partner hubs of other operators.
pub const MAX_PARTNERS: usize = 16;
/// Longest operator name of a partner.
pub const MAX_OPERATOR_NAME_LEN: usize = 32;
/// Largest encoded list.
pub const MAX_HUB_LIST_LEN: usize = MAX_HUBS * (MAX_HUB_URL_LEN + MAX_REGION_LEN + 48)
    + MAX_PARTNERS * (MAX_HUB_URL_LEN + MAX_OPERATOR_NAME_LEN + 48)
    + 128;

/// One hub.
#[derive(Clone, PartialEq, Eq)]
pub struct HubEntry {
    /// `wss://` URL.
    pub url: String,
    /// The hub's Ed25519 public key, for hub-to-hub authentication.
    pub key: [u8; 32],
    /// Region label, if any.
    pub region: Option<String>,
}

impl fmt::Debug for HubEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HubEntry")
            .field("url", &self.url)
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

/// A hub of another operator that this network exchanges bundles with.
#[derive(Clone, PartialEq, Eq)]
pub struct PartnerEntry {
    /// `wss://` URL.
    pub url: String,
    /// The hub's Ed25519 public key, for hub-to-hub authentication.
    pub key: [u8; 32],
    /// The partner operator's name, shown in the app.
    pub operator: String,
}

impl fmt::Debug for PartnerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartnerEntry")
            .field("url", &self.url)
            .field("operator", &self.operator)
            .finish_non_exhaustive()
    }
}

/// The list, with the operator's signature.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HubList {
    /// Rises with every new list.
    pub serial: u64,
    /// Unix ms.
    pub issued_unix_ms: u64,
    /// Unix ms; the app ignores the list after this.
    pub expires_unix_ms: u64,
    /// Hubs, 1..=[`MAX_HUBS`].
    pub hubs: Vec<HubEntry>,
    /// Ed25519 over [`HUB_LIST_DOMAIN`] || [`HubList::signed_bytes`].
    pub signature: [u8; 64],
    /// Other operators' hubs, 0..=[`MAX_PARTNERS`] (key 6, absent when empty).
    pub partners: Vec<PartnerEntry>,
}

impl HubList {
    fn check(&self) -> Result<(), SchemaError> {
        if self.hubs.is_empty() || self.hubs.len() > MAX_HUBS {
            return Err(SchemaError::WrongLength { key: 4 });
        }
        for h in &self.hubs {
            if h.url.is_empty() || h.url.len() > MAX_HUB_URL_LEN {
                return Err(SchemaError::WrongLength { key: 4 });
            }
            if h.region
                .as_ref()
                .is_some_and(|r| r.is_empty() || r.len() > MAX_REGION_LEN)
            {
                return Err(SchemaError::WrongLength { key: 4 });
            }
        }
        if self.partners.len() > MAX_PARTNERS {
            return Err(SchemaError::WrongLength { key: 6 });
        }
        for p in &self.partners {
            if p.url.is_empty()
                || p.url.len() > MAX_HUB_URL_LEN
                || p.operator.is_empty()
                || p.operator.len() > MAX_OPERATOR_NAME_LEN
            {
                return Err(SchemaError::WrongLength { key: 6 });
            }
        }
        Ok(())
    }

    fn write(
        &self,
        e: &mut minicbor::Encoder<&mut Vec<u8>>,
        signed: bool,
    ) -> Result<(), minicbor::encode::Error<core::convert::Infallible>> {
        let partners = !self.partners.is_empty();
        e.map(5 + u64::from(signed) + u64::from(partners))?;
        e.u8(0)?.u8(1)?;
        e.u8(1)?.u64(self.serial)?;
        e.u8(2)?.u64(self.issued_unix_ms)?;
        e.u8(3)?.u64(self.expires_unix_ms)?;
        e.u8(4)?.array(self.hubs.len() as u64)?;
        for h in &self.hubs {
            e.map(2 + u64::from(h.region.is_some()))?;
            e.u8(0)?.str(&h.url)?;
            e.u8(1)?.bytes(&h.key)?;
            if let Some(r) = &h.region {
                e.u8(2)?.str(r)?;
            }
        }
        if signed {
            e.u8(5)?.bytes(&self.signature)?;
        }
        if partners {
            e.u8(6)?.array(self.partners.len() as u64)?;
            for p in &self.partners {
                e.map(3)?;
                e.u8(0)?.str(&p.url)?;
                e.u8(1)?.bytes(&p.key)?;
                e.u8(2)?.str(&p.operator)?;
            }
        }
        Ok(())
    }

    /// The bytes the operator signs: the canonical map of every key but 5 (the signature).
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the list is out of bounds.
    pub fn signed_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(512, |e| self.write(e, false)))
    }

    /// Canonical CBOR of the whole list.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the list is out of bounds.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(576, |e| self.write(e, true)))
    }

    /// Decodes an untrusted list. The signature is not checked here.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_HUB_LIST_LEN,
            max_container_len: MAX_HUBS as u64,
        };
        if input.len() > limits.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: limits.max_input_len,
            });
        }
        cbor::scan(input, &limits)?;
        let mut d = Decoder::new(input);
        let entries = d.map().ok().flatten().ok_or(SchemaError::MapShape)?;
        if !(6..=7).contains(&entries) {
            return Err(SchemaError::MapShape.into());
        }
        let key = |d: &mut Decoder<'_>, k: u64| -> Result<(), SchemaError> {
            (d.u64().ok() == Some(k))
                .then_some(())
                .ok_or(SchemaError::UnexpectedKey { expected: k })
        };
        key(&mut d, 0)?;
        if d.u8().ok() != Some(1) {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        key(&mut d, 1)?;
        let serial = d.u64().map_err(|_| SchemaError::WrongType { key: 1 })?;
        key(&mut d, 2)?;
        let issued_unix_ms = d.u64().map_err(|_| SchemaError::WrongType { key: 2 })?;
        key(&mut d, 3)?;
        let expires_unix_ms = d.u64().map_err(|_| SchemaError::WrongType { key: 3 })?;
        key(&mut d, 4)?;
        let n = d
            .array()
            .ok()
            .flatten()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| (1..=MAX_HUBS).contains(n))
            .ok_or(SchemaError::WrongLength { key: 4 })?;
        let mut hubs = Vec::with_capacity(n);
        for _ in 0..n {
            let entries = d
                .map()
                .ok()
                .flatten()
                .ok_or(SchemaError::WrongType { key: 4 })?;
            if !(2..=3).contains(&entries) {
                return Err(SchemaError::MapShape.into());
            }
            key(&mut d, 0)?;
            let url = String::from(d.str().map_err(|_| SchemaError::WrongType { key: 4 })?);
            key(&mut d, 1)?;
            let k = d
                .bytes()
                .ok()
                .and_then(|b| <[u8; 32]>::try_from(b).ok())
                .ok_or(SchemaError::WrongLength { key: 4 })?;
            let region = if entries == 3 {
                key(&mut d, 2)?;
                Some(String::from(
                    d.str().map_err(|_| SchemaError::WrongType { key: 4 })?,
                ))
            } else {
                None
            };
            hubs.push(HubEntry {
                url,
                key: k,
                region,
            });
        }
        key(&mut d, 5)?;
        let signature = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 64]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 5 })?;
        let partners = if entries == 7 {
            key(&mut d, 6)?;
            decode_partners(&mut d)?
        } else {
            Vec::new()
        };
        if d.position() != input.len() {
            return Err(SchemaError::MapShape.into());
        }
        let list = Self {
            serial,
            issued_unix_ms,
            expires_unix_ms,
            hubs,
            signature,
            partners,
        };
        list.check()?;
        Ok(list)
    }
}

/// Key 6: the partner list, 1..=[`MAX_PARTNERS`] entries of three keys.
fn decode_partners(d: &mut Decoder<'_>) -> Result<Vec<PartnerEntry>, SchemaError> {
    let key = |d: &mut Decoder<'_>, k: u64| -> Result<(), SchemaError> {
        (d.u64().ok() == Some(k))
            .then_some(())
            .ok_or(SchemaError::UnexpectedKey { expected: k })
    };
    let text = |d: &mut Decoder<'_>| -> Result<String, SchemaError> {
        d.str()
            .map(String::from)
            .map_err(|_| SchemaError::WrongType { key: 6 })
    };
    let n = d
        .array()
        .ok()
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=MAX_PARTNERS).contains(n))
        .ok_or(SchemaError::WrongLength { key: 6 })?;
    let mut partners = Vec::with_capacity(n);
    for _ in 0..n {
        if d.map().ok().flatten() != Some(3) {
            return Err(SchemaError::MapShape);
        }
        key(d, 0)?;
        let url = text(d)?;
        key(d, 1)?;
        let k = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 6 })?;
        key(d, 2)?;
        let operator = text(d)?;
        partners.push(PartnerEntry {
            url,
            key: k,
            operator,
        });
    }
    Ok(partners)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    pub(crate) fn sample() -> HubList {
        HubList {
            serial: 3,
            issued_unix_ms: 1_800_000_000_000,
            expires_unix_ms: 1_802_592_000_000,
            hubs: alloc::vec![
                HubEntry {
                    url: "wss://hub-a.example.org/v1".into(),
                    key: [1; 32],
                    region: Some("eu-central".into()),
                },
                HubEntry {
                    url: "wss://hub-b.example.net/v1".into(),
                    key: [2; 32],
                    region: None,
                },
            ],
            signature: [9; 64],
            partners: Vec::new(),
        }
    }

    fn with_partners() -> HubList {
        HubList {
            partners: alloc::vec![PartnerEntry {
                url: "wss://hub.other.example/v1".into(),
                key: [5; 32],
                operator: "Other operator".into(),
            }],
            ..sample()
        }
    }

    #[test]
    fn partners_roundtrip_and_are_signed() {
        let l = with_partners();
        let bytes = l.encode().unwrap();
        assert_eq!(HubList::decode(&bytes).unwrap(), l);
        // Key 6 is covered by the signature: changing a partner changes the signed bytes.
        let signed = l.signed_bytes().unwrap();
        assert_eq!(signed[0], 0xa6, "map of keys 0..=4 and 6");
        let mut other = l.clone();
        other.partners[0].key[0] ^= 1;
        assert_ne!(other.signed_bytes().unwrap(), signed);
        // Bounds: an empty name, too many partners, an empty list on the wire.
        let mut bad = l.clone();
        bad.partners[0].operator.clear();
        assert!(bad.encode().is_err());
        let mut many = l.clone();
        many.partners = alloc::vec![l.partners[0].clone(); MAX_PARTNERS + 1];
        assert!(many.encode().is_err());
        let mut empty = sample().encode().unwrap();
        empty[0] += 1;
        empty.extend_from_slice(&[0x06, 0x80]);
        assert!(HubList::decode(&empty).is_err());
    }

    #[test]
    fn roundtrips_and_signs_without_the_signature() {
        let l = sample();
        let bytes = l.encode().unwrap();
        assert_eq!(HubList::decode(&bytes).unwrap(), l);
        let signed = l.signed_bytes().unwrap();
        assert!(!signed.windows(64).any(|w| w == [9; 64]));
        assert_eq!(signed[0], 0xa5, "map of keys 0..=4");
    }

    #[test]
    fn bounds() {
        let mut l = sample();
        l.hubs.clear();
        assert!(l.encode().is_err());
        let mut l = sample();
        l.hubs[0].url = "x".repeat(MAX_HUB_URL_LEN + 1);
        assert!(l.encode().is_err());
        assert!(HubList::decode(&[0xa0]).is_err());
    }

    /// Published vector (`spec/vectors/hublist/`), validated against `spec/hublist.cddl` in CI.
    #[test]
    fn published_vector() {
        extern crate std;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/hublist");
        let bytes = sample().encode().unwrap();
        if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("hublist.cbor"), &bytes).unwrap();
            std::fs::write(
                dir.join("hublist-partners.cbor"),
                with_partners().encode().unwrap(),
            )
            .unwrap();
        }
        assert_eq!(std::fs::read(dir.join("hublist.cbor")).unwrap(), bytes);
        assert_eq!(
            std::fs::read(dir.join("hublist-partners.cbor")).unwrap(),
            with_partners().encode().unwrap()
        );
    }
}
