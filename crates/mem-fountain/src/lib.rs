//! Fountain coding for low-rate, lossy, one-way channels: animated QR now, later audio and BLE.
//!
//! The code is RaptorQ (RFC 6330, `raptorq` crate) with a single source block. A [`Sender`] turns
//! an object into an endless stream of [`Frame`]s: source symbols first, then fresh repair
//! symbols. A receiver feeds whatever frames it catches, in any order, into an [`Assembler`],
//! which completes after about `K` distinct frames (`K` = source symbols) and checks the result
//! against the transfer id, a digest of the object.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

use mem_proto::ProtoError;
use mem_proto::frame::{
    Frame, MAX_ESI, MAX_OBJECT_LEN, MAX_SYMBOL_LEN, MIN_SYMBOL_LEN, ObjectKind, TRANSFER_ID_LEN,
    TransferId,
};
use raptorq::{
    Decoder, EncodingPacket, ObjectTransmissionInformation, PayloadId, SourceBlockEncoder,
};

pub use mem_proto::frame;

const TRANSFER_CONTEXT: &str = "MEM v1 2026-10 qr-transfer";
/// Transfers a receiver tracks at once; the least recently fed one is dropped first.
pub const MAX_CONCURRENT_TRANSFERS: usize = 4;
/// Extra distinct symbols a transfer may collect beyond `K` before it is restarted. RaptorQ
/// almost always decodes from `K` or `K + 2` symbols, so this only bounds memory.
pub const MAX_EXTRA_SYMBOLS: u32 = 64;

/// Errors from the fountain layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FountainError {
    /// The object is empty or larger than [`MAX_OBJECT_LEN`].
    #[error("object length outside 1..=65536 bytes")]
    ObjectLength,
    /// The symbol size is outside `MIN_SYMBOL_LEN..=MAX_SYMBOL_LEN`.
    #[error("symbol size outside 64..=1024 bytes")]
    SymbolLength,
    /// A frame did not decode.
    #[error("bad frame: {0}")]
    Frame(#[from] ProtoError),
}

/// The transfer id for an object: `BLAKE3-derive_key(TRANSFER_CONTEXT, kind || object)[0..8]`.
/// An integrity check, not authentication.
#[must_use]
pub fn transfer_id(kind: ObjectKind, object: &[u8]) -> TransferId {
    let mut h = blake3::Hasher::new_derive_key(TRANSFER_CONTEXT);
    h.update(&[kind.to_u8()]);
    h.update(object);
    let mut id = [0u8; TRANSFER_ID_LEN];
    h.finalize_xof().fill(&mut id);
    TransferId(id)
}

/// RFC 6330 parameters for a transfer: one source block, no sub-blocks, alignment 1. Both
/// sides derive them from the frame header, so they never travel on the wire.
fn oti(object_len: u32, symbol_len: u16) -> ObjectTransmissionInformation {
    ObjectTransmissionInformation::new(u64::from(object_len), symbol_len, 1, 1, 1)
}

/// Produces the frames of one object.
pub struct Sender {
    transfer: TransferId,
    kind: ObjectKind,
    object_len: u32,
    source_symbols: u32,
    encoder: SourceBlockEncoder,
}

impl core::fmt::Debug for Sender {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sender")
            .field("transfer", &self.transfer)
            .field("source_symbols", &self.source_symbols)
            .finish_non_exhaustive()
    }
}

