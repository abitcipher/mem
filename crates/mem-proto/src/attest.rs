//! In-person attestation for a relying party (`spec/attest.cddl`).
//!
//! Four objects, each a single-frame text QR or pasted text:
//!
//! - [`AttestRequest`] (`memattreq1:`): what the relying party asks for. Unsigned and untrusted;
//!   its origin and submit URL are checked here, the rest is shown to the user.
//! - [`AttestOffer`] (`memattest1:`): what one phone shows the other.
//! - [`AttestStatement`]: what both phones sign, built from the two offers.
//! - [`Attestation`] (`memattsig1:`): one phone's signed statement, returned to the relying party.
//!
//! Attestations are *not* deniable, unlike everything else MEM sends. Nothing here is used by
//! messaging, encounters or pairing.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use data_encoding::BASE64URL_NOPAD;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Text prefix of a relying party's request.
pub const REQUEST_PREFIX: &str = "memattreq1:";
/// Text prefix of a phone's offer.
pub const OFFER_PREFIX: &str = "memattest1:";
/// Text prefix of an attestation (the copy/paste return path).
pub const ATTESTATION_PREFIX: &str = "memattsig1:";

/// Longest relying-party origin, `scheme://host[:port]`.
pub const MAX_ORIGIN_LEN: usize = 270;
/// Longest submit URL.
pub const MAX_SUBMIT_URL_LEN: usize = 512;
/// Longest subject label.
pub const MAX_LABEL_LEN: usize = 32;
/// Longest role name.
pub const MAX_ROLE_LEN: usize = 16;

const MAX_HOST_LEN: usize = 253;
/// Largest encoded request (all text fields at their maximum, with CBOR headers).
pub const MAX_REQUEST_LEN: usize = 1024;
/// Largest encoded offer.
pub const MAX_OFFER_LEN: usize = 128;
/// Largest encoded statement.
pub const MAX_STATEMENT_LEN: usize = 512;
/// Largest encoded attestation.
pub const MAX_ATTESTATION_LEN: usize = 640;

/// A relying party's request.
#[derive(Clone, PartialEq, Eq)]
pub struct AttestRequest {
    /// `https://host[:port]` (or `http://` for loopback hosts only).
    pub rp_origin: String,
    /// The hash being attested.
    pub subject: [u8; 32],
    /// What the relying party calls the subject; shown to the user, untrusted.
    pub subject_label: String,
    /// This user's role, lowercase letters and `_`.
    pub role: String,
    /// Fresh, single-use challenge from the relying party.
    pub rp_challenge: [u8; 16],
    /// Where the attestation is sent; on `rp_origin`.
    pub submit_url: String,
}

impl fmt::Debug for AttestRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttestRequest")
            .field("rp_origin", &self.rp_origin)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl AttestRequest {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] if a field breaks the rules in [`AttestRequest::check`].
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(256, |e| {
            e.map(7)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .str(&self.rp_origin)?
                .u8(2)?
                .bytes(&self.subject)?
                .u8(3)?
                .str(&self.subject_label)?
                .u8(4)?
                .str(&self.role)?
                .u8(5)?
                .bytes(&self.rp_challenge)?
                .u8(6)?
                .str(&self.submit_url)?;
            Ok(())
        }))
    }

    /// Decodes and checks an untrusted request.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or invalid input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_REQUEST_LEN,
            max_container_len: 7,
        };
        let mut r = MapReader::new(input, &limits)?;
        version(&mut r)?;
        let req = Self {
            rp_origin: String::from(r.text(1)?),
            subject: r.array::<32>(2)?,
            subject_label: String::from(r.text(3)?),
            role: String::from(r.text(4)?),
            rp_challenge: r.array::<16>(5)?,
            submit_url: String::from(r.text(6)?),
        };
        r.finish()?;
        req.check()?;
        Ok(req)
    }

    /// The rules for a request: a valid origin, a submit URL on that origin, a short
    /// printable label and a well-formed role.
    ///
    /// # Errors
    ///
    /// [`SchemaError::UnknownValue`] naming the offending key.
    pub fn check(&self) -> Result<(), SchemaError> {
        check_origin(&self.rp_origin).map_err(|()| SchemaError::UnknownValue { key: 1 })?;
        check_label(&self.subject_label).map_err(|()| SchemaError::UnknownValue { key: 3 })?;
        check_role(&self.role).map_err(|()| SchemaError::UnknownValue { key: 4 })?;
        check_submit_url(&self.rp_origin, &self.submit_url)
            .map_err(|()| SchemaError::UnknownValue { key: 6 })
    }
}

