//! The in-person attestation session, sans-IO.
//!
//! The app drives it: [`AttestSession::start`] with the scanned request (after the user saw
//! the disclosure), show [`AttestSession::offer_text`], feed the other phone's offer to
//! [`AttestSession::accept_peer`], show the returned emoji, and only after the users tapped
//! "They match", [`AttestSession::sign`]. Sending the result is the app's job (HTTPS POST to
//! the request's submit URL, or the `memattsig1:` text).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use mem_crypto::IdentityKey;
use mem_crypto::attest::{self as keys, AttestKey, SAS_LEN};
use mem_proto::ProtoError;
use mem_proto::attest::{
    self as wire, ATTESTATION_PREFIX, AttestOffer, AttestRequest, AttestStatement, AttestText,
    Attestation, OFFER_PREFIX, OfferMismatch,
};
use rand_core::CryptoRng;

/// Why an attestation step failed. Carries no keys, nonces or subjects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AttestError {
    /// The scanned text is not the expected kind of object.
    #[error("not an attestation code of the expected kind")]
    WrongObject,
    /// The object is malformed or breaks a rule (origin, submit URL, role…).
    #[error("invalid attestation object: {0}")]
    Invalid(#[from] ProtoError),
    /// The other phone's offer does not belong to this one.
    #[error("{0}")]
    Mismatch(#[from] OfferMismatch),
    /// Signing was asked for before the other phone's offer was scanned.
    #[error("the other phone's code has not been scanned yet")]
    NoPeer,
    /// A signature does not verify.
    #[error("the signature does not verify")]
    BadSignature,
    /// Only the main device holds the identity, so only it attests (v1).
    #[error("only the main device can confirm in person")]
    NotMainDevice,
}

/// One phone's side of an attestation. Holds the derived key until dropped.
pub struct AttestSession {
    request: AttestRequest,
    key: AttestKey,
    offer: AttestOffer,
    statement: Option<(AttestStatement, Vec<u8>)>,
}

impl fmt::Debug for AttestSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttestSession")
            .field("request", &self.request)
            .field("peer", &self.statement.is_some())
            .finish_non_exhaustive()
    }
}

impl AttestSession {
    /// Starts from the relying party's request (text from the scanner or the clipboard).
    ///
    /// # Errors
    ///
    /// [`AttestError::WrongObject`] if the text holds no request, [`AttestError::Invalid`] if
    /// the request breaks the rules.
    pub fn start(
        identity: &IdentityKey,
        request_text: &str,
        rng: &mut impl CryptoRng,
    ) -> Result<Self, AttestError> {
        let Some(AttestText::Request(bytes)) = wire::dearmor(request_text)? else {
            return Err(AttestError::WrongObject);
        };
        let request = AttestRequest::decode(&bytes)?;
        let key = AttestKey::derive(identity, &request.rp_origin)
            .map_err(|_| AttestError::WrongObject)?;
        let mut nonce = [0u8; 16];
        rng.fill_bytes(&mut nonce);
        let offer = AttestOffer {
            attest_pk: key.public(),
            subject: request.subject,
            role: request.role.clone(),
            nonce,
        };
        Ok(Self {
            request,
            key,
            offer,
            statement: None,
        })
    }

    /// The request, for the disclosure and confirmation screens.
    #[must_use]
    pub fn request(&self) -> &AttestRequest {
        &self.request
    }

    /// This phone's offer as `memattest1:` text, for a single QR code.
    ///
    /// # Errors
    ///
    /// [`AttestError::Invalid`] only if the request's role was invalid (already checked).
    pub fn offer_text(&self) -> Result<String, AttestError> {
        Ok(wire::armor(OFFER_PREFIX, &self.offer.encode()?))
    }

    /// Takes the other phone's offer and returns the emoji indexes to compare.
    ///
    /// # Errors
    ///
    /// [`AttestError::WrongObject`], [`AttestError::Invalid`] or [`AttestError::Mismatch`].
    pub fn accept_peer(&mut self, offer_text: &str) -> Result<[u8; SAS_LEN], AttestError> {
        let Some(AttestText::Offer(bytes)) = wire::dearmor(offer_text)? else {
            return Err(AttestError::WrongObject);
        };
        let peer = AttestOffer::decode(&bytes)?;
        let statement = AttestStatement::from_offers(&self.request.rp_origin, &self.offer, &peer)?;
        let encoded = statement.encode()?;
        let sas = keys::sas(&encoded);
        self.statement = Some((statement, encoded));
        Ok(sas)
    }

