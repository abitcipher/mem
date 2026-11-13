//! Attachments: each file sealed once under its own key, its blob sent as hidden part messages,
//! and parts from a contact reassembled and checked against the blob's content address.
//!
//! Small blobs ([`SMALL_SEALED_MAX`]) travel as parts: ordinary sealed bundles (content type
//! [`CONTENT_TYPE_ATTACHMENT_PART`]) with the same channels, grades, consent, acks,
//! retransmission and purge as text messages; incoming parts wait in the state until their blob
//! is complete, within [`MAX_PENDING_PART_BYTES`].
//!
//! Large blobs are pulled by the recipient over a direct encounter with the sender
//! ([`EncounterMsg::BlobWant`] / [`EncounterMsg::BlobPart`]), in windows of [`PULL_WINDOW`],
//! resumable from the last byte written. The request carries a proof bound to the encounter;
//! both sides use a channel only within the contact's floor (or a standing rule). The runtime
//! moves the bytes; the core decides who may have what.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::ClaId;
use mem_crypto::attachment::{ContentAddress, FileKey, SEALED_CHUNK_LEN, seal, sealed_len};
use mem_proto::BundleKind;
use mem_proto::attachment::{
    AttachmentPart, AttachmentRef, CONTENT_TYPE_ATTACHMENT_PART, MAX_ATTACHMENTS, PART_LEN,
};
use mem_proto::encounter::{EncounterMsg, MAX_BLOB_PART};
use rand_core::CryptoRng;

use super::{Agent, Message, MsgDirection, Output, StorageOp};
use crate::model::{
    AttachmentData, AttachmentState, AttachmentView, ChannelUse, ContactId, MsgId, NewAttachment,
    PeerHandle, RejectReason,
};

/// Largest sealed blob this version sends and accepts: a file padded to 256 KiB (four chunks).
pub const SMALL_SEALED_MAX: u64 = 4 * SEALED_CHUNK_LEN as u64;
/// Incoming parts waiting for the rest of their blob, in bytes; the oldest blob is dropped
/// beyond this.
pub const MAX_PENDING_PART_BYTES: usize = 4 * 1024 * 1024;
/// Bytes of a large blob asked for at a time.
pub const PULL_WINDOW: u64 = 4 * MAX_BLOB_PART as u64;

/// Whether a file of `size` bytes is pulled directly rather than sent in parts.
fn is_large(size: u64) -> bool {
    sealed_len(size).is_none_or(|l| l > SMALL_SEALED_MAX)
}

/// A large incoming blob being pulled from its sender.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Pull {
    /// Who sent the message naming it; only their encounters are asked.
    pub(crate) contact: ContactId,
    /// Length of the whole sealed blob.
    pub(crate) total: u64,
    /// Bytes written so far, from the start (the runtime writes at offsets, so a repeat is
    /// harmless).
    pub(crate) have: u64,
    /// The encounter asked now, and the end of the window asked for (volatile).
    pub(crate) peer: Option<PeerHandle>,
    pub(crate) until: u64,
    /// How the last piece arrived, recorded on the message once the blob checks out (volatile).
    pub(crate) via: Option<ChannelUse>,
    /// Complete; the runtime is checking it (volatile).
    pub(crate) checking: bool,
}

/// An attached file of a stored message.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    /// The reference as sent or received (holds the file key).
    pub(crate) reference: AttachmentRef,
    /// Ours: the hidden part messages carrying the blob; a part no longer in the outbox has
    /// been confirmed (or expired).
    pub(crate) parts: Vec<MsgId>,
}

/// Parts of one incoming blob, waiting for the rest.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PartialBlob {
    /// Who sends it: parts from anyone else for the same address are refused.
    pub(crate) contact: ContactId,
    /// Length of the whole sealed blob.
    pub(crate) total: u64,
    /// Parts by offset.
    pub(crate) parts: BTreeMap<u64, Vec<u8>>,
    /// When the first part arrived, Unix ms (the oldest blob is dropped first).
    pub(crate) first_unix_ms: u64,
}

/// What the runtime needs to open an attached file stored on this device.
pub struct AttachmentSecret {
    /// Content address of the sealed blob (its storage name).
    pub address: [u8; 32],
    /// File size in bytes.
    pub size: u64,
    /// The file key.
    pub key: FileKey,
    /// File name (chosen by the sender).
    pub name: String,
    /// Media type (stated by the sender).
    pub media_type: String,
}

