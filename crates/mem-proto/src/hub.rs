//! Client ↔ hub messages (`spec/hub.cddl`).
//!
//! One WebSocket per client; every binary frame is one message `[kind, {body}]` in deterministic
//! CBOR. The hub never decrypts: it stores envelopes by tag and deletes them only on a purge
//! token or expiry. Clients fetch by 2-byte tag prefixes padded with decoys,
//! so the hub never learns full tags from a fetch.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::encounter::{MAX_BUNDLES_PER_MSG, MAX_MSG_LEN, MAX_PURGE_TOKENS};
use crate::envelope::MAX_BUNDLE_LEN;
use crate::error::{ProtoError, SchemaError};

/// Hub protocol version.
pub const HUB_VERSION: u8 = 1;
/// Fewest prefixes in a `fetch` or `wake_register` (decoys pad up to it).
pub const MIN_PREFIXES: usize = 8;
/// Most prefixes in one `fetch` or `wake_register`.
pub const MAX_PREFIXES: usize = 64;
/// Longest push endpoint URL.
pub const MAX_ENDPOINT_LEN: usize = 1024;

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_MSG_LEN,
    max_container_len: MAX_PURGE_TOKENS as u64,
};

const KIND_HELLO: u8 = 0;
const KIND_PUT: u8 = 1;
const KIND_FETCH: u8 = 2;
const KIND_PURGE: u8 = 3;
const KIND_BYE: u8 = 4;
const KIND_WAKE_REGISTER: u8 = 5;
const KIND_WAKE_UNREGISTER: u8 = 6;
const KIND_GET_HUBS: u8 = 7;
const KIND_HUBS: u8 = 21;
const KIND_PEER_HELLO: u8 = 32;
const KIND_PEER_AUTH: u8 = 33;
const KIND_SYNC_FROM: u8 = 34;
const KIND_REPLICATE: u8 = 35;
const KIND_REPLICATE_PURGE: u8 = 36;
/// Domain separation for hub-to-hub authentication.
pub const HUB_PEER_DOMAIN: &[u8] = b"MEM/v1/hub-peer";
const KIND_WELCOME: u8 = 16;
const KIND_PUT_OK: u8 = 17;
const KIND_PUT_ERR: u8 = 18;
const KIND_BUNDLES: u8 = 19;
const KIND_FETCH_DONE: u8 = 20;

/// Whether an upload may cross to partner operators' hubs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossing {
    /// It stays with this hub's operator.
    OwnOperator,
    /// Partner operators' hubs may carry it too (grade P3 for the sender).
    Partners,
}

/// Where a bundle from the hub came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubOrigin {
    /// This hub's operator.
    OwnOperator,
    /// Another operator's hub (grade P3 for the receiver).
    OtherOperator,
}

/// The hub's limits, announced in `welcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubLimits {
    /// Largest bundle it stores.
    pub max_bundle: u32,
    /// Bytes per destination tag.
    pub per_tag: u32,
    /// Uploads per minute from one address.
    pub puts_per_minute: u32,
}

/// Why the hub did not store an upload. Never silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutError {
    /// Not a valid envelope.
    Malformed,
    /// Larger than the hub's limit.
    TooLarge,
    /// Lifetime already over.
    Expired,
    /// A quota (per tag, intro, total) is full.
    Quota,
    /// Too many uploads from this address.
    RateLimited,
    /// Its purge commitment is tombstoned.
    Purged,
    /// An intro bundle without a valid proof of work.
    BadProofOfWork,
    /// A version or kind the hub does not handle.
    Unsupported,
}

impl PutError {
    const fn code(self) -> u8 {
        match self {
            Self::Malformed => 0,
            Self::TooLarge => 1,
            Self::Expired => 2,
            Self::Quota => 3,
            Self::RateLimited => 4,
            Self::Purged => 5,
            Self::BadProofOfWork => 6,
            Self::Unsupported => 7,
        }
    }

    const fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            0 => Self::Malformed,
            1 => Self::TooLarge,
            2 => Self::Expired,
            3 => Self::Quota,
            4 => Self::RateLimited,
            5 => Self::Purged,
            6 => Self::BadProofOfWork,
            7 => Self::Unsupported,
            _ => return None,
        })
    }
}

