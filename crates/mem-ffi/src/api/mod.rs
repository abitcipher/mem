//! The Dart-facing API.
//!
//! Coarse calls on one opaque [`MemNode`]. Only display data, armored ciphertext, QR bit
//! matrices, camera luma going in, and hex ids cross the bridge; keys never do. Consent
//! and channel disclosures are rendered here from exposure profiles, so the UI can style but
//! not rewrite them.

use std::sync::{Mutex, PoisonError};

use cla_qr::{Animation, Pace, ScanEvent, Scanner};
use flutter_rust_bridge::frb;
use mem_core::disclosure::{self, Locale};
use mem_core::model::{
    ChannelOffer, ConsentBasis, ConsentRecord, ConsentScope, ContactId, DeliveryStatus, GroupId,
    GroupStatus, GroupView, MemberRole, MessageKind, MessageView, MsgDirection, MsgId, OfferId,
    OpId, RejectReason, RequestId, RuleId, TrustLevel, sas_emoji,
};
use mem_core::{ClaError, ClaId, Input, Output, PrivacyGrade};
use mem_proto::frame::ObjectKind;
use mem_runtime::{BleCommand, Incoming, Node, NodeConfig, NodeEvent, Runtime};

use crate::frb_generated::StreamSink;

/// Called once by the generated Dart loader.
#[frb(init)]
pub fn init_app() {
    // Not `setup_default_user_utils`: it logs at Trace in every build, and the Iroh stack's trace
    // and debug output carries endpoint ids and addresses into the system log.
    flutter_rust_bridge::setup_log_to_console(if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    });
    flutter_rust_bridge::setup_backtrace();
}

/// A channel the UI can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Animated QR shown on screen, scanned by the other phone (P0).
    Qr,
    /// Direct connection on the same Wi-Fi / local network (P1).
    Lan,
    /// Bluetooth LE between phones next to each other (P0).
    Ble,
    /// The MEM hub, a mailbox on the Internet (P2).
    Hub,
    /// The text itself, unencrypted, through another app (P5).
    Plain,
    /// Through the user's mail account (P4).
    Email,
    /// Armored text through another app or the clipboard (P4).
    Share,
    /// Carried by nearby MEM devices that are not the recipient (P3).
    Courier,
    /// Wi-Fi Direct between nearby phones, no router (P0).
    WifiDirect,
    /// Sound in the same room: pairing cards only, never messages (P0).
    Audio,
}

impl From<Channel> for ClaId {
    fn from(c: Channel) -> Self {
        match c {
            Channel::Qr => Self::Qr,
            Channel::Lan => Self::Iroh,
            Channel::Ble => Self::Ble,
            Channel::Hub => Self::Hub,
            Channel::Plain => Self::Plain,
            Channel::Email => Self::Email,
            Channel::Share => Self::Share,
            Channel::Courier => Self::Courier,
            Channel::WifiDirect => Self::WifiDirect,
            Channel::Audio => Self::Audio,
        }
    }
}

impl From<ClaId> for Channel {
    fn from(c: ClaId) -> Self {
        match c {
            ClaId::Qr => Self::Qr,
            ClaId::Iroh => Self::Lan,
            ClaId::Ble => Self::Ble,
            ClaId::Hub => Self::Hub,
            ClaId::Plain => Self::Plain,
            ClaId::Email => Self::Email,
            ClaId::Share => Self::Share,
            ClaId::Courier => Self::Courier,
            ClaId::WifiDirect => Self::WifiDirect,
            ClaId::Audio => Self::Audio,
        }
    }
}

/// A contact code before import.
pub struct CardPreviewItem {
    /// Grouped fingerprint.
    pub fingerprint: String,
    /// Name on the card (unverified).
    pub display_name: Option<String>,
    /// Already a contact.
    pub known: bool,
}

/// This device's Wi-Fi Direct endpoint.
pub struct WifiEndpoint {
    /// Hex endpoint id (transport key).
    pub id: String,
    /// UDP port.
    pub port: u16,
}

/// Our own profile.
pub struct ProfileView {
    /// Grouped base32 fingerprint.
    pub fingerprint: String,
    /// Display name, if set.
    pub display_name: Option<String>,
}

/// A QR code as a square bit matrix (row-major, 1 = dark), drawn by the UI.
pub struct QrView {
    /// Modules per side.
    pub width: u32,
    /// `width * width` entries of 0 or 1.
    pub modules: Vec<u8>,
}

impl From<cla_qr::QrMatrix> for QrView {
    fn from(m: cla_qr::QrMatrix) -> Self {
        Self {
            width: m.width,
            modules: m.modules,
        }
    }
}

/// An animated QR transfer (fountain-coded). Show [`QrAnimation::frame`] `0, 1, 2…`
/// at a steady rate until the user is done; the receiver needs any [`QrAnimation::frames_needed`]
/// of them.
#[frb(opaque)]
pub struct QrAnimation {
    kind: ObjectKind,
    object: Vec<u8>,
    pace: Pace,
    animation: Animation,
}

/// How densely and quickly an animated QR is shown (a user setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrPace {
    /// Large modules, 5 frames/s: for weak cameras and glare.
    Easy,
    /// The default: 6 frames/s.
    Normal,
    /// Dense, 8 frames/s: for good cameras.
    Fast,
}

impl From<QrPace> for Pace {
    fn from(p: QrPace) -> Self {
        match p {
            QrPace::Easy => Self::Easy,
            QrPace::Normal => Self::Normal,
            QrPace::Fast => Self::Fast,
        }
    }
}

impl From<Pace> for QrPace {
    fn from(p: Pace) -> Self {
        match p {
            Pace::Easy => Self::Easy,
            Pace::Normal => Self::Normal,
            Pace::Fast => Self::Fast,
        }
    }
}

impl QrAnimation {
    /// Prepares `object` at the default pace. `None` if it cannot be fountain-coded (empty or
    /// over 64 KiB).
    fn new(kind: ObjectKind, object: &[u8]) -> Option<Self> {
        let pace = Pace::default();
        let animation = Animation::with_symbol_len(kind, object, pace.symbol_len()).ok()?;
        Some(Self {
            kind,
            object: object.to_vec(),
            pace,
            animation,
        })
    }

    /// A small object (a connect hint) as one frame of its own size, but at least the smallest
    /// symbol frames allow: the smallest QR code, so a camera reads it from farther away than a
    /// padded animation frame.
    fn single(kind: ObjectKind, object: &[u8]) -> Option<Self> {
        let len = u16::try_from(object.len())
            .ok()?
            .max(mem_proto::frame::MIN_SYMBOL_LEN);
        let animation = Animation::with_symbol_len(kind, object, len).ok()?;
        Some(Self {
            kind,
            object: object.to_vec(),
            pace: Pace::default(),
            animation,
        })
    }

    /// The current pace.
    #[frb(sync)]
    #[must_use]
    pub fn pace(&self) -> QrPace {
        self.pace.into()
    }

    /// Re-encodes the transfer at another pace. Frame numbers restart at 0; a receiver part way
    /// through keeps going (it collects the new frames as a parallel transfer).
    #[frb(sync)]
    pub fn set_pace(&mut self, pace: QrPace) {
        let pace = Pace::from(pace);
        if pace == self.pace {
            return;
        }
        // Cannot fail: the object was already accepted at another symbol size in the same bounds.
        if let Ok(animation) =
            Animation::with_symbol_len(self.kind, &self.object, pace.symbol_len())
        {
            self.pace = pace;
            self.animation = animation;
        }
    }

    /// Frames to show per second at the current pace.
    #[frb(sync)]
    #[must_use]
    pub fn frames_per_second(&self) -> u8 {
        self.pace.frames_per_second()
    }

    /// Minimum frames the receiver must catch.
    #[frb(sync)]
    #[must_use]
    pub fn frames_needed(&self) -> u32 {
        self.animation.frames_needed()
    }

    /// Frame `n` of the endless stream.
    #[frb(sync)]
    #[must_use]
    pub fn frame(&self, n: u32) -> Option<QrView> {
        self.animation.frame(n).ok().map(QrView::from)
    }
}

/// Reassembles QR transfers from camera images. One per open scanner screen.
#[frb(opaque)]
pub struct QrScanner {
    scanner: Mutex<Scanner>,
}

/// Creates a scanner.
#[frb(sync)]
#[must_use]
pub fn new_qr_scanner() -> QrScanner {
    QrScanner {
        scanner: Mutex::new(Scanner::new()),
    }
}

/// Which sound pairing uses.
pub enum SoundProfile {
    /// Audible tones: works on every phone.
    Audible,
    /// Near-ultrasound: quieter to people; some phones cannot play or hear it.
    Ultrasound,
}

/// One exchange by sound: pairing (cards) or connecting (connect hints).
/// Plays ours, listens for the other phone's. Keep one at a time; drop it when the screen closes
/// (it frees the microphone decoder).
#[frb(opaque)]
pub struct SoundSession {
    kind: ObjectKind,
    pairing: Mutex<cla_audio::SoundPairing>,
}

impl SoundSession {
    /// Frames in one round of our card (what the other phone usually needs).
    #[frb(sync)]
    #[must_use]
    pub fn frames_per_round(&self) -> u32 {
        self.pairing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .frames_per_round()
    }

    /// Whether another phone is playing right now; wait before the next burst.
    #[frb(sync)]
    #[must_use]
    pub fn busy(&self) -> bool {
        self.pairing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .busy()
    }

    /// The next frame of our card as 16-bit mono PCM at [`sound_sample_rate`], to play once.
    ///
    /// # Errors
    ///
    /// `"sound_failed"` if encoding fails.
    pub fn next_burst(&self) -> Result<Vec<i16>, String> {
        self.pairing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .next_burst()
            .map_err(|_| "sound_failed".into())
    }
}

/// Sample rate of all sound PCM, Hz.
#[frb(sync)]
#[must_use]
pub fn sound_sample_rate() -> u32 {
    cla_audio::SAMPLE_RATE
}

/// What one camera image achieved.
pub enum ScanUpdate {
    /// Nothing usable in this image.
    Nothing,
    /// A transfer is in progress.
    Progress {
        /// Frames collected.
        received: u32,
        /// Frames needed.
        needed: u32,
    },
    /// Something complete arrived and was handed to the node.
    Received(ReceiveOutcome),
    /// An encrypted backup arrived. It is not opened: the user must type the
    /// recovery phrase, then pass these bytes to [`MemNode::restore_with_phrase`].
    Backup(Vec<u8>),
    /// A sealed device setup arrived; only a device waiting to be added uses it.
    DeviceSetup(Vec<u8>),
    /// A new device's request (`memdevice1:`) was read; only the main device's "Add a device"
    /// screen uses it, after the user confirms.
    DeviceRequest(Vec<u8>),
    /// Another phone's connect hint was read (sound or QR) and is being dialled over
    /// the local network; contacts' messages follow by themselves.
    Connecting,
    /// An attestation code was read: a relying party's request (`memattreq1:`) or
    /// another phone's offer (`memattest1:`). Only the "Confirm in person" screen uses it.
    Attest(String),
}

