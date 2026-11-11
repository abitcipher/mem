//! The sans-IO agent.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

mod devices;
pub(crate) use devices::{ContactDevice, list_valid as device_list_valid};
pub use own::{DeviceJoin, JoinError};
pub(crate) use own::{Identity as OwnIdentity, OwnDevices};
mod email;
mod group;
mod hub;
mod intro;
mod own;

pub(crate) use intro::{ContactRequest, PendingIntro};
pub use intro::{
    INTRO_COPIES, INTRO_LIFETIME_S, MAX_BLOCKED, MAX_REQUESTS, POW_CHECKS_PER_ENCOUNTER, PowJob,
};

use mem_cla::{ClaId, DEFAULT_FLOOR, PrivacyGrade};
use mem_crypto::backup::{PhraseForm, RecoverySecret};
use mem_crypto::contact::epoch_for_unix_ms;
use mem_crypto::intro::{intro_key_id, intro_tag, week_for_unix_ms};
use mem_crypto::mailbox::{identity_key_id, open_with_identity};
use mem_crypto::purge::commit;
use mem_crypto::{
    ContactSecret, Direction, IdentityKey, IdentityPublic, MailboxKey, MailboxPublic, PairingNonce,
    PurgeCommitment, PurgeToken, SUITE_ID, SUITE_XWING, SenderPurgeKey, XWingPublic,
};
use mem_proto::card::{CARD_SIGNATURE_DOMAIN, CardFields, MAX_DISPLAY_NAME_LEN, PairingCard};
use mem_proto::encounter::{
    BundleEntry, ENCOUNTER_VERSION, EncounterMsg, MAX_OFFER_TAGS, MAX_PURGE_TOKENS, MAX_TAGS,
};
use mem_proto::envelope::{MAX_HOPS, aad_for};
use mem_proto::hub::{Crossing, HubOrigin};
use mem_proto::inner::{
    CONTENT_TYPE_ACK, CONTENT_TYPE_KEY_UPDATE, CONTENT_TYPE_TEXT, Inner, InnerFields, KeyUpdate,
    MAX_TEXT_LEN, SIGNATURE_DOMAIN, decode_ack_ids, decode_text, encode_ack_ids, encode_text, pad,
    unpad,
};
use mem_proto::payload::Payload;
use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, Pow, RendezvousTag};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

use crate::courier::{
    CourierClass, CourierKey, CourierMeta, CourierRefusal, CourierStore, DEFAULT_COURIER_BYTES,
    EncounterUsage, MAX_COURIER_BYTES, SPRAY_PER_BATCH_BYTES, StoreWrite, TombstoneRow,
};
use crate::model::AttestLogEntry;
use crate::model::{
    ChannelOffer, ChannelOption, ChannelUse, ConsentBasis, ConsentRecord, ConsentRule,
    ConsentScope, ContactId, ContactView, DeliveryStatus, GroupId, MessageKind, MessageView,
    MsgDirection, MsgId, OfferId, OpId, PeerHandle, PrivacyLogEntry, RejectReason, RequestId,
    RuleId, TrustLevel,
};
pub use email::EmailAccount;
pub(crate) use email::EmailState;
pub(crate) use group::Group;
use group::GroupContent;
pub use group::{MAX_GROUPS, MAX_INVITATIONS};
pub(crate) use hub::HubState;
use mem_crypto::bloom::SaltedBloom;
use mem_proto::group::{
    CONTENT_TYPE_GROUP_REPLY, CONTENT_TYPE_GROUP_STATE, CONTENT_TYPE_GROUP_TEXT,
};

/// Default lifetime of a message bundle, and so how long an unconfirmed message is retried: the
/// 30-day cap of the envelope. A user setting within [`MIN_MESSAGE_LIFETIME_S`]..=this.
pub const MESSAGE_LIFETIME_S: u32 = mem_proto::envelope::MAX_LIFETIME_S;
/// Shortest message lifetime the user may choose: one day.
pub const MIN_MESSAGE_LIFETIME_S: u32 = 24 * 60 * 60;
/// Lifetime of a standalone ack bundle.
pub const ACK_LIFETIME_S: u32 = 7 * 24 * 60 * 60;
/// A standalone ack goes out when no message has carried the pending acks for this long.
pub const ACK_DELAY_MS: u64 = 2_000;
/// Within one encounter, an unconfirmed message is sent again after this long.
pub const RETRANSMIT_AFTER_MS: u64 = 30_000;
/// Most sends of one message within one encounter; then it waits for the next encounter.
pub const MAX_SENDS_PER_ENCOUNTER: u8 = 3;
/// Most acks owed to one contact; older ones are dropped and healed by retransmission.
pub const MAX_PENDING_ACKS: usize = 256;
/// Most attestations kept in the attestation log; the oldest go first.
pub const MAX_ATTEST_LOG: usize = 256;
/// How many received message ids are remembered for deduplication.
pub const SEEN_CAPACITY: usize = 10_000;
/// Maximum purge tombstones kept: about 1 MiB at 32 bytes each.
pub const TOMBSTONE_CAPACITY: usize = 32_768;
/// Most bundles one peer may hand us in one encounter. Direct delivery between two
/// people needs far fewer; a peer that sends more is cut off.
pub const PEER_BUNDLE_QUOTA: u32 = 256;
/// Advertised tag lists are padded with random decoys to at least this many entries,
/// so peers learn only a coarse bound on the number of contacts.
pub const MIN_ADVERTISED_TAGS: usize = 8;
/// Tombstones outlive any bundle they could match: the 30-day maximum lifetime.
const TOMBSTONE_LIFETIME_MS: u64 = mem_proto::envelope::MAX_LIFETIME_S as u64 * 1000;

/// Error from a channel adapter, reported back via [`Input::TransmitResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaError {
    /// The user cancelled (closed the QR, dismissed the share sheet).
    Cancelled,
    /// The channel failed.
    Failed,
}

/// Everything the agent reacts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Injected wall time. Send before every other input.
    Tick {
        /// Unix time in ms.
        wall_unix_ms: u64,
    },
    /// The user typed a message.
    UserSend {
        /// Recipient.
        contact: ContactId,
        /// Text.
        text: String,
    },
    /// The user picked a channel for a queued message.
    RequestTransmit {
        /// Message.
        message: MsgId,
        /// Channel.
        cla: ClaId,
    },
    /// The user answered a [`ChannelOffer`].
    ConsentGiven(ConsentRecord),
    /// Outcome of an [`Output::Transmit`].
    TransmitResult {
        /// Operation.
        op: OpId,
        /// Outcome.
        result: Result<(), ClaError>,
    },
    /// Bundle bytes arrived over a channel.
    Received {
        /// Channel.
        cla: ClaId,
        /// Encoded bundle (untrusted).
        bytes: Vec<u8>,
    },
    /// A pairing card arrived over a channel.
    ImportCard {
        /// Channel.
        cla: ClaId,
        /// Encoded card (untrusted).
        bytes: Vec<u8>,
    },
    /// Generate a fresh pairing nonce.
    NewPairingCode,
    /// Change our display name (`None` removes it).
    SetDisplayName(Option<String>),
    /// An attestation was returned to its relying party; recorded in the
    /// attestation log.
    AttestationReturned(AttestLogEntry),
    /// Change a contact's privacy floor.
    SetFloor {
        /// Contact.
        contact: ContactId,
        /// New floor.
        floor: PrivacyGrade,
    },
    /// Change the floor given to contacts added from now on (P0–P4; default P2). Existing
    /// contacts keep theirs.
    SetDefaultFloor {
        /// New default floor.
        floor: PrivacyGrade,
    },
    /// Revoke a standing consent rule.
    RevokeRule(RuleId),
    /// Turn a channel on or off ("Channels").
    SetChannelEnabled {
        /// Channel.
        cla: ClaId,
        /// New state.
        enabled: bool,
    },
    /// Set the lifetime of new messages, and so how long they are retried. Clamped to
    /// [`MIN_MESSAGE_LIFETIME_S`]..=[`MESSAGE_LIFETIME_S`].
    SetMessageLifetime {
        /// Seconds.
        seconds: u32,
    },
    /// Draw a fresh recovery secret to show. It stays pending, in memory
    /// only, until [`Input::ConfirmRecoveryPhrase`]: until then backups stay off, or keep the old
    /// phrase. Replaces any earlier pending secret.
    NewRecoveryPhrase {
        /// How the phrase is written.
        form: PhraseForm,
    },
    /// The user typed back the pending phrase correctly: it becomes the recovery phrase and
    /// backups are on.
    ConfirmRecoveryPhrase,
    /// The user left without confirming: forget the pending phrase.
    DiscardRecoveryPhrase,
    /// What the runtime loaded from its database at start or after a restore: the
    /// courier index and the purge tombstones.
    StoreLoaded {
        /// Carried bundles, metadata only.
        courier: Vec<CourierMeta>,
        /// Purge tombstones.
        tombstones: Vec<TombstoneRow>,
    },
    /// Change the courier storage limit in bytes; 0 turns courier mode off. Clamped to
    /// [`MAX_COURIER_BYTES`]; carried bundles beyond the new limit are evicted.
    SetCourierLimit {
        /// Bytes.
        bytes: u64,
    },
    /// Change how the recovery phrase is written; the secret stays.
    SetPhraseForm {
        /// Form.
        form: PhraseForm,
    },
    /// A peer connected over a network channel; the encounter begins.
    EncounterStarted {
        /// Runtime-assigned handle.
        peer: PeerHandle,
        /// Channel.
        cla: ClaId,
    },
    /// A peer sent an encounter message (untrusted; already bounded by its decoder).
    EncounterMessage {
        /// Peer.
        peer: PeerHandle,
        /// Message.
        msg: EncounterMsg,
    },
    /// The connection to a peer ended.
    EncounterEnded {
        /// Peer.
        peer: PeerHandle,
    },
    /// Ask the owner of a card to become a contact, without meeting.
    SendContactRequest {
        /// Their card (`spec/card.cddl`).
        card: Vec<u8>,
        /// A short note shown with the request (at most 280 bytes).
        note: Option<String>,
    },
    /// The runtime solved a [`PowJob`].
    PowSolved {
        /// The job's request.
        message: MsgId,
        /// A nonce meeting the difficulty.
        nonce: [u8; 8],
    },
    /// Accept a contact request.
    ApproveRequest(RequestId),
    /// Delete a contact request.
    RejectRequest(RequestId),
    /// Delete a contact request and ignore that sender's requests from now on.
    BlockRequest(RequestId),
    /// Advertise our intro tag in encounters, so nearby devices can deliver requests
    /// (off by default).
    SetAcceptRequestsNearby {
        /// On or off.
        enabled: bool,
    },
    /// Create a group with us as admin and invite these contacts.
    CreateGroup {
        /// Name, 1..=64 bytes.
        name: String,
        /// Contacts to invite (at most 15).
        members: Vec<ContactId>,
    },
    /// Admin: invite another contact.
    InviteToGroup {
        /// Group.
        group: GroupId,
        /// Contact.
        contact: ContactId,
    },
    /// Admin: remove a member, or withdraw an invitation.
    RemoveFromGroup {
        /// Group.
        group: GroupId,
        /// Member.
        member: ContactId,
    },
    /// Join a group we were invited to.
    AcceptGroupInvitation(GroupId),
    /// Decline an invitation.
    DeclineGroupInvitation(GroupId),
    /// Leave a group (as admin: end it for everyone).
    LeaveGroup(GroupId),
    /// Make a fellow group member a full contact on our side. Their 1:1 messages are shown, and
    /// ours reach them, once they do the same.
    AddMemberAsContact(ContactId),
    /// Send a text to every other member.
    SendGroupText {
        /// Group.
        group: GroupId,
        /// Text.
        text: String,
    },
    /// Set the hub's address (`wss://host[:port]/v1`; `ws://` only to this machine, for
    /// development), or `None` to forget it.
    SetHubUrl(Option<String>),
    /// "Only my operator's hubs": when on, no upload may cross to partner
    /// operators' hubs. Off by default.
    SetOwnOperatorOnly {
        /// On or off.
        enabled: bool,
    },
    /// Add, replace or remove the mail account the email bridge uses.
    SetEmailAccount(Option<EmailAccount>),
    /// Main device: add the device whose request (`spec/device.cddl`) was scanned in person;
    /// answered with [`Output::DeviceSetupReady`].
    ApproveDevice {
        /// The request's bytes.
        request: Vec<u8>,
    },
    /// Main device: retire one of our other devices.
    RetireDevice {
        /// Its id.
        device: [u8; 8],
    },
    /// Remove a contact: its pairing secret, conversation, queued messages and rules go. If it is
    /// in a group with us it stays only a fellow group member (its 1:1 messages are refused).
    RemoveContact(ContactId),
    /// Set or clear a contact's email address (typed by the user; stays local).
    SetContactEmail {
        /// Contact.
        contact: ContactId,
        /// Address, or `None`.
        address: Option<String>,
    },
    /// The runtime's mail client logged in.
    EmailConnected,
    /// The mail client lost its connection.
    EmailDisconnected,
    /// Set the push endpoint the hub wakes us at (UnifiedPush), or `None` to stop
    /// wake-ups. `https://`, or `http://` to this machine only.
    SetPushEndpoint(Option<String>),
    /// The operator keys built into the app that may sign hub lists. Volatile: the
    /// runtime sets them at every start.
    SetOperatorKeys(Vec<[u8; 32]>),
    /// A hub list arrived (from a hub, or built into the app): untrusted until verified.
    HubListReceived {
        /// `spec/hublist.cddl` bytes.
        bytes: Vec<u8>,
    },
    /// The runtime connected to the hub and got its `welcome`.
    HubConnected,
    /// The hub connection ended.
    HubDisconnected,
    /// The hub sent everything stored for the last [`Output::HubFetch`].
    HubFetchDone,
    /// Bundles from the hub (untrusted; mostly other people's, by prefix), each with
    /// where the hub says it came from (another operator's hub is grade P3).
    HubBundles {
        /// Encoded bundles and their origin.
        bundles: Vec<(Vec<u8>, HubOrigin)>,
    },
}

/// Storage operations for the runtime. v0.1 persists whole snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageOp {
    /// Write [`Agent::snapshot`].
    SaveSnapshot,
    /// Apply these database changes (courier store, tombstones), in order, in one transaction.
    Store(Vec<StoreWrite>),
}

/// Everything the agent asks the outside world to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// Send these bytes over the channel. Only produced for an authorized send.
    Transmit {
        /// Operation, to quote in [`Input::TransmitResult`].
        op: OpId,
        /// Message.
        message: MsgId,
        /// Channel.
        cla: ClaId,
        /// For network channels, the connected peer to send to.
        peer: Option<PeerHandle>,
        /// Encoded bundle.
        bytes: Vec<u8>,
        /// Direct-delivery proof for the peer; only for sends to a peer.
        proof: Option<[u8; 16]>,
        /// For the hub only: whether partner operators' hubs may carry it.
        crossing: Option<Crossing>,
    },
    /// Hand a carried bundle to a peer: the runtime reads its bytes from the
    /// courier database, sets these mutable envelope fields and sends it in a `bundles` message.
    Forward {
        /// Peer.
        peer: PeerHandle,
        /// Which carried bundle.
        key: CourierKey,
        /// `hops_remaining` for the copy sent.
        hops: u8,
        /// `copies` for the copy sent.
        copies: u8,
        /// `age_ms` for the copy sent.
        age_ms: u64,
    },
    /// The user allowed couriers to carry this message; it goes with the next
    /// courier met.
    CourierQueued {
        /// Message.
        message: MsgId,
    },
    /// Send an encounter message to a peer.
    Encounter {
        /// Peer.
        peer: PeerHandle,
        /// Message.
        msg: EncounterMsg,
    },
    /// Close the connection to a peer.
    EndEncounter {
        /// Peer.
        peer: PeerHandle,
    },
    /// The set of contacts reachable through connected peers changed.
    ReachabilityChanged,
    /// Show this sealed device setup (`spec/device.cddl`) as an animated QR code (frame object
    /// kind 3) to the new device.
    DeviceSetupReady {
        /// Encoded `device_setup`.
        setup: Vec<u8>,
    },
    /// Our main device retired this device: the app should delete its data.
    DeviceRetired,
    /// Contacts were added or removed by our main device.
    ContactsChanged,
    /// Persist state.
    Persist(StorageOp),
    /// A message was received and stored.
    Deliver {
        /// Contact.
        contact: ContactId,
        /// Message.
        message: MsgId,
    },
    /// A queued message was created.
    Queued {
        /// Contact.
        contact: ContactId,
        /// Message.
        message: MsgId,
    },
    /// The chosen channel is below the floor; the user must decide.
    ConsentRequired(ChannelOffer),
    /// A message status changed.
    StatusChanged {
        /// Message.
        message: MsgId,
        /// New status.
        status: DeliveryStatus,
    },
    /// A contact was added or re-paired.
    ContactPaired {
        /// Contact.
        contact: ContactId,
        /// Whether it already existed.
        updated: bool,
        /// The same pairing as before and nothing that needs checking changed (its card was read
        /// again): no short authentication string to compare.
        unchanged: bool,
    },
    /// An input was refused.
    Rejected(RejectReason),
    /// Solve this proof of work off the agent's thread and answer with [`Input::PowSolved`].
    SolvePow(PowJob),
    /// A contact request arrived in quarantine.
    ContactRequest {
        /// Request.
        request: RequestId,
    },
    /// A group invitation arrived.
    GroupInvitation {
        /// Group.
        group: GroupId,
    },
    /// A group's state or our standing in it changed.
    GroupChanged {
        /// Group.
        group: GroupId,
    },
    /// A group message arrived.
    GroupMessage {
        /// Group.
        group: GroupId,
        /// Message.
        message: MsgId,
    },
    /// Ask the hub for everything under these tag prefixes, decoys included.
    /// Answer with [`Input::HubBundles`] and then [`Input::HubFetchDone`].
    HubFetch {
        /// 2-byte prefixes, sorted.
        prefixes: Vec<[u8; 2]>,
    },
    /// Present these purge tokens to the hub.
    HubPurge {
        /// Tokens.
        tokens: Vec<[u8; 16]>,
    },
    /// Ask the hub to wake this endpoint for these prefixes.
    HubWakeRegister {
        /// Push endpoint URL.
        endpoint: String,
        /// The live set's padded prefixes.
        prefixes: Vec<[u8; 2]>,
    },
    /// Ask the connected hub for the signed hub list.
    HubGetList,
    /// The hubs to try changed ([`Agent::hub_candidates`]); reconnect if needed.
    HubCandidatesChanged,
    /// Withdraw a wake-up registration.
    HubWakeUnregister {
        /// Push endpoint URL.
        endpoint: String,
    },
    /// Send this bundle by email to `to`; report with [`Input::TransmitResult`].
    EmailSend {
        /// Operation.
        op: OpId,
        /// Message (or the standalone ack's id).
        message: MsgId,
        /// The contact's address.
        to: String,
        /// Encoded bundle.
        bytes: Vec<u8>,
    },
    /// The user allowed this one message to go as plain text (P5, `Once`): hand
    /// `text` to another app, then report with [`Input::TransmitResult`]. Content leaves the core
    /// unencrypted only here.
    PlaintextReady {
        /// Operation.
        op: OpId,
        /// Message.
        message: MsgId,
        /// The message text, exactly as it will be exposed.
        text: String,
    },
}

impl Output {
    /// Someone we asked approved us: an existing contact, nothing to compare.
    const fn approved(contact: ContactId) -> Self {
        Self::ContactPaired {
            contact,
            updated: true,
            unchanged: false,
        }
    }
}

/// A checked pairing card before import ([`Agent::card_preview`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardPreview {
    /// Grouped fingerprint of the card's identity.
    pub fingerprint: String,
    /// The name the card carries (unverified).
    pub display_name: Option<String>,
    /// Already a contact (not only a fellow group member).
    pub known: bool,
}

pub(crate) struct Contact {
    pub(crate) identity: IdentityPublic,
    pub(crate) mailbox: MailboxPublic,
    pub(crate) secret: ContactSecret,
    pub(crate) display_name: Option<String>,
    pub(crate) trust: TrustLevel,
    pub(crate) floor: PrivacyGrade,
    pub(crate) added_unix_ms: u64,
    /// Serial of `mailbox` (0 when it came from a card).
    pub(crate) mailbox_serial: u32,
    /// When we learned `mailbox`; past [`CONTACT_KEY_STALE_MS`] it may be deleted.
    pub(crate) mailbox_learned_ms: u64,
    /// Highest serial of ours this contact has confirmed (by acking a bundle that carried it).
    pub(crate) told_serial: u32,
    /// We sent them a contact request and have not heard back.
    pub(crate) requested: bool,
    /// Only a fellow group member, introduced by an admin: not shown as a contact,
    /// and 1:1 messages from them are refused.
    pub(crate) group_only: bool,
    /// Their email address, typed by the user.
    pub(crate) email: Option<String>,
    /// The X-Wing half of `mailbox`, once announced: messages then use suite 2.
    pub(crate) xwing: Option<XWingPublic>,
    /// Highest serial of ours whose X-Wing half this contact has confirmed: keeps
    /// announcing to contacts who confirmed the classical key before this version.
    pub(crate) told_xwing: u32,
    /// The contact's other devices; the fields above are its main device's.
    pub(crate) devices: Vec<devices::ContactDevice>,
    /// The contact's newest verified device list.
    pub(crate) device_list: Option<mem_proto::device::DeviceList>,
    /// Highest serial of our device list this contact's main device confirmed.
    pub(crate) told_list: u32,
}

