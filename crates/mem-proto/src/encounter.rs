//! Encounter protocol messages (`spec/encounter.cddl`).
//!
//! `hello`, `tags`, `summary` (anti-entropy blooms), `purge-tokens` (gossip), `bundles`
//! (each optionally with a direct-delivery proof) and `bye`.

use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::envelope::MAX_BUNDLE_LEN;
use crate::error::{ProtoError, SchemaError};

/// Encounter protocol version.
pub const ENCOUNTER_VERSION: u8 = 1;
/// Most receiving tags in one `tags` message.
pub const MAX_TAGS: usize = 4096;
/// Most bundles in one `bundles` message.
pub const MAX_BUNDLES_PER_MSG: usize = 16;
/// Largest encoded encounter message: a full `bundles` message (with proofs) plus headers.
pub const MAX_MSG_LEN: usize = MAX_BUNDLES_PER_MSG * (MAX_BUNDLE_LEN + 8 + 20) + 64;
/// Largest bloom filter in a `summary`, in bytes.
pub const MAX_BLOOM_LEN: usize = 32 * 1024;
/// Most purge tokens in one `purge-tokens` message.
pub const MAX_PURGE_TOKENS: usize = 1024;

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_MSG_LEN,
    max_container_len: MAX_TAGS as u64,
};

const KIND_HELLO: u8 = 0;
const KIND_TAGS: u8 = 1;
const KIND_SUMMARY: u8 = 2;
const KIND_BUNDLES: u8 = 3;
const KIND_BYE: u8 = 4;
const KIND_PURGE_TOKENS: u8 = 5;
const KIND_OFFER: u8 = 6;
const KIND_WANT: u8 = 7;
/// Most tags in one `offer` or `want`.
pub const MAX_OFFER_TAGS: usize = 1024;

/// One bundle in a `bundles` message.
#[derive(Clone, PartialEq, Eq)]
pub struct BundleEntry {
    /// The encoded bundle (untrusted; decoded by the agent).
    pub bytes: Vec<u8>,
    /// Direct-delivery proof: present only when the sender hands its own bundle
    /// to the recipient.
    pub proof: Option<[u8; 16]>,
}

impl BundleEntry {
    /// An entry without a proof (relayed, or not addressed to the peer).
    #[must_use]
    pub const fn bare(bytes: Vec<u8>) -> Self {
        Self { bytes, proof: None }
    }
}

/// One encounter message.
#[derive(Clone, PartialEq, Eq)]
pub enum EncounterMsg {
    /// Opens the session.
    Hello {
        /// Protocol version.
        version: u8,
        /// Random per encounter; never reused.
        ephemeral_id: [u8; 16],
    },
    /// The sender's current receiving tags, padded with random decoys.
    Tags(Vec<[u8; 16]>),
    /// What the sender holds, as salted blooms.
    Summary {
        /// Random per summary; keys the bloom hashes.
        salt: [u8; 16],
        /// Bloom over held `bundle_id || purge_commitment`.
        held: Vec<u8>,
        /// Bloom over held purge commitments whose token the sender does not know.
        wanted: Vec<u8>,
    },
    /// Purge tokens to apply and pass on.
    PurgeTokens(Vec<[u8; 16]>),
    /// Tags of older bundles a courier carries, for the peer to claim.
    Offer(Vec<[u8; 16]>),
    /// Offered tags the peer recognises as its own.
    Want(Vec<[u8; 16]>),
    /// Bundles, each still untrusted.
    Bundles(Vec<BundleEntry>),
    /// Ends the session.
    Bye,
}

impl fmt::Debug for EncounterMsg {
    /// Never prints tags or bundle bytes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hello { version, .. } => write!(f, "Hello(v{version})"),
            Self::Tags(t) => write!(f, "Tags({})", t.len()),
            Self::Summary { held, wanted, .. } => {
                write!(f, "Summary({}+{} B)", held.len(), wanted.len())
            }
            Self::PurgeTokens(t) => write!(f, "PurgeTokens({})", t.len()),
            Self::Offer(t) => write!(f, "Offer({})", t.len()),
            Self::Want(t) => write!(f, "Want({})", t.len()),
            Self::Bundles(b) => write!(f, "Bundles({})", b.len()),
            Self::Bye => f.write_str("Bye"),
        }
    }
}

