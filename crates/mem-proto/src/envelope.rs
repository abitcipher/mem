//! Outer bundle envelope, v1 (`spec/envelope.cddl`).
//!
//! The envelope is the only part of a bundle that carriers see. By construction it contains
//! only the fields of `spec/envelope.cddl`. The payload is opaque here; its HPKE wrapper is
//! specified in `spec/payload.cddl`.

use alloc::vec::Vec;
use core::convert::Infallible;
use core::fmt;

use minicbor::{Decoder, Encoder, encode};

use crate::cbor::{self, DecodeLimits};
use crate::error::{InvalidBundle, ProtoError, SchemaError};

/// The only supported envelope version.
pub const PROTOCOL_VERSION: u64 = 1;
/// Maximum encoded bundle size on any CLA, v1.
pub const MAX_BUNDLE_LEN: usize = 64 * 1024;
/// Maximum `hops_remaining` (sender, up to four couriers, receiver).
pub const MAX_HOPS: u8 = 5;
/// Maximum spray-and-wait copy budget.
pub const MAX_COPIES: u8 = 64;
/// Maximum lifetime of any bundle: 30 days.
pub const MAX_LIFETIME_S: u32 = 30 * 24 * 60 * 60;
/// Maximum lifetime of an intro bundle: 72 hours.
pub const MAX_INTRO_LIFETIME_S: u32 = 72 * 60 * 60;
/// Length of the proof-of-work field: `nonce (8) || difficulty (1)`.
pub const POW_LEN: usize = 9;
/// Maximum copy budget of an intro bundle.
pub const MAX_INTRO_COPIES: u8 = 4;

/// Decoder bounds for a bundle. The envelope is a flat map of at most 10 entries.
const BUNDLE_LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_BUNDLE_LEN,
    max_container_len: 11,
};

mod key {
    pub(super) const VERSION: u64 = 0;
    pub(super) const BUNDLE_ID: u64 = 1;
    pub(super) const KIND: u64 = 2;
    pub(super) const DST_TAG: u64 = 3;
    pub(super) const LIFETIME_S: u64 = 4;
    pub(super) const AGE_MS: u64 = 5;
    pub(super) const HOPS_REMAINING: u64 = 6;
    pub(super) const COPIES: u64 = 7;
    pub(super) const POW: u64 = 8;
    pub(super) const PAYLOAD: u64 = 9;
    pub(super) const PURGE_COMMITMENT: u64 = 10;
}

/// 128 random bits identifying a bundle.
///
/// Never a UUIDv7: a timestamped id would leak creation time to carriers.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BundleId([u8; 16]);

impl BundleId {
    /// Wraps 16 bytes. Callers must fill them from an injected CSPRNG.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw id.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for BundleId {
    /// Shows only a 2-byte prefix: full bundle ids must never be logged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BundleId({:02x}{:02x}…)", self.0[0], self.0[1])
    }
}

/// Blinded, rotating destination identifier. Never a user id.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RendezvousTag([u8; 16]);

impl RendezvousTag {
    /// Wraps 16 bytes produced by tag derivation in `mem-crypto`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The raw tag.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for RendezvousTag {
    /// Fully redacted: tags must never be logged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RendezvousTag(<redacted>)")
    }
}

/// Bundle kind (envelope field 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BundleKind {
    /// A user message.
    Message,
    /// A delivery acknowledgement.
    Ack,
    /// A remote introduction to an unapproved recipient.
    Intro,
    /// Protocol control traffic.
    Control,
    /// A mailbox key or tag epoch update.
    KeyUpdate,
}

