//! NIF for `mem_hub`: the hub decodes and checks everything with the very code
//! the apps use, so there is one bounded, fuzzed parser for the protocol, not two.
//!
//! - `decode_msg/1`, `encode_msg/1`: hub protocol messages <-> Elixir terms.
//! - `check_bundle/2`: validates an upload (envelope, expiry, intro proof of work) and returns
//!   the fields the hub stores. Proof of work is Argon2id, so it runs on a dirty CPU scheduler.
//! - `commit/1`: the purge commitment of a token.
//! - `hub_public/1`, `peer_sign/3`, `peer_verify/4`, `verify_hub_list/3`: the hub network:
//! hub keys, the hub-to-hub handshake and the operator-signed list.
//!
//! Nothing panics on hostile input: every decoder returns errors.
#![forbid(unsafe_code)]

use mem_crypto::PurgeToken;
use mem_crypto::intro::{POW_DIFFICULTY, pow_valid, pow_weeks};
use mem_crypto::purge::commit as purge_commit;
use mem_crypto::{IdentityKey as Identity, IdentityPublic};
use mem_proto::hub::{HUB_PEER_DOMAIN, HubLimits, HubMsg, PutError};
use mem_proto::hublist::{HUB_LIST_DOMAIN, HubList};
use mem_proto::{Bundle, BundleKind, InvalidBundle, ProtoError};
use rustler::{Atom, Binary, Env, NifResult, OwnedBinary, Term};

mod atoms {
    rustler::atoms! {
        ok, error,
        hello, put, fetch, purge, bye, wake_register, wake_unregister,
        welcome, put_ok, put_err, bundles, fetch_done,
        malformed, too_large, expired, quota, rate_limited, purged, bad_proof_of_work, unsupported,
        message, ack, intro, control, key_update,
        get_hubs, hubs, peer_hello, peer_auth, sync_from, replicate, replicate_purge,
    }
}

/// A binary term; `:error` if the VM cannot allocate (never a panic).
fn bin<'a>(env: Env<'a>, bytes: &[u8]) -> Term<'a> {
    match OwnedBinary::new(bytes.len()) {
        Some(mut out) => {
            out.as_mut_slice().copy_from_slice(bytes);
            out.release(env).encode(env)
        }
        None => atoms::error().encode(env),
    }
}

use rustler::Encoder;

/// A client message as a term: `{:hello, v}`, `{:put, id, bundle, may_cross}`, `{:fetch, [prefix]}`,
/// `{:purge, [token]}`, `:bye`, `{:wake_register, url, [prefix]}`, `{:wake_unregister, url}`.
/// Hub-to-client kinds decode too (for tests). `{:error, :malformed}` for anything invalid.
#[rustler::nif]
fn decode_msg<'a>(env: Env<'a>, data: Binary<'a>) -> Term<'a> {
    let Ok(msg) = HubMsg::decode(data.as_slice()) else {
        return (atoms::error(), atoms::malformed()).encode(env);
    };
    let list = |items: Vec<Term<'a>>| items.encode(env);
    let t = match msg {
        HubMsg::Hello { version } => (atoms::hello(), version).encode(env),
        HubMsg::Put {
            id,
            bundle,
            may_cross,
        } => (atoms::put(), id, bin(env, &bundle), may_cross).encode(env),
        HubMsg::Fetch(p) => (
            atoms::fetch(),
            list(p.iter().map(|x| bin(env, x)).collect()),
        )
            .encode(env),
        HubMsg::Purge(t) => (
            atoms::purge(),
            list(t.iter().map(|x| bin(env, x)).collect()),
        )
            .encode(env),
        HubMsg::Bye => atoms::bye().encode(env),
        HubMsg::WakeRegister { endpoint, prefixes } => (
            atoms::wake_register(),
            endpoint,
            list(prefixes.iter().map(|x| bin(env, x)).collect()),
        )
            .encode(env),
        HubMsg::WakeUnregister { endpoint } => (atoms::wake_unregister(), endpoint).encode(env),
        HubMsg::Welcome { version, .. } => (atoms::welcome(), version).encode(env),
        HubMsg::PutOk { id } => (atoms::put_ok(), id).encode(env),
        HubMsg::PutErr { id, reason } => (atoms::put_err(), id, reason_atom(reason)).encode(env),
        HubMsg::Bundles { bundles, foreign } => (
            atoms::bundles(),
            list(bundles.iter().map(|x| bin(env, x)).collect()),
            foreign,
        )
            .encode(env),
        HubMsg::FetchDone => atoms::fetch_done().encode(env),
        HubMsg::GetHubs => atoms::get_hubs().encode(env),
        HubMsg::Hubs(b) => (atoms::hubs(), bin(env, &b)).encode(env),
        HubMsg::PeerHello { key, nonce } => {
            (atoms::peer_hello(), bin(env, &key), bin(env, &nonce)).encode(env)
        }
        HubMsg::PeerAuth(sig) => (atoms::peer_auth(), bin(env, &sig)).encode(env),
        HubMsg::SyncFrom(n) => (atoms::sync_from(), n).encode(env),
        HubMsg::Replicate {
            seq,
            bundle,
            expires_unix_ms,
            foreign,
            may_cross,
        } => (
            atoms::replicate(),
            seq,
            bin(env, &bundle),
            expires_unix_ms,
            foreign,
            may_cross,
        )
            .encode(env),
        HubMsg::ReplicatePurge { seq, tokens } => (
            atoms::replicate_purge(),
            seq,
            list(tokens.iter().map(|x| bin(env, x)).collect()),
        )
            .encode(env),
    };
    (atoms::ok(), t).encode(env)
}

