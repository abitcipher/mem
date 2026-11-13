//! Small groups over DTN by pairwise fan-out.
//!
//! The admin (creator) alone changes membership. An invitation is a group state with role
//! *invited*; the invitee joins by replying with its own card; the admin then sends every member
//! the next epoch with the joined members' cards. Two members who are not contacts derive their
//! pair secret from the two cards in the same state, and keep each other as
//! group-only contacts: usable for this group's traffic, never shown as contacts, refused for 1:1
//! messages.
//!
//! Every group message and control message is an ordinary hidden outbox item to one member, so
//! consent, floors, acks and retransmission apply per copy, exactly as for 1:1 messages.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use mem_crypto::purge::commit;
use mem_crypto::{ContactSecret, PairingNonce, SenderPurgeKey};
use mem_proto::BundleKind;
use mem_proto::group::{
    CONTENT_TYPE_GROUP_REPLY, CONTENT_TYPE_GROUP_STATE, CONTENT_TYPE_GROUP_TEXT, GroupReply,
    GroupRole, GroupState, GroupText, MAX_GROUP_MEMBERS, MAX_GROUP_NAME_LEN,
};
use mem_proto::inner::MAX_TEXT_LEN;
use rand_core::CryptoRng;

use super::{
    Agent, Contact, Message, MsgDirection, OutboxItem, Output, Recipient, Sealing, StorageOp,
    verify_card,
};
use crate::model::{
    ChannelUse, ContactId, DeliveryStatus, GroupId, GroupMemberView, GroupStatus, GroupView,
    MemberRole, MessageKind, MessageView, MsgId, RejectReason, TrustLevel,
};

/// Most groups a device keeps.
pub const MAX_GROUPS: usize = 64;
/// Most invitations waiting for an answer.
pub const MAX_INVITATIONS: usize = 16;

/// A group as this device knows it.
pub(crate) struct Group {
    pub(crate) name: String,
    /// The admin's identity key (fixed at creation).
    pub(crate) admin: [u8; 32],
    pub(crate) epoch: u32,
    /// Cards of the joined members as in the last state, the admin first.
    pub(crate) cards: Vec<Vec<u8>>,
    /// Their identity keys, in the same order.
    pub(crate) members: Vec<[u8; 32]>,
    pub(crate) status: GroupStatus,
    /// Admin only: contacts invited who have not answered.
    pub(crate) invited: Vec<[u8; 32]>,
    pub(crate) created_unix_ms: u64,
}

/// What arrived from a member, after decryption and the signature check.
pub(crate) enum GroupContent {
    Text(GroupText),
    State(GroupState),
    Reply(GroupReply),
}

impl GroupContent {
    pub(crate) fn decode(content_type: u64, bytes: &[u8]) -> Result<Self, RejectReason> {
        let bad = |_| RejectReason::Malformed;
        match content_type {
            CONTENT_TYPE_GROUP_TEXT => GroupText::decode(bytes).map(Self::Text).map_err(bad),
            CONTENT_TYPE_GROUP_STATE => GroupState::decode(bytes).map(Self::State).map_err(bad),
            CONTENT_TYPE_GROUP_REPLY => GroupReply::decode(bytes).map(Self::Reply).map_err(bad),
            _ => Err(RejectReason::Unsupported),
        }
    }
}

/// A verified member card.
struct MemberCard {
    identity: mem_crypto::IdentityPublic,
    mailbox: mem_crypto::MailboxPublic,
    nonce: PairingNonce,
    name: Option<String>,
}

/// Verifies every card of a state; all or nothing.
fn verify_cards(cards: &[Vec<u8>]) -> Result<Vec<MemberCard>, RejectReason> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(cards.len());
    for c in cards {
        let (identity, mailbox, nonce, name) = verify_card(c)?;
        if !seen.insert(*identity.as_bytes()) {
            return Err(RejectReason::Malformed);
        }
        out.push(MemberCard {
            identity,
            mailbox,
            nonce,
            name,
        });
    }
    Ok(out)
}