impl Sender {
    /// Prepares `object` for transfer in symbols of `symbol_len` bytes.
    ///
    /// # Errors
    ///
    /// [`FountainError`] if the object or symbol size is out of range.
    pub fn new(kind: ObjectKind, object: &[u8], symbol_len: u16) -> Result<Self, FountainError> {
        let object_len = u32::try_from(object.len())
            .ok()
            .filter(|l| (1..=MAX_OBJECT_LEN).contains(l))
            .ok_or(FountainError::ObjectLength)?;
        if !(MIN_SYMBOL_LEN..=MAX_SYMBOL_LEN).contains(&symbol_len) {
            return Err(FountainError::SymbolLength);
        }
        let config = oti(object_len, symbol_len);
        // The single source block is the object zero-padded to a whole number of symbols.
        let source_symbols = object_len.div_ceil(u32::from(symbol_len));
        let mut block = Vec::from(object);
        block.resize(source_symbols as usize * usize::from(symbol_len), 0);
        Ok(Self {
            transfer: transfer_id(kind, object),
            kind,
            object_len,
            source_symbols,
            encoder: SourceBlockEncoder::new(0, &config, &block),
        })
    }

    /// Number of source symbols `K`: the minimum number of frames a receiver needs.
    #[must_use]
    pub const fn source_symbols(&self) -> u32 {
        self.source_symbols
    }

    /// Transfer id.
    #[must_use]
    pub const fn transfer(&self) -> TransferId {
        self.transfer
    }

    /// Frame number `n` of the endless stream, encoded. Frames `0..K` carry the source symbols;
    /// later frames carry distinct repair symbols, so any `K` (rarely `K + 2`) frames suffice.
    /// Wraps around after 2^24 frames.
    #[must_use]
    pub fn frame(&self, n: u32) -> Vec<u8> {
        let esi = n & MAX_ESI;
        let packet = if esi < self.source_symbols {
            self.encoder.source_packets().into_iter().nth(esi as usize)
        } else {
            self.encoder
                .repair_packets(esi - self.source_symbols, 1)
                .into_iter()
                .next()
        };
        packet
            .and_then(|p| {
                let (id, data) = p.split();
                Frame::new(
                    self.transfer,
                    self.kind,
                    self.object_len,
                    id.encoding_symbol_id(),
                    data,
                )
                .ok()
            })
            .map(|f| f.encode())
            .unwrap_or_default()
    }
}

/// What feeding one frame achieved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// The frame belongs to a transfer still in progress.
    Partial {
        /// Distinct symbols collected.
        received: u32,
        /// Source symbols `K`; decoding usually completes at `K`.
        needed: u32,
    },
    /// A transfer completed and passed the integrity check.
    Complete {
        /// What it carried.
        kind: ObjectKind,
        /// The object bytes.
        object: Vec<u8>,
    },
    /// A frame of a transfer that already completed; nothing new.
    AlreadyComplete,
}

struct Transfer {
    id: TransferId,
    kind: ObjectKind,
    object_len: u32,
    symbol_len: u16,
    decoder: Decoder,
    seen: Vec<u32>,
    last_fed: u64,
}

/// Reassembles objects from frames (every input is hostile; memory is bounded by
/// [`MAX_CONCURRENT_TRANSFERS`] × (`K` + [`MAX_EXTRA_SYMBOLS`]) symbols of at most 1 KiB).
#[derive(Default)]
pub struct Assembler {
    transfers: Vec<Transfer>,
    completed: Vec<TransferId>,
    clock: u64,
}

impl core::fmt::Debug for Assembler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Assembler")
            .field("transfers", &self.transfers.len())
            .finish_non_exhaustive()
    }
}