impl BundleKind {
    const fn to_wire(self) -> u8 {
        match self {
            Self::Message => 0,
            Self::Ack => 1,
            Self::Intro => 2,
            Self::Control => 3,
            Self::KeyUpdate => 4,
        }
    }

    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Message),
            1 => Some(Self::Ack),
            2 => Some(Self::Intro),
            3 => Some(Self::Control),
            4 => Some(Self::KeyUpdate),
            _ => None,
        }
    }

    /// Lifetime cap for this kind.
    #[must_use]
    pub const fn max_lifetime_s(self) -> u32 {
        match self {
            Self::Intro => MAX_INTRO_LIFETIME_S,
            _ => MAX_LIFETIME_S,
        }
    }
}

/// Proof of work of an intro bundle: `nonce (8) || difficulty (1)`. Checked by
/// `mem-crypto`; this type only fixes the shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pow([u8; POW_LEN]);

impl Pow {
    /// Builds the field from its parts.
    #[must_use]
    pub fn new(nonce: [u8; 8], difficulty: u8) -> Self {
        let mut b = [0u8; POW_LEN];
        b[..8].copy_from_slice(&nonce);
        b[8] = difficulty;
        Self(b)
    }

    /// Parses the wire bytes.
    ///
    /// # Errors
    ///
    /// [`InvalidBundle::PowLength`] unless `bytes` is exactly [`POW_LEN`] long.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InvalidBundle> {
        <[u8; POW_LEN]>::try_from(bytes)
            .map(Self)
            .map_err(|_| InvalidBundle::PowLength(bytes.len()))
    }

    /// The nonce.
    #[must_use]
    pub fn nonce(&self) -> [u8; 8] {
        let mut n = [0u8; 8];
        n.copy_from_slice(&self.0[..8]);
        n
    }

    /// The claimed difficulty in leading zero bits.
    #[must_use]
    pub const fn difficulty(&self) -> u8 {
        self.0[8]
    }

    /// The raw field.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; POW_LEN] {
        &self.0
    }
}

/// Inputs for [`Bundle::new`]. Field meanings follow `spec/envelope.cddl`.
#[derive(Clone, PartialEq, Eq)]
pub struct BundleParams {
    /// Random bundle id.
    pub bundle_id: BundleId,
    /// Bundle kind.
    pub kind: BundleKind,
    /// Destination rendezvous tag.
    pub dst_tag: RendezvousTag,
    /// Maximum lifetime in seconds.
    pub lifetime_s: u32,
    /// Time since creation in milliseconds, accumulated across hops.
    pub age_ms: u64,
    /// Remaining hand-offs.
    pub hops_remaining: u8,
    /// Spray-and-wait copy budget.
    pub copies: u8,
    /// Optional proof of work.
    pub pow: Option<Pow>,
    /// Encrypted payload (HPKE or MLS ciphertext). Never plaintext.
    pub payload: Vec<u8>,
    /// Commitment to the purge token carried inside the payload.
    pub purge_commitment: [u8; 16],
}

impl fmt::Debug for BundleParams {
    /// Redacts the id prefix, the tag and the payload contents.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundleParams")
            .field("bundle_id", &self.bundle_id)
            .field("kind", &self.kind)
            .field("dst_tag", &self.dst_tag)
            .field("lifetime_s", &self.lifetime_s)
            .field("age_ms", &self.age_ms)
            .field("hops_remaining", &self.hops_remaining)
            .field("copies", &self.copies)
            .field("pow_len", &self.pow.as_ref().map(|p| p.0.len()))
            .field("payload_len", &self.payload.len())
            .field(
                "purge_commitment",
                &format_args!("{:02x}…", self.purge_commitment[0]),
            )
            .finish()
    }
}

/// A validated bundle envelope.
///
/// Every value of this type satisfies the v1 protocol rules: hop limit, copy budget,
/// lifetime cap for its kind, not expired, PoW length, and encoded size at most
/// [`MAX_BUNDLE_LEN`]. Fields are private so that the rules cannot be bypassed.
#[derive(Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "field names mirror spec/envelope.cddl"
)]
pub struct Bundle {
    bundle_id: BundleId,
    kind: BundleKind,
    dst_tag: RendezvousTag,
    lifetime_s: u32,
    age_ms: u64,
    hops_remaining: u8,
    copies: u8,
    pow: Option<Pow>,
    payload: Vec<u8>,
    purge_commitment: [u8; 16],
}