pub(crate) struct Message {
    pub(crate) id: MsgId,
    pub(crate) contact: ContactId,
    pub(crate) direction: MsgDirection,
    pub(crate) kind: MessageKind,
    pub(crate) text: String,
    pub(crate) created_unix_ms: u64,
    pub(crate) status: DeliveryStatus,
    pub(crate) channels: Vec<ChannelUse>,
    /// The group it belongs to.
    pub(crate) group: Option<GroupId>,
    /// Our group message: the id of its first copy, shared by all copies (local only).
    pub(crate) fanout: Option<MsgId>,
}

impl From<&Message> for MessageView {
    fn from(m: &Message) -> Self {
        Self {
            id: m.id,
            contact: m.contact,
            direction: m.direction,
            kind: m.kind,
            text: m.text.clone(),
            created_unix_ms: m.created_unix_ms,
            status: m.status,
            channels: m.channels.clone(),
            group: m.group,
            recipients: None,
        }
    }
}

/// A new mailbox key every 7 days (fixed, not a setting).
pub const MAILBOX_ROTATE_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// A retired mailbox key is kept this long for bundles still in flight, then zeroized.
pub const MAILBOX_RETAIN_MS: u64 = 30 * 24 * 60 * 60 * 1000;
/// A contact key learned this long ago may already be deleted on their side: it can have been
/// current for up to 7 more days, then kept 30. Hold messages instead of sealing
/// to it.
pub const CONTACT_KEY_STALE_MS: u64 = MAILBOX_ROTATE_MS + MAILBOX_RETAIN_MS;
/// Send a standalone key update to a contact not told within this long after a rotation.
pub const KEY_UPDATE_AFTER_MS: u64 = 24 * 60 * 60 * 1000;

/// One of our mailbox keys. The last in [`Agent::mailboxes`] is current.
pub(crate) struct HeldMailbox {
    pub(crate) serial: u32,
    pub(crate) created_unix_ms: u64,
    /// When it stopped being current; `None` for the current key.
    pub(crate) retired_unix_ms: Option<u64>,
    pub(crate) key: MailboxKey,
}

/// Why our own message may go to couriers.
enum CourierPermit {
    /// Couriers are within the contact's floor, or the user chose them for this message.
    WithinFloor,
    /// A standing rule covers them; the hand-off is logged as a downgrade.
    Rule(ConsentBasis),
}

/// How many days back a receiver recognises its own tags: the 30-day maximum lifetime plus one
/// day of clock skew.
pub(crate) const RECEIVE_TAG_DAYS: u32 = 31;

/// Carried bundles sprayed to one peer per encounter, in total.
const SPRAY_PER_ENCOUNTER_BYTES: u64 = 64 * 1024;

/// An arrival batch above this is a flood: its bundles are delivered but never sprayed on.
/// Honest hand-offs stay far below it: a sender sprays at most 64 KiB per batch
/// per encounter; a flood fills the 2 MiB per-encounter allowance.
const FLOOD_BATCH_BYTES: u64 = 256 * 1024;

/// Spray copies of a new message (`L`): 32 from `mem-sim` under community
/// mobility, where 8 left most cross-community messages undelivered.
pub(crate) const DEFAULT_COPIES: u8 = 32;

/// A bundle's courier identity, `bundle_id || purge_commitment`.
fn courier_key_bytes(b: &Bundle) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[..16].copy_from_slice(b.bundle_id().as_bytes());
    k[16..].copy_from_slice(b.purge_commitment());
    k
}

/// The same identity for a carried entry.
fn key_bytes(k: &CourierKey) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(&k.bundle_id);
    out[16..].copy_from_slice(&k.commitment);
    out
}

/// Clamps a stored or requested message lifetime to the allowed range (0 means the default).
#[must_use]
pub fn clamp_lifetime(seconds: u32) -> u32 {
    if seconds == 0 {
        MESSAGE_LIFETIME_S
    } else {
        seconds.clamp(MIN_MESSAGE_LIFETIME_S, MESSAGE_LIFETIME_S)
    }
}

/// A message we delivered, by the purge commitment of its bundle, so a retransmitted copy can be
/// acknowledged again without being admitted.
pub(crate) struct DeliveredEntry {
    pub(crate) contact: ContactId,
    pub(crate) message: MsgId,
    pub(crate) expires_unix_ms: u64,
    /// The contact's other device that sent it (re-acks go there).
    pub(crate) device: Option<[u8; 8]>,
}

/// What an in-flight transmission carries.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Op {
    /// A message bundle.
    Message {
        message: MsgId,
        cla: ClaId,
        peer: Option<PeerHandle>,
        /// The grade this send has (the hub's depends on crossing).
        grade: PrivacyGrade,
    },
    /// A standalone ack for these of `contact`'s messages; `peer` is `None` on the hub.
    Ack {
        contact: ContactId,
        ids: Vec<MsgId>,
        peer: Option<PeerHandle>,
    },
}

impl Op {
    /// A send of `message` over `channel` (hub or email) is in flight.
    pub(crate) fn is_upload(&self, id: MsgId, channel: ClaId) -> bool {
        matches!(self, Self::Message { message, cla, .. } if *message == id && *cla == channel)
    }
}

/// Sends of one message within one encounter (volatile).
#[derive(Clone, Copy)]
pub(crate) struct SendState {
    peer: PeerHandle,
    count: u8,
    last_unix_ms: u64,
}

/// A purged commitment and its token, kept for re-gossip.
pub(crate) struct Tombstone {
    pub(crate) token: PurgeToken,
    pub(crate) expires_unix_ms: u64,
}

/// A bundle that decrypted and verified.
struct Opened {
    contact: ContactId,
    id: MsgId,
    commitment: PurgeCommitment,
    token: PurgeToken,
    /// Piggybacked acks (field 10) or a standalone ack's ids.
    acks: Vec<MsgId>,
    content: OpenedContent,
    /// The sender's mailbox key announcement (field 6), authenticated by the signature.
    key_update: Option<KeyUpdate>,
    /// Serial of our mailbox key whose X-Wing half sealed this bundle (suite 2): the sender
    /// holds that key, which confirms our announcement.
    xwing_serial: Option<u32>,
    /// The contact's other device that signed it; `None`: its main device.
    from_device: Option<[u8; 8]>,
}

/// A bundle just delivered: what storing its text needs.
#[derive(Clone, Copy)]
struct Delivered {
    contact: ContactId,
    id: MsgId,
    commitment: PurgeCommitment,
    created_unix_ms: u64,
    ack_requested: bool,
    /// The contact's other device that sent it.
    from_device: Option<[u8; 8]>,
}

/// What [`Agent::seal`] puts in a bundle.
struct Sealing {
    kind: BundleKind,
    content_type: u64,
    content: Vec<u8>,
    ack_requested: Option<bool>,
    acks: Vec<[u8; 16]>,
    lifetime_s: u32,
    /// Our current mailbox key, while the contact has not confirmed it.
    key_update: Option<KeyUpdate>,
    /// Keep this message id (resealing to a newer contact key); a new one otherwise.
    id: Option<MsgId>,
    /// Which key and tag to seal to.
    recipient: Recipient,
}

/// Where a bundle is sealed to.
enum Recipient {
    /// The contact's mailbox key and rendezvous tag.
    Mailbox,
    /// The contact's identity-derived key (key updates only).
    Identity,
    /// A contact request: the card's mailbox key, the recipient's intro tag and a proof of work.
    Intro { dst_tag: [u8; 16], pow: Pow },
    /// One of the contact's other devices: its mailbox key, the contact's tag.
    Device([u8; 8]),
}

enum OpenedContent {
    /// An own-device sync item.
    OwnSync { data: Vec<u8> },
    /// A standalone key update.
    KeyUpdate { ack_requested: bool },
    Text {
        created_unix_ms: u64,
        text: String,
        ack_requested: bool,
    },
    /// A standalone ack bundle (kind 1, content type 2).
    Ack,
    /// Group traffic.
    Group {
        created_unix_ms: u64,
        content: GroupContent,
    },
}

/// A connected peer (volatile).
pub(crate) struct Peer {
    cla: ClaId,
    hello: bool,
    tags: BTreeSet<[u8; 16]>,
    bundles_received: u32,
    /// Courier admissions charged to this encounter.
    courier_usage: EncounterUsage,
    /// Our `hello` id in this encounter (direct-delivery proofs to us are bound to it).
    our_id: [u8; 16],
    /// The peer's `hello` id (our proofs to it are bound to it).
    their_id: Option<[u8; 16]>,
    /// The peer's summary: what it holds. A peer that sent one takes part in courier exchange.
    held: Option<SaltedBloom>,
    /// Commitments the peer holds bundles for but has no token.
    wanted: Option<SaltedBloom>,
    /// Bundles already sent to this peer in this encounter, by `bundle_id || commitment`.
    handed: BTreeSet<[u8; 32]>,
    /// What we sent this peer as courier traffic, within its per-encounter allowance.
    sent: EncounterUsage,
    /// Purge commitments whose tokens this peer already has from us (sent at most once).
    tokens_sent: BTreeSet<[u8; 16]>,
    /// Arrival-encounter id for what this peer hands us.
    batch: u64,
    /// Spray bytes handed to this peer per arrival batch, within [`SPRAY_PER_BATCH_BYTES`].
    spread: BTreeMap<u64, u64>,
    /// Carried bundles sprayed to this peer in this encounter, within
    /// [`SPRAY_PER_ENCOUNTER_BYTES`].
    sprayed: u64,
    /// Older carried tags already offered to this peer.
    offered: BTreeSet<[u8; 16]>,
    /// Offered tags this peer claimed as its own: deliver those bundles to it.
    claimed: BTreeSet<[u8; 16]>,
    /// Proof-of-work checks spent on this encounter, and whether one failed.
    intro_checks: u8,
    intro_bad: bool,
}

/// Whether the consent gate may ask the user, or must only use what is already allowed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    /// Interactive send: below-floor channels produce a [`ChannelOffer`].
    User,
    /// Automatic send (outbox flush): never asks; only within-floor channels or standing rules.
    Never,
}

pub(crate) struct OutboxItem {
    pub(crate) contact: ContactId,
    pub(crate) bundle: Bundle,
    pub(crate) created_unix_ms: u64,
    /// Acks piggybacked in this bundle; they stop being owed once it is handed to a channel.
    pub(crate) acks: Vec<MsgId>,
    /// Spray copies we still hold for courier hand-offs. Direct sends never
    /// spend them: those go out with one copy and no hops.
    pub(crate) copies: u8,
    /// The user allowed couriers for this message although they are below the floor.
    pub(crate) courier_ok: bool,
    /// Our key serial announced in this bundle (field 6), if any; an ack for it tells us the
    /// contact knows that key.
    pub(crate) key_serial: Option<u32>,
    /// Serial of our device list carried in this bundle (field 6 key 4).
    pub(crate) list_serial: Option<u32>,
    /// Serial of the contact key this bundle is sealed to, and when we had learned that key.
    pub(crate) sealed_serial: u32,
    pub(crate) sealed_learned_ms: u64,
    /// Sealed to the contact's identity key (a key update whose mailbox key was stale).
    pub(crate) via_identity: bool,
    /// Content type and bytes of a hidden item (group traffic), kept for resealing.
    pub(crate) content: Option<(u64, Vec<u8>)>,
    /// Sealed to one of the contact's other devices; `None`: its main device.
    pub(crate) device: Option<[u8; 8]>,
    /// A per-device copy of this item: its ack marks that one delivered.
    pub(crate) copy_of: Option<MsgId>,
}

/// Proof that a transmission passed the consent check. Only [`Agent::authorize`] and
/// [`Agent::consent_given`] construct it, and [`Agent::transmit`] requires it.
struct Authorized {
    message: MsgId,
    cla: ClaId,
    below_floor: Option<ConsentBasis>,
}

/// The MEM agent: all protocol state and logic, no I/O.
pub struct Agent {
    pub(crate) identity: own::Identity,
    /// Our devices: this device's key, certificate and our device list.
    pub(crate) own: own::OwnDevices,
    /// Main device: the contacts each other device has been sent.
    pub(crate) own_sync: BTreeMap<[u8; 8], BTreeSet<ContactId>>,
    /// Our mailbox keyring, oldest first; the last is current.
    pub(crate) mailboxes: Vec<HeldMailbox>,
    pub(crate) pairing_nonce: PairingNonce,
    pub(crate) display_name: Option<String>,
    pub(crate) contacts: BTreeMap<ContactId, Contact>,
    pub(crate) messages: Vec<Message>,
    pub(crate) outbox: BTreeMap<MsgId, OutboxItem>,
    pub(crate) seen: VecDeque<MsgId>,
    pub(crate) rules: Vec<ConsentRule>,
    pub(crate) privacy_log: Vec<PrivacyLogEntry>,
    /// Attestations made on this device, oldest first.
    pub(crate) attest_log: Vec<AttestLogEntry>,
    /// Created on first send if missing (old snapshots lack it).
    pub(crate) purge_key: Option<SenderPurgeKey>,
    pub(crate) tombstones: BTreeMap<PurgeCommitment, Tombstone>,
    /// Channels the user turned off.
    pub(crate) disabled: BTreeSet<ClaId>,
    /// Acks we owe, per contact.
    pub(crate) pending_acks: BTreeMap<ContactId, Vec<MsgId>>,
    /// Owed acks for messages from a contact's other device: the ack goes to that device.
    /// Messages from the main device are not listed.
    pub(crate) ack_from: BTreeMap<MsgId, (ContactId, [u8; 8])>,
    /// Messages we delivered, by bundle commitment.
    pub(crate) delivered: BTreeMap<PurgeCommitment, DeliveredEntry>,
    /// Lifetime of new messages, seconds (user setting).
    pub(crate) message_lifetime_s: u32,
    /// Floor given to newly added contacts (user setting, never P5).
    pub(crate) default_floor: PrivacyGrade,
    /// Tombstones came from an old snapshot and must be written to the database (not persisted).
    pub(crate) migrate_tombstones: bool,
    /// Other people's bundles we carry: index only; bytes live in the runtime's database.
    pub(crate) courier: CourierStore,
    /// Courier storage limit in bytes (user setting; 0 = courier mode off).
    pub(crate) courier_limit: u64,
    /// Recovery secret, if backups are on.
    pub(crate) recovery: Option<RecoverySecret>,
    /// How the recovery phrase is shown.
    pub(crate) recovery_form: PhraseForm,
    /// A new phrase shown but not yet confirmed (never persisted).
    pub(crate) pending_recovery: Option<(RecoverySecret, PhraseForm)>,
    pub(crate) next_rule: u64,
    pub(crate) created_unix_ms: u64,
    // Volatile state: not persisted.
    pub(crate) now_unix_ms: u64,
    pub(crate) offers: BTreeMap<OfferId, ChannelOffer>,
    pub(crate) ops: BTreeMap<OpId, Op>,
    pub(crate) sends: BTreeMap<MsgId, SendState>,
    /// When acks for a contact became owed (for [`ACK_DELAY_MS`]).
    pub(crate) ack_due: BTreeMap<ContactId, u64>,
    pub(crate) next_id: u64,
    pub(crate) peers: BTreeMap<PeerHandle, Peer>,
    /// Epoch of the tags last advertised to peers.
    pub(crate) advertised_epoch: u32,
    /// The pairing nonce before the last "new pairing code", still accepted in contact requests
    /// made from an older card.
    pub(crate) previous_pairing_nonce: Option<PairingNonce>,
    /// Advertise our intro tag in encounters (user setting, off by default).
    pub(crate) accept_intros: bool,
    /// Contact requests in quarantine, oldest first.
    pub(crate) requests: Vec<ContactRequest>,
    /// Senders whose requests we ignore, oldest first (identity keys).
    pub(crate) blocked: Vec<[u8; 32]>,
    /// Our contact requests waiting for their proof of work.
    pub(crate) pending_intros: BTreeMap<MsgId, PendingIntro>,
    /// Proof-of-work jobs handed to the runtime (volatile).
    pub(crate) solving: BTreeSet<MsgId>,
    /// Groups we are in, invited to, or left.
    pub(crate) groups: BTreeMap<GroupId, Group>,
    /// The hub connection's state.
    pub(crate) hub: HubState,
    /// The hub's address (user setting; no default until a MEM-operated hub exists).
    pub(crate) hub_url: Option<String>,
    /// Where the hub wakes us (UnifiedPush endpoint); `None` without push.
    pub(crate) push_endpoint: Option<String>,
    /// The user's mail account for the email bridge.
    pub(crate) email_account: Option<EmailAccount>,
    /// The newest verified hub list.
    pub(crate) hub_list: Option<mem_proto::hublist::HubList>,
    /// This device's random starting point in the hub list.
    pub(crate) hub_pick: u32,
    /// "Only my operator's hubs".
    pub(crate) own_operator_only: bool,
    /// Operator keys that may sign hub lists (volatile, from the runtime).
    pub(crate) operator_keys: Vec<[u8; 32]>,
    /// Spray copies of a new message (`L`); [`DEFAULT_COPIES`] except in `mem-sim` runs that
    /// compare budgets. Volatile.
    pub(crate) spray_copies: u8,
    /// Email bridge state.
    pub(crate) email: EmailState,
}

impl core::fmt::Debug for Agent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Agent")
            .field("contacts", &self.contacts.len())
            .field("messages", &self.messages.len())
            .finish_non_exhaustive()
    }
}

impl Agent {
    /// Creates a new identity.
    pub fn new(now_unix_ms: u64, rng: &mut impl CryptoRng) -> Self {
        Self {
            identity: own::Identity::main(IdentityKey::generate(rng)),
            own: own::OwnDevices::default(),
            own_sync: BTreeMap::new(),
            mailboxes: alloc::vec![HeldMailbox {
                serial: 1,
                created_unix_ms: now_unix_ms,
                retired_unix_ms: None,
                key: MailboxKey::generate(rng),
            }],
            pairing_nonce: PairingNonce::generate(rng),
            display_name: None,
            contacts: BTreeMap::new(),
            messages: Vec::new(),
            outbox: BTreeMap::new(),
            seen: VecDeque::new(),
            rules: Vec::new(),
            privacy_log: Vec::new(),
            attest_log: Vec::new(),
            purge_key: Some(SenderPurgeKey::generate(rng)),
            tombstones: BTreeMap::new(),
            disabled: ClaId::ALL
                .into_iter()
                .filter(|c| !c.on_by_default())
                .collect(),
            pending_acks: BTreeMap::new(),
            ack_from: BTreeMap::new(),
            delivered: BTreeMap::new(),
            message_lifetime_s: MESSAGE_LIFETIME_S,
            default_floor: DEFAULT_FLOOR,
            migrate_tombstones: false,
            courier: CourierStore::default(),
            courier_limit: DEFAULT_COURIER_BYTES,
            recovery: None,
            recovery_form: PhraseForm::English,
            pending_recovery: None,
            next_rule: 1,
            created_unix_ms: now_unix_ms,
            now_unix_ms,
            offers: BTreeMap::new(),
            ops: BTreeMap::new(),
            sends: BTreeMap::new(),
            ack_due: BTreeMap::new(),
            next_id: 1,
            peers: BTreeMap::new(),
            advertised_epoch: epoch_for_unix_ms(now_unix_ms),
            previous_pairing_nonce: None,
            accept_intros: false,
            requests: Vec::new(),
            blocked: Vec::new(),
            pending_intros: BTreeMap::new(),
            solving: BTreeSet::new(),
            groups: BTreeMap::new(),
            hub: HubState::default(),
            hub_url: None,
            push_endpoint: None,
            email_account: None,
            email: EmailState::default(),
            hub_list: None,
            hub_pick: rng.next_u32(),
            own_operator_only: false,
            operator_keys: Vec::new(),
            spray_copies: DEFAULT_COPIES,
        }
    }

