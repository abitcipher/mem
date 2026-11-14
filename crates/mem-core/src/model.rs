//! Domain types shared by the agent, its inputs/outputs and the UI layer.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use mem_cla::{ClaId, ExposureProfile, PrivacyGrade};

/// A contact, identified locally by its identity public key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContactId(pub(crate) [u8; 32]);

impl ContactId {
    /// Raw bytes (the contact's identity public key).
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Wraps raw bytes, e.g. from the UI layer.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for ContactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContactId({:02x}{:02x}…)", self.0[0], self.0[1])
    }
}

/// A message, identified by its UUIDv7 `message_id`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MsgId(pub(crate) [u8; 16]);

impl MsgId {
    /// Raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Wraps raw bytes, e.g. from the UI layer.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for MsgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MsgId({:02x}{:02x}…)", self.0[14], self.0[15])
    }
}

/// Handle for one requested transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpId(pub u64);

/// Local handle for one connected peer during one encounter. Assigned by the
/// runtime; never derived from the peer's transport key or identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerHandle(pub u64);

/// Handle for one consent offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OfferId(pub u64);

/// Handle for one pending contact request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// A group: 16 random bytes chosen by its admin.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub(crate) [u8; 16]);

impl GroupId {
    /// Raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Wraps raw bytes, e.g. from the UI layer.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupId({:02x}{:02x}…)", self.0[0], self.0[1])
    }
}

/// Where this device stands in a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStatus {
    /// Invited; not answered yet.
    Invited,
    /// Accepted; waiting for the admin's state that lists us.
    Joining,
    /// A member.
    Member,
    /// We left (or, as admin, ended the group).
    Left,
    /// The admin removed us.
    Removed,
}

/// One member of a group, as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMemberView {
    /// Member (a contact id; the member may be group-only).
    pub contact: ContactId,
    /// Name from their card (unverified).
    pub display_name: Option<String>,
    /// The name the user gave them, if they are a contact.
    pub alias: Option<String>,
    /// Grouped base32 fingerprint.
    pub fingerprint: String,
    /// One of our contacts, not only a group member.
    pub is_contact: bool,
    /// This device.
    pub is_me: bool,
    /// Admin, member, or invited.
    pub role: MemberRole,
}

/// A member's place in a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberRole {
    /// The admin (creator).
    Admin,
    /// A member who joined.
    Member,
    /// Invited, not joined yet (shown to the admin only).
    Invited,
}

/// A group, as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupView {
    /// Group id.
    pub id: GroupId,
    /// Name.
    pub name: String,
    /// Our standing.
    pub status: GroupStatus,
    /// We are its admin.
    pub is_admin: bool,
    /// Current epoch.
    pub epoch: u32,
    /// Members (joined first, then invited).
    pub members: Vec<GroupMemberView>,
    /// When we created it or were invited, Unix ms.
    pub created_unix_ms: u64,
}

/// Handle for one standing consent rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(pub u64);

/// How a contact was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// A fellow group member introduced by the group's admin; group-only.
    Introduced,
    /// Cards exchanged remotely through another app.
    Remote,
    /// Cards exchanged by scanning QR codes in person.
    MetInPerson,
}

/// One of our own devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnDeviceView {
    /// `BLAKE3(device key)[0..8]` (zeros on a main device that never added another).
    pub id: [u8; 8],
    /// The main device (holds the identity key).
    pub main: bool,
    /// This device.
    pub this_device: bool,
    /// Whether the device has opened its setup and reported in; only the main
    /// device knows, and shows a device still waiting so an abandoned one can be removed.
    pub joined: bool,
}

/// Public view of a contact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactView {
    /// Contact id.
    pub id: ContactId,
    /// Self-chosen name from the card (untrusted), if any.
    pub display_name: Option<String>,
    /// Grouped base32 fingerprint.
    pub fingerprint: String,
    /// How the contact was paired.
    pub trust: TrustLevel,
    /// Privacy floor for this contact.
    pub floor: PrivacyGrade,
    /// Short authentication string as emoji-table indexes.
    pub sas: [u8; 6],
    /// When the contact was added, Unix ms.
    pub added_unix_ms: u64,
    /// We sent them a contact request they have not answered yet.
    pub requested: bool,
    /// Their email address, as the user typed it.
    pub email: Option<String>,
    /// The name the user gave this contact, shown instead of `display_name`.
    pub alias: Option<String>,
    /// The incoming-call tone chosen for this contact (an app tone id).
    pub ringtone: Option<String>,
    /// How late we accept this contact's edits, when not the global limit (a scale position).
    pub edit_limit: Option<u8>,
}

