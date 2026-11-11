//! Mailbox keys and HPKE sealing.

use alloc::vec::Vec;
use core::fmt;

use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::{X25519HkdfSha256, XWing};
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

use crate::contact::{ContactSecret, hmac_sha256};
use crate::error::CryptoError;
use crate::identity::{IdentityKey, IdentityPublic};

/// Suite 1: DHKEM(X25519, HKDF-SHA256) / HKDF-SHA256 / ChaCha20-Poly1305, mode Base.
pub const SUITE_ID: u8 = 1;
/// Suite 2: X-Wing (ML-KEM-768 + X25519, HPKE KEM `0x647a`) / HKDF-SHA256 / ChaCha20-Poly1305,
/// mode Base.
pub const SUITE_XWING: u8 = 2;
/// X-Wing public key length.
pub const XWING_PUBLIC_LEN: usize = 1216;
/// X-Wing encapsulated key length.
pub const XWING_ENC_LEN: usize = 1120;

type Kem = X25519HkdfSha256;

/// A mailbox key pair, stored as its RFC 9180 `DeriveKeyPair` seed. Zeroized on drop.
pub struct MailboxKey {
    seed: Zeroizing<[u8; 32]>,
    secret: <Kem as hpke::Kem>::PrivateKey,
    public: MailboxPublic,
    /// The post-quantum half, derived from the same seed.
    xwing: <XWing as hpke::Kem>::PrivateKey,
    xwing_public: XWingPublic,
}

impl MailboxKey {
    /// Generates a new mailbox key from the injected CSPRNG.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut seed = Zeroizing::new([0u8; 32]);
        rng.fill_bytes(seed.as_mut());
        Self::from_seed(&seed)
    }

    /// Restores a mailbox key from its seed (storage only).
    #[must_use]
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let (secret, public) = Kem::derive_keypair(seed);
        let mut pk = [0u8; 32];
        pk.copy_from_slice(&public.to_bytes());
        // The X-Wing key pair comes from its own HKDF output of the seed, so both halves rotate,
        // are restored and are zeroized together.
        let mut ikm = Zeroizing::new([0u8; 32]);
        let _ =
            hkdf::Hkdf::<sha2::Sha256>::new(None, seed).expand(b"MEM/v1/xwing-seed", ikm.as_mut());
        let (xwing, xwing_pub) = XWing::derive_keypair(ikm.as_ref());
        Self {
            seed: Zeroizing::new(*seed),
            secret,
            public: MailboxPublic(pk),
            xwing,
            xwing_public: XWingPublic(xwing_pub.to_bytes().to_vec()),
        }
    }

    /// The post-quantum (X-Wing) public half.
    #[must_use]
    pub fn xwing_public(&self) -> XWingPublic {
        self.xwing_public.clone()
    }

    /// Opens a payload sealed with [`XWingPublic::seal`] (suite 2).
    ///
    /// # Errors
    ///
    /// [`CryptoError::Open`] on any failure.
    pub fn open_xwing(
        &self,
        key_id: &MailboxKeyId,
        enc: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let encapped =
            <XWing as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| CryptoError::Open)?;
        hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, XWing>(
            &OpModeR::Base,
            &self.xwing,
            &encapped,
            &info(SUITE_XWING, *key_id),
            ciphertext,
            aad,
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Open)
    }

    /// The seed, for encrypted local storage only.
    ///
    /// # Security
    ///
    /// Secret. Never log it and never pass it across FFI.
    #[must_use]
    pub fn to_seed(&self) -> Zeroizing<[u8; 32]> {
        self.seed.clone()
    }

    /// The public half.
    #[must_use]
    pub const fn public(&self) -> MailboxPublic {
        self.public
    }

    /// Opens a payload sealed with [`MailboxPublic::seal`].
    ///
    /// # Errors
    ///
    /// [`CryptoError::Open`] on any failure: malformed `enc`, wrong key, or tampered
    /// `info`, `aad` or ciphertext.
    pub fn open(
        &self,
        key_id: &MailboxKeyId,
        enc: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let encapped =
            <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| CryptoError::Open)?;
        hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(
            &OpModeR::Base,
            &self.secret,
            &encapped,
            &info(SUITE_ID, *key_id),
            ciphertext,
            aad,
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Open)
    }
}

impl fmt::Debug for MailboxKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MailboxKey(<secret>)")
    }
}

/// X25519 mailbox public key: the HPKE recipient key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailboxPublic([u8; 32]);

/// Output of [`MailboxPublic::seal`].
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    /// HPKE encapsulated key (32 bytes for X25519).
    pub enc: Vec<u8>,
    /// AEAD ciphertext including the 16-byte tag.
    pub ciphertext: Vec<u8>,
}

