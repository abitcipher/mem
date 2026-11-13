//! Whole-state snapshot for local storage.
//!
//! The snapshot contains secrets. It is only ever handed to `mem-runtime`, which encrypts it
//! before writing; it never crosses FFI.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::{ClaId, LEGACY_DEFAULT_FLOOR, PrivacyGrade};
use mem_crypto::backup::{PhraseForm, RecoverySecret};
use mem_crypto::{
    ContactSecret, IdentityKey, IdentityPublic, MailboxKey, MailboxPublic, PairingNonce,
    PurgeCommitment, PurgeToken, SenderPurgeKey,
};
use mem_proto::Bundle;
use mem_proto::attachment::AttachmentRef;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::agent::{
    Agent, Attachment, Contact, ContactRequest, DeliveredEntry, EmailAccount, Group, HeldMailbox,
    Message, OutboxItem, PartialBlob, PendingIntro, Pull, Tombstone,
};
use crate::model::{
    AttestLogEntry, AttestReturn, ChannelUse, ConsentBasis, ConsentRule, ContactId, DeliveryStatus,
    GroupId, GroupStatus, MessageKind, MsgDirection, MsgId, PrivacyLogEntry, RequestId, RuleId,
    TrustLevel,
};

/// Snapshot format version.
const VERSION: u8 = 1;

/// Failure to restore a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SnapshotError {
    /// Not a decodable snapshot.
    #[error("corrupt snapshot")]
    Corrupt,
    /// Written by a newer build.
    #[error("unsupported snapshot version {0}")]
    Version(u8),
}

/// `(purge commitment, contact, message_id, expires_unix_ms)`.
type DeliveredDto = ([u8; 16], [u8; 32], [u8; 16], u64);

#[derive(Serialize, Deserialize)]
struct StateV1 {
    version: u8,
    /// The identity key, on the main device; another device stores `identity_pk` instead.
    identity_seed: Option<[u8; 32]>,
    /// The current mailbox key (kept so older builds can still read the state).
    mailbox_seed: [u8; 32],
    /// The keyring `(serial, created_unix_ms, retired_unix_ms, seed)`; absent before
    /// rotation existed.
    #[serde(default)]
    mailboxes: Vec<(u32, u64, Option<u64>, [u8; 32])>,
    pairing_nonce: [u8; 16],
    display_name: Option<String>,
    created_unix_ms: u64,
    next_rule: u64,
    contacts: Vec<ContactDto>,
    messages: Vec<MessageDto>,
    outbox: Vec<OutboxDto>,
    seen: Vec<[u8; 16]>,
    rules: Vec<RuleDto>,
    privacy_log: Vec<LogDto>,
    // Added with purge tokens; absent in snapshots written by v0.1.0.
    #[serde(default)]
    purge_key: Option<[u8; 32]>,
    /// `(commitment, token, expires_unix_ms)`. Only read, from snapshots written before the
    /// tombstones moved to the database; always written empty now.
    #[serde(default)]
    tombstones: Vec<([u8; 16], [u8; 16], u64)>,
    /// Channels the user turned off.
    #[serde(default)]
    disabled_channels: Vec<String>,
    /// Channels that are off by default and the user turned on (BLE). Absent in
    /// older snapshots, so those channels stay off after an update.
    #[serde(default)]
    enabled_channels: Vec<String>,
    /// Acks owed per contact: `(contact, message_ids)`.
    #[serde(default)]
    pending_acks: Vec<([u8; 32], Vec<[u8; 16]>)>,
    /// Delivered index: `(purge commitment, contact, message_id, expires_unix_ms)`.
    #[serde(default)]
    delivered: Vec<DeliveredDto>,
    /// Lifetime of new messages in seconds (user setting); 0 means the default.
    #[serde(default)]
    message_lifetime_s: u32,
    /// Recovery secret, if backups are on.
    #[serde(default)]
    recovery_secret: Option<[u8; 16]>,
    /// Phrase form: 0 English, 1 proquint.
    #[serde(default)]
    recovery_form: u8,
    /// Courier storage limit in bytes; absent in snapshots from before courier mode,
    /// which start with it off (it was never offered to them).
    #[serde(default)]
    courier_limit: Option<u64>,
    /// Floor for new contacts (user setting); absent in snapshots from before the setting, which
    /// keep the old default P2.
    #[serde(default)]
    default_floor: Option<u8>,
    /// The pairing nonce before the last new code, the opt-in setting, requests in
    /// quarantine, blocked senders and our requests waiting for proof of work.
    #[serde(default)]
    previous_pairing_nonce: Option<[u8; 16]>,
    #[serde(default)]
    accept_intros: bool,
    #[serde(default)]
    requests: Vec<RequestDto>,
    #[serde(default)]
    blocked: Vec<[u8; 32]>,
    #[serde(default)]
    pending_intros: Vec<PendingIntroDto>,
    /// Groups.
    #[serde(default)]
    groups: Vec<GroupDto>,
    /// The last day fetched completely, purge tokens not yet handed to the hub, and
    /// outbox items the hub already holds.
    #[serde(default)]
    hub_last_fetch_epoch: u32,
    #[serde(default)]
    hub_tokens: Vec<[u8; 16]>,
    #[serde(default)]
    hub_uploaded: Vec<[u8; 16]>,
    #[serde(default)]
    hub_url: Option<String>,
    #[serde(default)]
    push_endpoint: Option<String>,
    /// The mail account (its app password included: the snapshot is encrypted at rest)
    /// and messages already sent by email.
    #[serde(default)]
    email_account: Option<EmailAccountDto>,
    #[serde(default)]
    email_sent: Vec<[u8; 16]>,
    /// The verified hub list (re-verified at load) and this device's pick.
    #[serde(default)]
    hub_list: Option<Vec<u8>>,
    #[serde(default)]
    hub_pick: u32,
    /// "Only my operator's hubs".
    #[serde(default)]
    own_operator_only: bool,
    /// Acks owed to a contact's other device (message, contact, device), and the
    /// device of delivered entries from one (commitment, device).
    #[serde(default)]
    ack_from: Vec<([u8; 16], [u8; 32], [u8; 8])>,
    #[serde(default)]
    delivered_devices: Vec<([u8; 16], [u8; 8])>,
    /// Attachment blobs stored on this device, and incoming ones still missing parts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blobs: Vec<[u8; 32]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    partials: Vec<PartialDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pulls: Vec<PullDto>,
    /// The identity's public key (another device), this device's key, its
    /// certificate and our device list.
    #[serde(default)]
    identity_pk: Option<[u8; 32]>,
    #[serde(default)]
    device_seed: Option<[u8; 32]>,
    #[serde(default)]
    device_cert: Option<Vec<u8>>,
    #[serde(default)]
    own_list: Option<Vec<u8>>,
    /// Main device: the contacts each other device has been sent.
    #[serde(default)]
    own_sync: Vec<([u8; 8], Vec<[u8; 32]>)>,
    /// Attestations made here, `(at_unix_ms, rp_origin, subject_label, returned)`
    /// with returned 0 sent, 1 copied. Left out when empty, so state without attestations
    /// keeps its earlier bytes (published backup vectors).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attest_log: Vec<(u64, String, String, u8)>,
}