/// A contact request waiting in quarantine. Never shown as a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestView {
    /// Request id.
    pub id: RequestId,
    /// Grouped base32 fingerprint of the sender.
    pub fingerprint: String,
    /// Name from the sender's card: unverified.
    pub display_name: Option<String>,
    /// The sender's note, if any (at most 280 bytes).
    pub note: Option<String>,
    /// When it arrived, Unix ms.
    pub received_unix_ms: u64,
    /// The channel it arrived on.
    pub via: ChannelUse,
}

/// What a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// A text message.
    Text,
    /// Our contact request to someone (its text is the note, possibly empty).
    ContactRequest,
    /// A voice call, kept as a line in the chat (local only; never sent).
    Call(CallInfo),
    /// A file of a share: the answer to a download request (ours or theirs). Shown under
    /// "Shared with me" / the share, not in the chat.
    SharedFile {
        /// The share.
        share: ShareId,
        /// The item.
        item: ShareItemId,
    },
    /// A quiet line in the chat: the contact shared new files (its text is the share's name).
    ShareNotice {
        /// How many items are new.
        new_items: u16,
    },
}

/// What happened in a voice call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallInfo {
    /// How long the two talked, in seconds (0 when they never did).
    pub duration_s: u32,
    /// How it went.
    pub outcome: CallOutcome,
}

/// How a call went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallOutcome {
    /// They talked.
    Answered,
    /// Their call rang here and nobody answered.
    Missed,
    /// Declined (by us or by them).
    Declined,
    /// The other side was in another call.
    Busy,
    /// The media could not connect.
    Failed,
    /// Our call rang there and nobody answered.
    Unanswered,
}

impl CallOutcome {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Answered => 0,
            Self::Missed => 1,
            Self::Declined => 2,
            Self::Busy => 3,
            Self::Failed => 4,
            Self::Unanswered => 5,
        }
    }

    pub(crate) const fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            0 => Self::Answered,
            1 => Self::Missed,
            2 => Self::Declined,
            3 => Self::Busy,
            4 => Self::Failed,
            5 => Self::Unanswered,
            _ => return None,
        })
    }
}

/// Direction of a message relative to this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgDirection {
    /// Sent by us.
    Outgoing,
    /// Received from the contact.
    Incoming,
}

/// Status of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    /// Sealed and waiting for the user to pick a channel.
    Queued,
    /// Handed to at least one channel (see `channels`), not yet confirmed.
    Sent,
    /// The recipient confirmed it with a signed, encrypted ack.
    Delivered,
    /// The last transmission attempt failed.
    Failed,
    /// Received from the contact.
    Received,
}

/// One use of a channel for a message: honest provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelUse {
    /// Channel.
    pub cla: ClaId,
    /// Grade of that channel for this message.
    pub grade: PrivacyGrade,
    /// When, Unix ms.
    pub at_unix_ms: u64,
}

/// Public view of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageView {
    /// Message id.
    pub id: MsgId,
    /// Contact.
    pub contact: ContactId,
    /// Direction.
    pub direction: MsgDirection,
    /// What it is.
    pub kind: MessageKind,
    /// Text.
    pub text: String,
    /// Creation time from the sender's UUIDv7, Unix ms.
    pub created_unix_ms: u64,
    /// Status.
    pub status: DeliveryStatus,
    /// Every channel that carried this message, in order.
    pub channels: Vec<ChannelUse>,
    /// The group it belongs to; `None` for 1:1.
    pub group: Option<GroupId>,
    /// Our group message: members who confirmed it, of all it went to.
    pub recipients: Option<(u16, u16)>,
    /// Attached files, in order.
    pub attachments: Vec<AttachmentView>,
    /// Corrected by its sender after sending.
    pub edited: bool,
    /// Ours, and still within the conversation's edit limit: "edit" and "delete" may be offered.
    pub editable: bool,
    /// Ours: until when it may be edited or deleted, Unix ms (`None`: without end, or not ours).
    pub edit_until_unix_ms: Option<u64>,
}

