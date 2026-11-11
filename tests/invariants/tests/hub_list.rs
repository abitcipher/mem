//! The operator-signed hub list: only a list signed by a pinned operator key and
//! not expired is used; a newer serial replaces an older one; the user's own address wins; each
//! device starts at its own place in the list and tries the rest in order.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::{Input, Output};
use mem_crypto::IdentityKey;
use mem_invariants::{Device, NOW};
use mem_proto::hublist::{HUB_LIST_DOMAIN, HubEntry, HubList};

fn operator() -> IdentityKey {
    IdentityKey::from_seed(&[7; 32])
}

fn list(by: &IdentityKey, serial: u64, expires: u64, urls: &[&str]) -> Vec<u8> {
    let mut l = HubList {
        serial,
        issued_unix_ms: NOW,
        expires_unix_ms: expires,
        hubs: urls
            .iter()
            .enumerate()
            .map(|(i, u)| HubEntry {
                url: (*u).into(),
                key: *IdentityKey::from_seed(&[u8::try_from(i).unwrap() + 1; 32])
                    .public()
                    .as_bytes(),
                region: None,
            })
            .collect(),
        signature: [0; 64],
        partners: Vec::new(),
    };
    l.signature = by.sign(HUB_LIST_DOMAIN, &[&l.signed_bytes().unwrap()]);
    l.encode().unwrap()
}

fn device(seed: u8) -> Device {
    let mut d = Device::new(seed);
    d.handle(Input::SetOperatorKeys(vec![
        *operator().public().as_bytes(),
    ]));
    d
}

const DAY: u64 = 86_400_000;
const THREE: [&str; 3] = [
    "wss://a.example/v1",
    "wss://b.example/v1",
    "wss://c.example/v1",
];

#[test]
fn only_a_signed_unexpired_list_is_used() {
    let mut d = device(150);
    // Wrong signer.
    let stranger = IdentityKey::from_seed(&[9; 32]);
    d.handle(Input::HubListReceived {
        bytes: list(&stranger, 1, NOW + DAY, &THREE),
    });
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
    // Expired.
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW - 1, &THREE),
    });
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
    // A hub address the app would not use (plain ws:// to another machine).
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW + DAY, &["ws://evil.example/v1"]),
    });
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
    // Malformed bytes.
    d.handle(Input::HubListReceived {
        bytes: vec![0xa6, 0, 1],
    });
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
    // Signed and valid.
    let out = d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW + DAY, &THREE),
    });
    assert!(out.contains(&Output::HubCandidatesChanged));
    let mut got = d.agent.hub_candidates();
    got.sort();
    assert_eq!(got, THREE);
}

#[test]
fn without_operator_keys_no_list_is_trusted() {
    let mut d = Device::new(151);
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW + DAY, &THREE),
    });
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
}

#[test]
fn a_newer_serial_replaces_an_older_one_never_the_reverse() {
    let mut d = device(152);
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 5, NOW + DAY, &THREE),
    });
    let out = d.handle(Input::HubListReceived {
        bytes: list(&operator(), 4, NOW + DAY, &THREE[..1]),
    });
    assert!(!out.contains(&Output::HubCandidatesChanged));
    assert_eq!(d.agent.hub_candidates().len(), 3);
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 6, NOW + DAY, &THREE[..2]),
    });
    assert_eq!(d.agent.hub_candidates().len(), 2);
    assert_eq!(d.agent.hub_list().unwrap().serial, 6);
}

#[test]
fn the_users_own_address_wins_and_clearing_it_returns_to_the_list() {
    let mut d = device(153);
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW + DAY, &THREE),
    });
    let out = d.handle(Input::SetHubUrl(Some("wss://mine.example/v1".into())));
    assert!(out.contains(&Output::HubCandidatesChanged));
    assert_eq!(d.agent.hub_candidates(), ["wss://mine.example/v1"]);
    d.handle(Input::SetHubUrl(None));
    assert_eq!(d.agent.hub_candidates().len(), 3);
}

/// Devices spread over the hubs (each starts at its own random place) and each tries every hub,
/// in a fixed order, so failover reaches all of them.
#[test]
fn devices_start_at_different_hubs_and_keep_their_place() {
    let firsts: std::collections::BTreeSet<String> = (160..176)
        .map(|seed| {
            let mut d = device(seed);
            d.handle(Input::HubListReceived {
                bytes: list(&operator(), 1, NOW + DAY, &THREE),
            });
            let c = d.agent.hub_candidates();
            let mut all = c.clone();
            all.sort();
            assert_eq!(all, THREE, "every hub is a candidate");
            // Restarting keeps the same order, and the list, once the keys are set again.
            d.restart().unwrap();
            d.handle(Input::SetOperatorKeys(vec![
                *operator().public().as_bytes(),
            ]));
            assert_eq!(d.agent.hub_candidates(), c);
            c[0].clone()
        })
        .collect();
    assert!(firsts.len() > 1, "devices do not all start at one hub");
}

/// A kept list is dropped when the app no longer trusts the key that signed it.
#[test]
fn a_list_from_a_retired_operator_key_is_dropped() {
    let mut d = device(154);
    d.handle(Input::HubListReceived {
        bytes: list(&operator(), 1, NOW + DAY, &THREE),
    });
    let next = IdentityKey::from_seed(&[8; 32]);
    d.handle(Input::SetOperatorKeys(vec![*next.public().as_bytes()]));
    assert_eq!(d.agent.hub_candidates(), Vec::<String>::new());
}