/// Parts of a sealed blob of `total` bytes.
fn part_count(total: u64) -> u16 {
    u16::try_from(total.div_ceil(PART_LEN as u64)).unwrap_or(u16::MAX)
}

/// A file sealed for sending, with its blob when the core sealed it (small files); large files
/// are already stored by the runtime.
pub(crate) struct SealedFile {
    pub(super) reference: AttachmentRef,
    pub(super) sealed: Option<Vec<u8>>,
}

impl SealedFile {
    /// The record kept on our message (its part ids are filled in once queued).
    pub(super) fn stored(&self) -> Attachment {
        Attachment {
            reference: self.reference.clone(),
            parts: Vec::new(),
        }
    }
}

impl Agent {
    /// Seals each file under a fresh key. Checks every bound before any work.
    pub(super) fn seal_files(
        files: Vec<NewAttachment>,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<SealedFile>, RejectReason> {
        if files.len() > MAX_ATTACHMENTS {
            return Err(RejectReason::FileTooLarge);
        }
        files
            .into_iter()
            .map(|f| {
                let (address, size, key, sealed) = match f.data {
                    AttachmentData::Bytes(data) => {
                        let size = data.len() as u64;
                        if is_large(size) {
                            return Err(RejectReason::FileTooLarge);
                        }
                        let key = FileKey::generate(rng);
                        let sealed = seal(&key, &data).map_err(|_| RejectReason::FileTooLarge)?;
                        (ContentAddress::of(&sealed).0, size, key, Some(sealed))
                    }
                    // Only large files are stored ahead; small ones would never be pulled.
                    AttachmentData::Stored { address, size, key } => {
                        if !is_large(size) || sealed_len(size).is_none() {
                            return Err(RejectReason::InvalidAttachment);
                        }
                        (address, size, key, None)
                    }
                };
                let reference = AttachmentRef {
                    address,
                    size,
                    key: *key.as_bytes(),
                    name: f.name,
                    media_type: f.media_type,
                    preview: f.preview,
                };
                mem_proto::attachment::check_list(core::slice::from_ref(&reference))
                    .map_err(|_| RejectReason::InvalidAttachment)?;
                Ok(SealedFile { reference, sealed })
            })
            .collect()
    }

    /// Queues the parts of each blob to `contact` and its other devices; returns the part
    /// ids of each file, in order.
    pub(super) fn queue_parts(
        &mut self,
        contact: ContactId,
        files: &[SealedFile],
        rng: &mut impl CryptoRng,
    ) -> Vec<Vec<MsgId>> {
        let mut ids = Vec::with_capacity(files.len());
        for f in files {
            let mut part_ids = Vec::new();
            let Some(sealed) = &f.sealed else {
                ids.push(part_ids);
                continue;
            };
            for part in AttachmentPart::split(f.reference.address, sealed) {
                // Offsets and lengths come from `split`, so encoding cannot fail.
                let Ok(content) = part.encode() else {
                    continue;
                };
                if let Some(id) =
                    self.queue_hidden(contact, CONTENT_TYPE_ATTACHMENT_PART, content.clone(), rng)
                {
                    self.queue_device_copies(
                        contact,
                        id,
                        BundleKind::Message,
                        CONTENT_TYPE_ATTACHMENT_PART,
                        &content,
                        rng,
                    );
                    part_ids.push(id);
                }
            }
            ids.push(part_ids);
        }
        ids
    }

    /// Sends the blobs of message `id` in parts and keeps our own copies: the part ids go on
    /// the message, and the blobs come back as storage requests to apply before the snapshot.
    pub(super) fn send_files_of(
        &mut self,
        contact: ContactId,
        id: MsgId,
        files: Vec<SealedFile>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let part_ids = self.queue_parts(contact, &files, rng);
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            for (a, ids) in m.attachments.iter_mut().zip(part_ids) {
                a.parts = ids;
            }
        }
        files
            .into_iter()
            .filter_map(|f| {
                self.blobs.insert(f.reference.address);
                f.sealed.map(|sealed| {
                    Output::Persist(StorageOp::PutBlob {
                        address: f.reference.address,
                        sealed,
                    })
                })
            })
            .collect()
    }

