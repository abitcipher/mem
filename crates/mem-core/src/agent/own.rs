//! Our own identity and devices.
//!
//! The main device holds the identity key: it pairs, signs cards and signs the device list.
//! Another device holds only the identity's public half and its own device key, certified by
//! the identity; it signs messages with that key and shows the certificate (inner key 11).

use mem_crypto::{IdentityKey, IdentityPublic};

/// Our identity on this device.
pub(crate) struct Identity {
    /// The identity key, on the main device only.
    key: Option<IdentityKey>,
    public: IdentityPublic,
}

impl Identity {
    /// The main device, holding the identity key.
    pub(crate) fn main(key: IdentityKey) -> Self {
        Self {
            public: key.public(),
            key: Some(key),
        }
    }

    /// Another device: the identity's public half only.
    pub(crate) const fn other(public: IdentityPublic) -> Self {
        Self { key: None, public }
    }

    /// The identity's public key.
    pub(crate) const fn public(&self) -> IdentityPublic {
        self.public
    }

    /// The identity key, on the main device; `None` on another device.
    pub(crate) const fn key(&self) -> Option<&IdentityKey> {
        self.key.as_ref()
    }
}

/// This device's place among our devices.
#[derive(Default)]
pub(crate) struct OwnDevices {
    /// This device's key: the main device makes one when it adds its first device; another
    /// device has it from its setup.
    pub(crate) key: Option<IdentityKey>,
    /// This device's certificate, once it is in a list.
    pub(crate) cert: Option<mem_proto::device::DeviceCert>,
    /// Our newest device list, signed by the identity (the main device first).
    pub(crate) list: Option<mem_proto::device::DeviceList>,
}

impl OwnDevices {
    /// This device's id, once it has a key.
    pub(crate) fn id(&self) -> Option<[u8; 8]> {
        self.key.as_ref().map(|k| k.public().device_id())
    }
}

// ------------------------------------------------------------------ the agent's own devices

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use mem_crypto::{ContactSecret, MailboxKey, MailboxPublic};
use mem_proto::BundleKind;
use mem_proto::device::{
    CONTENT_TYPE_OWN_SYNC, DEVICE_CERT_DOMAIN, DEVICE_LIST_DOMAIN, DEVICE_REQUEST_DOMAIN,
    DeviceCert, DeviceList, DeviceRequest, DeviceSetup, MAX_DEVICES, OwnSync, OwnSyncKind,
};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

use super::devices::ContactDevice;
use super::{Agent, Contact, HeldMailbox, Output, StorageOp};
use crate::model::{ContactId, OwnDeviceView, RejectReason, TrustLevel};

impl Agent {
    /// Our own identity as a contact: the hidden "self" contact through which our devices write
    /// to each other. Never shown, paired or grouped.
    pub(crate) fn self_id(&self) -> ContactId {
        ContactId(*self.identity.public().as_bytes())
    }

    /// Whether `id` is the self contact.
    pub(crate) fn is_self(&self, id: ContactId) -> bool {
        id == self.self_id()
    }

    /// Whether this is the main device (it holds the identity key).
    #[must_use]
    pub fn is_main_device(&self) -> bool {
        self.identity.key().is_some()
    }

    /// Starts an in-person attestation from a relying party's request text. The
    /// caller has shown the disclosure first. Main device only (v1).
    ///
    /// # Errors
    ///
    /// [`crate::attest::AttestError::NotMainDevice`] on another device, or the request's errors.
    pub fn start_attestation(
        &self,
        request_text: &str,
        rng: &mut impl CryptoRng,
    ) -> Result<crate::attest::AttestSession, crate::attest::AttestError> {
        let key = self
            .identity
            .key()
            .ok_or(crate::attest::AttestError::NotMainDevice)?;
        crate::attest::AttestSession::start(key, request_text, rng)
    }