/// How the recovery phrase is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseFormItem {
    /// 12 English words (BIP-39).
    English,
    /// 9 Latin syllables (proquints), language-neutral.
    Syllables,
}

impl From<PhraseFormItem> for mem_core::PhraseForm {
    fn from(f: PhraseFormItem) -> Self {
        match f {
            PhraseFormItem::English => Self::English,
            PhraseFormItem::Syllables => Self::Proquint,
        }
    }
}

/// Backup state for the settings screen. No secret material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupStatus {
    /// Whether a recovery phrase exists.
    pub enabled: bool,
    /// How the phrase is shown.
    pub form: PhraseFormItem,
}

/// The outcome of a restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreResult {
    /// When the backup was made, Unix ms.
    pub created_unix_ms: i64,
    /// Whether message history was included.
    pub history: bool,
    /// The phrase as corrected (one mistyped letter fixed), for the user to check. This is the
    /// phrase the user typed, corrected; it is part of the restore call.
    pub corrected_phrase: Option<String>,
    /// Whether an existing identity was replaced (its state was kept, encrypted).
    pub replaced: bool,
}

/// A UI refresh hint from the node. Ids only.
pub enum NodeEventItem {
    /// A message from this contact arrived over the network.
    MessageReceived {
        /// Contact hex id.
        contact_id: String,
    },
    /// A message's status changed.
    MessageStatus {
        /// Message hex id.
        message_id: String,
    },
    /// Who is reachable nearby changed.
    Reachability,
    /// The contact list changed.
    Contacts,
    /// A contact request arrived.
    Requests,
    /// A group changed or an invitation arrived.
    Groups,
    /// A group message arrived.
    GroupMessage {
        /// Group hex id.
        group_id: String,
    },
    /// Our main device removed this device: tell the user, then
    /// [`MemNode::forget_this_device`].
    DeviceRetired,
}

/// A command for the Bluetooth radio plugin. Link ids are the plugin's own.
pub enum BleCommandItem {
    /// Write these stream bytes to the link in order, then call `ble_sent(op, ok)`.
    Send {
        /// Plugin link id.
        link: u64,
        /// Write id.
        op: u64,
        /// Stream bytes.
        bytes: Vec<u8>,
    },
    /// Close the link (then report it down as usual).
    Close {
        /// Plugin link id.
        link: u64,
    },
}

/// A contact request waiting for an answer. Never shown as a message.
pub struct RequestItem {
    /// Request id.
    pub id: u64,
    /// Grouped base32 fingerprint of the sender.
    pub fingerprint: String,
    /// Name from the sender's card: not verified.
    pub display_name: Option<String>,
    /// The sender's note.
    pub note: Option<String>,
    /// When it arrived, Unix ms.
    pub received_unix_ms: i64,
    /// How it arrived.
    pub via: ChannelUseItem,
}

/// A channel, its exposure and whether it is on (the Channels screen).
pub struct ChannelInfo {
    /// Channel.
    pub channel: Channel,
    /// Localized name.
    pub name: String,
    /// Grade label.
    pub grade: String,
    /// Whether the user can turn it off.
    pub can_disable: bool,
    /// Whether it is on.
    pub enabled: bool,
    /// Who observes.
    pub observers: String,
    /// What they learn.
    pub exposed: Vec<String>,
    /// What they learn partially.
    pub limited: Vec<String>,
    /// What stays protected.
    pub protected: Vec<String>,
}

/// Result of [`MemNode::send_text`].
pub struct SentText {
    /// Message hex id.
    pub message_id: String,
    /// Whether it is already on its way to the contact over a nearby connection.
    pub sending_nearby: bool,
}

/// Our pairing card.
pub struct CardView {
    /// Armored text (`memcard1:...`).
    pub text: String,
    /// The same text as a QR code.
    pub qr: Option<QrView>,
}

/// A contact for display.
pub struct ContactItem {
    /// Hex id.
    pub id: String,
    /// Self-chosen name from the card, untrusted.
    pub display_name: Option<String>,
    /// Grouped base32 fingerprint.
    pub fingerprint: String,
    /// Short authentication string as emoji.
    pub sas: String,
    /// Whether the cards were exchanged in person by QR.
    pub met_in_person: bool,
    /// We sent them a contact request they have not answered yet.
    pub requested: bool,
    /// Privacy floor, 0..=5 for P0..P5.
    pub floor: u8,
    /// Last message preview, if any.
    pub last_text: Option<String>,
    /// Time of the last message or of pairing, Unix ms.
    pub last_unix_ms: i64,
    /// Their email address, typed by the user.
    pub email: Option<String>,
}

/// The mail account for the email bridge. The password goes in, never out.
pub struct EmailAccountItem {
    /// Address.
    pub address: String,
    /// IMAP host.
    pub imap_host: String,
    /// IMAP port.
    pub imap_port: u16,
    /// SMTP host.
    pub smtp_host: String,
    /// SMTP port.
    pub smtp_port: u16,
    /// Login.
    pub username: String,
    /// The app password when setting; `None` keeps the saved one. Always `None` when read.
    pub password: Option<String>,
}

/// One channel use, for provenance badges.
pub struct ChannelUseItem {
    /// Channel.
    pub channel: Channel,
    /// Grade label, e.g. `"P0"`.
    pub grade: String,
    /// When, Unix ms.
    pub at_unix_ms: i64,
}

/// Message status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    /// Waiting for the user to pick a channel.
    Queued,
    /// Handed to a channel, not yet confirmed by the recipient.
    Sent,
    /// Confirmed by the recipient's ack.
    Delivered,
    /// Last attempt failed, or it expired unconfirmed.
    Failed,
    /// Received.
    Received,
}

const DAY_S: u32 = 24 * 60 * 60;

/// The retry period setting, in days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryDays {
    /// Current value.
    pub current: u32,
    /// Smallest allowed.
    pub min: u32,
    /// Largest allowed.
    pub max: u32,
}

/// Courier mode: what this device carries for other people.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CourierStatus {
    /// Bytes in use.
    pub used_bytes: u64,
    /// Limit in bytes; 0 means courier mode is off.
    pub limit_bytes: u64,
    /// Largest limit the user can choose.
    pub max_bytes: u64,
    /// Default limit for a new install.
    pub default_bytes: u64,
    /// Bundles carried.
    pub bundles: u32,
}

/// A message for display.
pub struct MessageItem {
    /// Hex id.
    pub id: String,
    /// Whether we sent it.
    pub outgoing: bool,
    /// Our contact request (its text is the note, possibly empty), not a text message.
    pub is_request: bool,
    /// Text.
    pub text: String,
    /// Sender's creation time, Unix ms.
    pub created_unix_ms: i64,
    /// Status.
    pub status: MessageStatus,
    /// Every channel that carried it.
    pub channels: Vec<ChannelUseItem>,
    /// Hex id of the contact or group member who wrote it, or the recipient of our 1:1 message.
    pub sender_id: String,
    /// Our group message: members who confirmed it, of `sent_to`.
    pub delivered_to: Option<u16>,
    /// Our group message: members it went to.
    pub sent_to: Option<u16>,
}

/// Where we stand in a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStatusItem {
    /// Invited; not answered.
    Invited,
    /// Accepted; waiting for the admin.
    Joining,
    /// A member.
    Member,
    /// We left (or ended it as admin).
    Left,
    /// Removed by the admin.
    Removed,
}

/// A member's place in a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRoleItem {
    /// The admin.
    Admin,
    /// Joined.
    Member,
    /// Invited, not joined (shown to the admin).
    Invited,
}

/// A group member.
pub struct GroupMemberItem {
    /// Hex id.
    pub contact_id: String,
    /// Name from their card (unverified).
    pub display_name: Option<String>,
    /// Grouped fingerprint.
    pub fingerprint: String,
    /// One of our contacts.
    pub is_contact: bool,
    /// This device.
    pub is_me: bool,
    /// Role.
    pub role: MemberRoleItem,
}

/// A group.
pub struct GroupItem {
    /// Hex id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Our standing.
    pub status: GroupStatusItem,
    /// We are the admin.
    pub is_admin: bool,
    /// Members, joined first, then invited.
    pub members: Vec<GroupMemberItem>,
    /// When created or received, Unix ms.
    pub created_unix_ms: i64,
}

/// A channel choice for a queued message.
pub struct ChannelOptionItem {
    /// Channel.
    pub channel: Channel,
    /// Localized channel name.
    pub name: String,
    /// Grade label.
    pub grade: String,
    /// Whether the message fits this channel.
    pub fits: bool,
    /// Whether choosing it will ask for consent first.
    pub needs_consent: bool,
    /// Whether it can deliver now (network channels: the contact is nearby).
    pub reachable: bool,
}

/// A consent request with its generated disclosure.
pub struct ConsentRequest {
    /// Offer id to answer with.
    pub offer_id: u64,
    /// Message hex id.
    pub message_id: String,
    /// Channel.
    pub channel: Channel,
    /// Grade, 0..=5.
    pub grade: u8,
    /// Disclosure headline.
    pub title: String,
    /// Who observes.
    pub observers: String,
    /// What they learn.
    pub exposed: Vec<String>,
    /// What they learn partially.
    pub limited: Vec<String>,
    /// What stays protected.
    pub protected: Vec<String>,
}

/// Result of choosing a channel.
pub enum TransmitOutcome {
    /// Ready to hand to the channel. Report back with [`MemNode::transmit_done`].
    Ready {
        /// Operation id.
        op: u64,
        /// Channel.
        channel: Channel,
        /// Armored bundle text (share channel).
        text: String,
        /// Animated QR frames (QR channel).
        animation: Option<QrAnimation>,
    },
    /// Handed to a nearby peer; the status updates by itself (no `transmit_done`).
    Sending {
        /// Channel.
        channel: Channel,
    },
    /// Allowed to travel with nearby MEM devices; it goes with the next one met.
    CourierQueued,
    /// Allowed, once, as plain text (P5): show `text` for a last confirmation, hand
    /// it to another app, and report with [`MemNode::transmit_done`].
    PlainReady {
        /// Operation id.
        op: u64,
        /// The exact text that will be exposed.
        text: String,
    },
    /// The user must decide first.
    ConsentRequired(ConsentRequest),
    /// Refused; `reason` is a stable key for localization.
    Rejected {
        /// Reason key.
        reason: String,
    },
}

/// Consent scope chosen in the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentChoice {
    /// Only this message.
    Once,
    /// This contact over this channel until revoked.
    ThisContact,
}

/// Result of feeding pasted or scanned text.
pub enum ReceiveOutcome {
    /// A message was received from this contact.
    Message {
        /// Contact hex id.
        contact_id: String,
    },
    /// A contact was added or re-paired.
    Contact {
        /// Contact hex id.
        contact_id: String,
        /// Whether it already existed.
        updated: bool,
        /// Its card was read again and nothing needs checking: no symbols to compare.
        unchanged: bool,
        /// Short authentication string to compare.
        sas: String,
    },
    /// Refused; `reason` is a stable key for localization.
    Rejected {
        /// Reason key.
        reason: String,
    },
    /// A contact request arrived and waits on the requests screen.
    Request,
    /// The text contains no MEM data.
    NotMem,
}