impl Assembler {
    /// An empty assembler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one encoded frame.
    ///
    /// # Errors
    ///
    /// [`FountainError::Frame`] if the bytes are not a valid frame.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Progress, FountainError> {
        let frame = Frame::decode(bytes)?;
        self.clock += 1;
        let id = frame.transfer();
        if self.completed.contains(&id) {
            return Ok(Progress::AlreadyComplete);
        }
        let index = match self.transfers.iter().position(|t| {
            t.id == id
                && t.kind == frame.kind()
                && t.object_len == frame.object_len()
                && t.symbol_len == frame.symbol_len()
        }) {
            Some(i) => i,
            None => self.start(&frame),
        };
        let clock = self.clock;
        let Some(t) = self.transfers.get_mut(index) else {
            return Ok(Progress::Partial {
                received: 0,
                needed: frame.source_symbols(),
            });
        };
        t.last_fed = clock;
        let needed = frame.source_symbols();
        let esi = frame.esi();
        if t.seen.contains(&esi) {
            return Ok(Progress::Partial {
                received: count(&t.seen),
                needed,
            });
        }
        if count(&t.seen) >= needed + MAX_EXTRA_SYMBOLS {
            // Undecodable so far: someone is feeding junk under this id. Start over.
            t.decoder = Decoder::new(oti(t.object_len, t.symbol_len));
            t.seen.clear();
        }
        t.seen.push(esi);
        let packet = EncodingPacket::new(PayloadId::new(0, esi), frame.into_symbol());
        let Some(object) = t.decoder.decode(packet) else {
            return Ok(Progress::Partial {
                received: count(&t.seen),
                needed,
            });
        };
        let kind = t.kind;
        self.transfers.swap_remove(index);
        if transfer_id(kind, &object) != id {
            // Corrupt or forged symbols: drop the transfer; a clean retry can still complete.
            return Ok(Progress::Partial {
                received: 0,
                needed,
            });
        }
        if self.completed.len() >= 16 {
            self.completed.remove(0);
        }
        self.completed.push(id);
        Ok(Progress::Complete { kind, object })
    }

    /// Opens a transfer slot for `frame`, evicting the least recently fed one if full.
    fn start(&mut self, frame: &Frame) -> usize {
        if self.transfers.len() >= MAX_CONCURRENT_TRANSFERS
            && let Some(oldest) = self
                .transfers
                .iter()
                .enumerate()
                .min_by_key(|(_, t)| t.last_fed)
                .map(|(i, _)| i)
        {
            self.transfers.swap_remove(oldest);
        }
        self.transfers.push(Transfer {
            id: frame.transfer(),
            kind: frame.kind(),
            object_len: frame.object_len(),
            symbol_len: frame.symbol_len(),
            decoder: Decoder::new(oti(frame.object_len(), frame.symbol_len())),
            seen: Vec::new(),
            last_fed: self.clock,
        });
        self.transfers.len() - 1
    }

    /// Forgets every transfer in progress (e.g. when the scanner closes).
    pub fn reset(&mut self) {
        self.transfers.clear();
    }
}