    /// Our devices, the main one first; just this one before any other was added.
    #[must_use]
    pub fn own_devices(&self) -> Vec<OwnDeviceView> {
        let this = self.own.id();
        match &self.own.list {
            None => alloc::vec![OwnDeviceView {
                id: this.unwrap_or_default(),
                main: self.is_main_device(),
                this_device: true,
                joined: true,
            }],
            Some(l) => l
                .certs
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let id = super::devices::cert_device_id(c)?;
                    Some(OwnDeviceView {
                        id,
                        main: i == 0,
                        this_device: Some(id) == this,
                        joined: i == 0 || self.device_joined(id),
                    })
                })
                .collect(),
        }
    }

    /// Whether our other device `id` has joined: on the main device, once that device's own
    /// signed key update arrived (it sends one at once, with its X-Wing key, which the request
    /// does not carry); other devices do not track it and say yes.
    fn device_joined(&self, id: [u8; 8]) -> bool {
        if !self.is_main_device() {
            return true;
        }
        self.contacts
            .get(&self.self_id())
            .and_then(|c| c.devices.iter().find(|d| d.id == id))
            .is_some_and(|d| d.xwing.is_some())
    }

    /// The main device adds a device whose request it scanned in person:
    /// certifies its key, signs a new list, and seals what the new device needs to it.
    pub(super) fn approve_device(
        &mut self,
        request: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let reject = |r| alloc::vec![Output::Rejected(r)];
        if !self.is_main_device() {
            return reject(RejectReason::MainDeviceOnly);
        }
        let Ok(req) = DeviceRequest::decode(request) else {
            return reject(RejectReason::Malformed);
        };
        let (Ok(device), Ok(mailbox)) = (
            mem_crypto::IdentityPublic::from_bytes(&req.device_pk),
            MailboxPublic::from_bytes(&req.mailbox_pk),
        ) else {
            return reject(RejectReason::Malformed);
        };
        if device
            .verify(
                DEVICE_REQUEST_DOMAIN,
                &[&req.signed_bytes()],
                &req.signature,
            )
            .is_err()
        {
            return reject(RejectReason::BadSignature);
        }
        if self.own.key.is_none() {
            self.own.key = Some(IdentityKey::generate(rng));
        }
        let Some(list) = self.sign_list_with(Some(req.device_pk), None) else {
            return reject(RejectReason::TooManyDevices);
        };
        let Some(new_cert) = list
            .certs
            .iter()
            .find(|c| c.device_pk == req.device_pk)
            .cloned()
        else {
            return reject(RejectReason::DeviceKnown);
        };
        self.ensure_self_contact(&list, rng);
        let me = self.self_id();
        let now = self.now_unix_ms;
        if let Some(c) = self.contacts.get_mut(&me) {
            c.devices.push(ContactDevice {
                id: device.device_id(),
                key: device,
                mailbox: Some(mailbox),
                // A new device's first mailbox key is serial 1 (`DeviceJoin`).
                mailbox_serial: 1,
                mailbox_learned_ms: now,
                xwing: None,
                told_serial: 0,
                told_list: list.serial,
                retired: false,
            });
        }
        let state = self.device_setup_state(&new_cert, device.device_id());
        let Ok(sealed) = mailbox.seal_device_setup(&state, &req.nonce, rng) else {
            return reject(RejectReason::Malformed);
        };
        let Ok(enc) = <[u8; 32]>::try_from(sealed.enc.as_slice()) else {
            return reject(RejectReason::Malformed);
        };
        let Ok(setup) = (DeviceSetup {
            enc,
            ciphertext: sealed.ciphertext,
        })
        .encode() else {
            // More contacts than one QR transfer carries (64 KiB).
            return reject(RejectReason::TooLargeForChannel);
        };
        // The new device has every contact there is now.
        let all: BTreeSet<ContactId> = self.contacts.keys().copied().filter(|c| *c != me).collect();
        self.own_sync.insert(device.device_id(), all);
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::DeviceSetupReady { setup },
        ];
        out.extend(self.advertise_all(rng));
        out
    }

    /// The main device retires one of its other devices: a new list without it.
    /// Contacts learn it with our next key updates, at once; the retired device gets that
    /// list once more, so it knows (it is forgotten when it confirms).
    pub(super) fn retire_device(
        &mut self,
        device: [u8; 8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if !self.is_main_device() {
            return alloc::vec![Output::Rejected(RejectReason::MainDeviceOnly)];
        }
        let listed = self.own.list.as_ref().is_some_and(|l| {
            l.certs
                .iter()
                .skip(1)
                .any(|c| super::devices::cert_device_id(c) == Some(device))
        });
        if !listed {
            return alloc::vec![Output::Rejected(RejectReason::UnknownDevice)];
        }
        let Some(list) = self.sign_list_with(None, Some(device)) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownDevice)];
        };
        let me = self.self_id();
        if let Some(c) = self.contacts.get_mut(&me) {
            c.device_list = Some(list);
        }
        self.outbox
            .retain(|_, i| i.contact != me || i.device != Some(device));
        self.own_sync.remove(&device);
        // The farewell: a key update with the list that drops it, sealed while it is still
        // known; then it gets nothing else. Also to a device that has not reported in: its
        // setup was shown and may still be read, and then that device must learn it is out.
        let farewell = self.queue_copy(
            me,
            device,
            None,
            BundleKind::KeyUpdate,
            mem_proto::inner::CONTENT_TYPE_KEY_UPDATE,
            &[],
            rng,
        );
        if let Some(d) = self
            .contacts
            .get_mut(&me)
            .and_then(|c| c.devices.iter_mut().find(|d| d.id == device))
        {
            d.retired = true;
        }
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        if let Some(id) = farewell {
            out.extend(self.flush_message(id));
        }
        out
    }

    /// Signs our next list: the current devices (the main one first), plus `add`, minus
    /// `remove`. `None` when it would be too long, or `add` is already listed.
    fn sign_list_with(
        &mut self,
        add: Option<[u8; 32]>,
        remove: Option<[u8; 8]>,
    ) -> Option<DeviceList> {
        let identity = self.identity.key()?;
        let me = *self.identity.public().as_bytes();
        let main = *self.own.key.as_ref()?.public().as_bytes();
        let cert = |device_pk: [u8; 32]| {
            let mut c = DeviceCert {
                identity_pk: me,
                device_pk,
                signature: [0; 64],
            };
            c.signature = identity.sign(DEVICE_CERT_DOMAIN, &[&c.signed_bytes()]);
            c
        };
        let mut certs: Vec<DeviceCert> = match &self.own.list {
            Some(l) => l.certs.clone(),
            None => alloc::vec![cert(main)],
        };
        if let Some(id) = remove {
            certs.retain(|c| super::devices::cert_device_id(c) != Some(id));
        }
        if let Some(pk) = add {
            if certs.iter().any(|c| c.device_pk == pk) || certs.len() >= MAX_DEVICES {
                return None;
            }
            certs.push(cert(pk));
        }
        let mut list = DeviceList {
            identity_pk: me,
            serial: self
                .own
                .list
                .as_ref()
                .map_or(1, |l| l.serial.saturating_add(1)),
            certs,
            signature: [0; 64],
        };
        list.signature = identity.sign(DEVICE_LIST_DOMAIN, &[&list.signed_bytes().ok()?]);
        self.own.cert = list.certs.first().cloned();
        self.own.list = Some(list.clone());
        Some(list)
    }

    /// The self contact on the main device: our identity, a fresh own-device secret, our list.
    fn ensure_self_contact(&mut self, list: &DeviceList, rng: &mut impl CryptoRng) {
        let me = self.self_id();
        let cur = self.current_mailbox();
        let (mailbox, serial) = (cur.key.public(), cur.serial);
        let now = self.now_unix_ms;
        let floor = self.default_floor;
        let c = self.contacts.entry(me).or_insert_with(|| {
            let mut secret = Zeroizing::new([0u8; 32]);
            rng.fill_bytes(secret.as_mut());
            Contact {
                identity: mem_crypto::IdentityPublic::from_bytes(&me.0)
                    .unwrap_or_else(|_| unreachable!("our own identity key")),
                mailbox,
                secret: ContactSecret::from_bytes(&secret),
                display_name: None,
                trust: TrustLevel::MetInPerson,
                floor,
                added_unix_ms: now,
                mailbox_serial: serial,
                mailbox_learned_ms: now,
                // Nothing is ever sent to the main device's own self.
                told_serial: u32::MAX,
                requested: false,
                group_only: false,
                email: None,
                alias: None,
                xwing: None,
                told_xwing: u32::MAX,
                devices: Vec::new(),
                device_list: None,
                told_list: u32::MAX,
            }
        });
        c.device_list = Some(list.clone());
    }

    /// The main device sends each other device the contacts it has not had yet, and the
    /// removal of those that are gone.
    pub(super) fn sync_own_devices(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        if !self.is_main_device() {
            return Vec::new();
        }
        let me = self.self_id();
        let devices: Vec<[u8; 8]> = self
            .contacts
            .get(&me)
            .map(|c| c.devices.iter().map(|d| d.id).collect())
            .unwrap_or_default();
        let current: BTreeSet<ContactId> =
            self.contacts.keys().copied().filter(|c| *c != me).collect();
        let mut queued = Vec::new();
        for d in devices {
            let sent = self.own_sync.get(&d).cloned().unwrap_or_default();
            let mut items: Vec<OwnSync> = Vec::new();
            for id in current.difference(&sent) {
                if let Some(record) = self.contact_record(*id) {
                    items.push(OwnSync {
                        kind: OwnSyncKind::ContactAdded,
                        data: record,
                    });
                }
            }
            for id in sent.difference(&current) {
                items.push(OwnSync {
                    kind: OwnSyncKind::ContactRemoved,
                    data: id.0.to_vec(),
                });
            }
            if items.is_empty() {
                continue;
            }
            for item in items {
                let Ok(content) = item.encode() else {
                    continue;
                };
                if let Some(id) = self.queue_copy(
                    me,
                    d,
                    None,
                    BundleKind::Message,
                    CONTENT_TYPE_OWN_SYNC,
                    &content,
                    rng,
                ) {
                    queued.push(id);
                }
            }
            self.own_sync.insert(d, current.clone());
        }
        if queued.is_empty() {
            return Vec::new();
        }
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        for id in queued {
            out.extend(self.flush_message(id));
        }
        out
    }

    /// A sync item from our main device (only it sends them): add or remove a contact.
    pub(super) fn received_own_sync(
        &mut self,
        data: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Ok(item) = OwnSync::decode(data) else {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        };
        let me = self.self_id();
        match item.kind {
            OwnSyncKind::ContactAdded => {
                let Some((id, contact)) = self.contact_from_record(&item.data) else {
                    return alloc::vec![Output::Rejected(RejectReason::Malformed)];
                };
                // Never our own identity; a contact this device already knows keeps its state
                // (it may have newer keys from the contact itself).
                if id == me || self.contacts.contains_key(&id) {
                    return Vec::new();
                }
                self.contacts.insert(id, contact);
                let mut out = alloc::vec![
                    Output::Persist(StorageOp::SaveSnapshot),
                    Output::ContactsChanged,
                ];
                out.extend(self.advertise_all(rng));
                out
            }
            OwnSyncKind::ContactRemoved => {
                let Ok(pk) = <[u8; 32]>::try_from(item.data.as_slice()) else {
                    return alloc::vec![Output::Rejected(RejectReason::Malformed)];
                };
                let id = ContactId(pk);
                if id == me {
                    return Vec::new();
                }
                let mut out = self.remove_contact(id, rng);
                out.push(Output::ContactsChanged);
                out
            }
        }
    }
}