/// A file to attach to a message. The core seals it; the bytes never leave in clear.
#[derive(Clone, PartialEq, Eq)]
pub struct NewAttachment {
    /// File name shown to the recipient.
    pub name: String,
    /// Media type, e.g. `image/jpeg`.
    pub media_type: String,
    /// The file, or where the runtime stored it sealed.
    pub data: AttachmentData,
    /// A small preview (thumbnail), if the app made one.
    pub preview: Option<Vec<u8>>,
}

impl core::fmt::Debug for NewAttachment {
    /// Never prints the file or its name.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewAttachment")
            .field("media_type", &self.media_type)
            .finish_non_exhaustive()
    }
}

/// The content of a file to attach.
#[derive(Clone, PartialEq, Eq)]
pub enum AttachmentData {
    /// A small file: the core seals it and sends it in parts with the message.
    Bytes(Vec<u8>),
    /// A large file the runtime already sealed under `key` and stored as `address`; the
    /// recipient pulls it directly.
    Stored {
        /// Content address of the sealed blob.
        address: [u8; 32],
        /// File size in bytes.
        size: u64,
        /// The file key it was sealed with.
        key: mem_crypto::attachment::FileKey,
    },
}

/// An attached file as shown. The file key stays inside the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentView {
    /// File name (chosen by the sender).
    pub name: String,
    /// Media type (stated by the sender).
    pub media_type: String,
    /// File size in bytes.
    pub size: u64,
    /// Small preview, if the sender made one.
    pub preview: Option<Vec<u8>>,
    /// Whether the file is here yet.
    pub state: AttachmentState,
}

/// Where an attached file stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentState {
    /// Ours, going out: `sent` of `total` parts confirmed by the recipient.
    Sending {
        /// Parts confirmed.
        sent: u16,
        /// All parts.
        total: u16,
    },
    /// Theirs, coming in: `have` of `total` parts arrived.
    Receiving {
        /// Parts here.
        have: u16,
        /// All parts.
        total: u16,
    },
    /// The whole file is on this device and can be opened.
    Ready,
}

/// Scope of a consent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentScope {
    /// This send only.
    Once,
    /// This contact and channel, until the given Unix ms (or until revoked).
    ThisContact {
        /// Expiry, Unix ms.
        until_unix_ms: Option<u64>,
    },
    /// This channel for every contact, until the given Unix ms (or until revoked).
    ChannelAlways {
        /// Expiry, Unix ms.
        until_unix_ms: Option<u64>,
    },
}

/// The user's answer to a [`ChannelOffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsentRecord {
    /// Offer being answered.
    pub offer: OfferId,
    /// Message the offer was for.
    pub message: MsgId,
    /// Channel the user accepted.
    pub cla: ClaId,
    /// Grade the user was shown.
    pub grade: PrivacyGrade,
    /// Scope chosen by the user.
    pub scope: ConsentScope,
}

/// A below-floor channel offered to the user with its disclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelOffer {
    /// Offer id to quote in the [`ConsentRecord`].
    pub id: OfferId,
    /// Message.
    pub message: MsgId,
    /// Contact.
    pub contact: ContactId,
    /// Channel.
    pub cla: ClaId,
    /// Channel grade.
    pub grade: PrivacyGrade,
    /// The contact's floor that this send goes below.
    pub floor: PrivacyGrade,
    /// Exposure profile the disclosure is generated from.
    pub exposure: ExposureProfile,
}

/// A standing consent rule created by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsentRule {
    /// Rule id.
    pub id: RuleId,
    /// Contact the rule is limited to, or `None` for every contact.
    pub contact: Option<ContactId>,
    /// Channel.
    pub cla: ClaId,
    /// Highest grade the rule permits.
    pub max_grade: PrivacyGrade,
    /// Expiry, Unix ms.
    pub until_unix_ms: Option<u64>,
    /// Creation time, Unix ms.
    pub created_unix_ms: u64,
}

/// Why a below-floor send was allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentBasis {
    /// The user consented for this send.
    Once,
    /// The user created a standing rule just now.
    NewRule(RuleId),
    /// An existing standing rule matched.
    ExistingRule(RuleId),
}

/// How an attestation went back to its relying party.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestReturn {
    /// Sent over HTTPS to the request's submit URL.
    Sent,
    /// Shown as text or a QR code for the user to paste.
    Copied,
}