fn reason_atom(r: PutError) -> Atom {
    match r {
        PutError::Malformed => atoms::malformed(),
        PutError::TooLarge => atoms::too_large(),
        PutError::Expired => atoms::expired(),
        PutError::Quota => atoms::quota(),
        PutError::RateLimited => atoms::rate_limited(),
        PutError::Purged => atoms::purged(),
        PutError::BadProofOfWork => atoms::bad_proof_of_work(),
        PutError::Unsupported => atoms::unsupported(),
    }
}

fn reason_from(a: Atom) -> Option<PutError> {
    [
        PutError::Malformed,
        PutError::TooLarge,
        PutError::Expired,
        PutError::Quota,
        PutError::RateLimited,
        PutError::Purged,
        PutError::BadProofOfWork,
        PutError::Unsupported,
    ]
    .into_iter()
    .find(|r| reason_atom(*r) == a)
}

/// A hub message from a term: `{:welcome, version, max_bundle, per_tag, puts_per_minute}`,
/// `{:put_ok, id}`, `{:put_err, id, reason}`, `{:bundles, [bin]}` or `{:bundles, [bin], [foreign index]}`,
/// `{:replicate, seq, bundle, expires, foreign, may_cross}`, `:fetch_done`, `:bye`.
#[rustler::nif]
fn encode_msg<'a>(env: Env<'a>, term: Term<'a>) -> NifResult<Term<'a>> {
    let msg = if let Ok(a) = term.decode::<Atom>() {
        if a == atoms::fetch_done() {
            HubMsg::FetchDone
        } else if a == atoms::get_hubs() {
            HubMsg::GetHubs
        } else if a == atoms::bye() {
            HubMsg::Bye
        } else {
            return Err(rustler::Error::BadArg);
        }
    } else if let Ok((tag, version, max_bundle, per_tag, ppm)) =
        term.decode::<(Atom, u8, u32, u32, u32)>()
        && tag == atoms::welcome()
    {
        HubMsg::Welcome {
            version,
            limits: HubLimits {
                max_bundle,
                per_tag,
                puts_per_minute: ppm,
            },
        }
    } else if let Ok((tag, id, reason)) = term.decode::<(Atom, u64, Atom)>()
        && tag == atoms::put_err()
    {
        HubMsg::PutErr {
            id,
            reason: reason_from(reason).ok_or(rustler::Error::BadArg)?,
        }
    } else if let Ok((tag, id)) = term.decode::<(Atom, u64)>()
        && tag == atoms::put_ok()
    {
        HubMsg::PutOk { id }
    } else if let Ok((tag, list)) = term.decode::<(Atom, Vec<Binary>)>()
        && tag == atoms::bundles()
    {
        HubMsg::Bundles {
            bundles: list.iter().map(|b| b.as_slice().to_vec()).collect(),
            foreign: Vec::new(),
        }
    } else if let Ok((tag, list, foreign)) = term.decode::<(Atom, Vec<Binary>, Vec<u8>)>()
        && tag == atoms::bundles()
    {
        HubMsg::Bundles {
            bundles: list.iter().map(|b| b.as_slice().to_vec()).collect(),
            foreign,
        }
    } else if let Ok((tag, b)) = term.decode::<(Atom, Binary)>() {
        if tag == atoms::hubs() {
            HubMsg::Hubs(b.as_slice().to_vec())
        } else if tag == atoms::peer_auth() {
            HubMsg::PeerAuth(fixed(&b)?)
        } else {
            return Err(rustler::Error::BadArg);
        }
    } else if let Ok((tag, key, nonce)) = term.decode::<(Atom, Binary, Binary)>()
        && tag == atoms::peer_hello()
    {
        HubMsg::PeerHello {
            key: fixed(&key)?,
            nonce: fixed(&nonce)?,
        }
    } else if let Ok((tag, n)) = term.decode::<(Atom, u64)>()
        && tag == atoms::sync_from()
    {
        HubMsg::SyncFrom(n)
    } else if let Ok((tag, seq, bundle, expires, foreign, may_cross)) =
        term.decode::<(Atom, u64, Binary, u64, bool, bool)>()
        && tag == atoms::replicate()
    {
        HubMsg::Replicate {
            seq,
            bundle: bundle.as_slice().to_vec(),
            expires_unix_ms: expires,
            foreign,
            may_cross,
        }
    } else if let Ok((tag, seq, tokens)) = term.decode::<(Atom, u64, Vec<Binary>)>()
        && tag == atoms::replicate_purge()
    {
        HubMsg::ReplicatePurge {
            seq,
            tokens: tokens.iter().map(fixed).collect::<NifResult<_>>()?,
        }
    } else {
        return Err(rustler::Error::BadArg);
    };
    match msg.encode() {
        Ok(bytes) => Ok((atoms::ok(), bin(env, &bytes)).encode(env)),
        Err(_) => Ok((atoms::error(), atoms::malformed()).encode(env)),
    }
}

