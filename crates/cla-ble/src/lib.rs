//! Bluetooth LE channel, Rust side.
//!
//! The radio work (advertising, scanning, L2CAP or GATT links) lives in the native `mem_radio`
//! plugin, because Rust cannot reach the radio on Android. The plugin hands the runtime a byte
//! stream per link, in whatever pieces the radio delivered (L2CAP reads or GATT chunks of
//! `ATT MTU − 3` bytes). This crate turns those pieces back into encounter messages and keeps the
//! link ↔ peer bookkeeping:
//!
//! - **Framing:** the encounter stream framing, the same as Iroh: a `u32` big-endian length,
//!   then the CBOR message, at most [`MAX_MSG_LEN`] bytes, checked before anything is buffered.
//! - **Handles:** a link id from the plugin becomes a peer handle with [`HANDLE_BIT`] set, so BLE
//!   peers never collide with Iroh peers (whose handles count up from 1). Handles are local and
//!   never on the wire.
//!
//! Nothing received here is trusted: BLE links are unauthenticated.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use mem_proto::encounter::EncounterMsg;
pub use mem_proto::encounter::MAX_MSG_LEN;

/// Set on every BLE peer handle.
pub const HANDLE_BIT: u64 = 1 << 63;

/// Most links open at once. Phones keep only a handful of BLE connections; more would only be
/// someone flooding us.
pub const MAX_LINKS: usize = 8;

/// Whether a peer handle belongs to a BLE link.
#[must_use]
pub const fn is_ble(peer: u64) -> bool {
    peer & HANDLE_BIT != 0
}

/// One encounter message as stream bytes: length prefix, then CBOR. `None` if it cannot be
/// encoded or is larger than a peer would accept.
#[must_use]
pub fn frame(msg: &EncounterMsg) -> Option<Vec<u8>> {
    let body = msg.encode().ok()?;
    if body.is_empty() || body.len() > MAX_MSG_LEN {
        return None;
    }
    let len = u32::try_from(body.len()).ok()?;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Some(out)
}

/// Why a link's stream is unusable. The link must be closed: a stream cannot resynchronise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// A zero length.
    Empty,
    /// A length above [`MAX_MSG_LEN`].
    TooLong,
    /// The message did not decode.
    Malformed,
}

/// Reassembles length-prefixed messages from pieces of any size.
#[derive(Debug, Default)]
pub struct Deframer {
    /// The current frame so far: its 4 length bytes, then body bytes.
    buf: Vec<u8>,
}

impl Deframer {
    /// Adds received bytes; returns every message they complete, in order.
    ///
    /// # Errors
    ///
    /// [`FrameError`] at the first bad frame. Messages completed before it are lost with the link.
    ///
    /// # Security
    ///
    /// The length is checked before any body byte is buffered, so a peer can make us hold at most
    /// [`MAX_MSG_LEN`] + 4 bytes per link.
    pub fn push(&mut self, mut input: &[u8]) -> Result<Vec<EncounterMsg>, FrameError> {
        let mut out = Vec::new();
        while !input.is_empty() {
            if self.buf.len() < 4 {
                let take = (4 - self.buf.len()).min(input.len());
                self.buf.extend_from_slice(&input[..take]);
                input = &input[take..];
                if self.buf.len() < 4 {
                    break;
                }
            }
            let len = self.body_len()?;
            let take = (4 + len - self.buf.len()).min(input.len());
            self.buf.extend_from_slice(&input[..take]);
            input = &input[take..];
            if self.buf.len() == 4 + len {
                let msg =
                    EncounterMsg::decode(&self.buf[4..]).map_err(|_| FrameError::Malformed)?;
                out.push(msg);
                self.buf.clear();
            }
        }
        // A header alone is checked now, not when its body starts to arrive.
        if self.buf.len() >= 4 {
            self.body_len()?;
        }
        Ok(out)
    }

    fn body_len(&self) -> Result<usize, FrameError> {
        let mut len = [0u8; 4];
        len.copy_from_slice(&self.buf[..4]);
        let len = usize::try_from(u32::from_be_bytes(len)).map_err(|_| FrameError::TooLong)?;
        match len {
            0 => Err(FrameError::Empty),
            n if n > MAX_MSG_LEN => Err(FrameError::TooLong),
            n => Ok(n),
        }
    }
}

/// What received bytes meant for a link.
#[derive(Debug, PartialEq, Eq)]
pub enum Received {
    /// Complete messages (possibly none yet).
    Messages(Vec<EncounterMsg>),
    /// The stream broke; the link is forgotten and must be closed.
    Broken(FrameError),
    /// No such open link.
    Unknown,
}

/// The open links.
#[derive(Debug, Default)]
pub struct Links {
    links: BTreeMap<u64, Deframer>,
}

impl Links {
    /// The peer handle of a plugin link id. `None` for ids that would not fit.
    #[must_use]
    pub const fn peer(link: u64) -> Option<u64> {
        if is_ble(link) {
            None
        } else {
            Some(link | HANDLE_BIT)
        }
    }

    /// The plugin link id of a BLE peer handle.
    #[must_use]
    pub const fn link(peer: u64) -> Option<u64> {
        if is_ble(peer) {
            Some(peer & !HANDLE_BIT)
        } else {
            None
        }
    }

