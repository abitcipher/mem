//! Group content types inside the end-to-end payload: small groups
//! over DTN by pairwise fan-out. Nothing here appears in the envelope.
//!
//! - type 5 [`GroupText`]: a member's message, one copy per member;
//! - type 6 [`GroupState`]: the admin's signed membership state (the inner signature covers it);
//! - type 7 [`GroupReply`]: an invitee joining (with its card) or a member leaving (without).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};
use crate::inner::MAX_TEXT_LEN;

/// Content type of a group message.
pub const CONTENT_TYPE_GROUP_TEXT: u64 = 5;
/// Content type of a group state.
pub const CONTENT_TYPE_GROUP_STATE: u64 = 6;
/// Content type of a reply to a group state.
pub const CONTENT_TYPE_GROUP_REPLY: u64 = 7;
/// Most members of a group, the admin included.
pub const MAX_GROUP_MEMBERS: usize = 16;
/// Group name length in bytes.
pub const MAX_GROUP_NAME_LEN: usize = 64;
/// Largest card accepted inside group content (as `spec/card.cddl` bounds it).
pub const MAX_GROUP_CARD_LEN: usize = 512;

/// A group message (type 5).
#[derive(Clone, PartialEq, Eq)]
pub struct GroupText {
    /// Group id.
    pub group: [u8; 16],
    /// The epoch the sender saw when sending.
    pub epoch: u32,
    /// Text, 1..=[`MAX_TEXT_LEN`] bytes.
    pub text: String,
}

impl fmt::Debug for GroupText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupText")
            .field("epoch", &self.epoch)
            .field("text_len", &self.text.len())
            .finish_non_exhaustive()
    }
}

impl GroupText {
    fn check(&self) -> Result<(), SchemaError> {
        if self.text.is_empty() || self.text.len() > MAX_TEXT_LEN {
            return Err(SchemaError::WrongLength { key: 2 });
        }
        Ok(())
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for empty or oversized text.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        Ok(encode_with(self.text.len() + 40, |e| {
            e.map(3)?
                .u8(0)?
                .bytes(&self.group)?
                .u8(1)?
                .u32(self.epoch)?
                .u8(2)?
                .str(&self.text)?;
            Ok(())
        }))
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_TEXT_LEN + 40,
            max_container_len: 3,
        };
        let mut r = MapReader::new(input, &limits)?;
        let group = r.array::<16>(0)?;
        let epoch = u32::try_from(r.u64(1)?).map_err(|_| SchemaError::WrongType { key: 1 })?;
        let text = String::from(r.text(2)?);
        r.finish()?;
        let t = Self { group, epoch, text };
        t.check()?;
        Ok(t)
    }
}

/// What a group state says about its recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupRole {
    /// Invited: may join by replying with its card.
    Invited,
    /// A member of this epoch.
    Member,
    /// No longer a member (removed, or its leave was applied).
    Removed,
}

impl GroupRole {
    const fn code(self) -> u8 {
        match self {
            Self::Invited => 0,
            Self::Member => 1,
            Self::Removed => 2,
        }
    }

    const fn from_code(c: u8) -> Option<Self> {
        match c {
            0 => Some(Self::Invited),
            1 => Some(Self::Member),
            2 => Some(Self::Removed),
            _ => None,
        }
    }
}

/// The admin's membership state (type 6). `cards` lists the members who joined, the admin first;
/// pairs of members who are not contacts derive their secret from the two cards here.
#[derive(Clone, PartialEq, Eq)]
pub struct GroupState {
    /// Group id.
    pub group: [u8; 16],
    /// Rises with every change; older states are ignored.
    pub epoch: u32,
    /// Name, 1..=[`MAX_GROUP_NAME_LEN`] bytes.
    pub name: String,
    /// Members' cards (`spec/card.cddl` bytes), 1..=[`MAX_GROUP_MEMBERS`], admin first.
    pub cards: Vec<Vec<u8>>,
    /// The recipient's role.
    pub role: GroupRole,
}

impl fmt::Debug for GroupState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupState")
            .field("epoch", &self.epoch)
            .field("members", &self.cards.len())
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

impl GroupState {
    fn check(&self) -> Result<(), SchemaError> {
        if self.name.is_empty() || self.name.len() > MAX_GROUP_NAME_LEN {
            return Err(SchemaError::WrongLength { key: 2 });
        }
        if self.cards.is_empty()
            || self.cards.len() > MAX_GROUP_MEMBERS
            || self
                .cards
                .iter()
                .any(|c| c.is_empty() || c.len() > MAX_GROUP_CARD_LEN)
        {
            return Err(SchemaError::WrongLength { key: 3 });
        }
        Ok(())
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for a name or member list out of bounds.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        let size = self.cards.iter().map(|c| c.len() + 3).sum::<usize>() + self.name.len() + 48;
        Ok(encode_with(size, |e| {
            e.map(5)?
                .u8(0)?
                .bytes(&self.group)?
                .u8(1)?
                .u32(self.epoch)?
                .u8(2)?
                .str(&self.name)?
                .u8(3)?
                .array(self.cards.len() as u64)?;
            for c in &self.cards {
                e.bytes(c)?;
            }
            e.u8(4)?.u8(self.role.code())?;
            Ok(())
        }))
    }