impl MailboxPublic {
    /// Parses a public key.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] if the bytes are rejected by the KEM.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        <Kem as hpke::Kem>::PublicKey::from_bytes(bytes)
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(*bytes))
    }

    /// Raw key bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The opaque per-contact handle for this key.
    ///
    /// # Security
    ///
    /// Keyed by the contact secret, so couriers cannot correlate the same mailbox key across
    /// different contacts.
    #[must_use]
    pub fn key_id(&self, contact: &ContactSecret) -> MailboxKeyId {
        let tag = hmac_sha256(contact.expose(), &[b"MEM/v1/mailbox-key-id", &self.0]);
        let mut out = [0u8; 8];
        out.copy_from_slice(&tag[..8]);
        MailboxKeyId(out)
    }

    /// HPKE `SealBase` to this key.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Seal`] if HPKE rejects the key or inputs.
    ///
    /// # Security
    ///
    /// `aad` must be `Bundle::aad()` of the envelope that will carry the payload, binding the
    /// two together.
    pub fn seal(
        &self,
        key_id: &MailboxKeyId,
        plaintext: &[u8],
        aad: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Result<Sealed, CryptoError> {
        let pk =
            <Kem as hpke::Kem>::PublicKey::from_bytes(&self.0).map_err(|_| CryptoError::Seal)?;
        let (enc, ciphertext) =
            hpke::single_shot_seal_with_rng::<ChaCha20Poly1305, HkdfSha256, Kem>(
                &OpModeS::Base,
                &pk,
                &info(SUITE_ID, *key_id),
                plaintext,
                aad,
                rng,
            )
            .map_err(|_| CryptoError::Seal)?;
        Ok(Sealed {
            enc: enc.to_bytes().to_vec(),
            ciphertext,
        })
    }
}

/// HPKE `info` of the device setup object: its own label, so a setup can never
/// be taken for a message payload or the reverse.
pub const DEVICE_SETUP_INFO: &[u8] = b"MEM/v1/device-setup";

impl MailboxPublic {
    /// Seals a device setup object to the mailbox key a new device showed in its request:
    /// HPKE suite 1, mode Base, `info` [`DEVICE_SETUP_INFO`].
    ///
    /// # Errors
    ///
    /// [`CryptoError::Seal`] if HPKE rejects the key or inputs.
    ///
    /// # Security
    ///
    /// `aad` must be the request's nonce, binding the setup to that request. The key came from a
    /// QR code scanned in person; that scan is what authenticates the new device.
    pub fn seal_device_setup(
        &self,
        plaintext: &[u8],
        aad: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Result<Sealed, CryptoError> {
        let pk =
            <Kem as hpke::Kem>::PublicKey::from_bytes(&self.0).map_err(|_| CryptoError::Seal)?;
        let (enc, ciphertext) =
            hpke::single_shot_seal_with_rng::<ChaCha20Poly1305, HkdfSha256, Kem>(
                &OpModeS::Base,
                &pk,
                DEVICE_SETUP_INFO,
                plaintext,
                aad,
                rng,
            )
            .map_err(|_| CryptoError::Seal)?;
        Ok(Sealed {
            enc: enc.to_bytes().to_vec(),
            ciphertext,
        })
    }
}

impl MailboxKey {
    /// Opens a device setup object sealed with [`MailboxPublic::seal_device_setup`].
    ///
    /// # Errors
    ///
    /// [`CryptoError::Open`] if it was not sealed to this key with this `aad`.
    pub fn open_device_setup(
        &self,
        enc: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let encapped =
            <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| CryptoError::Open)?;
        hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(
            &OpModeR::Base,
            &self.secret,
            &encapped,
            DEVICE_SETUP_INFO,
            ciphertext,
            aad,
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Open)
    }
}

impl MailboxPublic {
    /// A contact's identity-derived X25519 key as an HPKE recipient: the fallback for standalone
    /// key updates when every mailbox key we know of may be deleted. The same
    /// birational map as the pairing DH; it never rotates.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] for an identity key that does not map.
    ///
    /// # Security
    ///
    /// Bundles sealed to it have no forward secrecy, so only key-update bundles (a public key,
    /// no content) may use it; the receiver enforces that.
    pub fn of_identity(identity: &IdentityPublic) -> Result<Self, CryptoError> {
        Self::from_bytes(identity.x25519_public()?.as_bytes())
    }
}

/// The opaque handle for the identity fallback, distinct from every mailbox key
/// id because the label differs.
#[must_use]
pub fn identity_key_id(contact: &ContactSecret) -> MailboxKeyId {
    let tag = hmac_sha256(contact.expose(), &[b"MEM/v1/identity-key-id"]);
    let mut out = [0u8; 8];
    out.copy_from_slice(&tag[..8]);
    MailboxKeyId(out)
}

