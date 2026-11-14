//! Files shared with a contact or a group, download only.
//!
//! The owner offers a list of files ([`CONTENT_TYPE_SHARE_CATALOG`], end to end, replacing the
//! previous list); the contact asks for one item ([`CONTENT_TYPE_SHARE_REQUEST`]); the owner's
//! device checks the request again and answers with the file ([`CONTENT_TYPE_SHARE_FILE`], one
//! attachment), automatically: offering it was the consent. The file then travels as any
//! attachment (parts or a direct pull), never through couriers (one copy). The core never
//! reads files: it asks the runtime for an item ([`Output::ShareItemWanted`]) and gets it back
//! with [`super::Input::ServeSharedItem`].

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use mem_proto::share::{
    CONTENT_TYPE_SHARE_CATALOG, CONTENT_TYPE_SHARE_FILE, CONTENT_TYPE_SHARE_REQUEST, ShareCatalog,
    ShareItem, ShareRef,
};
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};

use super::{Agent, Message, MsgDirection, Output, StorageOp};
use crate::model::{
    ContactId, DeliveryStatus, GroupId, GroupStatus, IntroducedMembers, MessageKind, MsgId,
    NewAttachment, NewShareItem, OwnShareItemView, OwnShareView, ReceivedSharedFileView,
    RejectReason, ShareAudience, ShareId, ShareItemId, SharedItemState, TheirShareItemView,
    TheirShareView,
};

/// Requests served per contact and minute; more are dropped.
const MAX_REQUESTS_PER_MINUTE: usize = 8;
/// Downloads remembered per share (oldest dropped).
const MAX_DOWNLOADS: usize = 256;
/// Shares received from all contacts together (oldest dropped).
const MAX_THEIR_SHARES: usize = 256;
/// Requests we wait on (oldest dropped).
const MAX_REQUESTED: usize = 512;

