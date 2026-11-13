//! Contact requests without meeting: intro bundles and proof of work.
//! Requests never become messages before approval.

use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::ClaId;
use mem_crypto::intro::{
    POW_DIFFICULTY, intro_key_id, intro_tag, pow_valid, pow_weeks, week_for_unix_ms,
};
use mem_crypto::purge::commit;
use mem_crypto::{
    ContactSecret, IdentityPublic, MailboxPublic, PairingNonce, PurgeCommitment, PurgeToken,
    SUITE_ID, SenderPurgeKey,
};
use mem_proto::inner::{
    CONTENT_TYPE_INTRO, Inner, IntroContent, MAX_INTRO_NOTE_LEN, SIGNATURE_DOMAIN, unpad,
};
use mem_proto::payload::Payload;
use mem_proto::{Bundle, BundleKind, Pow};
use rand_core::CryptoRng;

use super::{
    Agent, Contact, ContactPrefs, Message, MsgDirection, OutboxItem, Output, Recipient, Sealing,
    StorageOp, uuid_v7, verify_card,
};
use crate::model::{
    ChannelUse, ContactId, DeliveryStatus, MessageKind, MsgId, RejectReason, RequestId,
    RequestView, TrustLevel,
};

/// Most pending contact requests; beyond that the oldest is dropped.
pub const MAX_REQUESTS: usize = 64;
/// Most blocked senders remembered; the oldest is forgotten first.
pub const MAX_BLOCKED: usize = 1024;
/// Spray copies of an intro bundle.
pub const INTRO_COPIES: u8 = 4;
/// Lifetime of an intro bundle: 72 hours.
pub const INTRO_LIFETIME_S: u32 = 72 * 60 * 60;
/// Argon2id proof checks a node runs per encounter.
pub const POW_CHECKS_PER_ENCOUNTER: u8 = 16;

/// Work for the runtime: find a proof-of-work nonce off the agent's thread and answer with
/// [`crate::Input::PowSolved`]. Solving takes seconds; `handle` must not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowJob {
    /// The contact request this is for.
    pub message: MsgId,
    /// Inputs of the proof.
    pub bundle_id: [u8; 16],
    /// Destination intro tag.
    pub dst_tag: [u8; 16],
    /// Intro week.
    pub week: u32,
    /// Required leading zero bits.
    pub difficulty: u8,
}

/// A contact request we received and have not answered.
pub(crate) struct ContactRequest {
    pub(crate) id: RequestId,
    pub(crate) identity: IdentityPublic,
    pub(crate) mailbox: MailboxPublic,
    /// Their pairing nonce (from their card) and ours (the one they used).
    pub(crate) their_nonce: PairingNonce,
    pub(crate) our_nonce: PairingNonce,
    pub(crate) display_name: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) received_unix_ms: u64,
    pub(crate) via: ClaId,
    /// The intro's message id and commitment: approving acks it.
    pub(crate) message: MsgId,
    pub(crate) commitment: PurgeCommitment,
}

/// A contact request of ours waiting for its proof of work.
pub(crate) struct PendingIntro {
    pub(crate) contact: ContactId,
    pub(crate) bundle_id: [u8; 16],
    pub(crate) dst_tag: [u8; 16],
    pub(crate) week: u32,
    pub(crate) recipient_nonce: [u8; 16],
    pub(crate) note: Option<String>,
}

impl PendingIntro {
    const fn job(&self, message: MsgId) -> PowJob {
        PowJob {
            message,
            bundle_id: self.bundle_id,
            dst_tag: self.dst_tag,
            week: self.week,
            difficulty: POW_DIFFICULTY,
        }
    }
}

/// A trimmed note within bounds, or `Err` if non-empty and too long.
fn clean_note(note: Option<String>) -> Result<Option<String>, RejectReason> {
    match note.map(|n| String::from(n.trim())) {
        Some(n) if n.is_empty() => Ok(None),
        Some(n) if n.len() > MAX_INTRO_NOTE_LEN => Err(RejectReason::InvalidNote),
        other => Ok(other),
    }
}