/// One phone's offer.
#[derive(Clone, PartialEq, Eq)]
pub struct AttestOffer {
    /// This phone's attestation key for the relying party (Ed25519).
    pub attest_pk: [u8; 32],
    /// The subject, as in this phone's request.
    pub subject: [u8; 32],
    /// This user's role.
    pub role: String,
    /// Fresh random nonce.
    pub nonce: [u8; 16],
}

impl fmt::Debug for AttestOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttestOffer")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl AttestOffer {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for an invalid role.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        check_role(&self.role).map_err(|()| SchemaError::UnknownValue { key: 3 })?;
        Ok(encode_with(MAX_OFFER_LEN, |e| {
            e.map(5)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&self.attest_pk)?
                .u8(2)?
                .bytes(&self.subject)?
                .u8(3)?
                .str(&self.role)?
                .u8(4)?
                .bytes(&self.nonce)?;
            Ok(())
        }))
    }

    /// Decodes an untrusted offer.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or invalid input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_OFFER_LEN,
            max_container_len: 5,
        };
        let mut r = MapReader::new(input, &limits)?;
        version(&mut r)?;
        let offer = Self {
            attest_pk: r.array::<32>(1)?,
            subject: r.array::<32>(2)?,
            role: String::from(r.text(3)?),
            nonce: r.array::<16>(4)?,
        };
        r.finish()?;
        check_role(&offer.role).map_err(|()| SchemaError::UnknownValue { key: 3 })?;
        Ok(offer)
    }
}

/// One side of a statement.
#[derive(Clone, PartialEq, Eq)]
pub struct Attester {
    /// Attestation key (Ed25519).
    pub attest_pk: [u8; 32],
    /// Role.
    pub role: String,
    /// Nonce from this attester's offer.
    pub nonce: [u8; 16],
}

impl fmt::Debug for Attester {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attester")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// Why two offers cannot form a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OfferMismatch {
    /// The offers name different subjects.
    #[error("the other phone confirms something else")]
    Subject,
    /// Both offers have the same role.
    #[error("both phones have the same role")]
    Role,
    /// Both offers have the same key (a phone scanned its own code).
    #[error("this phone's own code")]
    SameKey,
}

/// What both phones sign. `lo` has the smaller `attest_pk`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AttestStatement {
    /// The relying party's origin.
    pub rp_origin: String,
    /// The subject.
    pub subject: [u8; 32],
    /// The attester with the smaller key.
    pub lo: Attester,
    /// The attester with the larger key.
    pub hi: Attester,
}

impl AttestStatement {
    /// Builds the statement from this phone's offer and the scanned one, in either order.
    ///
    /// # Errors
    ///
    /// [`OfferMismatch`] if the offers do not belong together.
    pub fn from_offers(
        rp_origin: &str,
        own: &AttestOffer,
        peer: &AttestOffer,
    ) -> Result<Self, OfferMismatch> {
        if own.subject != peer.subject {
            return Err(OfferMismatch::Subject);
        }
        if own.attest_pk == peer.attest_pk {
            return Err(OfferMismatch::SameKey);
        }
        if own.role == peer.role {
            return Err(OfferMismatch::Role);
        }
        let side = |o: &AttestOffer| Attester {
            attest_pk: o.attest_pk,
            role: o.role.clone(),
            nonce: o.nonce,
        };
        let (lo, hi) = if own.attest_pk < peer.attest_pk {
            (side(own), side(peer))
        } else {
            (side(peer), side(own))
        };
        Ok(Self {
            rp_origin: String::from(rp_origin),
            subject: own.subject,
            lo,
            hi,
        })
    }