/// Opens a payload sealed to our identity-derived key ([`MailboxPublic::of_identity`]).
///
/// # Errors
///
/// [`CryptoError::Open`] on any failure.
pub fn open_with_identity(
    identity: &IdentityKey,
    key_id: &MailboxKeyId,
    enc: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let scalar = Zeroizing::new(identity.x25519_secret().to_bytes());
    let secret = <Kem as hpke::Kem>::PrivateKey::from_bytes(scalar.as_ref())
        .map_err(|_| CryptoError::Open)?;
    let encapped =
        <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| CryptoError::Open)?;
    hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(
        &OpModeR::Base,
        &secret,
        &encapped,
        &info(SUITE_ID, *key_id),
        ciphertext,
        aad,
    )
    .map(Zeroizing::new)
    .map_err(|_| CryptoError::Open)
}

impl fmt::Debug for MailboxPublic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MailboxPublic(..)")
    }
}

/// A contact's X-Wing public key (1 216 bytes), the suite-2 HPKE recipient.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct XWingPublic(Vec<u8>);

impl XWingPublic {
    /// Parses a public key.
    ///
    /// # Errors
    ///
    /// [`CryptoError::InvalidPublicKey`] for the wrong length or a key the KEM rejects.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() != XWING_PUBLIC_LEN {
            return Err(CryptoError::InvalidPublicKey);
        }
        <XWing as hpke::Kem>::PublicKey::from_bytes(bytes)
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self(bytes.to_vec()))
    }

    /// Raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The opaque per-contact handle for this key: its own label, so it never equals a suite-1
    /// id.
    #[must_use]
    pub fn key_id(&self, contact: &ContactSecret) -> MailboxKeyId {
        let tag = hmac_sha256(contact.expose(), &[b"MEM/v1/xwing-key-id", &self.0]);
        let mut out = [0u8; 8];
        out.copy_from_slice(&tag[..8]);
        MailboxKeyId(out)
    }

    /// HPKE `SealBase` to this key, suite 2.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Seal`] if HPKE rejects the key or inputs.
    ///
    /// # Security
    ///
    /// As [`MailboxPublic::seal`]: `aad` binds the envelope.
    pub fn seal(
        &self,
        key_id: &MailboxKeyId,
        plaintext: &[u8],
        aad: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Result<Sealed, CryptoError> {
        let pk =
            <XWing as hpke::Kem>::PublicKey::from_bytes(&self.0).map_err(|_| CryptoError::Seal)?;
        let (enc, ciphertext) =
            hpke::single_shot_seal_with_rng::<ChaCha20Poly1305, HkdfSha256, XWing>(
                &OpModeS::Base,
                &pk,
                &info(SUITE_XWING, *key_id),
                plaintext,
                aad,
                rng,
            )
            .map_err(|_| CryptoError::Seal)?;
        Ok(Sealed {
            enc: enc.to_bytes().to_vec(),
            ciphertext,
        })
    }
}

impl fmt::Debug for XWingPublic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("XWingPublic(..)")
    }
}

/// Opaque 8-byte mailbox key handle carried in the payload header.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailboxKeyId([u8; 8]);

impl MailboxKeyId {
    /// Wraps raw bytes from the wire.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl fmt::Debug for MailboxKeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MailboxKeyId(..)")
    }
}