    /// A link came up. Returns its peer handle, or `None` if the link must be closed (unusable
    /// id, already open, or [`MAX_LINKS`] reached).
    pub fn open(&mut self, link: u64) -> Option<u64> {
        let peer = Self::peer(link)?;
        if self.links.contains_key(&link) || self.links.len() >= MAX_LINKS {
            return None;
        }
        self.links.insert(link, Deframer::default());
        Some(peer)
    }

    /// Bytes arrived on `link`.
    pub fn receive(&mut self, link: u64, bytes: &[u8]) -> Received {
        let Some(d) = self.links.get_mut(&link) else {
            return Received::Unknown;
        };
        match d.push(bytes) {
            Ok(msgs) => Received::Messages(msgs),
            Err(e) => {
                self.links.remove(&link);
                Received::Broken(e)
            }
        }
    }

    /// The link went down (or we closed it). Returns its peer handle if it was open.
    pub fn close(&mut self, link: u64) -> Option<u64> {
        self.links.remove(&link).and_then(|_| Self::peer(link))
    }

    /// Forgets every link (BLE turned off or the plugin stopped). Returns their peer handles.
    pub fn clear(&mut self) -> Vec<u64> {
        let peers = self.links.keys().filter_map(|l| Self::peer(*l)).collect();
        self.links.clear();
        peers
    }

    /// Number of open links.
    #[must_use]
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Whether no link is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(n: u8) -> EncounterMsg {
        EncounterMsg::Hello {
            version: 1,
            ephemeral_id: [n; 16],
        }
    }

    fn tags(count: usize) -> EncounterMsg {
        EncounterMsg::Tags(
            (0..count)
                .map(|i| [u8::try_from(i % 251).unwrap(); 16])
                .collect(),
        )
    }

    /// GATT fallback: the stream arrives in `ATT MTU − 3` chunks of any size, frames split
    /// across chunks and several frames per chunk; every split gives back the same messages.
    #[test]
    fn reassembles_any_chunking() {
        let msgs = [hello(1), tags(300), EncounterMsg::Bye, hello(2)];
        let stream: Vec<u8> = msgs.iter().flat_map(|m| frame(m).unwrap()).collect();
        for chunk in [1, 2, 3, 4, 5, 17, 20, 182, 244, 509, 4096, stream.len()] {
            let mut d = Deframer::default();
            let mut got = Vec::new();
            for piece in stream.chunks(chunk) {
                got.extend(d.push(piece).unwrap());
            }
            assert_eq!(got, msgs, "chunk size {chunk}");
            assert_eq!(d.buf, Vec::<u8>::new());
        }
    }

    #[test]
    fn rejects_bad_lengths_before_buffering() {
        let mut d = Deframer::default();
        assert_eq!(d.push(&[0, 0, 0, 0]), Err(FrameError::Empty));
        let too_long = u32::try_from(MAX_MSG_LEN + 1).unwrap().to_be_bytes();
        let mut d = Deframer::default();
        // The header alone is refused: no body byte is waited for.
        assert_eq!(d.push(&too_long), Err(FrameError::TooLong));
        let mut d = Deframer::default();
        assert_eq!(d.push(&[0xff, 0xff]), Ok(Vec::new()));
        assert_eq!(d.push(&[0xff, 0xff, 1]), Err(FrameError::TooLong));
        // The largest allowed length is accepted as a header.
        let max = u32::try_from(MAX_MSG_LEN).unwrap().to_be_bytes();
        assert_eq!(Deframer::default().push(&max), Ok(Vec::new()));
    }

    #[test]
    fn rejects_malformed_messages() {
        let mut d = Deframer::default();
        assert_eq!(
            d.push(&[0, 0, 0, 2, 0xff, 0xff]),
            Err(FrameError::Malformed)
        );
    }

    #[test]
    fn links_map_to_distinct_handles_and_are_bounded() {
        let mut links = Links::default();
        let peer = links.open(1).unwrap();
        assert!(is_ble(peer));
        assert_ne!(peer, 1, "never equal to an Iroh handle");
        assert_eq!(Links::link(peer), Some(1));
        assert_eq!(Links::link(1), None);
        assert_eq!(links.open(1), None, "already open");
        assert_eq!(links.open(HANDLE_BIT | 5), None, "unusable id");
        for l in 2..=u64::try_from(MAX_LINKS).unwrap() {
            assert!(links.open(l).is_some());
        }
        assert_eq!(links.open(100), None, "full");
        assert_eq!(links.close(1), Some(peer));
        assert_eq!(links.close(1), None);
        assert!(links.open(100).is_some());
        assert_eq!(links.clear().len(), MAX_LINKS);
        assert!(links.is_empty());
    }

    #[test]
    fn a_broken_stream_forgets_the_link() {
        let mut links = Links::default();
        links.open(7).unwrap();
        let f = frame(&hello(3)).unwrap();
        assert_eq!(links.receive(7, &f[..3]), Received::Messages(Vec::new()));
        assert_eq!(
            links.receive(7, &f[3..]),
            Received::Messages(vec![hello(3)])
        );
        assert_eq!(
            links.receive(7, &[0, 0, 0, 0]),
            Received::Broken(FrameError::Empty)
        );
        assert_eq!(links.receive(7, &f), Received::Unknown);
        assert_eq!(links.close(7), None);
    }
}