/// One hub protocol message.
#[derive(Clone, PartialEq, Eq)]
pub enum HubMsg {
    /// Client opens the session.
    Hello {
        /// Protocol version.
        version: u8,
    },
    /// Client uploads one bundle.
    Put {
        /// Request id, echoed in the answer.
        id: u64,
        /// Encoded bundle (untrusted by the hub).
        bundle: Vec<u8>,
        /// The sender lets it cross to partner operators' hubs (key 2).
        may_cross: bool,
    },
    /// Client asks for everything under these tag prefixes (replaces an earlier fetch).
    Fetch(Vec<[u8; 2]>),
    /// Client presents purge tokens; the hub deletes and tombstones.
    Purge(Vec<[u8; 16]>),
    /// Either side ends the session.
    Bye,
    /// Client asks to be woken for these prefixes.
    WakeRegister {
        /// Push endpoint URL.
        endpoint: String,
        /// Tag prefixes, padded.
        prefixes: Vec<[u8; 2]>,
    },
    /// Client stops being woken at this endpoint.
    WakeUnregister {
        /// Push endpoint URL.
        endpoint: String,
    },
    /// Hub accepts the session.
    Welcome {
        /// Protocol version.
        version: u8,
        /// Its limits.
        limits: HubLimits,
    },
    /// Hub stored (or already had) an upload.
    PutOk {
        /// Request id.
        id: u64,
    },
    /// Hub refused an upload.
    PutErr {
        /// Request id.
        id: u64,
        /// Why.
        reason: PutError,
    },
    /// Hub sends stored or newly arrived bundles matching the fetch.
    Bundles {
        /// Encoded bundles.
        bundles: Vec<Vec<u8>>,
        /// Positions in `bundles` of those that came from another operator (key 1),
        /// strictly increasing.
        foreign: Vec<u8>,
    },
    /// Hub sent the whole backlog; live bundles follow.
    FetchDone,
    /// Client asks for the signed hub list.
    GetHubs,
    /// Hub sends the signed hub list (`spec/hublist.cddl` bytes, checked by the client).
    Hubs(Vec<u8>),
    /// Hub to hub: my hub key and a fresh nonce.
    PeerHello {
        /// Hub public key (Ed25519).
        key: [u8; 32],
        /// Random per connection.
        nonce: [u8; 16],
    },
    /// Hub to hub: signature over [`HUB_PEER_DOMAIN`] || peer's nonce || my nonce.
    PeerAuth([u8; 64]),
    /// Hub to hub: send me everything you stored after this sequence number.
    SyncFrom(u64),
    /// Hub to hub: a stored bundle and its sequence number at the sender.
    Replicate {
        /// Sender's sequence number.
        seq: u64,
        /// Envelope.
        bundle: Vec<u8>,
        /// When the bundle expires, Unix ms (its stored `age_ms` is from upload time).
        expires_unix_ms: u64,
        /// It came from another operator (key 3).
        foreign: bool,
        /// Its sender let it cross to partner operators (key 4).
        may_cross: bool,
    },
    /// Hub to hub: purge tokens applied, with the sender's sequence number.
    ReplicatePurge {
        /// Sender's sequence number.
        seq: u64,
        /// Tokens.
        tokens: Vec<[u8; 16]>,
    },
}

impl fmt::Debug for HubMsg {
    /// Never prints prefixes, endpoints or bundle bytes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hello { version } => write!(f, "Hello(v{version})"),
            Self::Put { id, bundle, .. } => write!(f, "Put(#{id}, {} B)", bundle.len()),
            Self::Fetch(p) => write!(f, "Fetch({})", p.len()),
            Self::Purge(t) => write!(f, "Purge({})", t.len()),
            Self::Bye => f.write_str("Bye"),
            Self::WakeRegister { prefixes, .. } => write!(f, "WakeRegister({})", prefixes.len()),
            Self::WakeUnregister { .. } => f.write_str("WakeUnregister"),
            Self::Welcome { version, .. } => write!(f, "Welcome(v{version})"),
            Self::PutOk { id } => write!(f, "PutOk(#{id})"),
            Self::PutErr { id, reason } => write!(f, "PutErr(#{id}, {reason:?})"),
            Self::Bundles { bundles, .. } => write!(f, "Bundles({})", bundles.len()),
            Self::FetchDone => f.write_str("FetchDone"),
            Self::GetHubs => f.write_str("GetHubs"),
            Self::Hubs(b) => write!(f, "Hubs({} B)", b.len()),
            Self::PeerHello { .. } => f.write_str("PeerHello"),
            Self::PeerAuth(_) => f.write_str("PeerAuth"),
            Self::SyncFrom(n) => write!(f, "SyncFrom({n})"),
            Self::Replicate { seq, .. } => write!(f, "Replicate(#{seq})"),
            Self::ReplicatePurge { seq, tokens } => {
                write!(f, "ReplicatePurge(#{seq}, {})", tokens.len())
            }
        }
    }
}