impl EncounterMsg {
    /// Canonical CBOR encoding.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if a list, bloom or bundle exceeds its limit.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        match self {
            Self::Hello {
                version,
                ephemeral_id,
            } => Ok(encode_with(32, |e| {
                e.array(2)?.u8(KIND_HELLO)?.map(3)?;
                e.u8(0)?.u8(*version)?;
                e.u8(1)?.bytes(ephemeral_id)?;
                e.u8(2)?.map(0)?;
                Ok(())
            })),
            Self::Tags(tags) => {
                if tags.len() > MAX_TAGS {
                    return Err(SchemaError::WrongLength { key: 0 }.into());
                }
                Ok(encode_with(8 + tags.len() * 17, |e| {
                    e.array(2)?.u8(KIND_TAGS)?.map(1)?.u8(0)?;
                    e.array(tags.len() as u64)?;
                    for t in tags {
                        e.bytes(t)?;
                    }
                    Ok(())
                }))
            }
            Self::Summary { salt, held, wanted } => {
                if held.len() > MAX_BLOOM_LEN || wanted.len() > MAX_BLOOM_LEN {
                    return Err(SchemaError::WrongLength { key: 1 }.into());
                }
                Ok(encode_with(40 + held.len() + wanted.len(), |e| {
                    e.array(2)?.u8(KIND_SUMMARY)?.map(3)?;
                    e.u8(0)?.bytes(salt)?;
                    e.u8(1)?.bytes(held)?;
                    e.u8(2)?.bytes(wanted)?;
                    Ok(())
                }))
            }
            Self::PurgeTokens(tokens) => {
                if tokens.len() > MAX_PURGE_TOKENS {
                    return Err(SchemaError::WrongLength { key: 0 }.into());
                }
                Ok(encode_with(8 + tokens.len() * 17, |e| {
                    e.array(2)?.u8(KIND_PURGE_TOKENS)?.map(1)?.u8(0)?;
                    e.array(tokens.len() as u64)?;
                    for t in tokens {
                        e.bytes(t)?;
                    }
                    Ok(())
                }))
            }
            Self::Offer(tags) | Self::Want(tags) => {
                if tags.len() > MAX_OFFER_TAGS {
                    return Err(SchemaError::WrongLength { key: 0 }.into());
                }
                let kind = if matches!(self, Self::Offer(_)) {
                    KIND_OFFER
                } else {
                    KIND_WANT
                };
                Ok(encode_with(8 + tags.len() * 17, |e| {
                    e.array(2)?.u8(kind)?.map(1)?.u8(0)?;
                    e.array(tags.len() as u64)?;
                    for t in tags {
                        e.bytes(t)?;
                    }
                    Ok(())
                }))
            }
            Self::Bundles(bundles) => {
                if bundles.len() > MAX_BUNDLES_PER_MSG
                    || bundles.iter().any(|b| b.bytes.len() > MAX_BUNDLE_LEN)
                {
                    return Err(SchemaError::WrongLength { key: 0 }.into());
                }
                let total: usize = bundles.iter().map(|b| b.bytes.len() + 24).sum();
                Ok(encode_with(8 + total, |e| {
                    e.array(2)?.u8(KIND_BUNDLES)?.map(1)?.u8(0)?;
                    e.array(bundles.len() as u64)?;
                    for b in bundles {
                        // A bare bundle is a byte string; with a proof, a two-element array.
                        match &b.proof {
                            None => {
                                e.bytes(&b.bytes)?;
                            }
                            Some(proof) => {
                                e.array(2)?.bytes(&b.bytes)?.bytes(proof)?;
                            }
                        }
                    }
                    Ok(())
                }))
            }
            Self::Bye => Ok(encode_with(4, |e| {
                e.array(2)?.u8(KIND_BYE)?.map(0)?;
                Ok(())
            })),
        }
    }

    /// Decodes an untrusted message.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for oversized, malformed, non-canonical or unknown messages.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        if input.len() > LIMITS.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: LIMITS.max_input_len,
            });
        }
        cbor::scan(input, &LIMITS)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(2)) {
            return Err(SchemaError::MapShape.into());
        }
        let kind = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
        let entries = d.map().ok().flatten().ok_or(SchemaError::MapShape)?;
        let msg = match (kind, entries) {
            (KIND_HELLO, 3) => {
                key(&mut d, 0)?;
                let version = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                let ephemeral_id = d
                    .bytes()
                    .ok()
                    .and_then(|b| <[u8; 16]>::try_from(b).ok())
                    .ok_or(SchemaError::WrongLength { key: 1 })?;
                key(&mut d, 2)?;
                skip_caps(&mut d)?;
                Self::Hello {
                    version,
                    ephemeral_id,
                }
            }
            (KIND_TAGS, 1) => Self::Tags(id16_list(&mut d, MAX_TAGS)?),
            (KIND_SUMMARY, 3) => {
                key(&mut d, 0)?;
                let salt = bytes16(&mut d, 0)?;
                key(&mut d, 1)?;
                let held = bloom(&mut d, 1)?;
                key(&mut d, 2)?;
                let wanted = bloom(&mut d, 2)?;
                Self::Summary { salt, held, wanted }
            }
            (KIND_PURGE_TOKENS, 1) => Self::PurgeTokens(id16_list(&mut d, MAX_PURGE_TOKENS)?),
            (KIND_OFFER, 1) => Self::Offer(id16_list(&mut d, MAX_OFFER_TAGS)?),
            (KIND_WANT, 1) => Self::Want(id16_list(&mut d, MAX_OFFER_TAGS)?),
            (KIND_BUNDLES, 1) => {
                key(&mut d, 0)?;
                let n = list_len(&mut d, MAX_BUNDLES_PER_MSG)?;
                let mut bundles = Vec::with_capacity(n);
                for _ in 0..n {
                    let entry = bundle_entry(&mut d)?;
                    if entry.bytes.len() > MAX_BUNDLE_LEN {
                        return Err(SchemaError::WrongLength { key: 0 }.into());
                    }
                    bundles.push(entry);
                }
                Self::Bundles(bundles)
            }
            (KIND_BYE, 0) => Self::Bye,
            (
                KIND_HELLO | KIND_TAGS | KIND_SUMMARY | KIND_PURGE_TOKENS | KIND_OFFER | KIND_WANT
                | KIND_BUNDLES | KIND_BYE,
                _,
            ) => {
                return Err(SchemaError::MapShape.into());
            }
            _ => return Err(SchemaError::UnknownValue { key: 0 }.into()),
        };
        // `cbor::scan` guarantees exactly one top-level item, so nothing can follow.
        if d.position() != input.len() {
            return Err(SchemaError::MapShape.into());
        }
        Ok(msg)
    }
}

