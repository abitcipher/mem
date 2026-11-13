//! Editing sent 1:1 texts, within the recipient's limit.
//!
//! The recipient decides how late an edit is accepted (globally, or per conversation) and tells
//! the contact ([`CONTENT_TYPE_EDIT_LIMIT`]); the sender offers "edit" only within it. The
//! recipient measures on its own clock, from the original's arrival to the edit's arrival, and
//! drops a late edit silently. No earlier versions are kept: editing is for typos between
//! well-meaning people.

use alloc::string::String;
use alloc::vec::Vec;

use mem_proto::edit::{
    CONTENT_TYPE_DELETE, CONTENT_TYPE_EDIT, CONTENT_TYPE_EDIT_LIMIT, EDIT_LIMIT_POSITIONS, Edit,
    encode_delete, encode_edit_limit,
};
use mem_proto::inner::MAX_TEXT_LEN;
use rand_core::CryptoRng;

use super::{Agent, Message, MsgDirection, Output, StorageOp};
use crate::model::{ContactId, MessageKind, MsgId, RejectReason};

const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// The edit window of each position between "always" (0) and "never" (12).
const WINDOWS_MS: [u64; 11] = [
    5 * MINUTE,
    15 * MINUTE,
    30 * MINUTE,
    HOUR,
    2 * HOUR,
    8 * HOUR,
    12 * HOUR,
    DAY,
    5 * DAY,
    7 * DAY,
    31 * DAY,
];

/// Whether an edit `elapsed_ms` after the original is within the limit at `position`.
#[must_use]
pub const fn edit_allowed(position: u8, elapsed_ms: u64) -> bool {
    match position {
        0 => true,
        p if p >= EDIT_LIMIT_POSITIONS - 1 => false,
        p => elapsed_ms < WINDOWS_MS[(p - 1) as usize],
    }
}

/// Our own 1:1 text that may be edited.
fn own_text(m: &Message) -> bool {
    m.direction == MsgDirection::Outgoing && m.kind == MessageKind::Text && m.group.is_none()
}

impl Agent {
    /// How late we accept `contact`'s edits: its own limit, else the global one.
    fn effective_edit_limit(&self, contact: ContactId) -> u8 {
        self.contacts
            .get(&contact)
            .and_then(|c| c.prefs.edit_limit)
            .unwrap_or(self.edit_limit)
    }

    /// Whether our message `m` may still be edited: an own 1:1 text within the limit its
    /// recipient told us (judged from our send time, which errs on the safe side).
    pub(super) fn editable(&self, m: &Message) -> bool {
        own_text(m)
            && self.contacts.get(&m.contact).is_some_and(|c| {
                edit_allowed(
                    c.prefs.their_edit_limit,
                    self.now_unix_ms.saturating_sub(m.created_unix_ms),
                )
            })
    }

