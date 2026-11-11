//! Fountain-coded transfer frames for animated QR and other low-rate channels
//! (`spec/frame.cddl`).
//!
//! One frame carries one RaptorQ (RFC 6330) encoding symbol of an object (a bundle or a pairing
//! card) plus everything a receiver needs to start decoding from any frame, in any order.

use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Frame format version.
pub const FRAME_VERSION: u8 = 1;
/// Largest object a transfer can carry: the v1 bundle cap.
pub const MAX_OBJECT_LEN: u32 = 65_536;
/// Smallest symbol size. Bounds the source symbol count of any transfer to 1024.
pub const MIN_SYMBOL_LEN: u16 = 64;
/// Largest symbol size: more than any QR code version carries.
pub const MAX_SYMBOL_LEN: u16 = 1024;
/// Encoding symbol ids are 24 bits (RFC 6330 §3.2).
pub const MAX_ESI: u32 = (1 << 24) - 1;
/// Largest encoded frame.
pub const MAX_FRAME_LEN: usize = MAX_SYMBOL_LEN as usize + 32;
/// Length of a transfer id.
pub const TRANSFER_ID_LEN: usize = 8;

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_FRAME_LEN,
    max_container_len: 7,
};

/// What a transfer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    /// An encoded bundle (`envelope.cddl`).
    Bundle,
    /// An encoded pairing card (`card.cddl`).
    Card,
    /// An encrypted backup file (`backup.cddl`).
    Backup,
    /// A sealed device setup (`device.cddl`).
    DeviceSetup,
    /// Where to reach a phone on the local network (`hint.cddl`).
    ConnectHint,
}

impl ObjectKind {
    /// Wire value.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Bundle => 0,
            Self::Card => 1,
            Self::Backup => 2,
            Self::DeviceSetup => 3,
            Self::ConnectHint => 4,
        }
    }

    const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Bundle),
            1 => Some(Self::Card),
            2 => Some(Self::Backup),
            3 => Some(Self::DeviceSetup),
            4 => Some(Self::ConnectHint),
            _ => None,
        }
    }
}

/// Identifies a transfer and checks its integrity: a digest of the object.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransferId(pub [u8; TRANSFER_ID_LEN]);

impl fmt::Debug for TransferId {
    /// Identifier-like: never print it in full.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransferId({:02x}…)", self.0[0])
    }
}

/// One validated frame. Fields are private so every value satisfies the limits above.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    transfer: TransferId,
    kind: ObjectKind,
    object_len: u32,
    symbol_len: u16,
    esi: u32,
    symbol: Vec<u8>,
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("transfer", &self.transfer)
            .field("kind", &self.kind)
            .field("object_len", &self.object_len)
            .field("symbol_len", &self.symbol_len)
            .field("esi", &self.esi)
            .finish_non_exhaustive()
    }
}

impl Frame {
    /// Builds a frame, checking every limit.
    ///
    /// # Errors
    ///
    /// [`SchemaError`] if a value is outside its range or `symbol` is not `symbol_len` bytes.
    pub fn new(
        transfer: TransferId,
        kind: ObjectKind,
        object_len: u32,
        esi: u32,
        symbol: Vec<u8>,
    ) -> Result<Self, ProtoError> {
        if object_len == 0 || object_len > MAX_OBJECT_LEN {
            return Err(SchemaError::WrongType { key: 3 }.into());
        }
        let symbol_len = u16::try_from(symbol.len())
            .ok()
            .filter(|t| (MIN_SYMBOL_LEN..=MAX_SYMBOL_LEN).contains(t))
            .ok_or(SchemaError::WrongLength { key: 6 })?;
        if esi > MAX_ESI {
            return Err(SchemaError::WrongType { key: 5 }.into());
        }
        Ok(Self {
            transfer,
            kind,
            object_len,
            symbol_len,
            esi,
            symbol,
        })
    }

    /// Transfer id.
    #[must_use]
    pub const fn transfer(&self) -> TransferId {
        self.transfer
    }

    /// Object kind.
    #[must_use]
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// Length of the whole object in bytes.
    #[must_use]
    pub const fn object_len(&self) -> u32 {
        self.object_len
    }

    /// Symbol size of this transfer.
    #[must_use]
    pub const fn symbol_len(&self) -> u16 {
        self.symbol_len
    }

    /// Number of source symbols: `ceil(object_len / symbol_len)`, at most 1024.
    #[must_use]
    pub const fn source_symbols(&self) -> u32 {
        self.object_len.div_ceil(self.symbol_len as u32)
    }