fn fixed<const N: usize>(b: &Binary) -> NifResult<[u8; N]> {
    <[u8; N]>::try_from(b.as_slice()).map_err(|_| rustler::Error::BadArg)
}

fn kind_atom(k: BundleKind) -> Atom {
    match k {
        BundleKind::Message => atoms::message(),
        BundleKind::Ack => atoms::ack(),
        BundleKind::Intro => atoms::intro(),
        BundleKind::Control => atoms::control(),
        BundleKind::KeyUpdate => atoms::key_update(),
    }
}

/// Validates an upload at `now_ms`: `{:ok, %{...}}` with the stored fields, or
/// `{:error, reason}` with a `put_err` reason atom.
#[rustler::nif(schedule = "DirtyCpu")]
fn check_bundle<'a>(env: Env<'a>, data: Binary<'a>, now_ms: u64) -> Term<'a> {
    let err = |r: PutError| (atoms::error(), reason_atom(r)).encode(env);
    let bundle = match Bundle::decode(data.as_slice()) {
        Ok(b) => b,
        Err(ProtoError::Invalid(InvalidBundle::Expired)) => return err(PutError::Expired),
        Err(ProtoError::Invalid(InvalidBundle::UnsupportedVersion(_))) => {
            return err(PutError::Unsupported);
        }
        Err(ProtoError::InputTooLarge { .. }) => return err(PutError::TooLarge),
        Err(_) => return err(PutError::Malformed),
    };
    let kind = bundle.kind();
    if kind == BundleKind::Intro {
        // Same rule as couriers: the creation week estimated from age, plus the adjacent week
        // near a boundary.
        let Some(pow) = bundle.pow() else {
            return err(PutError::BadProofOfWork);
        };
        let (weeks, n) = pow_weeks(now_ms.saturating_sub(bundle.age_ms()));
        let ok = pow.difficulty() >= POW_DIFFICULTY
            && weeks.into_iter().take(n).any(|w| {
                pow_valid(
                    bundle.bundle_id().as_bytes(),
                    bundle.dst_tag().as_bytes(),
                    w,
                    &pow.nonce(),
                    pow.difficulty(),
                )
            });
        if !ok {
            return err(PutError::BadProofOfWork);
        }
    }
    let remaining_ms =
        (u64::from(bundle.lifetime_s()) * 1000).saturating_sub(bundle.age_ms());
    let map = rustler::types::map::map_new(env)
        .map_put(atoms_key(env, "tag"), bin(env, bundle.dst_tag().as_bytes()))
        .and_then(|m| m.map_put(atoms_key(env, "commitment"), bin(env, bundle.purge_commitment())))
        .and_then(|m| m.map_put(atoms_key(env, "bundle_id"), bin(env, bundle.bundle_id().as_bytes())))
        .and_then(|m| m.map_put(atoms_key(env, "kind"), kind_atom(kind).encode(env)))
        .and_then(|m| m.map_put(atoms_key(env, "expires_at_ms"), (now_ms + remaining_ms).encode(env)))
        .and_then(|m| m.map_put(atoms_key(env, "size"), data.len().encode(env)));
    match map {
        Ok(m) => (atoms::ok(), m).encode(env),
        Err(_) => err(PutError::Malformed),
    }
}