/// One attestation this device made. No keys, nonces or subjects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestLogEntry {
    /// When, Unix ms.
    pub at_unix_ms: u64,
    /// The relying party's origin.
    pub rp_origin: String,
    /// What the relying party called the subject.
    pub subject_label: String,
    /// How it went back.
    pub returned: AttestReturn,
}

/// One entry of the privacy log: every below-floor send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyLogEntry {
    /// When, Unix ms.
    pub at_unix_ms: u64,
    /// Contact.
    pub contact: ContactId,
    /// Message.
    pub message: MsgId,
    /// Channel.
    pub cla: ClaId,
    /// Channel grade.
    pub grade: PrivacyGrade,
    /// Floor that was undercut.
    pub floor: PrivacyGrade,
    /// Why it was allowed.
    pub basis: ConsentBasis,
}

/// A channel the user may pick for a queued message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelOption {
    /// Channel.
    pub cla: ClaId,
    /// Grade.
    pub grade: PrivacyGrade,
    /// Whether the bundle fits this channel.
    pub fits: bool,
    /// Whether sending needs consent (below floor and no matching rule).
    pub needs_consent: bool,
    /// Whether the channel can deliver now: always for user-driven channels; for network
    /// channels, only while a connected peer advertises the contact's tag.
    pub reachable: bool,
}

/// Why an input was refused. Safe to show to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Data could not be parsed.
    Malformed,
    /// Valid MEM data of a kind or version this build does not handle.
    Unsupported,
    /// Not addressed to any of our contacts (or from an unapproved sender).
    NotForUs,
    /// Addressed to us, but decryption failed.
    Undecryptable,
    /// The signature or sender identity did not verify.
    BadSignature,
    /// Already received.
    Duplicate,
    /// The bundle's purge commitment is tombstoned: it was already delivered or recalled.
    Purged,
    /// Bundle lifetime is over.
    Expired,
    /// The pasted card is our own.
    OwnCard,
    /// Only the main device can do this: it needs the identity key.
    MainDeviceOnly,
    /// An identity has at most four devices.
    TooManyDevices,
    /// That device is already one of ours.
    DeviceKnown,
    /// Not one of our other devices.
    UnknownDevice,
    /// Unknown contact id.
    UnknownContact,
    /// Unknown or no longer queued message id.
    UnknownMessage,
    /// The bundle does not fit the chosen channel.
    TooLargeForChannel,
    /// The consent record did not match a pending offer.
    ConsentMismatch,
    /// The message text is empty or too long.
    InvalidText,
    /// An attachment is too large for this version, or the message has too many.
    FileTooLarge,
    /// An attachment's name, type or preview is out of bounds.
    InvalidAttachment,
    /// The display name is empty or too long.
    InvalidName,
    /// No connected peer can take the bundle to this contact right now.
    Unreachable,
    /// The user turned this channel off.
    ChannelDisabled,
    /// A peer exceeded its per-encounter quota.
    Quota,
    /// P5 cannot be a privacy floor.
    InvalidFloor,
    /// There is no new recovery phrase waiting to be confirmed.
    NoPendingPhrase,
    /// The contact's mailbox key may already be deleted, or a newer one just arrived: the
    /// message waits for the next message from them.
    WaitingForKey,
    /// The contact has not approved our contact request yet.
    AwaitingApproval,
    /// A contact request's proof of work is missing, too weak or invalid.
    BadProofOfWork,
    /// Unknown or already answered contact request.
    UnknownRequest,
    /// A contact request to someone who already is a contact, or one is already on its way.
    AlreadyContact,
    /// The note is empty or longer than 280 bytes.
    InvalidNote,
    /// No such group, or not one we are a member of.
    UnknownGroup,
    /// Only the group's admin may do that.
    NotGroupAdmin,
    /// A group has at most 16 members; a device keeps at most 64 groups.
    GroupFull,
    /// No other member to send to.
    GroupEmpty,
    /// A 1:1 message from someone who is only a fellow group member.
    NotAContact,
    /// Not a hub address MEM connects to (`wss://`, or `ws://` to this machine only).
    InvalidHubUrl,
    /// Not a push endpoint the hub may call (`https://`, or `http://` to this machine).
    InvalidPushEndpoint,
    /// Only a 1:1 text message can go as plain text.
    NotPlainText,
    /// The mail account settings are incomplete or malformed.
    InvalidEmailAccount,
    /// Not a plausible email address.
    InvalidEmailAddress,
}

