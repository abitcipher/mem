//! Several devices for one identity (`spec/device.cddl`).
//!
//! - [`DeviceCert`]: the identity certifies a device's Ed25519 key.
//! - [`DeviceList`]: the identity's current devices (1..=[`MAX_DEVICES`], the primary first),
//!   signed by the identity; a higher serial replaces a lower one.
//! - [`DeviceRequest`]: shown as a QR code by a new device; the primary scans it in person.
//!
//! Inside other objects (a key update, an inner payload) a certificate or list travels as a byte
//! string holding its own canonical CBOR, as group states carry pairing cards. Decoding checks
//! shape and bounds only; signatures are checked by the core.

use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Domain separation for a device certificate.
pub const DEVICE_CERT_DOMAIN: &[u8] = b"MEM/v1/device-cert";
/// Domain separation for a device list.
pub const DEVICE_LIST_DOMAIN: &[u8] = b"MEM/v1/device-list";
/// Domain separation for a device request.
pub const DEVICE_REQUEST_DOMAIN: &[u8] = b"MEM/v1/device-request";
/// Most devices per identity, the primary included.
pub const MAX_DEVICES: usize = 4;
/// Text form of a device request: this prefix and base64url without padding.
pub const DEVICE_REQUEST_PREFIX: &str = "memdevice1:";
/// Feature bit 0 in a key update: the sender understands devices.
pub const FEATURE_DEVICES: u32 = 1;
/// Length of an encoded certificate.
pub const DEVICE_CERT_LEN: usize = 140;
/// Largest encoded device list.
pub const MAX_DEVICE_LIST_LEN: usize = 64 + MAX_DEVICES * (DEVICE_CERT_LEN + 2) + 70;

/// The identity's signature over a device's key.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCert {
    /// The identity (Ed25519).
    pub identity_pk: [u8; 32],
    /// The device key (Ed25519).
    pub device_pk: [u8; 32],
    /// Ed25519 by the identity over [`DEVICE_CERT_DOMAIN`] || [`DeviceCert::signed_bytes`].
    pub signature: [u8; 64],
}

impl fmt::Debug for DeviceCert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCert").finish_non_exhaustive()
    }
}

impl DeviceCert {
    fn write(&self, signed: bool) -> Vec<u8> {
        encode_with(DEVICE_CERT_LEN, |e| {
            e.map(3 + u64::from(signed))?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&self.identity_pk)?
                .u8(2)?
                .bytes(&self.device_pk)?;
            if signed {
                e.u8(3)?.bytes(&self.signature)?;
            }
            Ok(())
        })
    }

    /// The bytes the identity signs: the canonical map without key 3.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        self.write(false)
    }

    /// Canonical CBOR.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.write(true)
    }

    /// Decodes an untrusted certificate. The signature is not checked here.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed or non-canonical input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: DEVICE_CERT_LEN,
            max_container_len: 4,
        };
        let mut r = MapReader::new(input, &limits)?;
        if r.u8(0)? != 1 {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let identity_pk = r.array::<32>(1)?;
        let device_pk = r.array::<32>(2)?;
        let signature = r.array::<64>(3)?;
        r.finish()?;
        Ok(Self {
            identity_pk,
            device_pk,
            signature,
        })
    }
}

/// The identity's current devices, signed by the identity.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceList {
    /// The identity (Ed25519).
    pub identity_pk: [u8; 32],
    /// Rises with every new list.
    pub serial: u32,
    /// 1..=[`MAX_DEVICES`] certificates, the primary first, each for this identity, all device
    /// keys distinct.
    pub certs: Vec<DeviceCert>,
    /// Ed25519 by the identity over [`DEVICE_LIST_DOMAIN`] || [`DeviceList::signed_bytes`].
    pub signature: [u8; 64],
}

impl fmt::Debug for DeviceList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceList")
            .field("serial", &self.serial)
            .field("devices", &self.certs.len())
            .finish_non_exhaustive()
    }
}

impl DeviceList {
    fn check(&self) -> Result<(), SchemaError> {
        if self.certs.is_empty() || self.certs.len() > MAX_DEVICES {
            return Err(SchemaError::WrongLength { key: 2 });
        }
        for (i, c) in self.certs.iter().enumerate() {
            if c.identity_pk != self.identity_pk
                || self.certs[..i].iter().any(|o| o.device_pk == c.device_pk)
            {
                return Err(SchemaError::UnknownValue { key: 2 });
            }
        }
        Ok(())
    }

    fn write(&self, signed: bool) -> Vec<u8> {
        encode_with(MAX_DEVICE_LIST_LEN, |e| {
            e.map(3 + u64::from(signed))?
                .u8(0)?
                .bytes(&self.identity_pk)?
                .u8(1)?
                .u32(self.serial)?
                .u8(2)?
                .array(self.certs.len() as u64)?;
            for c in &self.certs {
                e.bytes(&c.encode())?;
            }
            if signed {
                e.u8(3)?.bytes(&self.signature)?;
            }
            Ok(())
        })
    }