fn prefixes_ok(p: &[[u8; 2]]) -> bool {
    (MIN_PREFIXES..=MAX_PREFIXES).contains(&p.len())
}

fn endpoint_ok(e: &str) -> bool {
    !e.is_empty() && e.len() <= MAX_ENDPOINT_LEN
}

impl HubMsg {
    /// Canonical CBOR.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] if a list, bundle or endpoint is out of bounds.
    #[allow(clippy::too_many_lines, reason = "one arm per message kind")]
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        let bad = |key| Err(SchemaError::WrongLength { key }.into());
        match self {
            Self::Hello { version } => Ok(encode_with(8, |e| {
                e.array(2)?.u8(KIND_HELLO)?.map(1)?.u8(0)?.u8(*version)?;
                Ok(())
            })),
            Self::Put {
                id,
                bundle,
                may_cross,
            } => {
                if bundle.is_empty() || bundle.len() > MAX_BUNDLE_LEN {
                    return bad(1);
                }
                Ok(encode_with(bundle.len() + 24, |e| {
                    e.array(2)?.u8(KIND_PUT)?.map(2 + u64::from(*may_cross))?;
                    e.u8(0)?.u64(*id)?.u8(1)?.bytes(bundle)?;
                    if *may_cross {
                        e.u8(2)?.bool(true)?;
                    }
                    Ok(())
                }))
            }
            Self::Fetch(p) => {
                if !prefixes_ok(p) {
                    return bad(0);
                }
                Ok(encode_with(8 + p.len() * 3, |e| {
                    e.array(2)?.u8(KIND_FETCH)?.map(1)?.u8(0)?;
                    e.array(p.len() as u64)?;
                    for x in p {
                        e.bytes(x)?;
                    }
                    Ok(())
                }))
            }
            Self::Purge(t) => {
                if t.is_empty() || t.len() > MAX_PURGE_TOKENS {
                    return bad(0);
                }
                Ok(encode_with(8 + t.len() * 17, |e| {
                    e.array(2)?.u8(KIND_PURGE)?.map(1)?.u8(0)?;
                    e.array(t.len() as u64)?;
                    for x in t {
                        e.bytes(x)?;
                    }
                    Ok(())
                }))
            }
            Self::Bye => Ok(encode_with(4, |e| {
                e.array(2)?.u8(KIND_BYE)?.map(0)?;
                Ok(())
            })),
            Self::WakeRegister { endpoint, prefixes } => {
                if !endpoint_ok(endpoint) {
                    return bad(0);
                }
                if !prefixes_ok(prefixes) {
                    return bad(1);
                }
                Ok(encode_with(16 + endpoint.len() + prefixes.len() * 3, |e| {
                    e.array(2)?.u8(KIND_WAKE_REGISTER)?.map(2)?;
                    e.u8(0)?.str(endpoint)?.u8(1)?;
                    e.array(prefixes.len() as u64)?;
                    for x in prefixes {
                        e.bytes(x)?;
                    }
                    Ok(())
                }))
            }
            Self::WakeUnregister { endpoint } => {
                if !endpoint_ok(endpoint) {
                    return bad(0);
                }
                Ok(encode_with(8 + endpoint.len(), |e| {
                    e.array(2)?.u8(KIND_WAKE_UNREGISTER)?.map(1)?;
                    e.u8(0)?.str(endpoint)?;
                    Ok(())
                }))
            }
            Self::Welcome { version, limits } => Ok(encode_with(32, |e| {
                e.array(2)?.u8(KIND_WELCOME)?.map(2)?;
                e.u8(0)?.u8(*version)?.u8(1)?.map(3)?;
                e.u8(0)?.u32(limits.max_bundle)?;
                e.u8(1)?.u32(limits.per_tag)?;
                e.u8(2)?.u32(limits.puts_per_minute)?;
                Ok(())
            })),
            Self::PutOk { id } => Ok(encode_with(16, |e| {
                e.array(2)?.u8(KIND_PUT_OK)?.map(1)?.u8(0)?.u64(*id)?;
                Ok(())
            })),
            Self::PutErr { id, reason } => Ok(encode_with(16, |e| {
                e.array(2)?.u8(KIND_PUT_ERR)?.map(2)?;
                e.u8(0)?.u64(*id)?.u8(1)?.u8(reason.code())?;
                Ok(())
            })),
            Self::Bundles {
                bundles: b,
                foreign,
            } => {
                if b.len() > MAX_BUNDLES_PER_MSG
                    || b.iter().any(|x| x.is_empty() || x.len() > MAX_BUNDLE_LEN)
                {
                    return bad(0);
                }
                if !foreign_ok(foreign, b.len()) {
                    return bad(1);
                }
                let total: usize = b.iter().map(|x| x.len() + 5).sum();
                Ok(encode_with(8 + total + foreign.len() * 2, |e| {
                    e.array(2)?.u8(KIND_BUNDLES)?;
                    e.map(1 + u64::from(!foreign.is_empty()))?.u8(0)?;
                    e.array(b.len() as u64)?;
                    for x in b {
                        e.bytes(x)?;
                    }
                    if !foreign.is_empty() {
                        e.u8(1)?.array(foreign.len() as u64)?;
                        for i in foreign {
                            e.u8(*i)?;
                        }
                    }
                    Ok(())
                }))
            }
            Self::FetchDone => Ok(encode_with(4, |e| {
                e.array(2)?.u8(KIND_FETCH_DONE)?.map(0)?;
                Ok(())
            })),
            Self::GetHubs => Ok(encode_with(4, |e| {
                e.array(2)?.u8(KIND_GET_HUBS)?.map(0)?;
                Ok(())
            })),
            Self::Hubs(b) => {
                if b.is_empty() || b.len() > crate::hublist::MAX_HUB_LIST_LEN {
                    return bad(0);
                }
                Ok(encode_with(b.len() + 12, |e| {
                    e.array(2)?.u8(KIND_HUBS)?.map(1)?.u8(0)?.bytes(b)?;
                    Ok(())
                }))
            }
            Self::PeerHello { key, nonce } => Ok(encode_with(64, |e| {
                e.array(2)?.u8(KIND_PEER_HELLO)?.map(2)?;
                e.u8(0)?.bytes(key)?.u8(1)?.bytes(nonce)?;
                Ok(())
            })),
            Self::PeerAuth(sig) => Ok(encode_with(80, |e| {
                e.array(2)?.u8(KIND_PEER_AUTH)?.map(1)?.u8(0)?.bytes(sig)?;
                Ok(())
            })),
            Self::SyncFrom(n) => Ok(encode_with(16, |e| {
                e.array(2)?.u8(KIND_SYNC_FROM)?.map(1)?.u8(0)?.u64(*n)?;
                Ok(())
            })),
            Self::Replicate {
                seq,
                bundle,
                expires_unix_ms,
                foreign,
                may_cross,
            } => {
                if bundle.is_empty() || bundle.len() > MAX_BUNDLE_LEN {
                    return bad(1);
                }
                Ok(encode_with(bundle.len() + 40, |e| {
                    e.array(2)?.u8(KIND_REPLICATE)?;
                    e.map(3 + u64::from(*foreign) + u64::from(*may_cross))?;
                    e.u8(0)?.u64(*seq)?.u8(1)?.bytes(bundle)?;
                    e.u8(2)?.u64(*expires_unix_ms)?;
                    if *foreign {
                        e.u8(3)?.bool(true)?;
                    }
                    if *may_cross {
                        e.u8(4)?.bool(true)?;
                    }
                    Ok(())
                }))
            }
            Self::ReplicatePurge { seq, tokens } => {
                if tokens.is_empty() || tokens.len() > MAX_PURGE_TOKENS {
                    return bad(1);
                }
                Ok(encode_with(24 + tokens.len() * 17, |e| {
                    e.array(2)?.u8(KIND_REPLICATE_PURGE)?.map(2)?;
                    e.u8(0)?.u64(*seq)?.u8(1)?.array(tokens.len() as u64)?;
                    for t in tokens {
                        e.bytes(t)?;
                    }
                    Ok(())
                }))
            }
        }
    }

    /// Decodes an untrusted message, from either side.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for oversized, malformed, non-canonical or unknown messages.
    #[allow(clippy::too_many_lines, reason = "one arm per message kind")]
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        if input.len() > LIMITS.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: LIMITS.max_input_len,
            });
        }
        cbor::scan(input, &LIMITS)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(2)) {
            return Err(SchemaError::MapShape.into());
        }
        let kind = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
        let entries = d.map().ok().flatten().ok_or(SchemaError::MapShape)?;
        let msg = match (kind, entries) {
            (KIND_HELLO, 1) => {
                key(&mut d, 0)?;
                Self::Hello {
                    version: d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?,
                }
            }
            (KIND_PUT, 2..=3) => {
                key(&mut d, 0)?;
                let id = d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                let bundle = bounded_bytes(&mut d, 1)?;
                let may_cross = entries == 3;
                if may_cross {
                    flag(&mut d, 2)?;
                }
                Self::Put {
                    id,
                    bundle,
                    may_cross,
                }
            }
            (KIND_FETCH, 1) => {
                key(&mut d, 0)?;
                Self::Fetch(prefix_list(&mut d, 0)?)
            }
            (KIND_PURGE, 1) => {
                key(&mut d, 0)?;
                let n = list_len(&mut d, 1, MAX_PURGE_TOKENS)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(fixed::<16>(&mut d, 0)?);
                }
                Self::Purge(out)
            }
            (KIND_BYE, 0) => Self::Bye,
            (KIND_WAKE_REGISTER, 2) => {
                key(&mut d, 0)?;
                let endpoint = endpoint(&mut d)?;
                key(&mut d, 1)?;
                let prefixes = prefix_list(&mut d, 1)?;
                Self::WakeRegister { endpoint, prefixes }
            }
            (KIND_WAKE_UNREGISTER, 1) => {
                key(&mut d, 0)?;
                Self::WakeUnregister {
                    endpoint: endpoint(&mut d)?,
                }
            }
            (KIND_WELCOME, 2) => {
                key(&mut d, 0)?;
                let version = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                if d.map().ok().flatten() != Some(3) {
                    return Err(SchemaError::WrongType { key: 1 }.into());
                }
                let mut v = [0u32; 3];
                for (i, slot) in v.iter_mut().enumerate() {
                    key(&mut d, i as u64)?;
                    *slot = d.u32().map_err(|_| SchemaError::WrongType { key: 1 })?;
                }
                Self::Welcome {
                    version,
                    limits: HubLimits {
                        max_bundle: v[0],
                        per_tag: v[1],
                        puts_per_minute: v[2],
                    },
                }
            }
            (KIND_PUT_OK, 1) => {
                key(&mut d, 0)?;
                Self::PutOk {
                    id: d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?,
                }
            }
            (KIND_PUT_ERR, 2) => {
                key(&mut d, 0)?;
                let id = d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                let reason = d
                    .u8()
                    .ok()
                    .and_then(PutError::from_code)
                    .ok_or(SchemaError::UnknownValue { key: 1 })?;
                Self::PutErr { id, reason }
            }
            (KIND_BUNDLES, 1..=2) => {
                key(&mut d, 0)?;
                let n = list_len(&mut d, 0, MAX_BUNDLES_PER_MSG)?;
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(bounded_bytes(&mut d, 0)?);
                }
                let mut foreign = Vec::new();
                if entries == 2 {
                    key(&mut d, 1)?;
                    let m = list_len(&mut d, 1, MAX_BUNDLES_PER_MSG)?;
                    for _ in 0..m {
                        foreign.push(d.u8().map_err(|_| SchemaError::WrongType { key: 1 })?);
                    }
                    if !foreign_ok(&foreign, out.len()) {
                        return Err(SchemaError::UnknownValue { key: 1 }.into());
                    }
                }
                Self::Bundles {
                    bundles: out,
                    foreign,
                }
            }
            (KIND_FETCH_DONE, 0) => Self::FetchDone,
            (KIND_GET_HUBS, 0) => Self::GetHubs,
            (KIND_HUBS, 1) => {
                key(&mut d, 0)?;
                let b = d.bytes().map_err(|_| SchemaError::WrongType { key: 0 })?;
                if b.is_empty() || b.len() > crate::hublist::MAX_HUB_LIST_LEN {
                    return Err(SchemaError::WrongLength { key: 0 }.into());
                }
                Self::Hubs(b.to_vec())
            }
            (KIND_PEER_HELLO, 2) => {
                key(&mut d, 0)?;
                let k = fixed::<32>(&mut d, 0)?;
                key(&mut d, 1)?;
                Self::PeerHello {
                    key: k,
                    nonce: fixed::<16>(&mut d, 1)?,
                }
            }
            (KIND_PEER_AUTH, 1) => {
                key(&mut d, 0)?;
                Self::PeerAuth(fixed::<64>(&mut d, 0)?)
            }
            (KIND_SYNC_FROM, 1) => {
                key(&mut d, 0)?;
                Self::SyncFrom(d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?)
            }
            (KIND_REPLICATE, 3..=5) => {
                key(&mut d, 0)?;
                let seq = d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                let bundle = bounded_bytes(&mut d, 1)?;
                key(&mut d, 2)?;
                let expires_unix_ms = d.u64().map_err(|_| SchemaError::WrongType { key: 2 })?;
                // Optional flags, in key order, each only ever `true`.
                let (mut foreign, mut may_cross) = (false, false);
                for _ in 3..entries {
                    match d.u64() {
                        Ok(3) if !foreign && !may_cross => foreign = true,
                        Ok(4) if !may_cross => may_cross = true,
                        _ => return Err(SchemaError::MapShape.into()),
                    }
                    if d.bool().ok() != Some(true) {
                        return Err(SchemaError::UnknownValue { key: 3 }.into());
                    }
                }
                Self::Replicate {
                    seq,
                    bundle,
                    expires_unix_ms,
                    foreign,
                    may_cross,
                }
            }
            (KIND_REPLICATE_PURGE, 2) => {
                key(&mut d, 0)?;
                let seq = d.u64().map_err(|_| SchemaError::WrongType { key: 0 })?;
                key(&mut d, 1)?;
                let n = list_len(&mut d, 1, MAX_PURGE_TOKENS)?;
                let mut tokens = Vec::with_capacity(n);
                for _ in 0..n {
                    tokens.push(fixed::<16>(&mut d, 1)?);
                }
                Self::ReplicatePurge { seq, tokens }
            }
            (
                KIND_HELLO | KIND_PUT | KIND_FETCH | KIND_PURGE | KIND_BYE | KIND_WAKE_REGISTER
                | KIND_WAKE_UNREGISTER | KIND_WELCOME | KIND_PUT_OK | KIND_PUT_ERR | KIND_BUNDLES
                | KIND_FETCH_DONE | KIND_GET_HUBS | KIND_HUBS | KIND_PEER_HELLO | KIND_PEER_AUTH
                | KIND_SYNC_FROM | KIND_REPLICATE | KIND_REPLICATE_PURGE,
                _,
            ) => return Err(SchemaError::MapShape.into()),
            _ => return Err(SchemaError::UnknownValue { key: 0 }.into()),
        };
        if d.position() != input.len() {
            return Err(SchemaError::MapShape.into());
        }
        Ok(msg)
    }
}