impl Agent {
    /// Dispatches the contact-request inputs.
    pub(super) fn request_input(
        &mut self,
        input: crate::Input,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        use crate::Input;
        match input {
            Input::SendContactRequest { card, note } => self.send_contact_request(&card, note, rng),
            Input::PowSolved { message, nonce } => self.pow_solved(message, nonce, rng),
            Input::ApproveRequest(r) => self.approve_request(r, rng),
            Input::RejectRequest(r) => self.drop_request(r, false),
            Input::BlockRequest(r) => self.drop_request(r, true),
            Input::SetAcceptRequestsNearby { enabled } => {
                self.accept_intros = enabled;
                let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
                out.extend(self.advertise_all(rng));
                // The hub too: requests waiting there arrive now, not at the next reconnect.
                out.extend(self.refetch(rng));
                out
            }
            _ => Vec::new(),
        }
    }

    // ------------------------------------------------------------------ sending

    /// Starts a contact request to the owner of `card`. The contact is added as
    /// "requested"; the bundle is sealed once the runtime solved the proof of work.
    pub(super) fn send_contact_request(
        &mut self,
        card: &[u8],
        note: Option<String>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let note = match clean_note(note) {
            Ok(n) => n,
            Err(r) => return alloc::vec![Output::Rejected(r)],
        };
        let (identity, mailbox, their_nonce, name) = match verify_card(card) {
            Ok(parts) => parts,
            Err(r) => return alloc::vec![Output::Rejected(r)],
        };
        if identity == self.identity.public() {
            return alloc::vec![Output::Rejected(RejectReason::OwnCard)];
        }
        let id = ContactId(*identity.as_bytes());
        let in_flight = self.pending_intros.values().any(|p| p.contact == id)
            || self
                .outbox
                .values()
                .any(|i| i.contact == id && i.bundle.kind() == BundleKind::Intro);
        if in_flight || self.contacts.get(&id).is_some_and(|c| !c.requested) {
            return alloc::vec![Output::Rejected(RejectReason::AlreadyContact)];
        }
        // Contact requests need the identity key: the main device sends them.
        let Some(key) = self.identity.key() else {
            return alloc::vec![Output::Rejected(RejectReason::MainDeviceOnly)];
        };
        let Ok(secret) = ContactSecret::derive(key, &self.pairing_nonce, &identity, &their_nonce)
        else {
            return alloc::vec![Output::Rejected(RejectReason::BadSignature)];
        };
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        let serial = self.current_mailbox().serial;
        self.contacts.insert(
            id,
            Contact {
                identity,
                mailbox,
                secret,
                display_name: name,
                trust: TrustLevel::Remote,
                floor: self.default_floor,
                added_unix_ms: self.now_unix_ms,
                mailbox_serial: 0,
                mailbox_learned_ms: self.now_unix_ms,
                // Our card travels inside the request, with our current key.
                told_serial: serial,
                requested: true,
                group_only: false,
                email: None,
                prefs: ContactPrefs::default(),
                xwing: None,
                told_xwing: 0,
                devices: Vec::new(),
                device_list: None,
                told_list: 0,
            },
        );
        let message = MsgId(uuid_v7(self.now_unix_ms, rng));
        self.messages.push(Message {
            id: message,
            contact: id,
            direction: MsgDirection::Outgoing,
            kind: MessageKind::ContactRequest,
            text: note.clone().unwrap_or_default(),
            created_unix_ms: self.now_unix_ms,
            status: DeliveryStatus::Queued,
            channels: Vec::new(),
            group: None,
            fanout: None,
            edit: None,
            attachments: Vec::new(),
        });
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let week = week_for_unix_ms(self.now_unix_ms);
        let pending = PendingIntro {
            contact: id,
            bundle_id,
            dst_tag: intro_tag(&identity, week),
            week,
            recipient_nonce: *their_nonce.as_bytes(),
            note,
        };
        let job = pending.job(message);
        self.pending_intros.insert(message, pending);
        self.solving.insert(message);
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::ContactPaired {
                contact: id,
                updated: false,
                unchanged: false,
            },
            Output::Queued {
                contact: id,
                message,
            },
            Output::SolvePow(job),
        ];
        // New contact, new receiving tag: their approval comes back on it.
        out.extend(self.advertise_all(rng));
        out
    }

    /// The runtime found a nonce: seal the request and queue it.
    pub(super) fn pow_solved(
        &mut self,
        message: MsgId,
        nonce: [u8; 8],
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        self.solving.remove(&message);
        let Some(p) = self.pending_intros.get(&message) else {
            return Vec::new();
        };
        // Never emit an invalid proof: a runtime bug would otherwise make every courier refuse it.
        if !pow_valid(&p.bundle_id, &p.dst_tag, p.week, &nonce, POW_DIFFICULTY) {
            let job = p.job(message);
            self.solving.insert(message);
            return alloc::vec![Output::SolvePow(job)];
        }
        let Some(p) = self.pending_intros.remove(&message) else {
            return Vec::new();
        };
        let (Some(contact), Ok(card), Some(purge_key)) = (
            self.contacts.get(&p.contact),
            self.own_card(),
            self.purge_key.as_ref(),
        ) else {
            return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        };
        let Ok(content) = (IntroContent {
            card,
            recipient_nonce: p.recipient_nonce,
            note: p.note,
        })
        .encode() else {
            return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        };
        let token = purge_key.derive_token(&p.bundle_id);
        let Ok((bundle, _)) = self.seal(
            contact,
            Sealing {
                kind: BundleKind::Intro,
                content_type: CONTENT_TYPE_INTRO,
                content,
                attachments: Vec::new(),
                ack_requested: Some(true),
                acks: Vec::new(),
                lifetime_s: INTRO_LIFETIME_S,
                key_update: None,
                id: Some(message),
                recipient: Recipient::Intro {
                    dst_tag: p.dst_tag,
                    pow: Pow::new(nonce, POW_DIFFICULTY),
                },
            },
            p.bundle_id,
            &token,
            commit(&token),
            rng,
        ) else {
            return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        };
        self.outbox.insert(
            message,
            OutboxItem {
                contact: p.contact,
                bundle,
                created_unix_ms: self.now_unix_ms,
                acks: Vec::new(),
                copies: INTRO_COPIES,
                courier_ok: false,
                key_serial: None,
                list_serial: None,
                sealed_serial: 0,
                sealed_learned_ms: self.now_unix_ms,
                via_identity: false,
                content: None,
                device: None,
                copy_of: None,
            },
        );
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        out.extend(self.flush_message(message));
        out
    }

    /// Proof-of-work jobs lost with a restart are handed to the runtime again.
    pub(super) fn resume_pow(&mut self) -> Vec<Output> {
        let jobs: Vec<PowJob> = self
            .pending_intros
            .iter()
            .filter(|(m, _)| !self.solving.contains(m))
            .map(|(m, p)| p.job(*m))
            .collect();
        jobs.into_iter()
            .map(|job| {
                self.solving.insert(job.message);
                Output::SolvePow(job)
            })
            .collect()
    }

    /// Intro tags of `identity` that a node accepts now: weeks −1…+1.
    pub(super) fn intro_tags_of(&self, identity: &IdentityPublic) -> [[u8; 16]; 3] {
        let w = week_for_unix_ms(self.now_unix_ms);
        [w.saturating_sub(1), w, w.saturating_add(1)].map(|w| intro_tag(identity, w))
    }

    /// The week of our own intro tag `tag`, if it is one we accept now.
    pub(super) fn own_intro_week(&self, tag: &[u8; 16]) -> Option<u32> {
        let me = self.identity.public();
        let w = week_for_unix_ms(self.now_unix_ms);
        [w.saturating_sub(1), w, w.saturating_add(1)]
            .into_iter()
            .find(|w| intro_tag(&me, *w) == *tag)
    }

    // ---------------------------------------------------------------- receiving

    /// Opens a contact request addressed to one of our intro tags. Whatever happens after
    /// decryption, the bundle is tombstoned; requests never become messages.
    pub(super) fn received_intro(&mut self, cla: ClaId, bundle: &Bundle) -> Vec<Output> {
        let week = self
            .own_intro_week(bundle.dst_tag().as_bytes())
            .ok_or(RejectReason::NotForUs);
        let opened = week.and_then(|w| self.open_intro(bundle, w));
        let (request, token) = match opened {
            Ok(r) => r,
            Err(reason) => return alloc::vec![Output::Rejected(reason)],
        };
        let mut out = alloc::vec![Output::Persist(StorageOp::Store(
            self.tombstone(request.commitment, token)
        ))];
        let id = ContactId(*request.identity.as_bytes());
        if self.blocked.contains(request.identity.as_bytes()) {
            out.push(Output::Persist(StorageOp::SaveSnapshot));
            return out;
        }
        if let Some(c) = self.contacts.get_mut(&id) {
            if !c.requested {
                out.push(Output::Rejected(RejectReason::AlreadyContact));
                return out;
            }
            // We asked them too: their request answers ours.
            c.requested = false;
            self.remember_delivered(request.commitment, id, request.message, None);
            self.owe_ack(id, request.message);
            out.push(Output::Persist(StorageOp::SaveSnapshot));
            out.push(Output::ContactPaired {
                contact: id,
                updated: true,
                unchanged: false,
            });
            return out;
        }
        self.requests.retain(|r| r.identity != request.identity);
        if self.requests.len() >= MAX_REQUESTS {
            self.requests.remove(0);
        }
        let rid = request.id;
        self.requests.push(ContactRequest {
            via: cla,
            ..request
        });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        out.push(Output::ContactRequest { request: rid });
        out
    }

    /// Verifies and decrypts an intro bundle for intro week `week`.
    fn open_intro(
        &mut self,
        bundle: &Bundle,
        week: u32,
    ) -> Result<(ContactRequest, PurgeToken), RejectReason> {
        let pow = bundle.pow().ok_or(RejectReason::BadProofOfWork)?;
        if pow.difficulty() < POW_DIFFICULTY
            || !pow_valid(
                bundle.bundle_id().as_bytes(),
                bundle.dst_tag().as_bytes(),
                week,
                &pow.nonce(),
                pow.difficulty(),
            )
        {
            return Err(RejectReason::BadProofOfWork);
        }
        let payload = Payload::decode(bundle.payload()).map_err(|_| RejectReason::Malformed)?;
        if payload.suite_id != SUITE_ID {
            return Err(RejectReason::Unsupported);
        }
        // The sender took a mailbox key from our card; it may be one we already retired.
        let (held, key_id) = self
            .mailboxes
            .iter()
            .rev()
            .map(|h| (h, intro_key_id(&h.key.public())))
            .find(|(_, id)| payload.mailbox_key_id == *id.as_bytes())
            .ok_or(RejectReason::Undecryptable)?;
        let padded = held
            .key
            .open(&key_id, &payload.enc, &payload.ciphertext, &bundle.aad())
            .map_err(|_| RejectReason::Undecryptable)?;
        let inner = unpad(&padded)
            .and_then(Inner::decode)
            .map_err(|_| RejectReason::Malformed)?;
        if inner.fields.content_type != CONTENT_TYPE_INTRO
            || !inner.fields.acks.is_empty()
            || inner.fields.key_update.is_some()
        {
            return Err(RejectReason::Malformed);
        }
        let content =
            IntroContent::decode(&inner.fields.content).map_err(|_| RejectReason::Malformed)?;
        let (identity, mailbox, their_nonce, display_name) = verify_card(&content.card)?;
        let me = self.identity.public();
        if inner.fields.sender != *identity.as_bytes() || identity == me {
            return Err(RejectReason::BadSignature);
        }
        identity
            .verify(
                SIGNATURE_DOMAIN,
                &[me.as_bytes(), &inner.fields.signed_bytes()],
                &inner.signature,
            )
            .map_err(|_| RejectReason::BadSignature)?;
        let token = PurgeToken::from_bytes(inner.fields.purge_token);
        let commitment = PurgeCommitment::from_bytes(*bundle.purge_commitment());
        if commit(&token) != commitment {
            return Err(RejectReason::Malformed);
        }
        // A request made from our current card or the one before it.
        let our_nonce = [
            Some(&self.pairing_nonce),
            self.previous_pairing_nonce.as_ref(),
        ]
        .into_iter()
        .flatten()
        .find(|n| n.as_bytes() == &content.recipient_nonce)
        .map(|n| PairingNonce::from_bytes(*n.as_bytes()))
        .ok_or(RejectReason::NotForUs)?;
        let key = self.identity.key().ok_or(RejectReason::MainDeviceOnly)?;
        let secret = ContactSecret::derive(key, &our_nonce, &identity, &their_nonce)
            .map_err(|_| RejectReason::BadSignature)?;
        if inner.fields.conversation_id != secret.conversation_id() {
            return Err(RejectReason::Malformed);
        }
        let id = RequestId(self.next_id());
        Ok((
            ContactRequest {
                id,
                identity,
                mailbox,
                their_nonce,
                our_nonce,
                display_name,
                note: content.note,
                received_unix_ms: self.now_unix_ms,
                via: ClaId::Courier,
                message: MsgId(inner.fields.message_id),
                commitment,
            },
            token,
        ))
    }

    /// Courier check of a stranger's intro bundle: at most
    /// [`POW_CHECKS_PER_ENCOUNTER`] Argon2id runs per encounter, none after the first invalid
    /// proof. The week is the bundle's creation week as far as our clock tells.
    pub(super) fn courier_pow_ok(&mut self, peer: super::PeerHandle, bundle: &Bundle) -> bool {
        let Some(pow) = bundle.pow() else {
            return false;
        };
        if pow.difficulty() < POW_DIFFICULTY {
            return false;
        }
        let (weeks, n) = pow_weeks(self.now_unix_ms.saturating_sub(bundle.age_ms()));
        for week in weeks.into_iter().take(n) {
            if !self.spend_pow_check(peer) {
                return false;
            }
            if pow_valid(
                bundle.bundle_id().as_bytes(),
                bundle.dst_tag().as_bytes(),
                week,
                &pow.nonce(),
                pow.difficulty(),
            ) {
                return true;
            }
        }
        if let Some(p) = self.peers.get_mut(&peer) {
            p.intro_bad = true;
        }
        false
    }

    /// Takes one proof check from this encounter's budget.
    pub(super) fn spend_pow_check(&mut self, peer: super::PeerHandle) -> bool {
        match self.peers.get_mut(&peer) {
            Some(p) if !p.intro_bad && p.intro_checks < POW_CHECKS_PER_ENCOUNTER => {
                p.intro_checks += 1;
                true
            }
            _ => false,
        }
    }

    // -------------------------------------------------------------- answering

    /// Approves a request: the sender becomes a contact (trust Remote) and its intro is acked
    /// on the normal tags, which confirms the contact on their side.
    pub(super) fn approve_request(
        &mut self,
        request: RequestId,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(pos) = self.requests.iter().position(|r| r.id == request) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownRequest)];
        };
        let Some(key) = self.identity.key() else {
            return alloc::vec![Output::Rejected(RejectReason::MainDeviceOnly)];
        };
        let r = self.requests.remove(pos);
        let Ok(secret) = ContactSecret::derive(key, &r.our_nonce, &r.identity, &r.their_nonce)
        else {
            return alloc::vec![Output::Rejected(RejectReason::BadSignature)];
        };
        let id = ContactId(*r.identity.as_bytes());
        self.contacts.insert(
            id,
            Contact {
                identity: r.identity,
                mailbox: r.mailbox,
                secret,
                display_name: r.display_name,
                trust: TrustLevel::Remote,
                floor: self.default_floor,
                added_unix_ms: self.now_unix_ms,
                mailbox_serial: 0,
                mailbox_learned_ms: self.now_unix_ms,
                // We cannot know which of our keys their copy of our card had: announce ours.
                told_serial: 0,
                requested: false,
                group_only: false,
                email: None,
                prefs: ContactPrefs::default(),
                xwing: None,
                told_xwing: 0,
                devices: Vec::new(),
                device_list: None,
                told_list: 0,
            },
        );
        self.remember_delivered(r.commitment, id, r.message, None);
        self.owe_ack(id, r.message);
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::ContactPaired {
                contact: id,
                updated: false,
                unchanged: false,
            },
        ];
        out.extend(self.advertise_all(rng));
        out
    }

    /// Deletes a request; `block` also ignores that sender's requests from now on.
    pub(super) fn drop_request(&mut self, request: RequestId, block: bool) -> Vec<Output> {
        let Some(pos) = self.requests.iter().position(|r| r.id == request) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownRequest)];
        };
        let r = self.requests.remove(pos);
        if block {
            let key = *r.identity.as_bytes();
            if !self.blocked.contains(&key) {
                if self.blocked.len() >= MAX_BLOCKED {
                    self.blocked.remove(0);
                }
                self.blocked.push(key);
            }
        }
        alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
    }

    /// Pending requests, oldest first.
    #[must_use]
    pub fn requests(&self) -> Vec<RequestView> {
        self.requests
            .iter()
            .map(|r| RequestView {
                id: r.id,
                fingerprint: r.identity.fingerprint().to_string_grouped(),
                display_name: r.display_name.clone(),
                note: r.note.clone(),
                received_unix_ms: r.received_unix_ms,
                via: ChannelUse {
                    cla: r.via,
                    grade: r.via.grade(),
                    at_unix_ms: r.received_unix_ms,
                },
            })
            .collect()
    }

    /// Whether our intro tag is advertised in encounters.
    #[must_use]
    pub const fn accepts_requests_nearby(&self) -> bool {
        self.accept_intros
    }
}