fn atoms_key<'a>(env: Env<'a>, name: &str) -> Term<'a> {
    Atom::from_str(env, name).map_or_else(|_| name.encode(env), |a| a.encode(env))
}

/// The purge commitment of a 16-byte token.
#[rustler::nif]
fn commit<'a>(env: Env<'a>, token: Binary<'a>) -> Term<'a> {
    match <[u8; 16]>::try_from(token.as_slice()) {
        Ok(t) => (
            atoms::ok(),
            bin(env, purge_commit(&PurgeToken::from_bytes(t)).as_bytes()),
        )
            .encode(env),
        Err(_) => (atoms::error(), atoms::malformed()).encode(env),
    }
}

/// The public hub key of a 32-byte seed.
#[rustler::nif]
fn hub_public<'a>(env: Env<'a>, seed: Binary<'a>) -> NifResult<Term<'a>> {
    let id = Identity::from_seed(&fixed(&seed)?);
    Ok(bin(env, id.public().as_bytes()))
}

/// This hub's handshake signature over [`HUB_PEER_DOMAIN`] || peer's nonce || own nonce.
#[rustler::nif]
fn peer_sign<'a>(
    env: Env<'a>,
    seed: Binary<'a>,
    peer_nonce: Binary<'a>,
    own_nonce: Binary<'a>,
) -> NifResult<Term<'a>> {
    let id = Identity::from_seed(&fixed(&seed)?);
    let n1: [u8; 16] = fixed(&peer_nonce)?;
    let n2: [u8; 16] = fixed(&own_nonce)?;
    Ok(bin(env, &id.sign(HUB_PEER_DOMAIN, &[&n1, &n2])))
}

/// Checks a peer's handshake signature: it signed our nonce, then its own.
#[rustler::nif]
fn peer_verify(
    key: Binary,
    our_nonce: Binary,
    their_nonce: Binary,
    sig: Binary,
) -> bool {
    let (Ok(k), Ok(n1), Ok(n2), Ok(s)) = (
        fixed::<32>(&key),
        fixed::<16>(&our_nonce),
        fixed::<16>(&their_nonce),
        fixed::<64>(&sig),
    ) else {
        return false;
    };
    IdentityPublic::from_bytes(&k)
        .is_ok_and(|k| k.verify(HUB_PEER_DOMAIN, &[&n1, &n2], &s).is_ok())
}

/// Verifies a hub list against operator keys at `now_ms`:
/// `{:ok, serial, [{url, key}], [{url, key}]}` (own hubs, then partner hubs) or
/// `{:error, :invalid}`.
#[rustler::nif]
fn verify_hub_list<'a>(
    env: Env<'a>,
    bytes: Binary<'a>,
    operator_keys: Vec<Binary<'a>>,
    now_ms: u64,
) -> Term<'a> {
    let invalid = (atoms::error(), Atom::from_str(env, "invalid").ok()).encode(env);
    let Ok(list) = HubList::decode(bytes.as_slice()) else {
        return invalid;
    };
    let Ok(signed) = list.signed_bytes() else {
        return invalid;
    };
    let ok = list.expires_unix_ms > now_ms
        && operator_keys.iter().any(|k| {
            fixed::<32>(k)
                .ok()
                .and_then(|k| IdentityPublic::from_bytes(&k).ok())
                .is_some_and(|k| k.verify(HUB_LIST_DOMAIN, &[&signed], &list.signature).is_ok())
        });
    if !ok {
        return invalid;
    }
    let hubs: Vec<Term<'a>> = list
        .hubs
        .iter()
        .map(|h| (h.url.as_str(), bin(env, &h.key)).encode(env))
        .collect();
    let partners: Vec<Term<'a>> = list
        .partners
        .iter()
        .map(|p| (p.url.as_str(), bin(env, &p.key)).encode(env))
        .collect();
    (atoms::ok(), list.serial, hubs, partners).encode(env)
}

rustler::init!("Elixir.MemHub.Native");
