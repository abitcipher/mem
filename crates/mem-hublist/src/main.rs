//! `mem-hublist`: the operator's tool for the hub network.
//!
//! ```text
//! mem-hublist keygen <seed-file>        new Ed25519 key (operator or hub); prints the public key
//! mem-hublist pubkey <seed-file>        prints the public key of a seed file
//! mem-hublist sign <operator-seed> <serial> <days> <out.cbor> <url>=<hub-key-hex>[@region]...
//!                   [partner:<url>=<hub-key-hex>@<operator name>]...
//! mem-hublist show <list.cbor> [<operator-key-hex>]
//! ```
//!
//! `partner:` entries name other operators' hubs this network exchanges bundles with.
//!
//! Seed files hold 32 raw bytes, are created with mode 0600 and never overwritten. Keep the
//! operator seed offline; a hub needs only its own seed.

#![forbid(unsafe_code)]
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool"
)]

use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use mem_crypto::{IdentityKey, IdentityPublic};
use mem_proto::hublist::{HUB_LIST_DOMAIN, HubEntry, HubList, PartnerEntry};
use zeroize::Zeroizing;

const DAY_MS: u64 = 86_400_000;
/// A list may be valid for at most a year: a stolen list should not stay usable for long.
const MAX_DAYS: u64 = 365;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("keygen") if args.len() == 2 => keygen(&args[1]),
        Some("pubkey") if args.len() == 2 => {
            read_seed(&args[1]).map(|k| hex(k.public().as_bytes()))
        }
        Some("sign") if args.len() >= 6 => sign(&args[1], &args[2], &args[3], &args[4], &args[5..]),
        Some("show") if (2..=3).contains(&args.len()) => show(&args[1], args.get(2)),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage:
  mem-hublist keygen <seed-file>
  mem-hublist pubkey <seed-file>
  mem-hublist sign <operator-seed> <serial> <days> <out.cbor> <url>=<hub-key-hex>[@region]...
                   [partner:<url>=<hub-key-hex>@<operator name>]...
  mem-hublist show <list.cbor> [<operator-key-hex>]";

fn keygen(path: &str) -> Result<String, String> {
    let mut seed = Zeroizing::new([0u8; 32]);
    getrandom::fill(seed.as_mut()).map_err(|e| format!("no randomness: {e}"))?;
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts
        .open(path)
        .map_err(|e| format!("{path}: {e} (an existing key is never overwritten)"))?;
    f.write_all(seed.as_ref())
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(hex(IdentityKey::from_seed(&seed).public().as_bytes()))
}

fn read_seed(path: &str) -> Result<IdentityKey, String> {
    let bytes = Zeroizing::new(std::fs::read(path).map_err(|e| format!("{path}: {e}"))?);
    let seed: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("{path}: not a 32-byte seed"))?;
    let key = IdentityKey::from_seed(&seed);
    drop(Zeroizing::new(seed));
    Ok(key)
}

fn sign(
    operator: &str,
    serial: &str,
    days: &str,
    out: &str,
    hubs: &[String],
) -> Result<String, String> {
    let key = read_seed(operator)?;
    let serial: u64 = serial.parse().map_err(|_| "serial: not a number")?;
    let days: u64 = days
        .parse()
        .ok()
        .filter(|d| (1..=MAX_DAYS).contains(d))
        .ok_or(format!("days: 1..={MAX_DAYS}"))?;
    let (partners, hubs): (Vec<&String>, Vec<&String>) =
        hubs.iter().partition(|h| h.starts_with("partner:"));
    let hubs = hubs
        .iter()
        .map(|h| entry(h))
        .collect::<Result<Vec<_>, _>>()?;
    let partners = partners
        .iter()
        .map(|p| partner(p.trim_start_matches("partner:")))
        .collect::<Result<Vec<_>, _>>()?;
    if hubs.iter().any(|h| partners.iter().any(|p| p.key == h.key)) {
        return Err("a key is listed both as own hub and as partner".into());
    }
    let now = now_ms();
    let mut list = HubList {
        serial,
        issued_unix_ms: now,
        expires_unix_ms: now + days * DAY_MS,
        hubs,
        signature: [0; 64],
        partners,
    };
    let signed = list.signed_bytes().map_err(|e| format!("list: {e:?}"))?;
    list.signature = key.sign(HUB_LIST_DOMAIN, &[&signed]);
    let bytes = list.encode().map_err(|e| format!("list: {e:?}"))?;
    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    Ok(format!(
        "{out}: serial {serial}, {} hubs, {} partners, {} bytes",
        list.hubs.len(),
        list.partners.len(),
        bytes.len()
    ))
}

/// `<url>=<hub-key-hex>@<operator name>` (the part after `partner:`).
fn partner(s: &str) -> Result<PartnerEntry, String> {
    let (rest, operator) = s
        .rsplit_once('@')
        .ok_or(format!("partner:{s}: expected <url>=<key>@<operator name>"))?;
    let hub = entry(rest)?;
    Ok(PartnerEntry {
        url: hub.url,
        key: hub.key,
        operator: operator.to_owned(),
    })
}

/// `<url>=<hub-key-hex>[@region]`.
fn entry(s: &str) -> Result<HubEntry, String> {
    let (url, rest) = s
        .split_once('=')
        .ok_or(format!("{s}: expected <url>=<key>"))?;
    let (key, region) = match rest.split_once('@') {
        Some((k, r)) => (k, Some(r.to_owned())),
        None => (rest, None),
    };
    // As the app (`valid_url`): wss://, or ws:// only to this machine for testing.
    let loopback = ["ws://127.0.0.1:", "ws://localhost:", "ws://[::1]:"]
        .iter()
        .any(|p| url.starts_with(p));
    if !url.starts_with("wss://") && !loopback {
        return Err(format!("{url}: hubs in a list must use wss://"));
    }
    let key: [u8; 32] = unhex(key)
        .and_then(|b| b.try_into().ok())
        .ok_or(format!("{key}: expected 64 hex digits"))?;
    IdentityPublic::from_bytes(&key).map_err(|_| format!("{url}: not a valid public key"))?;
    Ok(HubEntry {
        url: url.to_owned(),
        key,
        region,
    })
}

fn show(path: &str, operator: Option<&String>) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let list = HubList::decode(&bytes).map_err(|e| format!("{path}: {e:?}"))?;
    let mut out = format!(
        "serial {}\nissued {} ms\nexpires {} ms ({})\n",
        list.serial,
        list.issued_unix_ms,
        list.expires_unix_ms,
        if list.expires_unix_ms > now_ms() {
            "valid"
        } else {
            "EXPIRED"
        }
    );
    for h in &list.hubs {
        let _ = writeln!(
            out,
            "  {} {} {}",
            h.url,
            hex(&h.key),
            h.region.as_deref().unwrap_or("-")
        );
    }
    for p in &list.partners {
        let _ = writeln!(out, "  partner {} {} {}", p.url, hex(&p.key), p.operator);
    }
    if let Some(op) = operator {
        let key: [u8; 32] = unhex(op)
            .and_then(|b| b.try_into().ok())
            .ok_or("operator key: expected 64 hex digits")?;
        let signed = list.signed_bytes().map_err(|e| format!("{e:?}"))?;
        let ok = IdentityPublic::from_bytes(&key).is_ok_and(|k| {
            k.verify(HUB_LIST_DOMAIN, &[&signed], &list.signature)
                .is_ok()
        });
        out += if ok {
            "signature: valid"
        } else {
            "signature: INVALID"
        };
    }
    Ok(out.trim_end().to_owned())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}