/// One privacy log entry.
pub struct PrivacyLogItem {
    /// When, Unix ms.
    pub at_unix_ms: i64,
    /// Contact display name or fingerprint.
    pub contact: String,
    /// Channel.
    pub channel: Channel,
    /// Grade label.
    pub grade: String,
    /// Floor label.
    pub floor: String,
    /// `"once"`, `"new_rule"` or `"rule"`.
    pub basis: String,
}

/// One standing consent rule.
pub struct RuleItem {
    /// Rule id.
    pub id: u64,
    /// Contact name, or `None` for all contacts.
    pub contact: Option<String>,
    /// Contact id (hex), or `None` for all contacts. Match rules to a contact by this, never by
    /// name: two contacts may share a name.
    pub contact_id: Option<String>,
    /// Channel.
    pub channel: Channel,
    /// Highest permitted grade label.
    pub max_grade: String,
    /// End of the rule, Unix ms, or `None` until revoked.
    pub until_unix_ms: Option<u64>,
}

/// The local MEM node. Opaque to Dart.
#[frb(opaque)]
pub struct MemNode {
    node: Node,
    /// A device waiting to be added, between showing its request and reading
    /// the setup. Memory only.
    join: Mutex<Option<mem_core::DeviceJoin>>,
}

/// One of our own devices.
pub struct DeviceItem {
    /// Hex device id (16 digits; zeros for a main device that never added another).
    pub id: String,
    /// The main device (holds the identity key, pairs, adds and removes devices).
    pub main: bool,
    /// This device.
    pub this_device: bool,
    /// Opened its setup and reported in; `false` only on the main device, for a device still
    /// waiting (an abandoned one can be removed).
    pub joined: bool,
}

/// Opens (or creates) the profile stored in `dir` and starts its channels.
///
/// # Errors
///
/// A human-readable message if storage cannot be opened.
pub fn open_node(dir: String) -> Result<MemNode, String> {
    // The app trusts hub lists signed by the operator keys built into it.
    let config = NodeConfig {
        operator_keys: mem_runtime::OPERATOR_KEYS.to_vec(),
        // Android: the state key is held by the Keystore.
        #[cfg(target_os = "android")]
        key_guard: crate::keystore::guard(),
        ..NodeConfig::default()
    };
    Node::open(dir, config)
        .map(|node| MemNode {
            node,
            join: Mutex::new(None),
        })
        .map_err(|e| e.to_string())
}

/// One phone's side of an in-person attestation. Opaque: the derived key stays in
/// Rust; signing consumes it.
#[frb(opaque)]
pub struct AttestFlow {
    session: Mutex<Option<mem_core::attest::AttestSession>>,
    view: AttestRequestView,
}

/// What a relying party asks for, for the confirmation screens.
#[derive(Clone)]
pub struct AttestRequestView {
    /// Who asks, `https://host`.
    pub origin: String,
    /// What they call it (untrusted, printable ASCII).
    pub label: String,
    /// The first 8 characters of the subject (lowercase base32), to compare with their page.
    pub subject_short: String,
    /// This user's role, as the relying party names it.
    pub role: String,
    /// Where the attestation is sent.
    pub submit_url: String,
    /// The disclosure, in the requested language.
    pub disclosure: String,
}

/// A signed attestation, ready to send.
pub struct SignedAttestation {
    /// The HTTPS body (`application/cbor`).
    pub body: Vec<u8>,
    /// The `memattsig1:` text, for the copy/paste fallback.
    pub text: String,
    /// Where to send `body`.
    pub submit_url: String,
}

/// One attestation in the log.
pub struct AttestLogItem {
    /// When, Unix ms.
    pub at_unix_ms: i64,
    /// Who asked.
    pub origin: String,
    /// What they called it.
    pub label: String,
    /// Sent over HTTPS (`true`) or copied (`false`).
    pub sent: bool,
}

fn attest_error(e: &mem_core::attest::AttestError) -> String {
    use mem_core::attest::AttestError as E;
    use mem_proto::attest::OfferMismatch as M;
    match e {
        E::WrongObject => "wrong_object",
        E::Invalid(_) => "invalid",
        E::Mismatch(M::Subject) => "other_subject",
        E::Mismatch(M::Role) => "same_role",
        E::Mismatch(M::SameKey) => "own_code",
        E::NoPeer => "no_peer",
        E::BadSignature => "bad_signature",
        E::NotMainDevice => "main_device_only",
        _ => "invalid",
    }
    .into()
}

impl AttestFlow {
    /// The request, for the disclosure and confirmation screens.
    #[frb(sync)]
    #[must_use]
    pub fn request(&self) -> AttestRequestView {
        self.view.clone()
    }

    /// This phone's offer as a single QR code (and its text).
    ///
    /// # Errors
    ///
    /// `"done"` after signing, `"invalid"` otherwise.
    #[frb(sync)]
    pub fn offer(&self) -> Result<CardView, String> {
        let slot = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        let session = slot.as_ref().ok_or("done")?;
        let text = session.offer_text().map_err(|e| attest_error(&e))?;
        Ok(CardView {
            qr: qr(&text),
            text,
        })
    }

    /// Takes the other phone's scanned or pasted offer; returns the six emoji to compare.
    ///
    /// # Errors
    ///
    /// `"wrong_object"`, `"invalid"`, `"other_subject"`, `"same_role"`, `"own_code"` or `"done"`.
    #[frb(sync)]
    pub fn accept_peer(&self, text: String) -> Result<String, String> {
        let mut slot = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        let session = slot.as_mut().ok_or("done")?;
        session
            .accept_peer(&text)
            .map(|sas| sas_emoji(&sas))
            .map_err(|e| attest_error(&e))
    }

    /// Signs, after the users confirmed that the emoji match. Ends the session: the key is
    /// dropped, and a second call fails.
    ///
    /// # Errors
    ///
    /// `"no_peer"` before [`AttestFlow::accept_peer`] succeeded, `"done"` after signing.
    #[frb(sync)]
    pub fn sign(&self) -> Result<SignedAttestation, String> {
        let mut slot = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        let session = slot.take().ok_or("done")?;
        match session.sign() {
            Ok(body) => Ok(SignedAttestation {
                text: mem_core::attest::attestation_text(&body),
                body,
                submit_url: self.view.submit_url.clone(),
            }),
            Err(mem_core::attest::AttestError::NoPeer) => Err("no_peer".into()),
            Err(e) => Err(attest_error(&e)),
        }
    }
}

/// Renders arbitrary text as a QR code (for showing armored text).
#[frb(sync)]
#[must_use]
pub fn qr_for_text(text: String) -> Option<QrView> {
    qr(&text)
}