/// Emoji table for the short authentication string. Shared by every frontend so
/// both devices render the same symbols. Chosen to be distinct and easy to name aloud.
pub const SAS_EMOJI: [&str; 64] = [
    "🐶", "🐱", "🦁", "🐴", "🦄", "🐷", "🐘", "🐰", "🐼", "🐓", "🐧", "🐢", "🐟", "🐙", "🦋", "🌷",
    "🌳", "🌵", "🍄", "🌏", "🌙", "☁️", "🔥", "🍌", "🍎", "🍓", "🌽", "🍕", "🎂", "❤️", "😀", "🤖",
    "🎩", "👓", "🔧", "🎅", "👍", "☂️", "⌛", "⏰", "🎁", "💡", "📕", "✏️", "📎", "✂️", "🔒", "🔑",
    "🔨", "☎️", "🏁", "🚂", "🚲", "✈️", "🚀", "🏆", "⚽", "🎸", "🎺", "🔔", "⚓", "🎧", "📁", "📌",
];

/// Renders a short authentication string as space-separated emoji.
#[must_use]
pub fn sas_emoji(sas: &[u8; 6]) -> String {
    let mut out = String::new();
    for (i, idx) in sas.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(
            SAS_EMOJI
                .get(usize::from(*idx & 63))
                .copied()
                .unwrap_or("?"),
        );
    }
    out
}

/// A share: files offered to one contact or a group's members, download only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShareId(pub(crate) [u8; 16]);

/// One item of a share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShareItemId(pub(crate) [u8; 16]);

macro_rules! raw_bytes {
    ($t:ty) => {
        impl $t {
            /// Raw bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            /// Wraps raw bytes, e.g. from the UI layer.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }
        }
    };
}
raw_bytes!(ShareId);
raw_bytes!(ShareItemId);

/// Who a share is offered to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareAudience {
    /// One contact.
    Contact(ContactId),
    /// The members of a group we belong to.
    Group(GroupId),
}

/// Whether group members we never met (introduced by the group's admin) see a group share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntroducedMembers {
    /// Only our own contacts in the group (the default).
    #[default]
    Excluded,
    /// Every current member.
    Included,
}

/// A file the user adds to a share. MEM does not copy it: `handle` says where it is (on
/// Android, a document address the app keeps permission for); it is read only when requested.
#[derive(Clone, PartialEq, Eq)]
pub struct NewShareItem {
    /// File name shown to the contact.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type, e.g. `image/jpeg`.
    pub media_type: String,
    /// Last change, Unix seconds, if known.
    pub modified_s: Option<u64>,
    /// Where the file is (opaque to the core; never sent).
    pub handle: String,
}

impl core::fmt::Debug for NewShareItem {
    /// Never prints the name or where the file is.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewShareItem")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// One of our shares, for the share editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnShareView {
    /// Id.
    pub id: ShareId,
    /// Name.
    pub name: String,
    /// Who it is offered to.
    pub audience: ShareAudience,
    /// For a group share: whether members we never met see it.
    pub introduced: IntroducedMembers,
    /// The items.
    pub items: Vec<OwnShareItemView>,
}

/// One item of our share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnShareItemView {
    /// Id.
    pub id: ShareItemId,
    /// The file as the user added it.
    pub item: NewShareItem,
    /// Who downloaded it and when (local only).
    pub downloads: Vec<(ContactId, u64)>,
}

/// A share a contact offers us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheirShareView {
    /// Who offers it.
    pub contact: ContactId,
    /// Id.
    pub id: ShareId,
    /// Name.
    pub name: String,
    /// The group it is offered to, if not to us alone.
    pub group: Option<GroupId>,
    /// The items.
    pub items: Vec<TheirShareItemView>,
    /// When the latest list arrived.
    pub updated_unix_ms: u64,
}

/// One item a contact offers us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheirShareItemView {
    /// Id.
    pub id: ShareItemId,
    /// File name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type.
    pub media_type: String,
    /// Last change, Unix seconds, if known.
    pub modified_s: Option<u64>,
    /// Whether we asked for it or have it.
    pub state: SharedItemState,
}

/// Where a download stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedItemState {
    /// Offered; not asked for.
    Available,
    /// Asked for; the file has not arrived.
    Requested,
    /// Here: the message that carries it (its attachment holds the file).
    Received(MsgId),
}