fn key(d: &mut Decoder<'_>, expected: u64) -> Result<(), SchemaError> {
    match d.u64() {
        Ok(k) if k == expected => Ok(()),
        _ => Err(SchemaError::UnexpectedKey { expected }),
    }
}

/// A bare bundle (byte string) or `[bundle, proof]`.
fn bundle_entry(d: &mut Decoder<'_>) -> Result<BundleEntry, SchemaError> {
    if matches!(d.datatype(), Ok(minicbor::data::Type::Array)) {
        if d.array().ok().flatten() != Some(2) {
            return Err(SchemaError::WrongLength { key: 0 });
        }
        let b = d.bytes().map_err(|_| SchemaError::WrongType { key: 0 })?;
        let proof = bytes16(d, 0)?;
        return Ok(BundleEntry {
            bytes: b.to_vec(),
            proof: Some(proof),
        });
    }
    let b = d.bytes().map_err(|_| SchemaError::WrongType { key: 0 })?;
    Ok(BundleEntry::bare(b.to_vec()))
}

/// `{0: [0*max bstr .size 16]}` after the kind: tags, tokens, offers and wants.
fn id16_list(d: &mut Decoder<'_>, max: usize) -> Result<Vec<[u8; 16]>, SchemaError> {
    key(d, 0)?;
    let n = list_len(d, max)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(bytes16(d, 0)?);
    }
    Ok(out)
}

fn bytes16(d: &mut Decoder<'_>, key: u64) -> Result<[u8; 16], SchemaError> {
    d.bytes()
        .ok()
        .and_then(|b| <[u8; 16]>::try_from(b).ok())
        .ok_or(SchemaError::WrongLength { key })
}

fn bloom(d: &mut Decoder<'_>, key: u64) -> Result<Vec<u8>, SchemaError> {
    let b = d.bytes().map_err(|_| SchemaError::WrongType { key })?;
    if b.len() > MAX_BLOOM_LEN {
        return Err(SchemaError::WrongLength { key });
    }
    Ok(b.to_vec())
}

fn list_len(d: &mut Decoder<'_>, max: usize) -> Result<usize, SchemaError> {
    d.array()
        .ok()
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n <= max)
        .ok_or(SchemaError::WrongLength { key: 0 })
}