impl MemNode {
    #[allow(
        clippy::unnecessary_wraps,
        reason = "keeps every query fallible from Dart's view"
    )]
    fn with<T>(&self, f: impl FnOnce(&Runtime) -> T) -> Result<T, String> {
        Ok(self.node.read(f))
    }

    fn handle(&self, input: Input) -> Result<Vec<Output>, String> {
        self.node.handle(input).map_err(|e| e.to_string())
    }

    /// Streams UI refresh hints until the stream is closed on the Dart side.
    ///
    /// # Errors
    ///
    /// Never; fallible for FRB's stream signature.
    pub fn events(&self, sink: StreamSink<NodeEventItem>) -> Result<(), String> {
        self.node.on_event(move |e| {
            let item = match e {
                NodeEvent::MessageReceived(c) => NodeEventItem::MessageReceived {
                    contact_id: hex::encode(c.as_bytes()),
                },
                NodeEvent::MessageStatus(m) => NodeEventItem::MessageStatus {
                    message_id: hex::encode(m.as_bytes()),
                },
                NodeEvent::Reachability => NodeEventItem::Reachability,
                NodeEvent::Contacts => NodeEventItem::Contacts,
                NodeEvent::Requests => NodeEventItem::Requests,
                NodeEvent::Groups => NodeEventItem::Groups,
                NodeEvent::GroupMessage(g) => NodeEventItem::GroupMessage {
                    group_id: hex::encode(g.as_bytes()),
                },
                NodeEvent::DeviceRetired => NodeEventItem::DeviceRetired,
            };
            sink.add(item).is_ok()
        });
        Ok(())
    }

    /// Feeds one camera image (its luma plane) to `scanner`. A completed transfer or a MEM
    /// text code goes straight to the node, received over the QR channel (P0).
    ///
    /// # Errors
    ///
    /// A storage error while receiving.
    pub fn scan_frame(
        &self,
        scanner: &QrScanner,
        luma: Vec<u8>,
        width: u32,
        height: u32,
        row_stride: u32,
    ) -> Result<ScanUpdate, String> {
        let event = scanner
            .scanner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .scan_luma(&luma, width, height, row_stride);
        Ok(match event {
            ScanEvent::Nothing | ScanEvent::AlreadyComplete => ScanUpdate::Nothing,
            ScanEvent::Progress { received, needed } => ScanUpdate::Progress { received, needed },
            ScanEvent::Text(text) if matches!(mem_proto::attest::dearmor(&text), Ok(Some(_))) => {
                ScanUpdate::Attest(text)
            }
            ScanEvent::Text(text) if text.contains(mem_proto::device::DEVICE_REQUEST_PREFIX) => {
                match mem_proto::armor::dearmor(&text) {
                    Ok(mem_proto::armor::Armored::DeviceRequest(r)) => ScanUpdate::DeviceRequest(r),
                    _ => ScanUpdate::Nothing,
                }
            }
            ScanEvent::Text(text) => match self.receive_text(text, Channel::Qr)? {
                ReceiveOutcome::NotMem => ScanUpdate::Nothing,
                outcome => ScanUpdate::Received(outcome),
            },
            ScanEvent::Complete { kind, object } => {
                let input = match kind {
                    ObjectKind::Bundle => Input::Received {
                        cla: ClaId::Qr,
                        bytes: object,
                    },
                    ObjectKind::Card => Input::ImportCard {
                        cla: ClaId::Qr,
                        bytes: object,
                    },
                    ObjectKind::Backup => return Ok(ScanUpdate::Backup(object)),
                    ObjectKind::DeviceSetup => return Ok(ScanUpdate::DeviceSetup(object)),
                    ObjectKind::ConnectHint => return Ok(self.dial_hint(&object)),
                };
                ScanUpdate::Received(self.receive_outcome(&self.handle(input)?)?)
            }
        })
    }

    /// Starts pairing by sound with our current card.
    ///
    /// # Errors
    ///
    /// `"sound_failed"` if the sound engine cannot start, or a storage error.
    pub fn start_sound_pairing(&self, profile: SoundProfile) -> Result<SoundSession, String> {
        let card = self.with(|rt| rt.agent().own_card().map_err(reason_key))??;
        let profile = match profile {
            SoundProfile::Audible => cla_audio::Profile::Audible,
            SoundProfile::Ultrasound => cla_audio::Profile::Ultrasound,
        };
        let pairing =
            cla_audio::SoundPairing::new(&card, profile).map_err(|_| "sound_failed".to_string())?;
        Ok(SoundSession {
            kind: ObjectKind::Card,
            pairing: Mutex::new(pairing),
        })
    }

    /// Starts connecting by sound: plays this phone's connect hint and listens for
    /// another phone's, which it then dials.
    ///
    /// # Errors
    ///
    /// `"no_local_network"` while the local network channel is off or the phone has no local
    /// IPv4 address; `"sound_failed"` if the sound engine cannot start.
    pub fn start_sound_connect(&self, profile: SoundProfile) -> Result<SoundSession, String> {
        let hint = self.own_hint()?;
        let profile = match profile {
            SoundProfile::Audible => cla_audio::Profile::Audible,
            SoundProfile::Ultrasound => cla_audio::Profile::Ultrasound,
        };
        let pairing = cla_audio::SoundPairing::for_object(ObjectKind::ConnectHint, &hint, profile)
            .map_err(|_| "sound_failed".to_string())?;
        Ok(SoundSession {
            kind: ObjectKind::ConnectHint,
            pairing: Mutex::new(pairing),
        })
    }

    /// This phone's connect hint as a one-frame QR code ("Connect by code").
    ///
    /// # Errors
    ///
    /// `"no_local_network"` as for [`MemNode::start_sound_connect`].
    pub fn connect_hint_qr(&self) -> Result<QrAnimation, String> {
        let hint = self.own_hint()?;
        QrAnimation::single(ObjectKind::ConnectHint, &hint).ok_or_else(|| "no_local_network".into())
    }

    fn own_hint(&self) -> Result<Vec<u8>, String> {
        self.node
            .connect_hint()
            .and_then(|h| h.encode().ok())
            .ok_or_else(|| "no_local_network".into())
    }

    /// Dials the phone a received hint names; anything malformed is ignored.
    fn dial_hint(&self, bytes: &[u8]) -> ScanUpdate {
        match mem_proto::hint::ConnectHint::decode(bytes) {
            Ok(hint) if self.node.dial_hint(&hint) => ScanUpdate::Connecting,
            _ => ScanUpdate::Nothing,
        }
    }

    /// Feeds recorded PCM to a sound pairing. A complete card is imported as met in person,
    /// received over sound (P0), exactly as a scanned QR card.
    ///
    /// # Errors
    ///
    /// A storage error while importing.
    pub fn sound_listen(
        &self,
        session: &SoundSession,
        pcm: Vec<i16>,
    ) -> Result<ScanUpdate, String> {
        let heard = session
            .pairing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .listen(&pcm);
        Ok(match heard {
            cla_audio::Heard::Nothing => ScanUpdate::Nothing,
            cla_audio::Heard::Progress { received, needed } => {
                ScanUpdate::Progress { received, needed }
            }
            cla_audio::Heard::Complete(bytes) if session.kind == ObjectKind::ConnectHint => {
                self.dial_hint(&bytes)
            }
            cla_audio::Heard::Complete(bytes) => {
                ScanUpdate::Received(self.receive_outcome(&self.handle(Input::ImportCard {
                    cla: ClaId::Audio,
                    bytes,
                })?)?)
            }
        })
    }

    /// Whether the local-network channel is on.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn lan_enabled(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().channel_enabled(ClaId::Iroh))
    }

    /// Turns the local-network channel on or off (persisted).
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_lan_enabled(&self, enabled: bool) -> Result<(), String> {
        self.node
            .set_lan_enabled(enabled)
            .map_err(|e| e.to_string())
    }

    /// The hub's address, if set.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn hub_url(&self) -> Result<Option<String>, String> {
        self.with(|rt| rt.agent().hub_url().map(String::from))
    }

    /// Sets (or clears) the hub's address and reconnects.
    ///
    /// # Errors
    ///
    /// `"invalid_hub_url"` (only `wss://`, or `ws://` to this machine), or a storage error.
    pub fn set_hub_url(&self, url: Option<String>) -> Result<(), String> {
        let out = self.node.set_hub_url(url).map_err(|e| e.to_string())?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// Whether the hub channel is on.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn hub_enabled(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().channel_enabled(ClaId::Hub))
    }

    /// Turns the hub channel on or off (persisted); connects when on and an address is set.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_hub_enabled(&self, enabled: bool) -> Result<(), String> {
        self.node
            .set_hub_enabled(enabled)
            .map_err(|e| e.to_string())
    }

    /// Names of the partner operators whose hubs exchange mail with this one, from the verified
    /// hub list; empty without a list or partners.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn hub_partner_operators(&self) -> Result<Vec<String>, String> {
        self.with(|rt| {
            let mut names: Vec<String> = rt
                .agent()
                .hub_partners()
                .iter()
                .map(|p| p.operator.clone())
                .collect();
            names.sort();
            names.dedup();
            names
        })
    }

    /// Whether "Only my operator's hubs" is on.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn own_operator_only(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().own_operator_only())
    }

    /// Turns "Only my operator's hubs" on or off (persisted).
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_own_operator_only(&self, enabled: bool) -> Result<(), String> {
        self.handle(Input::SetOwnOperatorOnly { enabled })
            .map(|_| ())
    }

    /// The mail account, without its password.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn email_account(&self) -> Result<Option<EmailAccountItem>, String> {
        self.with(|rt| {
            rt.agent().email_account().map(|a| EmailAccountItem {
                address: a.address,
                imap_host: a.imap_host,
                imap_port: a.imap_port,
                smtp_host: a.smtp_host,
                smtp_port: a.smtp_port,
                username: a.username,
                password: None,
            })
        })
    }

    /// Sets the mail account (`None` removes it). A `None` password keeps the saved one.
    ///
    /// # Errors
    ///
    /// `"invalid_email_account"`, or a storage error.
    pub fn set_email_account(&self, account: Option<EmailAccountItem>) -> Result<(), String> {
        let account = match account {
            None => None,
            Some(a) => {
                let password = match a.password {
                    Some(p) => zeroize::Zeroizing::new(p),
                    None => self
                        .with(|rt| rt.agent().email_account().map(|old| old.password))?
                        .ok_or("invalid_email_account")?,
                };
                Some(mem_core::EmailAccount {
                    address: a.address.trim().into(),
                    imap_host: a.imap_host.trim().into(),
                    imap_port: a.imap_port,
                    smtp_host: a.smtp_host.trim().into(),
                    smtp_port: a.smtp_port,
                    username: a.username.trim().into(),
                    password,
                })
            }
        };
        let out = self
            .node
            .set_email_account(account)
            .map_err(|e| e.to_string())?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// Whether the email bridge is on.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn email_enabled(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().channel_enabled(ClaId::Email))
    }

    /// Turns the email bridge on or off.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_email_enabled(&self, enabled: bool) -> Result<(), String> {
        self.node
            .set_email_enabled(enabled)
            .map_err(|e| e.to_string())
    }

    /// Whether the mail client is logged in now.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn email_online(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().email_online())
    }

    /// Removes a contact: its conversation, queued messages and rules; a fellow group member
    /// stays only in its groups.
    ///
    /// # Errors
    ///
    /// `"unknown_contact"`.
    pub fn remove_contact(&self, contact_id: String) -> Result<(), String> {
        self.handle(Input::RemoveContact(contact(&contact_id)?))?;
        Ok(())
    }

    /// Sets or clears a contact's email address.
    ///
    /// # Errors
    ///
    /// `"invalid_email_address"`, `"unknown_contact"`.
    pub fn set_contact_email(
        &self,
        contact_id: String,
        address: Option<String>,
    ) -> Result<(), String> {
        let out = self.handle(Input::SetContactEmail {
            contact: contact(&contact_id)?,
            address,
        })?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// The push endpoint the hub wakes us at, if set.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn push_endpoint(&self) -> Result<Option<String>, String> {
        self.with(|rt| rt.agent().push_endpoint().map(String::from))
    }

    /// Sets (or clears) the push endpoint from the UnifiedPush distributor.
    ///
    /// # Errors
    ///
    /// `"invalid_push_endpoint"`, or a storage error.
    pub fn set_push_endpoint(&self, endpoint: Option<String>) -> Result<(), String> {
        let out = self
            .node
            .set_push_endpoint(endpoint)
            .map_err(|e| e.to_string())?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// Whether the hub is connected now.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn hub_online(&self) -> Result<bool, String> {
        Ok(self.node.hub_online())
    }

    /// Whether Bluetooth LE is on (off until the user turns it on).
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn ble_enabled(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().channel_enabled(ClaId::Ble))
    }

    /// Turns Bluetooth LE on or off (persisted). The app starts or stops the radio itself.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_ble_enabled(&self, enabled: bool) -> Result<(), String> {
        self.node
            .set_ble_enabled(enabled)
            .map_err(|e| e.to_string())
    }

    /// Whether Wi-Fi Direct is on.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn wifi_direct_enabled(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().channel_enabled(ClaId::WifiDirect))
    }

    /// Turns Wi-Fi Direct on or off (persisted). The app starts or stops the radio itself.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_wifi_direct_enabled(&self, enabled: bool) -> Result<(), String> {
        self.node
            .set_wifi_direct_enabled(enabled)
            .map_err(|e| e.to_string())
    }

    /// This device's Wi-Fi Direct endpoint for the advertisement: hex endpoint id (the per-start
    /// transport key, never the identity) and UDP port; none while the channel is off.
    #[frb(sync)]
    #[must_use]
    pub fn wifi_local(&self) -> Option<WifiEndpoint> {
        self.node.wifi_local().map(|(id, port)| WifiEndpoint {
            id: hex::encode(id),
            port,
        })
    }

    /// The radio joined a Wi-Fi Direct group: dial the peer's endpoint at `ip:port`. Bad input
    /// from a nearby advertisement is ignored.
    #[frb(sync)]
    pub fn wifi_peer(&self, id: String, ip: String, port: u16) {
        let (Ok(id), Ok(ip)) = (hex::decode(id), ip.parse::<std::net::IpAddr>()) else {
            return;
        };
        if let Ok(id) = <[u8; 32]>::try_from(id.as_slice()) {
            self.node.wifi_peer(id, std::net::SocketAddr::new(ip, port));
        }
    }

    /// Open Wi-Fi Direct encounters (the app ends an idle group).
    #[frb(sync)]
    #[must_use]
    pub fn wifi_connections(&self) -> u32 {
        u32::try_from(self.node.wifi_connections()).unwrap_or(u32::MAX)
    }

    /// Streams commands for the radio plugin until the stream is closed on the Dart side.
    /// Links opened before are ended (the plugin restarted).
    ///
    /// # Errors
    ///
    /// Never; fallible for FRB's stream signature.
    pub fn ble_commands(&self, sink: StreamSink<BleCommandItem>) -> Result<(), String> {
        self.node.on_ble_command(move |c| {
            let item = match c {
                BleCommand::Send { link, op, bytes } => BleCommandItem::Send { link, op, bytes },
                BleCommand::Close { link } => BleCommandItem::Close { link },
            };
            sink.add(item).is_ok()
        });
        Ok(())
    }

    /// The plugin connected a link (either role).
    ///
    /// # Errors
    ///
    /// Never.
    pub fn ble_link_up(&self, link: u64) -> Result<(), String> {
        self.node.ble_link_up(link);
        Ok(())
    }

    /// Bytes arrived on a link, in stream order.
    ///
    /// # Errors
    ///
    /// Never.
    pub fn ble_received(&self, link: u64, bytes: Vec<u8>) -> Result<(), String> {
        self.node.ble_received(link, &bytes);
        Ok(())
    }

    /// A link went down.
    ///
    /// # Errors
    ///
    /// Never.
    pub fn ble_link_down(&self, link: u64) -> Result<(), String> {
        self.node.ble_link_down(link);
        Ok(())
    }

    /// A write finished.
    ///
    /// # Errors
    ///
    /// Never.
    pub fn ble_sent(&self, op: u64, ok: bool) -> Result<(), String> {
        self.node.ble_sent(op, ok);
        Ok(())
    }

    /// The radio stopped; every Bluetooth encounter is over.
    ///
    /// # Errors
    ///
    /// Never.
    pub fn ble_stopped(&self) -> Result<(), String> {
        self.node.ble_stopped();
        Ok(())
    }

    /// Backup state (no secret material).
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn backup_status(&self) -> Result<BackupStatus, String> {
        self.with(|rt| BackupStatus {
            enabled: rt.agent().backups_enabled(),
            form: match rt.agent().phrase_form() {
                mem_core::PhraseForm::English => PhraseFormItem::English,
                mem_core::PhraseForm::Proquint => PhraseFormItem::Syllables,
            },
        })
    }

    /// Draws a new recovery phrase to show with [`MemNode::backup_phrase_show`]. Nothing changes
    /// until [`MemNode::confirm_recovery_phrase`]: backups stay off, or keep the old phrase.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    pub fn new_recovery_phrase(&self, form: PhraseFormItem) -> Result<(), String> {
        self.handle(Input::NewRecoveryPhrase { form: form.into() })
            .map(|_| ())
    }

    /// The user typed back the new phrase: it becomes the recovery phrase and backups are on.
    ///
    /// # Errors
    ///
    /// `no_pending_phrase`, or a storage error.
    pub fn confirm_recovery_phrase(&self) -> Result<(), String> {
        let out = self.handle(Input::ConfirmRecoveryPhrase)?;
        rejected(&out).map_or(Ok(()), |r| Err(reason_key(r)))
    }

    /// The user left without confirming: the new phrase is forgotten.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    pub fn discard_recovery_phrase(&self) -> Result<(), String> {
        self.handle(Input::DiscardRecoveryPhrase).map(|_| ())
    }

    /// Changes how the phrase is written; the secret and existing backups stay valid.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_phrase_form(&self, form: PhraseFormItem) -> Result<(), String> {
        self.handle(Input::SetPhraseForm { form: form.into() })
            .map(|_| ())
    }

    /// The recovery phrase words, to show on the capture-protected phrase screen only.
    ///
    /// # Security
    ///
    /// One of the two functions allowed to carry the phrase across FFI.
    /// Never log, store or copy the result.
    ///
    /// # Errors
    ///
    /// If backups are off.
    pub fn backup_phrase_show(&self) -> Result<Vec<String>, String> {
        self.with(|rt| rt.agent().recovery_phrase().map(|w| w.to_vec()))?
            .ok_or_else(|| "backups are not turned on".into())
    }

    /// An encrypted backup file (`.membackup`). Slow (Argon2id).
    ///
    /// # Errors
    ///
    /// If backups are off.
    pub fn make_backup(&self, history: bool) -> Result<Vec<u8>, String> {
        self.node.make_backup(history).map_err(|e| e.to_string())
    }

    /// The same backup as `membackup1:` text, for the share sheet.
    ///
    /// # Errors
    ///
    /// If backups are off.
    pub fn backup_text(&self, history: bool) -> Result<String, String> {
        Ok(Runtime::backup_text(&self.make_backup(history)?))
    }

    /// Whether this is the main device.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn is_main_device(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().is_main_device())
    }

    /// Starts confirming in person from a relying party's request text. The app
    /// shows [`AttestRequestView::disclosure`] before anything else happens.
    ///
    /// # Errors
    ///
    /// `"main_device_only"`, `"wrong_object"` (not a request) or `"invalid"`.
    pub fn start_attestation(&self, text: String, locale: String) -> Result<AttestFlow, String> {
        let session = self.node.start_attestation(&text).map_err(|e| match e {
            mem_runtime::RuntimeError::Attest(a) => attest_error(&a),
            other => other.to_string(),
        })?;
        let r = session.request();
        let view = AttestRequestView {
            origin: r.rp_origin.clone(),
            label: r.subject_label.clone(),
            subject_short: mem_proto::attest::subject_short(&r.subject),
            role: r.role.clone(),
            submit_url: r.submit_url.clone(),
            disclosure: disclosure::attestation(
                &r.rp_origin,
                &r.subject_label,
                parse_locale(&locale),
            ),
        };
        Ok(AttestFlow {
            session: Mutex::new(Some(session)),
            view,
        })
    }

    /// Records how an attestation went back, for the attestation log.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn record_attestation(
        &self,
        origin: String,
        label: String,
        sent: bool,
    ) -> Result<(), String> {
        self.handle(Input::AttestationReturned(
            mem_core::model::AttestLogEntry {
                at_unix_ms: 0,
                rp_origin: origin,
                subject_label: label,
                returned: if sent {
                    mem_core::model::AttestReturn::Sent
                } else {
                    mem_core::model::AttestReturn::Copied
                },
            },
        ))
        .map(|_| ())
    }

    /// Attestations made on this device, newest first.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn attest_log(&self) -> Result<Vec<AttestLogItem>, String> {
        self.with(|rt| {
            rt.agent()
                .attest_log()
                .into_iter()
                .map(|e| AttestLogItem {
                    at_unix_ms: ms(e.at_unix_ms),
                    origin: e.rp_origin,
                    label: e.subject_label,
                    sent: e.returned == mem_core::model::AttestReturn::Sent,
                })
                .collect()
        })
    }

    /// Our devices, the main one first.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn own_devices(&self) -> Result<Vec<DeviceItem>, String> {
        self.with(|rt| {
            rt.agent()
                .own_devices()
                .into_iter()
                .map(|d| DeviceItem {
                    id: hex::encode(d.id),
                    main: d.main,
                    this_device: d.this_device,
                    joined: d.joined,
                })
                .collect()
        })
    }

    /// Main device: adds the device whose request was scanned in person, after the user
    /// confirmed; returns its setup as an animated QR (frame kind 3), or `None` if it is too
    /// large for one transfer.
    ///
    /// # Errors
    ///
    /// `"main_device_only"`, `"too_many_devices"`, `"device_known"`, `"bad_signature"`,
    /// `"malformed"`, or a storage error.
    pub fn approve_device(&self, request: Vec<u8>) -> Result<Option<QrAnimation>, String> {
        let out = self.handle(Input::ApproveDevice { request })?;
        if let Some(r) = rejected(&out) {
            return Err(reason_key(r));
        }
        Ok(out.iter().find_map(|o| match o {
            Output::DeviceSetupReady { setup } => QrAnimation::new(ObjectKind::DeviceSetup, setup),
            _ => None,
        }))
    }

    /// Main device: removes one of our other devices (hex id).
    ///
    /// # Errors
    ///
    /// `"main_device_only"`, `"unknown_device"`, `"malformed"`, or a storage error.
    pub fn retire_device(&self, id: String) -> Result<(), String> {
        let device: [u8; 8] = hex::decode(&id)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or("malformed")?;
        let out = self.handle(Input::RetireDevice { device })?;
        rejected(&out).map_or(Ok(()), |r| Err(reason_key(r)))
    }

    /// A fresh profile becomes another device of the user's MEM: returns the request to show
    /// as a QR code (`memdevice1:` text) and waits for the setup.
    ///
    /// # Errors
    ///
    /// `"not_empty"` if this profile has contacts, or a storage error.
    pub fn start_device_join(&self) -> Result<String, String> {
        let join = self.node.device_join().map_err(|e| match e {
            mem_runtime::RuntimeError::NotEmpty => String::from("not_empty"),
            other => other.to_string(),
        })?;
        let text = join.request_text();
        *self.join.lock().unwrap_or_else(PoisonError::into_inner) = Some(join);
        Ok(text)
    }

    /// Finishes joining with the setup the main device showed.
    ///
    /// # Errors
    ///
    /// `"no_join"` without [`MemNode::start_device_join`], `"not_for_this_device"` for a setup
    /// meant for another request (the request stays open), `"not_empty"`, or a storage error.
    pub fn finish_device_join(&self, setup: Vec<u8>) -> Result<(), String> {
        let mut slot = self.join.lock().unwrap_or_else(PoisonError::into_inner);
        let join = slot.as_ref().ok_or("no_join")?;
        // A wrong code keeps the request open; only success ends it.
        match self.node.join_device(join, &setup) {
            Ok(()) => {
                *slot = None;
                Ok(())
            }
            Err(mem_runtime::RuntimeError::Join(_)) => Err("not_for_this_device".into()),
            Err(mem_runtime::RuntimeError::NotEmpty) => Err("not_empty".into()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Stops waiting to be added as a device.
    #[frb(sync)]
    pub fn cancel_device_join(&self) {
        *self.join.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// After [`NodeEventItem::DeviceRetired`]: replaces this device's state with a fresh identity.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn forget_this_device(&self) -> Result<(), String> {
        self.node.forget_this_device().map_err(|e| e.to_string())
    }

    /// The backup as an animated QR (frame kind 2) for a new phone. `None` if it exceeds the
    /// 64 KiB frame limit; offer "without history" then.
    ///
    /// # Errors
    ///
    /// If backups are off.
    pub fn backup_animation(&self, history: bool) -> Result<Option<QrAnimation>, String> {
        let file = self.make_backup(history)?;
        Ok(QrAnimation::new(ObjectKind::Backup, &file))
    }

    /// Restores a backup (file bytes or `membackup1:` text) with the typed phrase, replacing
    /// this profile's state. The previous state is kept, encrypted, in the profile directory.
    ///
    /// # Security
    ///
    /// The second function allowed to carry phrase words across FFI.
    ///
    /// # Errors
    ///
    /// A wrong phrase, a damaged file, or a storage error.
    pub fn restore_with_phrase(
        &self,
        data: Vec<u8>,
        words: String,
    ) -> Result<RestoreResult, String> {
        let words = zeroize::Zeroizing::new(words);
        let summary = self
            .node
            .restore_backup(&data, &words)
            .map_err(|e| e.to_string())?;
        Ok(RestoreResult {
            created_unix_ms: i64::try_from(summary.created_unix_ms).unwrap_or(i64::MAX),
            history: summary.history,
            corrected_phrase: summary.corrected.as_ref().map(|c| c.to_string()),
            replaced: summary.replaced,
        })
    }

    /// Courier mode status.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn courier_status(&self) -> Result<CourierStatus, String> {
        self.with(|rt| {
            let (used, limit, count) = rt.agent().courier_status();
            CourierStatus {
                used_bytes: used,
                limit_bytes: limit,
                max_bytes: mem_core::courier::MAX_COURIER_BYTES,
                default_bytes: mem_core::courier::DEFAULT_COURIER_BYTES,
                bundles: u32::try_from(count).unwrap_or(u32::MAX),
            }
        })
    }

    /// Sets the courier storage limit; 0 turns courier mode off. Carried bundles beyond a lower
    /// limit are dropped.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_courier_limit(&self, bytes: u64) -> Result<(), String> {
        self.handle(Input::SetCourierLimit { bytes }).map(|_| ())
    }

    /// How many days an unconfirmed message is retried (its lifetime), and the allowed range.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn retry_days(&self) -> Result<RetryDays, String> {
        self.with(|rt| RetryDays {
            current: rt.agent().message_lifetime_s() / DAY_S,
            min: mem_core::MIN_MESSAGE_LIFETIME_S / DAY_S,
            max: mem_core::MESSAGE_LIFETIME_S / DAY_S,
        })
    }

    /// Sets how many days new messages are retried; clamped to the allowed range by the core.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_retry_days(&self, days: u32) -> Result<(), String> {
        self.handle(Input::SetMessageLifetime {
            seconds: days.saturating_mul(DAY_S),
        })
        .map(|_| ())
    }

    /// Hex ids of contacts reachable right now over the local network.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn nearby(&self) -> Result<Vec<String>, String> {
        self.with(|rt| {
            rt.agent()
                .reachable_contacts()
                .into_iter()
                .map(|(c, _)| hex::encode(c.as_bytes()))
                .collect()
        })
    }

    /// Number of MEM devices connected on the local network (contacts or not).
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn nearby_devices(&self) -> Result<u32, String> {
        self.with(|rt| u32::try_from(rt.agent().peer_count()).unwrap_or(u32::MAX))
    }

    /// Every channel with its exposure profile ("Channels").
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn channels(&self, locale: String) -> Result<Vec<ChannelInfo>, String> {
        let locale = parse_locale(&locale);
        self.with(|rt| {
            ClaId::ALL
                .into_iter()
                .map(|cla| {
                    let exposure = rt.agent().channel_exposure(cla);
                    let d = disclosure::render(cla, cla.grade(), cla.grade(), &exposure, locale);
                    ChannelInfo {
                        channel: cla.into(),
                        name: disclosure::channel_name(cla, locale).into(),
                        grade: cla.grade().label().into(),
                        can_disable: matches!(
                            cla,
                            ClaId::Iroh
                                | ClaId::Ble
                                | ClaId::Hub
                                | ClaId::Email
                                | ClaId::WifiDirect
                        ),
                        enabled: rt.agent().channel_enabled(cla),
                        observers: d.observers,
                        exposed: d.exposed,
                        limited: d.limited,
                        protected: d.protected,
                    }
                })
                .collect()
        })
    }

    /// Our fingerprint and name.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn profile(&self) -> Result<ProfileView, String> {
        self.with(|rt| ProfileView {
            fingerprint: rt.agent().fingerprint(),
            display_name: rt.agent().display_name().map(String::from),
        })
    }

    /// Sets (or clears, with `None`/empty) our display name.
    ///
    /// # Errors
    ///
    /// `"invalid_name"` for a name over 64 bytes, or a storage error.
    pub fn set_display_name(&self, name: Option<String>) -> Result<(), String> {
        let out = self.handle(Input::SetDisplayName(name))?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// Our pairing card as text and QR.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn own_card(&self) -> Result<CardView, String> {
        self.with(|rt| {
            let text = rt.own_card_text();
            let qr = qr(&text);
            CardView { text, qr }
        })
    }

    /// Asks the owner of a card (`memcard1:` text, pasted or scanned) to become a contact,
    /// without meeting. Returns the new contact's hex id; the request is sealed after
    /// a few seconds of proof of work and then sent like a message.
    ///
    /// # Errors
    ///
    /// `"not_a_card"`, `"own_card"`, `"already_contact"`, `"invalid_note"`, `"bad_signature"`,
    /// or a storage error.
    pub fn send_contact_request(
        &self,
        card_text: String,
        note: Option<String>,
    ) -> Result<String, String> {
        let Ok(mem_proto::armor::Armored::Card(card)) = mem_proto::armor::dearmor(&card_text)
        else {
            return Err("not_a_card".into());
        };
        let out = self.handle(Input::SendContactRequest { card, note })?;
        if let Some(r) = rejected(&out) {
            return Err(reason_key(r));
        }
        out.iter()
            .find_map(|o| match o {
                Output::ContactPaired { contact, .. } => Some(hex::encode(contact.as_bytes())),
                _ => None,
            })
            .ok_or_else(|| String::from("not_queued"))
    }

    /// Contact requests waiting for an answer, oldest first.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn requests(&self) -> Result<Vec<RequestItem>, String> {
        self.with(|rt| {
            rt.agent()
                .requests()
                .into_iter()
                .map(|r| RequestItem {
                    id: r.id.0,
                    fingerprint: r.fingerprint,
                    display_name: r.display_name,
                    note: r.note,
                    received_unix_ms: ms(r.received_unix_ms),
                    via: ChannelUseItem {
                        channel: r.via.cla.into(),
                        grade: r.via.grade.label().into(),
                        at_unix_ms: ms(r.via.at_unix_ms),
                    },
                })
                .collect()
        })
    }

    /// Accepts a contact request; returns the new contact's hex id.
    ///
    /// # Errors
    ///
    /// `"unknown_request"` or a storage error.
    pub fn approve_request(&self, id: u64) -> Result<String, String> {
        let out = self.handle(Input::ApproveRequest(RequestId(id)))?;
        if let Some(r) = rejected(&out) {
            return Err(reason_key(r));
        }
        out.iter()
            .find_map(|o| match o {
                Output::ContactPaired { contact, .. } => Some(hex::encode(contact.as_bytes())),
                _ => None,
            })
            .ok_or_else(|| String::from("unknown_request"))
    }

    /// Deletes a contact request; with `block`, that sender's requests are ignored from now on.
    ///
    /// # Errors
    ///
    /// `"unknown_request"` or a storage error.
    pub fn decline_request(&self, id: u64, block: bool) -> Result<(), String> {
        let input = if block {
            Input::BlockRequest(RequestId(id))
        } else {
            Input::RejectRequest(RequestId(id))
        };
        let out = self.handle(input)?;
        rejected(&out).map_or(Ok(()), |r| Err(reason_key(r)))
    }

    /// Whether nearby devices may deliver contact requests (our intro tag is advertised).
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn accepts_requests_nearby(&self) -> Result<bool, String> {
        self.with(|rt| rt.agent().accepts_requests_nearby())
    }

    /// Turns "Accept contact requests through nearby devices" on or off.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn set_accepts_requests_nearby(&self, enabled: bool) -> Result<(), String> {
        self.handle(Input::SetAcceptRequestsNearby { enabled })
            .map(|_| ())
    }

    /// Generates a fresh pairing code (new nonce).
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn new_pairing_code(&self) -> Result<(), String> {
        self.handle(Input::NewPairingCode).map(|_| ())
    }

    /// Feeds text scanned from a QR (`channel = Qr`) or pasted / shared in (`Share`).
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn receive_text(&self, text: String, channel: Channel) -> Result<ReceiveOutcome, String> {
        let incoming = self
            .node
            .receive_text(&text, channel.into())
            .map_err(|e| e.to_string())?;
        match incoming {
            Incoming::NotMem => Ok(ReceiveOutcome::NotMem),
            Incoming::Bundle(out) | Incoming::Card(out) => self.receive_outcome(&out),
        }
    }

    /// A contact code checked but not imported (codes shared by another app are
    /// confirmed by the user first). `None` if `text` is not a valid contact code.
    #[frb(sync)]
    #[must_use]
    pub fn card_preview(&self, text: String) -> Option<CardPreviewItem> {
        let Ok(mem_proto::armor::Armored::Card(bytes)) = mem_proto::armor::dearmor(text.trim())
        else {
            return None;
        };
        self.with(|rt| rt.agent().card_preview(&bytes).ok())
            .ok()
            .flatten()
            .map(|p| CardPreviewItem {
                fingerprint: p.fingerprint,
                display_name: p.display_name,
                known: p.known,
            })
    }

    fn receive_outcome(&self, out: &[Output]) -> Result<ReceiveOutcome, String> {
        if let Some(r) = rejected(out) {
            return Ok(ReceiveOutcome::Rejected {
                reason: reason_key(r),
            });
        }
        if out
            .iter()
            .any(|o| matches!(o, Output::ContactRequest { .. }))
        {
            return Ok(ReceiveOutcome::Request);
        }
        let found = out.iter().find_map(|o| match o {
            Output::Deliver { contact, .. } => Some((*contact, None)),
            Output::ContactPaired {
                contact,
                updated,
                unchanged,
            } => Some((*contact, Some((*updated, *unchanged)))),
            _ => None,
        });
        Ok(match found {
            Some((contact, None)) => ReceiveOutcome::Message {
                contact_id: hex::encode(contact.as_bytes()),
            },
            Some((contact, Some((updated, unchanged)))) => {
                let sas = self
                    .with(|rt| rt.agent().contact(contact).map(|c| sas_emoji(&c.sas)))?
                    .unwrap_or_default();
                ReceiveOutcome::Contact {
                    contact_id: hex::encode(contact.as_bytes()),
                    updated,
                    unchanged,
                    sas,
                }
            }
            None => ReceiveOutcome::NotMem,
        })
    }

    /// All contacts, most recent conversation first.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn contacts(&self) -> Result<Vec<ContactItem>, String> {
        self.with(|rt| {
            let agent = rt.agent();
            let mut items: Vec<ContactItem> = agent
                .contacts()
                .into_iter()
                .map(|c| {
                    let last = agent.messages(c.id).into_iter().last();
                    ContactItem {
                        id: hex::encode(c.id.as_bytes()),
                        display_name: c.display_name.clone(),
                        fingerprint: c.fingerprint.clone(),
                        sas: sas_emoji(&c.sas),
                        met_in_person: c.trust == TrustLevel::MetInPerson,
                        requested: c.requested,
                        floor: grade_u8(c.floor),
                        last_unix_ms: ms(last
                            .as_ref()
                            .map_or(c.added_unix_ms, |m| m.created_unix_ms)),
                        last_text: last.map(|m| m.text),
                        email: c.email.clone(),
                    }
                })
                .collect();
            items.sort_by(|a, b| b.last_unix_ms.cmp(&a.last_unix_ms));
            items
        })
    }

    /// Messages with a contact, oldest first.
    ///
    /// # Errors
    ///
    /// `"unknown_contact"` for a bad id.
    #[frb(sync)]
    pub fn messages(&self, contact_id: String) -> Result<Vec<MessageItem>, String> {
        let id = contact(&contact_id)?;
        self.with(|rt| {
            rt.agent()
                .messages(id)
                .into_iter()
                .map(message_item)
                .collect()
        })
    }

    // ---------------------------------------------------------------- groups

    /// Groups: member, invited, joining, or past ones, newest first.
    ///
    /// # Errors
    ///
    /// Never.
    #[frb(sync)]
    pub fn groups(&self) -> Result<Vec<GroupItem>, String> {
        self.with(|rt| rt.agent().groups().into_iter().map(group_item).collect())
    }

    /// One group.
    ///
    /// # Errors
    ///
    /// `"unknown_group"`.
    #[frb(sync)]
    pub fn group(&self, group_id: String) -> Result<GroupItem, String> {
        let id = parse_group(&group_id)?;
        self.with(|rt| rt.agent().group(id).map(group_item))?
            .ok_or_else(|| String::from("unknown_group"))
    }

    /// A group's messages, oldest first; ours show once, with how many members confirmed.
    ///
    /// # Errors
    ///
    /// `"unknown_group"`.
    #[frb(sync)]
    pub fn group_messages(&self, group_id: String) -> Result<Vec<MessageItem>, String> {
        let id = parse_group(&group_id)?;
        self.with(|rt| {
            rt.agent()
                .group_messages(id)
                .into_iter()
                .map(message_item)
                .collect()
        })
    }

    fn group_call(&self, input: Input) -> Result<(), String> {
        let out = self.handle(input)?;
        match rejected(&out) {
            Some(r) => Err(reason_key(r)),
            None => Ok(()),
        }
    }

    /// Creates a group with us as admin and invites these contacts (at most 15). Returns its id.
    ///
    /// # Errors
    ///
    /// `"invalid_name"`, `"unknown_contact"`, `"group_full"`, or a storage error.
    pub fn create_group(&self, name: String, member_ids: Vec<String>) -> Result<String, String> {
        let members = member_ids
            .iter()
            .map(|m| contact(m))
            .collect::<Result<Vec<_>, _>>()?;
        let out = self.handle(Input::CreateGroup { name, members })?;
        if let Some(r) = rejected(&out) {
            return Err(reason_key(r));
        }
        out.iter()
            .find_map(|o| match o {
                Output::GroupChanged { group } => Some(hex::encode(group.as_bytes())),
                _ => None,
            })
            .ok_or_else(|| String::from("unknown_group"))
    }

    /// Admin: invites a contact.
    ///
    /// # Errors
    ///
    /// A reason key.
    pub fn invite_to_group(&self, group_id: String, contact_id: String) -> Result<(), String> {
        self.group_call(Input::InviteToGroup {
            group: parse_group(&group_id)?,
            contact: contact(&contact_id)?,
        })
    }

    /// Admin: removes a member or withdraws an invitation.
    ///
    /// # Errors
    ///
    /// A reason key.
    pub fn remove_from_group(&self, group_id: String, member_id: String) -> Result<(), String> {
        self.group_call(Input::RemoveFromGroup {
            group: parse_group(&group_id)?,
            member: contact(&member_id)?,
        })
    }

    /// Joins (`accept`) or declines an invitation.
    ///
    /// # Errors
    ///
    /// A reason key.
    pub fn answer_group_invitation(&self, group_id: String, accept: bool) -> Result<(), String> {
        let group = parse_group(&group_id)?;
        self.group_call(if accept {
            Input::AcceptGroupInvitation(group)
        } else {
            Input::DeclineGroupInvitation(group)
        })
    }

    /// Leaves a group (the admin ends it for everyone).
    ///
    /// # Errors
    ///
    /// A reason key.
    pub fn leave_group(&self, group_id: String) -> Result<(), String> {
        self.group_call(Input::LeaveGroup(parse_group(&group_id)?))
    }

    /// Sends a text to every other member; each copy goes out on its own, automatically.
    ///
    /// # Errors
    ///
    /// `"invalid_text"`, `"unknown_group"`, `"group_empty"`, or a storage error.
    pub fn send_group_text(&self, group_id: String, text: String) -> Result<(), String> {
        self.group_call(Input::SendGroupText {
            group: parse_group(&group_id)?,
            text,
        })
    }

    /// Makes a fellow group member one of our contacts (their 1:1 messages then reach us once
    /// they do the same).
    ///
    /// # Errors
    ///
    /// `"already_contact"`, `"unknown_contact"`.
    pub fn add_member_as_contact(&self, contact_id: String) -> Result<(), String> {
        self.group_call(Input::AddMemberAsContact(contact(&contact_id)?))
    }

    /// Seals a text message for a contact and queues it. If the contact is nearby on the local
    /// network (and that is within the floor), it is sent right away.
    ///
    /// # Errors
    ///
    /// `"invalid_text"`, `"unknown_contact"`, or a storage error.
    pub fn send_text(&self, contact_id: String, text: String) -> Result<SentText, String> {
        let contact = contact(&contact_id)?;
        let out = self.handle(Input::UserSend { contact, text })?;
        if let Some(r) = rejected(&out) {
            return Err(reason_key(r));
        }
        let message_id = out
            .iter()
            .find_map(|o| match o {
                Output::Queued { message, .. } => Some(hex::encode(message.as_bytes())),
                _ => None,
            })
            .ok_or_else(|| String::from("not_queued"))?;
        Ok(SentText {
            message_id,
            sending_nearby: out
                .iter()
                .any(|o| matches!(o, Output::Transmit { peer: Some(_), .. })),
        })
    }

    /// Channels available for a queued message.
    ///
    /// # Errors
    ///
    /// `"unknown_message"` for a bad id.
    #[frb(sync)]
    pub fn channel_options(
        &self,
        message_id: String,
        locale: String,
    ) -> Result<Vec<ChannelOptionItem>, String> {
        let message = msg(&message_id)?;
        let locale = parse_locale(&locale);
        self.with(|rt| {
            rt.agent()
                .channel_options(message)
                .into_iter()
                .map(|o| ChannelOptionItem {
                    channel: o.cla.into(),
                    name: disclosure::channel_name(o.cla, locale).into(),
                    grade: o.grade.label().into(),
                    fits: o.fits,
                    needs_consent: o.needs_consent,
                    reachable: o.reachable,
                })
                .collect()
        })
    }

    /// Asks to send a queued message over `channel`. Below-floor channels return
    /// [`TransmitOutcome::ConsentRequired`] instead of data.
    ///
    /// # Errors
    ///
    /// Bad id or storage error.
    pub fn transmit(
        &self,
        message_id: String,
        channel: Channel,
        locale: String,
    ) -> Result<TransmitOutcome, String> {
        let message = msg(&message_id)?;
        let out = self.handle(Input::RequestTransmit {
            message,
            cla: channel.into(),
        })?;
        Ok(outcome(&out, parse_locale(&locale)))
    }

    /// Answers a consent request. A rule for this contact lasts
    /// `rule_days` (1, 7 or 30) days, or until revoked when `None`.
    ///
    /// # Errors
    ///
    /// Bad id, `"bad_duration"`, or a storage error.
    pub fn give_consent(
        &self,
        request: ConsentRequestRef,
        choice: ConsentChoice,
        rule_days: Option<u32>,
        locale: String,
    ) -> Result<TransmitOutcome, String> {
        let until_unix_ms = match rule_days {
            None => None,
            Some(days @ (1 | 7 | 30)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "clock")?;
                let now = u64::try_from(now.as_millis()).map_err(|_| "clock")?;
                Some(now + u64::from(days) * 86_400_000)
            }
            Some(_) => return Err("bad_duration".into()),
        };
        let record = ConsentRecord {
            offer: OfferId(request.offer_id),
            message: msg(&request.message_id)?,
            cla: request.channel.into(),
            grade: grade_from(request.grade).ok_or("bad_grade")?,
            scope: match choice {
                ConsentChoice::Once => ConsentScope::Once,
                ConsentChoice::ThisContact => ConsentScope::ThisContact { until_unix_ms },
            },
        };
        let out = self.handle(Input::ConsentGiven(record))?;
        Ok(outcome(&out, parse_locale(&locale)))
    }

    /// Reports whether the user completed a transmission (`done`) or backed out.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn transmit_done(&self, op: u64, done: bool) -> Result<(), String> {
        let result = if done {
            Ok(())
        } else {
            Err(ClaError::Cancelled)
        };
        self.handle(Input::TransmitResult {
            op: OpId(op),
            result,
        })
        .map(|_| ())
    }

    /// Changes the floor given to contacts added from now on (0–4 for P0–P4; P5 is refused).
    /// Existing contacts keep theirs.
    ///
    /// # Errors
    ///
    /// `bad_grade`, `invalid_floor`, or a storage error.
    pub fn set_default_floor(&self, floor: u8) -> Result<(), String> {
        let floor = grade_from(floor).ok_or("bad_grade")?;
        let out = self.handle(Input::SetDefaultFloor { floor })?;
        rejected(&out).map_or(Ok(()), |r| Err(reason_key(r)))
    }

    /// The floor given to contacts added from now on (0–4 for P0–P4).
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn default_floor(&self) -> Result<u8, String> {
        self.with(|rt| grade_u8(rt.agent().default_floor()))
    }

    /// Changes one contact's privacy floor (0–4 for P0–P4; P5 is refused).
    ///
    /// # Errors
    ///
    /// `bad_grade`, `invalid_floor`, `unknown_contact`, or a storage error.
    pub fn set_floor(&self, contact_id: String, floor: u8) -> Result<(), String> {
        let contact = contact(&contact_id)?;
        let floor = grade_from(floor).ok_or("bad_grade")?;
        let out = self.handle(Input::SetFloor { contact, floor })?;
        rejected(&out).map_or(Ok(()), |r| Err(reason_key(r)))
    }

    /// The privacy log, newest first.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn privacy_log(&self) -> Result<Vec<PrivacyLogItem>, String> {
        self.with(|rt| {
            let agent = rt.agent();
            agent
                .privacy_log()
                .into_iter()
                .map(|e| PrivacyLogItem {
                    at_unix_ms: ms(e.at_unix_ms),
                    contact: contact_name(agent, e.contact),
                    channel: e.cla.into(),
                    grade: e.grade.label().into(),
                    floor: e.floor.label().into(),
                    basis: match e.basis {
                        ConsentBasis::Once => "once",
                        ConsentBasis::NewRule(_) => "new_rule",
                        ConsentBasis::ExistingRule(_) => "rule",
                    }
                    .into(),
                })
                .collect()
        })
    }

    /// Standing consent rules.
    ///
    /// # Errors
    ///
    /// Only if the node lock is poisoned.
    #[frb(sync)]
    pub fn rules(&self) -> Result<Vec<RuleItem>, String> {
        self.with(|rt| {
            let agent = rt.agent();
            agent
                .rules()
                .into_iter()
                .map(|r| RuleItem {
                    id: r.id.0,
                    contact: r.contact.map(|c| contact_name(agent, c)),
                    contact_id: r.contact.map(|c| hex::encode(c.as_bytes())),
                    channel: r.cla.into(),
                    max_grade: r.max_grade.label().into(),
                    until_unix_ms: r.until_unix_ms,
                })
                .collect()
        })
    }

    /// Revokes a standing rule.
    ///
    /// # Errors
    ///
    /// A storage error.
    pub fn revoke_rule(&self, id: u64) -> Result<(), String> {
        self.handle(Input::RevokeRule(RuleId(id))).map(|_| ())
    }
}

