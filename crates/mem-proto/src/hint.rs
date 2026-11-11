//! A connect hint (`spec/hint.cddl`): where to reach a phone on the local network,
//! played as one sound frame or shown as one QR code (frame object kind 4).
//!
//! It carries the node's current Iroh endpoint id (a random transport key per app start, never
//! linked to the MEM identity) and at most two IPv4 socket addresses. Nothing in it is
//! trusted: a false hint can only make the phone dial a stranger, as mDNS already allows.

use alloc::vec::Vec;
use core::fmt;

use crate::cbor::{DecodeLimits, MapReader, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Most addresses in one hint (one sound frame).
pub const MAX_HINT_ADDRS: usize = 2;
/// Largest encoded hint (with two addresses it is 55 bytes).
pub const MAX_HINT_LEN: usize = 64;

/// One IPv4 address and UDP port: 4 bytes of address, then the port, big-endian.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HintAddr(pub [u8; 6]);

impl HintAddr {
    /// From an IPv4 address and a port.
    #[must_use]
    pub const fn new(ip: [u8; 4], port: u16) -> Self {
        let p = port.to_be_bytes();
        Self([ip[0], ip[1], ip[2], ip[3], p[0], p[1]])
    }

    /// The IPv4 address.
    #[must_use]
    pub const fn ip(&self) -> [u8; 4] {
        [self.0[0], self.0[1], self.0[2], self.0[3]]
    }

    /// The UDP port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        u16::from_be_bytes([self.0[4], self.0[5]])
    }
}

impl fmt::Debug for HintAddr {
    /// Addresses stay out of logs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HintAddr(…)")
    }
}

/// Where to reach a phone on the local network.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectHint {
    /// The node's current Iroh endpoint id (its random transport key for this app run).
    pub endpoint_id: [u8; 32],
    /// 1..=[`MAX_HINT_ADDRS`] local IPv4 socket addresses of that endpoint.
    pub addrs: Vec<HintAddr>,
}

impl fmt::Debug for ConnectHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectHint")
            .field("addrs", &self.addrs.len())
            .finish_non_exhaustive()
    }
}

impl ConnectHint {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] if there are no addresses or more than [`MAX_HINT_ADDRS`].
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        if self.addrs.is_empty() || self.addrs.len() > MAX_HINT_ADDRS {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        Ok(encode_with(MAX_HINT_LEN, |e| {
            e.map(3)?
                .u8(0)?
                .u8(1)?
                .u8(1)?
                .bytes(&self.endpoint_id)?
                .u8(2)?;
            e.array(self.addrs.len() as u64)?;
            for a in &self.addrs {
                e.bytes(&a.0)?;
            }
            Ok(())
        }))
    }

    /// Decodes an untrusted hint: bounded, canonical, version 1 only.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical or unknown-version input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_HINT_LEN,
            max_container_len: 3,
        };
        let mut r = MapReader::new(input, &limits)?;
        if r.u8(0)? != 1 {
            return Err(SchemaError::UnknownValue { key: 0 }.into());
        }
        let endpoint_id = r.array::<32>(1)?;
        let raw = r.bytes_list(2, MAX_HINT_ADDRS, 6)?;
        let mut addrs = Vec::with_capacity(raw.len());
        for a in raw {
            let a: [u8; 6] = a
                .try_into()
                .map_err(|_| SchemaError::WrongLength { key: 2 })?;
            addrs.push(HintAddr(a));
        }
        r.finish()?;
        Ok(Self { endpoint_id, addrs })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    fn hint(n: usize) -> ConnectHint {
        ConnectHint {
            endpoint_id: [9; 32],
            addrs: (0..n)
                .map(|i| {
                    let i = u8::try_from(i).unwrap();
                    HintAddr::new([192, 168, 1, 10 + i], 41_000 + u16::from(i))
                })
                .collect(),
        }
    }

    #[test]
    fn round_trip_fits_one_sound_frame() {
        for n in 1..=MAX_HINT_ADDRS {
            let h = hint(n);
            let b = h.encode().unwrap();
            assert!(b.len() <= MAX_HINT_LEN, "{}", b.len());
            assert_eq!(ConnectHint::decode(&b).unwrap(), h);
        }
        assert_eq!(hint(1).addrs[0].port(), 41_000);
        assert_eq!(hint(1).addrs[0].ip(), [192, 168, 1, 10]);
    }

    #[test]
    fn rejects_bad_counts_versions_and_trailing() {
        assert!(hint(0).encode().is_err());
        assert!(hint(3).encode().is_err());
        let b = hint(1).encode().unwrap();
        let mut v2 = b.clone();
        v2[2] = 2;
        assert!(ConnectHint::decode(&v2).is_err());
        let mut long = b;
        long.push(0);
        assert!(ConnectHint::decode(&long).is_err());
        assert!(ConnectHint::decode(&[]).is_err());
    }

    /// Published vector (`spec/vectors/hint/`). Regenerate with `MEM_UPDATE_VECTORS=1`.
    #[test]
    fn published_vector() {
        extern crate std;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/hint");
        let bytes = hint(2).encode().unwrap();
        if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("hint.cbor"), &bytes).unwrap();
        }
        assert_eq!(std::fs::read(dir.join("hint.cbor")).unwrap(), bytes);
    }
}