    /// Canonical CBOR, the bytes that are signed.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for an invalid origin or roles.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(MAX_STATEMENT_LEN, |e| {
            e.map(9)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .str(&self.rp_origin)?
                .u8(2)?
                .bytes(&self.subject)?
                .u8(3)?
                .bytes(&self.lo.attest_pk)?
                .u8(4)?
                .str(&self.lo.role)?
                .u8(5)?
                .bytes(&self.lo.nonce)?
                .u8(6)?
                .bytes(&self.hi.attest_pk)?
                .u8(7)?
                .str(&self.hi.role)?
                .u8(8)?
                .bytes(&self.hi.nonce)?;
            Ok(())
        }))
    }

    /// Decodes an untrusted statement.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or invalid input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_STATEMENT_LEN,
            max_container_len: 9,
        };
        let mut r = MapReader::new(input, &limits)?;
        version(&mut r)?;
        let rp_origin = String::from(r.text(1)?);
        let subject = r.array::<32>(2)?;
        let lo = Attester {
            attest_pk: r.array::<32>(3)?,
            role: String::from(r.text(4)?),
            nonce: r.array::<16>(5)?,
        };
        let hi = Attester {
            attest_pk: r.array::<32>(6)?,
            role: String::from(r.text(7)?),
            nonce: r.array::<16>(8)?,
        };
        r.finish()?;
        let s = Self {
            rp_origin,
            subject,
            lo,
            hi,
        };
        s.check()?;
        Ok(s)
    }

    /// The attester with this key, if it is one of the two.
    #[must_use]
    pub fn attester(&self, attest_pk: &[u8; 32]) -> Option<&Attester> {
        [&self.lo, &self.hi]
            .into_iter()
            .find(|a| a.attest_pk == *attest_pk)
    }

    fn check(&self) -> Result<(), SchemaError> {
        check_origin(&self.rp_origin).map_err(|()| SchemaError::UnknownValue { key: 1 })?;
        check_role(&self.lo.role).map_err(|()| SchemaError::UnknownValue { key: 4 })?;
        check_role(&self.hi.role).map_err(|()| SchemaError::UnknownValue { key: 7 })?;
        // lo < hi strictly: one canonical order, two distinct keys, two distinct roles
        if self.lo.attest_pk >= self.hi.attest_pk {
            return Err(SchemaError::UnknownValue { key: 6 });
        }
        if self.lo.role == self.hi.role {
            return Err(SchemaError::UnknownValue { key: 7 });
        }
        Ok(())
    }
}

/// One phone's signed statement.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Attestation {
    /// The statement.
    pub statement: AttestStatement,
    /// The challenge from this phone's request.
    pub rp_challenge: [u8; 16],
    /// The signer's key, one of the statement's two.
    pub attest_pk: [u8; 32],
    /// Ed25519 signature over `"MEM/v1/attestation" || rp_challenge || statement bytes`.
    pub signature: [u8; 64],
}

impl Attestation {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for an invalid statement or a signer not named in it.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        let statement = self.statement.encode()?;
        if self.statement.attester(&self.attest_pk).is_none() {
            return Err(SchemaError::UnknownValue { key: 3 }.into());
        }
        Ok(encode_with(MAX_ATTESTATION_LEN, |e| {
            e.map(5)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&statement)?
                .u8(2)?
                .bytes(&self.rp_challenge)?
                .u8(3)?
                .bytes(&self.attest_pk)?
                .u8(4)?
                .bytes(&self.signature)?;
            Ok(())
        }))
    }

    /// Decodes an untrusted attestation. The signature is *not* verified here.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or invalid input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_ATTESTATION_LEN,
            max_container_len: 5,
        };
        let mut r = MapReader::new(input, &limits)?;
        version(&mut r)?;
        let statement = AttestStatement::decode(r.bytes(1)?)?;
        let rp_challenge = r.array::<16>(2)?;
        let attest_pk = r.array::<32>(3)?;
        let signature = r.array::<64>(4)?;
        r.finish()?;
        if statement.attester(&attest_pk).is_none() {
            return Err(SchemaError::UnknownValue { key: 3 }.into());
        }
        Ok(Self {
            statement,
            rp_challenge,
            attest_pk,
            signature,
        })
    }
}

/// An attestation object recognised in scanned or pasted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestText {
    /// `memattreq1:`
    Request(Vec<u8>),
    /// `memattest1:`
    Offer(Vec<u8>),
    /// `memattsig1:`
    Attestation(Vec<u8>),
}

/// The text form of an object: prefix + base64url without padding.
#[must_use]
pub fn armor(prefix: &str, data: &[u8]) -> String {
    let mut out = String::with_capacity(prefix.len() + data.len() * 4 / 3 + 4);
    out.push_str(prefix);
    out.push_str(&BASE64URL_NOPAD.encode(data));
    out
}