    /// Takes one part from `contact`. When it completes a blob whose content address checks
    /// out, the blob goes to storage. Outputs to apply before the snapshot is saved.
    pub(super) fn received_part(
        &mut self,
        contact: ContactId,
        part: AttachmentPart,
    ) -> Vec<Output> {
        if part.total > SMALL_SEALED_MAX || self.blobs.contains(&part.address) {
            // Too large for this version, or already here: nothing to keep.
            return Vec::new();
        }
        let now = self.now_unix_ms;
        let entry = self
            .partials
            .entry(part.address)
            .or_insert_with(|| PartialBlob {
                contact,
                total: part.total,
                parts: BTreeMap::new(),
                first_unix_ms: now,
            });
        if entry.contact != contact || entry.total != part.total {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        entry.parts.insert(part.offset, part.bytes);
        let complete = entry.parts.len() == usize::from(part_count(entry.total));
        if !complete {
            self.limit_partials(part.address);
            return Vec::new();
        }
        let Some(done) = self.partials.remove(&part.address) else {
            return Vec::new();
        };
        let sealed: Vec<u8> = done.parts.into_values().flatten().collect();
        if ContentAddress::of(&sealed).0 != part.address {
            // A part was altered by the sender's side: the blob can never complete.
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        self.blobs.insert(part.address);
        alloc::vec![Output::Persist(StorageOp::PutBlob {
            address: part.address,
            sealed,
        })]
    }

    /// Drops the oldest incomplete blobs (never `keep`) while waiting parts exceed
    /// [`MAX_PENDING_PART_BYTES`].
    fn limit_partials(&mut self, keep: [u8; 32]) {
        loop {
            let held: usize = self
                .partials
                .values()
                .flat_map(|p| p.parts.values())
                .map(Vec::len)
                .sum();
            if held <= MAX_PENDING_PART_BYTES {
                return;
            }
            let Some(oldest) = self
                .partials
                .iter()
                .filter(|(a, _)| **a != keep)
                .min_by_key(|(_, p)| p.first_unix_ms)
                .map(|(a, _)| *a)
            else {
                return;
            };
            self.partials.remove(&oldest);
        }
    }

    /// How `m`'s attachments are shown.
    pub(super) fn attachment_views(&self, m: &Message) -> Vec<AttachmentView> {
        m.attachments
            .iter()
            .map(|a| {
                let r = &a.reference;
                let total = sealed_len(r.size).map_or(0, part_count);
                let state = match m.direction {
                    // A large file of ours is here from the start; the recipient pulls it.
                    MsgDirection::Outgoing if a.parts.is_empty() && is_large(r.size) => {
                        AttachmentState::Ready
                    }
                    MsgDirection::Outgoing => {
                        let sent = a
                            .parts
                            .iter()
                            .filter(|id| !self.outbox.contains_key(id))
                            .count();
                        let sent = u16::try_from(sent).unwrap_or(u16::MAX);
                        if sent >= total {
                            AttachmentState::Ready
                        } else {
                            AttachmentState::Sending { sent, total }
                        }
                    }
                    MsgDirection::Incoming if self.blobs.contains(&r.address) => {
                        AttachmentState::Ready
                    }
                    MsgDirection::Incoming => AttachmentState::Receiving {
                        have: match self.pulls.get(&r.address) {
                            Some(p) => part_count(p.have),
                            None => self
                                .partials
                                .get(&r.address)
                                .map_or(0, |p| u16::try_from(p.parts.len()).unwrap_or(u16::MAX)),
                        },
                        total,
                    },
                };
                AttachmentView {
                    name: r.name.clone(),
                    media_type: r.media_type.clone(),
                    size: r.size,
                    preview: r.preview.clone(),
                    state,
                }
            })
            .collect()
    }

    /// Starts pulling the large files of a message from `contact` that are not here yet, at
    /// once if that contact is connected.
    pub(super) fn track_pulls(
        &mut self,
        contact: ContactId,
        refs: &[AttachmentRef],
    ) -> Vec<Output> {
        for r in refs.iter().filter(|r| is_large(r.size)) {
            let Some(total) = sealed_len(r.size) else {
                continue;
            };
            if !self.blobs.contains(&r.address) {
                self.pulls.entry(r.address).or_insert(Pull {
                    contact,
                    total,
                    have: 0,
                    peer: None,
                    until: 0,
                    via: None,
                    checking: false,
                });
            }
        }
        let peers: Vec<PeerHandle> = self.peers.keys().copied().collect();
        peers
            .into_iter()
            .flat_map(|p| self.request_blobs(p))
            .collect()
    }

    /// Whether a blob transfer with `contact` may use `cla` without asking: within the
    /// contact's floor, or under a standing rule the user created.
    fn blob_channel_ok(&self, contact: ContactId, cla: ClaId) -> bool {
        let grade = cla.grade();
        self.contacts.get(&contact).is_some_and(|c| {
            grade.within(c.floor) || self.matching_rule(contact, cla, grade).is_some()
        })
    }

    /// Asks `peer` for the next window of one blob it can give us, unless a pull from it is
    /// already under way (one at a time per encounter).
    pub(super) fn request_blobs(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(p) = self.peers.get(&peer) else {
            return Vec::new();
        };
        let Some(asked_id) = p.their_id else {
            return Vec::new();
        };
        if !p.hello || self.pulls.values().any(|x| x.peer == Some(peer)) {
            return Vec::new();
        }
        let wanted = self.pulls.iter().find_map(|(address, x)| {
            let c = self.contacts.get(&x.contact)?;
            let theirs = self.delivery_tags(c).iter().any(|t| p.tags.contains(t));
            (x.peer.is_none() && !x.checking && theirs && self.blob_channel_ok(x.contact, p.cla))
                .then(|| (*address, c.secret.blob_proof(address, &asked_id)))
        });
        let Some((address, proof)) = wanted else {
            return Vec::new();
        };
        let Some(x) = self.pulls.get_mut(&address) else {
            return Vec::new();
        };
        x.peer = Some(peer);
        x.until = x.have.saturating_add(PULL_WINDOW).min(x.total);
        alloc::vec![Output::Encounter {
            peer,
            msg: EncounterMsg::BlobWant {
                address,
                offset: x.have,
                proof,
            },
        }]
    }

    /// A peer asks for a blob of ours: answered only if the proof shows it is the contact the
    /// file went to, in this encounter, over a channel within that contact's floor.
    pub(super) fn blob_wanted(
        &self,
        peer: PeerHandle,
        address: [u8; 32],
        offset: u64,
        proof: &[u8; 16],
    ) -> Vec<Output> {
        let Some(p) = self.peers.get(&peer) else {
            return Vec::new();
        };
        let sent_to = self.messages.iter().find_map(|m| {
            let a = m
                .attachments
                .iter()
                .find(|a| a.reference.address == address)?;
            (m.direction == MsgDirection::Outgoing).then_some((m.contact, a.reference.size))
        });
        let Some((contact, size)) = sent_to else {
            return Vec::new();
        };
        let Some(total) = sealed_len(size) else {
            return Vec::new();
        };
        let proven = self
            .contacts
            .get(&contact)
            .is_some_and(|c| c.secret.verify_blob_proof(&address, &p.our_id, proof));
        if !proven
            || !self.blob_channel_ok(contact, p.cla)
            || !self.blobs.contains(&address)
            || offset >= total
        {
            return Vec::new();
        }
        alloc::vec![Output::SendBlob {
            peer,
            address,
            offset,
            len: PULL_WINDOW.min(total - offset),
            total,
        }]
    }

    /// A piece of a blob we asked `peer` for. Pieces out of order or not asked for are ignored.
    pub(super) fn blob_part(
        &mut self,
        peer: PeerHandle,
        address: [u8; 32],
        offset: u64,
        total: u64,
        bytes: Vec<u8>,
    ) -> Vec<Output> {
        let Some(cla) = self.peers.get(&peer).map(|p| p.cla) else {
            return Vec::new();
        };
        let now = self.now_unix_ms;
        let Some(x) = self.pulls.get_mut(&address) else {
            return Vec::new();
        };
        if x.peer != Some(peer) || x.total != total || x.have != offset {
            return Vec::new();
        }
        x.have += bytes.len() as u64;
        let (have, until) = (x.have, x.until);
        let mut out = alloc::vec![Output::Persist(StorageOp::WriteBlob {
            address,
            offset,
            bytes,
        })];
        if have >= total {
            x.peer = None;
            x.checking = true;
            x.via = Some(ChannelUse {
                cla,
                grade: cla.grade(),
                at_unix_ms: now,
            });
            out.push(Output::Persist(StorageOp::FinishBlob { address, total }));
        } else if have >= until {
            // The window is done: keep the progress, show it, then ask for the next one.
            x.peer = None;
            out.push(Output::Persist(StorageOp::SaveSnapshot));
            out.extend(self.attachment_changed(address));
            out.extend(self.request_blobs(peer));
        }
        out
    }

    /// The runtime's verdict on a completed pull. A match is stored and its channel recorded on
    /// the messages that name it (honest provenance); a mismatch starts over.
    pub(super) fn blob_checked(&mut self, address: [u8; 32], ok: bool) -> Vec<Output> {
        let Some(mut x) = self.pulls.remove(&address) else {
            return Vec::new();
        };
        if !ok {
            x.have = 0;
            x.checking = false;
            x.via = None;
            self.pulls.insert(address, x);
            return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        }
        self.blobs.insert(address);
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        for m in self
            .messages
            .iter_mut()
            .filter(|m| m.attachments.iter().any(|a| a.reference.address == address))
        {
            if let Some(via) = x.via
                && !m
                    .channels
                    .iter()
                    .any(|u| u.cla == via.cla && u.grade == via.grade)
            {
                m.channels.push(via);
            }
        }
        out.extend(self.attachment_changed(address));
        out
    }

    /// Tells the app that the messages naming blob `address` changed (progress or arrival),
    /// without announcing them as new.
    fn attachment_changed(&self, address: [u8; 32]) -> Vec<Output> {
        self.messages
            .iter()
            .filter(|m| m.attachments.iter().any(|a| a.reference.address == address))
            .map(|m| Output::StatusChanged {
                message: m.id,
                status: m.status,
            })
            .collect()
    }

    /// The attachment references of message `id`, or of the message it is a device copy of.
    pub(super) fn attachment_refs(&self, id: MsgId) -> Vec<AttachmentRef> {
        let main = self.outbox.get(&id).and_then(|o| o.copy_of).unwrap_or(id);
        self.messages
            .iter()
            .find(|m| m.id == main)
            .map(|m| m.attachments.iter().map(|a| a.reference.clone()).collect())
            .unwrap_or_default()
    }

    /// What the runtime needs to open attachment `index` of `message`, once its blob is here.
    #[must_use]
    pub fn attachment(&self, message: MsgId, index: usize) -> Option<AttachmentSecret> {
        let r = &self
            .messages
            .iter()
            .find(|m| m.id == message)?
            .attachments
            .get(index)?
            .reference;
        Some(AttachmentSecret {
            address: r.address,
            size: r.size,
            key: FileKey::from_bytes(r.key),
            name: r.name.clone(),
            media_type: r.media_type.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Input;
    use mem_cla::ClaId;
    use mem_crypto::attachment::open;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    const NOW: u64 = 1_792_022_400_000;

    fn agent(seed: u8) -> (Agent, ChaCha20Rng) {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let mut a = Agent::new(NOW, &mut rng);
        let _ = a.handle(Input::Tick { wall_unix_ms: NOW }, &mut rng);
        (a, rng)
    }

    /// Two contacts; returns A, B and B's id at A.
    fn paired() -> (Agent, ChaCha20Rng, Agent, ChaCha20Rng, ContactId) {
        let (mut a, mut ra) = agent(1);
        let (mut b, mut rb) = agent(2);
        let (card_a, card_b) = (a.own_card().unwrap(), b.own_card().unwrap());
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

    fn file(len: usize) -> NewAttachment {
        NewAttachment {
            name: "photo.jpg".into(),
            media_type: "image/jpeg".into(),
            data: AttachmentData::Bytes((0..len).map(|i| u8::try_from(i % 251).unwrap()).collect()),
            preview: None,
        }
    }

    /// Every bundle A holds for B, in the given order.
    fn bundles(a: &Agent) -> Vec<Vec<u8>> {
        a.outbox.values().map(|o| o.bundle.encode()).collect()
    }

    fn deliver(b: &mut Agent, rb: &mut ChaCha20Rng, bytes: Vec<u8>) -> Vec<Output> {
        b.handle(
            Input::Received {
                cla: ClaId::Qr,
                bytes,
            },
            rb,
        )
    }

    fn blob_of(out: &[Output]) -> Option<&Vec<u8>> {
        out.iter().find_map(|o| match o {
            Output::Persist(StorageOp::PutBlob { sealed, .. }) => Some(sealed),
            _ => None,
        })
    }

    #[test]
    fn a_file_arrives_whole_in_any_order() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let sent = file(100_000);
        let out = a.handle(
            Input::UserSendFiles {
                contact: b_id,
                text: String::new(),
                files: alloc::vec![sent.clone()],
            },
            &mut ra,
        );
        // Our own copy is stored before the snapshot that names it.
        assert!(matches!(out[0], Output::Persist(StorageOp::PutBlob { .. })));
        let view = &a.messages(b_id)[0].attachments[0];
        assert_eq!(view.state, AttachmentState::Sending { sent: 0, total: 5 });

        // Parts first and in reverse, the message last.
        let mut stored = None;
        for bytes in bundles(&a).into_iter().rev() {
            let out = deliver(&mut b, &mut rb, bytes);
            if let Some(blob) = blob_of(&out) {
                // Stored before anything else is persisted.
                assert!(matches!(out[0], Output::Persist(StorageOp::PutBlob { .. })));
                stored = Some(blob.clone());
            }
        }
        let a_id = b.contacts()[0].id;
        let msg = &b.messages(a_id)[0];
        assert_eq!(msg.attachments[0].state, AttachmentState::Ready);
        let secret = b.attachment(msg.id, 0).unwrap();
        let file = open(&secret.key, &stored.unwrap(), secret.size).unwrap();
        let AttachmentData::Bytes(data) = &sent.data else {
            unreachable!()
        };
        assert_eq!(&*file, &data[..]);
        assert_eq!(secret.name, "photo.jpg");
        assert!(b.partials.is_empty());
    }

    #[test]
    fn waiting_parts_survive_a_restart() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let _ = a.handle(
            Input::UserSendFiles {
                contact: b_id,
                text: "two halves".into(),
                files: alloc::vec![file(70_000)],
            },
            &mut ra,
        );
        let all = bundles(&a);
        let (first, rest) = all.split_at(2);
        for bytes in first {
            let _ = deliver(&mut b, &mut rb, bytes.clone());
        }
        let mut b = Agent::restore(&b.snapshot(), NOW).unwrap();
        // The message and the first part arrived; the part waits in the state.
        assert_eq!(b.partials.len(), 1);
        let mut stored = false;
        for bytes in rest {
            stored |= blob_of(&deliver(&mut b, &mut rb, bytes.clone())).is_some();
        }
        assert!(stored);
        let a_id = b.contacts()[0].id;
        assert_eq!(
            b.messages(a_id)[0].attachments[0].state,
            AttachmentState::Ready
        );
    }

    #[test]
    fn limits_are_checked_before_sending() {
        let (mut a, mut ra, _, _, b_id) = paired();
        let send = |a: &mut Agent, ra: &mut ChaCha20Rng, text: &str, files| {
            a.handle(
                Input::UserSendFiles {
                    contact: b_id,
                    text: text.into(),
                    files,
                },
                ra,
            )
        };
        // Above the small-file bucket.
        let out = send(&mut a, &mut ra, "", alloc::vec![file(262_144)]);
        assert_eq!(
            out,
            alloc::vec![Output::Rejected(RejectReason::FileTooLarge)]
        );
        let out = send(&mut a, &mut ra, "", Vec::new());
        assert_eq!(
            out,
            alloc::vec![Output::Rejected(RejectReason::InvalidText)]
        );
        let mut unnamed = file(10);
        unnamed.name = String::new();
        let out = send(&mut a, &mut ra, "x", alloc::vec![unnamed]);
        assert_eq!(
            out,
            alloc::vec![Output::Rejected(RejectReason::InvalidAttachment)]
        );
        let out = send(
            &mut a,
            &mut ra,
            "",
            alloc::vec![file(10); MAX_ATTACHMENTS + 1],
        );
        assert_eq!(
            out,
            alloc::vec![Output::Rejected(RejectReason::FileTooLarge)]
        );
        assert!(a.messages.is_empty() && a.outbox.is_empty() && a.blobs.is_empty());
    }

    #[test]
    fn an_altered_blob_is_never_stored() {
        let (_, _, mut b, _, _) = paired();
        let a_id = b.contacts()[0].id;
        let sealed = alloc::vec![1u8; 2 * PART_LEN];
        let mut parts = AttachmentPart::split([9; 32], &sealed);
        parts[1].bytes[0] ^= 1;
        assert_eq!(b.received_part(a_id, parts.remove(0)), Vec::new());
        let out = b.received_part(a_id, parts.remove(0));
        assert_eq!(out, alloc::vec![Output::Rejected(RejectReason::Malformed)]);
        assert!(b.blobs.is_empty() && b.partials.is_empty());
    }

    /// Plays the runtime between two agents in one encounter (B is handle 1 at A, A is handle 2
    /// at B): carries
    /// encounter messages, serves A's stored blobs, writes B's partial files and checks them.
    struct Link {
        a_disk: BTreeMap<[u8; 32], Vec<u8>>,
        b_disk: BTreeMap<[u8; 32], Vec<u8>>,
        /// What each side produced, for failure messages.
        log: Vec<String>,
    }

    impl Link {
        fn run(
            &mut self,
            a: &mut Agent,
            ra: &mut ChaCha20Rng,
            b: &mut Agent,
            rb: &mut ChaCha20Rng,
            first: Vec<(bool, Input)>,
        ) {
            let mut queue = alloc::collections::VecDeque::from(first);
            let mut steps = 0;
            while let Some((to_a, input)) = queue.pop_front() {
                steps += 1;
                assert!(steps < 10_000, "no progress");
                let out = if to_a {
                    a.handle(input, ra)
                } else {
                    b.handle(input, rb)
                };
                for o in out.iter().filter(|o| !matches!(o, Output::Persist(_))) {
                    self.log
                        .push(alloc::format!("{} -> {o:?}", if to_a { "A" } else { "B" }));
                }
                let other = PeerHandle(if to_a { 2 } else { 1 });
                for o in out {
                    match o {
                        Output::Encounter { msg, .. } => {
                            queue.push_back((!to_a, Input::EncounterMessage { peer: other, msg }));
                        }
                        Output::SendBlob {
                            address,
                            offset,
                            len,
                            total,
                            ..
                        } => {
                            let blob = &self.a_disk[&address];
                            let start = usize::try_from(offset).unwrap();
                            let end = usize::try_from(offset + len).unwrap();
                            for piece in blob[start..end].chunks(MAX_BLOB_PART) {
                                let at = piece.as_ptr() as usize - blob.as_ptr() as usize;
                                queue.push_back((
                                    false,
                                    Input::EncounterMessage {
                                        // At B, A is handle 2.
                                        peer: PeerHandle(2),
                                        msg: EncounterMsg::BlobPart {
                                            address,
                                            offset: at as u64,
                                            total,
                                            bytes: piece.to_vec(),
                                        },
                                    },
                                ));
                            }
                        }
                        Output::Persist(StorageOp::WriteBlob {
                            address,
                            offset,
                            bytes,
                        }) => {
                            let f = self.b_disk.entry(address).or_default();
                            let start = usize::try_from(offset).unwrap();
                            let end = start + bytes.len();
                            if f.len() < end {
                                f.resize(end, 0);
                            }
                            f[start..end].copy_from_slice(&bytes);
                        }
                        Output::Persist(StorageOp::FinishBlob { address, .. }) => {
                            let ok = ContentAddress::of(&self.b_disk[&address]).0 == address;
                            queue.push_back((false, Input::BlobChecked { address, ok }));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// A sends B a large file sealed ahead (as the runtime does); returns the file and the
    /// harness holding A's copy.
    fn send_large(
        a: &mut Agent,
        ra: &mut ChaCha20Rng,
        b: &mut Agent,
        rb: &mut ChaCha20Rng,
        b_id: ContactId,
    ) -> (Vec<u8>, Link) {
        let file: Vec<u8> = (0..1_500_000u32).map(|i| (i % 253) as u8).collect();
        let key = FileKey::generate(ra);
        let sealed = seal(&key, &file).unwrap();
        let address = ContentAddress::of(&sealed).0;
        let out = a.handle(
            Input::UserSendFiles {
                contact: b_id,
                text: "big".into(),
                files: alloc::vec![NewAttachment {
                    name: "video.mp4".into(),
                    media_type: "video/mp4".into(),
                    data: AttachmentData::Stored {
                        address,
                        size: file.len() as u64,
                        key,
                    },
                    preview: None,
                }],
            },
            ra,
        );
        // Nothing to store or send in parts: the runtime already holds the blob.
        assert!(
            !out.iter()
                .any(|o| matches!(o, Output::Persist(StorageOp::PutBlob { .. })))
        );
        assert_eq!(a.outbox.len(), 1);
        for bytes in bundles(a) {
            let _ = deliver(b, rb, bytes);
        }
        let link = Link {
            a_disk: BTreeMap::from([(address, sealed)]),
            b_disk: BTreeMap::new(),
            log: Vec::new(),
        };
        (file, link)
    }

    fn meet() -> Vec<(bool, Input)> {
        alloc::vec![
            (
                true,
                Input::EncounterStarted {
                    peer: PeerHandle(1),
                    cla: ClaId::Iroh,
                },
            ),
            (
                false,
                Input::EncounterStarted {
                    peer: PeerHandle(2),
                    cla: ClaId::Iroh,
                },
            ),
        ]
    }

    #[test]
    fn a_large_file_is_pulled_over_a_direct_encounter() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let (file, mut link) = send_large(&mut a, &mut ra, &mut b, &mut rb, b_id);
        let a_id = b.contacts()[0].id;
        let waiting = &b.messages(a_id)[0];
        assert!(matches!(
            waiting.attachments[0].state,
            AttachmentState::Receiving { have: 0, .. }
        ));
        assert_eq!(
            a.messages(b_id)[0].attachments[0].state,
            AttachmentState::Ready
        );

        link.run(&mut a, &mut ra, &mut b, &mut rb, meet());

        let msg = &b.messages(a_id)[0];
        assert_eq!(
            msg.attachments[0].state,
            AttachmentState::Ready,
            "{}",
            link.log.join("\n")
        );
        // The channel that carried the file is shown on the message.
        assert!(msg.channels.iter().any(|u| u.cla == ClaId::Iroh));
        let secret = b.attachment(msg.id, 0).unwrap();
        let blob = &link.b_disk[&secret.address];
        assert_eq!(&*open(&secret.key, blob, secret.size).unwrap(), &file[..]);
        assert!(b.pulls.is_empty());
    }

    #[test]
    fn a_pull_resumes_after_a_restart() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let (_, mut link) = send_large(&mut a, &mut ra, &mut b, &mut rb, b_id);
        // One window arrives, then the encounter ends and B restarts.
        let address = *b.pulls.keys().next().unwrap();
        b.pulls.get_mut(&address).unwrap().have = PULL_WINDOW;
        let window = usize::try_from(PULL_WINDOW).unwrap();
        let first_window = link.a_disk[&address][..window].to_vec();
        link.b_disk.insert(address, first_window);
        let mut b = Agent::restore(&b.snapshot(), NOW).unwrap();
        assert_eq!(b.pulls[&address].have, PULL_WINDOW);
        link.run(&mut a, &mut ra, &mut b, &mut rb, meet());
        assert!(b.blobs.contains(&address));
    }

    #[test]
    fn nobody_but_the_recipient_gets_the_blob() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let (_, _link) = send_large(&mut a, &mut ra, &mut b, &mut rb, b_id);
        let address = *b.pulls.keys().next().unwrap();
        let _ = a.handle(
            Input::EncounterStarted {
                peer: PeerHandle(9),
                cla: ClaId::Iroh,
            },
            &mut ra,
        );
        let _ = a.handle(
            Input::EncounterMessage {
                peer: PeerHandle(9),
                msg: EncounterMsg::Hello {
                    version: mem_proto::encounter::ENCOUNTER_VERSION,
                    ephemeral_id: [4; 16],
                },
            },
            &mut ra,
        );
        // A stranger who learned the address cannot make a proof.
        let out = a.handle(
            Input::EncounterMessage {
                peer: PeerHandle(9),
                msg: EncounterMsg::BlobWant {
                    address,
                    offset: 0,
                    proof: [0; 16],
                },
            },
            &mut ra,
        );
        assert!(!out.iter().any(|o| matches!(o, Output::SendBlob { .. })));
    }

    #[test]
    fn a_pull_stays_within_the_floor() {
        let (mut a, mut ra, mut b, mut rb, b_id) = paired();
        let (_, mut link) = send_large(&mut a, &mut ra, &mut b, &mut rb, b_id);
        let a_id = b.contacts()[0].id;
        // B allows only face-to-face channels for A: the local network (P1) is not used.
        let _ = b.handle(
            Input::SetFloor {
                contact: a_id,
                floor: mem_cla::PrivacyGrade::P0,
            },
            &mut rb,
        );
        link.run(&mut a, &mut ra, &mut b, &mut rb, meet());
        assert!(link.b_disk.is_empty());
        assert!(matches!(
            b.messages(a_id)[0].attachments[0].state,
            AttachmentState::Receiving { .. }
        ));
    }
}