/// A new device waiting to be added: its key, first mailbox key and the
/// request it shows as a QR code. Kept in memory only, until the setup arrives.
pub struct DeviceJoin {
    key: IdentityKey,
    mailbox: MailboxKey,
    nonce: [u8; 16],
}

/// Why a device setup could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    /// Not a setup, or not for this request.
    NotForThisDevice,
    /// The setup opened but its contents are unusable.
    Corrupt,
}

impl DeviceJoin {
    /// A fresh device key, mailbox key and nonce.
    pub fn new(rng: &mut impl CryptoRng) -> Self {
        let mut nonce = [0u8; 16];
        rng.fill_bytes(&mut nonce);
        Self {
            key: IdentityKey::generate(rng),
            mailbox: MailboxKey::generate(rng),
            nonce,
        }
    }

    /// The signed request (`spec/device.cddl`).
    #[must_use]
    pub fn request(&self) -> Vec<u8> {
        let mut r = DeviceRequest {
            device_pk: *self.key.public().as_bytes(),
            mailbox_pk: *self.mailbox.public().as_bytes(),
            nonce: self.nonce,
            signature: [0; 64],
        };
        r.signature = self.key.sign(DEVICE_REQUEST_DOMAIN, &[&r.signed_bytes()]);
        r.encode()
    }