impl Agent {
    /// Dispatches the group inputs.
    pub(super) fn group_input(
        &mut self,
        input: crate::Input,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        use crate::Input;
        let result = match input {
            Input::CreateGroup { name, members } => self.create_group(&name, &members, rng),
            Input::InviteToGroup { group, contact } => self.invite(group, contact, rng),
            Input::RemoveFromGroup { group, member } => self.remove_member(group, member, rng),
            Input::AcceptGroupInvitation(group) => self.answer_invitation(group, true, rng),
            Input::DeclineGroupInvitation(group) => self.answer_invitation(group, false, rng),
            Input::LeaveGroup(group) => self.leave_group(group, rng),
            Input::SendGroupText { group, text } => self.send_group_text(group, &text, rng),
            Input::AddMemberAsContact(id) => match self.contacts.get_mut(&id) {
                Some(c) if c.group_only => {
                    c.group_only = false;
                    Ok(alloc::vec![
                        Output::ContactPaired {
                            contact: id,
                            updated: false,
                            unchanged: false,
                        },
                        Output::Persist(StorageOp::SaveSnapshot),
                    ])
                }
                Some(_) => Err(RejectReason::AlreadyContact),
                None => Err(RejectReason::UnknownContact),
            },
            _ => Err(RejectReason::Unsupported),
        };
        result.unwrap_or_else(|r| alloc::vec![Output::Rejected(r)])
    }

    fn me(&self) -> [u8; 32] {
        *self.identity.public().as_bytes()
    }

    /// A full contact we may invite: not group-only, not a pending request.
    fn full_contact(&self, id: ContactId) -> Result<&Contact, RejectReason> {
        match self.contacts.get(&id) {
            Some(c) if !c.group_only && !c.requested => Ok(c),
            _ => Err(RejectReason::UnknownContact),
        }
    }