impl fmt::Debug for Bundle {
    /// Redacts the id prefix, the tag and the payload contents.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bundle")
            .field("bundle_id", &self.bundle_id)
            .field("kind", &self.kind)
            .field("dst_tag", &self.dst_tag)
            .field("lifetime_s", &self.lifetime_s)
            .field("age_ms", &self.age_ms)
            .field("hops_remaining", &self.hops_remaining)
            .field("copies", &self.copies)
            .field("pow_len", &self.pow.as_ref().map(|p| p.0.len()))
            .field("payload_len", &self.payload.len())
            .field(
                "purge_commitment",
                &format_args!("{:02x}…", self.purge_commitment[0]),
            )
            .finish()
    }
}

impl Bundle {
    /// Builds a bundle, enforcing every v1 protocol rule.
    ///
    /// # Errors
    ///
    /// [`ProtoError::Invalid`] naming the first violated rule.
    pub fn new(params: BundleParams) -> Result<Self, ProtoError> {
        let BundleParams {
            bundle_id,
            kind,
            dst_tag,
            lifetime_s,
            age_ms,
            hops_remaining,
            copies,
            pow,
            payload,
            purge_commitment,
        } = params;
        let bundle = Self {
            bundle_id,
            kind,
            dst_tag,
            lifetime_s,
            age_ms,
            hops_remaining,
            copies,
            pow,
            payload,
            purge_commitment,
        };
        bundle.validate_rules()?;
        let len = bundle.encode().len();
        if len > MAX_BUNDLE_LEN {
            return Err(InvalidBundle::TooLarge {
                len,
                max: MAX_BUNDLE_LEN,
            }
            .into());
        }
        Ok(bundle)
    }

