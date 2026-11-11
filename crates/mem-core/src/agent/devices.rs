//! Contacts with several devices, sans-IO.
//!
//! A contact's main device keeps the fields it always had on [`Contact`] (mailbox key, serial,
//! X-Wing key, what it was told). Its other devices are [`ContactDevice`]s, learned from the
//! contact's signed device list and their own key updates.
//!
//! - **Lists** are verified against the contact's identity (the list and every certificate) and
//!   replace an older serial only. A device missing from the newest list is retired: its keys go,
//!   copies queued for it are dropped, and its signatures are refused.
//! - **Messages signed by another device** carry its certificate (inner key 11); the signature is
//!   checked with that device key, and the device must be in the newest list.
//! - **Sending**: every bundle to the main device gets a copy per other device with a fresh key
//!   (learned within [`CONTACT_KEY_STALE_MS`]), sealed to that key and addressed to the device's
//!   own tag (`ContactSecret::device_tag`); copies are hidden outbox items pointing to the
//!   visible message, which counts as delivered at the first device's ack. Each copy carries
//!   the acks owed to that device and our key while that device has not confirmed it.

use alloc::vec::Vec;

use mem_crypto::purge::commit;
use mem_crypto::{IdentityPublic, MailboxPublic, SenderPurgeKey, XWingPublic};
use mem_proto::BundleKind;
use mem_proto::device::{
    DEVICE_CERT_DOMAIN, DEVICE_LIST_DOMAIN, DeviceCert, DeviceList, FEATURE_DEVICES,
};
use mem_proto::inner::{KeyUpdate, MAX_ACK_IDS};
use rand_core::CryptoRng;

use super::{Agent, CONTACT_KEY_STALE_MS, Contact, OutboxItem, Recipient, Sealing};
use crate::model::{ContactId, MsgId};

/// One of a contact's other devices (not the main one).
#[derive(Clone)]
pub(crate) struct ContactDevice {
    /// `BLAKE3(device key)[0..8]`.
    pub(crate) id: [u8; 8],
    /// Its certified Ed25519 key.
    pub(crate) key: IdentityPublic,
    /// Its current mailbox key, once announced, with serial and when we learned it.
    pub(crate) mailbox: Option<MailboxPublic>,
    pub(crate) mailbox_serial: u32,
    pub(crate) mailbox_learned_ms: u64,
    /// The X-Wing half of `mailbox`.
    pub(crate) xwing: Option<XWingPublic>,
    /// Highest serial of ours this device confirmed.
    pub(crate) told_serial: u32,
    /// Highest serial of our device list this device confirmed.
    pub(crate) told_list: u32,
    /// One of our own devices that our main device retired: it gets only the list that drops
    /// it, and is forgotten once it confirms that.
    pub(crate) retired: bool,
}

/// A certificate is the contact identity's signature over a device key.
pub(crate) fn cert_valid(identity: &IdentityPublic, cert: &DeviceCert) -> bool {
    cert.identity_pk == *identity.as_bytes()
        && identity
            .verify(DEVICE_CERT_DOMAIN, &[&cert.signed_bytes()], &cert.signature)
            .is_ok()
}

/// A list is the identity's, signed by it, and so is every certificate in it.
pub(crate) fn list_valid(identity: &IdentityPublic, list: &DeviceList) -> bool {
    list.identity_pk == *identity.as_bytes()
        && list.signed_bytes().is_ok_and(|b| {
            identity
                .verify(DEVICE_LIST_DOMAIN, &[&b], &list.signature)
                .is_ok()
        })
        && list.certs.iter().all(|c| cert_valid(identity, c))
}

/// The device id of a certificate's key.
pub(crate) fn cert_device_id(cert: &DeviceCert) -> Option<[u8; 8]> {
    IdentityPublic::from_bytes(&cert.device_pk)
        .ok()
        .map(|k| k.device_id())
}

impl Agent {
    /// How many devices `contact` has, the main one included (1 when it never announced others,
    /// or for an unknown contact).
    #[must_use]
    pub fn contact_devices(&self, contact: ContactId) -> usize {
        self.contacts
            .get(&contact)
            .map_or(1, |c| 1 + c.devices.len())
    }