    fn create_group(
        &mut self,
        name: &str,
        members: &[ContactId],
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        let name = clean_name(name)?;
        let invitees: BTreeSet<ContactId> = members.iter().copied().collect();
        if invitees.is_empty() || invitees.len() >= MAX_GROUP_MEMBERS {
            return Err(RejectReason::GroupFull);
        }
        for id in &invitees {
            self.full_contact(*id)?;
        }
        if self.groups.len() >= MAX_GROUPS {
            return Err(RejectReason::GroupFull);
        }
        let own_card = self.own_card()?;
        let mut id = [0u8; 16];
        rng.fill_bytes(&mut id);
        let group = GroupId(id);
        self.groups.insert(
            group,
            Group {
                name,
                admin: self.me(),
                epoch: 1,
                cards: alloc::vec![own_card],
                members: alloc::vec![self.me()],
                status: GroupStatus::Member,
                invited: invitees.iter().map(|c| c.0).collect(),
                created_unix_ms: self.now_unix_ms,
            },
        );
        let mut out = alloc::vec![Output::GroupChanged { group }];
        for c in invitees {
            out.extend(self.send_state(group, c, GroupRole::Invited, rng));
        }
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    /// The admin's group, if we are its admin and it is live.
    fn admin_group(&self, group: GroupId) -> Result<&Group, RejectReason> {
        match self.groups.get(&group) {
            Some(g) if g.admin == self.me() && g.status == GroupStatus::Member => Ok(g),
            Some(_) => Err(RejectReason::NotGroupAdmin),
            None => Err(RejectReason::UnknownGroup),
        }
    }

    fn invite(
        &mut self,
        group: GroupId,
        contact: ContactId,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        self.full_contact(contact)?;
        let g = self.admin_group(group)?;
        if g.members.contains(&contact.0) || g.invited.contains(&contact.0) {
            return Err(RejectReason::Duplicate);
        }
        if g.members.len() + g.invited.len() >= MAX_GROUP_MEMBERS {
            return Err(RejectReason::GroupFull);
        }
        if let Some(g) = self.groups.get_mut(&group) {
            g.invited.push(contact.0);
        }
        let mut out = self.send_state(group, contact, GroupRole::Invited, rng);
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    fn remove_member(
        &mut self,
        group: GroupId,
        member: ContactId,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        let g = self.admin_group(group)?;
        let was_member = g.members.contains(&member.0);
        if member.0 == self.me() || (!was_member && !g.invited.contains(&member.0)) {
            return Err(RejectReason::UnknownContact);
        }
        let mut out = Vec::new();
        if let Some(g) = self.groups.get_mut(&group) {
            g.invited.retain(|m| *m != member.0);
            if was_member {
                drop_member(g, &member.0);
            }
        }
        out.extend(self.send_state(group, member, GroupRole::Removed, rng));
        if was_member {
            out.extend(self.send_state_to_members(group, rng));
        }
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    fn answer_invitation(
        &mut self,
        group: GroupId,
        accept: bool,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        let g = self.groups.get(&group).ok_or(RejectReason::UnknownGroup)?;
        if g.status != GroupStatus::Invited {
            return Err(RejectReason::UnknownGroup);
        }
        let admin = ContactId(g.admin);
        let card = if accept { Some(self.own_card()?) } else { None };
        let mut out = self.send_reply(group, admin, card, rng);
        if accept {
            if let Some(g) = self.groups.get_mut(&group) {
                g.status = GroupStatus::Joining;
            }
        } else {
            self.groups.remove(&group);
        }
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    fn leave_group(
        &mut self,
        group: GroupId,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        let g = self.groups.get(&group).ok_or(RejectReason::UnknownGroup)?;
        let mut out = Vec::new();
        if g.admin == self.me() {
            // The admin leaving ends the group for everyone (v1 has no hand-over).
            if g.status != GroupStatus::Member {
                return Err(RejectReason::UnknownGroup);
            }
            let others: Vec<[u8; 32]> = g
                .members
                .iter()
                .chain(&g.invited)
                .filter(|m| **m != self.me())
                .copied()
                .collect();
            if let Some(g) = self.groups.get_mut(&group) {
                g.epoch = g.epoch.saturating_add(1);
                g.status = GroupStatus::Left;
                g.invited.clear();
            }
            for m in others {
                out.extend(self.send_state(group, ContactId(m), GroupRole::Removed, rng));
            }
        } else {
            if !matches!(g.status, GroupStatus::Member | GroupStatus::Joining) {
                return Err(RejectReason::UnknownGroup);
            }
            let admin = ContactId(g.admin);
            out.extend(self.send_reply(group, admin, None, rng));
            if let Some(g) = self.groups.get_mut(&group) {
                g.status = GroupStatus::Left;
            }
        }
        self.prune_group_contacts();
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    /// One copy of the text per member; each goes out like a 1:1 message.
    fn send_group_text(
        &mut self,
        group: GroupId,
        text: &str,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<Output>, RejectReason> {
        let text = text.trim();
        if text.is_empty() || text.len() > MAX_TEXT_LEN {
            return Err(RejectReason::InvalidText);
        }
        let g = self.groups.get(&group).ok_or(RejectReason::UnknownGroup)?;
        if g.status != GroupStatus::Member {
            return Err(RejectReason::UnknownGroup);
        }
        let content = GroupText {
            group: group.0,
            epoch: g.epoch,
            text: String::from(text),
        }
        .encode()
        .map_err(|_| RejectReason::InvalidText)?;
        let me = self.me();
        let recipients: Vec<ContactId> = g
            .members
            .iter()
            .filter(|m| **m != me)
            .map(|m| ContactId(*m))
            .filter(|c| self.contacts.get(c).is_some_and(|c| !c.requested))
            .collect();
        if recipients.is_empty() {
            return Err(RejectReason::GroupEmpty);
        }
        let mut out = Vec::new();
        let mut fanout = None;
        for c in recipients {
            let Some(id) = self.queue_hidden(c, CONTENT_TYPE_GROUP_TEXT, content.clone(), rng)
            else {
                continue;
            };
            let first = *fanout.get_or_insert(id);
            self.messages.push(Message {
                id,
                contact: c,
                direction: MsgDirection::Outgoing,
                kind: MessageKind::Text,
                text: String::from(text),
                created_unix_ms: self.now_unix_ms,
                status: DeliveryStatus::Queued,
                channels: Vec::new(),
                group: Some(group),
                fanout: Some(first),
                attachments: Vec::new(),
            });
            out.extend(self.flush_message(id));
        }
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        Ok(out)
    }

    /// Sends the current state to one contact with `role`.
    fn send_state(
        &mut self,
        group: GroupId,
        to: ContactId,
        role: GroupRole,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(g) = self.groups.get(&group) else {
            return Vec::new();
        };
        let Ok(content) = (GroupState {
            group: group.0,
            epoch: g.epoch,
            name: g.name.clone(),
            cards: g.cards.clone(),
            role,
        })
        .encode() else {
            return Vec::new();
        };
        match self.queue_hidden(to, CONTENT_TYPE_GROUP_STATE, content, rng) {
            Some(id) => self.flush_message(id),
            None => Vec::new(),
        }
    }

    /// A new epoch's state to every member but us.
    fn send_state_to_members(&mut self, group: GroupId, rng: &mut impl CryptoRng) -> Vec<Output> {
        let me = self.me();
        let members: Vec<[u8; 32]> = self
            .groups
            .get(&group)
            .map(|g| g.members.iter().filter(|m| **m != me).copied().collect())
            .unwrap_or_default();
        let mut out = Vec::new();
        for m in members {
            out.extend(self.send_state(group, ContactId(m), GroupRole::Member, rng));
        }
        out
    }

    fn send_reply(
        &mut self,
        group: GroupId,
        admin: ContactId,
        card: Option<Vec<u8>>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Ok(content) = (GroupReply {
            group: group.0,
            card,
        })
        .encode() else {
            return Vec::new();
        };
        match self.queue_hidden(admin, CONTENT_TYPE_GROUP_REPLY, content, rng) {
            Some(id) => self.flush_message(id),
            None => Vec::new(),
        }
    }

    /// A hidden outbox item with this content for one contact: sent only automatically, within
    /// the floor or a standing rule, acknowledged and retried like a message. The content
    /// is kept so the item can be resealed to a newer key.
    pub(super) fn queue_hidden(
        &mut self,
        contact_id: ContactId,
        content_type: u64,
        content: Vec<u8>,
        rng: &mut impl CryptoRng,
    ) -> Option<MsgId> {
        let how = Hidden {
            lifetime_s: self.message_lifetime_s,
            ack: true,
            copies: self.spray_copies,
            device_copies: true,
        };
        self.queue_hidden_as(contact_id, content_type, content, how, rng)
    }

    /// As [`Self::queue_hidden`], with its lifetime, acknowledgement, copy budget and device
    /// copies chosen by the caller.
    pub(super) fn queue_hidden_as(
        &mut self,
        contact_id: ContactId,
        content_type: u64,
        content: Vec<u8>,
        how: Hidden,
        rng: &mut impl CryptoRng,
    ) -> Option<MsgId> {
        if self.purge_key.is_none() {
            self.purge_key = Some(SenderPurgeKey::generate(rng));
        }
        let (contact, purge_key) = (self.contacts.get(&contact_id)?, self.purge_key.as_ref()?);
        if contact.requested {
            return None;
        }
        let mut bundle_id = [0u8; 16];
        rng.fill_bytes(&mut bundle_id);
        let token = purge_key.derive_token(&bundle_id);
        let commitment = commit(&token);
        let acks = self.owed_acks(contact_id, None);
        let key_update = self.key_update_for(contact);
        let key_serial = key_update.as_ref().map(|k| k.serial);
        let list_serial = key_update
            .as_ref()
            .and_then(|k| k.device_list.as_ref())
            .map(|l| l.serial);
        let (sealed_serial, sealed_learned_ms) =
            (contact.mailbox_serial, contact.mailbox_learned_ms);
        let (bundle, id) = self
            .seal(
                contact,
                Sealing {
                    kind: BundleKind::Message,
                    content_type,
                    content: content.clone(),
                    attachments: Vec::new(),
                    ack_requested: Some(how.ack),
                    acks: acks.iter().map(|m| m.0).collect(),
                    lifetime_s: how.lifetime_s,
                    key_update,
                    id: None,
                    recipient: Recipient::Mailbox,
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
                copies: how.copies,
                courier_ok: false,
                key_serial,
                list_serial,
                sealed_serial,
                sealed_learned_ms,
                via_identity: false,
                content: Some((content_type, content)),
                device: None,
                copy_of: None,
            },
        );
        // The member's other devices get copies of it.
        if let Some((t, c)) = self
            .outbox
            .get(&id)
            .filter(|_| how.device_copies)
            .and_then(|i| i.content.clone())
        {
            self.queue_device_copies(contact_id, id, BundleKind::Message, t, &c, rng);
        }
        Some(id)
    }

    // ---------------------------------------------------------------- receiving

    /// Group content from `from` (already authenticated, tombstoned and acknowledged).
    pub(super) fn received_group(
        &mut self,
        from: ContactId,
        id: MsgId,
        created_unix_ms: u64,
        content: GroupContent,
        via: ChannelUse,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        match content {
            GroupContent::Text(t) => self.received_group_text(from, id, created_unix_ms, t, via),
            GroupContent::State(s) => self.received_state(from, s),
            GroupContent::Reply(r) => self.received_reply(from, r, rng),
        }
    }

    fn received_group_text(
        &mut self,
        from: ContactId,
        id: MsgId,
        created_unix_ms: u64,
        t: GroupText,
        via: ChannelUse,
    ) -> Vec<Output> {
        let group = GroupId(t.group);
        let member = self
            .groups
            .get(&group)
            .is_some_and(|g| g.status == GroupStatus::Member && g.members.contains(&from.0));
        if !member {
            return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)];
        }
        if self.messages.iter().any(|m| m.id == id) {
            return alloc::vec![Output::Rejected(RejectReason::Duplicate)];
        }
        self.messages.push(Message {
            id,
            contact: from,
            direction: MsgDirection::Incoming,
            kind: MessageKind::Text,
            text: t.text,
            created_unix_ms,
            status: DeliveryStatus::Received,
            channels: alloc::vec![via],
            group: Some(group),
            fanout: None,
            attachments: Vec::new(),
        });
        alloc::vec![
            Output::GroupMessage { group, message: id },
            Output::Persist(StorageOp::SaveSnapshot),
        ]
    }

    fn received_state(&mut self, from: ContactId, s: GroupState) -> Vec<Output> {
        let group = GroupId(s.group);
        let Ok(cards) = verify_cards(&s.cards) else {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        };
        // The admin's card comes first and must be the sender's.
        if cards.first().map(|c| *c.identity.as_bytes()) != Some(from.0) {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        let members: Vec<[u8; 32]> = cards.iter().map(|c| *c.identity.as_bytes()).collect();
        let Some(g) = self.groups.get(&group) else {
            // New: only an invitation from a full contact is shown.
            let from_contact = self.contacts.get(&from).is_some_and(|c| !c.group_only);
            let invitations = self
                .groups
                .values()
                .filter(|g| g.status == GroupStatus::Invited)
                .count();
            if s.role != GroupRole::Invited || !from_contact {
                return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)];
            }
            if invitations >= MAX_INVITATIONS || self.groups.len() >= MAX_GROUPS {
                return alloc::vec![Output::Rejected(RejectReason::Quota)];
            }
            self.groups.insert(
                group,
                Group {
                    name: s.name,
                    admin: from.0,
                    epoch: s.epoch,
                    cards: s.cards,
                    members,
                    status: GroupStatus::Invited,
                    invited: Vec::new(),
                    created_unix_ms: self.now_unix_ms,
                },
            );
            return alloc::vec![
                Output::GroupInvitation { group },
                Output::Persist(StorageOp::SaveSnapshot),
            ];
        };
        // Only the admin speaks for the group, and only forward in epochs.
        if g.admin != from.0 || s.epoch <= g.epoch {
            return alloc::vec![Output::Rejected(RejectReason::Duplicate)];
        }
        let status = g.status;
        let next = match (s.role, status) {
            (GroupRole::Removed, _) => GroupStatus::Removed,
            // A new invitation after we left or were removed.
            (
                GroupRole::Invited,
                GroupStatus::Left | GroupStatus::Removed | GroupStatus::Invited,
            ) => GroupStatus::Invited,
            (GroupRole::Member, GroupStatus::Joining | GroupStatus::Member)
                if members.contains(&self.me()) =>
            {
                GroupStatus::Member
            }
            // Anything else (a member state we never asked for) changes nothing.
            _ => return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)],
        };
        if next == GroupStatus::Member {
            self.add_group_contacts(&cards);
        }
        if let Some(g) = self.groups.get_mut(&group) {
            g.epoch = s.epoch;
            g.name = s.name;
            g.cards = s.cards;
            g.members = members;
            g.status = next;
        }
        self.prune_group_contacts();
        alloc::vec![
            if next == GroupStatus::Invited && status != GroupStatus::Invited {
                Output::GroupInvitation { group }
            } else {
                Output::GroupChanged { group }
            },
            Output::Persist(StorageOp::SaveSnapshot),
        ]
    }

    /// Group-only contacts for members who are not contacts yet, with the pair secret derived
    /// from the two cards of this state.
    fn add_group_contacts(&mut self, cards: &[MemberCard]) {
        let me = self.me();
        let Some(mine) = cards.iter().find(|c| *c.identity.as_bytes() == me) else {
            return;
        };
        let my_nonce = PairingNonce::from_bytes(*mine.nonce.as_bytes());
        for c in cards {
            let id = ContactId(*c.identity.as_bytes());
            if id.0 == me || self.contacts.contains_key(&id) {
                continue;
            }
            // On another device the main device sends these secrets.
            let Some(Ok(secret)) = self
                .identity
                .key()
                .map(|k| ContactSecret::derive(k, &my_nonce, &c.identity, &c.nonce))
            else {
                continue;
            };
            self.contacts.insert(
                id,
                Contact {
                    identity: c.identity,
                    mailbox: c.mailbox,
                    secret,
                    display_name: c.name.clone(),
                    trust: TrustLevel::Introduced,
                    floor: self.default_floor,
                    added_unix_ms: self.now_unix_ms,
                    mailbox_serial: 0,
                    mailbox_learned_ms: self.now_unix_ms,
                    told_serial: 0,
                    requested: false,
                    group_only: true,
                    email: None,
                    alias: None,
                    xwing: None,
                    told_xwing: 0,
                    devices: Vec::new(),
                    device_list: None,
                    told_list: 0,
                },
            );
        }
    }

    /// Forgets group-only contacts who share no live group with us any more.
    fn prune_group_contacts(&mut self) {
        let live: BTreeSet<[u8; 32]> = self
            .groups
            .values()
            .filter(|g| matches!(g.status, GroupStatus::Member | GroupStatus::Joining))
            .flat_map(|g| g.members.iter().copied())
            .collect();
        let gone: Vec<ContactId> = self
            .contacts
            .iter()
            .filter(|(id, c)| c.group_only && !live.contains(&id.0))
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.contacts.remove(&id);
            self.pending_acks.remove(&id);
            self.ack_due.remove(&id);
            self.outbox.retain(|_, i| i.contact != id);
        }
    }