    /// Processes one input. Deterministic given inputs and RNG; never blocks.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per input; larger ones delegate"
    )]
    pub fn handle(&mut self, input: Input, rng: &mut impl CryptoRng) -> Vec<Output> {
        match input {
            Input::Tick { wall_unix_ms } => {
                self.now_unix_ms = wall_unix_ms;
                self.on_tick(rng)
            }
            Input::UserSend { contact, text } => {
                let mut out = self.user_send(contact, &text, rng);
                if let Some(message) = out.iter().find_map(|o| match o {
                    Output::Queued { message, .. } => Some(*message),
                    _ => None,
                }) {
                    out.extend(self.flush_message(message));
                }
                out
            }
            Input::RequestTransmit { message, cla } => self.request_transmit(message, cla),
            Input::ConsentGiven(record) => self.consent_given(record),
            Input::TransmitResult { op, result } => self.transmit_result(op, result),
            Input::Received { cla, bytes } => self.received(cla, &bytes, rng),
            Input::ImportCard { cla, bytes } => self.import_card(cla, &bytes, rng),
            Input::NewPairingCode => {
                let old = core::mem::replace(&mut self.pairing_nonce, PairingNonce::generate(rng));
                self.previous_pairing_nonce = Some(old);
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::SetDisplayName(name) => self.set_display_name(name),
            Input::AttestationReturned(entry) => self.record_attestation(entry),
            Input::SetFloor { floor, .. } | Input::SetDefaultFloor { floor }
                if !floor.allowed_as_floor() =>
            {
                alloc::vec![Output::Rejected(RejectReason::InvalidFloor)]
            }
            Input::SetDefaultFloor { floor } => {
                self.default_floor = floor;
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::SetFloor { contact, floor } => match self.contacts.get_mut(&contact) {
                Some(c) => {
                    c.floor = floor;
                    alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
                }
                None => alloc::vec![Output::Rejected(RejectReason::UnknownContact)],
            },
            Input::RevokeRule(id) => {
                self.rules.retain(|r| r.id != id);
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::SetChannelEnabled { cla, enabled } => self.set_channel_enabled(cla, enabled),
            Input::NewRecoveryPhrase { form } => {
                self.new_pending_recovery(form, rng);
                Vec::new()
            }
            Input::ConfirmRecoveryPhrase => {
                if self.confirm_pending_recovery() {
                    alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
                } else {
                    alloc::vec![Output::Rejected(RejectReason::NoPendingPhrase)]
                }
            }
            Input::DiscardRecoveryPhrase => {
                self.pending_recovery = None;
                Vec::new()
            }
            Input::StoreLoaded {
                courier,
                tombstones,
            } => self.store_loaded(courier, tombstones),
            Input::SetCourierLimit { bytes } => self.set_courier_limit(bytes),
            Input::SetPhraseForm { form } => {
                self.set_phrase_form(form);
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::SetMessageLifetime { seconds } => {
                self.message_lifetime_s = clamp_lifetime(seconds);
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::EncounterStarted { peer, cla } => self.encounter_started(peer, cla, rng),
            Input::EncounterMessage { peer, msg } => self.encounter_message(peer, msg, rng),
            input @ (Input::SendContactRequest { .. }
            | Input::PowSolved { .. }
            | Input::ApproveRequest(_)
            | Input::RejectRequest(_)
            | Input::BlockRequest(_)
            | Input::SetAcceptRequestsNearby { .. }) => self.request_input(input, rng),
            input @ (Input::CreateGroup { .. }
            | Input::InviteToGroup { .. }
            | Input::RemoveFromGroup { .. }
            | Input::AcceptGroupInvitation(_)
            | Input::DeclineGroupInvitation(_)
            | Input::LeaveGroup(_)
            | Input::AddMemberAsContact(_)
            | Input::SendGroupText { .. }) => self.group_input(input, rng),
            Input::SetHubUrl(url) => match url.map(|u| String::from(u.trim())) {
                Some(u) if !hub::valid_url(&u) => {
                    alloc::vec![Output::Rejected(RejectReason::InvalidHubUrl)]
                }
                url => {
                    self.hub_url = url.filter(|u| !u.is_empty());
                    alloc::vec![
                        Output::HubCandidatesChanged,
                        Output::Persist(StorageOp::SaveSnapshot),
                    ]
                }
            },
            Input::ApproveDevice { request } => self.approve_device(&request, rng),
            Input::RetireDevice { device } => self.retire_device(device, rng),
            Input::SetOwnOperatorOnly { enabled } => {
                self.own_operator_only = enabled;
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            Input::SetPushEndpoint(endpoint) => self.set_push_endpoint(endpoint, rng),
            Input::SetOperatorKeys(keys) => {
                self.operator_keys = keys;
                // A list kept from before must still be signed by a current key.
                let kept = self.hub_list.take().and_then(|l| l.encode().ok());
                kept.map(|b| self.hub_list_received(&b)).unwrap_or_default()
            }
            Input::HubListReceived { bytes } => self.hub_list_received(&bytes),
            Input::RemoveContact(contact) => self.remove_contact(contact, rng),
            input @ (Input::SetEmailAccount(_)
            | Input::SetContactEmail { .. }
            | Input::EmailConnected
            | Input::EmailDisconnected) => self.email_input(input),
            input @ (Input::HubConnected
            | Input::HubDisconnected
            | Input::HubFetchDone
            | Input::HubBundles { .. }) => self.hub_input(input, rng),
            Input::EncounterEnded { peer } => {
                self.forget_peer_ops(peer);
                if self.peers.remove(&peer).is_some() {
                    alloc::vec![Output::ReachabilityChanged]
                } else {
                    Vec::new()
                }
            }
        }
    }

    // ---------------------------------------------------------------- sending

    /// Whether the user may write to `contact_id`: a contact that approved us, not only a fellow
    /// group member, and never our own devices' hidden self contact.
    fn may_write_to(&self, contact_id: ContactId) -> Result<(), RejectReason> {
        let c = self
            .contacts
            .get(&contact_id)
            .ok_or(RejectReason::UnknownContact)?;
        if c.requested {
            return Err(RejectReason::AwaitingApproval);
        }
        if c.group_only || self.is_self(contact_id) {
            return Err(RejectReason::NotAContact);
        }
        Ok(())
    }

    fn user_send(
        &mut self,
        contact_id: ContactId,
        text: &str,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_TEXT_LEN {
            return alloc::vec![Output::Rejected(RejectReason::InvalidText)];
        }
        if let Err(r) = self.may_write_to(contact_id) {
            return alloc::vec![Output::Rejected(r)];
        }
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        let (Some(contact), Some(purge_key)) = (self.contacts.get(&contact_id), &self.purge_key)
        else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownContact)];
        };
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let token = purge_key.derive_token(&bundle_id);
        let commitment = commit(&token);
        // Acks owed to this contact ride along; they stay owed until this bundle
        // is actually handed to a channel.
        let acks = self.owed_acks(contact_id, None);
        let Ok(content) = encode_text(text) else {
            return alloc::vec![Output::Rejected(RejectReason::InvalidText)];
        };
        let copy_content = content.clone();
        let key_update = self.key_update_for(contact);
        let key_serial = key_update.as_ref().map(|k| k.serial);
        let list_serial = key_update
            .as_ref()
            .and_then(|k| k.device_list.as_ref())
            .map(|l| l.serial);
        let (sealed_serial, sealed_learned_ms) =
            (contact.mailbox_serial, contact.mailbox_learned_ms);
        let Ok(bundle) = self.seal(
            contact,
            Sealing {
                kind: BundleKind::Message,
                content_type: CONTENT_TYPE_TEXT,
                content,
                ack_requested: Some(true),
                acks: acks.iter().map(|m| m.0).collect(),
                lifetime_s: self.message_lifetime_s,
                key_update,
                id: None,
                recipient: Recipient::Mailbox,
            },
            bundle_id,
            &token,
            commitment,
            rng,
        ) else {
            return alloc::vec![Output::Rejected(RejectReason::InvalidText)];
        };
        let (bundle, id) = bundle;
        self.messages.push(Message {
            id,
            contact: contact_id,
            direction: MsgDirection::Outgoing,
            kind: MessageKind::Text,
            text: String::from(text),
            created_unix_ms: self.now_unix_ms,
            status: DeliveryStatus::Queued,
            channels: Vec::new(),
            group: None,
            fanout: None,
        });
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
                content: None,
                device: None,
                copy_of: None,
            },
        );
        // A copy for each of the contact's other devices.
        self.queue_device_copies(
            contact_id,
            id,
            BundleKind::Message,
            CONTENT_TYPE_TEXT,
            &copy_content,
            rng,
        );
        alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::Queued {
                contact: contact_id,
                message: id,
            },
        ]
    }

    /// Builds a sealed bundle. Callers pass the purge token and its
    /// commitment so tests can construct a mismatched pair.
    fn seal(
        &self,
        contact: &Contact,
        what: Sealing,
        bundle_id: [u8; 16],
        token: &PurgeToken,
        commitment: PurgeCommitment,
        rng: &mut impl CryptoRng,
    ) -> Result<(Bundle, MsgId), mem_proto::ProtoError> {
        let me = self.identity.public();
        let id = what
            .id
            .unwrap_or_else(|| MsgId(uuid_v7(self.now_unix_ms, rng)));
        let fields = InnerFields {
            sender: *me.as_bytes(),
            message_id: id.0,
            conversation_id: contact.secret.conversation_id(),
            content_type: what.content_type,
            content: what.content,
            ack_requested: what.ack_requested,
            purge_token: *token.as_bytes(),
            acks: what.acks,
            key_update: what.key_update,
            // Another device signs with its device key and shows its certificate.
            device_cert: if self.identity.key().is_some() {
                None
            } else {
                self.own.cert.clone()
            },
        };
        let signature = self
            .sign_inner(&contact.identity, &fields)
            .ok_or(mem_proto::SchemaError::MapShape)?;
        let padded = pad(&Inner { fields, signature }.encode())?;

        let bundle_id = BundleId::from_bytes(bundle_id);
        let dst_tag = RendezvousTag::from_bytes(self.dst_tag(contact, &what.recipient));
        let aad = aad_for(&bundle_id, what.kind, &dst_tag, commitment.as_bytes());
        let device = match &what.recipient {
            Recipient::Device(id) => contact.devices.iter().find(|d| d.id == *id),
            _ => None,
        };
        let (recipient, key_id, pow) = match what.recipient {
            Recipient::Mailbox => (
                contact.mailbox,
                contact.mailbox.key_id(&contact.secret),
                None,
            ),
            Recipient::Identity => (
                MailboxPublic::of_identity(&contact.identity)
                    .map_err(|_| mem_proto::SchemaError::MapShape)?,
                identity_key_id(&contact.secret),
                None,
            ),
            Recipient::Intro { pow, .. } => {
                (contact.mailbox, intro_key_id(&contact.mailbox), Some(pow))
            }
            Recipient::Device(id) => {
                let m = device
                    .and_then(|d| d.mailbox)
                    .ok_or(mem_proto::SchemaError::MapShape)?;
                let _ = id;
                (m, m.key_id(&contact.secret), None)
            }
        };
        // Suite 2 whenever the contact's X-Wing key is known: messages to its mailbox only. The
        // identity fallback and contact requests carry no content worth keeping.
        let pq = match (&what.recipient, &contact.xwing) {
            (Recipient::Mailbox, Some(x)) => Some(x),
            (Recipient::Device(_), _) => device.and_then(|d| d.xwing.as_ref()),
            _ => None,
        };
        let (suite_id, key_id, sealed) = match pq {
            Some(x) => {
                let id = x.key_id(&contact.secret);
                (SUITE_XWING, id, x.seal(&id, &padded, &aad, rng))
            }
            None => (
                SUITE_ID,
                key_id,
                recipient.seal(&key_id, &padded, &aad, rng),
            ),
        };
        let sealed = sealed.map_err(|_| mem_proto::SchemaError::MapShape)?;
        let payload = Payload {
            suite_id,
            mailbox_key_id: *key_id.as_bytes(),
            enc: sealed.enc,
            ciphertext: sealed.ciphertext,
        };
        let bundle = Bundle::new(BundleParams {
            bundle_id,
            kind: what.kind,
            dst_tag,
            lifetime_s: what.lifetime_s,
            age_ms: 0,
            // v0.1 channels deliver directly to the receiver: no courier hand-offs.
            hops_remaining: 0,
            copies: 1,
            pow,
            payload: payload.encode(),
            purge_commitment: *commitment.as_bytes(),
        })?;
        Ok((bundle, id))
    }

    /// The tag a bundle to `recipient` of `contact` is addressed to: the
    /// contact's, its other device's own, or the intro tag of a request.
    fn dst_tag(&self, contact: &Contact, recipient: &Recipient) -> [u8; 16] {
        let direction = Direction::towards(&contact.identity, &self.identity.public());
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        match recipient {
            Recipient::Intro { dst_tag, .. } => *dst_tag,
            Recipient::Mailbox | Recipient::Identity => contact.secret.tag(direction, epoch),
            Recipient::Device(id) => contact.secret.device_tag(direction, epoch, id),
        }
    }

    /// The inner signature to `recipient`: by the identity on the main device,
    /// by this device's certified key on another one (`fields` then carries the certificate).
    fn sign_inner(&self, recipient: &IdentityPublic, fields: &InnerFields) -> Option<[u8; 64]> {
        let signed = [recipient.as_bytes().as_slice(), &fields.signed_bytes()];
        match (self.identity.key(), &self.own.key) {
            (Some(identity), _) => Some(identity.sign(SIGNATURE_DOMAIN, &signed)),
            (None, Some(device)) if fields.device_cert.is_some() => {
                Some(device.sign(SIGNATURE_DOMAIN, &signed))
            }
            _ => None,
        }
    }

    /// A text message bundle with an explicit purge token and commitment (tests build mismatched
    /// pairs with it).
    #[cfg(test)]
    fn seal_message(
        &self,
        contact: &Contact,
        text: &str,
        bundle_id: [u8; 16],
        token: &PurgeToken,
        commitment: PurgeCommitment,
        rng: &mut impl CryptoRng,
    ) -> Result<(Bundle, MsgId), mem_proto::ProtoError> {
        self.seal(
            contact,
            Sealing {
                kind: BundleKind::Message,
                content_type: CONTENT_TYPE_TEXT,
                content: encode_text(text)?,
                ack_requested: Some(true),
                acks: Vec::new(),
                lifetime_s: self.message_lifetime_s,
                key_update: None,
                id: None,
                recipient: Recipient::Mailbox,
            },
            bundle_id,
            token,
            commitment,
            rng,
        )
    }

    /// Channels the user can pick for a queued message.
    #[must_use]
    pub fn channel_options(&self, message: MsgId) -> Vec<ChannelOption> {
        let Some(item) = self.outbox.get(&message) else {
            return Vec::new();
        };
        let len = item.bundle.encode().len();
        let floor = self
            .contacts
            .get(&item.contact)
            .map_or(DEFAULT_FLOOR, |c| c.floor);
        ClaId::ALL
            .into_iter()
            .filter(|cla| !self.disabled.contains(cla))
            .map(|cla| {
                let grade = cla.grade();
                ChannelOption {
                    cla,
                    grade,
                    fits: len <= cla.caps().max_bundle_len,
                    needs_consent: !grade.within(floor)
                        && self.matching_rule(item.contact, cla, grade).is_none(),
                    reachable: match cla {
                        ClaId::Hub => self.hub.online,
                        ClaId::Email => self.email.online && self.email_to(item.contact).is_some(),
                        c if c.caps().needs_peer => self.peer_for(item.contact, c).is_some(),
                        _ => true,
                    },
                }
            })
            .collect()
    }

    fn request_transmit(&mut self, message: MsgId, cla: ClaId) -> Vec<Output> {
        match self.authorize(message, cla, Ask::User) {
            Ok(auth) => self.transmit(auth, None),
            Err(out) => alloc::vec![out],
        }
    }

    /// The consent gate. With [`Ask::Never`] it never creates
    /// an offer: a below-floor channel without a standing rule is simply refused.
    fn authorize(&mut self, message: MsgId, cla: ClaId, ask: Ask) -> Result<Authorized, Output> {
        if self.disabled.contains(&cla) {
            return Err(Output::Rejected(RejectReason::ChannelDisabled));
        }
        let item = self
            .outbox
            .get(&message)
            .ok_or(Output::Rejected(RejectReason::UnknownMessage))?;
        if self.waiting_for_key(item) {
            return Err(Output::Rejected(RejectReason::WaitingForKey));
        }
        if item.bundle.encode().len() > cla.caps().max_bundle_len {
            return Err(Output::Rejected(RejectReason::TooLargeForChannel));
        }
        let contact_id = item.contact;
        let floor = self
            .contacts
            .get(&contact_id)
            .ok_or(Output::Rejected(RejectReason::UnknownContact))?
            .floor;
        let grade = cla.grade();
        if grade.within(floor) {
            return Ok(Authorized {
                message,
                cla,
                below_floor: None,
            });
        }
        if let Some(rule) = self.matching_rule(contact_id, cla, grade) {
            return Ok(Authorized {
                message,
                cla,
                below_floor: Some(ConsentBasis::ExistingRule(rule)),
            });
        }
        if ask == Ask::Never {
            return Err(Output::Rejected(RejectReason::ConsentMismatch));
        }
        let offer = ChannelOffer {
            id: OfferId(self.next_id()),
            message,
            contact: contact_id,
            cla,
            grade,
            floor,
            exposure: self.channel_exposure(cla),
        };
        self.offers.insert(offer.id, offer.clone());
        Err(Output::ConsentRequired(offer))
    }

    /// Simulation only (`mem-sim`): the copy budget for new messages, 1..=64.
    /// Not a user setting and not saved.
    #[doc(hidden)]
    pub fn set_spray_copies_for_simulation(&mut self, copies: u8) {
        self.spray_copies = copies.clamp(1, 64);
    }

    /// A channel's exposure profile with this device's settings (push wake-ups for the hub).
    #[must_use]
    pub fn channel_exposure(&self, cla: ClaId) -> mem_cla::ExposureProfile {
        let push = if self.push_endpoint.is_some() && self.channel_enabled(ClaId::Hub) {
            mem_cla::PushWake::On
        } else {
            mem_cla::PushWake::Off
        };
        cla.exposure_with(push)
    }

    fn matching_rule(&self, contact: ContactId, cla: ClaId, grade: PrivacyGrade) -> Option<RuleId> {
        // P5 never matches a standing rule.
        if grade == PrivacyGrade::P5 {
            return None;
        }
        self.rules
            .iter()
            .find(|r| {
                r.cla == cla
                    && grade <= r.max_grade
                    && r.contact.is_none_or(|c| c == contact)
                    && r.until_unix_ms.is_none_or(|t| self.now_unix_ms < t)
            })
            .map(|r| r.id)
    }

    /// Verify the record matches the offer, then transmit.
    fn consent_given(&mut self, record: ConsentRecord) -> Vec<Output> {
        let Some(offer) = self.offers.remove(&record.offer) else {
            return alloc::vec![Output::Rejected(RejectReason::ConsentMismatch)];
        };
        if offer.message != record.message || offer.cla != record.cla || offer.grade != record.grade
        {
            return alloc::vec![Output::Rejected(RejectReason::ConsentMismatch)];
        }
        let basis = match record.scope {
            ConsentScope::Once => ConsentBasis::Once,
            // P5 consent is always Once.
            _ if offer.grade == PrivacyGrade::P5 => {
                return alloc::vec![Output::Rejected(RejectReason::ConsentMismatch)];
            }
            ConsentScope::ThisContact { until_unix_ms } => {
                ConsentBasis::NewRule(self.add_rule(Some(offer.contact), &offer, until_unix_ms))
            }
            ConsentScope::ChannelAlways { until_unix_ms } => {
                ConsentBasis::NewRule(self.add_rule(None, &offer, until_unix_ms))
            }
        };
        self.transmit(
            Authorized {
                message: offer.message,
                cla: offer.cla,
                below_floor: Some(basis),
            },
            None,
        )
    }

    fn add_rule(
        &mut self,
        contact: Option<ContactId>,
        offer: &ChannelOffer,
        until: Option<u64>,
    ) -> RuleId {
        let id = RuleId(self.next_rule);
        self.next_rule += 1;
        self.rules.push(ConsentRule {
            id,
            contact,
            cla: offer.cla,
            max_grade: offer.grade,
            until_unix_ms: until,
            created_unix_ms: self.now_unix_ms,
        });
        id
    }

    /// The only producer of [`Output::Transmit`]. Network channels go to `peer` if given,
    /// else to any connected peer that advertises the contact's tag.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "taking the token by value makes each authorization single-use"
    )]
    fn transmit(&mut self, auth: Authorized, peer: Option<PeerHandle>) -> Vec<Output> {
        // Consume the token: it authorizes exactly one transmission.
        let Authorized {
            message,
            cla,
            below_floor,
        } = auth;
        if self.disabled.contains(&cla) {
            return alloc::vec![Output::Rejected(RejectReason::ChannelDisabled)];
        }
        if cla == ClaId::Courier {
            return self.allow_couriers(message, below_floor);
        }
        let Some(item) = self.outbox.get(&message) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        };
        let (email_to, plaintext) = match self.channel_extras(cla, message, item) {
            Ok(x) => x,
            Err(reason) => return alloc::vec![Output::Rejected(reason)],
        };
        let age = self.now_unix_ms.saturating_sub(item.created_unix_ms);
        let (contact, device) = (item.contact, item.device);
        let peer = if cla.caps().needs_peer {
            let Some(p) = peer.or_else(|| self.peer_for_target(contact, device, cla)) else {
                return alloc::vec![Output::Rejected(RejectReason::Unreachable)];
            };
            Some(p)
        } else {
            None
        };
        let Ok(aged) = item.bundle.clone().with_age_ms(age) else {
            self.outbox.remove(&message);
            self.sends.remove(&message);
            self.set_status(message, DeliveryStatus::Failed);
            return alloc::vec![
                Output::Persist(StorageOp::SaveSnapshot),
                Output::Rejected(RejectReason::Expired),
            ];
        };
        let mut out = Vec::new();
        let crossing = (cla == ClaId::Hub).then(|| self.hub_crossing(contact));
        let grade = hub::send_grade(cla, crossing);
        if let Some(basis) = below_floor {
            self.log_below_floor(contact, message, cla, grade, basis);
            out.push(Output::Persist(StorageOp::SaveSnapshot));
        }
        let op = OpId(self.next_id());
        self.ops.insert(
            op,
            Op::Message {
                message,
                cla,
                peer,
                grade,
            },
        );
        if let Some(peer) = peer {
            let now = self.now_unix_ms;
            let count = match self.sends.get(&message) {
                Some(st) if st.peer == peer => st.count.saturating_add(1),
                _ => 1,
            };
            self.sends.insert(
                message,
                SendState {
                    peer,
                    count,
                    last_unix_ms: now,
                },
            );
        }
        let proof = peer.and_then(|p| self.direct_proof(p, contact, &aged));
        if let Some(p) = peer
            && let Some(state) = self.peers.get_mut(&p)
        {
            state.handed.insert(courier_key_bytes(&aged));
        }
        if let Some(text) = plaintext {
            out.push(Output::PlaintextReady { op, message, text });
            return out;
        }
        if let Some(to) = email_to {
            out.push(Output::EmailSend {
                op,
                message,
                to,
                bytes: aged.encode(),
            });
            return out;
        }
        out.push(Output::Transmit {
            op,
            message,
            cla,
            peer,
            bytes: aged.encode(),
            proof,
            crossing,
        });
        out
    }

    /// Records a send below the contact's floor in the privacy log.
    fn log_below_floor(
        &mut self,
        contact: ContactId,
        message: MsgId,
        cla: ClaId,
        grade: PrivacyGrade,
        basis: ConsentBasis,
    ) {
        let floor = self
            .contacts
            .get(&contact)
            .map_or(DEFAULT_FLOOR, |c| c.floor);
        self.privacy_log.push(PrivacyLogEntry {
            at_unix_ms: self.now_unix_ms,
            contact,
            message,
            cla,
            grade,
            floor,
            basis,
        });
    }

    /// What a send over `cla` needs beyond the bundle: the contact's address for email,
    /// the exposed text for P5.
    fn channel_extras(
        &self,
        cla: ClaId,
        message: MsgId,
        item: &OutboxItem,
    ) -> Result<(Option<String>, Option<String>), RejectReason> {
        match cla {
            ClaId::Email => self
                .email_to(item.contact)
                .map(|to| (Some(to), None))
                .ok_or(RejectReason::Unreachable),
            ClaId::Plain => self
                .plain_text_of(message, item)
                .map(|t| (None, Some(t)))
                .ok_or(RejectReason::NotPlainText),
            _ => Ok((None, None)),
        }
    }

    /// The text a P5 send may expose: only a 1:1 text message, never control traffic
    /// or group copies.
    fn plain_text_of(&self, message: MsgId, item: &OutboxItem) -> Option<String> {
        if item.content.is_some() || item.bundle.kind() != BundleKind::Message {
            return None;
        }
        self.messages
            .iter()
            .find(|m| m.id == message && m.kind == MessageKind::Text && m.group.is_none())
            .map(|m| m.text.clone())
    }

    /// Proof for `peer` that we, the sender, hand it our own bundle.
    fn direct_proof(
        &self,
        peer: PeerHandle,
        contact: ContactId,
        bundle: &Bundle,
    ) -> Option<[u8; 16]> {
        let their_id = self.peers.get(&peer)?.their_id?;
        let c = self.contacts.get(&contact)?;
        Some(
            c.secret
                .direct_proof(bundle.bundle_id().as_bytes(), &their_id),
        )
    }

    fn transmit_result(
        &mut self,
        op: OpId,
        result: Result<(), crate::agent::ClaError>,
    ) -> Vec<Output> {
        let (message, cla, grade) = match self.ops.remove(&op) {
            Some(Op::Message {
                message,
                cla,
                grade,
                ..
            }) => (message, cla, grade),
            Some(Op::Ack { contact, ids, .. }) => {
                // Handed over: those acks are no longer owed. On failure they stay owed.
                if result.is_ok() {
                    self.acks_sent(contact, &ids);
                    return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
                }
                return Vec::new();
            }
            None => return Vec::new(),
        };
        let now = self.now_unix_ms;
        if cla == ClaId::Hub && result.is_ok() {
            self.hub.uploaded.insert(message);
        }
        // A per-device copy shows on the message it copies.
        let shown = self
            .outbox
            .get(&message)
            .and_then(|i| i.copy_of)
            .unwrap_or(message);
        if cla == ClaId::Email && result.is_ok() {
            self.email.sent.insert(message);
        }
        let status = match result {
            Ok(()) => {
                if let Some(m) = self.messages.iter_mut().find(|m| m.id == shown)
                    // A copy adds its channel once; the message's own sends list every use.
                    && (shown == message || !m.channels.iter().any(|c| c.cla == cla))
                {
                    m.channels.push(ChannelUse {
                        cla,
                        grade,
                        at_unix_ms: now,
                    });
                }
                if let Some((contact, acks)) = self
                    .outbox
                    .get(&message)
                    .map(|i| (i.contact, i.acks.clone()))
                {
                    self.acks_sent(contact, &acks);
                }
                DeliveryStatus::Sent
            }
            Err(ClaError::Cancelled) => return Vec::new(),
            // A copy that failed leaves the message as it is: its own send decides.
            Err(ClaError::Failed) if shown != message => {
                return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
            }
            Err(ClaError::Failed) => DeliveryStatus::Failed,
        };
        let Some(status) = self.set_status(shown, status) else {
            return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        };
        alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::StatusChanged {
                message: shown,
                status,
            },
        ]
    }

    /// Updates a message's status and returns the new one, or `None` if it did not change.
    /// `Delivered` is final; a failed retry does not erase an earlier successful send.
    fn set_status(&mut self, message: MsgId, status: DeliveryStatus) -> Option<DeliveryStatus> {
        let m = self.messages.iter_mut().find(|m| m.id == message)?;
        if m.status == DeliveryStatus::Delivered
            || m.status == status
            || (status == DeliveryStatus::Failed && !m.channels.is_empty())
        {
            return None;
        }
        m.status = status;
        Some(status)
    }

    /// Acks handed to a channel stop being owed.
    fn acks_sent(&mut self, contact: ContactId, ids: &[MsgId]) {
        if ids.is_empty() {
            return;
        }
        if let Some(owed) = self.pending_acks.get_mut(&contact) {
            owed.retain(|m| !ids.contains(m));
            if owed.is_empty() {
                self.pending_acks.remove(&contact);
                self.ack_due.remove(&contact);
            }
        }
        for id in ids {
            self.ack_from.remove(id);
        }
    }

    /// Drops in-flight operations on a peer that went away; their messages become due again on the
    /// next encounter.
    fn forget_peer_ops(&mut self, peer: PeerHandle) {
        self.ops.retain(|_, op| match op {
            Op::Message { peer: p, .. } | Op::Ack { peer: p, .. } => *p != Some(peer),
        });
        // Send counts belong to one encounter; a later encounter starts afresh even if the
        // runtime reuses the handle.
        self.sends.retain(|_, st| st.peer != peer);
    }

    // ------------------------------------------------------------- encounters

    /// Opens an encounter: our `hello`, then our receiving tags.
    fn encounter_started(
        &mut self,
        peer: PeerHandle,
        cla: ClaId,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if self.disabled.contains(&cla) || !cla.caps().needs_peer {
            return alloc::vec![Output::EndEncounter { peer }];
        }
        self.peers.insert(
            peer,
            Peer {
                cla,
                hello: false,
                tags: BTreeSet::new(),
                bundles_received: 0,
                courier_usage: EncounterUsage::for_channel(cla),
                our_id: [0; 16],
                their_id: None,
                held: None,
                wanted: None,
                handed: BTreeSet::new(),
                sent: EncounterUsage::for_channel(cla),
                tokens_sent: BTreeSet::new(),
                batch: 0,
                spread: BTreeMap::new(),
                sprayed: 0,
                offered: BTreeSet::new(),
                claimed: BTreeSet::new(),
                intro_checks: 0,
                intro_bad: false,
            },
        );
        let mut ephemeral_id = [0u8; 16];
        rng.fill_bytes(&mut ephemeral_id);
        let batch = self.courier.new_batch();
        if let Some(p) = self.peers.get_mut(&peer) {
            p.our_id = ephemeral_id;
            p.batch = batch;
        }
        alloc::vec![
            Output::Encounter {
                peer,
                msg: EncounterMsg::Hello {
                    version: ENCOUNTER_VERSION,
                    ephemeral_id,
                },
            },
            Output::Encounter {
                peer,
                msg: EncounterMsg::Tags(self.advertised_tags(rng)),
            },
            Output::Encounter {
                peer,
                msg: self.summary(rng),
            },
        ]
    }

    fn encounter_message(
        &mut self,
        peer: PeerHandle,
        msg: EncounterMsg,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(p) = self.peers.get_mut(&peer) else {
            return Vec::new();
        };
        let cla = p.cla;
        match msg {
            EncounterMsg::Hello {
                version,
                ephemeral_id,
            } if version == ENCOUNTER_VERSION && !p.hello => {
                p.hello = true;
                p.their_id = Some(ephemeral_id);
                Vec::new()
            }
            // Anything before a valid hello, a second hello, or `bye` ends the encounter.
            _ if !p.hello => self.end_encounter(peer),
            EncounterMsg::Hello { .. } | EncounterMsg::Bye => self.end_encounter(peer),
            EncounterMsg::Tags(tags) => {
                p.tags = tags.into_iter().collect();
                let mut out = alloc::vec![Output::ReachabilityChanged];
                out.extend(self.flush_peer(peer));
                out
            }
            EncounterMsg::Bundles(bundles) => {
                let mut out = Vec::new();
                for entry in bundles {
                    let (bytes, proof) = (entry.bytes, entry.proof);
                    let Some(p) = self.peers.get_mut(&peer) else {
                        break;
                    };
                    p.bundles_received = p.bundles_received.saturating_add(1);
                    if p.bundles_received > PEER_BUNDLE_QUOTA {
                        out.push(Output::Rejected(RejectReason::Quota));
                        out.extend(self.end_encounter(peer));
                        break;
                    }
                    out.extend(self.received_in_encounter(peer, cla, &bytes, proof, rng));
                }
                // A delivery here makes a new token: pass it on at once.
                let peers: Vec<PeerHandle> = self.peers.keys().copied().collect();
                for p in peers {
                    out.extend(self.send_tokens(p));
                }
                out
            }
            EncounterMsg::Summary { salt, held, wanted } => {
                p.held = SaltedBloom::from_bytes(&salt, held);
                p.wanted = SaltedBloom::from_bytes(&salt, wanted);
                let mut out = self.send_tokens(peer);
                out.extend(self.send_offer(peer));
                out.extend(self.flush_courier(peer));
                out
            }
            EncounterMsg::PurgeTokens(tokens) => self.apply_tokens(&tokens),
            EncounterMsg::Offer(tags) => {
                // Claim our own older tags; strangers' tags stay unanswered.
                let mine: Vec<[u8; 16]> = tags
                    .into_iter()
                    .filter(|t| {
                        self.contact_for_tag(&RendezvousTag::from_bytes(*t))
                            .is_some()
                            || (self.accept_intros && self.own_intro_week(t).is_some())
                    })
                    .collect();
                if mine.is_empty() {
                    Vec::new()
                } else {
                    alloc::vec![Output::Encounter {
                        peer,
                        msg: EncounterMsg::Want(mine),
                    }]
                }
            }
            EncounterMsg::Want(tags) => {
                // Only tags we offered count: a peer cannot pull anything else this way.
                let offered = p.offered.clone();
                p.claimed
                    .extend(tags.into_iter().filter(|t| offered.contains(t)));
                self.flush_courier(peer)
            }
        }
    }

    fn end_encounter(&mut self, peer: PeerHandle) -> Vec<Output> {
        self.peers.remove(&peer);
        self.forget_peer_ops(peer);
        alloc::vec![Output::EndEncounter { peer }, Output::ReachabilityChanged]
    }

    /// Our receiving tags for the current epoch only, padded with random decoys and sorted, so
    /// the list does not reveal how many contacts we have beyond a power-of-two bound, nor
    /// which entries are real. Advertising a single epoch keeps different
    /// days unlinkable; senders match the epoch window on their side.
    fn advertised_tags(&mut self, rng: &mut impl CryptoRng) -> Vec<[u8; 16]> {
        let me = self.identity.public();
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        self.advertised_epoch = epoch;
        let mut tags: BTreeSet<[u8; 16]> = self
            .contacts
            .values()
            .map(|c| self.my_tag(c, epoch))
            .take(MAX_TAGS - 1)
            .collect();
        if self.accept_intros {
            tags.insert(intro_tag(&me, week_for_unix_ms(self.now_unix_ms)));
        }
        let target = tags
            .len()
            .max(MIN_ADVERTISED_TAGS)
            .next_power_of_two()
            .min(MAX_TAGS);
        while tags.len() < target {
            let mut decoy = [0u8; 16];
            rng.fill_bytes(&mut decoy);
            tags.insert(decoy);
        }
        tags.into_iter().collect()
    }

    /// Re-sends our tags to every connected peer (new contact, new epoch).
    fn advertise_all(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let peers: Vec<PeerHandle> = self.peers.keys().copied().collect();
        let mut out = Vec::new();
        for peer in peers {
            out.push(Output::Encounter {
                peer,
                msg: EncounterMsg::Tags(self.advertised_tags(rng)),
            });
        }
        out
    }

    fn on_tick(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let mut out = self.expire_outbox();
        out.extend(self.expire_rules());
        out.extend(self.resume_pow());
        if let Some(w) = self.courier.expire(self.now_unix_ms) {
            out.push(Output::Persist(StorageOp::Store(alloc::vec![w])));
        }
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        if epoch != self.advertised_epoch && !self.peers.is_empty() {
            out.extend(self.advertise_all(rng));
        }
        // The epoch window may have moved, and in-encounter retransmission timers may have run
        // out.
        out.extend(self.rotate_mailbox(rng));
        out.extend(self.reseal_outdated(rng));
        out.extend(self.standalone_key_updates(rng));
        out.extend(self.sync_own_devices(rng));
        // Acks first: one going via couriers is then handed on in this same tick.
        out.extend(self.standalone_acks(rng));
        out.extend(self.hub_tick(rng));
        out.extend(self.flush_email());
        let peers: Vec<PeerHandle> = self.peers.keys().copied().collect();
        for peer in peers {
            out.extend(self.flush_peer(peer));
            out.extend(self.send_tokens(peer));
            out.extend(self.send_offer(peer));
            out.extend(self.flush_courier(peer));
        }
        out
    }

    /// Drops standing rules whose end date passed; they already stopped
    /// applying then. The privacy log keeps the sends they allowed.
    fn expire_rules(&mut self) -> Vec<Output> {
        let now = self.now_unix_ms;
        let before = self.rules.len();
        self.rules
            .retain(|r| r.until_unix_ms.is_none_or(|t| now < t));
        if self.rules.len() == before {
            Vec::new()
        } else {
            alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
        }
    }

    /// Removes messages whose lifetime ran out without an ack.
    fn expire_outbox(&mut self) -> Vec<Output> {
        let now = self.now_unix_ms;
        let expired: Vec<MsgId> = self
            .outbox
            .iter()
            .filter(|(_, i)| {
                now.saturating_sub(i.created_unix_ms) >= u64::from(i.bundle.lifetime_s()) * 1000
            })
            .map(|(id, _)| *id)
            .collect();
        let mut out = Vec::new();
        for id in expired {
            self.outbox.remove(&id);
            self.sends.remove(&id);
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == id)
                && m.status != DeliveryStatus::Delivered
            {
                // Never confirmed within its lifetime: report it as failed even if it was sent.
                m.status = DeliveryStatus::Failed;
                out.push(Output::StatusChanged {
                    message: id,
                    status: DeliveryStatus::Failed,
                });
            }
        }
        let before = self.delivered.len();
        self.delivered.retain(|_, d| d.expires_unix_ms > now);
        if !out.is_empty() || self.delivered.len() != before {
            out.insert(0, Output::Persist(StorageOp::SaveSnapshot));
        }
        out
    }

    /// Sends acks no message carried within [`ACK_DELAY_MS`] as standalone ack bundles, over an
    /// automatic channel within the contact's floor. Below the floor they wait
    /// for a piggyback; standalone acks never ask and never use a standing rule.
    fn standalone_acks(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let now = self.now_unix_ms;
        // Acks restored from a snapshot have no due time yet.
        for contact in self.pending_acks.keys() {
            self.ack_due.entry(*contact).or_insert(now);
        }
        let due: Vec<ContactId> = self
            .ack_due
            .iter()
            .filter(|(c, since)| {
                now.saturating_sub(**since) >= ACK_DELAY_MS
                    && !self
                        .ops
                        .values()
                        .any(|op| matches!(op, Op::Ack { contact, .. } if contact == *c))
            })
            .map(|(c, _)| *c)
            .collect();
        let mut out = Vec::new();
        for contact_id in due {
            // The main device's acks first, then one other device's.
            let Some((target, ids)) = self.next_ack_batch(contact_id) else {
                self.ack_due.remove(&contact_id);
                continue;
            };
            let direct = ClaId::ALL
                .into_iter()
                .filter(|c| c.caps().needs_peer && !self.disabled.contains(c))
                .find_map(|c| self.peer_for_target(contact_id, target, c).map(|p| (p, c)));
            let Some(contact) = self.contacts.get(&contact_id) else {
                self.pending_acks.remove(&contact_id);
                self.ack_due.remove(&contact_id);
                continue;
            };
            // No direct path: couriers may carry the ack where they may carry messages,
            // only if a courier is here to take it.
            let via_couriers = direct.is_none()
                && !self.disabled.contains(&ClaId::Courier)
                && (ClaId::Courier.grade().within(contact.floor)
                    || self
                        .matching_rule(contact_id, ClaId::Courier, ClaId::Courier.grade())
                        .is_some())
                && self.peers.values().any(|p| p.held.is_some());
            let route = match direct {
                Some((peer, cla)) if cla.grade().within(contact.floor) => Some((peer, cla)),
                _ => None,
            };
            // The hub carries acks wherever it may carry messages without asking.
            let via_hub = route.is_none()
                && self.hub.online
                && !self.disabled.contains(&ClaId::Hub)
                && ClaId::Hub.grade().within(contact.floor);
            // Email carries acks only where a standing rule allows email for that contact
            // (it is below the default floor; acks never ask).
            let via_email = route.is_none() && !via_hub && self.email_carries_acks(contact_id);
            if route.is_none() && !via_hub && !via_email && !via_couriers {
                continue;
            }
            let Some((bundle, id)) = self.seal_ack(contact_id, target, &ids, rng) else {
                continue;
            };
            if via_hub {
                out.push(self.ack_via_hub(contact_id, ids, &bundle, id));
                continue;
            }
            if via_email {
                out.extend(self.ack_via_email(contact_id, ids, &bundle, id));
                continue;
            }
            let Some((peer, cla)) = route else {
                out.extend(self.ack_via_couriers(contact_id, &ids, bundle));
                continue;
            };
            let op = OpId(self.next_id());
            self.ops.insert(
                op,
                Op::Ack {
                    contact: contact_id,
                    ids,
                    peer: Some(peer),
                },
            );
            let proof = self.direct_proof(peer, contact_id, &bundle);
            out.push(Output::Transmit {
                op,
                message: id,
                cla,
                peer: Some(peer),
                bytes: bundle.encode(),
                proof,
                crossing: None,
            });
        }
        out
    }

    /// Puts a standalone ack into our own courier store; the normal spray takes it from there.
    fn ack_via_couriers(
        &mut self,
        contact: ContactId,
        ids: &[MsgId],
        bundle: Bundle,
    ) -> Vec<Output> {
        let Ok(relay) = bundle.with_relay(MAX_HOPS, self.spray_copies) else {
            return Vec::new();
        };
        let bytes = relay.encode();
        let mut usage = EncounterUsage::default();
        let batch = self.courier.new_batch();
        match self.courier.admit(
            &relay,
            &bytes,
            CourierClass::ToContact,
            self.courier_limit.max(DEFAULT_COURIER_BYTES),
            &mut usage,
            batch,
            self.now_unix_ms,
        ) {
            Ok(writes) => {
                self.acks_sent(contact, ids);
                alloc::vec![
                    Output::Persist(StorageOp::Store(writes)),
                    Output::Persist(StorageOp::SaveSnapshot),
                ]
            }
            Err(_) => Vec::new(),
        }
    }

    /// Whether email may carry acks to `contact`: connected, an address, and email within the
    /// floor or under a standing rule (acks never ask).
    fn email_carries_acks(&self, contact: ContactId) -> bool {
        let Some(c) = self.contacts.get(&contact) else {
            return false;
        };
        self.email.online
            && !self.disabled.contains(&ClaId::Email)
            && self.email_to(contact).is_some()
            && (ClaId::Email.grade().within(c.floor)
                || self
                    .matching_rule(contact, ClaId::Email, ClaId::Email.grade())
                    .is_some())
    }

    /// Mails a standalone ack; the ids stop being owed once submitted.
    fn ack_via_email(
        &mut self,
        contact: ContactId,
        ids: Vec<MsgId>,
        bundle: &Bundle,
        id: MsgId,
    ) -> Option<Output> {
        let to = self.email_to(contact)?;
        let op = OpId(self.next_id());
        self.ops.insert(
            op,
            Op::Ack {
                contact,
                ids,
                peer: None,
            },
        );
        Some(Output::EmailSend {
            op,
            message: id,
            to,
            bytes: bundle.encode(),
        })
    }

    /// Hands a standalone ack to the hub: the ids stop being owed on `put_ok`.
    fn ack_via_hub(
        &mut self,
        contact: ContactId,
        ids: Vec<MsgId>,
        bundle: &Bundle,
        id: MsgId,
    ) -> Output {
        let op = OpId(self.next_id());
        self.ops.insert(
            op,
            Op::Ack {
                contact,
                ids,
                peer: None,
            },
        );
        Output::Transmit {
            op,
            message: id,
            cla: ClaId::Hub,
            peer: None,
            bytes: bundle.encode(),
            proof: None,
            // An ack follows its message: it may cross when a message to this contact may.
            crossing: Some(self.hub_crossing(contact)),
        }
    }

    /// A standalone ack bundle for `ids` to `contact`.
    /// The acks to send next to `contact_id`: those for its main device, else those for one of
    /// its other devices. Acks owed to a device we hold no key for can never go: they are
    /// dropped (its sender keeps retrying, and its retries are acked again once it is known).
    fn next_ack_batch(&mut self, contact_id: ContactId) -> Option<(Option<[u8; 8]>, Vec<MsgId>)> {
        let main = self.owed_acks(contact_id, None);
        if !main.is_empty() {
            return Some((None, main));
        }
        let devices: Vec<[u8; 8]> = self
            .ack_from
            .values()
            .filter(|(c, _)| *c == contact_id)
            .map(|(_, d)| *d)
            .collect();
        for d in devices {
            let ids = self.owed_acks(contact_id, Some(d));
            let reachable = self
                .contacts
                .get(&contact_id)
                .is_some_and(|c| self.fresh_devices(c).contains(&d));
            if reachable && !ids.is_empty() {
                return Some((Some(d), ids));
            }
            self.acks_sent(contact_id, &ids);
        }
        None
    }

    fn seal_ack(
        &mut self,
        contact_id: ContactId,
        target: Option<[u8; 8]>,
        ids: &[MsgId],
        rng: &mut impl CryptoRng,
    ) -> Option<(Bundle, MsgId)> {
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        let purge_key = self.purge_key.as_ref()?;
        let contact = self.contacts.get(&contact_id)?;
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let token = purge_key.derive_token(&bundle_id);
        let commitment = commit(&token);
        let raw: Vec<[u8; 16]> = ids.iter().map(|m| m.0).collect();
        let content = encode_ack_ids(&raw).ok()?;
        self.seal(
            contact,
            Sealing {
                kind: BundleKind::Ack,
                content_type: CONTENT_TYPE_ACK,
                content,
                ack_requested: None,
                acks: Vec::new(),
                lifetime_s: ACK_LIFETIME_S,
                key_update: self.key_update_to(contact, target),
                id: None,
                recipient: target.map_or(Recipient::Mailbox, Recipient::Device),
            },
            bundle_id,
            &token,
            commitment,
            rng,
        )
        .ok()
    }

    /// `contact`'s receiving tags for our epoch ±1 (receivers accept that
    /// window, so a peer advertising any of them is reachable).
    /// Tags our bundles to `contact` go to: its intro tags while our request is unanswered
    /// (that is all we send then), its receiving tags otherwise.
    fn delivery_tags(&self, contact: &Contact) -> [[u8; 16]; 3] {
        if contact.requested {
            self.intro_tags_of(&contact.identity)
        } else {
            self.contact_tags(contact)
        }
    }

    /// Tags a bundle for `device` of `contact` goes to (the main device when `None`).
    fn target_tags(&self, contact: &Contact, device: Option<[u8; 8]>) -> [[u8; 16]; 3] {
        let Some(id) = device else {
            return self.delivery_tags(contact);
        };
        let direction = Direction::towards(&contact.identity, &self.identity.public());
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        [epoch.saturating_sub(1), epoch, epoch.saturating_add(1)]
            .map(|e| contact.secret.device_tag(direction, e, &id))
    }

    /// The newest connected peer on `cla` that receives for `device` of `contact`: each device
    /// advertises its own tags, so a peer gets only the bundles meant for it.
    fn peer_for_target(
        &self,
        contact: ContactId,
        device: Option<[u8; 8]>,
        cla: ClaId,
    ) -> Option<PeerHandle> {
        if device.is_none() {
            return self.peer_for(contact, cla);
        }
        let tags = self.target_tags(self.contacts.get(&contact)?, device);
        self.peers
            .iter()
            .rev()
            .find(|(_, p)| p.cla == cla && p.hello && tags.iter().any(|t| p.tags.contains(t)))
            .map(|(h, _)| *h)
    }

    fn contact_tags(&self, contact: &Contact) -> [[u8; 16]; 3] {
        let me = self.identity.public();
        let direction = Direction::towards(&contact.identity, &me);
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        [epoch.saturating_sub(1), epoch, epoch.saturating_add(1)]
            .map(|e| contact.secret.tag(direction, e))
    }

    /// The newest connected peer on `cla` that advertises one of `contact`'s receiving tags.
    /// Preferring the newest means a reconnected device is used at once, while a stale encounter
    /// with it times out. Older encounters are not ended: tags are visible to
    /// anyone on the network, so a stranger replaying them must not be able to cut a contact off.
    fn peer_for(&self, contact: ContactId, cla: ClaId) -> Option<PeerHandle> {
        let tags = self.delivery_tags(self.contacts.get(&contact)?);
        self.peers
            .iter()
            .rev()
            .find(|(_, p)| p.cla == cla && p.hello && tags.iter().any(|t| p.tags.contains(t)))
            .map(|(h, _)| *h)
    }

    /// Whether unconfirmed `message` should be sent to `peer` now: it is still in
    /// the outbox, has nothing in flight, and either was not sent in this encounter yet, or its
    /// retransmission timer ran out and it has sends left for this encounter.
    fn due_on(&self, message: MsgId, peer: PeerHandle) -> bool {
        if !self.outbox.contains_key(&message)
            || self
                .ops
                .values()
                .any(|op| matches!(op, Op::Message { message: m, .. } if *m == message))
        {
            return false;
        }
        match self.sends.get(&message) {
            Some(st) if st.peer == peer => {
                st.count < MAX_SENDS_PER_ENCOUNTER
                    && self.now_unix_ms.saturating_sub(st.last_unix_ms) >= RETRANSMIT_AFTER_MS
            }
            _ => true,
        }
    }

    /// Hand a queued message to a connected peer that can deliver
    /// it, if the consent gate allows that without asking.
    fn flush_message(&mut self, message: MsgId) -> Vec<Output> {
        let Some((contact, device)) = self.outbox.get(&message).map(|i| (i.contact, i.device))
        else {
            return Vec::new();
        };
        let Some((peer, cla)) = ClaId::ALL
            .into_iter()
            .filter(|c| c.caps().needs_peer)
            .find_map(|c| self.peer_for_target(contact, device, c).map(|p| (p, c)))
        else {
            // No one nearby: the hub, if connected, takes it, and email where a
            // standing rule allows it.
            let mut out = self.flush_hub();
            out.extend(self.flush_email());
            return out;
        };
        if !self.due_on(message, peer) {
            return Vec::new();
        }
        match self.authorize(message, cla, Ask::Never) {
            Ok(auth) => self.transmit(auth, Some(peer)),
            Err(_) => Vec::new(),
        }
    }

    /// Sends every unconfirmed message that is due on this peer, oldest first.
    fn flush_peer(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(cla) = self.peers.get(&peer).map(|p| p.cla) else {
            return Vec::new();
        };
        let mut candidates: Vec<(u64, MsgId)> = self
            .outbox
            .iter()
            .filter(|(id, item)| {
                self.due_on(**id, peer)
                    && self.peer_for_target(item.contact, item.device, cla) == Some(peer)
            })
            .map(|(id, item)| (item.created_unix_ms, *id))
            .collect();
        candidates.sort_unstable();
        let candidates: Vec<MsgId> = candidates.into_iter().map(|(_, id)| id).collect();
        let mut out = Vec::new();
        for message in candidates {
            if let Ok(auth) = self.authorize(message, cla, Ask::Never) {
                out.extend(self.transmit(auth, Some(peer)));
            }
        }
        out
    }

    // ------------------------------------------------------------ couriers

    /// Our `summary`: salted blooms over what we hold, and over carried commitments we have no
    /// token for.
    fn summary(&self, rng: &mut impl CryptoRng) -> EncounterMsg {
        let mut salt = [0u8; 16];
        rng.fill_bytes(&mut salt);
        let held_items: Vec<[u8; 32]> = self
            .courier
            .entries()
            .map(|m| key_bytes(&m.key))
            .chain(self.outbox.values().map(|i| courier_key_bytes(&i.bundle)))
            .collect();
        let mut held = SaltedBloom::with_capacity(&salt, held_items.len());
        for k in &held_items {
            held.insert(k);
        }
        let mut wanted = SaltedBloom::with_capacity(&salt, self.courier.len());
        for m in self.courier.entries() {
            wanted.insert(&m.key.commitment);
        }
        EncounterMsg::Summary {
            salt,
            held: held.as_bytes().to_vec(),
            wanted: wanted.as_bytes().to_vec(),
        }
    }

    /// Purge tokens for a peer: first those it wants, then the newest, at most 1024.
    /// Every node gossips every token it holds.
    fn send_tokens(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(p) = self.peers.get(&peer) else {
            return Vec::new();
        };
        // Courier exchange starts with the peer's summary; peers that send none get no tokens.
        if p.held.is_none() {
            return Vec::new();
        }
        let mut all: Vec<(bool, u64, [u8; 16], [u8; 16])> = self
            .tombstones
            .iter()
            .filter(|(c, _)| !p.tokens_sent.contains(c.as_bytes()))
            .map(|(c, t)| {
                let wanted = p.wanted.as_ref().is_some_and(|w| w.contains(c.as_bytes()));
                (
                    wanted,
                    t.expires_unix_ms,
                    *t.token.as_bytes(),
                    *c.as_bytes(),
                )
            })
            .collect();
        if all.is_empty() {
            return Vec::new();
        }
        all.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        all.truncate(MAX_PURGE_TOKENS);
        if let Some(p) = self.peers.get_mut(&peer) {
            p.tokens_sent.extend(all.iter().map(|x| x.3));
        }
        alloc::vec![Output::Encounter {
            peer,
            msg: EncounterMsg::PurgeTokens(all.into_iter().map(|x| x.2).collect()),
        }]
    }

    /// Offers a courier peer the tags of carried bundles it did not advertise, newest first, each
    /// once per encounter: bundles older than a day carry a tag the recipient no
    /// longer advertises.
    fn send_offer(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(p) = self.peers.get(&peer) else {
            return Vec::new();
        };
        if p.held.is_none() {
            return Vec::new();
        }
        let mut entries: Vec<(u64, [u8; 16])> = self
            .courier
            .entries()
            .filter(|m| !p.tags.contains(&m.dst_tag) && !p.offered.contains(&m.dst_tag))
            .map(|m| (m.admitted_unix_ms, m.dst_tag))
            .collect();
        entries.sort_unstable_by(|a, b| b.cmp(a));
        let mut tags: Vec<[u8; 16]> = Vec::new();
        for (_, t) in entries {
            if !tags.contains(&t) {
                tags.push(t);
            }
            if tags.len() == MAX_OFFER_TAGS {
                break;
            }
        }
        if tags.is_empty() {
            return Vec::new();
        }
        if let Some(p) = self.peers.get_mut(&peer) {
            p.offered.extend(tags.iter().copied());
        }
        alloc::vec![Output::Encounter {
            peer,
            msg: EncounterMsg::Offer(tags),
        }]
    }

    /// Applies gossiped purge tokens: tombstone, drop carried copies, refuse them from now on.
    /// Presenting a token proves nothing about the presenter.
    fn apply_tokens(&mut self, tokens: &[[u8; 16]]) -> Vec<Output> {
        let mut writes = Vec::new();
        for t in tokens {
            let token = PurgeToken::from_bytes(*t);
            let c = commit(&token);
            if self.tombstones.contains_key(&c) {
                continue;
            }
            writes.extend(self.tombstone(c, token));
        }
        if writes.is_empty() {
            Vec::new()
        } else {
            alloc::vec![Output::Persist(StorageOp::Store(writes))]
        }
    }

    /// Whether our own message may be handed to couriers without asking: within the contact's
    /// floor, a standing rule, or the user's explicit choice for this message.
    fn couriers_allowed(&self, message: MsgId) -> Option<CourierPermit> {
        let item = self.outbox.get(&message)?;
        if self.disabled.contains(&ClaId::Courier) {
            return None;
        }
        let contact = self.contacts.get(&item.contact)?;
        let grade = ClaId::Courier.grade();
        if grade.within(contact.floor) || item.courier_ok {
            return Some(CourierPermit::WithinFloor);
        }
        self.matching_rule(item.contact, ClaId::Courier, grade)
            .map(|r| CourierPermit::Rule(ConsentBasis::ExistingRule(r)))
    }

    /// The user chose couriers for a message (directly or after the consent dialog).
    fn allow_couriers(&mut self, message: MsgId, basis: Option<ConsentBasis>) -> Vec<Output> {
        let Some(item) = self.outbox.get_mut(&message) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        };
        item.courier_ok = true;
        let contact = item.contact;
        if let Some(basis) = basis {
            let floor = self
                .contacts
                .get(&contact)
                .map_or(DEFAULT_FLOOR, |c| c.floor);
            self.privacy_log.push(PrivacyLogEntry {
                at_unix_ms: self.now_unix_ms,
                contact,
                message,
                cla: ClaId::Courier,
                grade: ClaId::Courier.grade(),
                floor,
                basis,
            });
        }
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::CourierQueued { message },
        ];
        let peers: Vec<PeerHandle> = self.peers.keys().copied().collect();
        for peer in peers {
            out.extend(self.flush_courier(peer));
        }
        out
    }

    /// Courier exchange with a peer that sent a summary: deliver carried bundles
    /// to it if it advertises their tag, then hand spray copies of carried and own bundles on.
    fn flush_courier(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(p) = self.peers.get(&peer) else {
            return Vec::new();
        };
        if p.held.is_none() || !p.hello || self.disabled.contains(&ClaId::Courier) {
            return Vec::new();
        }
        // Today's advertised tags, plus older ones the peer claimed from our offer.
        let mut tags = p.tags.clone();
        tags.extend(p.claimed.iter().copied());
        let mut out = self.flush_carried(peer, &tags);
        out.extend(self.flush_own(peer, &tags));
        out
    }

    /// Carried bundles, highest priority class first, then oldest: delivered to a peer that
    /// advertises their tag, otherwise half the copies handed on (Binary Spray-and-Wait).
    fn flush_carried(&mut self, peer: PeerHandle, tags: &BTreeSet<[u8; 16]>) -> Vec<Output> {
        let now = self.now_unix_ms;
        let mut out = Vec::new();
        let mut writes = Vec::new();
        // Deliveries first; then spray, favouring bundles that arrived in small batches (honest
        // traffic) and capping what any one batch may spread per encounter.
        let mut carried: Vec<(
            bool,
            CourierClass,
            u64,
            core::cmp::Reverse<u64>,
            CourierMeta,
        )> = self
            .courier
            .entries()
            .map(|m| {
                (
                    !tags.contains(&m.dst_tag),
                    m.class,
                    self.courier.batch_bytes(m.batch),
                    core::cmp::Reverse(m.age_ms(now)),
                    m.clone(),
                )
            })
            .collect();
        carried.sort_by_key(|x| (x.0, x.1, x.2, x.3));
        for (spray, _, _, _, m) in carried {
            if m.expired(now) || !self.may_send(peer, &key_bytes(&m.key), m.size) {
                continue;
            }
            let (hops, copies, keep) = if !spray {
                (m.hops.saturating_sub(1), 1, m.copies)
            } else if m.copies > 1 && m.hops >= 2 {
                // Bundles from a flood (an arrival batch above FLOOD_BATCH_BYTES) are only
                // delivered, never handed on: passed on, they would land in other devices'
                // batches beside honest bundles and get those evicted.
                if self.courier.batch_bytes(m.batch) > FLOOD_BATCH_BYTES {
                    continue;
                }
                // One spray allowance per encounter for everything carried, served smallest
                // arrival batch first (the sort above): honest hand-offs come in small batches,
                // so junk rarely gets a turn, whatever size its batches are.
                let (spread, sprayed) = self.peers.get(&peer).map_or((0, 0), |p| {
                    (p.spread.get(&m.batch).copied().unwrap_or(0), p.sprayed)
                });
                let size = u64::from(m.size);
                if spread.saturating_add(size) > SPRAY_PER_BATCH_BYTES
                    || sprayed.saturating_add(size) > SPRAY_PER_ENCOUNTER_BYTES
                {
                    continue;
                }
                if let Some(p) = self.peers.get_mut(&peer) {
                    *p.spread.entry(m.batch).or_default() += size;
                    p.sprayed += size;
                }
                (m.hops - 1, m.copies / 2, m.copies - m.copies / 2)
            } else {
                continue;
            };
            self.mark_sent(peer, key_bytes(&m.key), m.size);
            if keep != m.copies {
                writes.extend(self.courier.set_copies(&m.key, keep));
            }
            out.push(Output::Forward {
                peer,
                key: m.key,
                hops,
                copies,
                age_ms: m.age_ms(now),
            });
        }
        if !writes.is_empty() {
            out.push(Output::Persist(StorageOp::Store(writes)));
        }
        out
    }

    /// Our own messages: spray copies to couriers, never to the recipient (that is a direct
    /// send, with its proof), and only where couriers are allowed.
    fn flush_own(&mut self, peer: PeerHandle, tags: &BTreeSet<[u8; 16]>) -> Vec<Output> {
        let now = self.now_unix_ms;
        let mut out = Vec::new();
        let mut own: Vec<(u64, MsgId)> = self
            .outbox
            .iter()
            .filter(|(_, i)| i.copies > 1)
            .map(|(id, i)| (i.created_unix_ms, *id))
            .collect();
        own.sort_unstable();
        for (_, message) in own {
            let Some(permit) = self.couriers_allowed(message) else {
                continue;
            };
            let basis = match permit {
                CourierPermit::WithinFloor => None,
                CourierPermit::Rule(b) => Some(b),
            };
            let Some(item) = self.outbox.get(&message) else {
                continue;
            };
            let key = courier_key_bytes(&item.bundle);
            let commitment = PurgeCommitment::from_bytes(*item.bundle.purge_commitment());
            let to_peer = self
                .contacts
                .get(&item.contact)
                .is_some_and(|c| self.delivery_tags(c).iter().any(|t| tags.contains(t)));
            if to_peer || self.tombstones.contains_key(&commitment) {
                continue;
            }
            let age = now.saturating_sub(item.created_unix_ms);
            let give = item.copies / 2;
            let Ok(copy) = item
                .bundle
                .clone()
                .with_relay(MAX_HOPS - 1, give)
                .and_then(|b| b.with_age_ms(age))
            else {
                continue;
            };
            let bytes = copy.encode();
            let size = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
            if !self.may_send(peer, &key, size) {
                continue;
            }
            let contact = item.contact;
            self.mark_sent(peer, key, size);
            if let Some(item) = self.outbox.get_mut(&message) {
                item.copies -= give;
            }
            if let Some(basis) = basis {
                let floor = self
                    .contacts
                    .get(&contact)
                    .map_or(DEFAULT_FLOOR, |c| c.floor);
                self.privacy_log.push(PrivacyLogEntry {
                    at_unix_ms: now,
                    contact,
                    message,
                    cla: ClaId::Courier,
                    grade: ClaId::Courier.grade(),
                    floor,
                    basis,
                });
            }
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == message) {
                m.channels.push(ChannelUse {
                    cla: ClaId::Courier,
                    grade: ClaId::Courier.grade(),
                    at_unix_ms: now,
                });
            }
            if let Some(status) = self.set_status(message, DeliveryStatus::Sent) {
                out.push(Output::StatusChanged { message, status });
            }
            out.push(Output::Encounter {
                peer,
                msg: EncounterMsg::Bundles(alloc::vec![BundleEntry::bare(bytes)]),
            });
            out.push(Output::Persist(StorageOp::SaveSnapshot));
        }
        out
    }

    /// Whether `peer` may still receive this bundle from us in this encounter: not sent yet, not
    /// in its summary, and within its per-encounter allowance.
    fn may_send(&self, peer: PeerHandle, key: &[u8; 32], size: u32) -> bool {
        self.peers.get(&peer).is_some_and(|p| {
            !p.handed.contains(key)
                && !p.held.as_ref().is_some_and(|h| h.contains(key))
                && p.sent.allows(size)
        })
    }

    fn mark_sent(&mut self, peer: PeerHandle, key: [u8; 32], size: u32) {
        if let Some(p) = self.peers.get_mut(&peer) {
            p.handed.insert(key);
            p.sent.bundles = p.sent.bundles.saturating_add(1);
            p.sent.bytes = p.sent.bytes.saturating_add(u64::from(size));
        }
    }

    // ------------------------------------------------------- mailbox keys

    /// Our current mailbox key.
    pub(crate) fn current_mailbox(&self) -> &HeldMailbox {
        // The keyring is never empty: `new` and `restore` create one, rotation only adds.
        self.mailboxes
            .iter()
            .rev()
            .find(|h| h.retired_unix_ms.is_none())
            .or_else(|| self.mailboxes.last())
            .unwrap_or_else(|| unreachable!("keyring is never empty"))
    }

    /// Our current key, while `contact` has not confirmed it.
    fn key_update_for(&self, contact: &Contact) -> Option<KeyUpdate> {
        self.key_update_to(contact, None)
    }

    /// Rotates the current key after [`MAILBOX_ROTATE_MS`] and zeroizes retired keys after
    /// [`MAILBOX_RETAIN_MS`].
    fn rotate_mailbox(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let now = self.now_unix_ms;
        let mut changed = false;
        let (serial, created) = {
            let cur = self.current_mailbox();
            (cur.serial, cur.created_unix_ms)
        };
        if now.saturating_sub(created) >= MAILBOX_ROTATE_MS {
            for h in &mut self.mailboxes {
                if h.retired_unix_ms.is_none() {
                    h.retired_unix_ms = Some(now);
                }
            }
            self.mailboxes.push(HeldMailbox {
                serial: serial.saturating_add(1),
                created_unix_ms: now,
                retired_unix_ms: None,
                key: MailboxKey::generate(rng),
            });
            changed = true;
        }
        let before = self.mailboxes.len();
        // Dropping a `MailboxKey` zeroizes it.
        self.mailboxes.retain(|h| {
            h.retired_unix_ms
                .is_none_or(|r| now.saturating_sub(r) < MAILBOX_RETAIN_MS)
        });
        changed |= self.mailboxes.len() != before;
        if changed {
            alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
        } else {
            Vec::new()
        }
    }

    /// Removes a contact (the user asked). Group membership survives as group-only, so groups
    /// keep working; everything 1:1 goes: messages, queued sends, owed acks, rules, requests we
    /// sent. The privacy log keeps its entries (it is an audit of what was sent).
    pub(super) fn remove_contact(
        &mut self,
        id: ContactId,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if !self.contacts.contains_key(&id) || self.is_self(id) {
            return Vec::new();
        }
        let key = *id.as_bytes();
        let in_group = self
            .groups
            .values()
            .any(|g| g.admin == key || g.members.contains(&key) || g.invited.contains(&key));
        let gone: Vec<MsgId> = self
            .outbox
            .iter()
            .filter(|(_, i)| i.contact == id)
            .map(|(m, _)| *m)
            .collect();
        for m in &gone {
            self.outbox.remove(m);
            self.sends.remove(m);
            self.pending_intros.remove(m);
            self.solving.remove(m);
        }
        self.messages
            .retain(|m| m.contact != id || m.group.is_some());
        self.pending_acks.remove(&id);
        self.ack_due.remove(&id);
        self.ack_from.retain(|_, (c, _)| *c != id);
        self.rules.retain(|r| r.contact != Some(id));
        if in_group {
            if let Some(c) = self.contacts.get_mut(&id) {
                c.group_only = true;
                c.requested = false;
                c.email = None;
            }
        } else {
            self.contacts.remove(&id);
        }
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        // Its receiving tag is gone: tell connected peers our current tags.
        out.extend(self.advertise_all(rng));
        out
    }

    /// A bundle sealed to our X-Wing key of `serial` proves the contact holds it: stop announcing
    /// it, also to contacts who only ever acknowledge.
    fn confirm_xwing(&mut self, contact: ContactId, device: Option<[u8; 8]>, serial: Option<u32>) {
        if let Some(serial) = serial {
            self.confirm_key(contact, device, serial);
        }
    }

    /// `device` of `contact` (its main device when `None`) knows our key of `serial`, with its
    /// X-Wing half (our key updates carry it).
    fn confirm_key(&mut self, contact: ContactId, device: Option<[u8; 8]>, serial: u32) {
        let Some(c) = self.contacts.get_mut(&contact) else {
            return;
        };
        if let Some(d) = device.and_then(|d| c.devices.iter_mut().find(|x| x.id == d)) {
            d.told_serial = d.told_serial.max(serial);
        } else {
            c.told_xwing = c.told_xwing.max(serial);
            c.told_serial = c.told_serial.max(serial);
        }
    }

    /// A contact announced a key: keep it if its serial is newer (replays are harmless).
    fn learn_key(&mut self, contact: ContactId, k: &KeyUpdate) {
        let now = self.now_unix_ms;
        let Some(c) = self.contacts.get_mut(&contact) else {
            return;
        };
        let xwing = k
            .xwing_pk
            .as_deref()
            .and_then(|b| XWingPublic::from_bytes(b).ok());
        // The X-Wing half of the key we already hold, from a contact who upgraded.
        if k.serial == c.mailbox_serial
            && c.xwing.is_none()
            && k.mailbox_pk == *c.mailbox.as_bytes()
        {
            c.xwing = xwing;
            return;
        }
        if k.serial <= c.mailbox_serial {
            return;
        }
        if let Ok(pk) = MailboxPublic::from_bytes(&k.mailbox_pk) {
            c.mailbox = pk;
            c.mailbox_serial = k.serial;
            c.mailbox_learned_ms = now;
            c.xwing = xwing;
        }
    }

    /// Whether an outbox item must wait: sealed to a contact key that may already be deleted, or
    /// to an older key than the one we now know (it is resealed on the next tick).
    fn waiting_for_key(&self, item: &OutboxItem) -> bool {
        let Some(c) = self.contacts.get(&item.contact) else {
            return false;
        };
        // A copy for another device: that device's key.
        if let Some(d) = item.device {
            return Self::target_key(c, Some(d)).is_none_or(|(serial, learned)| {
                serial > item.sealed_serial
                    || self.now_unix_ms.saturating_sub(learned) >= CONTACT_KEY_STALE_MS
            });
        }
        if item.bundle.kind() == BundleKind::Intro {
            // Sealed to the key in their card; nothing newer can exist before they answer.
            return false;
        }
        if item.bundle.kind() == BundleKind::KeyUpdate {
            // Resealed (to the identity key when stale) on the next tick; never held.
            return c.mailbox_serial > item.sealed_serial
                || (self.key_stale(c) && !item.via_identity);
        }
        c.mailbox_serial > item.sealed_serial
            || self.now_unix_ms.saturating_sub(item.sealed_learned_ms) >= CONTACT_KEY_STALE_MS
    }

    /// Whether every key of `c` we know may already be deleted on their side.
    fn key_stale(&self, c: &Contact) -> bool {
        self.now_unix_ms.saturating_sub(c.mailbox_learned_ms) >= CONTACT_KEY_STALE_MS
    }

    /// Reseals outbox items to the contact's newer key (or a key update to the identity key once
    /// the mailbox key is stale), keeping their message ids, and recalls the old copies by
    /// releasing their purge tokens.
    fn reseal_outdated(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let due: Vec<MsgId> = self
            .outbox
            .iter()
            .filter(|(_, i)| {
                i.bundle.kind() != BundleKind::Intro
                    && self
                        .contacts
                        .get(&i.contact)
                        .is_some_and(|c| match i.device {
                            Some(d) => Self::target_key(c, Some(d))
                                .is_some_and(|(serial, _)| serial > i.sealed_serial),
                            None => {
                                c.mailbox_serial > i.sealed_serial
                                    || (i.bundle.kind() == BundleKind::KeyUpdate
                                        && self.key_stale(c)
                                        && !i.via_identity)
                            }
                        })
            })
            .map(|(id, _)| *id)
            .collect();
        let mut writes = Vec::new();
        for message in due {
            writes.extend(self.reseal(message, rng));
        }
        if writes.is_empty() {
            Vec::new()
        } else {
            alloc::vec![
                Output::Persist(StorageOp::Store(writes)),
                Output::Persist(StorageOp::SaveSnapshot),
            ]
        }
    }

    fn reseal(&mut self, message: MsgId, rng: &mut impl CryptoRng) -> Vec<StoreWrite> {
        let Some(item) = self.outbox.get(&message) else {
            return Vec::new();
        };
        let (kind, contact_id, device) = (item.bundle.kind(), item.contact, item.device);
        let old_id = *item.bundle.bundle_id().as_bytes();
        let old_commitment = PurgeCommitment::from_bytes(*item.bundle.purge_commitment());
        let acks: Vec<[u8; 16]> = item.acks.iter().map(|m| m.0).collect();
        let (content_type, content, ack_requested) = if kind == BundleKind::KeyUpdate {
            (CONTENT_TYPE_KEY_UPDATE, Vec::new(), Some(true))
        } else if let Some((t, c)) = &item.content {
            (*t, c.clone(), Some(true))
        } else {
            let Some(text) = self
                .messages
                .iter()
                .find(|m| m.id == message)
                .map(|m| m.text.clone())
            else {
                return Vec::new();
            };
            let Ok(content) = encode_text(&text) else {
                return Vec::new();
            };
            (CONTENT_TYPE_TEXT, content, Some(true))
        };
        let (Some(contact), Some(purge_key)) = (self.contacts.get(&contact_id), &self.purge_key)
        else {
            return Vec::new();
        };
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let token = purge_key.derive_token(&bundle_id);
        let old_token = purge_key.derive_token(&old_id);
        let commitment = commit(&token);
        let key_update = self.key_update_to(contact, device);
        let key_serial = key_update.as_ref().map(|k| k.serial);
        let list_serial = key_update
            .as_ref()
            .and_then(|k| k.device_list.as_ref())
            .map(|l| l.serial);
        let Some((sealed_serial, sealed_learned_ms)) = Self::target_key(contact, device) else {
            return Vec::new();
        };
        // The identity fallback reaches only the main device.
        let to_identity =
            device.is_none() && kind == BundleKind::KeyUpdate && self.key_stale(contact);
        let Ok((bundle, _)) = self.seal(
            contact,
            Sealing {
                kind,
                content_type,
                content,
                ack_requested,
                acks,
                lifetime_s: self.message_lifetime_s,
                key_update,
                id: Some(message),
                recipient: match (to_identity, device) {
                    (true, _) => Recipient::Identity,
                    (false, Some(d)) => Recipient::Device(d),
                    (false, None) => Recipient::Mailbox,
                },
            },
            bundle_id,
            &token,
            commitment,
            rng,
        ) else {
            return Vec::new();
        };
        if let Some(item) = self.outbox.get_mut(&message) {
            item.bundle = bundle;
            item.key_serial = key_serial;
            item.list_serial = list_serial;
            item.sealed_serial = sealed_serial;
            item.sealed_learned_ms = sealed_learned_ms;
            item.via_identity = to_identity;
            item.copies = self.spray_copies;
        }
        self.sends.remove(&message);
        self.hub.uploaded.remove(&message);
        self.email.sent.remove(&message);
        // The old copies can never be opened now: recall them from couriers.
        self.tombstone(old_commitment, old_token)
    }

    /// Standalone key updates for the contacts' other devices that do not know our key of
    /// `serial` (once `key_due`) or our list of `list_serial`; `true` if any was queued.
    fn device_key_updates(
        &mut self,
        serial: u32,
        key_due: bool,
        list_serial: u32,
        pending: &BTreeSet<(ContactId, Option<[u8; 8]>)>,
        rng: &mut impl CryptoRng,
    ) -> bool {
        let due: Vec<(ContactId, [u8; 8])> = self
            .contacts
            .iter()
            .filter(|(_, c)| !c.requested)
            .flat_map(|(id, c)| {
                let fresh = self.fresh_devices(c);
                c.devices
                    .iter()
                    .filter(move |d| {
                        ((key_due && d.told_serial < serial) || d.told_list < list_serial)
                            && fresh.contains(&d.id)
                    })
                    .map(move |d| (*id, d.id))
            })
            .filter(|(id, d)| !pending.contains(&(*id, Some(*d))))
            .collect();
        for (contact_id, device) in &due {
            self.queue_copy(
                *contact_id,
                *device,
                None,
                BundleKind::KeyUpdate,
                CONTENT_TYPE_KEY_UPDATE,
                &[],
                rng,
            );
        }
        !due.is_empty()
    }

    /// A standalone key update for each contact not told within [`KEY_UPDATE_AFTER_MS`] after a
    /// rotation. It is a hidden outbox item: sent only automatically, within the
    /// floor or a standing rule, never asking, and confirmed by an ack like a message.
    fn standalone_key_updates(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let (serial, created) = {
            let cur = self.current_mailbox();
            (cur.serial, cur.created_unix_ms)
        };
        // A rotated key waits a day for a message to carry it; a new device list goes at once,
        // so contacts start writing to a new device.
        let key_due = self.now_unix_ms.saturating_sub(created) >= KEY_UPDATE_AFTER_MS;
        let list_serial = self.own_list_serial();
        let pending: BTreeSet<(ContactId, Option<[u8; 8]>)> = self
            .outbox
            .values()
            .filter(|i| i.bundle.kind() == BundleKind::KeyUpdate)
            .map(|i| (i.contact, i.device))
            .collect();
        let due: Vec<ContactId> = self
            .contacts
            .iter()
            .filter(|(id, c)| {
                let due = if self.is_self(**id) {
                    // Another device tells our main device its keys at once, X-Wing half
                    // included: that is how the main device learns it joined.
                    // The main device never writes to itself.
                    !self.is_main_device() && c.told_serial.min(c.told_xwing) < serial
                } else {
                    key_due && c.told_serial < serial
                };
                (due || c.told_list < list_serial)
                    && !c.requested
                    && !pending.contains(&(**id, None))
            })
            .map(|(id, _)| *id)
            .collect();
        // The other devices of each contact, each told separately.
        let devices = self.device_key_updates(serial, key_due, list_serial, &pending, rng);
        if due.is_empty() && !devices {
            return Vec::new();
        }
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        for contact_id in due {
            let (Some(contact), Some(purge_key)) =
                (self.contacts.get(&contact_id), &self.purge_key)
            else {
                continue;
            };
            let mut bundle_id = [0u8; 16];
            rng.fill_bytes(&mut bundle_id);
            let token = purge_key.derive_token(&bundle_id);
            let commitment = commit(&token);
            let key_update = self.key_update_for(contact);
            let key_serial = key_update.as_ref().map(|k| k.serial);
            let list_serial = key_update
                .as_ref()
                .and_then(|k| k.device_list.as_ref())
                .map(|l| l.serial);
            let (sealed_serial, sealed_learned_ms) =
                (contact.mailbox_serial, contact.mailbox_learned_ms);
            let to_identity = self.key_stale(contact);
            let Ok((bundle, id)) = self.seal(
                contact,
                Sealing {
                    kind: BundleKind::KeyUpdate,
                    content_type: CONTENT_TYPE_KEY_UPDATE,
                    content: Vec::new(),
                    ack_requested: Some(true),
                    acks: Vec::new(),
                    lifetime_s: self.message_lifetime_s,
                    key_update,
                    id: None,
                    recipient: if to_identity {
                        Recipient::Identity
                    } else {
                        Recipient::Mailbox
                    },
                },
                bundle_id,
                &token,
                commitment,
                rng,
            ) else {
                continue;
            };
            self.outbox.insert(
                id,
                OutboxItem {
                    contact: contact_id,
                    bundle,
                    created_unix_ms: self.now_unix_ms,
                    acks: Vec::new(),
                    copies: self.spray_copies,
                    courier_ok: false,
                    key_serial,
                    list_serial,
                    sealed_serial,
                    sealed_learned_ms,
                    via_identity: to_identity,
                    content: None,
                    device: None,
                    copy_of: None,
                },
            );
        }
        alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
    }

    fn set_channel_enabled(&mut self, cla: ClaId, enabled: bool) -> Vec<Output> {
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        if cla == ClaId::Hub && !enabled {
            self.hub.reset();
        }
        if cla == ClaId::Email && !enabled {
            self.email.online = false;
        }
        if enabled {
            self.disabled.remove(&cla);
        } else {
            self.disabled.insert(cla);
            let ended: Vec<PeerHandle> = self
                .peers
                .iter()
                .filter(|(_, p)| p.cla == cla)
                .map(|(h, _)| *h)
                .collect();
            for peer in ended {
                out.extend(self.end_encounter(peer));
            }
        }
        out
    }

    // -------------------------------------------------------------- receiving

    /// A bundle from an encounter peer: ours goes through [`Agent::received`]; anyone else's is
    /// offered to the courier store (in order: bounds, expiry, hops,
    /// tombstone, own tag, then courier admission).
    fn received_in_encounter(
        &mut self,
        peer: PeerHandle,
        cla: ClaId,
        bytes: &[u8],
        proof: Option<[u8; 16]>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Ok(bundle) = Bundle::decode(bytes) else {
            return self.received(cla, bytes, rng);
        };
        let commitment = PurgeCommitment::from_bytes(*bundle.purge_commitment());
        if self.tombstones.contains_key(&commitment) {
            return self.received(cla, bytes, rng);
        }
        if bundle.kind() == BundleKind::Intro {
            // Each proof costs an Argon2id run: within this encounter's budget only.
            // Without a contact secret nothing proves who sent it: label it couriers.
            if self.own_intro_week(bundle.dst_tag().as_bytes()).is_some() {
                if !self.spend_pow_check(peer) {
                    return Vec::new();
                }
                let out = self.received(ClaId::Courier, bytes, rng);
                if out.contains(&Output::Rejected(RejectReason::BadProofOfWork))
                    && let Some(p) = self.peers.get_mut(&peer)
                {
                    p.intro_bad = true;
                }
                return out;
            }
            if self.courier_limit == 0 || !self.courier_pow_ok(peer, &bundle) {
                return Vec::new();
            }
        }
        if let Some((_, contact)) = self.contact_for_tag(&bundle.dst_tag()) {
            // Honest provenance: only a valid proof shows the peer is the
            // sender; anything else may have been relayed and is labelled as couriers.
            let direct = proof.is_some_and(|pr| {
                self.peers.get(&peer).is_some_and(|p| {
                    contact.secret.verify_direct_proof(
                        bundle.bundle_id().as_bytes(),
                        &p.our_id,
                        &pr,
                    )
                })
            });
            return self.received(if direct { cla } else { ClaId::Courier }, bytes, rng);
        }
        let to_contact = self
            .contacts
            .values()
            .any(|c| self.contact_tags(c).contains(bundle.dst_tag().as_bytes()));
        let class = if to_contact {
            CourierClass::ToContact
        } else {
            CourierClass::Other
        };
        let now = self.now_unix_ms;
        let limit = self.courier_limit;
        let Some(p) = self.peers.get_mut(&peer) else {
            return Vec::new();
        };
        match self.courier.admit(
            &bundle,
            bytes,
            class,
            limit,
            &mut p.courier_usage,
            p.batch,
            now,
        ) {
            Ok(writes) => alloc::vec![Output::Persist(StorageOp::Store(writes))],
            Err(CourierRefusal::EncounterQuota) => {
                alloc::vec![Output::Rejected(RejectReason::Quota)]
            }
            // Not stored; nothing to tell the peer (it cannot learn whether we carry it).
            Err(_) => Vec::new(),
        }
    }

    /// Installs what the database holds: tombstones (merged with any still in an old snapshot,
    /// which move to the database now) and the courier index, dropping what has expired or been
    /// purged meanwhile.
    fn store_loaded(&mut self, rows: Vec<CourierMeta>, stored: Vec<TombstoneRow>) -> Vec<Output> {
        let mut writes = Vec::new();
        if self.migrate_tombstones {
            self.migrate_tombstones = false;
            writes.extend(self.tombstones.iter().map(|(c, t)| {
                StoreWrite::TombstonePut(TombstoneRow {
                    commitment: *c.as_bytes(),
                    token: *t.token.as_bytes(),
                    expires_unix_ms: t.expires_unix_ms,
                })
            }));
        }
        for t in stored {
            self.tombstones.insert(
                PurgeCommitment::from_bytes(t.commitment),
                Tombstone {
                    token: PurgeToken::from_bytes(t.token),
                    expires_unix_ms: t.expires_unix_ms,
                },
            );
        }
        self.courier = CourierStore::from_loaded(rows);
        let purged: Vec<PurgeCommitment> = self.tombstones.keys().copied().collect();
        for c in purged {
            writes.extend(self.courier.purge(&c));
        }
        writes.extend(self.courier.expire(self.now_unix_ms));
        writes.extend(self.courier.shrink_to(self.courier_limit, self.now_unix_ms));
        if writes.is_empty() {
            Vec::new()
        } else {
            alloc::vec![Output::Persist(StorageOp::Store(writes))]
        }
    }

    /// Changes the courier storage limit, evicting what no longer fits.
    fn set_courier_limit(&mut self, bytes: u64) -> Vec<Output> {
        self.courier_limit = bytes.min(MAX_COURIER_BYTES);
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        if let Some(w) = self.courier.shrink_to(self.courier_limit, self.now_unix_ms) {
            out.push(Output::Persist(StorageOp::Store(alloc::vec![w])));
        }
        out
    }

    /// What we carry for others, metadata only.
    #[must_use]
    pub fn courier_entries(&self) -> Vec<CourierMeta> {
        self.courier.entries().cloned().collect()
    }

    /// Courier storage: `(used bytes, limit bytes, bundles carried)`.
    #[must_use]
    pub fn courier_status(&self) -> (u64, u64, usize) {
        (
            self.courier.used_bytes(),
            self.courier_limit,
            self.courier.len(),
        )
    }

    /// Receives a bundle: only bundles that decrypt and verify as coming from an
    /// approved contact become messages. Acks they carry are applied; acks we owe are recorded
    /// after the message is stored.
    fn received(&mut self, cla: ClaId, bytes: &[u8], rng: &mut impl CryptoRng) -> Vec<Output> {
        self.received_as(cla, cla.grade(), bytes, rng)
    }

    /// Before decryption: a retransmitted copy of a bundle we delivered is refused but
    /// acknowledged again so the sender stops retrying; a contact request goes
    /// its own way. `None`: open it.
    fn refuse_purged_or_intro(&mut self, cla: ClaId, bytes: &[u8]) -> Option<Vec<Output>> {
        let bundle = Bundle::decode(bytes).ok()?;
        let commitment = PurgeCommitment::from_bytes(*bundle.purge_commitment());
        if self.tombstones.contains_key(&commitment) {
            if let Some((contact, message, device)) = self
                .delivered
                .get(&commitment)
                .map(|d| (d.contact, d.message, d.device))
            {
                self.owe_ack_from(contact, message, device);
                return Some(alloc::vec![
                    Output::Persist(StorageOp::SaveSnapshot),
                    Output::Rejected(RejectReason::Purged),
                ]);
            }
            return Some(alloc::vec![Output::Rejected(RejectReason::Purged)]);
        }
        (bundle.kind() == BundleKind::Intro).then(|| self.received_intro(cla, &bundle))
    }

    /// A sync item from one of our devices: applied only on another device, and only when our
    /// main device sent it.
    fn sync_from(
        &mut self,
        from_device: Option<[u8; 8]>,
        data: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if from_device.is_none() && !self.is_main_device() {
            self.received_own_sync(data, rng)
        } else {
            Vec::new()
        }
    }

    /// Whether `content` may come from `contact`: no 1:1 message from someone who is only a
    /// fellow group member; from our own devices (the self contact) only
    /// sync items, acks and key updates, and sync items from nobody else.
    fn content_allowed(
        &self,
        contact: ContactId,
        content: &OpenedContent,
    ) -> Result<(), RejectReason> {
        if matches!(content, OpenedContent::Text { .. })
            && self.contacts.get(&contact).is_some_and(|c| c.group_only)
        {
            return Err(RejectReason::NotAContact);
        }
        let neutral = matches!(
            content,
            OpenedContent::Ack | OpenedContent::KeyUpdate { .. }
        );
        if !neutral && self.is_self(contact) != matches!(content, OpenedContent::OwnSync { .. }) {
            return Err(RejectReason::Unsupported);
        }
        Ok(())
    }

    /// Takes a key update: a newer device list first (it may name the device whose key this
    /// is), then the key for the device that sent it. `true` if this device was retired.
    fn learn_announcement(
        &mut self,
        contact: ContactId,
        from_device: Option<[u8; 8]>,
        mut k: KeyUpdate,
    ) -> bool {
        let retired = k
            .device_list
            .take()
            .is_some_and(|list| self.learn_device_list(contact, list));
        match from_device {
            Some(d) => self.learn_device_key(contact, d, &k),
            None => self.learn_key(contact, &k),
        }
        retired
    }

    /// As [`Self::received`], recording `grade` as the arrival's grade: lower than the
    /// channel's own when the bundle went through more hands (another operator's hub).
    fn received_as(
        &mut self,
        cla: ClaId,
        grade: PrivacyGrade,
        bytes: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if let Some(out) = self.refuse_purged_or_intro(cla, bytes) {
            return out;
        }
        let opened = match self.open_bundle(bytes) {
            Ok(o) => o,
            Err(reason) => return alloc::vec![Output::Rejected(reason)],
        };
        let Opened {
            contact,
            id,
            commitment,
            token,
            acks,
            content,
            key_update,
            xwing_serial,
            from_device,
        } = opened;
        self.confirm_xwing(contact, from_device, xwing_serial);
        if let Err(r) = self.content_allowed(contact, &content) {
            return alloc::vec![Output::Rejected(r)];
        }
        let retired = key_update.is_some_and(|k| self.learn_announcement(contact, from_device, k));
        // Any authentic bundle from someone we asked means they approved.
        let confirmed = self
            .contacts
            .get_mut(&contact)
            .is_some_and(|c| core::mem::replace(&mut c.requested, false));
        // Delivered (or duplicate): either way this bundle is done. Tombstone it so later copies
        // are refused before decryption.
        let writes = self.tombstone(commitment, token);
        let mut out = alloc::vec![
            Output::Persist(StorageOp::Store(writes)),
            Output::Persist(StorageOp::SaveSnapshot),
        ];
        out.extend(confirmed.then(|| Output::approved(contact)));
        out.extend(self.apply_acks(contact, from_device, &acks));
        if retired {
            out.push(Output::DeviceRetired);
            return out;
        }
        if let OpenedContent::OwnSync { data } = content {
            self.remember_delivered(commitment, contact, id, from_device);
            self.owe_ack_from(contact, id, from_device);
            out.extend(self.sync_from(from_device, &data, rng));
            return out;
        }
        if let OpenedContent::KeyUpdate { ack_requested } = content {
            self.remember_delivered(commitment, contact, id, from_device);
            if ack_requested {
                self.owe_ack_from(contact, id, from_device);
            }
            return out;
        }
        if let OpenedContent::Group {
            created_unix_ms,
            content,
        } = content
        {
            // Acknowledged even when ignored, so an authentic sender stops retrying.
            self.remember_delivered(commitment, contact, id, from_device);
            self.owe_ack_from(contact, id, from_device);
            let via = ChannelUse {
                cla,
                grade,
                at_unix_ms: self.now_unix_ms,
            };
            out.extend(self.received_group(contact, id, created_unix_ms, content, via, rng));
            return out;
        }
        if let OpenedContent::Text {
            created_unix_ms,
            text,
            ack_requested,
        } = content
        {
            let channel = ChannelUse {
                cla,
                grade,
                at_unix_ms: self.now_unix_ms,
            };
            let delivered = Delivered {
                contact,
                id,
                commitment,
                created_unix_ms,
                ack_requested,
                from_device,
            };
            out.extend(self.store_text(delivered, text, channel));
        }
        out
    }

    /// Stores a delivered 1:1 text (already tombstoned) and owes its ack.
    fn store_text(&mut self, d: Delivered, text: String, channel: ChannelUse) -> Vec<Output> {
        let Delivered {
            contact,
            id,
            commitment,
            created_unix_ms,
            ack_requested,
            from_device,
        } = d;
        self.remember_delivered(commitment, contact, id, from_device);
        if ack_requested {
            self.owe_ack_from(contact, id, from_device);
        }
        if self.seen.contains(&id) || self.messages.iter().any(|m| m.id == id) {
            return alloc::vec![Output::Rejected(RejectReason::Duplicate)];
        }
        if self.seen.len() >= SEEN_CAPACITY {
            self.seen.pop_front();
        }
        self.seen.push_back(id);
        self.messages.push(Message {
            id,
            contact,
            direction: MsgDirection::Incoming,
            kind: MessageKind::Text,
            text,
            created_unix_ms,
            status: DeliveryStatus::Received,
            channels: alloc::vec![channel],
            group: None,
            fanout: None,
        });
        // `Persist` comes first in `out`: the runtime stores the message before any ack for it can
        // be transmitted, since acks only leave on a later tick or with a later message.
        alloc::vec![Output::Deliver {
            contact,
            message: id,
        }]
    }

    /// As [`Self::owe_ack`], for a message from `device` (one of the contact's other devices):
    /// the ack goes to that device.
    fn owe_ack_from(&mut self, contact: ContactId, message: MsgId, device: Option<[u8; 8]>) {
        if let Some(d) = device {
            self.ack_from.insert(message, (contact, d));
        }
        self.owe_ack(contact, message);
    }

    /// Records that we owe `contact` an ack for `message`, within [`MAX_PENDING_ACKS`].
    fn owe_ack(&mut self, contact: ContactId, message: MsgId) {
        let owed = self.pending_acks.entry(contact).or_default();
        if !owed.contains(&message) {
            if owed.len() >= MAX_PENDING_ACKS {
                let dropped = owed.remove(0);
                self.ack_from.remove(&dropped);
            }
            owed.push(message);
        }
        let now = self.now_unix_ms;
        self.ack_due.entry(contact).or_insert(now);
    }

    /// Remembers a delivered bundle for re-acking, within the tombstone quota and lifetime.
    fn remember_delivered(
        &mut self,
        commitment: PurgeCommitment,
        contact: ContactId,
        id: MsgId,
        device: Option<[u8; 8]>,
    ) {
        let now = self.now_unix_ms;
        if self.delivered.len() >= TOMBSTONE_CAPACITY
            && !self.delivered.contains_key(&commitment)
            && let Some(oldest) = self
                .delivered
                .iter()
                .min_by_key(|(_, d)| d.expires_unix_ms)
                .map(|(c, _)| *c)
        {
            self.delivered.remove(&oldest);
        }
        self.delivered.insert(
            commitment,
            DeliveredEntry {
                contact,
                message: id,
                expires_unix_ms: now.saturating_add(TOMBSTONE_LIFETIME_MS),
                device,
            },
        );
    }

    /// Applies acks from `contact` (from its device `from`, `None` for the main one): only for
    /// messages we sent to that contact. A per-device copy's ack marks the
    /// message it copies delivered; the other copies keep their retries.
    fn apply_acks(
        &mut self,
        contact: ContactId,
        from: Option<[u8; 8]>,
        ids: &[MsgId],
    ) -> Vec<Output> {
        let mut out = Vec::new();
        for id in ids {
            let ours = self.messages.iter().any(|m| {
                m.id == *id && m.contact == contact && m.direction == MsgDirection::Outgoing
            }) || self.outbox.get(id).is_some_and(|i| i.contact == contact);
            if !ours {
                continue;
            }
            let item = self.outbox.get(id);
            // Only the device a copy was sealed to can ack it.
            if item.is_some_and(|i| i.device != from) {
                continue;
            }
            let (key_serial, list_serial, copy_of) = item.map_or((None, None, None), |i| {
                (i.key_serial, i.list_serial, i.copy_of)
            });
            // An ack for a bundle that carried our device list: that device knows it now.
            if let Some(serial) = list_serial
                && let Some(c) = self.contacts.get_mut(&contact)
            {
                match from.and_then(|d| c.devices.iter_mut().find(|x| x.id == d)) {
                    Some(d) => d.told_list = d.told_list.max(serial),
                    None => c.told_list = c.told_list.max(serial),
                }
            }
            // An ack for a bundle that announced our key: that device knows that key now.
            if let Some(serial) = key_serial {
                self.confirm_key(contact, from, serial);
            }
            self.outbox.remove(id);
            self.sends.remove(id);
            // A retired device confirmed the list that drops it: forget it.
            if let Some(d) = from
                && let Some(c) = self.contacts.get_mut(&contact)
            {
                c.devices.retain(|x| x.id != d || !x.retired);
            }
            let shown = copy_of.unwrap_or(*id);
            if let Some(status) = self.set_status(shown, DeliveryStatus::Delivered) {
                out.push(Output::StatusChanged {
                    message: shown,
                    status,
                });
            }
        }
        out
    }

    /// Opens a payload with the key its opaque id names: one of our mailbox keys (recomputed per
    /// held key), or the identity fallback, which only key updates may use.
    fn decrypt(
        &self,
        contact: &Contact,
        payload: &Payload,
        bundle: &Bundle,
    ) -> Result<(Zeroizing<Vec<u8>>, Option<u32>), RejectReason> {
        if payload.suite_id == SUITE_XWING {
            let (held, key_id) = self
                .mailboxes
                .iter()
                .rev()
                .map(|h| (h, h.key.xwing_public().key_id(&contact.secret)))
                .find(|(_, id)| payload.mailbox_key_id == *id.as_bytes())
                .ok_or(RejectReason::Undecryptable)?;
            return held
                .key
                .open_xwing(&key_id, &payload.enc, &payload.ciphertext, &bundle.aad())
                .map(|pt| (pt, Some(held.serial)))
                .map_err(|_| RejectReason::Undecryptable);
        }
        let fallback = identity_key_id(&contact.secret);
        if payload.mailbox_key_id == *fallback.as_bytes() {
            if bundle.kind() != BundleKind::KeyUpdate {
                return Err(RejectReason::Undecryptable);
            }
            // Only the main device holds the identity key.
            let Some(identity) = self.identity.key() else {
                return Err(RejectReason::Undecryptable);
            };
            return open_with_identity(
                identity,
                &fallback,
                &payload.enc,
                &payload.ciphertext,
                &bundle.aad(),
            )
            .map(|pt| (pt, None))
            .map_err(|_| RejectReason::Undecryptable);
        }
        let (held, key_id) = self
            .mailboxes
            .iter()
            .rev()
            .map(|h| (h, h.key.public().key_id(&contact.secret)))
            .find(|(_, id)| payload.mailbox_key_id == *id.as_bytes())
            .ok_or(RejectReason::Undecryptable)?;
        held.key
            .open(&key_id, &payload.enc, &payload.ciphertext, &bundle.aad())
            .map(|pt| (pt, None))
            .map_err(|_| RejectReason::Undecryptable)
    }

    fn open_bundle(&self, bytes: &[u8]) -> Result<Opened, RejectReason> {
        let bundle = Bundle::decode(bytes).map_err(|e| match e {
            mem_proto::ProtoError::Invalid(mem_proto::InvalidBundle::Expired) => {
                RejectReason::Expired
            }
            mem_proto::ProtoError::Invalid(mem_proto::InvalidBundle::UnsupportedVersion(_)) => {
                RejectReason::Unsupported
            }
            _ => RejectReason::Malformed,
        })?;
        let commitment = PurgeCommitment::from_bytes(*bundle.purge_commitment());
        if self.tombstones.contains_key(&commitment) {
            return Err(RejectReason::Purged);
        }
        if !matches!(
            bundle.kind(),
            BundleKind::Message | BundleKind::Ack | BundleKind::KeyUpdate
        ) {
            return Err(RejectReason::Unsupported);
        }
        let (contact_id, contact) = self
            .contact_for_tag(&bundle.dst_tag())
            .ok_or(RejectReason::NotForUs)?;
        let payload = Payload::decode(bundle.payload()).map_err(|_| RejectReason::Malformed)?;
        if payload.suite_id != SUITE_ID && payload.suite_id != SUITE_XWING {
            return Err(RejectReason::Unsupported);
        }
        let (padded, xwing_serial) = self.decrypt(contact, &payload, &bundle)?;
        let inner = unpad(&padded)
            .and_then(Inner::decode)
            .map_err(|_| RejectReason::Malformed)?;
        if inner.fields.sender != *contact.identity.as_bytes() {
            return Err(RejectReason::BadSignature);
        }
        // Signed by the identity (the main device), or by another device of the contact with
        // its certificate, listed in the newest list we hold or the one this bundle brings.
        // A device announces only its own key.
        let offered = inner
            .fields
            .key_update
            .as_ref()
            .and_then(|k| k.device_list.as_ref());
        let (signer, from_device) = match &inner.fields.device_cert {
            None => (contact.identity, None),
            Some(cert) => {
                let (key, id) = Self::signing_device(contact, cert, offered)
                    .map_err(|()| RejectReason::BadSignature)?;
                let main = Self::newest_list(contact, offered)
                    .and_then(|l| l.certs.first())
                    .and_then(devices::cert_device_id);
                (key, (main != Some(id)).then_some(id))
            }
        };
        if inner
            .fields
            .key_update
            .as_ref()
            .and_then(|k| k.device_id)
            .is_some_and(|d| from_device.is_some_and(|f| f != d))
        {
            return Err(RejectReason::Malformed);
        }
        signer
            .verify(
                SIGNATURE_DOMAIN,
                &[
                    self.identity.public().as_bytes(),
                    &inner.fields.signed_bytes(),
                ],
                &inner.signature,
            )
            .map_err(|_| RejectReason::BadSignature)?;
        // The commitment is in the AAD, so a courier cannot have changed it; a mismatch means the
        // sender built a malformed bundle: deliver nothing.
        let token = PurgeToken::from_bytes(inner.fields.purge_token);
        if commit(&token) != commitment {
            return Err(RejectReason::Malformed);
        }
        if inner.fields.conversation_id != contact.secret.conversation_id() {
            return Err(RejectReason::Malformed);
        }
        let id = MsgId(inner.fields.message_id);
        let mut acks: Vec<MsgId> = inner.fields.acks.iter().map(|m| MsgId(*m)).collect();
        let content = opened_content(bundle.kind(), &inner.fields, id, &mut acks)?;
        Ok(Opened {
            contact: contact_id,
            id,
            commitment,
            token,
            acks,
            content,
            key_update: inner.fields.key_update,
            xwing_serial,
            from_device,
        })
    }

    /// Tombstones `commitment` within the quota and drops every carried copy with it.
    /// Expired entries go first, then the soonest-expiring; early eviction can
    /// only re-admit a copy. Returns the database writes.
    fn tombstone(&mut self, commitment: PurgeCommitment, token: PurgeToken) -> Vec<StoreWrite> {
        self.queue_hub_token(*token.as_bytes());
        let mut writes: Vec<StoreWrite> = self.courier.purge(&commitment).into_iter().collect();
        let now = self.now_unix_ms;
        let mut gone: Vec<[u8; 16]> = self
            .tombstones
            .iter()
            .filter(|(_, t)| t.expires_unix_ms <= now)
            .map(|(c, _)| *c.as_bytes())
            .collect();
        self.tombstones.retain(|_, t| t.expires_unix_ms > now);
        if self.tombstones.len() >= TOMBSTONE_CAPACITY
            && !self.tombstones.contains_key(&commitment)
            && let Some(oldest) = self
                .tombstones
                .iter()
                .min_by_key(|(_, t)| t.expires_unix_ms)
                .map(|(c, _)| *c)
        {
            self.tombstones.remove(&oldest);
            gone.push(*oldest.as_bytes());
        }
        let expires_unix_ms = now.saturating_add(TOMBSTONE_LIFETIME_MS);
        writes.push(StoreWrite::TombstonePut(TombstoneRow {
            commitment: *commitment.as_bytes(),
            token: *token.as_bytes(),
            expires_unix_ms,
        }));
        self.tombstones.insert(
            commitment,
            Tombstone {
                token,
                expires_unix_ms,
            },
        );
        if !gone.is_empty() {
            writes.push(StoreWrite::TombstoneDelete(gone));
        }
        writes
    }

    /// Whether a commitment is tombstoned (for tests and future courier admission paths).
    #[must_use]
    pub fn is_purged(&self, commitment: &[u8; 16]) -> bool {
        self.tombstones
            .contains_key(&PurgeCommitment::from_bytes(*commitment))
    }

    /// Our receiving tag for `contact` in day `epoch`: the contact tag on the main
    /// device, this device's own tag on another one.
    pub(crate) fn my_tag(&self, contact: &Contact, epoch: u32) -> [u8; 16] {
        let direction = Direction::towards(&self.identity.public(), &contact.identity);
        match self.receiving_device() {
            Some(id) => contact.secret.device_tag(direction, epoch, &id),
            None => contact.secret.tag(direction, epoch),
        }
    }

    /// This device's id when it is not the main device (it then receives on device tags).
    pub(crate) fn receiving_device(&self) -> Option<[u8; 8]> {
        if self.identity.key().is_some() {
            None
        } else {
            self.own.id()
        }
    }

    /// Finds the contact whose receiving tag matches.
    ///
    /// The window reaches back over the longest bundle lifetime, [`RECEIVE_TAG_DAYS`]: a bundle
    /// keeps the tag of the day it was sealed while it waits in an outbox or a courier for up to
    /// 30 days. Checking older tags is local work only; nothing extra is
    /// advertised.
    fn contact_for_tag(&self, tag: &RendezvousTag) -> Option<(ContactId, &Contact)> {
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        let oldest = epoch.saturating_sub(RECEIVE_TAG_DAYS);
        self.contacts.iter().find_map(|(id, c)| {
            (oldest..=epoch.saturating_add(1))
                .rev()
                .any(|e| self.my_tag(c, e) == *tag.as_bytes())
                .then_some((*id, c))
        })
    }

    // ---------------------------------------------------------------- pairing

    /// Our signed pairing card.
    ///
    /// # Errors
    ///
    /// Only if the stored display name is invalid, which [`Input::SetDisplayName`] prevents.
    pub fn own_card(&self) -> Result<Vec<u8>, RejectReason> {
        // Only the main device can sign a card.
        let identity = self.identity.key().ok_or(RejectReason::MainDeviceOnly)?;
        let fields = CardFields {
            identity_pk: *self.identity.public().as_bytes(),
            mailbox_pk: *self.current_mailbox().key.public().as_bytes(),
            pairing_nonce: *self.pairing_nonce.as_bytes(),
            display_name: self.display_name.clone(),
        };
        let signed = fields
            .signed_bytes()
            .map_err(|_| RejectReason::InvalidName)?;
        let signature = identity.sign(CARD_SIGNATURE_DOMAIN, &[&signed]);
        PairingCard { fields, signature }
            .encode()
            .map_err(|_| RejectReason::InvalidName)
    }

    fn import_card(&mut self, cla: ClaId, bytes: &[u8], rng: &mut impl CryptoRng) -> Vec<Output> {
        match verify_card(bytes) {
            Ok((identity, mailbox, nonce, name)) => {
                if identity == self.identity.public() {
                    return alloc::vec![Output::Rejected(RejectReason::OwnCard)];
                }
                // Pairing needs the identity key: the main device pairs.
                let Some(key) = self.identity.key() else {
                    return alloc::vec![Output::Rejected(RejectReason::MainDeviceOnly)];
                };
                let Ok(secret) = ContactSecret::derive(key, &self.pairing_nonce, &identity, &nonce)
                else {
                    return alloc::vec![Output::Rejected(RejectReason::BadSignature)];
                };
                let trust = match cla {
                    // The code was on a screen or in the air of the same room.
                    ClaId::Qr | ClaId::Audio => TrustLevel::MetInPerson,
                    // Being on the same network is not meeting in person.
                    ClaId::Iroh
                    | ClaId::Ble
                    | ClaId::WifiDirect
                    | ClaId::Hub
                    | ClaId::Share
                    | ClaId::Courier
                    | ClaId::Email
                    | ClaId::Plain => TrustLevel::Remote,
                };
                let id = ContactId(*identity.as_bytes());
                // The same pairing again (same identities and nonces, so the same secret): keep
                // the keys learned since (the card's key may be retired by now) and
                // take only what the card adds.
                if let Some(c) = self.contacts.get_mut(&id)
                    && c.secret.conversation_id() == secret.conversation_id()
                {
                    let unchanged = trust <= c.trust && !c.requested && !c.group_only;
                    c.trust = c.trust.max(trust);
                    c.requested = false;
                    c.group_only = false;
                    c.display_name = name;
                    return alloc::vec![
                        Output::Persist(StorageOp::SaveSnapshot),
                        Output::ContactPaired {
                            contact: id,
                            updated: true,
                            unchanged,
                        },
                    ];
                }
                let updated = self.contacts.contains_key(&id);
                // A re-pairing keeps the address the user typed.
                let kept_email = self.contacts.get(&id).and_then(|c| c.email.clone());
                let (floor, added, trust) = self
                    .contacts
                    .get(&id)
                    .map_or((self.default_floor, self.now_unix_ms, trust), |c| {
                        (c.floor, c.added_unix_ms, c.trust.max(trust))
                    });
                self.contacts.insert(
                    id,
                    Contact {
                        identity,
                        mailbox,
                        secret,
                        display_name: name,
                        trust,
                        floor,
                        added_unix_ms: added,
                        // A card's key has no serial: any later key update replaces it.
                        mailbox_serial: 0,
                        mailbox_learned_ms: self.now_unix_ms,
                        // Pairing is mutual: they took our current key from our card.
                        told_serial: self.current_mailbox().serial,
                        // Exchanging cards answers any request between us.
                        requested: false,
                        group_only: false,
                        email: kept_email,
                        xwing: None,
                        told_xwing: 0,
                        devices: Vec::new(),
                        device_list: None,
                        told_list: 0,
                    },
                );
                let mut out = alloc::vec![
                    Output::Persist(StorageOp::SaveSnapshot),
                    Output::ContactPaired {
                        contact: id,
                        updated,
                        unchanged: false,
                    },
                ];
                // A new contact means a new receiving tag: tell connected peers.
                out.extend(self.advertise_all(rng));
                out
            }
            Err(reason) => alloc::vec![Output::Rejected(reason)],
        }
    }

    fn set_display_name(&mut self, name: Option<String>) -> Vec<Output> {
        let name = name
            .map(|n| String::from(n.trim()))
            .filter(|n| !n.is_empty());
        if name
            .as_ref()
            .is_some_and(|n| n.len() > MAX_DISPLAY_NAME_LEN)
        {
            return alloc::vec![Output::Rejected(RejectReason::InvalidName)];
        }
        self.display_name = name;
        alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
    }

    // ---------------------------------------------------------------- queries

    /// What a pairing card says, checked but not imported: for asking the user first when the
    /// card did not come from their own scan or paste (another app shared it).
    ///
    /// # Errors
    ///
    /// [`RejectReason`] for a malformed or badly signed card, or our own.
    pub fn card_preview(&self, bytes: &[u8]) -> Result<CardPreview, RejectReason> {
        let (identity, _, _, display_name) = verify_card(bytes)?;
        if identity == self.identity.public() {
            return Err(RejectReason::OwnCard);
        }
        Ok(CardPreview {
            fingerprint: identity.fingerprint().to_string_grouped(),
            display_name,
            known: self
                .contacts
                .get(&ContactId(*identity.as_bytes()))
                .is_some_and(|c| !c.group_only),
        })
    }

    /// Our identity's fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        self.identity.public().fingerprint().to_string_grouped()
    }

    /// Our display name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// All contacts, sorted by name then fingerprint.
    #[must_use]
    pub fn contacts(&self) -> Vec<ContactView> {
        // Group-only members are not contacts.
        let me = self.self_id();
        let mut out: Vec<_> = self
            .contacts
            .iter()
            .filter(|(id, c)| !c.group_only && **id != me)
            .map(|(id, c)| view(*id, c))
            .collect();
        out.sort_by(|a, b| {
            (a.display_name.as_deref(), &a.fingerprint)
                .cmp(&(b.display_name.as_deref(), &b.fingerprint))
        });
        out
    }

    /// One contact.
    #[must_use]
    pub fn contact(&self, id: ContactId) -> Option<ContactView> {
        self.contacts
            .get(&id)
            .filter(|c| !c.group_only && !self.is_self(id))
            .map(|c| view(id, c))
    }

    /// Messages with one contact, oldest first.
    #[must_use]
    pub fn messages(&self, contact: ContactId) -> Vec<MessageView> {
        self.messages
            .iter()
            .filter(|m| m.contact == contact && m.group.is_none())
            .map(MessageView::from)
            .collect()
    }

    /// The privacy log, newest first.
    #[must_use]
    pub fn privacy_log(&self) -> Vec<PrivacyLogEntry> {
        self.privacy_log.iter().rev().copied().collect()
    }

    /// The attestation log, newest first.
    #[must_use]
    pub fn attest_log(&self) -> Vec<AttestLogEntry> {
        self.attest_log.iter().rev().cloned().collect()
    }

    fn record_attestation(&mut self, mut entry: AttestLogEntry) -> Vec<Output> {
        // The fields come from a checked request; bound them anyway.
        if entry.rp_origin.len() > mem_proto::attest::MAX_ORIGIN_LEN
            || entry.subject_label.len() > mem_proto::attest::MAX_LABEL_LEN
        {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        entry.at_unix_ms = self.now_unix_ms;
        if self.attest_log.len() >= MAX_ATTEST_LOG {
            self.attest_log.remove(0);
        }
        self.attest_log.push(entry);
        alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
    }

    /// Standing consent rules.
    #[must_use]
    pub fn rules(&self) -> Vec<ConsentRule> {
        self.rules.clone()
    }

    /// Whether a channel is turned on.
    #[must_use]
    pub fn channel_enabled(&self, cla: ClaId) -> bool {
        !self.disabled.contains(&cla)
    }

    /// Contacts reachable right now through a connected peer, with the channel.
    #[must_use]
    pub fn reachable_contacts(&self) -> Vec<(ContactId, ClaId)> {
        self.contacts
            .keys()
            .filter_map(|id| {
                ClaId::ALL
                    .into_iter()
                    .filter(|c| c.caps().needs_peer)
                    .find(|c| self.peer_for(*id, *c).is_some())
                    .map(|c| (*id, c))
            })
            .collect()
    }

    /// Lifetime of new messages in seconds: how long an unconfirmed message is retried.
    #[must_use]
    pub const fn message_lifetime_s(&self) -> u32 {
        self.message_lifetime_s
    }

    /// Floor given to contacts added from now on.
    #[must_use]
    pub const fn default_floor(&self) -> PrivacyGrade {
        self.default_floor
    }

    /// Number of connected peers that completed `hello`.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.values().filter(|p| p.hello).count()
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[allow(
    clippy::type_complexity,
    reason = "private helper returning parsed card parts"
)]
fn verify_card(
    bytes: &[u8],
) -> Result<(IdentityPublic, MailboxPublic, PairingNonce, Option<String>), RejectReason> {
    let card = PairingCard::decode(bytes).map_err(|_| RejectReason::Malformed)?;
    let identity = IdentityPublic::from_bytes(&card.fields.identity_pk)
        .map_err(|_| RejectReason::Malformed)?;
    let signed = card
        .fields
        .signed_bytes()
        .map_err(|_| RejectReason::Malformed)?;
    identity
        .verify(CARD_SIGNATURE_DOMAIN, &[&signed], &card.signature)
        .map_err(|_| RejectReason::BadSignature)?;
    let mailbox =
        MailboxPublic::from_bytes(&card.fields.mailbox_pk).map_err(|_| RejectReason::Malformed)?;
    Ok((
        identity,
        mailbox,
        PairingNonce::from_bytes(card.fields.pairing_nonce),
        card.fields.display_name,
    ))
}

/// What an authenticated inner payload holds, by envelope kind and content type. A standalone
/// ack's ids replace `acks`.
fn opened_content(
    kind: BundleKind,
    fields: &InnerFields,
    id: MsgId,
    acks: &mut Vec<MsgId>,
) -> Result<OpenedContent, RejectReason> {
    let content = match (kind, fields.content_type) {
        (BundleKind::Message, CONTENT_TYPE_TEXT) => OpenedContent::Text {
            created_unix_ms: uuid_v7_ms(&id.0),
            text: decode_text(&fields.content).map_err(|_| RejectReason::Malformed)?,
            // Messages from builds without delivery acks carry no flag; ack those too.
            ack_requested: fields.ack_requested != Some(false),
        },
        // A standalone ack: never asks for an ack and carries no piggyback of its own.
        (BundleKind::Ack, CONTENT_TYPE_ACK)
            if fields.ack_requested != Some(true) && acks.is_empty() =>
        {
            *acks = decode_ack_ids(&fields.content)
                .map_err(|_| RejectReason::Malformed)?
                .into_iter()
                .map(MsgId)
                .collect();
            OpenedContent::Ack
        }
        // A standalone key update: empty content, the key in field 6, asks for an ack.
        (BundleKind::KeyUpdate, CONTENT_TYPE_KEY_UPDATE)
            if fields.key_update.is_some() && fields.content.is_empty() =>
        {
            OpenedContent::KeyUpdate {
                ack_requested: fields.ack_requested == Some(true),
            }
        }
        // Between our own devices; the sender is checked by the caller.
        (BundleKind::Message, mem_proto::device::CONTENT_TYPE_OWN_SYNC)
            if fields.ack_requested == Some(true) =>
        {
            OpenedContent::OwnSync {
                data: fields.content.clone(),
            }
        }
        (
            BundleKind::Message,
            t @ (CONTENT_TYPE_GROUP_TEXT | CONTENT_TYPE_GROUP_STATE | CONTENT_TYPE_GROUP_REPLY),
        ) if fields.ack_requested == Some(true) => OpenedContent::Group {
            created_unix_ms: uuid_v7_ms(&id.0),
            content: GroupContent::decode(t, &fields.content)?,
        },
        (
            BundleKind::Message | BundleKind::Ack | BundleKind::KeyUpdate,
            CONTENT_TYPE_TEXT
            | CONTENT_TYPE_ACK
            | CONTENT_TYPE_KEY_UPDATE
            | CONTENT_TYPE_GROUP_TEXT
            | CONTENT_TYPE_GROUP_STATE
            | CONTENT_TYPE_GROUP_REPLY,
        ) => {
            return Err(RejectReason::Malformed);
        }
        _ => return Err(RejectReason::Unsupported),
    };
    Ok(content)
}

fn view(id: ContactId, c: &Contact) -> ContactView {
    ContactView {
        id,
        display_name: c.display_name.clone(),
        fingerprint: c.identity.fingerprint().to_string_grouped(),
        trust: c.trust,
        floor: c.floor,
        sas: c.secret.sas(),
        added_unix_ms: c.added_unix_ms,
        requested: c.requested,
        email: c.email.clone(),
    }
}

/// UUIDv7 (RFC 9562) from injected time and randomness.
fn uuid_v7(unix_ms: u64, rng: &mut impl CryptoRng) -> [u8; 16] {
    let mut b = [0u8; 16];
    rng.fill_bytes(&mut b);
    let ms = unix_ms.to_be_bytes();
    b[..6].copy_from_slice(&ms[2..]);
    b[6] = (b[6] & 0x0f) | 0x70;
    b[8] = (b[8] & 0x3f) | 0x80;
    b
}

fn uuid_v7_ms(id: &[u8; 16]) -> u64 {
    let mut ms = [0u8; 8];
    ms[2..].copy_from_slice(&id[..6]);
    u64::from_be_bytes(ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    const NOW: u64 = 1_792_022_400_000;

    fn agent(seed: u8) -> (Agent, ChaCha20Rng) {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let mut a = Agent::new(NOW, &mut rng);
        let _ = a.handle(Input::Tick { wall_unix_ms: NOW }, &mut rng);
        (a, rng)
    }

    fn paired() -> (Agent, ChaCha20Rng, Agent, ChaCha20Rng, ContactId) {
        let (mut a, mut ra) = agent(1);
        let (mut b, mut rb) = agent(2);
        let card_b = b.own_card().unwrap();
        let card_a = a.own_card().unwrap();
        let _ = a.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: card_b,
            },
            &mut ra,
        );
        let _ = b.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: card_a,
            },
            &mut rb,
        );
        let b_id = a.contacts()[0].id;
        (a, ra, b, rb, b_id)
    }

    /// A token that does not match the envelope commitment is a malformed sender
    /// bundle and is never delivered.
    #[test]
    fn mismatched_token_and_commitment_is_never_delivered() {
        let (a, mut ra, mut b, mut rb, b_id) = paired();
        let contact = a.contacts.get(&b_id).unwrap();
        let token = PurgeToken::from_bytes([1; 16]);
        let wrong = commit(&PurgeToken::from_bytes([2; 16]));
        let (bundle, _) = a
            .seal_message(contact, "forged", [7; 16], &token, wrong, &mut ra)
            .unwrap();
        let out = b.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes: bundle.encode(),
            },
            &mut rb,
        );
        assert_eq!(out, alloc::vec![Output::Rejected(RejectReason::Malformed)]);
        assert!(b.messages.is_empty());
        // Nothing was tombstoned for a malformed bundle.
        assert!(b.tombstones.is_empty());
    }

    /// The identity-key fallback carries key updates only. A message sealed to
    /// the identity key decrypts, but is refused (it would have no forward secrecy).
    #[test]
    fn identity_sealed_messages_are_refused() {
        let (a, mut ra, mut b, mut rb, b_id) = paired();
        let contact = a.contacts.get(&b_id).unwrap();
        let purge_key = a.purge_key.as_ref().unwrap();
        let token = purge_key.derive_token(&[8; 16]);
        let (bundle, _) = a
            .seal(
                contact,
                Sealing {
                    kind: BundleKind::Message,
                    content_type: CONTENT_TYPE_TEXT,
                    content: encode_text("no forward secrecy").unwrap(),
                    ack_requested: Some(true),
                    acks: Vec::new(),
                    lifetime_s: a.message_lifetime_s,
                    key_update: None,
                    id: None,
                    recipient: Recipient::Identity,
                },
                [8; 16],
                &token,
                commit(&token),
                &mut ra,
            )
            .unwrap();
        let out = b.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes: bundle.encode(),
            },
            &mut rb,
        );
        assert_eq!(
            out,
            alloc::vec![Output::Rejected(RejectReason::Undecryptable)]
        );
        assert!(b.messages.is_empty());
    }

    /// Only a higher serial replaces a contact's key; replays and late older
    /// announcements are ignored.
    #[test]
    fn older_or_equal_key_serials_are_ignored() {
        let (mut a, _ra, _b, _rb, b_id) = paired();
        let newer = MailboxKey::from_seed(&[5; 32]).public();
        let older = MailboxKey::from_seed(&[6; 32]).public();
        a.learn_key(
            b_id,
            &KeyUpdate {
                mailbox_pk: *newer.as_bytes(),
                serial: 3,
                xwing_pk: None,
                ..KeyUpdate::default()
            },
        );
        for serial in [2, 3] {
            a.learn_key(
                b_id,
                &KeyUpdate {
                    mailbox_pk: *older.as_bytes(),
                    serial,
                    xwing_pk: None,
                    ..KeyUpdate::default()
                },
            );
        }
        let c = a.contacts.get(&b_id).unwrap();
        assert_eq!((c.mailbox_serial, c.mailbox), (3, newer));
    }

    /// The X-Wing half of the key we already hold is taken from a contact who
    /// upgraded (same serial); a different key at the same serial is not.
    #[test]
    fn the_xwing_half_of_the_current_key_is_learned_once() {
        let (mut a, _ra, _b, _rb, b_id) = paired();
        let key = MailboxKey::from_seed(&[5; 32]);
        let other = MailboxKey::from_seed(&[6; 32]);
        a.learn_key(
            b_id,
            &KeyUpdate {
                mailbox_pk: *key.public().as_bytes(),
                serial: 3,
                xwing_pk: None,
                ..KeyUpdate::default()
            },
        );
        assert!(a.contacts.get(&b_id).unwrap().xwing.is_none());
        a.learn_key(
            b_id,
            &KeyUpdate {
                mailbox_pk: *other.public().as_bytes(),
                serial: 3,
                xwing_pk: Some(other.xwing_public().as_bytes().to_vec()),
                ..KeyUpdate::default()
            },
        );
        assert!(
            a.contacts.get(&b_id).unwrap().xwing.is_none(),
            "not for another key"
        );
        a.learn_key(
            b_id,
            &KeyUpdate {
                mailbox_pk: *key.public().as_bytes(),
                serial: 3,
                xwing_pk: Some(key.xwing_public().as_bytes().to_vec()),
                ..KeyUpdate::default()
            },
        );
        assert_eq!(
            a.contacts.get(&b_id).unwrap().xwing,
            Some(key.xwing_public())
        );
    }

    #[test]
    fn matching_token_is_delivered_and_tombstoned() {
        let (a, mut ra, mut b, mut rb, b_id) = paired();
        let contact = a.contacts.get(&b_id).unwrap();
        let token = PurgeToken::from_bytes([1; 16]);
        let (bundle, _) = a
            .seal_message(contact, "ok", [7; 16], &token, commit(&token), &mut ra)
            .unwrap();
        let out = b.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes: bundle.encode(),
            },
            &mut rb,
        );
        assert!(out.iter().any(|o| matches!(o, Output::Deliver { .. })));
        assert_eq!(
            b.tombstones.get(&commit(&token)).map(|t| t.token.clone()),
            Some(token)
        );
    }

    /// Acks count only from the contact a message was sent to. A contact who
    /// names another contact's message id, in a standalone ack or a piggyback, changes nothing.
    #[test]
    #[allow(clippy::too_many_lines, reason = "three hand-built bundles")]
    fn acks_from_the_wrong_contact_are_ignored() {
        let (mut a, mut ra, b, mut rb, b_id) = paired();
        let (mut c, mut rc) = agent(4);
        let card_c = c.own_card().unwrap();
        let card_a = a.own_card().unwrap();
        let _ = a.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: card_c,
            },
            &mut ra,
        );
        let _ = c.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: card_a,
            },
            &mut rc,
        );
        // A sends to B; it stays unconfirmed.
        let _ = a.handle(
            Input::UserSend {
                contact: b_id,
                text: "for B".into(),
            },
            &mut ra,
        );
        let msg = a.messages(b_id)[0].id;
        // C (a real contact of A) acks A's message to B, both ways.
        let a_in_c = c.contacts()[0].id;
        let contact = c.contacts.get(&a_in_c).unwrap();
        let token = PurgeToken::from_bytes([9; 16]);
        let (standalone, _) = c
            .seal(
                contact,
                Sealing {
                    kind: BundleKind::Ack,
                    content_type: CONTENT_TYPE_ACK,
                    content: encode_ack_ids(&[msg.0]).unwrap(),
                    ack_requested: None,
                    acks: Vec::new(),
                    lifetime_s: ACK_LIFETIME_S,
                    key_update: None,
                    id: None,
                    recipient: Recipient::Mailbox,
                },
                [5; 16],
                &token,
                commit(&token),
                &mut rc,
            )
            .unwrap();
        let token2 = PurgeToken::from_bytes([8; 16]);
        let (piggy, _) = c
            .seal(
                contact,
                Sealing {
                    kind: BundleKind::Message,
                    content_type: CONTENT_TYPE_TEXT,
                    content: encode_text("hi A").unwrap(),
                    ack_requested: Some(true),
                    acks: alloc::vec![msg.0],
                    lifetime_s: MESSAGE_LIFETIME_S,
                    key_update: None,
                    id: None,
                    recipient: Recipient::Mailbox,
                },
                [6; 16],
                &token2,
                commit(&token2),
                &mut rc,
            )
            .unwrap();
        for bundle in [standalone, piggy] {
            let out = a.handle(
                Input::Received {
                    cla: ClaId::Qr,
                    bytes: bundle.encode(),
                },
                &mut ra,
            );
            assert!(
                !out.iter()
                    .any(|o| matches!(o, Output::StatusChanged { .. })),
                "{out:?}"
            );
        }
        assert_eq!(a.messages(b_id)[0].status, DeliveryStatus::Queued);
        assert!(a.outbox.contains_key(&msg));
        // The genuine recipient's ack does confirm it.
        let a_in_b = b.contacts()[0].id;
        let token3 = PurgeToken::from_bytes([7; 16]);
        let (real, _) = b
            .seal(
                b.contacts.get(&a_in_b).unwrap(),
                Sealing {
                    kind: BundleKind::Ack,
                    content_type: CONTENT_TYPE_ACK,
                    content: encode_ack_ids(&[msg.0]).unwrap(),
                    ack_requested: None,
                    acks: Vec::new(),
                    lifetime_s: ACK_LIFETIME_S,
                    key_update: None,
                    id: None,
                    recipient: Recipient::Mailbox,
                },
                [4; 16],
                &token3,
                commit(&token3),
                &mut rb,
            )
            .unwrap();
        let _ = a.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes: real.encode(),
            },
            &mut ra,
        );
        assert_eq!(a.messages(b_id)[0].status, DeliveryStatus::Delivered);
        assert!(!a.outbox.contains_key(&msg));
    }

    /// A standalone ack that asks for an ack is malformed.
    #[test]
    fn malformed_standalone_acks_are_refused() {
        // `paired()` is deterministic: the second call yields an `a` with the same keys, which
        // receives what this `b` seals.
        let (_, _, b, mut rb, _) = paired();
        let (mut a, mut ra, _, _, _) = paired();
        let a_in_b = b.contacts()[0].id;
        let contact = b.contacts.get(&a_in_b).unwrap();
        let token = PurgeToken::from_bytes([3; 16]);
        let (bad, _) = b
            .seal(
                contact,
                Sealing {
                    kind: BundleKind::Ack,
                    content_type: CONTENT_TYPE_ACK,
                    content: encode_ack_ids(&[[1; 16]]).unwrap(),
                    ack_requested: Some(true),
                    acks: Vec::new(),
                    lifetime_s: ACK_LIFETIME_S,
                    key_update: None,
                    id: None,
                    recipient: Recipient::Mailbox,
                },
                [2; 16],
                &token,
                commit(&token),
                &mut rb,
            )
            .unwrap();
        let out = a.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes: bad.encode(),
            },
            &mut ra,
        );
        assert!(
            out.iter()
                .any(|o| matches!(o, Output::Rejected(RejectReason::Malformed))),
            "{out:?}"
        );
    }

    #[test]
    fn tombstone_quota_evicts_soonest_expiring() {
        let (mut b, _) = agent(3);
        let token = |i: u64| {
            let mut t = [0u8; 16];
            t[..8].copy_from_slice(&i.to_be_bytes());
            PurgeToken::from_bytes(t)
        };
        // Fill to capacity directly; entry i expires at NOW + LIFETIME + i.
        for i in 0..TOMBSTONE_CAPACITY as u64 {
            let t = token(i);
            b.tombstones.insert(
                commit(&t),
                Tombstone {
                    token: t,
                    expires_unix_ms: NOW + TOMBSTONE_LIFETIME_MS + i,
                },
            );
        }
        b.now_unix_ms = NOW + 1;
        let newcomer = token(u64::MAX);
        b.tombstone(commit(&newcomer), newcomer.clone());
        assert_eq!(b.tombstones.len(), TOMBSTONE_CAPACITY);
        assert!(b.tombstones.contains_key(&commit(&newcomer)));
        // The soonest-expiring entry (i = 0) was evicted; the next one survives.
        assert!(!b.tombstones.contains_key(&commit(&token(0))));
        assert!(b.tombstones.contains_key(&commit(&token(1))));
    }
}