/// Finds an attestation object in text, as [`crate::armor::dearmor`] does for bundles and cards.
/// `None` if the text holds none (it may then be another MEM object).
///
/// # Errors
///
/// [`ProtoError`] if a prefix is found but the payload is too long or not base64url.
pub fn dearmor(text: &str) -> Result<Option<AttestText>, ProtoError> {
    let max_chars = MAX_REQUEST_LEN * 4 / 3 + 4;
    for prefix in [REQUEST_PREFIX, OFFER_PREFIX, ATTESTATION_PREFIX] {
        let Some(start) = text.find(prefix) else {
            continue;
        };
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
        return Ok(Some(match prefix {
            REQUEST_PREFIX => AttestText::Request(data),
            OFFER_PREFIX => AttestText::Offer(data),
            _ => AttestText::Attestation(data),
        }));
    }
    Ok(None)
}

/// The first 8 characters of the subject in lowercase base32, shown next to the label so the
/// user can compare it with the relying party's page.
#[must_use]
pub fn subject_short(subject: &[u8; 32]) -> String {
    let mut s = data_encoding::BASE32_NOPAD.encode(&subject[..5]);
    s.make_ascii_lowercase();
    s
}

fn version(r: &mut MapReader<'_>) -> Result<(), ProtoError> {
    if r.u8(0)? == 1 {
        Ok(())
    } else {
        Err(SchemaError::UnknownValue { key: 0 }.into())
    }
}

/// `https://host[:port]`, lowercase host of letters, digits, `.` and `-`; `http://` only for
/// `localhost` and `127.0.0.1`. No path, user info or trailing dot.
fn check_origin(origin: &str) -> Result<(), ()> {
    if origin.len() > MAX_ORIGIN_LEN {
        return Err(());
    }
    let (secure, rest) = if let Some(r) = origin.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = origin.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(());
    };
    let (host, port) = match rest.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    let host_ok = !host.is_empty()
        && host.len() <= MAX_HOST_LEN
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && !host.contains("..");
    let port_ok = port.is_none_or(|p| {
        !p.is_empty()
            && p.len() <= 5
            && !p.starts_with('0')
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().is_ok()
    });
    let scheme_ok = secure || host == "localhost" || host == "127.0.0.1";
    if host_ok && port_ok && scheme_ok {
        Ok(())
    } else {
        Err(())
    }
}

/// The URL must be `origin + "/" + path`, printable ASCII without spaces or backslashes.
fn check_submit_url(origin: &str, url: &str) -> Result<(), ()> {
    let path = url
        .strip_prefix(origin)
        .and_then(|r| r.strip_prefix('/'))
        .ok_or(())?;
    let ok =
        url.len() <= MAX_SUBMIT_URL_LEN && path.bytes().all(|b| b.is_ascii_graphic() && b != b'\\');
    if ok { Ok(()) } else { Err(()) }
}

fn check_label(label: &str) -> Result<(), ()> {
    let ok = !label.is_empty()
        && label.len() <= MAX_LABEL_LEN
        && label.bytes().all(|b| b == b' ' || b.is_ascii_graphic());
    if ok { Ok(()) } else { Err(()) }
}