    fn received_reply(
        &mut self,
        from: ContactId,
        r: GroupReply,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let group = GroupId(r.group);
        let Ok(g) = self.admin_group(group) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)];
        };
        let invited = g.invited.contains(&from.0);
        let member = g.members.contains(&from.0);
        let full = g.members.len() >= MAX_GROUP_MEMBERS;
        let mut out = Vec::new();
        match r.card {
            Some(card) if invited && !full => {
                // The card must be the sender's own.
                let ok = verify_card(&card).is_ok_and(|(id, ..)| *id.as_bytes() == from.0);
                if !ok {
                    return alloc::vec![Output::Rejected(RejectReason::Malformed)];
                }
                if let Some(g) = self.groups.get_mut(&group) {
                    g.invited.retain(|m| *m != from.0);
                    g.cards.push(card);
                    g.members.push(from.0);
                    g.epoch = g.epoch.saturating_add(1);
                }
                out.extend(self.send_state_to_members(group, rng));
            }
            None if invited => {
                if let Some(g) = self.groups.get_mut(&group) {
                    g.invited.retain(|m| *m != from.0);
                }
            }
            None if member => {
                // It left on its own: no state back (it shows "left", not "removed").
                if let Some(g) = self.groups.get_mut(&group) {
                    drop_member(g, &from.0);
                }
                out.extend(self.send_state_to_members(group, rng));
            }
            _ => return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)],
        }
        out.push(Output::GroupChanged { group });
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        out
    }

    // ---------------------------------------------------------------- queries

    /// Groups, newest first.
    #[must_use]
    pub fn groups(&self) -> Vec<GroupView> {
        let mut out: Vec<GroupView> = self
            .groups
            .iter()
            .map(|(id, g)| self.group_view(*id, g))
            .collect();
        out.sort_by_key(|g| core::cmp::Reverse(g.created_unix_ms));
        out
    }

    /// One group.
    #[must_use]
    pub fn group(&self, id: GroupId) -> Option<GroupView> {
        self.groups.get(&id).map(|g| self.group_view(id, g))
    }

    fn group_view(&self, id: GroupId, g: &Group) -> GroupView {
        let me = self.me();
        let member = |m: &[u8; 32], invited: bool| {
            let c = self.contacts.get(&ContactId(*m));
            GroupMemberView {
                contact: ContactId(*m),
                display_name: c.and_then(|c| c.display_name.clone()),
                alias: c.and_then(|c| c.alias.clone()),
                fingerprint: mem_crypto::IdentityPublic::from_bytes(m)
                    .map(|p| p.fingerprint().to_string_grouped())
                    .unwrap_or_default(),
                is_contact: c.is_some_and(|c| !c.group_only),
                is_me: *m == me,
                role: if invited {
                    MemberRole::Invited
                } else if *m == g.admin {
                    MemberRole::Admin
                } else {
                    MemberRole::Member
                },
            }
        };
        GroupView {
            id,
            name: g.name.clone(),
            status: g.status,
            is_admin: g.admin == me,
            epoch: g.epoch,
            members: g
                .members
                .iter()
                .map(|m| member(m, false))
                .chain(g.invited.iter().map(|m| member(m, true)))
                .collect(),
            created_unix_ms: g.created_unix_ms,
        }
    }

    /// A group's messages, oldest first. Our copies to each member show as one message whose
    /// status counts the members who confirmed it.
    #[must_use]
    pub fn group_messages(&self, group: GroupId) -> Vec<MessageView> {
        let mut out: Vec<MessageView> = Vec::new();
        for m in self.messages.iter().filter(|m| m.group == Some(group)) {
            if let Some(f) = m.fanout
                && let Some(v) = out.iter_mut().find(|v| v.id == f)
            {
                let (done, total) = v.recipients.unwrap_or((0, 0));
                let delivered = u16::from(m.status == DeliveryStatus::Delivered);
                v.recipients = Some((done + delivered, total + 1));
                for c in &m.channels {
                    if !v.channels.iter().any(|x| x.cla == c.cla) {
                        v.channels.push(*c);
                    }
                }
                v.status = merge_status(v.status, m.status);
                continue;
            }
            let mut v = MessageView::from(m);
            if m.fanout.is_some() {
                v.id = m.fanout.unwrap_or(m.id);
                v.recipients = Some((u16::from(m.status == DeliveryStatus::Delivered), 1));
            }
            out.push(v);
        }
        out
    }

    /// Hidden copies of one of our group messages (for a resend by a manual channel).
    #[must_use]
    pub fn group_copies(&self, fanout: MsgId) -> Vec<MsgId> {
        self.messages
            .iter()
            .filter(|m| m.fanout == Some(fanout) && self.outbox.contains_key(&m.id))
            .map(|m| m.id)
            .collect()
    }
}