/// The parts of a [`ConsentRequest`] the core verifies when the user answers.
pub struct ConsentRequestRef {
    /// Offer id.
    pub offer_id: u64,
    /// Message hex id.
    pub message_id: String,
    /// Channel.
    pub channel: Channel,
    /// Grade the user was shown, 0..=5.
    pub grade: u8,
}

fn outcome(out: &[Output], locale: Locale) -> TransmitOutcome {
    for o in out {
        match o {
            // Network channels: the runtime carries it; the status updates by itself.
            Output::Transmit {
                peer: Some(_), cla, ..
            }
            | Output::Transmit {
                cla: cla @ ClaId::Hub,
                ..
            } => {
                return TransmitOutcome::Sending {
                    channel: (*cla).into(),
                };
            }
            Output::Transmit { op, cla, bytes, .. } => {
                let animation = if *cla == ClaId::Qr {
                    QrAnimation::new(ObjectKind::Bundle, bytes)
                } else {
                    None
                };
                return TransmitOutcome::Ready {
                    op: op.0,
                    channel: (*cla).into(),
                    text: Runtime::armor(bytes),
                    animation,
                };
            }
            Output::CourierQueued { .. } => return TransmitOutcome::CourierQueued,
            // The runtime mails it; the status updates by itself.
            Output::EmailSend { .. } => {
                return TransmitOutcome::Sending {
                    channel: Channel::Email,
                };
            }
            Output::PlaintextReady { op, text, .. } => {
                return TransmitOutcome::PlainReady {
                    op: op.0,
                    text: text.clone(),
                };
            }
            Output::ConsentRequired(offer) => {
                return TransmitOutcome::ConsentRequired(consent_request(offer, locale));
            }
            Output::Rejected(r) => {
                return TransmitOutcome::Rejected {
                    reason: reason_key(*r),
                };
            }
            _ => {}
        }
    }
    TransmitOutcome::Rejected {
        reason: "unknown".into(),
    }
}

