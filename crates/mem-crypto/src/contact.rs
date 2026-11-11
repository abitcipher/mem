//! Contact secret, short authentication string and rendezvous tags.

use core::fmt;

use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use rand_core::CryptoRng;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::identity::{IdentityKey, IdentityPublic};

/// Fresh randomness contributed by each side to a pairing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PairingNonce([u8; 16]);

impl PairingNonce {
    /// Generates a nonce from the injected CSPRNG.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut n = [0u8; 16];
        rng.fill_bytes(&mut n);
        Self(n)
    }

    /// Wraps bytes from a pairing card or storage.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for PairingNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingNonce(..)")
    }
}

/// Who receives a bundle, relative to the ordered pair of identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The receiver is the identity with the lexicographically smaller public key.
    ToLo,
    /// The receiver is the identity with the larger public key.
    ToHi,
}

impl Direction {
    /// Direction of a bundle addressed to `receiver` within the pair `{receiver, other}`.
    #[must_use]
    pub fn towards(receiver: &IdentityPublic, other: &IdentityPublic) -> Self {
        if receiver < other {
            Self::ToLo
        } else {
            Self::ToHi
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ToLo => 0,
            Self::ToHi => 1,
        }
    }
}

/// The shared secret of an approved contact pair. Zeroized on drop; never printed.
#[derive(Clone)]
pub struct ContactSecret(Zeroizing<[u8; 32]>);

/// Number of emoji in the short authentication string.
pub const SAS_LEN: usize = 6;

impl ContactSecret {
    /// Derives the contact secret from our identity, the peer's identity and both pairing
    /// nonces.
    ///
    /// Both sides obtain the same secret: inputs are ordered by identity public key.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] for an invalid peer key, [`CryptoError::NonContributory`]
    /// if the key agreement degenerates (low-order point), or if both identities are the same.
    pub fn derive(
        own: &IdentityKey,
        own_nonce: &PairingNonce,
        peer: &IdentityPublic,
        peer_nonce: &PairingNonce,
    ) -> Result<Self, CryptoError> {
        let own_public = own.public();
        if own_public == *peer {
            return Err(CryptoError::NonContributory);
        }
        let shared = own.x25519_secret().diffie_hellman(&peer.x25519_public()?);
        if !shared.was_contributory() {
            return Err(CryptoError::NonContributory);
        }
        let (lo, hi, nonce_lo, nonce_hi) = if own_public < *peer {
            (own_public, *peer, own_nonce, peer_nonce)
        } else {
            (*peer, own_public, peer_nonce, own_nonce)
        };

        let mut ikm = Zeroizing::new([0u8; 64]);
        ikm[..32].copy_from_slice(shared.as_bytes());
        ikm[32..48].copy_from_slice(&nonce_lo.0);
        ikm[48..].copy_from_slice(&nonce_hi.0);

        let mut info = [0u8; 14 + 64];
        info[..14].copy_from_slice(b"MEM/v1/contact");
        info[14..46].copy_from_slice(lo.as_bytes());
        info[46..].copy_from_slice(hi.as_bytes());

        let mut okm = Zeroizing::new([0u8; 32]);
        hkdf_expand(&ikm[..], &info, okm.as_mut());
        Ok(Self(okm))
    }