/// Positions of foreign bundles: at most one per bundle, strictly increasing, in range.
fn foreign_ok(foreign: &[u8], len: usize) -> bool {
    foreign.windows(2).all(|w| w[0] < w[1]) && foreign.last().is_none_or(|i| usize::from(*i) < len)
}

/// An optional flag: present only as `key => true` (absent means false; canonical form).
fn flag(d: &mut Decoder<'_>, k: u64) -> Result<(), SchemaError> {
    key(d, k)?;
    if d.bool().ok() == Some(true) {
        Ok(())
    } else {
        Err(SchemaError::UnknownValue { key: k })
    }
}

fn key(d: &mut Decoder<'_>, expected: u64) -> Result<(), SchemaError> {
    match d.u64() {
        Ok(k) if k == expected => Ok(()),
        _ => Err(SchemaError::UnexpectedKey { expected }),
    }
}

fn fixed<const N: usize>(d: &mut Decoder<'_>, key: u64) -> Result<[u8; N], SchemaError> {
    d.bytes()
        .ok()
        .and_then(|b| <[u8; N]>::try_from(b).ok())
        .ok_or(SchemaError::WrongLength { key })
}

fn bounded_bytes(d: &mut Decoder<'_>, key: u64) -> Result<Vec<u8>, SchemaError> {
    let b = d.bytes().map_err(|_| SchemaError::WrongType { key })?;
    if b.is_empty() || b.len() > MAX_BUNDLE_LEN {
        return Err(SchemaError::WrongLength { key });
    }
    Ok(b.to_vec())
}