    /// The user corrects an own message: shown at once here, sent to the contact.
    pub(super) fn edit_message(
        &mut self,
        message: MsgId,
        text: &str,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_TEXT_LEN {
            return alloc::vec![Output::Rejected(RejectReason::InvalidText)];
        }
        let Some(m) = self.messages.iter().find(|m| m.id == message) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        };
        if !self.editable(m) {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        }
        let contact = m.contact;
        let Ok(content) = (Edit {
            message_id: message.0,
            text: String::from(text),
        })
        .encode() else {
            return alloc::vec![Output::Rejected(RejectReason::InvalidText)];
        };
        let Some(edit_id) = self.queue_hidden(contact, CONTENT_TYPE_EDIT, content, rng) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownContact)];
        };
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == message) {
            m.text = String::from(text);
            m.edit = Some(edit_id);
        }
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::StatusChanged {
                message,
                status: self
                    .messages
                    .iter()
                    .find(|m| m.id == message)
                    .map_or(crate::model::DeliveryStatus::Sent, |m| m.status,),
            },
        ];
        out.extend(self.flush_message(edit_id));
        out
    }

    /// An edit from `contact` (its id `edit_id`): applied to their earlier text if it arrived
    /// within our limit and is newer than an edit already applied; otherwise dropped silently.
    pub(super) fn received_edit(
        &mut self,
        contact: ContactId,
        edit: Edit,
        edit_id: MsgId,
    ) -> Vec<Output> {
        let limit = self.effective_edit_limit(contact);
        let now = self.now_unix_ms;
        let Some(m) = self.messages.iter_mut().find(|m| {
            m.id.0 == edit.message_id
                && m.contact == contact
                && m.direction == MsgDirection::Incoming
                && m.kind == MessageKind::Text
                && m.group.is_none()
        }) else {
            return Vec::new();
        };
        let arrived = m
            .channels
            .first()
            .map_or(m.created_unix_ms, |u| u.at_unix_ms);
        // UUIDv7 ids order by creation: a late copy of an older edit never wins.
        let newer = m.edit.is_none_or(|e| e.0 < edit_id.0);
        if !newer || !edit_allowed(limit, now.saturating_sub(arrived)) {
            return Vec::new();
        }
        m.text = edit.text;
        m.edit = Some(edit_id);
        alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::StatusChanged {
                message: m.id,
                status: m.status,
            },
        ]
    }

    /// Sets how late we accept edits: globally (`contact` `None`, `position` required) or for
    /// one conversation (`position` `None`: as global). Contacts whose effective limit changed
    /// are told.
    pub(super) fn set_edit_limit(
        &mut self,
        contact: Option<ContactId>,
        position: Option<u8>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if position.is_some_and(|p| p >= EDIT_LIMIT_POSITIONS) {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        let before: Vec<(ContactId, u8)> = self
            .contacts
            .keys()
            .map(|c| (*c, self.effective_edit_limit(*c)))
            .collect();
        match (contact, position) {
            (None, Some(p)) => self.edit_limit = p,
            (Some(c), p) => match self.contacts.get_mut(&c) {
                Some(x) if !x.group_only => x.prefs.edit_limit = p,
                _ => return alloc::vec![Output::Rejected(RejectReason::UnknownContact)],
            },
            (None, None) => return alloc::vec![Output::Rejected(RejectReason::Malformed)],
        }
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        for (c, old) in before {
            let now = self.effective_edit_limit(c);
            let tellable = self
                .contacts
                .get(&c)
                .is_some_and(|x| !x.group_only && !x.requested)
                && !self.is_self(c);
            if now == old || !tellable {
                continue;
            }
            let Ok(content) = encode_edit_limit(now) else {
                continue;
            };
            if let Some(id) = self.queue_hidden(c, CONTENT_TYPE_EDIT_LIMIT, content, rng) {
                out.extend(self.flush_message(id));
            }
        }
        out
    }

    /// Deletes a message here; with `for_both`, also at the contact (only our own text, within
    /// the limit the contact told us, like an edit). A message not sent yet leaves the outbox.
    pub(super) fn delete_message(
        &mut self,
        message: MsgId,
        for_both: bool,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(m) = self.messages.iter().find(|m| m.id == message) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        };
        if for_both && !self.editable(m) {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        }
        let contact = m.contact;
        let mut out = Vec::new();
        if for_both
            && let Some(id) =
                self.queue_hidden(contact, CONTENT_TYPE_DELETE, encode_delete(&message.0), rng)
        {
            out.extend(self.flush_message(id));
        }
        self.messages.retain(|m| m.id != message);
        self.outbox.remove(&message);
        out.splice(
            0..0,
            [
                Output::Persist(StorageOp::SaveSnapshot),
                Output::StatusChanged {
                    message,
                    status: crate::model::DeliveryStatus::Failed,
                },
            ],
        );
        out
    }

    /// The contact withdraws its earlier text: removed if it arrived within our limit,
    /// otherwise ignored silently (deleting counts as editing).
    fn received_delete(&mut self, contact: ContactId, message: [u8; 16]) -> Vec<Output> {
        let limit = self.effective_edit_limit(contact);
        let now = self.now_unix_ms;
        let Some(m) = self.messages.iter().find(|m| {
            m.id.0 == message
                && m.contact == contact
                && m.direction == MsgDirection::Incoming
                && m.kind == MessageKind::Text
                && m.group.is_none()
        }) else {
            return Vec::new();
        };
        let arrived = m
            .channels
            .first()
            .map_or(m.created_unix_ms, |u| u.at_unix_ms);
        if !edit_allowed(limit, now.saturating_sub(arrived)) {
            return Vec::new();
        }
        let id = m.id;
        self.messages.retain(|m| m.id != id);
        alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::StatusChanged {
                message: id,
                status: crate::model::DeliveryStatus::Failed,
            },
        ]
    }

    /// An edit, a deletion or an edit limit from `contact` (other content is ignored here).
    pub(super) fn received_edit_content(
        &mut self,
        contact: ContactId,
        content: super::OpenedContent,
        id: MsgId,
    ) -> Vec<Output> {
        match content {
            super::OpenedContent::Edit(edit) => self.received_edit(contact, edit, id),
            super::OpenedContent::EditLimit(p) => self.received_edit_limit(contact, p),
            super::OpenedContent::Delete(m) => self.received_delete(contact, m),
            _ => Vec::new(),
        }
    }

    /// `contact` told us how late it accepts our edits.
    pub(super) fn received_edit_limit(&mut self, contact: ContactId, position: u8) -> Vec<Output> {
        match self.contacts.get_mut(&contact) {
            Some(c) => {
                c.prefs.their_edit_limit = position;
                alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
            }
            None => Vec::new(),
        }
    }

    /// How late we accept contacts' edits by default (a scale position).
    #[must_use]
    pub const fn edit_limit(&self) -> u8 {
        self.edit_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_runs_from_always_to_never() {
        assert!(edit_allowed(0, u64::MAX));
        assert!(edit_allowed(1, 4 * MINUTE));
        assert!(!edit_allowed(1, 5 * MINUTE));
        assert!(edit_allowed(11, 30 * DAY));
        assert!(!edit_allowed(11, 31 * DAY));
        assert!(!edit_allowed(12, 0));
    }

    mod agents {
        use super::*;
        use crate::agent::Input;
        use mem_cla::ClaId;
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;

        const NOW: u64 = 1_792_022_400_000;

        fn agent(seed: u8) -> (Agent, ChaCha20Rng) {
            let mut rng = ChaCha20Rng::from_seed([seed; 32]);
            let mut a = Agent::new(NOW, &mut rng);
            let _ = a.handle(Input::Tick { wall_unix_ms: NOW }, &mut rng);
            (a, rng)
        }

        struct Pair {
            a: Agent,
            ra: ChaCha20Rng,
            b: Agent,
            rb: ChaCha20Rng,
            b_id: ContactId,
            a_id: ContactId,
        }

        fn paired() -> Pair {
            let (mut a, mut ra) = agent(1);
            let (mut b, mut rb) = agent(2);
            let (ca, cb) = (a.own_card().unwrap(), b.own_card().unwrap());
            let _ = a.handle(
                Input::ImportCard {
                    cla: ClaId::Qr,
                    bytes: cb,
                },
                &mut ra,
            );
            let _ = b.handle(
                Input::ImportCard {
                    cla: ClaId::Qr,
                    bytes: ca,
                },
                &mut rb,
            );
            let (b_id, a_id) = (a.contacts()[0].id, b.contacts()[0].id);
            Pair {
                a,
                ra,
                b,
                rb,
                b_id,
                a_id,
            }
        }

        impl Pair {
            /// Hands everything one side holds to the other (copies already taken are refused).
            fn carry(&mut self, to_b: bool) {
                let (from, to, rng) = if to_b {
                    (&self.a, &mut self.b, &mut self.rb)
                } else {
                    (&self.b, &mut self.a, &mut self.ra)
                };
                let bundles: Vec<Vec<u8>> =
                    from.outbox.values().map(|o| o.bundle.encode()).collect();
                for bytes in bundles {
                    let _ = to.handle(
                        Input::Received {
                            cla: ClaId::Qr,
                            bytes,
                        },
                        rng,
                    );
                }
            }

            fn tick(&mut self, ms: u64) {
                let _ = self
                    .a
                    .handle(Input::Tick { wall_unix_ms: ms }, &mut self.ra);
                let _ = self
                    .b
                    .handle(Input::Tick { wall_unix_ms: ms }, &mut self.rb);
            }
        }

        #[test]
        fn a_correction_reaches_the_recipient() {
            let mut p = paired();
            let _ = p.a.handle(
                Input::UserSend {
                    contact: p.b_id,
                    text: "helo".into(),
                },
                &mut p.ra,
            );
            p.carry(true);
            let id = p.a.messages(p.b_id)[0].id;
            assert!(p.a.messages(p.b_id)[0].editable);
            let _ = p.a.handle(
                Input::EditMessage {
                    message: id,
                    text: "hello".into(),
                },
                &mut p.ra,
            );
            p.carry(true);
            let got = &p.b.messages(p.a_id)[0];
            assert_eq!((got.text.as_str(), got.edited), ("hello", true));
            let mine = &p.a.messages(p.b_id)[0];
            assert_eq!((mine.text.as_str(), mine.edited), ("hello", true));
        }

        #[test]
        fn the_recipients_limit_hides_edit_and_drops_late_edits() {
            let mut p = paired();
            // B accepts A's edits for 5 minutes and tells A.
            let _ = p.b.handle(
                Input::SetEditLimit {
                    contact: Some(p.a_id),
                    position: Some(1),
                },
                &mut p.rb,
            );
            p.carry(false);
            assert_eq!(p.a.contacts.get(&p.b_id).unwrap().prefs.their_edit_limit, 1);

            let _ = p.a.handle(
                Input::UserSend {
                    contact: p.b_id,
                    text: "first".into(),
                },
                &mut p.ra,
            );
            p.carry(true);
            let id = p.a.messages(p.b_id)[0].id;
            // A edits within its view of the limit, but the edit reaches B after 6 minutes.
            let _ = p.a.handle(
                Input::EditMessage {
                    message: id,
                    text: "second".into(),
                },
                &mut p.ra,
            );
            p.tick(NOW + 6 * MINUTE);
            p.carry(true);
            assert_eq!(p.b.messages(p.a_id)[0].text, "first");
            // After the limit A no longer offers "edit", and the core refuses it.
            assert!(!p.a.messages(p.b_id)[0].editable);
            let out = p.a.handle(
                Input::EditMessage {
                    message: id,
                    text: "third".into(),
                },
                &mut p.ra,
            );
            assert_eq!(
                out,
                alloc::vec![Output::Rejected(RejectReason::UnknownMessage)]
            );
        }

        #[test]
        fn deleting_for_both_follows_the_same_limit() {
            let mut p = paired();
            let _ = p.b.handle(
                Input::SetEditLimit {
                    contact: Some(p.a_id),
                    position: Some(1),
                },
                &mut p.rb,
            );
            p.carry(false);
            for text in ["one", "two"] {
                let _ = p.a.handle(
                    Input::UserSend {
                        contact: p.b_id,
                        text: text.into(),
                    },
                    &mut p.ra,
                );
            }
            p.carry(true);
            let ids: Vec<MsgId> = p.a.messages(p.b_id).iter().map(|m| m.id).collect();
            // Within the limit: gone on both sides.
            let _ = p.a.handle(
                Input::DeleteMessage {
                    message: ids[0],
                    for_both: true,
                },
                &mut p.ra,
            );
            p.carry(true);
            assert_eq!(p.a.messages(p.b_id).len(), 1);
            assert_eq!(p.b.messages(p.a_id).len(), 1);
            // A deletion that reaches B after its limit leaves B's copy alone.
            let _ = p.a.handle(
                Input::DeleteMessage {
                    message: ids[1],
                    for_both: true,
                },
                &mut p.ra,
            );
            p.tick(NOW + 6 * MINUTE);
            p.carry(true);
            assert_eq!(p.a.messages(p.b_id).len(), 0);
            assert_eq!(p.b.messages(p.a_id)[0].text, "two");
        }

        #[test]
        fn an_older_edit_never_wins() {
            let mut p = paired();
            let _ = p.a.handle(
                Input::UserSend {
                    contact: p.b_id,
                    text: "a".into(),
                },
                &mut p.ra,
            );
            p.carry(true);
            let id = p.a.messages(p.b_id)[0].id;
            let _ = p.a.handle(
                Input::EditMessage {
                    message: id,
                    text: "b".into(),
                },
                &mut p.ra,
            );
            let older: Vec<Vec<u8>> = p.a.outbox.values().map(|o| o.bundle.encode()).collect();
            p.tick(NOW + 1000);
            let _ = p.a.handle(
                Input::EditMessage {
                    message: id,
                    text: "c".into(),
                },
                &mut p.ra,
            );
            let all: Vec<Vec<u8>> = p.a.outbox.values().map(|o| o.bundle.encode()).collect();
            // The newer edit first, then the older one.
            for bytes in all
                .iter()
                .filter(|b| !older.contains(b))
                .chain(older.iter())
            {
                let _ = p.b.handle(
                    Input::Received {
                        cla: ClaId::Qr,
                        bytes: bytes.clone(),
                    },
                    &mut p.rb,
                );
            }
            assert_eq!(p.b.messages(p.a_id)[0].text, "c");
        }
    }
}