    /// Restores a stored secret.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(Zeroizing::new(*bytes))
    }

    /// Raw secret, for encrypted storage and keyed derivations only.
    ///
    /// # Security
    ///
    /// Secret. Never log it and never pass it across FFI.
    #[must_use]
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    /// Short authentication string as indexes into a 64-entry emoji table.
    #[must_use]
    pub fn sas(&self) -> [u8; SAS_LEN] {
        let mut out = [0u8; SAS_LEN];
        hkdf_expand(&self.0[..], b"MEM/v1/sas", &mut out);
        for b in &mut out {
            *b &= 63;
        }
        out
    }

    /// Conversation id of the 1:1 conversation with this contact.
    #[must_use]
    pub fn conversation_id(&self) -> [u8; 16] {
        let full = hmac_sha256(&self.0, &[b"MEM/v1/conversation"]);
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        out
    }

    /// Rendezvous tag for `direction` in day `epoch`.
    #[must_use]
    pub fn tag(&self, direction: Direction, epoch: u32) -> [u8; 16] {
        let full = hmac_sha256(
            &self.0,
            &[b"MEM/v1/tag", &[direction.byte()], &epoch.to_be_bytes()],
        );
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        out
    }

    /// Rendezvous tag of one of the receiver's other devices: as [`Self::tag`]
    /// with the device id as one more input, under its own label so it can never equal a main
    /// device's tag.
    /// `HMAC-SHA256(contact_secret, "MEM/v1/device-tag" || direction || epoch || device_id)[0..16]`.
    #[must_use]
    pub fn device_tag(&self, direction: Direction, epoch: u32, device_id: &[u8; 8]) -> [u8; 16] {
        let full = hmac_sha256(
            &self.0,
            &[
                b"MEM/v1/device-tag",
                &[direction.byte()],
                &epoch.to_be_bytes(),
                device_id,
            ],
        );
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        out
    }

    /// Direct-delivery proof: shows the recipient that the peer handing over
    /// `bundle_id` is the sender itself, not a courier.
    /// `HMAC-SHA256(contact_secret, "MEM/v1/direct" || bundle_id || receiver_ephemeral_id)[0..16]`.
    ///
    /// # Security
    ///
    /// Only the two contacts know the secret, so a courier cannot forge it; the receiver's
    /// per-encounter `hello` id stops replay into another encounter. Compare with
    /// [`ContactSecret::verify_direct_proof`] (constant time).
    #[must_use]
    pub fn direct_proof(&self, bundle_id: &[u8; 16], receiver_ephemeral_id: &[u8; 16]) -> [u8; 16] {
        let full = hmac_sha256(
            &self.0,
            &[b"MEM/v1/direct", bundle_id, receiver_ephemeral_id],
        );
        let mut out = [0u8; 16];
        out.copy_from_slice(&full[..16]);
        out
    }

    /// Checks a direct-delivery proof in constant time.
    #[must_use]
    pub fn verify_direct_proof(
        &self,
        bundle_id: &[u8; 16],
        receiver_ephemeral_id: &[u8; 16],
        proof: &[u8; 16],
    ) -> bool {
        let expected = self.direct_proof(bundle_id, receiver_ephemeral_id);
        expected
            .iter()
            .zip(proof)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

impl fmt::Debug for ContactSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContactSecret(<secret>)")
    }
}

/// Day number used as the tag epoch.
#[must_use]
pub fn epoch_for_unix_ms(unix_ms: u64) -> u32 {
    u32::try_from(unix_ms / 86_400_000).unwrap_or(u32::MAX)
}