fn check_role(role: &str) -> Result<(), ()> {
    let ok = !role.is_empty()
        && role.len() <= MAX_ROLE_LEN
        && role.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
    if ok { Ok(()) } else { Err(()) }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use alloc::format;

    pub(crate) fn request() -> AttestRequest {
        AttestRequest {
            rp_origin: String::from("https://basa.ua"),
            subject: [7; 32],
            subject_label: String::from("basa0:abcdefgh"),
            role: String::from("offeror"),
            rp_challenge: [5; 16],
            submit_url: String::from("https://basa.ua/mem/attest/abc"),
        }
    }

    fn offer(pk: u8, role: &str) -> AttestOffer {
        AttestOffer {
            attest_pk: [pk; 32],
            subject: [7; 32],
            role: String::from(role),
            nonce: [pk; 16],
        }
    }

    #[test]
    fn request_round_trip_and_text_form() {
        let r = request();
        let b = r.encode().unwrap();
        assert_eq!(AttestRequest::decode(&b).unwrap(), r);
        let text = armor(REQUEST_PREFIX, &b);
        assert_eq!(
            dearmor(&format!("scan this: {text} thanks")).unwrap(),
            Some(AttestText::Request(b))
        );
        assert_eq!(dearmor("memcard0:AAAA").unwrap(), None);
    }

    #[test]
    fn origins_and_submit_urls() {
        let ok = |o: &str, u: &str| {
            let mut r = request();
            r.rp_origin = String::from(o);
            r.submit_url = String::from(u);
            r.check().is_ok()
        };
        assert!(ok("https://basa.ua", "https://basa.ua/x"));
        assert!(ok("https://basa.ua:8443", "https://basa.ua:8443/x?y=1"));
        assert!(ok("http://localhost:4000", "http://localhost:4000/mem"));
        assert!(ok("http://127.0.0.1:4000", "http://127.0.0.1:4000/mem"));
        // plain http elsewhere, other hosts, user info, paths in the origin, odd ports
        assert!(!ok("http://basa.ua", "http://basa.ua/x"));
        assert!(!ok("https://basa.ua", "https://basa.ua.evil.com/x"));
        assert!(!ok("https://basa.ua", "https://basa.ua@evil.com/x"));
        assert!(!ok("https://basa.ua", "https://basa.ua"));
        assert!(!ok("https://basa.ua", "https://basa.ua/a b"));
        assert!(!ok("https://basa.ua", "https://basa.ua/\\evil"));
        assert!(!ok("https://Basa.ua", "https://Basa.ua/x"));
        assert!(!ok("https://basa.ua/", "https://basa.ua//x"));
        assert!(!ok("https://basa.ua.", "https://basa.ua./x"));
        assert!(!ok("https://basa.ua:0", "https://basa.ua:0/x"));
        assert!(!ok("https://basa.ua:99999", "https://basa.ua:99999/x"));
        assert!(!ok("https://", "https:///x"));
    }

    #[test]
    fn labels_and_roles() {
        let mut r = request();
        r.subject_label = String::from("line\nbreak");
        assert!(r.encode().is_err());
        let mut r = request();
        r.role = String::from("Offeror");
        assert!(r.encode().is_err());
        let mut o = offer(1, "offeror");
        o.role = String::new();
        assert!(o.encode().is_err());
    }

    #[test]
    fn statement_is_the_same_on_both_phones() {
        let a = offer(1, "offeror");
        let b = offer(2, "acceptor");
        let s1 = AttestStatement::from_offers("https://basa.ua", &a, &b).unwrap();
        let s2 = AttestStatement::from_offers("https://basa.ua", &b, &a).unwrap();
        assert_eq!(s1, s2);
        assert_eq!(s1.lo.role, "offeror");
        let bytes = s1.encode().unwrap();
        assert_eq!(AttestStatement::decode(&bytes).unwrap(), s1);
        assert!(bytes.len() <= MAX_STATEMENT_LEN);
    }

    fn from(x: &AttestOffer, y: &AttestOffer) -> Result<AttestStatement, OfferMismatch> {
        AttestStatement::from_offers("https://basa.ua", x, y)
    }

    #[test]
    fn offers_that_do_not_belong_together() {
        let a = offer(1, "offeror");
        let mut other = offer(2, "acceptor");
        other.subject = [8; 32];
        let same_role = offer(2, "offeror");
        assert_eq!(from(&a, &other), Err(OfferMismatch::Subject));
        assert_eq!(from(&a, &same_role), Err(OfferMismatch::Role));
        assert_eq!(from(&a, &a), Err(OfferMismatch::SameKey));
    }

    #[test]
    fn statement_decoder_rejects_wrong_order() {
        let a = offer(1, "offeror");
        let b = offer(2, "acceptor");
        let mut s = AttestStatement::from_offers("https://basa.ua", &a, &b).unwrap();
        core::mem::swap(&mut s.lo, &mut s.hi);
        assert!(s.encode().is_err());
    }

    #[test]
    fn attestation_round_trip_and_signer_check() {
        let s = AttestStatement::from_offers(
            "https://basa.ua",
            &offer(1, "offeror"),
            &offer(2, "acceptor"),
        )
        .unwrap();
        let att = Attestation {
            statement: s,
            rp_challenge: [5; 16],
            attest_pk: [2; 32],
            signature: [9; 64],
        };
        let b = att.encode().unwrap();
        assert!(b.len() <= MAX_ATTESTATION_LEN);
        assert_eq!(Attestation::decode(&b).unwrap(), att);
        let mut stranger = att;
        stranger.attest_pk = [3; 32];
        assert!(stranger.encode().is_err());
    }

    #[test]
    fn offers_fit_one_small_qr() {
        let b = offer(1, "acceptor").encode().unwrap();
        assert!(b.len() <= MAX_OFFER_LEN);
        assert_eq!(AttestOffer::decode(&b).unwrap(), offer(1, "acceptor"));
        let mut long = b;
        long.push(0);
        assert!(AttestOffer::decode(&long).is_err());
    }
}