/// Removes a member and its card; the epoch rises.
fn drop_member(g: &mut Group, member: &[u8; 32]) {
    if let Some(i) = g.members.iter().position(|m| m == member) {
        g.members.remove(i);
        if i < g.cards.len() {
            g.cards.remove(i);
        }
        g.epoch = g.epoch.saturating_add(1);
    }
}

/// The combined status of copies: the least advanced one wins, except that any copy delivered
/// makes "sent" at least.
const fn merge_status(a: DeliveryStatus, b: DeliveryStatus) -> DeliveryStatus {
    use DeliveryStatus::{Delivered, Failed, Queued, Received, Sent};
    match (a, b) {
        (Delivered, Delivered) => Delivered,
        (Queued, Queued) => Queued,
        (Failed, Failed) => Failed,
        (Received, _) | (_, Received) => Received,
        _ => Sent,
    }
}

fn clean_name(name: &str) -> Result<String, RejectReason> {
    let n = name.trim();
    if n.is_empty() || n.len() > MAX_GROUP_NAME_LEN {
        Err(RejectReason::InvalidName)
    } else {
        Ok(String::from(n))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use crate::Input;
    use mem_cla::ClaId;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn agent(seed: u8) -> (Agent, ChaCha20Rng) {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        (Agent::new(1_800_000_000_000, &mut rng), rng)
    }

    fn pair(a: &mut Agent, ra: &mut ChaCha20Rng, b: &mut Agent, rb: &mut ChaCha20Rng) {
        let (ca, cb) = (a.own_card().unwrap(), b.own_card().unwrap());
        a.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: cb,
            },
            ra,
        );
        b.handle(
            Input::ImportCard {
                cla: ClaId::Qr,
                bytes: ca,
            },
            rb,
        );
    }

    fn state(admin: &Agent, epoch: u32, role: GroupRole) -> GroupState {
        GroupState {
            group: [5; 16],
            epoch,
            name: "g".into(),
            cards: alloc::vec![admin.own_card().unwrap()],
            role,
        }
    }

    #[test]
    fn older_or_equal_epochs_are_ignored() {
        let (mut a, mut ra) = agent(1);
        let (mut b, mut rb) = agent(2);
        pair(&mut a, &mut ra, &mut b, &mut rb);
        let a_id = ContactId(*a.identity.public().as_bytes());
        let out = b.received_state(a_id, state(&a, 5, GroupRole::Invited));
        assert!(out.contains(&Output::GroupInvitation {
            group: GroupId([5; 16])
        }));
        for epoch in [5, 4] {
            let out = b.received_state(a_id, state(&a, epoch, GroupRole::Removed));
            assert_eq!(out, alloc::vec![Output::Rejected(RejectReason::Duplicate)]);
        }
        assert_eq!(
            b.group(GroupId([5; 16])).unwrap().status,
            GroupStatus::Invited
        );
        // A state not signed off by the admin (first card someone else's) is refused.
        let mut forged = state(&a, 9, GroupRole::Removed);
        forged.cards = alloc::vec![b.own_card().unwrap()];
        assert_eq!(
            b.received_state(a_id, forged),
            alloc::vec![Output::Rejected(RejectReason::Malformed)]
        );
    }

    /// An invitation from someone who is only a fellow group member is dropped.
    #[test]
    fn invitations_only_from_full_contacts() {
        let (mut a, mut ra) = agent(3);
        let (mut b, mut rb) = agent(4);
        pair(&mut a, &mut ra, &mut b, &mut rb);
        let a_id = ContactId(*a.identity.public().as_bytes());
        b.contacts.get_mut(&a_id).unwrap().group_only = true;
        assert_eq!(
            b.received_state(a_id, state(&a, 1, GroupRole::Invited)),
            alloc::vec![Output::Rejected(RejectReason::UnknownGroup)]
        );
        assert_eq!(b.groups(), Vec::new());
        let _ = &mut rb;
    }
}

/// How a hidden item travels.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hidden {
    /// Bundle lifetime.
    pub(super) lifetime_s: u32,
    /// Whether the recipient acknowledges it.
    pub(super) ack: bool,
    /// Spray-and-wait copy budget (1: direct only).
    pub(super) copies: u8,
    /// Whether the contact's other devices get copies.
    pub(super) device_copies: bool,
}