/// HMAC-SHA256 with a 32-byte key over the concatenation of `parts`.
///
/// RFC 2104 zero-pads keys shorter than the 64-byte block, so keying with the padded block is
/// exactly HMAC with the 32-byte key, and cannot fail.
pub(crate) fn hmac_sha256(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    let mut block = Zeroizing::new([0u8; 64]);
    block[..32].copy_from_slice(key);
    let mut mac = <Hmac<Sha256> as KeyInit>::new(&(*block).into());
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

/// HKDF-SHA256 with an empty salt. Outputs here are at most 32 bytes, far below the
/// 255 * 32 limit, so expansion cannot fail.
fn hkdf_expand(ikm: &[u8], info: &[u8], okm: &mut [u8]) {
    let hk = Hkdf::<Sha256>::new(None, ikm);
    if hk.expand(info, okm).is_err() {
        okm.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    /// A known answer computed independently (Python `hmac`), and the main tag
    /// for the same inputs, which must differ.
    #[test]
    fn device_tag_known_answer() {
        let secret = ContactSecret::from_bytes(&[7; 32]);
        let id = [1, 2, 3, 4, 5, 6, 7, 8];
        let unhex = |h: &str| -> alloc::vec::Vec<u8> {
            (0..h.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
                .collect()
        };
        assert_eq!(
            secret.device_tag(Direction::ToHi, 20_000, &id).to_vec(),
            unhex("3c3a3d365f28deeb2996859894547472")
        );
        assert_eq!(
            secret.tag(Direction::ToHi, 20_000).to_vec(),
            unhex("98374430839393d9737d7d856a029c5a")
        );
        assert_ne!(
            secret.device_tag(Direction::ToHi, 20_000, &id),
            secret.device_tag(Direction::ToHi, 20_000, &[9; 8])
        );
    }

    struct Side {
        key: IdentityKey,
        nonce: PairingNonce,
    }

    fn side(seed: u8) -> Side {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        Side {
            key: IdentityKey::generate(&mut rng),
            nonce: PairingNonce::generate(&mut rng),
        }
    }

    #[test]
    fn both_sides_derive_the_same_secret_sas_and_tags() {
        let (a, b) = (side(1), side(2));
        let sa = ContactSecret::derive(&a.key, &a.nonce, &b.key.public(), &b.nonce).unwrap();
        let sb = ContactSecret::derive(&b.key, &b.nonce, &a.key.public(), &a.nonce).unwrap();
        assert_eq!(sa.expose(), sb.expose());
        assert_eq!(sa.sas(), sb.sas());
        assert!(sa.sas().iter().all(|i| *i < 64));

        let to_a = Direction::towards(&a.key.public(), &b.key.public());
        let to_b = Direction::towards(&b.key.public(), &a.key.public());
        assert_ne!(to_a, to_b);
        assert_eq!(sa.tag(to_a, 20_000), sb.tag(to_a, 20_000));
        assert_ne!(sa.tag(to_a, 20_000), sa.tag(to_b, 20_000));
        assert_ne!(sa.tag(to_a, 20_000), sa.tag(to_a, 20_001));
    }

    #[test]
    fn nonce_change_changes_secret() {
        let (a, b) = (side(1), side(2));
        let s1 = ContactSecret::derive(&a.key, &a.nonce, &b.key.public(), &b.nonce).unwrap();
        let other = PairingNonce::from_bytes([0xee; 16]);
        let s2 = ContactSecret::derive(&a.key, &other, &b.key.public(), &b.nonce).unwrap();
        assert_ne!(s1.expose(), s2.expose());
        assert_ne!(s1.sas(), s2.sas());
    }

    #[test]
    fn rejects_pairing_with_self() {
        let a = side(1);
        assert_eq!(
            ContactSecret::derive(&a.key, &a.nonce, &a.key.public(), &a.nonce).err(),
            Some(CryptoError::NonContributory)
        );
    }

    #[test]
    fn padded_key_hmac_equals_rfc2104_hmac() {
        let key = [0x5a; 32];
        let mut reference = <Hmac<Sha256> as KeyInit>::new_from_slice(&key).unwrap();
        reference.update(b"abc");
        let expected: [u8; 32] = reference.finalize().into_bytes().into();
        assert_eq!(hmac_sha256(&key, &[b"a", b"bc"]), expected);
    }

    #[test]
    fn direct_proof_binds_bundle_and_encounter() {
        let s = ContactSecret(Zeroizing::new([5; 32]));
        let p = s.direct_proof(&[1; 16], &[2; 16]);
        assert!(s.verify_direct_proof(&[1; 16], &[2; 16], &p));
        assert!(
            !s.verify_direct_proof(&[1; 16], &[3; 16], &p),
            "another encounter"
        );
        assert!(
            !s.verify_direct_proof(&[9; 16], &[2; 16], &p),
            "another bundle"
        );
        let other = ContactSecret(Zeroizing::new([6; 32]));
        assert!(
            !other.verify_direct_proof(&[1; 16], &[2; 16], &p),
            "another contact"
        );
    }

    #[test]
    fn hmac_matches_independent_reference() {
        // Reference: printf 'Hi There' | openssl dgst -sha256 -mac HMAC -macopt hexkey:0b*32
        let key = [0x0b; 32];
        let got = hmac_sha256(&key, &[b"Hi There"]);
        assert_eq!(
            hex::encode(got),
            "198a607eb44bfbc69903a0f1cf2bbdc5ba0aa3f3d9ae3c1c7a3b1696a0b68cf7"
        );
    }

    #[test]
    fn epoch_is_day_number() {
        assert_eq!(epoch_for_unix_ms(0), 0);
        assert_eq!(epoch_for_unix_ms(86_399_999), 0);
        assert_eq!(epoch_for_unix_ms(86_400_000), 1);
    }
}