#[derive(Serialize, Deserialize)]
struct EmailAccountDto {
    address: String,
    imap_host: String,
    imap_port: u16,
    smtp_host: String,
    smtp_port: u16,
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct GroupDto {
    id: [u8; 16],
    name: String,
    admin: [u8; 32],
    epoch: u32,
    cards: Vec<Vec<u8>>,
    members: Vec<[u8; 32]>,
    /// 0 invited, 1 joining, 2 member, 3 left, 4 removed.
    status: u8,
    invited: Vec<[u8; 32]>,
    created_unix_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct RequestDto {
    id: u64,
    identity: [u8; 32],
    mailbox: [u8; 32],
    their_nonce: [u8; 16],
    our_nonce: [u8; 16],
    display_name: Option<String>,
    note: Option<String>,
    received_unix_ms: u64,
    via: String,
    message: [u8; 16],
    commitment: [u8; 16],
}

#[derive(Serialize, Deserialize)]
struct PendingIntroDto {
    message: [u8; 16],
    contact: [u8; 32],
    bundle_id: [u8; 16],
    dst_tag: [u8; 16],
    week: u32,
    recipient_nonce: [u8; 16],
    note: Option<String>,
}

impl Drop for StateV1 {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.identity_seed.zeroize();
        self.device_seed.zeroize();
        self.mailbox_seed.zeroize();
        for (_, _, _, seed) in &mut self.mailboxes {
            seed.zeroize();
        }
        if let Some(k) = &mut self.purge_key {
            k.zeroize();
        }
        if let Some(r) = &mut self.recovery_secret {
            r.zeroize();
        }
        for c in &mut self.contacts {
            c.secret.zeroize();
        }
        for a in self
            .messages
            .iter_mut()
            .flat_map(|m| m.attachments.iter_mut())
        {
            a.key.zeroize();
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ContactDto {
    identity: [u8; 32],
    mailbox: [u8; 32],
    secret: [u8; 32],
    display_name: Option<String>,
    trust: u8,
    floor: u8,
    added_unix_ms: u64,
    /// Key rotation state; absent before rotation existed.
    #[serde(default)]
    mailbox_serial: u32,
    #[serde(default)]
    mailbox_learned_ms: Option<u64>,
    #[serde(default)]
    told_serial: Option<u32>,
    #[serde(default)]
    requested: bool,
    /// Only a fellow group member.
    #[serde(default)]
    group_only: bool,
    /// The address the user typed.
    #[serde(default)]
    email: Option<String>,
    /// The name the user gave the contact.
    #[serde(default)]
    alias: Option<String>,
    /// The contact's X-Wing key (re-validated at load) and what they confirmed.
    #[serde(default)]
    xwing: Option<Vec<u8>>,
    #[serde(default)]
    told_xwing: u32,
    /// The contact's other devices and its newest device list (re-verified at load).
    #[serde(default)]
    devices: Vec<ContactDeviceDto>,
    #[serde(default)]
    device_list: Option<Vec<u8>>,
    #[serde(default)]
    told_list: u32,
}

#[derive(Serialize, Deserialize)]
struct ContactDeviceDto {
    id: [u8; 8],
    key: [u8; 32],
    mailbox: Option<[u8; 32]>,
    mailbox_serial: u32,
    mailbox_learned_ms: u64,
    xwing: Option<Vec<u8>>,
    told_serial: u32,
    #[serde(default)]
    told_list: u32,
    #[serde(default)]
    retired: bool,
}

#[derive(Serialize, Deserialize)]
struct MessageDto {
    id: [u8; 16],
    contact: [u8; 32],
    outgoing: bool,
    /// 0 text, 1 our contact request.
    #[serde(default)]
    kind: u8,
    text: String,
    created_unix_ms: u64,
    status: u8,
    channels: Vec<(String, u8, u64)>,
    /// Its group, and for our group copies the first copy's id.
    #[serde(default)]
    group: Option<[u8; 16]>,
    #[serde(default)]
    fanout: Option<[u8; 16]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<AttachmentDto>,
}

/// An attached file of a message, with the file key (the snapshot is encrypted).
#[derive(Serialize, Deserialize)]
struct AttachmentDto {
    address: [u8; 32],
    size: u64,
    key: [u8; 32],
    name: String,
    media_type: String,
    #[serde(default)]
    preview: Option<Vec<u8>>,
    /// Ours: the part messages carrying the blob.
    #[serde(default)]
    parts: Vec<[u8; 16]>,
}

/// A large blob being pulled: how far it got.
#[derive(Serialize, Deserialize)]
struct PullDto {
    address: [u8; 32],
    contact: [u8; 32],
    total: u64,
    have: u64,
}

/// Parts of an incoming blob still waiting for the rest.
#[derive(Serialize, Deserialize)]
struct PartialDto {
    address: [u8; 32],
    contact: [u8; 32],
    total: u64,
    parts: Vec<(u64, Vec<u8>)>,
    first_unix_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct OutboxDto {
    message: [u8; 16],
    contact: [u8; 32],
    bundle: Vec<u8>,
    created_unix_ms: u64,
    /// Acks piggybacked in this bundle.
    #[serde(default)]
    acks: Vec<[u8; 16]>,
    /// Spray copies still held; absent in older snapshots.
    #[serde(default)]
    copies: Option<u8>,
    /// The user allowed couriers for this message.
    #[serde(default)]
    courier_ok: bool,
    /// Our key serial announced in it, and the contact key it is sealed to.
    #[serde(default)]
    key_serial: Option<u32>,
    #[serde(default)]
    sealed_serial: u32,
    #[serde(default)]
    sealed_learned_ms: Option<u64>,
    #[serde(default)]
    via_identity: bool,
    /// Content type and bytes of a hidden group item.
    #[serde(default)]
    content: Option<(u64, Vec<u8>)>,
    /// The contact's other device it is sealed to, and the item it copies.
    #[serde(default)]
    device: Option<[u8; 8]>,
    #[serde(default)]
    copy_of: Option<[u8; 16]>,
    #[serde(default)]
    list_serial: Option<u32>,
}

#[derive(Serialize, Deserialize)]
struct RuleDto {
    id: u64,
    contact: Option<[u8; 32]>,
    cla: String,
    max_grade: u8,
    until_unix_ms: Option<u64>,
    created_unix_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct LogDto {
    at_unix_ms: u64,
    contact: [u8; 32],
    message: [u8; 16],
    cla: String,
    grade: u8,
    floor: u8,
    basis: (u8, u64),
}

const fn grade_to_u8(g: PrivacyGrade) -> u8 {
    match g {
        PrivacyGrade::P0 => 0,
        PrivacyGrade::P1 => 1,
        PrivacyGrade::P2 => 2,
        PrivacyGrade::P3 => 3,
        PrivacyGrade::P4 => 4,
        PrivacyGrade::P5 => 5,
    }
}

fn grade_from_u8(v: u8) -> Result<PrivacyGrade, SnapshotError> {
    Ok(match v {
        0 => PrivacyGrade::P0,
        1 => PrivacyGrade::P1,
        2 => PrivacyGrade::P2,
        3 => PrivacyGrade::P3,
        4 => PrivacyGrade::P4,
        5 => PrivacyGrade::P5,
        _ => return Err(SnapshotError::Corrupt),
    })
}

/// A stored floor. P5 is never a floor; an old snapshot holding one gets P4 instead.
fn floor_from_u8(v: u8) -> Result<PrivacyGrade, SnapshotError> {
    let g = grade_from_u8(v)?;
    Ok(if g.allowed_as_floor() {
        g
    } else {
        PrivacyGrade::P4
    })
}

fn cla(s: &str) -> Result<ClaId, SnapshotError> {
    ClaId::parse(s).ok_or(SnapshotError::Corrupt)
}

const fn status_to_u8(s: DeliveryStatus) -> u8 {
    match s {
        DeliveryStatus::Queued => 0,
        DeliveryStatus::Sent => 1,
        DeliveryStatus::Failed => 2,
        DeliveryStatus::Received => 3,
        DeliveryStatus::Delivered => 4,
    }
}

impl Agent {
    /// One contact's record, for an own-device sync item.
    pub(crate) fn contact_record(&self, id: ContactId) -> Option<Vec<u8>> {
        let c = self.contacts.get(&id)?;
        minicbor_serde::to_vec(contact_dto(id, c)).ok()
    }

    /// A contact from a sync record (keys and lists re-validated).
    pub(crate) fn contact_from_record(&self, bytes: &[u8]) -> Option<(ContactId, Contact)> {
        let dto: ContactDto = minicbor_serde::from_slice(bytes).ok()?;
        contact_from_dto(&dto, self.now_unix_ms).ok()
    }

    /// What a new device starts from: this state without anything private to
    /// this device (identity key, mailbox keys, messages, outbox, mail account, push, recovery
    /// secret, requests) and with the self contact as the new device sees it: our main device
    /// (this one) with its current key, our other devices, our list.
    pub(crate) fn device_setup_state(
        &self,
        cert: &mem_proto::device::DeviceCert,
        new: [u8; 8],
    ) -> Zeroizing<Vec<u8>> {
        let full = self.snapshot();
        let Ok(mut s) = minicbor_serde::from_slice::<StateV1>(&full) else {
            return Zeroizing::new(Vec::new());
        };
        let me = self.self_id();
        let cur = self.current_mailbox();
        s.identity_seed = None;
        s.identity_pk = Some(*self.identity.public().as_bytes());
        s.device_seed = None;
        s.device_cert = Some(cert.encode());
        s.mailbox_seed = [0; 32];
        s.mailboxes.clear();
        s.messages.clear();
        s.outbox.clear();
        s.seen.clear();
        s.privacy_log.clear();
        s.purge_key = None;
        s.pending_acks.clear();
        s.delivered.clear();
        s.delivered_devices.clear();
        s.ack_from.clear();
        s.recovery_secret = None;
        s.accept_intros = false;
        s.requests.clear();
        s.pending_intros.clear();
        s.hub_last_fetch_epoch = 0;
        s.hub_tokens.clear();
        s.hub_uploaded.clear();
        s.push_endpoint = None;
        s.email_account = None;
        s.email_sent.clear();
        s.own_sync.clear();
        s.attest_log.clear();
        if let Some(own) = s.contacts.iter_mut().find(|c| c.identity == me.0) {
            own.mailbox = *cur.key.public().as_bytes();
            own.mailbox_serial = cur.serial;
            own.mailbox_learned_ms = Some(self.now_unix_ms);
            own.xwing = Some(cur.key.xwing_public().as_bytes().to_vec());
            // We know the new device's first key (serial 1) from its request, not its X-Wing half.
            own.told_serial = Some(1);
            own.told_xwing = 0;
            own.told_list = self.own_list_serial();
            own.devices.retain(|d| d.id != new);
            for d in &mut own.devices {
                // It has told none of them its keys yet.
                d.told_serial = 0;
                d.told_list = 0;
            }
        }
        Zeroizing::new(minicbor_serde::to_vec(&s).unwrap_or_default())
    }

    /// Serializes all persistent state.
    ///
    /// # Security
    ///
    /// The result contains private keys and contact secrets. Encrypt it before writing and
    /// never pass it across FFI.
    #[must_use]
    pub fn snapshot(&self) -> Zeroizing<Vec<u8>> {
        self.state_bytes(true)
    }

    /// The snapshot for a backup: with `history` false, delivered and received messages are left
    /// out, but the outbox and its messages, seen ids, tombstones and the privacy log stay.
    ///
    /// # Security
    ///
    /// As [`Agent::snapshot`].
    #[must_use]
    pub fn snapshot_for_backup(&self, history: bool) -> Zeroizing<Vec<u8>> {
        self.state_bytes(history)
    }

    #[allow(clippy::too_many_lines, reason = "straight-line field mapping")]
    fn state_bytes(&self, history: bool) -> Zeroizing<Vec<u8>> {
        let state = StateV1 {
            version: VERSION,
            identity_seed: self.identity.key().map(|k| *k.to_seed()),
            mailbox_seed: *self.current_mailbox().key.to_seed(),
            mailboxes: self
                .mailboxes
                .iter()
                .map(|h| {
                    (
                        h.serial,
                        h.created_unix_ms,
                        h.retired_unix_ms,
                        *h.key.to_seed(),
                    )
                })
                .collect(),
            pairing_nonce: *self.pairing_nonce.as_bytes(),
            display_name: self.display_name.clone(),
            created_unix_ms: self.created_unix_ms,
            next_rule: self.next_rule,
            contacts: self
                .contacts
                .iter()
                .map(|(id, c)| contact_dto(*id, c))
                .collect(),
            messages: self
                .messages
                .iter()
                .filter(|m| {
                    history
                        || self.outbox.contains_key(&m.id)
                        || self.pending_intros.contains_key(&m.id)
                })
                .map(|m| MessageDto {
                    id: m.id.0,
                    contact: m.contact.0,
                    outgoing: m.direction == MsgDirection::Outgoing,
                    kind: match m.kind {
                        MessageKind::Text => 0,
                        MessageKind::ContactRequest => 1,
                    },
                    text: m.text.clone(),
                    created_unix_ms: m.created_unix_ms,
                    status: status_to_u8(m.status),
                    channels: m
                        .channels
                        .iter()
                        .map(|u| {
                            (
                                String::from(u.cla.as_str()),
                                grade_to_u8(u.grade),
                                u.at_unix_ms,
                            )
                        })
                        .collect(),
                    group: m.group.map(|g| g.0),
                    fanout: m.fanout.map(|f| f.0),
                    attachments: m
                        .attachments
                        .iter()
                        .map(|a| AttachmentDto {
                            address: a.reference.address,
                            size: a.reference.size,
                            key: a.reference.key,
                            name: a.reference.name.clone(),
                            media_type: a.reference.media_type.clone(),
                            preview: a.reference.preview.clone(),
                            parts: a.parts.iter().map(|p| p.0).collect(),
                        })
                        .collect(),
                })
                .collect(),
            outbox: self
                .outbox
                .iter()
                .map(|(id, o)| OutboxDto {
                    message: id.0,
                    contact: o.contact.0,
                    bundle: o.bundle.encode(),
                    created_unix_ms: o.created_unix_ms,
                    acks: o.acks.iter().map(|m| m.0).collect(),
                    copies: Some(o.copies),
                    courier_ok: o.courier_ok,
                    key_serial: o.key_serial,
                    sealed_serial: o.sealed_serial,
                    sealed_learned_ms: Some(o.sealed_learned_ms),
                    via_identity: o.via_identity,
                    content: o.content.clone(),
                    device: o.device,
                    copy_of: o.copy_of.map(|m| m.0),
                    list_serial: o.list_serial,
                })
                .collect(),
            seen: self.seen.iter().map(|m| m.0).collect(),
            rules: self
                .rules
                .iter()
                .map(|r| RuleDto {
                    id: r.id.0,
                    contact: r.contact.map(|c| c.0),
                    cla: String::from(r.cla.as_str()),
                    max_grade: grade_to_u8(r.max_grade),
                    until_unix_ms: r.until_unix_ms,
                    created_unix_ms: r.created_unix_ms,
                })
                .collect(),
            privacy_log: self
                .privacy_log
                .iter()
                .map(|e| LogDto {
                    at_unix_ms: e.at_unix_ms,
                    contact: e.contact.0,
                    message: e.message.0,
                    cla: String::from(e.cla.as_str()),
                    grade: grade_to_u8(e.grade),
                    floor: grade_to_u8(e.floor),
                    basis: match e.basis {
                        ConsentBasis::Once => (0, 0),
                        ConsentBasis::NewRule(r) => (1, r.0),
                        ConsentBasis::ExistingRule(r) => (2, r.0),
                    },
                })
                .collect(),
            purge_key: self.purge_key.as_ref().map(|k| *k.expose()),
            // Tombstones live in the database now.
            tombstones: Vec::new(),
            disabled_channels: self
                .disabled
                .iter()
                .filter(|c| c.on_by_default())
                .map(|c| String::from(c.as_str()))
                .collect(),
            enabled_channels: ClaId::ALL
                .into_iter()
                .filter(|c| !c.on_by_default() && !self.disabled.contains(c))
                .map(|c| String::from(c.as_str()))
                .collect(),
            pending_acks: self
                .pending_acks
                .iter()
                .map(|(c, ids)| (c.0, ids.iter().map(|m| m.0).collect()))
                .collect(),
            delivered: self
                .delivered
                .iter()
                .map(|(c, d)| (*c.as_bytes(), d.contact.0, d.message.0, d.expires_unix_ms))
                .collect(),
            blobs: self.blobs.iter().copied().collect(),
            pulls: self
                .pulls
                .iter()
                .map(|(address, p)| PullDto {
                    address: *address,
                    contact: p.contact.0,
                    total: p.total,
                    have: p.have,
                })
                .collect(),
            partials: self
                .partials
                .iter()
                .map(|(address, p)| PartialDto {
                    address: *address,
                    contact: p.contact.0,
                    total: p.total,
                    parts: p.parts.iter().map(|(o, b)| (*o, b.clone())).collect(),
                    first_unix_ms: p.first_unix_ms,
                })
                .collect(),
            delivered_devices: self
                .delivered
                .iter()
                .filter_map(|(c, d)| d.device.map(|dev| (*c.as_bytes(), dev)))
                .collect(),
            ack_from: self
                .ack_from
                .iter()
                .map(|(m, (c, d))| (m.0, c.0, *d))
                .collect(),
            message_lifetime_s: self.message_lifetime_s,
            default_floor: Some(grade_to_u8(self.default_floor)),
            courier_limit: Some(self.courier_limit),
            recovery_secret: self.recovery.as_ref().map(|r| *r.expose()),
            recovery_form: match self.recovery_form {
                PhraseForm::English => 0,
                PhraseForm::Proquint => 1,
            },
            previous_pairing_nonce: self.previous_pairing_nonce.map(|n| *n.as_bytes()),
            accept_intros: self.accept_intros,
            hub_url: self.hub_url.clone(),
            push_endpoint: self.push_endpoint.clone(),
            email_account: self.email_account.as_ref().map(|a| EmailAccountDto {
                address: a.address.clone(),
                imap_host: a.imap_host.clone(),
                imap_port: a.imap_port,
                smtp_host: a.smtp_host.clone(),
                smtp_port: a.smtp_port,
                username: a.username.clone(),
                password: String::from(a.password.as_str()),
            }),
            hub_list: self.hub_list.as_ref().and_then(|l| l.encode().ok()),
            hub_pick: self.hub_pick,
            own_operator_only: self.own_operator_only,
            identity_pk: Some(*self.identity.public().as_bytes()),
            device_seed: self.own.key.as_ref().map(|k| *k.to_seed()),
            device_cert: self
                .own
                .cert
                .as_ref()
                .map(mem_proto::device::DeviceCert::encode),
            own_list: self.own.list.as_ref().and_then(|l| l.encode().ok()),
            own_sync: self
                .own_sync
                .iter()
                .map(|(d, ids)| (*d, ids.iter().map(|c| c.0).collect()))
                .collect(),
            attest_log: self
                .attest_log
                .iter()
                .map(|e| {
                    let how = u8::from(e.returned == AttestReturn::Copied);
                    (
                        e.at_unix_ms,
                        e.rp_origin.clone(),
                        e.subject_label.clone(),
                        how,
                    )
                })
                .collect(),
            email_sent: self
                .email
                .sent
                .iter()
                .filter(|m| self.outbox.contains_key(m))
                .map(|m| m.0)
                .collect(),
            hub_last_fetch_epoch: self.hub.last_fetch_epoch,
            hub_tokens: self.hub.tokens.iter().copied().collect(),
            hub_uploaded: self
                .hub
                .uploaded
                .iter()
                .filter(|m| self.outbox.contains_key(m))
                .map(|m| m.0)
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|(id, g)| GroupDto {
                    id: id.0,
                    name: g.name.clone(),
                    admin: g.admin,
                    epoch: g.epoch,
                    cards: g.cards.clone(),
                    members: g.members.clone(),
                    status: match g.status {
                        GroupStatus::Invited => 0,
                        GroupStatus::Joining => 1,
                        GroupStatus::Member => 2,
                        GroupStatus::Left => 3,
                        GroupStatus::Removed => 4,
                    },
                    invited: g.invited.clone(),
                    created_unix_ms: g.created_unix_ms,
                })
                .collect(),
            requests: self
                .requests
                .iter()
                .map(|r| RequestDto {
                    id: r.id.0,
                    identity: *r.identity.as_bytes(),
                    mailbox: *r.mailbox.as_bytes(),
                    their_nonce: *r.their_nonce.as_bytes(),
                    our_nonce: *r.our_nonce.as_bytes(),
                    display_name: r.display_name.clone(),
                    note: r.note.clone(),
                    received_unix_ms: r.received_unix_ms,
                    via: String::from(r.via.as_str()),
                    message: r.message.0,
                    commitment: *r.commitment.as_bytes(),
                })
                .collect(),
            blocked: self.blocked.clone(),
            pending_intros: self
                .pending_intros
                .iter()
                .map(|(m, p)| PendingIntroDto {
                    message: m.0,
                    contact: p.contact.0,
                    bundle_id: p.bundle_id,
                    dst_tag: p.dst_tag,
                    week: p.week,
                    recipient_nonce: p.recipient_nonce,
                    note: p.note.clone(),
                })
                .collect(),
        };
        // Serializing plain data into a Vec cannot fail; an empty result would be rejected by
        // `restore` rather than silently accepted.
        Zeroizing::new(minicbor_serde::to_vec(&state).unwrap_or_default())
    }

    /// Restores an agent from [`Agent::snapshot`] output.
    ///
    /// # Errors
    ///
    /// [`SnapshotError`] if the data is corrupt or from an unsupported version.
    #[allow(clippy::too_many_lines, reason = "straight-line field mapping")]
    pub fn restore(bytes: &[u8], now_unix_ms: u64) -> Result<Self, SnapshotError> {
        let s: StateV1 = minicbor_serde::from_slice(bytes).map_err(|_| SnapshotError::Corrupt)?;
        if s.version != VERSION {
            return Err(SnapshotError::Version(s.version));
        }
        // The main device has the identity key; another device its public half.
        let own_identity = match (&s.identity_seed, &s.identity_pk) {
            (Some(seed), _) => crate::agent::OwnIdentity::main(IdentityKey::from_seed(seed)),
            (None, Some(pk)) => crate::agent::OwnIdentity::other(
                IdentityPublic::from_bytes(pk).map_err(|_| SnapshotError::Corrupt)?,
            ),
            (None, None) => return Err(SnapshotError::Corrupt),
        };
        let own = crate::agent::OwnDevices {
            key: s.device_seed.as_ref().map(IdentityKey::from_seed),
            cert: s
                .device_cert
                .as_deref()
                .and_then(|b| mem_proto::device::DeviceCert::decode(b).ok()),
            // Verified again: only a list our identity signed is kept.
            list: s
                .own_list
                .as_deref()
                .and_then(|b| mem_proto::device::DeviceList::decode(b).ok())
                .filter(|l| crate::agent::device_list_valid(&own_identity.public(), l)),
        };
        let mut contacts = BTreeMap::new();
        for c in &s.contacts {
            let (id, contact) = contact_from_dto(c, now_unix_ms)?;
            contacts.insert(id, contact);
        }
        let mut messages = Vec::with_capacity(s.messages.len());
        for m in &s.messages {
            let mut channels = Vec::with_capacity(m.channels.len());
            for (c, g, at) in &m.channels {
                channels.push(ChannelUse {
                    cla: cla(c)?,
                    grade: grade_from_u8(*g)?,
                    at_unix_ms: *at,
                });
            }
            messages.push(Message {
                id: MsgId(m.id),
                contact: ContactId(m.contact),
                direction: if m.outgoing {
                    MsgDirection::Outgoing
                } else {
                    MsgDirection::Incoming
                },
                kind: if m.kind == 1 {
                    MessageKind::ContactRequest
                } else {
                    MessageKind::Text
                },
                text: m.text.clone(),
                created_unix_ms: m.created_unix_ms,
                status: match m.status {
                    0 => DeliveryStatus::Queued,
                    1 => DeliveryStatus::Sent,
                    2 => DeliveryStatus::Failed,
                    3 => DeliveryStatus::Received,
                    4 => DeliveryStatus::Delivered,
                    _ => return Err(SnapshotError::Corrupt),
                },
                channels,
                group: m.group.map(GroupId),
                fanout: m.fanout.map(MsgId),
                attachments: m
                    .attachments
                    .iter()
                    .map(|a| Attachment {
                        reference: AttachmentRef {
                            address: a.address,
                            size: a.size,
                            key: a.key,
                            name: a.name.clone(),
                            media_type: a.media_type.clone(),
                            preview: a.preview.clone(),
                        },
                        parts: a.parts.iter().copied().map(MsgId).collect(),
                    })
                    .collect(),
            });
        }
        let mut outbox = BTreeMap::new();
        for o in &s.outbox {
            // Expired bundles are dropped on load rather than failing the whole restore.
            let age = now_unix_ms.saturating_sub(o.created_unix_ms);
            if let Ok(bundle) = Bundle::decode(&o.bundle).and_then(|b| b.with_age_ms(age)) {
                let bundle = bundle.with_age_ms(0).map_err(|_| SnapshotError::Corrupt)?;
                outbox.insert(
                    MsgId(o.message),
                    OutboxItem {
                        contact: ContactId(o.contact),
                        bundle,
                        created_unix_ms: o.created_unix_ms,
                        acks: o.acks.iter().map(|m| MsgId(*m)).collect(),
                        copies: o
                            .copies
                            .unwrap_or(crate::agent::DEFAULT_COPIES)
                            .clamp(1, mem_proto::envelope::MAX_COPIES),
                        courier_ok: o.courier_ok,
                        key_serial: o.key_serial,
                        sealed_serial: o.sealed_serial,
                        sealed_learned_ms: o.sealed_learned_ms.unwrap_or(now_unix_ms),
                        via_identity: o.via_identity,
                        content: o.content.clone(),
                        device: o.device,
                        copy_of: o.copy_of.map(MsgId),
                        list_serial: o.list_serial,
                    },
                );
            }
        }
        let mut rules = Vec::with_capacity(s.rules.len());
        for r in &s.rules {
            rules.push(ConsentRule {
                id: RuleId(r.id),
                contact: r.contact.map(ContactId),
                cla: cla(&r.cla)?,
                max_grade: grade_from_u8(r.max_grade)?,
                until_unix_ms: r.until_unix_ms,
                created_unix_ms: r.created_unix_ms,
            });
        }
        let mut privacy_log = Vec::with_capacity(s.privacy_log.len());
        for e in &s.privacy_log {
            privacy_log.push(PrivacyLogEntry {
                at_unix_ms: e.at_unix_ms,
                contact: ContactId(e.contact),
                message: MsgId(e.message),
                cla: cla(&e.cla)?,
                grade: grade_from_u8(e.grade)?,
                floor: grade_from_u8(e.floor)?,
                basis: match e.basis {
                    (0, _) => ConsentBasis::Once,
                    (1, r) => ConsentBasis::NewRule(RuleId(r)),
                    (2, r) => ConsentBasis::ExistingRule(RuleId(r)),
                    _ => return Err(SnapshotError::Corrupt),
                },
            });
        }
        let mut requests = Vec::with_capacity(s.requests.len());
        for r in &s.requests {
            requests.push(ContactRequest {
                id: RequestId(r.id),
                identity: IdentityPublic::from_bytes(&r.identity)
                    .map_err(|_| SnapshotError::Corrupt)?,
                mailbox: MailboxPublic::from_bytes(&r.mailbox)
                    .map_err(|_| SnapshotError::Corrupt)?,
                their_nonce: PairingNonce::from_bytes(r.their_nonce),
                our_nonce: PairingNonce::from_bytes(r.our_nonce),
                display_name: r.display_name.clone(),
                note: r.note.clone(),
                received_unix_ms: r.received_unix_ms,
                via: cla(&r.via)?,
                message: MsgId(r.message),
                commitment: PurgeCommitment::from_bytes(r.commitment),
            });
        }
        // Volatile ids restart after a restore; request ids are persisted, so start above them.
        let next_id = requests.iter().map(|r| r.id.0 + 1).max().unwrap_or(1);
        let partials = s
            .partials
            .iter()
            .map(|p| {
                (
                    p.address,
                    PartialBlob {
                        contact: ContactId(p.contact),
                        total: p.total,
                        parts: p.parts.iter().cloned().collect(),
                        first_unix_ms: p.first_unix_ms,
                    },
                )
            })
            .collect();
        // A pull that finished but was not checked before the restart starts over.
        let pulls = s
            .pulls
            .iter()
            .map(|p| {
                (
                    p.address,
                    Pull {
                        contact: ContactId(p.contact),
                        total: p.total,
                        have: if p.have >= p.total { 0 } else { p.have },
                        peer: None,
                        until: 0,
                        via: None,
                        checking: false,
                    },
                )
            })
            .collect();
        Ok(Self {
            blobs: s.blobs.iter().copied().collect(),
            partials,
            pulls,
            offer_limits: crate::agent::call_limits(),
            identity: own_identity,
            own,
            own_sync: s
                .own_sync
                .iter()
                .map(|(d, ids)| (*d, ids.iter().map(|c| ContactId(*c)).collect()))
                .collect(),
            attest_log: s
                .attest_log
                .iter()
                .map(|(at, origin, label, how)| AttestLogEntry {
                    at_unix_ms: *at,
                    rp_origin: origin.clone(),
                    subject_label: label.clone(),
                    returned: if *how == 1 {
                        AttestReturn::Copied
                    } else {
                        AttestReturn::Sent
                    },
                })
                .collect(),
            mailboxes: if s.mailboxes.is_empty() {
                // State from before rotation: the one key becomes serial 1, created now.
                alloc::vec![HeldMailbox {
                    serial: 1,
                    created_unix_ms: now_unix_ms,
                    retired_unix_ms: None,
                    key: MailboxKey::from_seed(&s.mailbox_seed),
                }]
            } else {
                s.mailboxes
                    .iter()
                    .map(|(serial, created, retired, seed)| HeldMailbox {
                        serial: *serial,
                        created_unix_ms: *created,
                        retired_unix_ms: *retired,
                        key: MailboxKey::from_seed(seed),
                    })
                    .collect()
            },
            pairing_nonce: PairingNonce::from_bytes(s.pairing_nonce),
            display_name: s.display_name.clone(),
            contacts,
            messages,
            outbox,
            seen: s.seen.iter().map(|m| MsgId(*m)).collect::<VecDeque<_>>(),
            rules,
            privacy_log,
            purge_key: s.purge_key.as_ref().map(SenderPurgeKey::from_bytes),
            tombstones: s
                .tombstones
                .iter()
                .map(|(c, t, exp)| {
                    (
                        PurgeCommitment::from_bytes(*c),
                        Tombstone {
                            token: PurgeToken::from_bytes(*t),
                            expires_unix_ms: *exp,
                        },
                    )
                })
                .collect(),
            // Anything still in an old snapshot moves to the database at the next load.
            migrate_tombstones: !s.tombstones.is_empty(),
            // Unknown channel names (from a newer build) are ignored, not fatal.
            disabled: s
                .disabled_channels
                .iter()
                .filter_map(|c| ClaId::parse(c))
                .chain(ClaId::ALL.into_iter().filter(|c| {
                    !c.on_by_default() && !s.enabled_channels.iter().any(|e| e == c.as_str())
                }))
                .collect(),
            pending_acks: s
                .pending_acks
                .iter()
                .map(|(c, ids)| (ContactId(*c), ids.iter().map(|m| MsgId(*m)).collect()))
                .collect(),
            delivered: s
                .delivered
                .iter()
                .map(|(c, contact, m, exp)| {
                    (
                        PurgeCommitment::from_bytes(*c),
                        DeliveredEntry {
                            contact: ContactId(*contact),
                            message: MsgId(*m),
                            expires_unix_ms: *exp,
                            device: s
                                .delivered_devices
                                .iter()
                                .find(|(k, _)| k == c)
                                .map(|(_, d)| *d),
                        },
                    )
                })
                .collect(),
            message_lifetime_s: crate::agent::clamp_lifetime(s.message_lifetime_s),
            default_floor: match s.default_floor {
                Some(f) => floor_from_u8(f)?,
                // Written before the setting existed: keep that install's old default.
                None => LEGACY_DEFAULT_FLOOR,
            },
            // The index arrives separately from the courier database (`Input::CourierLoaded`).
            courier: crate::courier::CourierStore::default(),
            courier_limit: s
                .courier_limit
                .unwrap_or(0)
                .min(crate::courier::MAX_COURIER_BYTES),
            recovery: s.recovery_secret.map(RecoverySecret::from_bytes),
            recovery_form: if s.recovery_form == 1 {
                PhraseForm::Proquint
            } else {
                PhraseForm::English
            },
            pending_recovery: None,
            next_rule: s.next_rule,
            created_unix_ms: s.created_unix_ms,
            now_unix_ms,
            offers: BTreeMap::new(),
            ops: BTreeMap::new(),
            sends: BTreeMap::new(),
            ack_due: BTreeMap::new(),
            next_id,
            peers: BTreeMap::new(),
            advertised_epoch: mem_crypto::contact::epoch_for_unix_ms(now_unix_ms),
            previous_pairing_nonce: s.previous_pairing_nonce.map(PairingNonce::from_bytes),
            accept_intros: s.accept_intros,
            requests,
            blocked: s.blocked.clone(),
            pending_intros: s
                .pending_intros
                .iter()
                .map(|p| {
                    (
                        MsgId(p.message),
                        PendingIntro {
                            contact: ContactId(p.contact),
                            bundle_id: p.bundle_id,
                            dst_tag: p.dst_tag,
                            week: p.week,
                            recipient_nonce: p.recipient_nonce,
                            note: p.note.clone(),
                        },
                    )
                })
                .collect(),
            solving: alloc::collections::BTreeSet::new(),
            groups: s
                .groups
                .iter()
                .map(|g| {
                    Ok((
                        GroupId(g.id),
                        Group {
                            name: g.name.clone(),
                            admin: g.admin,
                            epoch: g.epoch,
                            cards: g.cards.clone(),
                            members: g.members.clone(),
                            status: match g.status {
                                0 => GroupStatus::Invited,
                                1 => GroupStatus::Joining,
                                2 => GroupStatus::Member,
                                3 => GroupStatus::Left,
                                4 => GroupStatus::Removed,
                                _ => return Err(SnapshotError::Corrupt),
                            },
                            invited: g.invited.clone(),
                            created_unix_ms: g.created_unix_ms,
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
            hub: {
                let mut h = crate::agent::HubState::default();
                h.last_fetch_epoch = s.hub_last_fetch_epoch;
                h.tokens = s.hub_tokens.iter().copied().collect();
                h.uploaded = s.hub_uploaded.iter().map(|m| MsgId(*m)).collect();
                h
            },
            hub_url: s.hub_url.clone(),
            push_endpoint: s.push_endpoint.clone(),
            email_account: s.email_account.as_ref().map(|a| EmailAccount {
                address: a.address.clone(),
                imap_host: a.imap_host.clone(),
                imap_port: a.imap_port,
                smtp_host: a.smtp_host.clone(),
                smtp_port: a.smtp_port,
                username: a.username.clone(),
                password: zeroize::Zeroizing::new(a.password.clone()),
            }),
            // Decoded again here; checked against the operator keys when the runtime sets them.
            hub_list: s
                .hub_list
                .as_deref()
                .and_then(|b| mem_proto::hublist::HubList::decode(b).ok()),
            hub_pick: s.hub_pick,
            own_operator_only: s.own_operator_only,
            ack_from: s
                .ack_from
                .iter()
                .map(|(m, c, d)| (MsgId(*m), (ContactId(*c), *d)))
                .collect(),
            operator_keys: Vec::new(),
            spray_copies: crate::agent::DEFAULT_COPIES,
            email: crate::agent::EmailState {
                online: false,
                sent: s.email_sent.iter().map(|m| MsgId(*m)).collect(),
            },
        })
    }
}

/// A contact as the snapshot stores it.
fn contact_dto(id: ContactId, c: &Contact) -> ContactDto {
    ContactDto {
        identity: id.0,
        mailbox: *c.mailbox.as_bytes(),
        secret: *c.secret.expose(),
        display_name: c.display_name.clone(),
        trust: match c.trust {
            TrustLevel::Remote => 0,
            TrustLevel::MetInPerson => 1,
            TrustLevel::Introduced => 2,
        },
        floor: grade_to_u8(c.floor),
        added_unix_ms: c.added_unix_ms,
        mailbox_serial: c.mailbox_serial,
        mailbox_learned_ms: Some(c.mailbox_learned_ms),
        told_serial: Some(c.told_serial),
        requested: c.requested,
        group_only: c.group_only,
        email: c.email.clone(),
        alias: c.alias.clone(),
        xwing: c.xwing.as_ref().map(|x| x.as_bytes().to_vec()),
        told_xwing: c.told_xwing,
        devices: c
            .devices
            .iter()
            .map(|d| ContactDeviceDto {
                id: d.id,
                key: *d.key.as_bytes(),
                mailbox: d.mailbox.map(|m| *m.as_bytes()),
                mailbox_serial: d.mailbox_serial,
                mailbox_learned_ms: d.mailbox_learned_ms,
                xwing: d.xwing.as_ref().map(|x| x.as_bytes().to_vec()),
                told_serial: d.told_serial,
                told_list: d.told_list,
                retired: d.retired,
            })
            .collect(),
        device_list: c.device_list.as_ref().and_then(|l| l.encode().ok()),
        told_list: c.told_list,
    }
}

/// A contact from its snapshot record (keys and lists re-validated).
fn contact_from_dto(
    c: &ContactDto,
    now_unix_ms: u64,
) -> Result<(ContactId, Contact), SnapshotError> {
    let identity = IdentityPublic::from_bytes(&c.identity).map_err(|_| SnapshotError::Corrupt)?;
    let mailbox = MailboxPublic::from_bytes(&c.mailbox).map_err(|_| SnapshotError::Corrupt)?;
    Ok((
        ContactId(c.identity),
        Contact {
            identity,
            mailbox,
            secret: ContactSecret::from_bytes(&c.secret),
            display_name: c.display_name.clone(),
            trust: match c.trust {
                1 => TrustLevel::MetInPerson,
                2 => TrustLevel::Introduced,
                _ => TrustLevel::Remote,
            },
            floor: floor_from_u8(c.floor)?,
            added_unix_ms: c.added_unix_ms,
            mailbox_serial: c.mailbox_serial,
            // Before rotation keys never changed: count from the upgrade.
            mailbox_learned_ms: c.mailbox_learned_ms.unwrap_or(now_unix_ms),
            // Before rotation the contact knew our only key, now serial 1.
            told_serial: c.told_serial.unwrap_or(1),
            requested: c.requested,
            group_only: c.group_only,
            email: c.email.clone(),
            alias: c.alias.clone(),
            xwing: c
                .xwing
                .as_deref()
                .and_then(|b| mem_crypto::XWingPublic::from_bytes(b).ok()),
            told_xwing: c.told_xwing,
            devices: c
                .devices
                .iter()
                .filter_map(|d| {
                    Some(crate::agent::ContactDevice {
                        id: d.id,
                        key: IdentityPublic::from_bytes(&d.key).ok()?,
                        mailbox: d.mailbox.and_then(|m| MailboxPublic::from_bytes(&m).ok()),
                        mailbox_serial: d.mailbox_serial,
                        mailbox_learned_ms: d.mailbox_learned_ms,
                        xwing: d
                            .xwing
                            .as_deref()
                            .and_then(|b| mem_crypto::XWingPublic::from_bytes(b).ok()),
                        told_serial: d.told_serial,
                        told_list: d.told_list,
                        retired: d.retired,
                    })
                })
                .collect(),
            // Verified again: only a list the contact's identity signed is kept.
            device_list: c
                .device_list
                .as_deref()
                .and_then(|b| mem_proto::device::DeviceList::decode(b).ok())
                .filter(|l| crate::agent::device_list_valid(&identity, l)),
            told_list: c.told_list,
        },
    ))
}

#[cfg(test)]
mod migration_tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use crate::agent::{Input, Output, StorageOp};
    use crate::courier::StoreWrite;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    /// Older snapshots carry tombstones; they move to the database.
    #[test]
    fn old_snapshot_tombstones_move_to_the_database() {
        let mut rng = ChaCha20Rng::from_seed([4; 32]);
        let agent = Agent::new(1_000, &mut rng);
        let mut s: StateV1 = minicbor_serde::from_slice(&agent.snapshot()).unwrap();
        s.tombstones = alloc::vec![([1; 16], [2; 16], u64::MAX)];
        let old = minicbor_serde::to_vec(&s).unwrap();
        let mut restored = Agent::restore(&old, 1_000).unwrap();
        assert!(restored.is_purged(&[1; 16]));
        let out = restored.handle(
            Input::StoreLoaded {
                courier: alloc::vec![],
                tombstones: alloc::vec![],
            },
            &mut rng,
        );
        assert!(out.iter().any(|o| matches!(
            o,
            Output::Persist(StorageOp::Store(w))
                if w.iter().any(|x| matches!(x, StoreWrite::TombstonePut(t) if t.commitment == [1; 16]))
        )));
        // Written once: the snapshot no longer carries them.
        let again: StateV1 = minicbor_serde::from_slice(&restored.snapshot()).unwrap();
        assert_eq!(again.tombstones, alloc::vec![]);
    }

    /// Opt-in channels (BLE) stay off for state written before they existed,
    /// and a user's choice survives a restart.
    #[test]
    fn opt_in_channels_stay_off_until_turned_on() {
        let mut rng = ChaCha20Rng::from_seed([5; 32]);
        let mut agent = Agent::new(1_000, &mut rng);
        assert!(!agent.channel_enabled(ClaId::Ble));
        // State from an older build: only `disabled_channels`, which never listed BLE.
        let mut s: StateV1 = minicbor_serde::from_slice(&agent.snapshot()).unwrap();
        s.disabled_channels = alloc::vec![];
        s.enabled_channels = alloc::vec![];
        let old = Agent::restore(&minicbor_serde::to_vec(&s).unwrap(), 1_000).unwrap();
        assert!(!old.channel_enabled(ClaId::Ble));
        assert!(old.channel_enabled(ClaId::Iroh));

        agent.handle(
            Input::SetChannelEnabled {
                cla: ClaId::Ble,
                enabled: true,
            },
            &mut rng,
        );
        agent.handle(
            Input::SetChannelEnabled {
                cla: ClaId::Iroh,
                enabled: false,
            },
            &mut rng,
        );
        let back = Agent::restore(&agent.snapshot(), 1_000).unwrap();
        assert!(back.channel_enabled(ClaId::Ble));
        assert!(!back.channel_enabled(ClaId::Iroh));
    }

    /// The attestation log survives a restart, starts empty for older state, and
    /// keeps at most `MAX_ATTEST_LOG` entries.
    #[test]
    fn attestation_log_persists_and_is_bounded() {
        let mut rng = ChaCha20Rng::from_seed([6; 32]);
        let mut agent = Agent::new(1_000, &mut rng);
        agent.handle(
            Input::Tick {
                wall_unix_ms: 5_000,
            },
            &mut rng,
        );
        let entry = |label: &str, returned| AttestLogEntry {
            at_unix_ms: 0,
            rp_origin: String::from("https://basa.ua"),
            subject_label: String::from(label),
            returned,
        };
        agent.handle(
            Input::AttestationReturned(entry("first", AttestReturn::Sent)),
            &mut rng,
        );
        agent.handle(
            Input::AttestationReturned(entry("second", AttestReturn::Copied)),
            &mut rng,
        );
        let back = Agent::restore(&agent.snapshot(), 1_000).unwrap();
        let log = back.attest_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].subject_label, "second");
        assert_eq!(log[0].returned, AttestReturn::Copied);
        assert_eq!(log[1].at_unix_ms, 5_000);

        let mut s: StateV1 = minicbor_serde::from_slice(&agent.snapshot()).unwrap();
        s.attest_log.clear();
        let old = Agent::restore(&minicbor_serde::to_vec(&s).unwrap(), 1_000).unwrap();
        assert_eq!(old.attest_log(), alloc::vec![]);

        let too_long = String::from("x").repeat(33);
        let out = agent.handle(
            Input::AttestationReturned(entry(&too_long, AttestReturn::Sent)),
            &mut rng,
        );
        assert!(matches!(out.as_slice(), [Output::Rejected(_)]));
        for _ in 0..crate::MAX_ATTEST_LOG {
            agent.handle(
                Input::AttestationReturned(entry("n", AttestReturn::Sent)),
                &mut rng,
            );
        }
        assert_eq!(agent.attest_log().len(), crate::MAX_ATTEST_LOG);
    }
}