    /// Decodes and validates an untrusted bundle with the default size cap
    /// ([`MAX_BUNDLE_LEN`]).
    ///
    /// # Errors
    ///
    /// See [`Bundle::decode_with_max_len`].
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        Self::decode_with_max_len(input, MAX_BUNDLE_LEN)
    }

    /// Decodes and validates an untrusted bundle, rejecting input longer than `max_len`
    /// (clamped to [`MAX_BUNDLE_LEN`]). CLAs and courier quotas may use a smaller cap.
    ///
    /// Accepts only the deterministic encoding: for every `b` that decodes successfully,
    /// `Bundle::decode(b)?.encode() == b`.
    ///
    /// # Errors
    ///
    /// - [`ProtoError::InputTooLarge`] before any parsing;
    /// - [`ProtoError::Cbor`] for malformed or non-canonical CBOR;
    /// - [`ProtoError::Schema`] for missing, unknown or mistyped fields;
    /// - [`ProtoError::Invalid`] for protocol-rule violations.
    ///
    /// # Security
    ///
    /// The input is hostile. Size is checked first, then [`cbor::scan`] bounds the
    /// structure, and only then are fields read. Never panics; allocates at most the payload
    /// and PoW lengths, which the scan has already bounded by the input length.
    pub fn decode_with_max_len(input: &[u8], max_len: usize) -> Result<Self, ProtoError> {
        let max = max_len.min(MAX_BUNDLE_LEN);
        if input.len() > max {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max,
            });
        }
        cbor::scan(input, &BUNDLE_LIMITS)?;

        let mut d = Decoder::new(input);
        let Ok(Some(entries @ (10 | 11))) = d.map() else {
            return Err(SchemaError::MapShape.into());
        };

        expect_key(&mut d, key::VERSION)?;
        let version = d.u64().map_err(|_| wrong_type(key::VERSION))?;
        if version != PROTOCOL_VERSION {
            return Err(InvalidBundle::UnsupportedVersion(version).into());
        }

        expect_key(&mut d, key::BUNDLE_ID)?;
        let bundle_id = BundleId(read_array16(&mut d, key::BUNDLE_ID)?);

        expect_key(&mut d, key::KIND)?;
        let kind_raw = d.u8().map_err(|_| wrong_type(key::KIND))?;
        let kind =
            BundleKind::from_wire(kind_raw).ok_or(SchemaError::UnknownValue { key: key::KIND })?;

        expect_key(&mut d, key::DST_TAG)?;
        let dst_tag = RendezvousTag(read_array16(&mut d, key::DST_TAG)?);

        expect_key(&mut d, key::LIFETIME_S)?;
        let lifetime_s = d.u32().map_err(|_| wrong_type(key::LIFETIME_S))?;

        expect_key(&mut d, key::AGE_MS)?;
        let age_ms = d.u64().map_err(|_| wrong_type(key::AGE_MS))?;

        expect_key(&mut d, key::HOPS_REMAINING)?;
        let hops_remaining = d.u8().map_err(|_| wrong_type(key::HOPS_REMAINING))?;

        expect_key(&mut d, key::COPIES)?;
        let copies = d.u8().map_err(|_| wrong_type(key::COPIES))?;

        let pow = if entries == 11 {
            expect_key(&mut d, key::POW)?;
            let bytes = d.bytes().map_err(|_| wrong_type(key::POW))?;
            Some(Pow::from_bytes(bytes)?)
        } else {
            None
        };

        expect_key(&mut d, key::PAYLOAD)?;
        let payload = d.bytes().map_err(|_| wrong_type(key::PAYLOAD))?.to_vec();

        expect_key(&mut d, key::PURGE_COMMITMENT)?;
        let purge_commitment = read_array16(&mut d, key::PURGE_COMMITMENT)?;

        let bundle = Self {
            bundle_id,
            kind,
            dst_tag,
            lifetime_s,
            age_ms,
            hops_remaining,
            copies,
            pow,
            payload,
            purge_commitment,
        };
        bundle.validate_rules()?;
        Ok(bundle)
    }

    /// Encodes the bundle in deterministic CBOR.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.payload.len() + 96);
        // Writing to a `Vec` cannot fail (`Error = Infallible`) and no custom messages are
        // produced, so the result carries no information.
        let _: Result<(), encode::Error<Infallible>> = self.write(&mut Encoder::new(&mut buf));
        buf
    }

    /// The HPKE associated data binding this envelope to its payload:
    /// the deterministic CBOR map of fields 0 (version), 1 (`bundle_id`), 2 (kind),
    /// 3 (`dst_tag`) and 10 (`purge_commitment`).
    ///
    /// # Security
    ///
    /// Covers exactly the immutable fields that select the recipient and identify the bundle.
    /// Mutable per-hop fields (`age_ms`, `hops_remaining`, `copies`) are deliberately
    /// excluded. Changing this set is a wire-format change.
    #[must_use]
    pub fn aad(&self) -> Vec<u8> {
        aad_for(
            &self.bundle_id,
            self.kind,
            &self.dst_tag,
            &self.purge_commitment,
        )
    }

    fn write(&self, e: &mut Encoder<&mut Vec<u8>>) -> Result<(), encode::Error<Infallible>> {
        e.map(if self.pow.is_some() { 11 } else { 10 })?
            .u64(key::VERSION)?
            .u64(PROTOCOL_VERSION)?
            .u64(key::BUNDLE_ID)?
            .bytes(&self.bundle_id.0)?
            .u64(key::KIND)?
            .u8(self.kind.to_wire())?
            .u64(key::DST_TAG)?
            .bytes(&self.dst_tag.0)?
            .u64(key::LIFETIME_S)?
            .u32(self.lifetime_s)?
            .u64(key::AGE_MS)?
            .u64(self.age_ms)?
            .u64(key::HOPS_REMAINING)?
            .u8(self.hops_remaining)?
            .u64(key::COPIES)?
            .u8(self.copies)?;
        if let Some(pow) = &self.pow {
            e.u64(key::POW)?.bytes(&pow.0)?;
        }
        e.u64(key::PAYLOAD)?
            .bytes(&self.payload)?
            .u64(key::PURGE_COMMITMENT)?
            .bytes(&self.purge_commitment)?;
        Ok(())
    }

    fn validate_rules(&self) -> Result<(), InvalidBundle> {
        if self.hops_remaining > MAX_HOPS {
            return Err(InvalidBundle::HopLimit(self.hops_remaining));
        }
        if !(1..=MAX_COPIES).contains(&self.copies) {
            return Err(InvalidBundle::Copies(self.copies));
        }
        let max_s = self.kind.max_lifetime_s();
        if self.lifetime_s > max_s {
            return Err(InvalidBundle::Lifetime {
                got_s: self.lifetime_s,
                max_s,
            });
        }
        if self.age_ms >= u64::from(self.lifetime_s) * 1000 {
            return Err(InvalidBundle::Expired);
        }
        // Intro bundles carry a proof of work and few copies; nothing else carries one.
        if self.kind == BundleKind::Intro {
            if self.copies > MAX_INTRO_COPIES {
                return Err(InvalidBundle::Copies(self.copies));
            }
            if self.pow.is_none() {
                return Err(InvalidBundle::PowMissing);
            }
        } else if self.pow.is_some() {
            return Err(InvalidBundle::PowNotAllowed);
        }
        Ok(())
    }

    /// Returns the bundle with `age_ms` replaced, as each holder does before handing it on.
    /// Does not change the AAD, so the payload stays valid.
    ///
    /// # Errors
    ///
    /// [`InvalidBundle::Expired`] if the new age reaches the lifetime.
    pub fn with_age_ms(mut self, age_ms: u64) -> Result<Self, ProtoError> {
        self.age_ms = age_ms;
        self.validate_rules()?;
        Ok(self)
    }

    /// Returns the bundle with the hop and copy budget replaced, as a holder does when it hands a
    /// spray copy on. Both fields are outside the AAD, so the payload stays
    /// valid.
    ///
    /// # Errors
    ///
    /// [`InvalidBundle::HopLimit`] above [`MAX_HOPS`]; [`InvalidBundle::Copies`] outside
    /// `1..=MAX_COPIES`.
    pub fn with_relay(mut self, hops_remaining: u8, copies: u8) -> Result<Self, ProtoError> {
        self.hops_remaining = hops_remaining;
        self.copies = copies;
        self.validate_rules()?;
        Ok(self)
    }

    /// A stored bundle re-encoded for the next holder with new mutable fields (hops, copies,
    /// age), as couriers do when forwarding. The AAD and payload are unchanged.
    ///
    /// # Errors
    ///
    /// Any decode or rule error: the stored bytes are hostile like everything else.
    pub fn relayed(
        stored: &[u8],
        hops_remaining: u8,
        copies: u8,
        age_ms: u64,
    ) -> Result<Vec<u8>, ProtoError> {
        Ok(Self::decode(stored)?
            .with_relay(hops_remaining, copies)?
            .with_age_ms(age_ms)?
            .encode())
    }

    /// Bundle id.
    #[must_use]
    pub const fn bundle_id(&self) -> BundleId {
        self.bundle_id
    }

    /// Bundle kind.
    #[must_use]
    pub const fn kind(&self) -> BundleKind {
        self.kind
    }

    /// Destination rendezvous tag.
    #[must_use]
    pub const fn dst_tag(&self) -> RendezvousTag {
        self.dst_tag
    }

    /// Maximum lifetime in seconds.
    #[must_use]
    pub const fn lifetime_s(&self) -> u32 {
        self.lifetime_s
    }

    /// Time since creation in milliseconds.
    #[must_use]
    pub const fn age_ms(&self) -> u64 {
        self.age_ms
    }

    /// Remaining hand-offs.
    #[must_use]
    pub const fn hops_remaining(&self) -> u8 {
        self.hops_remaining
    }

    /// Spray-and-wait copy budget.
    #[must_use]
    pub const fn copies(&self) -> u8 {
        self.copies
    }

    /// Proof of work, if present.
    #[must_use]
    pub const fn pow(&self) -> Option<&Pow> {
        self.pow.as_ref()
    }

    /// Encrypted payload.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Purge commitment.
    #[must_use]
    pub const fn purge_commitment(&self) -> &[u8; 16] {
        &self.purge_commitment
    }
}