/// Who a share is for (stored form).
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Audience {
    Contact([u8; 32]),
    Group([u8; 16]),
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnItem {
    id: [u8; 16],
    name: String,
    size: u64,
    media_type: String,
    modified_s: Option<u64>,
    handle: String,
    /// (contact, Unix ms), newest last.
    downloads: Vec<([u8; 32], u64)>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnShare {
    id: [u8; 16],
    name: String,
    audience: Audience,
    introduced: bool,
    items: Vec<OwnItem>,
    /// Contacts that were sent a list (they get the empty one when the share ends for them).
    sent_to: BTreeSet<[u8; 32]>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TheirItem {
    id: [u8; 16],
    name: String,
    size: u64,
    media_type: String,
    modified_s: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TheirShare {
    contact: [u8; 32],
    id: [u8; 16],
    name: String,
    group: Option<[u8; 16]>,
    items: Vec<TheirItem>,
    updated_unix_ms: u64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Requested {
    contact: [u8; 32],
    share: [u8; 16],
    item: [u8; 16],
    at_unix_ms: u64,
    /// The message carrying the file, once it arrived.
    message: Option<[u8; 16]>,
}

/// Shares we offer and shares offered to us; kept in the encrypted state.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Shares {
    #[serde(default)]
    own: Vec<OwnShare>,
    #[serde(default)]
    theirs: Vec<TheirShare>,
    #[serde(default)]
    requested: Vec<Requested>,
    /// Requests served recently, per contact (volatile).
    #[serde(skip)]
    served: Vec<([u8; 32], u64)>,
}

impl Shares {
    /// Nothing offered, received or asked for.
    pub(crate) fn is_empty(&self) -> bool {
        self.own.is_empty() && self.theirs.is_empty() && self.requested.is_empty()
    }

    /// Whether we wait for a file from `contact` (its attachment parts are then welcome even if
    /// it is only a fellow group member).
    pub(crate) fn awaiting_from(&self, contact: ContactId) -> bool {
        self.requested
            .iter()
            .any(|r| r.contact == contact.0 && r.message.is_none())
    }
}

fn catalog_of(s: &OwnShare, empty: bool) -> ShareCatalog {
    ShareCatalog {
        share_id: s.id,
        name: s.name.clone(),
        items: if empty {
            Vec::new()
        } else {
            s.items
                .iter()
                .map(|i| ShareItem {
                    item_id: i.id,
                    name: i.name.clone(),
                    size: i.size,
                    media_type: i.media_type.clone(),
                    modified_s: i.modified_s,
                })
                .collect()
        },
        group: match s.audience {
            Audience::Group(g) => Some(g),
            Audience::Contact(_) => None,
        },
    }
}

impl Agent {
    /// The contacts a share reaches now: its contact, or the group's current members that are
    /// contacts (members we never met only when the share includes them).
    fn share_audience(&self, s: &OwnShare) -> BTreeSet<[u8; 32]> {
        let reachable = |c: &[u8; 32], introduced_ok: bool| {
            let id = ContactId(*c);
            self.contacts
                .get(&id)
                .is_some_and(|x| !x.requested && (!x.group_only || introduced_ok))
                && !self.is_self(id)
        };
        match s.audience {
            Audience::Contact(c) => {
                if reachable(&c, false) {
                    BTreeSet::from([c])
                } else {
                    BTreeSet::new()
                }
            }
            Audience::Group(g) => match self.groups.get(&GroupId(g)) {
                Some(group) if group.status == GroupStatus::Member => group
                    .members
                    .iter()
                    .filter(|m| reachable(m, s.introduced))
                    .copied()
                    .collect(),
                _ => BTreeSet::new(),
            },
        }
    }

    /// Sends the share's list to everyone it reaches now, and the empty list to those it no
    /// longer reaches (or to everyone, when `withdraw`).
    fn send_catalogs(
        &mut self,
        share: [u8; 16],
        withdraw: bool,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(s) = self.shares.own.iter().find(|s| s.id == share).cloned() else {
            return Vec::new();
        };
        let now = if withdraw {
            BTreeSet::new()
        } else {
            self.share_audience(&s)
        };
        let mut out = Vec::new();
        let gone: Vec<[u8; 32]> = s.sent_to.difference(&now).copied().collect();
        for (to, empty) in now
            .iter()
            .map(|c| (*c, false))
            .chain(gone.iter().map(|c| (*c, true)))
        {
            let Ok(content) = catalog_of(&s, empty).encode() else {
                continue;
            };
            if let Some(id) =
                self.queue_hidden(ContactId(to), CONTENT_TYPE_SHARE_CATALOG, content, rng)
            {
                out.extend(self.flush_message(id));
            }
        }
        if let Some(own) = self.shares.own.iter_mut().find(|x| x.id == share) {
            own.sent_to = now;
        }
        out
    }

    /// Creates a share (`share` `None`) or replaces one, and tells everyone it reaches. Items
    /// that stay (same handle) keep their ids, so downloads in progress go on.
    pub(super) fn save_share(
        &mut self,
        share: Option<ShareId>,
        name: &str,
        audience: ShareAudience,
        introduced: IntroducedMembers,
        items: Vec<NewShareItem>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let audience = match audience {
            ShareAudience::Contact(c) => {
                if self.may_write_to(c).is_err() {
                    return alloc::vec![Output::Rejected(RejectReason::UnknownContact)];
                }
                Audience::Contact(c.0)
            }
            ShareAudience::Group(g) => match self.groups.get(&g) {
                Some(x) if x.status == GroupStatus::Member => Audience::Group(g.0),
                _ => return alloc::vec![Output::Rejected(RejectReason::UnknownGroup)],
            },
        };
        let old = share.and_then(|s| self.shares.own.iter().find(|x| x.id == s.0).cloned());
        if share.is_some() && old.is_none() {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        let id = old.as_ref().map_or_else(
            || {
                let mut b = [0u8; 16];
                rng.fill_bytes(&mut b);
                b
            },
            |o| o.id,
        );
        let items: Vec<OwnItem> = items
            .into_iter()
            .map(|n| {
                let kept = old
                    .as_ref()
                    .and_then(|o| o.items.iter().find(|i| i.handle == n.handle));
                OwnItem {
                    id: kept.map_or_else(
                        || {
                            let mut b = [0u8; 16];
                            rng.fill_bytes(&mut b);
                            b
                        },
                        |k| k.id,
                    ),
                    downloads: kept.map(|k| k.downloads.clone()).unwrap_or_default(),
                    name: n.name,
                    size: n.size,
                    media_type: n.media_type,
                    modified_s: n.modified_s,
                    handle: n.handle,
                }
            })
            .collect();
        let new = OwnShare {
            id,
            name: String::from(name.trim()),
            audience,
            introduced: introduced == IntroducedMembers::Included,
            items,
            sent_to: old.as_ref().map(|o| o.sent_to.clone()).unwrap_or_default(),
        };
        // The list must be valid on the wire before anything is kept.
        if catalog_of(&new, false).encode().is_err() {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        // A share moved to someone else: the old audience gets the empty list.
        if old.as_ref().is_some_and(|o| o.audience != new.audience) {
            let mut out = self.send_catalogs(id, true, rng);
            self.shares.own.retain(|s| s.id != id);
            self.shares.own.push(OwnShare {
                sent_to: BTreeSet::new(),
                ..new
            });
            out.extend(self.finish_save(id, rng));
            return out;
        }
        self.shares.own.retain(|s| s.id != id);
        self.shares.own.push(new);
        self.finish_save(id, rng)
    }

    fn finish_save(&mut self, id: [u8; 16], rng: &mut impl CryptoRng) -> Vec<Output> {
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::ShareSaved { share: ShareId(id) },
        ];
        out.extend(self.send_catalogs(id, false, rng));
        out
    }

    /// Stops a share: everyone it reached gets the empty list.
    pub(super) fn remove_share(&mut self, share: ShareId, rng: &mut impl CryptoRng) -> Vec<Output> {
        if !self.shares.own.iter().any(|s| s.id == share.0) {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        let mut out = self.send_catalogs(share.0, true, rng);
        self.shares.own.retain(|s| s.id != share.0);
        out.insert(0, Output::Persist(StorageOp::SaveSnapshot));
        out
    }

    /// Group membership changed: each group share's list follows its members.
    pub(super) fn group_shares_follow(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        let ids: Vec<[u8; 16]> = self
            .shares
            .own
            .iter()
            .filter(|s| matches!(s.audience, Audience::Group(_)))
            .map(|s| s.id)
            .collect();
        let mut out = Vec::new();
        for id in ids {
            let reached = self
                .shares
                .own
                .iter()
                .find(|s| s.id == id)
                .map(|s| (s.sent_to.clone(), self.share_audience(s)));
            if let Some((before, now)) = reached
                && before != now
            {
                out.extend(self.send_catalogs(id, false, rng));
            }
        }
        if !out.is_empty() {
            out.insert(0, Output::Persist(StorageOp::SaveSnapshot));
        }
        out
    }

    /// A list from `contact`. Accepted from a contact for a 1:1 share, or from a fellow member
    /// of a group we belong to for that group's share; anything else is ignored.
    pub(super) fn received_catalog(
        &mut self,
        contact: ContactId,
        catalog: ShareCatalog,
    ) -> Vec<Output> {
        let Some(c) = self.contacts.get(&contact) else {
            return Vec::new();
        };
        let allowed = match catalog.group {
            None => !c.group_only,
            Some(g) => self
                .groups
                .get(&GroupId(g))
                .is_some_and(|x| x.status == GroupStatus::Member && x.members.contains(&contact.0)),
        };
        if !allowed {
            return Vec::new();
        }
        let now = self.now_unix_ms;
        let at = self
            .shares
            .theirs
            .iter()
            .position(|s| s.contact == contact.0 && s.id == catalog.share_id);
        let before: BTreeSet<[u8; 16]> = at
            .map(|i| self.shares.theirs[i].items.iter().map(|x| x.id).collect())
            .unwrap_or_default();
        if let Some(i) = at {
            self.shares.theirs.remove(i);
        }
        if catalog.items.is_empty() {
            // Withdrawn: what we asked for will not come.
            self.shares.requested.retain(|r| {
                !(r.contact == contact.0 && r.share == catalog.share_id && r.message.is_none())
            });
            return alloc::vec![
                Output::Persist(StorageOp::SaveSnapshot),
                Output::SharesChanged
            ];
        }
        let new_items = catalog
            .items
            .iter()
            .filter(|i| !before.contains(&i.item_id))
            .count();
        if self.shares.theirs.len() >= MAX_THEIR_SHARES {
            self.shares.theirs.remove(0);
        }
        self.shares.theirs.push(TheirShare {
            contact: contact.0,
            id: catalog.share_id,
            name: catalog.name.clone(),
            group: catalog.group,
            items: catalog
                .items
                .into_iter()
                .map(|i| TheirItem {
                    id: i.item_id,
                    name: i.name,
                    size: i.size,
                    media_type: i.media_type,
                    modified_s: i.modified_s,
                })
                .collect(),
            updated_unix_ms: now,
        });
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::SharesChanged
        ];
        if new_items > 0 {
            // A quiet line in the conversation the share belongs to.
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&now.to_be_bytes());
            id[8..].copy_from_slice(&catalog.share_id[..8]);
            let message = MsgId(id);
            self.messages.push(Message {
                id: message,
                contact,
                direction: MsgDirection::Incoming,
                kind: MessageKind::ShareNotice {
                    new_items: u16::try_from(new_items).unwrap_or(u16::MAX),
                },
                text: catalog.name,
                created_unix_ms: now,
                status: DeliveryStatus::Received,
                channels: Vec::new(),
                group: catalog.group.map(GroupId),
                fanout: None,
                edit: None,
                attachments: Vec::new(),
            });
            out.push(Output::Deliver { contact, message });
        }
        out
    }

    /// We ask `contact` for one item it offers us.
    pub(super) fn request_shared_item(
        &mut self,
        contact: ContactId,
        share: ShareId,
        item: ShareItemId,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let offered = self.shares.theirs.iter().any(|s| {
            s.contact == contact.0 && s.id == share.0 && s.items.iter().any(|i| i.id == item.0)
        });
        if !offered {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        }
        let content = ShareRef {
            share_id: share.0,
            item_id: item.0,
        }
        .encode();
        let Some(id) = self.queue_hidden(contact, CONTENT_TYPE_SHARE_REQUEST, content, rng) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownContact)];
        };
        self.shares
            .requested
            .retain(|r| !(r.contact == contact.0 && r.share == share.0 && r.item == item.0));
        if self.shares.requested.len() >= MAX_REQUESTED {
            self.shares.requested.remove(0);
        }
        self.shares.requested.push(Requested {
            contact: contact.0,
            share: share.0,
            item: item.0,
            at_unix_ms: self.now_unix_ms,
            message: None,
        });
        let mut out = alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::SharesChanged
        ];
        out.extend(self.flush_message(id));
        out
    }

    /// The handle of `item` if `contact` may download it now.
    fn servable(&self, contact: ContactId, r: ShareRef) -> Option<String> {
        let s = self.shares.own.iter().find(|s| s.id == r.share_id)?;
        if !self.share_audience(s).contains(&contact.0) {
            return None;
        }
        s.items
            .iter()
            .find(|i| i.id == r.item_id)
            .map(|i| i.handle.clone())
    }

    /// `contact` asks for an item: if it may have it (still offered to it, within the rate),
    /// the runtime is asked to read the file. Anything else is ignored silently.
    pub(super) fn received_share_request(
        &mut self,
        contact: ContactId,
        r: ShareRef,
    ) -> Vec<Output> {
        let Some(handle) = self.servable(contact, r) else {
            return Vec::new();
        };
        let now = self.now_unix_ms;
        self.shares
            .served
            .retain(|(_, at)| now.saturating_sub(*at) < 60_000);
        let recent = self
            .shares
            .served
            .iter()
            .filter(|(c, _)| *c == contact.0)
            .count();
        if recent >= MAX_REQUESTS_PER_MINUTE {
            return Vec::new();
        }
        self.shares.served.push((contact.0, now));
        alloc::vec![Output::ShareItemWanted {
            contact,
            share: ShareId(r.share_id),
            item: ShareItemId(r.item_id),
            handle,
        }]
    }

    /// The runtime read a requested file: it goes to `contact` as the answer, with one copy
    /// (never through couriers), if the item is still offered to it.
    pub(super) fn serve_shared_item(
        &mut self,
        contact: ContactId,
        share: ShareId,
        item: ShareItemId,
        file: NewAttachment,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let r = ShareRef {
            share_id: share.0,
            item_id: item.0,
        };
        if self.servable(contact, r).is_none() {
            return alloc::vec![Output::Rejected(RejectReason::NotAContact)];
        }
        let what = super::Outgoing {
            content_type: CONTENT_TYPE_SHARE_FILE,
            content: r.encode(),
            text: String::new(),
            kind: MessageKind::SharedFile { share, item },
        };
        let out = self.send_and_flush(contact, what, alloc::vec![file], true, rng);
        let now = self.now_unix_ms;
        if let Some(i) = self
            .shares
            .own
            .iter_mut()
            .find(|s| s.id == share.0)
            .and_then(|s| s.items.iter_mut().find(|i| i.id == item.0))
        {
            if i.downloads.len() >= MAX_DOWNLOADS {
                i.downloads.remove(0);
            }
            i.downloads.push((contact.0, now));
        }
        out
    }

    /// Whether a file answer from `contact` for `r` is one we asked for.
    pub(super) fn share_file_expected(&self, contact: ContactId, r: ShareRef) -> bool {
        self.shares.requested.iter().any(|q| {
            q.contact == contact.0
                && q.share == r.share_id
                && q.item == r.item_id
                && q.message.is_none()
        })
    }

    /// The answer for `r` arrived as `message`.
    pub(super) fn share_file_arrived(&mut self, contact: ContactId, r: ShareRef, message: MsgId) {
        if let Some(q) = self
            .shares
            .requested
            .iter_mut()
            .find(|q| q.contact == contact.0 && q.share == r.share_id && q.item == r.item_id)
        {
            q.message = Some(message.0);
        }
    }

    /// Our shares, for the editor.
    #[must_use]
    pub fn own_shares(&self) -> Vec<OwnShareView> {
        self.shares
            .own
            .iter()
            .map(|s| OwnShareView {
                id: ShareId(s.id),
                name: s.name.clone(),
                audience: match s.audience {
                    Audience::Contact(c) => ShareAudience::Contact(ContactId(c)),
                    Audience::Group(g) => ShareAudience::Group(GroupId(g)),
                },
                introduced: if s.introduced {
                    IntroducedMembers::Included
                } else {
                    IntroducedMembers::Excluded
                },
                items: s
                    .items
                    .iter()
                    .map(|i| OwnShareItemView {
                        id: ShareItemId(i.id),
                        item: NewShareItem {
                            name: i.name.clone(),
                            size: i.size,
                            media_type: i.media_type.clone(),
                            modified_s: i.modified_s,
                            handle: i.handle.clone(),
                        },
                        downloads: i
                            .downloads
                            .iter()
                            .map(|(c, at)| (ContactId(*c), *at))
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Files contacts shared with us that we downloaded (from one contact or all), newest
    /// first; also those whose share has ended.
    #[must_use]
    pub fn received_shared_files(&self, contact: Option<ContactId>) -> Vec<ReceivedSharedFileView> {
        let mut v: Vec<ReceivedSharedFileView> = self
            .messages
            .iter()
            .filter(|m| m.direction == MsgDirection::Incoming)
            .filter(|m| contact.is_none_or(|c| c == m.contact))
            .filter_map(|m| {
                let MessageKind::SharedFile { share, item } = m.kind else {
                    return None;
                };
                let file = &m.attachments.first()?.reference;
                Some(ReceivedSharedFileView {
                    message: m.id,
                    contact: m.contact,
                    name: file.name.clone(),
                    size: file.size,
                    media_type: file.media_type.clone(),
                    received_unix_ms: m
                        .channels
                        .first()
                        .map_or(m.created_unix_ms, |u| u.at_unix_ms),
                    share,
                    still_offered: self.shares.theirs.iter().any(|s| {
                        s.contact == m.contact.0
                            && s.id == share.0
                            && s.items.iter().any(|i| i.id == item.0)
                    }),
                })
            })
            .collect();
        v.sort_by_key(|f| core::cmp::Reverse(f.received_unix_ms));
        v
    }

    /// Deletes a downloaded shared file: the message, its stored blob (or the parts of it so
    /// far), and the record that we asked for it, so a share that still offers it shows it as
    /// available again.
    pub(super) fn delete_received_file(&mut self, message: MsgId) -> Vec<Output> {
        let Some(at) = self.messages.iter().position(|m| {
            m.id == message
                && m.direction == MsgDirection::Incoming
                && matches!(m.kind, MessageKind::SharedFile { .. })
        }) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownMessage)];
        };
        let gone = self.messages.remove(at);
        self.shares
            .requested
            .retain(|q| q.message != Some(message.0));
        let mut out = Vec::new();
        for a in &gone.attachments {
            let address = a.reference.address;
            // Content addresses are unique per sealing; still, keep a blob another message uses.
            if self
                .messages
                .iter()
                .any(|m| m.attachments.iter().any(|x| x.reference.address == address))
            {
                continue;
            }
            self.blobs.remove(&address);
            self.partials.remove(&address);
            self.pulls.remove(&address);
            out.push(Output::Persist(StorageOp::DeleteBlob { address }));
        }
        out.push(Output::Persist(StorageOp::SaveSnapshot));
        out.push(Output::SharesChanged);
        out
    }

    /// Shares offered to us ("Shared with me"), from one contact or all, newest first.
    #[must_use]
    pub fn shared_with_me(&self, contact: Option<ContactId>) -> Vec<TheirShareView> {
        let mut v: Vec<TheirShareView> = self
            .shares
            .theirs
            .iter()
            .filter(|s| contact.is_none_or(|c| c.0 == s.contact))
            .map(|s| TheirShareView {
                contact: ContactId(s.contact),
                id: ShareId(s.id),
                name: s.name.clone(),
                group: s.group.map(GroupId),
                items: s
                    .items
                    .iter()
                    .map(|i| TheirShareItemView {
                        id: ShareItemId(i.id),
                        name: i.name.clone(),
                        size: i.size,
                        media_type: i.media_type.clone(),
                        modified_s: i.modified_s,
                        state: self
                            .shares
                            .requested
                            .iter()
                            .find(|q| q.contact == s.contact && q.share == s.id && q.item == i.id)
                            .map_or(SharedItemState::Available, |q| match q.message {
                                Some(m) => SharedItemState::Received(MsgId(m)),
                                None => SharedItemState::Requested,
                            }),
                    })
                    .collect(),
                updated_unix_ms: s.updated_unix_ms,
            })
            .collect();
        v.sort_by_key(|s| core::cmp::Reverse(s.updated_unix_ms));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Input;
    use crate::model::AttachmentData;
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
        /// B as A's contact, and A as B's.
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
        /// Hands everything one side holds to the other; returns what the receiver produced.
        fn carry(&mut self, to_b: bool) -> Vec<Output> {
            let (from, to, rng) = if to_b {
                (&self.a, &mut self.b, &mut self.rb)
            } else {
                (&self.b, &mut self.a, &mut self.ra)
            };
            let bundles: Vec<Vec<u8>> = from.outbox.values().map(|o| o.bundle.encode()).collect();
            let mut out = Vec::new();
            for bytes in bundles {
                out.extend(to.handle(
                    Input::Received {
                        cla: ClaId::Qr,
                        bytes,
                    },
                    rng,
                ));
            }
            out
        }

        /// A shares two files with B.
        fn share(&mut self) -> ShareId {
            let out = self.a.handle(
                Input::SaveShare {
                    share: None,
                    name: "Holiday".into(),
                    audience: ShareAudience::Contact(self.b_id),
                    introduced: IntroducedMembers::Excluded,
                    items: alloc::vec![file("sea.jpg", "h1"), file("notes.txt", "h2")],
                },
                &mut self.ra,
            );
            out.iter()
                .find_map(|o| match o {
                    Output::ShareSaved { share } => Some(*share),
                    _ => None,
                })
                .unwrap()
        }
    }

    fn file(name: &str, handle: &str) -> NewShareItem {
        NewShareItem {
            name: name.into(),
            size: 1200,
            media_type: "image/jpeg".into(),
            modified_s: None,
            handle: handle.into(),
        }
    }

    fn wanted(out: &[Output]) -> Option<(ContactId, ShareId, ShareItemId, String)> {
        out.iter().find_map(|o| match o {
            Output::ShareItemWanted {
                contact,
                share,
                item,
                handle,
            } => Some((*contact, *share, *item, handle.clone())),
            _ => None,
        })
    }

    #[test]
    fn a_shared_file_is_offered_requested_and_downloaded() {
        let mut p = paired();
        let share = p.share();
        let got = p.carry(true);
        assert!(got.iter().any(|o| matches!(o, Output::SharesChanged)));
        let offered = p.b.shared_with_me(Some(p.a_id));
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].items.len(), 2);
        // A quiet line in B's chat with A.
        let notice = &p.b.messages(p.a_id)[0];
        assert_eq!(notice.kind, MessageKind::ShareNotice { new_items: 2 });
        assert_eq!(notice.text, "Holiday");

        let item = offered[0].items[0].id;
        let _ = p.b.handle(
            Input::RequestSharedItem {
                contact: p.a_id,
                share,
                item,
            },
            &mut p.rb,
        );
        assert_eq!(
            p.b.shared_with_me(None)[0].items[0].state,
            SharedItemState::Requested
        );
        let asked = p.carry(false);
        let (who, s, i, handle) = wanted(&asked).expect("A's runtime is asked for the file");
        assert_eq!((who, s, i, handle.as_str()), (p.b_id, share, item, "h1"));

        let out = p.a.handle(
            Input::ServeSharedItem {
                contact: p.b_id,
                share,
                item,
                file: NewAttachment {
                    name: "sea.jpg".into(),
                    media_type: "image/jpeg".into(),
                    data: AttachmentData::Bytes(alloc::vec![7; 1200]),
                    preview: None,
                },
            },
            &mut p.ra,
        );
        assert!(!out.iter().any(|o| matches!(o, Output::Rejected(_))));
        // Never through couriers: one copy, the parts too.
        assert!(
            p.a.outbox
                .values()
                .filter(|o| o.contact == p.b_id)
                .all(|o| o.copies <= 1)
        );
        p.carry(true);
        let state = p.b.shared_with_me(None)[0].items[0].state;
        let SharedItemState::Received(m) = state else {
            panic!("file not received: {state:?}");
        };
        // Kept with the share, not in the chat; the file is there.
        assert!(p.b.messages(p.a_id).iter().all(|x| x.id != m));
        let carrier = p.b.messages.iter().find(|x| x.id == m).unwrap();
        assert_eq!(carrier.attachments.len(), 1);
        assert_eq!(carrier.attachments[0].reference.name, "sea.jpg");
        // A remembers who downloaded it.
        assert_eq!(p.a.own_shares()[0].items[0].downloads.len(), 1);

        // Listed as downloaded; still there after A stops sharing.
        let kept = p.b.received_shared_files(Some(p.a_id));
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].message, kept[0].name.as_str()), (m, "sea.jpg"));
        assert!(kept[0].still_offered);
        let _ = p.a.handle(Input::RemoveShare { share }, &mut p.ra);
        p.carry(true);
        assert_eq!(p.b.shared_with_me(None).len(), 0);
        let kept = p.b.received_shared_files(None);
        assert_eq!(kept.len(), 1);
        assert!(!kept[0].still_offered);
        // Deleted: the message and its blob go.
        let address = carrier_address(&p.b, m);
        let out =
            p.b.handle(Input::DeleteReceivedFile { message: m }, &mut p.rb);
        assert!(out.contains(&Output::Persist(StorageOp::DeleteBlob { address })));
        assert_eq!(p.b.received_shared_files(None).len(), 0);
        assert!(!p.b.blobs.contains(&address));
        assert!(p.b.messages.iter().all(|x| x.id != m));
        // Only received shared files can be deleted this way.
        let notice = p.b.messages(p.a_id)[0].id;
        let out =
            p.b.handle(Input::DeleteReceivedFile { message: notice }, &mut p.rb);
        assert!(out.contains(&Output::Rejected(RejectReason::UnknownMessage)));
    }

    fn carrier_address(a: &Agent, m: MsgId) -> [u8; 32] {
        a.messages.iter().find(|x| x.id == m).unwrap().attachments[0]
            .reference
            .address
    }

    #[test]
    fn a_deleted_download_of_an_offered_file_is_available_again() {
        let mut p = paired();
        let share = p.share();
        p.carry(true);
        let item = p.b.shared_with_me(None)[0].items[0].id;
        let _ = p.b.handle(
            Input::RequestSharedItem {
                contact: p.a_id,
                share,
                item,
            },
            &mut p.rb,
        );
        p.carry(false);
        let _ = p.a.handle(
            Input::ServeSharedItem {
                contact: p.b_id,
                share,
                item,
                file: NewAttachment {
                    name: "sea.jpg".into(),
                    media_type: "image/jpeg".into(),
                    data: AttachmentData::Bytes(alloc::vec![7; 1200]),
                    preview: None,
                },
            },
            &mut p.ra,
        );
        p.carry(true);
        let SharedItemState::Received(m) = p.b.shared_with_me(None)[0].items[0].state else {
            panic!("not received");
        };
        let _ =
            p.b.handle(Input::DeleteReceivedFile { message: m }, &mut p.rb);
        assert_eq!(
            p.b.shared_with_me(None)[0].items[0].state,
            SharedItemState::Available
        );
    }

    #[test]
    fn withdrawn_items_and_unrequested_files_are_refused() {
        let mut p = paired();
        let share = p.share();
        p.carry(true);
        let item = p.b.shared_with_me(None)[0].items[1].id;
        // A keeps only the first file.
        let _ = p.a.handle(
            Input::SaveShare {
                share: Some(share),
                name: "Holiday".into(),
                audience: ShareAudience::Contact(p.b_id),
                introduced: IntroducedMembers::Excluded,
                items: alloc::vec![file("sea.jpg", "h1")],
            },
            &mut p.ra,
        );
        // The first item kept its id.
        assert_eq!(
            p.a.own_shares()[0].items[0].id,
            p.b.shared_with_me(None)[0].items[0].id
        );
        // B asks for the removed one before the new list arrives: A ignores it.
        let _ = p.b.handle(
            Input::RequestSharedItem {
                contact: p.a_id,
                share,
                item,
            },
            &mut p.rb,
        );
        assert!(wanted(&p.carry(false)).is_none());
        // Serving it anyway is refused.
        let out = p.a.handle(
            Input::ServeSharedItem {
                contact: p.b_id,
                share,
                item,
                file: NewAttachment {
                    name: "notes.txt".into(),
                    media_type: "text/plain".into(),
                    data: AttachmentData::Bytes(alloc::vec![1; 10]),
                    preview: None,
                },
            },
            &mut p.ra,
        );
        assert!(out.iter().any(|o| matches!(o, Output::Rejected(_))));
        // Stopping the share empties B's view.
        let _ = p.a.handle(Input::RemoveShare { share }, &mut p.ra);
        p.carry(true);
        assert_eq!(p.b.shared_with_me(None), Vec::new());
    }

    #[test]
    fn requests_beyond_the_rate_are_dropped() {
        let mut p = paired();
        let share = p.share();
        p.carry(true);
        let item = p.b.shared_with_me(None)[0].items[0].id;
        let r = ShareRef {
            share_id: share.0,
            item_id: item.0,
        };
        let served = (0..12)
            .filter(|_| wanted(&p.a.received_share_request(p.b_id, r)).is_some())
            .count();
        assert_eq!(served, MAX_REQUESTS_PER_MINUTE);
    }

    #[test]
    fn a_file_nobody_asked_for_is_not_kept() {
        let mut p = paired();
        let share = p.share();
        p.carry(true);
        let item = p.b.shared_with_me(None)[0].items[0].id;
        // A sends the file without a request from B.
        let what = crate::agent::Outgoing {
            content_type: CONTENT_TYPE_SHARE_FILE,
            content: ShareRef {
                share_id: share.0,
                item_id: item.0,
            }
            .encode(),
            text: String::new(),
            kind: MessageKind::SharedFile { share, item },
        };
        let file = NewAttachment {
            name: "sea.jpg".into(),
            media_type: "image/jpeg".into(),
            data: AttachmentData::Bytes(alloc::vec![7; 100]),
            preview: None,
        };
        let _ =
            p.a.send_and_flush(p.b_id, what, alloc::vec![file], true, &mut p.ra);
        p.carry(true);
        assert!(
            !p.b.messages
                .iter()
                .any(|m| matches!(m.kind, MessageKind::SharedFile { .. }))
        );
        assert_eq!(
            p.b.shared_with_me(None)[0].items[0].state,
            SharedItemState::Available
        );
    }

    #[test]
    fn a_group_share_reaches_members_and_leaves_with_them() {
        let mut p = paired();
        let g = GroupId([5; 16]);
        let me = *p.a.identity.public().as_bytes();
        p.a.groups.insert(
            g,
            super::super::group::Group {
                name: "Family".into(),
                admin: me,
                epoch: 1,
                cards: Vec::new(),
                members: alloc::vec![me, p.b_id.0],
                status: GroupStatus::Member,
                invited: Vec::new(),
                created_unix_ms: NOW,
            },
        );
        let me_b = *p.b.identity.public().as_bytes();
        p.b.groups.insert(
            g,
            super::super::group::Group {
                name: "Family".into(),
                admin: me,
                epoch: 1,
                cards: Vec::new(),
                members: alloc::vec![me, me_b],
                status: GroupStatus::Member,
                invited: Vec::new(),
                created_unix_ms: NOW,
            },
        );
        let out = p.a.handle(
            Input::SaveShare {
                share: None,
                name: "Photos".into(),
                audience: ShareAudience::Group(g),
                introduced: IntroducedMembers::Excluded,
                items: alloc::vec![file("a.jpg", "h1")],
            },
            &mut p.ra,
        );
        let share = out
            .iter()
            .find_map(|o| match o {
                Output::ShareSaved { share } => Some(*share),
                _ => None,
            })
            .unwrap();
        p.carry(true);
        let seen = p.b.shared_with_me(None);
        assert_eq!((seen.len(), seen[0].group), (1, Some(g)));
        // The quiet line goes to the group's conversation.
        assert!(
            p.b.messages
                .iter()
                .any(|m| m.group == Some(g) && matches!(m.kind, MessageKind::ShareNotice { .. }))
        );
        // B leaves the group: A withdraws the list from B, and B's requests are ignored.
        if let Some(x) = p.a.groups.get_mut(&g) {
            x.members.retain(|m| *m != p.b_id.0);
        }
        let _ = p.a.group_shares_follow(&mut p.ra);
        let item = seen[0].items[0].id;
        assert!(
            wanted(&p.a.received_share_request(
                p.b_id,
                ShareRef {
                    share_id: share.0,
                    item_id: item.0,
                }
            ))
            .is_none()
        );
        p.carry(true);
        assert_eq!(p.b.shared_with_me(None), Vec::new());
    }
}
