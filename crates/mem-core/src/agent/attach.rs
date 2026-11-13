//! Attachments: each file sealed once under its own key, its blob sent as hidden part messages,
//! and parts from a contact reassembled and checked against the blob's content address.
//!
//! Parts are ordinary sealed bundles (content type [`CONTENT_TYPE_ATTACHMENT_PART`]): they take
//! the same channels, grades, consent, acks, retransmission and purge as text messages. This
//! version carries small blobs only ([`SMALL_SEALED_MAX`]); incoming parts wait in the state
//! until their blob is complete, within [`MAX_PENDING_PART_BYTES`].

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use mem_crypto::attachment::{ContentAddress, FileKey, SEALED_CHUNK_LEN, seal, sealed_len};
use mem_proto::BundleKind;
use mem_proto::attachment::{
    AttachmentPart, AttachmentRef, CONTENT_TYPE_ATTACHMENT_PART, MAX_ATTACHMENTS, PART_LEN,
};
use rand_core::CryptoRng;

use super::{Agent, Message, MsgDirection, Output, StorageOp};
use crate::model::{
    AttachmentState, AttachmentView, ContactId, MsgId, NewAttachment, RejectReason,
};

/// Largest sealed blob this version sends and accepts: a file padded to 256 KiB (four chunks).
pub const SMALL_SEALED_MAX: u64 = 4 * SEALED_CHUNK_LEN as u64;
/// Incoming parts waiting for the rest of their blob, in bytes; the oldest blob is dropped
/// beyond this.
pub const MAX_PENDING_PART_BYTES: usize = 4 * 1024 * 1024;

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

/// A file sealed for sending, with its blob.
pub(crate) struct SealedFile {
    pub(super) reference: AttachmentRef,
    pub(super) sealed: Vec<u8>,
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
                let size = f.data.len() as u64;
                if sealed_len(size).is_none_or(|l| l > SMALL_SEALED_MAX) {
                    return Err(RejectReason::FileTooLarge);
                }
                let key = FileKey::generate(rng);
                let sealed = seal(&key, &f.data).map_err(|_| RejectReason::FileTooLarge)?;
                let reference = AttachmentRef {
                    address: ContentAddress::of(&sealed).0,
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
            for part in AttachmentPart::split(f.reference.address, &f.sealed) {
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
            .map(|f| {
                self.blobs.insert(f.reference.address);
                Output::Persist(StorageOp::PutBlob {
                    address: f.reference.address,
                    sealed: f.sealed,
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
                        have: self
                            .partials
                            .get(&r.address)
                            .map_or(0, |p| u16::try_from(p.parts.len()).unwrap_or(u16::MAX)),
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
            data: (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect(),
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
        assert_eq!(&*file, &sent.data[..]);
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
}