    /// RaptorQ encoding symbol id.
    #[must_use]
    pub const fn esi(&self) -> u32 {
        self.esi
    }

    /// The encoding symbol, exactly [`Frame::symbol_len`] bytes.
    #[must_use]
    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }

    /// Takes the symbol out.
    #[must_use]
    pub fn into_symbol(self) -> Vec<u8> {
        self.symbol
    }

    /// Canonical CBOR encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(self.symbol.len() + 32, |e| {
            e.array(7)?
                .u8(FRAME_VERSION)?
                .bytes(&self.transfer.0)?
                .u8(self.kind.to_u8())?
                .u32(self.object_len)?
                .u16(self.symbol_len)?
                .u32(self.esi)?
                .bytes(&self.symbol)?;
            Ok(())
        })
    }

    /// Decodes an untrusted frame.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or out-of-range input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        if input.len() > LIMITS.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: LIMITS.max_input_len,
            });
        }
        cbor::scan(input, &LIMITS)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(7)) {
            return Err(SchemaError::MapShape.into());
        }
        let version = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
        if version != FRAME_VERSION {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let transfer = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; TRANSFER_ID_LEN]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 1 })?;
        let kind = d
            .u8()
            .ok()
            .and_then(ObjectKind::from_u8)
            .ok_or(SchemaError::UnknownValue { key: 2 })?;
        let object_len = d.u32().map_err(|_| SchemaError::WrongType { key: 3 })?;
        let symbol_len = d.u16().map_err(|_| SchemaError::WrongType { key: 4 })?;
        let esi = d.u32().map_err(|_| SchemaError::WrongType { key: 5 })?;
        let symbol = d.bytes().map_err(|_| SchemaError::WrongType { key: 6 })?;
        if usize::from(symbol_len) != symbol.len() {
            return Err(SchemaError::WrongLength { key: 6 }.into());
        }
        Self::new(TransferId(transfer), kind, object_len, esi, symbol.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn frame(esi: u32) -> Frame {
        Frame::new(
            TransferId([7; 8]),
            ObjectKind::Bundle,
            1000,
            esi,
            vec![0xab; 480],
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_and_overhead() {
        for esi in [0, 23, 24, 255, 256, 65_535, 65_536, MAX_ESI] {
            let f = frame(esi);
            let bytes = f.encode();
            assert_eq!(Frame::decode(&bytes).unwrap(), f);
            // Header overhead stays small: at most 28 bytes around the symbol.
            assert!(bytes.len() <= 480 + 28, "{}", bytes.len());
        }
        assert_eq!(frame(0).source_symbols(), 3);
    }

    #[test]
    fn rejects_out_of_range() {
        let t = TransferId([0; 8]);
        assert!(Frame::new(t, ObjectKind::Card, 0, 0, vec![0; 64]).is_err());
        assert!(Frame::new(t, ObjectKind::Card, MAX_OBJECT_LEN + 1, 0, vec![0; 64]).is_err());
        assert!(Frame::new(t, ObjectKind::Card, 10, 0, vec![0; 63]).is_err());
        assert!(Frame::new(t, ObjectKind::Card, 10, 0, vec![0; 1025]).is_err());
        assert!(Frame::new(t, ObjectKind::Card, 10, MAX_ESI + 1, vec![0; 64]).is_err());
    }

    #[test]
    fn rejects_bad_encodings() {
        let good = frame(1).encode();
        // Wrong version.
        let mut v = good.clone();
        v[1] = 0x02;
        assert!(Frame::decode(&v).is_err());
        // Unknown kind.
        let mut k = good.clone();
        k[11] = 0x05;
        assert!(Frame::decode(&k).is_err());
        // Symbol length disagrees with the declared symbol size.
        let short = Frame::new(TransferId([7; 8]), ObjectKind::Bundle, 1000, 1, vec![0; 64])
            .unwrap()
            .encode();
        let mut lie = short.clone();
        // symbol_len 64 is encoded as 0x18 0x40; claim 65.
        let pos = lie.windows(2).position(|w| w == [0x18, 0x40]).unwrap();
        lie[pos + 1] = 0x41;
        assert!(Frame::decode(&lie).is_err());
        // Truncation and trailing bytes.
        assert!(Frame::decode(&good[..good.len() - 1]).is_err());
        let mut trailing = good;
        trailing.push(0);
        assert!(Frame::decode(&trailing).is_err());
    }
}