    /// The newer of `contact`'s stored list and `offered` (verified), if any: what a message's
    /// certificate is checked against.
    pub(super) fn newest_list<'a>(
        contact: &'a Contact,
        offered: Option<&'a DeviceList>,
    ) -> Option<&'a DeviceList> {
        let offered = offered.filter(|l| list_valid(&contact.identity, l));
        match (&contact.device_list, offered) {
            (Some(held), Some(new)) if new.serial > held.serial => Some(new),
            (Some(held), _) => Some(held),
            (None, new) => new,
        }
    }

    /// The device id a message's certificate names, if it is valid and listed (newest list,
    /// `offered` included). `Err` refuses the message.
    pub(super) fn signing_device(
        contact: &Contact,
        cert: &DeviceCert,
        offered: Option<&DeviceList>,
    ) -> Result<(IdentityPublic, [u8; 8]), ()> {
        if !cert_valid(&contact.identity, cert) {
            return Err(());
        }
        let listed = Self::newest_list(contact, offered)
            .is_some_and(|l| l.certs.iter().any(|c| c.device_pk == cert.device_pk));
        let key = IdentityPublic::from_bytes(&cert.device_pk).map_err(|_| ())?;
        if listed {
            Ok((key, key.device_id()))
        } else {
            Err(())
        }
    }

    /// Takes a contact's newer, valid list: adds devices, keeps the keys of those still listed,
    /// retires the rest (their queued copies go). Our own list (from our main device) also
    /// becomes ours, never with a copy to this device; `true` if this device is no longer in it.
    pub(super) fn learn_device_list(&mut self, contact_id: ContactId, list: DeviceList) -> bool {
        let own = self.is_self(contact_id);
        let this = self.own.id();
        let Some(c) = self.contacts.get(&contact_id) else {
            return false;
        };
        if !list_valid(&c.identity, &list)
            || c.device_list
                .as_ref()
                .is_some_and(|l| l.serial >= list.serial)
        {
            return false;
        }
        let retired = own
            && !list
                .certs
                .iter()
                .any(|cert| this.is_some() && cert_device_id(cert) == this);
        if own {
            self.own.list = Some(list.clone());
        }
        let Some(c) = self.contacts.get_mut(&contact_id) else {
            return false;
        };
        // The first entry is the main device: its keys are the contact's own fields.
        let others: Vec<(IdentityPublic, [u8; 8])> = list
            .certs
            .iter()
            .skip(1)
            .filter_map(|cert| {
                IdentityPublic::from_bytes(&cert.device_pk)
                    .ok()
                    .map(|k| (k, k.device_id()))
            })
            .filter(|(_, id)| !own || Some(*id) != this)
            .collect();
        let old = core::mem::take(&mut c.devices);
        c.devices = others
            .into_iter()
            .map(|(key, id)| {
                old.iter()
                    .find(|d| d.id == id)
                    .cloned()
                    .unwrap_or(ContactDevice {
                        id,
                        key,
                        mailbox: None,
                        mailbox_serial: 0,
                        mailbox_learned_ms: 0,
                        xwing: None,
                        told_serial: 0,
                        told_list: 0,
                        retired: false,
                    })
            })
            .collect();
        c.device_list = Some(list);
        let kept: Vec<[u8; 8]> = c.devices.iter().map(|d| d.id).collect();
        self.outbox
            .retain(|_, i| i.contact != contact_id || i.device.is_none_or(|d| kept.contains(&d)));
        self.ack_from
            .retain(|_, (c, d)| *c != contact_id || kept.contains(d));
        retired
    }

    /// A key update from one of a contact's other devices: kept if its serial is newer.
    pub(super) fn learn_device_key(&mut self, contact: ContactId, device: [u8; 8], k: &KeyUpdate) {
        let now = self.now_unix_ms;
        let Some(d) = self
            .contacts
            .get_mut(&contact)
            .and_then(|c| c.devices.iter_mut().find(|d| d.id == device))
        else {
            return;
        };
        let xwing = k
            .xwing_pk
            .as_deref()
            .and_then(|b| XWingPublic::from_bytes(b).ok());
        if k.serial == d.mailbox_serial
            && d.xwing.is_none()
            && d.mailbox.is_some_and(|m| *m.as_bytes() == k.mailbox_pk)
        {
            d.xwing = xwing;
            return;
        }
        if d.mailbox.is_some() && k.serial <= d.mailbox_serial {
            return;
        }
        if let Ok(pk) = MailboxPublic::from_bytes(&k.mailbox_pk) {
            d.mailbox = Some(pk);
            d.mailbox_serial = k.serial;
            d.mailbox_learned_ms = now;
            d.xwing = xwing;
        }
    }

    /// The other devices of `contact` with a key fresh enough to seal to.
    pub(super) fn fresh_devices(&self, contact: &Contact) -> Vec<[u8; 8]> {
        contact
            .devices
            .iter()
            .filter(|d| {
                !d.retired
                    && d.mailbox.is_some()
                    && self.now_unix_ms.saturating_sub(d.mailbox_learned_ms) < CONTACT_KEY_STALE_MS
            })
            .map(|d| d.id)
            .collect()
    }

    /// Our current key and device list for `device` of `contact` (the main device when `None`)
    /// while it has not confirmed them. Names this device when it is not the main one, and
    /// always marks that we understand devices.
    pub(super) fn key_update_to(
        &self,
        contact: &Contact,
        device: Option<[u8; 8]>,
    ) -> Option<KeyUpdate> {
        let cur = self.current_mailbox();
        let (told_key, told_list) = match device {
            None => (
                contact.told_serial.min(contact.told_xwing),
                contact.told_list,
            ),
            Some(id) => contact
                .devices
                .iter()
                .find(|d| d.id == id)
                .map_or((u32::MAX, u32::MAX), |d| (d.told_serial, d.told_list)),
        };
        let list = self.own.list.as_ref().filter(|l| told_list < l.serial);
        (told_key < cur.serial || list.is_some()).then(|| KeyUpdate {
            mailbox_pk: *cur.key.public().as_bytes(),
            serial: cur.serial,
            xwing_pk: Some(cur.key.xwing_public().as_bytes().to_vec()),
            device_id: self.receiving_device(),
            device_list: list.cloned(),
            features: FEATURE_DEVICES,
        })
    }

    /// Serial of our device list, 0 without one.
    pub(crate) fn own_list_serial(&self) -> u32 {
        self.own.list.as_ref().map_or(0, |l| l.serial)
    }

    /// The key a copy for `device` is sealed to: its serial and when we learned it (the main
    /// device's when `None`); `None` if that device has no key.
    pub(super) fn target_key(contact: &Contact, device: Option<[u8; 8]>) -> Option<(u32, u64)> {
        match device {
            None => Some((contact.mailbox_serial, contact.mailbox_learned_ms)),
            Some(id) => contact
                .devices
                .iter()
                .find(|d| d.id == id && d.mailbox.is_some())
                .map(|d| (d.mailbox_serial, d.mailbox_learned_ms)),
        }
    }

    /// Acks owed to `contact` for messages from `device` (the main device when `None`).
    pub(super) fn owed_acks(&self, contact: ContactId, device: Option<[u8; 8]>) -> Vec<MsgId> {
        self.pending_acks
            .get(&contact)
            .map(|ids| {
                ids.iter()
                    .filter(|m| self.ack_from.get(m).map(|(_, d)| *d) == device)
                    .take(MAX_ACK_IDS)
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Hidden copies of a bundle for `contact`'s other devices with a fresh key, each pointing
    /// to `main` (the item the user sees, or the hidden item it copies).
    pub(super) fn queue_device_copies(
        &mut self,
        contact_id: ContactId,
        main: MsgId,
        kind: BundleKind,
        content_type: u64,
        content: &[u8],
        rng: &mut impl CryptoRng,
    ) {
        let Some(contact) = self.contacts.get(&contact_id) else {
            return;
        };
        for device in self.fresh_devices(contact) {
            self.queue_copy(
                contact_id,
                device,
                Some(main),
                kind,
                content_type,
                content,
                rng,
            );
        }
    }

    /// One hidden item sealed to `device` of `contact_id`.
    #[allow(
        clippy::too_many_arguments,
        reason = "one sealed copy needs all of these"
    )]
    pub(super) fn queue_copy(
        &mut self,
        contact_id: ContactId,
        device: [u8; 8],
        copy_of: Option<MsgId>,
        kind: BundleKind,
        content_type: u64,
        content: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Option<MsgId> {
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        let acks = self.owed_acks(contact_id, Some(device));
        let (contact, purge_key) = (self.contacts.get(&contact_id)?, self.purge_key.as_ref()?);
        let (sealed_serial, sealed_learned_ms) = Self::target_key(contact, Some(device))?;
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let token = purge_key.derive_token(&bundle_id);
        let commitment = commit(&token);
        let key_update = self.key_update_to(contact, Some(device));
        let key_serial = key_update.as_ref().map(|k| k.serial);
        let list_serial = key_update
            .as_ref()
            .and_then(|k| k.device_list.as_ref())
            .map(|l| l.serial);
        let (bundle, id) = self
            .seal(
                contact,
                Sealing {
                    kind,
                    content_type,
                    content: content.to_vec(),
                    ack_requested: Some(true),
                    acks: acks.iter().map(|m| m.0).collect(),
                    lifetime_s: self.message_lifetime_s,
                    key_update,
                    id: None,
                    recipient: Recipient::Device(device),
                },
                bundle_id,
                &token,
                commitment,
                rng,
            )
            .ok()?;
        self.outbox.insert(
            id,
            OutboxItem {
                contact: contact_id,
                bundle,
                created_unix_ms: self.now_unix_ms,
                acks,
                copies: self.spray_copies,
                courier_ok: false,
                key_serial,
                list_serial,
                sealed_serial,
                sealed_learned_ms,
                via_identity: false,
                content: Some((content_type, content.to_vec())),
                device: Some(device),
                copy_of,
            },
        );
        Some(id)
    }
}