    /// The bytes the identity signs: the canonical map without key 3.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] if the list is empty, too long, mixes identities or repeats a device.
    pub fn signed_bytes(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(self.write(false))
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// As [`DeviceList::signed_bytes`].
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(self.write(true))
    }

    /// Decodes an untrusted list. Signatures are not checked here.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or out-of-bounds input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_DEVICE_LIST_LEN,
            max_container_len: MAX_DEVICES as u64,
        };
        let mut r = MapReader::new(input, &limits)?;
        let identity_pk = r.array::<32>(0)?;
        let serial = u32::try_from(r.u64(1)?).map_err(|_| SchemaError::WrongType { key: 1 })?;
        let certs = r
            .bytes_list(2, MAX_DEVICES, DEVICE_CERT_LEN)?
            .into_iter()
            .map(DeviceCert::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let signature = r.array::<64>(3)?;
        r.finish()?;
        let list = Self {
            identity_pk,
            serial,
            certs,
            signature,
        };
        list.check()?;
        Ok(list)
    }
}

/// A new device asks to be added; shown as a QR code, scanned in person.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceRequest {
    /// The new device's key (Ed25519).
    pub device_pk: [u8; 32],
    /// Its first mailbox key (X25519): the setup object is sealed to it.
    pub mailbox_pk: [u8; 32],
    /// Fresh per request; bound to the setup object.
    pub nonce: [u8; 16],
    /// Ed25519 by the device key over [`DEVICE_REQUEST_DOMAIN`] || [`DeviceRequest::signed_bytes`].
    pub signature: [u8; 64],
}

impl fmt::Debug for DeviceRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceRequest").finish_non_exhaustive()
    }
}

impl DeviceRequest {
    fn write(&self, signed: bool) -> Vec<u8> {
        encode_with(160, |e| {
            e.map(3 + u64::from(signed))?
                .u8(0)?
                .bytes(&self.device_pk)?
                .u8(1)?
                .bytes(&self.mailbox_pk)?
                .u8(2)?
                .bytes(&self.nonce)?;
            if signed {
                e.u8(3)?.bytes(&self.signature)?;
            }
            Ok(())
        })
    }

    /// The bytes the device key signs: the canonical map without key 3.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        self.write(false)
    }

    /// Canonical CBOR.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.write(true)
    }

    /// Decodes an untrusted request. The signature is not checked here.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed or non-canonical input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: 160,
            max_container_len: 4,
        };
        let mut r = MapReader::new(input, &limits)?;
        let device_pk = r.array::<32>(0)?;
        let mailbox_pk = r.array::<32>(1)?;
        let nonce = r.array::<16>(2)?;
        let signature = r.array::<64>(3)?;
        r.finish()?;
        Ok(Self {
            device_pk,
            mailbox_pk,
            nonce,
            signature,
        })
    }
}

/// Content type 8: own-device sync, between the devices of one identity.
pub const CONTENT_TYPE_OWN_SYNC: u64 = 8;
/// Largest data in one sync item (a contact record is well under 4 KiB).
pub const MAX_OWN_SYNC_LEN: usize = 8192;

/// What one own-device sync item says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OwnSyncKind {
    /// A contact to add: the data is its record in the app's snapshot format.
    ContactAdded,
    /// A contact to remove: the data is its identity key (32 bytes).
    ContactRemoved,
}

/// One own-device sync item: `{0: kind, 1: data}`.
#[derive(Clone, PartialEq, Eq)]
pub struct OwnSync {
    /// What it says.
    pub kind: OwnSyncKind,
    /// Its data.
    pub data: Vec<u8>,
}

impl fmt::Debug for OwnSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnSync")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl OwnSync {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the data is empty or too long.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        if self.data.is_empty() || self.data.len() > MAX_OWN_SYNC_LEN {
            return Err(SchemaError::WrongLength { key: 1 }.into());
        }
        let kind = match self.kind {
            OwnSyncKind::ContactAdded => 1,
            OwnSyncKind::ContactRemoved => 2,
        };
        Ok(encode_with(self.data.len() + 16, |e| {
            e.map(2)?.u8(0)?.u8(kind)?.u8(1)?.bytes(&self.data)?;
            Ok(())
        }))
    }

    /// Decodes an untrusted item (it is authenticated by the inner signature, but bounded
    /// all the same).
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_OWN_SYNC_LEN + 16,
            max_container_len: 2,
        };
        let mut r = MapReader::new(input, &limits)?;
        let kind = match r.u8(0)? {
            1 => OwnSyncKind::ContactAdded,
            2 => OwnSyncKind::ContactRemoved,
            _ => return Err(SchemaError::UnknownValue { key: 0 }.into()),
        };
        let data = r.bytes(1)?.to_vec();
        r.finish()?;
        if data.is_empty() {
            return Err(SchemaError::WrongLength { key: 1 }.into());
        }
        Ok(Self { kind, data })
    }
}