fn info(suite: u8, key_id: MailboxKeyId) -> Vec<u8> {
    let mut info = Vec::with_capacity(19);
    info.extend_from_slice(b"MEM/v1/msg");
    info.push(suite);
    info.extend_from_slice(&key_id.0);
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn setup() -> (ChaCha20Rng, MailboxKey, MailboxKeyId) {
        let mut rng = ChaCha20Rng::from_seed([9; 32]);
        let key = MailboxKey::generate(&mut rng);
        let id = MailboxKeyId::from_bytes([1; 8]);
        (rng, key, id)
    }

    /// A setup opens only with its key and nonce, and never as a message
    /// payload (or a payload as a setup): the labels differ.
    #[test]
    fn device_setup_is_bound_to_its_label_and_nonce() {
        let (mut rng, key, id) = setup();
        let sealed = key
            .public()
            .seal_device_setup(b"setup", b"nonce", &mut rng)
            .unwrap();
        let pt = key
            .open_device_setup(&sealed.enc, &sealed.ciphertext, b"nonce")
            .unwrap();
        assert_eq!(pt.as_slice(), b"setup");
        assert!(
            key.open_device_setup(&sealed.enc, &sealed.ciphertext, b"other")
                .is_err()
        );
        assert!(
            key.open(&id, &sealed.enc, &sealed.ciphertext, b"nonce")
                .is_err()
        );
        let msg = key.public().seal(&id, b"msg", b"nonce", &mut rng).unwrap();
        assert!(
            key.open_device_setup(&msg.enc, &msg.ciphertext, b"nonce")
                .is_err()
        );
    }

    #[test]
    fn seal_open_roundtrip() {
        let (mut rng, key, id) = setup();
        let sealed = key.public().seal(&id, b"hello", b"aad", &mut rng).unwrap();
        assert_eq!(sealed.enc.len(), 32);
        assert_eq!(sealed.ciphertext.len(), 5 + 16);
        let pt = key
            .open(&id, &sealed.enc, &sealed.ciphertext, b"aad")
            .unwrap();
        assert_eq!(pt.as_slice(), b"hello");
    }

    #[test]
    fn open_fails_on_any_mismatch() {
        let (mut rng, key, id) = setup();
        let sealed = key.public().seal(&id, b"hello", b"aad", &mut rng).unwrap();
        let other_id = MailboxKeyId::from_bytes([2; 8]);
        assert_eq!(
            key.open(&other_id, &sealed.enc, &sealed.ciphertext, b"aad"),
            Err(CryptoError::Open)
        );
        assert_eq!(
            key.open(&id, &sealed.enc, &sealed.ciphertext, b"aaX"),
            Err(CryptoError::Open)
        );
        let mut ct = sealed.ciphertext.clone();
        ct[0] ^= 1;
        assert_eq!(
            key.open(&id, &sealed.enc, &ct, b"aad"),
            Err(CryptoError::Open)
        );
        let other = MailboxKey::generate(&mut rng);
        assert_eq!(
            other.open(&id, &sealed.enc, &sealed.ciphertext, b"aad"),
            Err(CryptoError::Open)
        );
        assert_eq!(
            key.open(&id, &[0; 5], &sealed.ciphertext, b"aad"),
            Err(CryptoError::Open)
        );
    }

    /// Suite 2: seals and opens, sizes as specified, derived from the seed, and a
    /// suite-1 key id or key cannot open it.
    #[test]
    fn xwing_seal_open_roundtrip() {
        let (mut rng, key, id) = setup();
        let xp = key.xwing_public();
        assert_eq!(xp.as_bytes().len(), XWING_PUBLIC_LEN);
        let sealed = xp.seal(&id, b"hello", b"aad", &mut rng).unwrap();
        assert_eq!(sealed.enc.len(), XWING_ENC_LEN);
        let pt = key
            .open_xwing(&id, &sealed.enc, &sealed.ciphertext, b"aad")
            .unwrap();
        assert_eq!(pt.as_slice(), b"hello");
        assert!(
            key.open_xwing(&id, &sealed.enc, &sealed.ciphertext, b"aaX")
                .is_err()
        );
        assert!(
            key.open(&id, &sealed.enc, &sealed.ciphertext, b"aad")
                .is_err()
        );
        // Restored from the seed, the same X-Wing key; another seed, another key.
        assert_eq!(MailboxKey::from_seed(&key.to_seed()).xwing_public(), xp);
        assert_ne!(MailboxKey::generate(&mut rng).xwing_public(), xp);
        let secret = ContactSecret::from_bytes(&[3; 32]);
        assert_ne!(xp.key_id(&secret), key.public().key_id(&secret));
        assert!(XWingPublic::from_bytes(&[0; 32]).is_err());
    }

    #[test]
    fn seed_roundtrip() {
        let (_, key, _) = setup();
        assert_eq!(MailboxKey::from_seed(&key.to_seed()).public(), key.public());
    }

    #[test]
    fn identity_fallback_roundtrips_and_has_its_own_key_id() {
        use crate::identity::IdentityKey;
        let mut rng = rand_chacha::ChaCha20Rng::from_seed([9; 32]);
        let me = IdentityKey::generate(&mut rng);
        let to = MailboxPublic::of_identity(&me.public()).unwrap();
        let secret = ContactSecret::from_bytes(&[3; 32]);
        let id = identity_key_id(&secret);
        assert_ne!(id, to.key_id(&secret));
        let sealed = to.seal(&id, b"key update", b"aad", &mut rng).unwrap();
        let opened = open_with_identity(&me, &id, &sealed.enc, &sealed.ciphertext, b"aad").unwrap();
        assert_eq!(opened.as_slice(), b"key update");
        assert!(open_with_identity(&me, &id, &sealed.enc, &sealed.ciphertext, b"other").is_err());
    }
}