    /// Decodes untrusted content. The cards inside are decoded and verified separately.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_GROUP_MEMBERS * (MAX_GROUP_CARD_LEN + 3) + MAX_GROUP_NAME_LEN + 48,
            max_container_len: MAX_GROUP_MEMBERS as u64,
        };
        let mut r = MapReader::new(input, &limits)?;
        let group = r.array::<16>(0)?;
        let epoch = u32::try_from(r.u64(1)?).map_err(|_| SchemaError::WrongType { key: 1 })?;
        let name = String::from(r.text(2)?);
        let cards = r
            .bytes_list(3, MAX_GROUP_MEMBERS, MAX_GROUP_CARD_LEN)?
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect();
        let role = GroupRole::from_code(r.u8(4)?).ok_or(SchemaError::WrongType { key: 4 })?;
        r.finish()?;
        let s = Self {
            group,
            epoch,
            name,
            cards,
            role,
        };
        s.check()?;
        Ok(s)
    }
}

/// A reply to the admin (type 7): joining with one's current card, or leaving without one.
#[derive(Clone, PartialEq, Eq)]
pub struct GroupReply {
    /// Group id.
    pub group: [u8; 16],
    /// The sender's card when joining; `None` to decline or leave.
    pub card: Option<Vec<u8>>,
}

impl fmt::Debug for GroupReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupReply")
            .field("join", &self.card.is_some())
            .finish_non_exhaustive()
    }
}

impl GroupReply {
    fn check(&self) -> Result<(), SchemaError> {
        match &self.card {
            Some(c) if c.is_empty() || c.len() > MAX_GROUP_CARD_LEN => {
                Err(SchemaError::WrongLength { key: 1 })
            }
            _ => Ok(()),
        }
    }

    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for an empty or oversized card.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        self.check()?;
        let len = self.card.as_ref().map_or(0, Vec::len);
        Ok(encode_with(len + 32, |e| {
            e.map(1 + u64::from(self.card.is_some()))?
                .u8(0)?
                .bytes(&self.group)?;
            if let Some(c) = &self.card {
                e.u8(1)?.bytes(c)?;
            }
            Ok(())
        }))
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or oversized input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_GROUP_CARD_LEN + 32,
            max_container_len: 2,
        };
        let mut r = MapReader::new(input, &limits)?;
        let group = r.array::<16>(0)?;
        let card = if r.has(1) {
            Some(r.bytes(1)?.to_vec())
        } else {
            None
        };
        r.finish()?;
        let g = Self { group, card };
        g.check()?;
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    fn state() -> GroupState {
        GroupState {
            group: [7; 16],
            epoch: 3,
            name: "Hikers".into(),
            cards: alloc::vec![alloc::vec![1; 158], alloc::vec![2; 160]],
            role: GroupRole::Member,
        }
    }

    #[test]
    fn roundtrips() {
        let t = GroupText {
            group: [1; 16],
            epoch: 2,
            text: "hi all".into(),
        };
        assert_eq!(GroupText::decode(&t.encode().unwrap()).unwrap(), t);
        let s = state();
        assert_eq!(GroupState::decode(&s.encode().unwrap()).unwrap(), s);
        for card in [None, Some(alloc::vec![9; 150])] {
            let r = GroupReply {
                group: [4; 16],
                card,
            };
            assert_eq!(GroupReply::decode(&r.encode().unwrap()).unwrap(), r);
        }
    }

    /// Published vectors (`spec/vectors/group/`), validated against `spec/group.cddl` in CI.
    /// Regenerate with `MEM_UPDATE_VECTORS=1`.
    #[test]
    fn published_vectors() {
        extern crate std;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/group");
        let vectors = [
            (
                "group-text.cbor",
                GroupText {
                    group: [1; 16],
                    epoch: 2,
                    text: "trail at 9?".into(),
                }
                .encode()
                .unwrap(),
            ),
            ("group-state.cbor", state().encode().unwrap()),
            (
                "group-reply-join.cbor",
                GroupReply {
                    group: [7; 16],
                    card: Some(alloc::vec![3; 158]),
                }
                .encode()
                .unwrap(),
            ),
            (
                "group-reply-leave.cbor",
                GroupReply {
                    group: [7; 16],
                    card: None,
                }
                .encode()
                .unwrap(),
            ),
        ];
        for (name, bytes) in vectors {
            if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(name), &bytes).unwrap();
            }
            assert_eq!(
                std::fs::read(dir.join(name)).unwrap(),
                bytes,
                "{name} changed"
            );
        }
    }

    /// Sixteen cards of the largest measured size (158 bytes) stay in the 4 KiB padding bucket.
    #[test]
    fn a_full_state_fits_the_4_kib_bucket() {
        let mut s = state();
        s.name = "x".repeat(MAX_GROUP_NAME_LEN);
        s.cards = alloc::vec![alloc::vec![0xAB; 158]; MAX_GROUP_MEMBERS];
        assert!(s.encode().unwrap().len() < 4096 - 400);
    }

    #[test]
    fn bounds_are_enforced() {
        let mut s = state();
        s.cards = alloc::vec![alloc::vec![1; 10]; MAX_GROUP_MEMBERS + 1];
        assert!(s.encode().is_err());
        let mut s = state();
        s.name = String::new();
        assert!(s.encode().is_err());
        let t = GroupText {
            group: [1; 16],
            epoch: 1,
            text: "x".repeat(MAX_TEXT_LEN + 1),
        };
        assert!(t.encode().is_err());
        // An unknown role code.
        let mut bytes = state().encode().unwrap();
        let last = bytes.len() - 1;
        bytes[last] = 9;
        assert!(GroupState::decode(&bytes).is_err());
        // An unknown extra key.
        let mut r = GroupReply {
            group: [4; 16],
            card: None,
        }
        .encode()
        .unwrap();
        r[0] = 0xa2;
        r.extend_from_slice(&[0x02, 0x00]);
        assert!(GroupReply::decode(&r).is_err());
    }
}