    /// Signs, after the users confirmed the emoji match. Returns the attestation's CBOR (the
    /// HTTPS body) and consumes the session, so the key is dropped.
    ///
    /// # Errors
    ///
    /// [`AttestError::NoPeer`] before [`AttestSession::accept_peer`] succeeded.
    pub fn sign(self) -> Result<Vec<u8>, AttestError> {
        let Some((statement, encoded)) = self.statement else {
            return Err(AttestError::NoPeer);
        };
        let attestation = Attestation {
            signature: self.key.sign(&self.request.rp_challenge, &encoded),
            statement,
            rp_challenge: self.request.rp_challenge,
            attest_pk: self.offer.attest_pk,
        };
        Ok(attestation.encode()?)
    }
}

/// The `memattsig1:` text of an attestation (the copy/paste return path).
#[must_use]
pub fn attestation_text(attestation: &[u8]) -> String {
    wire::armor(ATTESTATION_PREFIX, attestation)
}

/// Decodes an attestation and checks its signature (what a relying party does).
///
/// # Errors
///
/// [`AttestError::Invalid`] or [`AttestError::BadSignature`].
pub fn verify_attestation(bytes: &[u8]) -> Result<Attestation, AttestError> {
    let a = Attestation::decode(bytes)?;
    let statement = a.statement.encode()?;
    keys::verify(&a.attest_pk, &a.rp_challenge, &statement, &a.signature)
        .map_err(|_| AttestError::BadSignature)?;
    Ok(a)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use mem_proto::attest::REQUEST_PREFIX;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn request_text(role: &str, challenge: u8) -> String {
        let r = AttestRequest {
            rp_origin: String::from("https://basa.ua"),
            subject: [7; 32],
            subject_label: String::from("basa0:abcdefgh"),
            role: String::from(role),
            rp_challenge: [challenge; 16],
            submit_url: String::from("https://basa.ua/mem/attest"),
        };
        wire::armor(REQUEST_PREFIX, &r.encode().unwrap())
    }

    #[test]
    fn two_phones_attest_together() {
        let mut rng = ChaCha20Rng::from_seed([1; 32]);
        let (olena, max) = (
            IdentityKey::from_seed(&[1; 32]),
            IdentityKey::from_seed(&[2; 32]),
        );
        let mut a = AttestSession::start(&olena, &request_text("offeror", 10), &mut rng).unwrap();
        let mut b = AttestSession::start(&max, &request_text("acceptor", 11), &mut rng).unwrap();

        let (offer_a, offer_b) = (a.offer_text().unwrap(), b.offer_text().unwrap());
        assert!(offer_a.len() < 200);
        let sas_b = b.accept_peer(&offer_a).unwrap();
        let sas_a = a.accept_peer(&offer_b).unwrap();
        assert_eq!(sas_a, sas_b);

        let att_a = verify_attestation(&a.sign().unwrap()).unwrap();
        let att_b = verify_attestation(&b.sign().unwrap()).unwrap();
        assert_eq!(att_a.statement, att_b.statement);
        assert_eq!(att_a.rp_challenge, [10; 16]);
        assert_eq!(att_b.rp_challenge, [11; 16]);
        assert_ne!(att_a.attest_pk, att_b.attest_pk);
    }

    #[test]
    fn refuses_wrong_objects_own_code_and_early_signing() {
        let mut rng = ChaCha20Rng::from_seed([2; 32]);
        let id = IdentityKey::from_seed(&[1; 32]);
        assert_eq!(
            AttestSession::start(&id, "memcard1:AAAA", &mut rng).unwrap_err(),
            AttestError::WrongObject
        );
        let mut s = AttestSession::start(&id, &request_text("offeror", 1), &mut rng).unwrap();
        let own = s.offer_text().unwrap();
        assert_eq!(
            s.accept_peer(&request_text("acceptor", 1)).unwrap_err(),
            AttestError::WrongObject
        );
        assert_eq!(
            s.accept_peer(&own).unwrap_err(),
            AttestError::Mismatch(OfferMismatch::SameKey)
        );
        assert_eq!(s.sign().unwrap_err(), AttestError::NoPeer);
    }

    #[test]
    fn tampered_attestation_fails() {
        let mut rng = ChaCha20Rng::from_seed([3; 32]);
        let mut a = AttestSession::start(
            &IdentityKey::from_seed(&[1; 32]),
            &request_text("offeror", 1),
            &mut rng,
        )
        .unwrap();
        let b = AttestSession::start(
            &IdentityKey::from_seed(&[2; 32]),
            &request_text("acceptor", 2),
            &mut rng,
        )
        .unwrap();
        a.accept_peer(&b.offer_text().unwrap()).unwrap();
        let mut att = Attestation::decode(&a.sign().unwrap()).unwrap();
        att.rp_challenge = [2; 16];
        assert_eq!(
            verify_attestation(&att.encode().unwrap()).unwrap_err(),
            AttestError::BadSignature
        );
    }

    /// Published vectors (`spec/vectors/attest/`), shared with BASA. Regenerate with
    /// `MEM_UPDATE_VECTORS=1`. The identity seeds are test-only secrets.
    #[test]
    fn published_vectors() {
        extern crate std;
        use alloc::format;

        fn hex(b: &[u8]) -> String {
            use core::fmt::Write;
            b.iter().fold(String::new(), |mut s, x| {
                let _ = write!(s, "{x:02x}");
                s
            })
        }

        let mut rng = ChaCha20Rng::from_seed([32; 32]);
        let (seed_a, seed_b) = ([0xa1; 32], [0xb2; 32]);
        let (req_a, req_b) = (
            request_text("offeror", 0x11),
            request_text("acceptor", 0x22),
        );
        let mut a =
            AttestSession::start(&IdentityKey::from_seed(&seed_a), &req_a, &mut rng).unwrap();
        let mut b =
            AttestSession::start(&IdentityKey::from_seed(&seed_b), &req_b, &mut rng).unwrap();
        let (offer_a, offer_b) = (a.offer_text().unwrap(), b.offer_text().unwrap());
        let sas = a.accept_peer(&offer_b).unwrap();
        assert_eq!(b.accept_peer(&offer_a).unwrap(), sas);
        let statement = a.statement.as_ref().unwrap().1.clone();
        let (pk_a, pk_b) = (a.offer.attest_pk, b.offer.attest_pk);
        let (att_a, att_b) = (a.sign().unwrap(), b.sign().unwrap());

        let json = format!(
            "{{\n  \"rp_origin\": \"https://basa.ua\",\n  \"identity_seed_a\": \"{}\",\n  \"identity_seed_b\": \"{}\",\n  \"attest_pk_a\": \"{}\",\n  \"attest_pk_b\": \"{}\",\n  \"request_a\": \"{req_a}\",\n  \"request_b\": \"{req_b}\",\n  \"offer_a\": \"{offer_a}\",\n  \"offer_b\": \"{offer_b}\",\n  \"statement\": \"{}\",\n  \"sas\": {:?},\n  \"attestation_a\": \"{}\",\n  \"attestation_b\": \"{}\",\n  \"attestation_a_text\": \"{}\"\n}}\n",
            hex(&seed_a),
            hex(&seed_b),
            hex(&pk_a),
            hex(&pk_b),
            hex(&statement),
            sas,
            hex(&att_a),
            hex(&att_b),
            attestation_text(&att_a),
        );
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/attest");
        if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("attest.json"), &json).unwrap();
            std::fs::write(dir.join("statement.cbor"), &statement).unwrap();
            std::fs::write(dir.join("attestation-a.cbor"), &att_a).unwrap();
            std::fs::write(dir.join("attestation-b.cbor"), &att_b).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("attest.json")).unwrap(),
            json
        );
        assert_eq!(
            std::fs::read(dir.join("statement.cbor")).unwrap(),
            statement
        );
        assert_eq!(
            std::fs::read(dir.join("attestation-a.cbor")).unwrap(),
            att_a
        );
        assert_eq!(
            std::fs::read(dir.join("attestation-b.cbor")).unwrap(),
            att_b
        );
        verify_attestation(&att_b).unwrap();
    }
}