fn list_len(d: &mut Decoder<'_>, min: usize, max: usize) -> Result<usize, SchemaError> {
    d.array()
        .ok()
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (min..=max).contains(n))
        .ok_or(SchemaError::WrongLength { key: 0 })
}

fn prefix_list(d: &mut Decoder<'_>, key: u64) -> Result<Vec<[u8; 2]>, SchemaError> {
    let n =
        list_len(d, MIN_PREFIXES, MAX_PREFIXES).map_err(|_| SchemaError::WrongLength { key })?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(fixed::<2>(d, key)?);
    }
    Ok(out)
}

fn endpoint(d: &mut Decoder<'_>) -> Result<String, SchemaError> {
    let s = d.str().map_err(|_| SchemaError::WrongType { key: 0 })?;
    if !endpoint_ok(s) {
        return Err(SchemaError::WrongLength { key: 0 });
    }
    Ok(String::from(s))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    pub(crate) fn all() -> Vec<HubMsg> {
        let prefixes: Vec<[u8; 2]> = (0..8u8).map(|i| [i, 0xA0]).collect();
        alloc::vec![
            HubMsg::Hello { version: 1 },
            HubMsg::Put {
                id: 7,
                bundle: alloc::vec![0xAB; 300],
                may_cross: false,
            },
            HubMsg::Fetch(prefixes.clone()),
            HubMsg::Purge(alloc::vec![[3; 16], [4; 16]]),
            HubMsg::Bye,
            HubMsg::WakeRegister {
                endpoint: "https://push.example/u/abc".into(),
                prefixes,
            },
            HubMsg::WakeUnregister {
                endpoint: "https://push.example/u/abc".into(),
            },
            HubMsg::Welcome {
                version: 1,
                limits: HubLimits {
                    max_bundle: 65_536,
                    per_tag: 524_288,
                    puts_per_minute: 60,
                },
            },
            HubMsg::PutOk { id: 7 },
            HubMsg::PutErr {
                id: 8,
                reason: PutError::Quota,
            },
            HubMsg::Bundles {
                bundles: alloc::vec![alloc::vec![1; 10], alloc::vec![2; 20]],
                foreign: Vec::new(),
            },
            HubMsg::FetchDone,
            HubMsg::GetHubs,
            HubMsg::Hubs(alloc::vec![0xa6; 40]),
            HubMsg::PeerHello {
                key: [5; 32],
                nonce: [6; 16],
            },
            HubMsg::PeerAuth([7; 64]),
            HubMsg::SyncFrom(41),
            HubMsg::Replicate {
                seq: 42,
                bundle: alloc::vec![0xCD; 100],
                expires_unix_ms: 1_800_000_000_000,
                foreign: false,
                may_cross: false,
            },
            HubMsg::ReplicatePurge {
                seq: 43,
                tokens: alloc::vec![[8; 16]],
            },
            // Partner operators.
            HubMsg::Put {
                id: 9,
                bundle: alloc::vec![0xAB; 300],
                may_cross: true,
            },
            HubMsg::Bundles {
                bundles: alloc::vec![alloc::vec![1; 10], alloc::vec![2; 20], alloc::vec![3; 5]],
                foreign: alloc::vec![0, 2],
            },
            HubMsg::Replicate {
                seq: 44,
                bundle: alloc::vec![0xCD; 100],
                expires_unix_ms: 1_800_000_000_000,
                foreign: true,
                may_cross: false,
            },
            HubMsg::Replicate {
                seq: 45,
                bundle: alloc::vec![0xCD; 100],
                expires_unix_ms: 1_800_000_000_000,
                foreign: false,
                may_cross: true,
            },
        ]
    }

    #[test]
    fn roundtrips() {
        for m in all() {
            let bytes = m.encode().unwrap();
            assert_eq!(HubMsg::decode(&bytes).unwrap(), m, "{m:?}");
        }
    }

    #[test]
    fn bounds_are_enforced() {
        // Too few prefixes would reveal the real ones.
        assert!(HubMsg::Fetch(alloc::vec![[1, 2]; 7]).encode().is_err());
        assert!(HubMsg::Fetch(alloc::vec![[1, 2]; 65]).encode().is_err());
        assert!(HubMsg::Purge(Vec::new()).encode().is_err());
        assert!(
            HubMsg::Put {
                id: 1,
                bundle: alloc::vec![0; MAX_BUNDLE_LEN + 1],
                may_cross: false,
            }
            .encode()
            .is_err()
        );
        assert!(
            HubMsg::Bundles {
                bundles: alloc::vec![alloc::vec![1]; MAX_BUNDLES_PER_MSG + 1],
                foreign: Vec::new(),
            }
            .encode()
            .is_err()
        );
        // Foreign positions: in range, strictly increasing.
        for foreign in [alloc::vec![2], alloc::vec![1, 1], alloc::vec![1, 0]] {
            let m = HubMsg::Bundles {
                bundles: alloc::vec![alloc::vec![1]; 2],
                foreign,
            };
            assert!(m.encode().is_err(), "{m:?}");
        }
        // An unknown put error code and an unknown kind.
        let mut b = HubMsg::PutErr {
            id: 1,
            reason: PutError::Quota,
        }
        .encode()
        .unwrap();
        let last = b.len() - 1;
        b[last] = 0x09;
        assert!(HubMsg::decode(&b).is_err());
        assert!(HubMsg::decode(&[0x82, 0x18, 0x63, 0xa0]).is_err());
    }

    #[test]
    fn flags_are_canonical() {
        // `put` with key 2 => false is not canonical (absent means false).
        let mut b = HubMsg::Put {
            id: 1,
            bundle: alloc::vec![1],
            may_cross: true,
        }
        .encode()
        .unwrap();
        let last = b.len() - 1;
        assert_eq!(b[last], 0xf5);
        b[last] = 0xf4;
        assert!(HubMsg::decode(&b).is_err());
        // `replicate` flags must come in key order, once each.
        let r = HubMsg::Replicate {
            seq: 1,
            bundle: alloc::vec![1],
            expires_unix_ms: 2,
            foreign: true,
            may_cross: true,
        };
        let bytes = r.encode().unwrap();
        assert_eq!(HubMsg::decode(&bytes).unwrap(), r);
        let n = bytes.len();
        let mut swapped = bytes.clone();
        swapped[n - 4] = 0x04;
        swapped[n - 2] = 0x03;
        assert!(HubMsg::decode(&swapped).is_err());
    }

    /// Published vectors (`spec/vectors/hub/`), validated against `spec/hub.cddl` in CI.
    /// Regenerate with `MEM_UPDATE_VECTORS=1`.
    #[test]
    fn published_vectors() {
        extern crate std;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/hub");
        for (i, m) in all().into_iter().enumerate() {
            let bytes = m.encode().unwrap();
            let name = alloc::format!("{i:02}.cbor");
            if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(&name), &bytes).unwrap();
            }
            assert_eq!(std::fs::read(dir.join(&name)).unwrap(), bytes, "{name}");
        }
    }
}