/// [`Bundle::aad`] computed from its inputs, so a payload can be sealed before the bundle
/// exists. Produces identical bytes to `Bundle::aad()` for the same fields.
#[must_use]
pub fn aad_for(
    bundle_id: &BundleId,
    kind: BundleKind,
    dst_tag: &RendezvousTag,
    purge_commitment: &[u8; 16],
) -> Vec<u8> {
    crate::cbor::encode_with(68, |e| {
        e.map(5)?
            .u64(key::VERSION)?
            .u64(PROTOCOL_VERSION)?
            .u64(key::BUNDLE_ID)?
            .bytes(&bundle_id.0)?
            .u64(key::KIND)?
            .u8(kind.to_wire())?
            .u64(key::DST_TAG)?
            .bytes(&dst_tag.0)?
            .u64(key::PURGE_COMMITMENT)?
            .bytes(purge_commitment)?;
        Ok(())
    })
}

fn wrong_type(key: u64) -> SchemaError {
    SchemaError::WrongType { key }
}

fn expect_key(d: &mut Decoder<'_>, expected: u64) -> Result<(), SchemaError> {
    match d.u64() {
        Ok(k) if k == expected => Ok(()),
        _ => Err(SchemaError::UnexpectedKey { expected }),
    }
}

fn read_array16(d: &mut Decoder<'_>, key: u64) -> Result<[u8; 16], SchemaError> {
    let bytes = d.bytes().map_err(|_| wrong_type(key))?;
    <[u8; 16]>::try_from(bytes).map_err(|_| SchemaError::WrongLength { key })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CborError;
    use alloc::format;
    use alloc::vec;

    fn params() -> BundleParams {
        BundleParams {
            bundle_id: BundleId::from_bytes([0x11; 16]),
            kind: BundleKind::Message,
            dst_tag: RendezvousTag::from_bytes([0x22; 16]),
            lifetime_s: 86_400,
            age_ms: 1_000,
            hops_remaining: 5,
            copies: 8,
            pow: None,
            payload: vec![0xAA; 4],
            purge_commitment: [0x33; 16],
        }
    }

    fn bundle() -> Bundle {
        Bundle::new(params()).unwrap()
    }

    /// Encodes a map from (key, pre-encoded CBOR value) pairs, in the given order.
    fn raw_map(entries: &[(u8, &str)]) -> Vec<u8> {
        let mut out = vec![0xa0 + u8::try_from(entries.len()).unwrap()];
        for (k, v) in entries {
            out.push(*k);
            out.extend(hex::decode(v).unwrap());
        }
        out
    }

    const ID: &str = "5011111111111111111111111111111111";
    const TAG: &str = "5022222222222222222222222222222222";
    const PC: &str = "5033333333333333333333333333333333";

    fn valid_entries() -> Vec<(u8, &'static str)> {
        vec![
            (0, "01"),
            (1, ID),
            (2, "00"),
            (3, TAG),
            (4, "1a00015180"),
            (5, "1903e8"),
            (6, "05"),
            (7, "08"),
            (9, "44aaaaaaaa"),
            (10, PC),
        ]
    }

    #[test]
    fn encoding_matches_hand_written_cbor() {
        assert_eq!(bundle().encode(), raw_map(&valid_entries()));
    }

    #[test]
    fn roundtrip_with_and_without_pow() {
        let b = bundle();
        assert_eq!(Bundle::decode(&b.encode()).unwrap(), b);

        let mut p = params();
        p.kind = BundleKind::Intro;
        p.lifetime_s = MAX_INTRO_LIFETIME_S;
        p.copies = MAX_INTRO_COPIES;
        p.pow = Some(Pow::new([1; 8], 4));
        let b = Bundle::new(p).unwrap();
        assert_eq!(Bundle::decode(&b.encode()).unwrap(), b);
        assert_eq!(
            b.pow().map(|w| (w.nonce(), w.difficulty())),
            Some(([1; 8], 4))
        );
    }

    #[test]
    fn rejects_rule_violations_on_construction() {
        type Case = (fn(&mut BundleParams), InvalidBundle);
        let cases: [Case; 9] = [
            (|p| p.hops_remaining = 6, InvalidBundle::HopLimit(6)),
            (|p| p.copies = 0, InvalidBundle::Copies(0)),
            (|p| p.copies = 65, InvalidBundle::Copies(65)),
            (
                |p| p.lifetime_s = MAX_LIFETIME_S + 1,
                InvalidBundle::Lifetime {
                    got_s: MAX_LIFETIME_S + 1,
                    max_s: MAX_LIFETIME_S,
                },
            ),
            (
                |p| {
                    p.kind = BundleKind::Intro;
                    p.lifetime_s = MAX_INTRO_LIFETIME_S + 1;
                },
                InvalidBundle::Lifetime {
                    got_s: MAX_INTRO_LIFETIME_S + 1,
                    max_s: MAX_INTRO_LIFETIME_S,
                },
            ),
            (|p| p.age_ms = 86_400_000, InvalidBundle::Expired),
            (
                |p| {
                    p.kind = BundleKind::Intro;
                    p.lifetime_s = 60;
                    p.copies = MAX_INTRO_COPIES;
                },
                InvalidBundle::PowMissing,
            ),
            (
                |p| {
                    p.kind = BundleKind::Intro;
                    p.lifetime_s = 60;
                    p.copies = MAX_INTRO_COPIES + 1;
                    p.pow = Some(Pow::new([0; 8], 4));
                },
                InvalidBundle::Copies(MAX_INTRO_COPIES + 1),
            ),
            (
                |p| p.pow = Some(Pow::new([0; 8], 4)),
                InvalidBundle::PowNotAllowed,
            ),
        ];
        for (mutate, expected) in cases {
            let mut p = params();
            mutate(&mut p);
            assert_eq!(Bundle::new(p), Err(ProtoError::Invalid(expected)));
        }
    }

    #[test]
    fn rejects_oversized_bundle_on_construction() {
        let mut p = params();
        p.payload = vec![0; MAX_BUNDLE_LEN];
        assert!(matches!(
            Bundle::new(p),
            Err(ProtoError::Invalid(InvalidBundle::TooLarge { .. }))
        ));
    }

    #[test]
    fn rejects_bad_pow_length() {
        assert_eq!(Pow::from_bytes(&[]), Err(InvalidBundle::PowLength(0)));
        assert_eq!(Pow::from_bytes(&[0; 10]), Err(InvalidBundle::PowLength(10)));
        assert!(Pow::from_bytes(&[0; POW_LEN]).is_ok());
    }

    #[test]
    fn rejects_rule_violations_on_decode() {
        let mut e = valid_entries();
        e[6] = (6, "06");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(InvalidBundle::HopLimit(6).into())
        );

        let mut e = valid_entries();
        e[0] = (0, "02");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(InvalidBundle::UnsupportedVersion(2).into())
        );
    }

    #[test]
    fn rejects_schema_violations() {
        // Unknown kind.
        let mut e = valid_entries();
        e[2] = (2, "05");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(SchemaError::UnknownValue { key: 2 }.into())
        );
        // Short bundle id.
        let mut e = valid_entries();
        e[1] = (1, "4111");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(SchemaError::WrongLength { key: 1 }.into())
        );
        // Wrong type: text instead of bytes for the tag.
        let mut e = valid_entries();
        e[3] = (3, "6161");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(SchemaError::WrongType { key: 3 }.into())
        );
        // copies does not fit in u8.
        let mut e = valid_entries();
        e[7] = (7, "190100");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(SchemaError::WrongType { key: 7 }.into())
        );
        // Not a map.
        assert_eq!(Bundle::decode(&[0x80]), Err(SchemaError::MapShape.into()));
        // Missing field.
        let mut e = valid_entries();
        e.remove(5);
        e.push((10, "00"));
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(SchemaError::UnexpectedKey { expected: 5 }.into())
        );
    }

    #[test]
    fn rejects_non_canonical_key_order_duplicates_and_unknown_keys() {
        let mut swapped = valid_entries();
        swapped.swap(4, 5);
        assert_eq!(
            Bundle::decode(&raw_map(&swapped)),
            Err(SchemaError::UnexpectedKey { expected: 4 }.into())
        );

        let mut duplicate = valid_entries();
        duplicate[5] = (4, "1a00015180");
        assert_eq!(
            Bundle::decode(&raw_map(&duplicate)),
            Err(SchemaError::UnexpectedKey { expected: 5 }.into())
        );

        let mut unknown = valid_entries();
        unknown.insert(8, (10, "00"));
        assert_eq!(
            Bundle::decode(&raw_map(&unknown)),
            Err(SchemaError::UnexpectedKey { expected: 8 }.into())
        );
    }

    #[test]
    fn rejects_non_minimal_integers() {
        let mut e = valid_entries();
        e[6] = (6, "1805");
        assert_eq!(
            Bundle::decode(&raw_map(&e)),
            Err(CborError::NonMinimal.into())
        );
    }

    #[test]
    fn enforces_size_cap_before_parsing() {
        let input = vec![0xff; 100];
        assert_eq!(
            Bundle::decode_with_max_len(&input, 64),
            Err(ProtoError::InputTooLarge { len: 100, max: 64 })
        );
        // A caller cannot raise the cap above the protocol maximum.
        let input = vec![0xff; MAX_BUNDLE_LEN + 1];
        assert_eq!(
            Bundle::decode_with_max_len(&input, usize::MAX),
            Err(ProtoError::InputTooLarge {
                len: MAX_BUNDLE_LEN + 1,
                max: MAX_BUNDLE_LEN
            })
        );
    }

    #[test]
    fn aad_covers_fields_0_to_3_and_10_only() {
        let b = bundle();
        let expected = raw_map(&[(0, "01"), (1, ID), (2, "00"), (3, TAG), (10, PC)]);
        assert_eq!(b.aad(), expected);
        assert_eq!(
            aad_for(&b.bundle_id(), b.kind(), &b.dst_tag(), b.purge_commitment()),
            expected
        );

        // Mutable per-hop fields are excluded.
        let mut p = params();
        p.age_ms = 2_000;
        p.hops_remaining = 1;
        p.copies = 1;
        assert_eq!(Bundle::new(p).unwrap().aad(), expected);

        // The purge commitment is bound: changing it changes the AAD.
        let mut p = params();
        p.purge_commitment[0] ^= 1;
        assert_ne!(Bundle::new(p).unwrap().aad(), expected);
    }

    #[test]
    fn debug_output_redacts_identifiers() {
        let s = format!("{:?}", bundle());
        assert!(s.contains("BundleId(1111…)"), "{s}");
        assert!(s.contains("RendezvousTag(<redacted>)"), "{s}");
        assert!(!s.contains("22"), "{s}");
        assert!(!s.contains("aa") && !s.contains("170"), "{s}");
    }
}