/// `caps = { * uint => any }`: keys must be ascending unsigned integers; values are skipped.
/// Nesting is already bounded by `cbor::scan`.
fn skip_caps(d: &mut Decoder<'_>) -> Result<(), SchemaError> {
    let n = d
        .map()
        .ok()
        .flatten()
        .ok_or(SchemaError::WrongType { key: 2 })?;
    let mut last = None;
    for _ in 0..n {
        let k = d.u64().map_err(|_| SchemaError::WrongType { key: 2 })?;
        if last.is_some_and(|l| k <= l) {
            return Err(SchemaError::UnexpectedKey { expected: 2 });
        }
        last = Some(k);
        d.skip().map_err(|_| SchemaError::WrongType { key: 2 })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec;

    fn roundtrip(m: &EncounterMsg) {
        let bytes = m.encode().unwrap();
        assert_eq!(&EncounterMsg::decode(&bytes).unwrap(), m);
    }

    #[test]
    fn roundtrips() {
        roundtrip(&EncounterMsg::Hello {
            version: 1,
            ephemeral_id: [9; 16],
        });
        roundtrip(&EncounterMsg::Tags(vec![]));
        roundtrip(&EncounterMsg::Tags(vec![[1; 16], [2; 16]]));
        roundtrip(&EncounterMsg::Bundles(vec![
            BundleEntry::bare(vec![1, 2, 3]),
            BundleEntry {
                bytes: vec![0; 70_000 - 5000],
                proof: Some([4; 16]),
            },
        ]));
        roundtrip(&EncounterMsg::Summary {
            salt: [1; 16],
            held: vec![0xF0; 1000],
            wanted: vec![],
        });
        roundtrip(&EncounterMsg::PurgeTokens(vec![[2; 16]; 3]));
        roundtrip(&EncounterMsg::Offer(vec![[3; 16]; 2]));
        roundtrip(&EncounterMsg::Want(vec![[4; 16]]));
        assert!(
            EncounterMsg::Offer(vec![[0; 16]; MAX_OFFER_TAGS + 1])
                .encode()
                .is_err()
        );
        roundtrip(&EncounterMsg::Bye);
    }

    #[test]
    fn known_vectors() {
        // [0, {0: 1, 1: h'00..', 2: {}}]
        let hello = EncounterMsg::Hello {
            version: 1,
            ephemeral_id: [0; 16],
        };
        assert_eq!(
            hex::encode(hello.encode().unwrap()),
            format!("8200a3000101{}{}02a0", "50", "00".repeat(16))
        );
        assert_eq!(hex::encode(EncounterMsg::Bye.encode().unwrap()), "8204a0");
    }

    #[test]
    fn hello_accepts_unknown_caps_but_checks_order() {
        // caps {1: true, 7: [1, 2]}
        let ok = hex::decode(format!("8200a300010150{}02a201f507820102", "00".repeat(16))).unwrap();
        assert!(matches!(
            EncounterMsg::decode(&ok).unwrap(),
            EncounterMsg::Hello { version: 1, .. }
        ));
        // caps {7: true, 1: true}: keys out of order.
        let bad = hex::decode(format!("8200a300010150{}02a207f501f5", "00".repeat(16))).unwrap();
        assert!(EncounterMsg::decode(&bad).is_err());
    }

    #[test]
    fn summary_and_tokens_are_bounded() {
        // The old skeleton summary [2, {0: h'', 1: []}] is no longer valid.
        assert!(EncounterMsg::decode(&hex::decode("8202a200400180").unwrap()).is_err());
        // [5, {0: []}] is an empty token list.
        assert_eq!(
            EncounterMsg::decode(&hex::decode("8205a10080").unwrap()).unwrap(),
            EncounterMsg::PurgeTokens(vec![])
        );
        assert!(
            EncounterMsg::PurgeTokens(vec![[0; 16]; MAX_PURGE_TOKENS + 1])
                .encode()
                .is_err()
        );
        assert!(
            EncounterMsg::Summary {
                salt: [0; 16],
                held: vec![0; MAX_BLOOM_LEN + 1],
                wanted: vec![],
            }
            .encode()
            .is_err()
        );
        // A proof of the wrong length.
        let bad = hex::decode("8203a1008182430102034101").unwrap();
        assert!(EncounterMsg::decode(&bad).is_err());
        // Unknown kind 9.
        assert!(EncounterMsg::decode(&hex::decode("8209a0").unwrap()).is_err());
    }

    #[test]
    fn enforces_limits() {
        assert!(
            EncounterMsg::Tags(vec![[0; 16]; MAX_TAGS + 1])
                .encode()
                .is_err()
        );
        assert!(
            EncounterMsg::Bundles(vec![BundleEntry::bare(vec![0]); MAX_BUNDLES_PER_MSG + 1])
                .encode()
                .is_err()
        );
        assert!(
            EncounterMsg::Bundles(vec![BundleEntry::bare(vec![0; MAX_BUNDLE_LEN + 1])])
                .encode()
                .is_err()
        );
        // A tag of the wrong length.
        assert!(EncounterMsg::decode(&hex::decode("8201a100814100").unwrap()).is_err());
        // Wrong map shape for bye.
        assert!(EncounterMsg::decode(&hex::decode("8204a10000").unwrap()).is_err());
        assert!(EncounterMsg::decode(&vec![0u8; MAX_MSG_LEN + 1]).is_err());
    }
}