    /// The request as the text a QR code shows: `memdevice1:` and base64url.
    #[must_use]
    pub fn request_text(&self) -> String {
        mem_proto::armor::armor_device_request(&self.request())
    }

    /// Becomes the new device from the setup the main device showed.
    ///
    /// # Errors
    ///
    /// [`JoinError`] if the setup is not for this request or unusable.
    pub fn finish(&self, setup: &[u8], now_unix_ms: u64) -> Result<Agent, JoinError> {
        let s = DeviceSetup::decode(setup).map_err(|_| JoinError::NotForThisDevice)?;
        let state = self
            .mailbox
            .open_device_setup(&s.enc, &s.ciphertext, &self.nonce)
            .map_err(|_| JoinError::NotForThisDevice)?;
        let mut agent = Agent::restore(&state, now_unix_ms).map_err(|_| JoinError::Corrupt)?;
        let me = self.key.public();
        let certified = agent.own.cert.as_ref().is_some_and(|c| {
            c.device_pk == *me.as_bytes() && super::devices::cert_valid(&agent.identity.public(), c)
        });
        let listed = agent.own.list.as_ref().is_some_and(|l| {
            l.certs
                .iter()
                .skip(1)
                .any(|c| c.device_pk == *me.as_bytes())
        });
        if agent.is_main_device() || !certified || !listed {
            return Err(JoinError::Corrupt);
        }
        // Copies of the keys: a wrong setup leaves the request open for the right one.
        agent.own.key = Some(IdentityKey::from_seed(&self.key.to_seed()));
        agent.mailboxes = alloc::vec![HeldMailbox {
            serial: 1,
            created_unix_ms: now_unix_ms,
            retired_unix_ms: None,
            key: MailboxKey::from_seed(&self.mailbox.to_seed()),
        }];
        Ok(agent)
    }
}