fn consent_request(offer: &ChannelOffer, locale: Locale) -> ConsentRequest {
    let d = disclosure::render(offer.cla, offer.grade, offer.floor, &offer.exposure, locale);
    ConsentRequest {
        offer_id: offer.id.0,
        message_id: hex::encode(offer.message.as_bytes()),
        channel: offer.cla.into(),
        grade: grade_u8(offer.grade),
        title: d.title,
        observers: d.observers,
        exposed: d.exposed,
        limited: d.limited,
        protected: d.protected,
    }
}

fn qr(text: &str) -> Option<QrView> {
    cla_qr::render_text(text).map(QrView::from)
}

fn rejected(out: &[Output]) -> Option<RejectReason> {
    out.iter().find_map(|o| match o {
        Output::Rejected(r) => Some(*r),
        _ => None,
    })
}

fn reason_key(r: RejectReason) -> String {
    match r {
        RejectReason::Malformed => "malformed",
        RejectReason::Unsupported => "unsupported",
        RejectReason::NotForUs => "not_for_us",
        RejectReason::Undecryptable => "undecryptable",
        RejectReason::BadSignature => "bad_signature",
        RejectReason::Duplicate => "duplicate",
        RejectReason::Purged => "purged",
        RejectReason::Expired => "expired",
        RejectReason::OwnCard => "own_card",
        RejectReason::MainDeviceOnly => "main_device_only",
        RejectReason::TooManyDevices => "too_many_devices",
        RejectReason::DeviceKnown => "device_known",
        RejectReason::UnknownDevice => "unknown_device",
        RejectReason::UnknownContact => "unknown_contact",
        RejectReason::UnknownMessage => "unknown_message",
        RejectReason::TooLargeForChannel => "too_large_for_channel",
        RejectReason::ConsentMismatch => "consent_mismatch",
        RejectReason::InvalidText => "invalid_text",
        RejectReason::InvalidName => "invalid_name",
        RejectReason::Unreachable => "unreachable",
        RejectReason::ChannelDisabled => "channel_disabled",
        RejectReason::Quota => "quota",
        RejectReason::InvalidFloor => "invalid_floor",
        RejectReason::NoPendingPhrase => "no_pending_phrase",
        RejectReason::WaitingForKey => "waiting_for_key",
        RejectReason::AwaitingApproval => "awaiting_approval",
        RejectReason::BadProofOfWork => "bad_proof_of_work",
        RejectReason::UnknownRequest => "unknown_request",
        RejectReason::AlreadyContact => "already_contact",
        RejectReason::InvalidNote => "invalid_note",
        RejectReason::UnknownGroup => "unknown_group",
        RejectReason::NotGroupAdmin => "not_group_admin",
        RejectReason::GroupFull => "group_full",
        RejectReason::GroupEmpty => "group_empty",
        RejectReason::NotAContact => "not_a_contact",
        RejectReason::InvalidHubUrl => "invalid_hub_url",
        RejectReason::InvalidPushEndpoint => "invalid_push_endpoint",
        RejectReason::NotPlainText => "not_plain_text",
        RejectReason::InvalidEmailAccount => "invalid_email_account",
        RejectReason::InvalidEmailAddress => "invalid_email_address",
    }
    .into()
}