fn count(seen: &[u32]) -> u32 {
    u32::try_from(seen.len()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{Rng, SeedableRng};

    fn object(len: usize, seed: u8) -> Vec<u8> {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let mut v = vec![0u8; len];
        rng.fill_bytes(&mut v);
        v
    }

    #[test]
    fn decodes_from_source_symbols_in_order() {
        let obj = object(5000, 1);
        let s = Sender::new(ObjectKind::Bundle, &obj, 480).unwrap();
        assert_eq!(s.source_symbols(), 11);
        let mut a = Assembler::new();
        let mut done = None;
        for n in 0..s.source_symbols() {
            if let Progress::Complete { kind, object } = a.push(&s.frame(n)).unwrap() {
                done = Some((kind, object, n));
            }
        }
        let (kind, got, at) = done.unwrap();
        assert_eq!((kind, got, at), (ObjectKind::Bundle, obj, 10));
        // A late frame of the same transfer is recognized, not restarted.
        assert_eq!(a.push(&s.frame(3)).unwrap(), Progress::AlreadyComplete);
    }

    /// The property animated QR relies on: any subset of about `K` frames, in any order, with
    /// many frames missed.
    #[test]
    fn decodes_from_random_subsets_with_small_overhead() {
        let mut rng = ChaCha20Rng::from_seed([9; 32]);
        let mut extra_total = 0u32;
        let trials = 40u32;
        for trial in 0..trials {
            let len = 200 + (rng.next_u32() as usize % 20_000);
            let obj = object(len, u8::try_from(trial).unwrap());
            let s = Sender::new(ObjectKind::Bundle, &obj, 384).unwrap();
            let k = s.source_symbols();
            let mut a = Assembler::new();
            let mut fed = 0u32;
            // Drop ~70% of frames; the camera misses most of them.
            let mut n = rng.next_u32() % 50;
            loop {
                n += 1 + rng.next_u32() % 4;
                fed += 1;
                if let Progress::Complete { object, .. } = a.push(&s.frame(n)).unwrap() {
                    assert_eq!(object, obj);
                    break;
                }
                assert!(fed < k + 10, "needed more than K + 10 frames");
            }
            extra_total += fed - k;
        }
        // RaptorQ: on average well under one extra frame per transfer.
        assert!(extra_total < trials, "extra frames: {extra_total}");
    }

    #[test]
    fn interleaved_transfers_and_eviction() {
        let objs: Vec<_> = (0..6u8).map(|i| object(2000, i + 20)).collect();
        let senders: Vec<_> = objs
            .iter()
            .map(|o| Sender::new(ObjectKind::Card, o, 256).unwrap())
            .collect();
        let mut a = Assembler::new();
        // Interleave two transfers: both complete.
        let mut done = 0;
        for n in 0..senders[0].source_symbols() {
            for s in &senders[..2] {
                if matches!(a.push(&s.frame(n)).unwrap(), Progress::Complete { .. }) {
                    done += 1;
                }
            }
        }
        assert_eq!(done, 2);
        // Six partial transfers: only MAX_CONCURRENT_TRANSFERS are tracked.
        for s in &senders {
            a.push(&s.frame(0)).unwrap();
        }
        assert!(a.transfers.len() <= MAX_CONCURRENT_TRANSFERS);
    }

    #[test]
    fn forged_symbols_never_complete_with_wrong_data() {
        let obj = object(1500, 3);
        let s = Sender::new(ObjectKind::Bundle, &obj, 512).unwrap();
        let mut a = Assembler::new();
        // An attacker replays the header with garbage symbols.
        for esi in 0..3 {
            let junk = Frame::new(s.transfer(), ObjectKind::Bundle, 1500, esi, vec![0x55; 512])
                .unwrap()
                .encode();
            assert!(!matches!(a.push(&junk).unwrap(), Progress::Complete { .. }));
        }
        // The real frames still get through afterwards.
        let mut ok = false;
        for n in 10..40 {
            if let Progress::Complete { object, .. } = a.push(&s.frame(n)).unwrap() {
                assert_eq!(object, obj);
                ok = true;
                break;
            }
        }
        assert!(ok);
    }

    #[test]
    fn bounds_symbols_per_transfer() {
        let s = Sender::new(ObjectKind::Bundle, &object(64 * 10, 4), 64).unwrap();
        let mut a = Assembler::new();
        // Distinct junk symbols beyond K + MAX_EXTRA_SYMBOLS restart the transfer.
        for esi in 0..200 {
            let junk = Frame::new(
                s.transfer(),
                ObjectKind::Bundle,
                640,
                1000 + esi,
                vec![1; 64],
            )
            .unwrap()
            .encode();
            let _ = a.push(&junk);
            assert!(a.transfers.iter().all(|t| t.seen.len() <= 10 + 64));
        }
    }

    #[test]
    fn rejects_bad_parameters() {
        assert_eq!(
            Sender::new(ObjectKind::Bundle, &[], 256).unwrap_err(),
            FountainError::ObjectLength
        );
        assert_eq!(
            Sender::new(ObjectKind::Bundle, &[1], 63).unwrap_err(),
            FountainError::SymbolLength
        );
        assert!(Sender::new(ObjectKind::Bundle, &vec![0; 65_537], 1024).is_err());
        assert!(Assembler::new().push(b"not a frame").is_err());
    }
}