/// A device setup sealed to a new device: HPKE suite 1 to the request's
/// mailbox key, `info` "MEM/v1/device-setup", `aad` the request's nonce. The plaintext is the
/// main device's state for the new device, in the app's own snapshot format (both devices run
/// the same version: the setup happens in person, phone next to phone).
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceSetup {
    /// HPKE encapsulated key (32 bytes).
    pub enc: [u8; 32],
    /// AEAD ciphertext.
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for DeviceSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceSetup")
            .field("len", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}

/// Largest sealed setup: a QR transfer carries at most 64 KiB (as a QR backup).
pub const MAX_DEVICE_SETUP_LEN: usize = 65_536;

impl DeviceSetup {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if the ciphertext is empty or too long.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        if self.ciphertext.is_empty() || self.ciphertext.len() > MAX_DEVICE_SETUP_LEN - 64 {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        Ok(encode_with(self.ciphertext.len() + 48, |e| {
            e.map(3)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&self.enc)?
                .u8(2)?
                .bytes(&self.ciphertext)?;
            Ok(())
        }))
    }

    /// Decodes an untrusted setup.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_DEVICE_SETUP_LEN,
            max_container_len: 3,
        };
        let mut r = MapReader::new(input, &limits)?;
        if r.u8(0)? != 1 {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let enc = r.array::<32>(1)?;
        let ciphertext = r.bytes(2)?.to_vec();
        r.finish()?;
        if ciphertext.is_empty() {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        Ok(Self { enc, ciphertext })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    pub(crate) fn cert(device: u8) -> DeviceCert {
        DeviceCert {
            identity_pk: [1; 32],
            device_pk: [device; 32],
            signature: [9; 64],
        }
    }

    pub(crate) fn list() -> DeviceList {
        DeviceList {
            identity_pk: [1; 32],
            serial: 2,
            certs: alloc::vec![cert(10), cert(11)],
            signature: [8; 64],
        }
    }

    fn request() -> DeviceRequest {
        DeviceRequest {
            device_pk: [3; 32],
            mailbox_pk: [4; 32],
            nonce: [5; 16],
            signature: [6; 64],
        }
    }

    #[test]
    fn roundtrips_and_signs_without_the_signature() {
        let c = cert(10);
        assert_eq!(c.encode().len(), DEVICE_CERT_LEN);
        assert_eq!(DeviceCert::decode(&c.encode()).unwrap(), c);
        assert_eq!(c.signed_bytes()[0], 0xa3);
        let l = list();
        assert_eq!(DeviceList::decode(&l.encode().unwrap()).unwrap(), l);
        assert_eq!(l.signed_bytes().unwrap()[0], 0xa3);
        let q = request();
        assert_eq!(DeviceRequest::decode(&q.encode()).unwrap(), q);
        assert_eq!(q.signed_bytes()[0], 0xa3);
    }

    #[test]
    fn list_bounds() {
        let mut l = list();
        l.certs.clear();
        assert!(l.encode().is_err());
        let mut l = list();
        l.certs = (10..15).map(cert).collect();
        assert!(l.encode().is_err(), "five devices");
        let mut l = list();
        l.certs[1] = cert(10);
        assert!(l.encode().is_err(), "a device twice");
        let mut l = list();
        l.certs[1].identity_pk = [2; 32];
        assert!(l.encode().is_err(), "another identity's device");
        // A list of four is the most.
        let mut l = list();
        l.certs = (10..14).map(cert).collect();
        assert!(l.encode().unwrap().len() <= MAX_DEVICE_LIST_LEN);
    }

    #[test]
    fn rejects_trailing_and_unknown() {
        let mut b = cert(10).encode();
        b.push(0);
        assert!(DeviceCert::decode(&b).is_err());
        let mut b = request().encode();
        b[0] += 1;
        b.extend_from_slice(&[0x04, 0x00]);
        assert!(DeviceRequest::decode(&b).is_err());
    }

    /// Published vectors (`spec/vectors/device/`), validated against `spec/device.cddl` in CI.
    /// Regenerate with `MEM_UPDATE_VECTORS=1`.
    #[test]
    fn published_vectors() {
        extern crate std;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/device");
        let setup = DeviceSetup {
            enc: [7; 32],
            ciphertext: alloc::vec![8; 100],
        };
        assert_eq!(
            DeviceSetup::decode(&setup.encode().unwrap()).unwrap(),
            setup
        );
        let sync = OwnSync {
            kind: OwnSyncKind::ContactRemoved,
            data: alloc::vec![1; 32],
        };
        assert_eq!(OwnSync::decode(&sync.encode().unwrap()).unwrap(), sync);
        let all = [
            ("sync.cbor", sync.encode().unwrap()),
            ("cert.cbor", cert(10).encode()),
            ("list.cbor", list().encode().unwrap()),
            ("request.cbor", request().encode()),
            ("setup.cbor", setup.encode().unwrap()),
        ];
        for (name, bytes) in all {
            if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(name), &bytes).unwrap();
            }
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes, "{name}");
        }
    }
}