fn contact(hex_id: &str) -> Result<ContactId, String> {
    let bytes: [u8; 32] = hex::decode(hex_id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("unknown_contact")?;
    Ok(ContactId::from_bytes(bytes))
}

fn parse_group(hex_id: &str) -> Result<GroupId, String> {
    let bytes: [u8; 16] = hex::decode(hex_id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("unknown_group")?;
    Ok(GroupId::from_bytes(bytes))
}

fn message_item(m: MessageView) -> MessageItem {
    MessageItem {
        id: hex::encode(m.id.as_bytes()),
        outgoing: m.direction == MsgDirection::Outgoing,
        is_request: m.kind == MessageKind::ContactRequest,
        text: m.text,
        created_unix_ms: ms(m.created_unix_ms),
        status: match m.status {
            DeliveryStatus::Queued => MessageStatus::Queued,
            DeliveryStatus::Sent => MessageStatus::Sent,
            DeliveryStatus::Delivered => MessageStatus::Delivered,
            DeliveryStatus::Failed => MessageStatus::Failed,
            DeliveryStatus::Received => MessageStatus::Received,
        },
        channels: m
            .channels
            .iter()
            .map(|u| ChannelUseItem {
                channel: u.cla.into(),
                grade: u.grade.label().into(),
                at_unix_ms: ms(u.at_unix_ms),
            })
            .collect(),
        sender_id: hex::encode(m.contact.as_bytes()),
        delivered_to: m.recipients.map(|(k, _)| k),
        sent_to: m.recipients.map(|(_, n)| n),
    }
}

fn group_item(g: GroupView) -> GroupItem {
    GroupItem {
        id: hex::encode(g.id.as_bytes()),
        name: g.name,
        status: match g.status {
            GroupStatus::Invited => GroupStatusItem::Invited,
            GroupStatus::Joining => GroupStatusItem::Joining,
            GroupStatus::Member => GroupStatusItem::Member,
            GroupStatus::Left => GroupStatusItem::Left,
            GroupStatus::Removed => GroupStatusItem::Removed,
        },
        is_admin: g.is_admin,
        members: g
            .members
            .into_iter()
            .map(|m| GroupMemberItem {
                contact_id: hex::encode(m.contact.as_bytes()),
                display_name: m.display_name,
                fingerprint: m.fingerprint,
                is_contact: m.is_contact,
                is_me: m.is_me,
                role: match m.role {
                    MemberRole::Admin => MemberRoleItem::Admin,
                    MemberRole::Member => MemberRoleItem::Member,
                    MemberRole::Invited => MemberRoleItem::Invited,
                },
            })
            .collect(),
        created_unix_ms: ms(g.created_unix_ms),
    }
}

fn msg(hex_id: &str) -> Result<MsgId, String> {
    let bytes: [u8; 16] = hex::decode(hex_id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("unknown_message")?;
    Ok(MsgId::from_bytes(bytes))
}

fn contact_name(agent: &mem_core::Agent, id: ContactId) -> String {
    agent
        .contact(id)
        .map(|c| c.display_name.unwrap_or(c.fingerprint))
        .unwrap_or_default()
}

fn parse_locale(s: &str) -> Locale {
    if s.starts_with("uk") {
        Locale::Uk
    } else {
        Locale::En
    }
}

const fn grade_u8(g: PrivacyGrade) -> u8 {
    match g {
        PrivacyGrade::P0 => 0,
        PrivacyGrade::P1 => 1,
        PrivacyGrade::P2 => 2,
        PrivacyGrade::P3 => 3,
        PrivacyGrade::P4 => 4,
        PrivacyGrade::P5 => 5,
    }
}

const fn grade_from(v: u8) -> Option<PrivacyGrade> {
    Some(match v {
        0 => PrivacyGrade::P0,
        1 => PrivacyGrade::P1,
        2 => PrivacyGrade::P2,
        3 => PrivacyGrade::P3,
        4 => PrivacyGrade::P4,
        5 => PrivacyGrade::P5,
        _ => return None,
    })
}

fn ms(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    /// A connect hint shows as one small QR code: one frame, version 6 at most, so
    /// a camera reads it from a normal distance; the padded animation frame was version 12.
    #[test]
    fn a_connect_hint_is_one_small_qr_code() {
        let hint = mem_proto::hint::ConnectHint {
            endpoint_id: [7; 32],
            addrs: vec![
                mem_proto::hint::HintAddr::new([192, 168, 0, 100], 46_059),
                mem_proto::hint::HintAddr::new([10, 0, 0, 2], 41_000),
            ],
        }
        .encode()
        .unwrap();
        let qr = QrAnimation::single(ObjectKind::ConnectHint, &hint).unwrap();
        assert_eq!(qr.animation.frames_needed(), 1);
        let matrix = qr.animation.frame(0).unwrap();
        // Version v has 17 + 4v modules a side.
        assert!(matrix.width <= 17 + 4 * 6, "{} modules", matrix.width);
        // Photographed (4 px a module, white margin) and scanned: the hint comes back.
        let w = matrix.width as usize;
        let side = (w + 8) * 4;
        let mut img = vec![235u8; side * side];
        for y in 0..w {
            for x in 0..w {
                if matrix.modules[y * w + x] == 1 {
                    for dy in 0..4 {
                        let row = ((y + 4) * 4 + dy) * side + (x + 4) * 4;
                        img[row..row + 4].fill(20);
                    }
                }
            }
        }
        let side = u32::try_from(side).unwrap();
        let event = cla_qr::Scanner::new().scan_luma(&img, side, side, side);
        assert_eq!(
            event,
            ScanEvent::Complete {
                kind: ObjectKind::ConnectHint,
                object: hint
            }
        );
    }
}
