//! Courier exchange: store-carry-forward between devices that never meet directly,
//! Binary Spray-and-Wait, purge-token gossip, consent for one's own messages, and honest
//! provenance for relayed bundles.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "test code: devices are named a, b, c…"
)]

use mem_core::model::{ContactId, DeliveryStatus, PeerHandle};
use mem_core::{ClaId, Input, Output, PrivacyGrade};
use mem_invariants::{Device, LINK_PEER, encounter, pair};

/// Meets `x` and `y` over the local network and lets the encounter run out, then parts them.
fn meet(x: &mut Device, y: &mut Device) -> (Vec<Output>, Vec<Output>) {
    let out = encounter(x, y, ClaId::Iroh);
    x.handle(Input::EncounterEnded { peer: LINK_PEER });
    y.handle(Input::EncounterEnded { peer: LINK_PEER });
    out
}

/// A and B paired in person, then apart; C is a stranger to both.
fn apart() -> (Device, Device, Device, ContactId, ContactId) {
    let (mut a, mut b, c) = (Device::new(1), Device::new(2), Device::new(3));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    (a, b, c, b_id, a_id)
}

fn texts(dev: &Device, contact: ContactId) -> Vec<String> {
    dev.agent
        .messages(contact)
        .into_iter()
        .map(|m| m.text)
        .collect()
}

#[test]
fn a_reaches_b_through_courier_c_without_ever_meeting() {
    let (mut a, mut b, mut c, b_id, a_id) = apart();
    let msg = a.send(b_id, "via C").unwrap();

    meet(&mut a, &mut c);
    assert_eq!(c.agent.courier_status().2, 1, "C carries A's bundle");
    let m = &a.agent.messages(b_id)[0];
    assert_eq!(m.status, DeliveryStatus::Sent);
    assert_eq!(m.channels[0].cla, ClaId::Courier);

    meet(&mut c, &mut b);
    assert_eq!(texts(&b, a_id), ["via C"]);
    // Honest provenance: relayed, so labelled couriers, P3.
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.channels[0].cla, ClaId::Courier);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P3);

    // B's ack travels back the same way and confirms the message.
    b.advance(3_000);
    b.tick();
    meet(&mut b, &mut c);
    meet(&mut c, &mut a);
    assert_eq!(
        a.agent
            .messages(b_id)
            .into_iter()
            .find(|m| m.id == msg)
            .unwrap()
            .status,
        DeliveryStatus::Delivered
    );
}

#[test]
fn spray_halves_copies_until_the_hop_limit_ends_it() {
    let (mut a, _b, mut c, b_id, _) = apart();
    a.send(b_id, "spray").unwrap();
    meet(&mut a, &mut c);
    let carried = c.agent.courier_entries();
    assert_eq!(carried[0].copies, 16, "A gave half of its 32");
    assert_eq!(carried[0].hops, 4);

    // Each courier hands on half and keeps half; hops fall by one per hand-off.
    let (mut d, mut e, mut f, mut g) = (
        Device::new(4),
        Device::new(5),
        Device::new(6),
        Device::new(7),
    );
    meet(&mut c, &mut d);
    assert_eq!(c.agent.courier_entries()[0].copies, 8);
    assert_eq!(d.agent.courier_entries()[0].copies, 8);
    assert_eq!(d.agent.courier_entries()[0].hops, 3);
    meet(&mut d, &mut e);
    assert_eq!(e.agent.courier_entries()[0].copies, 4);
    meet(&mut e, &mut f);
    assert_eq!(f.agent.courier_entries()[0].hops, 1);
    // One hop left: F only delivers, never hands on.
    meet(&mut f, &mut g);
    assert_eq!(g.agent.courier_status().2, 0);
}

#[test]
fn a_last_copy_only_delivers() {
    let (mut a, _b, mut c, b_id, _) = apart();
    a.agent.set_spray_copies_for_simulation(2);
    a.send(b_id, "last copy").unwrap();
    meet(&mut a, &mut c);
    assert_eq!(c.agent.courier_entries()[0].copies, 1);
    let mut d = Device::new(4);
    meet(&mut c, &mut d);
    assert_eq!(
        d.agent.courier_status().2,
        0,
        "a single copy is never sprayed"
    );
}

#[test]
fn hop_limit_is_never_exceeded() {
    // A chain of strangers: hops 4 at the first courier, so at most 4 couriers hold it.
    let (mut a, _b, _c, b_id, _) = apart();
    a.send(b_id, "far").unwrap();
    let mut chain: Vec<Device> = (10..20).map(Device::new).collect();
    meet(&mut a, &mut chain[0]);
    for i in 0..chain.len() - 1 {
        let (left, right) = chain.split_at_mut(i + 1);
        meet(&mut left[i], &mut right[0]);
    }
    for d in &chain {
        for m in d.agent.courier_entries() {
            assert!(m.hops >= 1 && m.hops <= 4, "hops {}", m.hops);
        }
    }
    let holders = chain
        .iter()
        .filter(|d| d.agent.courier_status().2 > 0)
        .count();
    assert!(holders <= 4, "{holders} holders");
}

#[test]
fn purge_tokens_spread_and_delete_carried_copies() {
    let (mut a, mut b, mut c, b_id, _) = apart();
    a.send(b_id, "once").unwrap();
    let mut d = Device::new(4);
    meet(&mut a, &mut c);
    meet(&mut c, &mut d);
    assert_eq!(d.agent.courier_status().2, 1);
    meet(&mut c, &mut b); // delivered; B tombstones it
    // C learns the token from B at once (B gossips its tombstones).
    assert_eq!(c.agent.courier_status().2, 0);
    // D never met B: C passes the token on, and D's copy goes too (every node gossips).
    meet(&mut c, &mut d);
    assert_eq!(d.agent.courier_status().2, 0);
    // And D now refuses the bundle if anyone offers it again.
    assert_eq!(d.store.tombstones.len(), 1);
}

#[test]
fn own_messages_go_to_couriers_only_where_allowed() {
    let (mut a, _b, mut c, b_id, _) = apart();
    // A contact at P2 (e.g. from before the P3 default): couriers are a downgrade.
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P2,
    });
    let msg = a.send(b_id, "strict").unwrap();
    meet(&mut a, &mut c);
    assert_eq!(c.agent.courier_status().2, 0, "never without consent");

    // The user picks couriers: the consent dialog, then it goes with the next courier.
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Courier,
    });
    let offer = out
        .iter()
        .find_map(|o| match o {
            Output::ConsentRequired(offer) => Some(offer.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(offer.grade, PrivacyGrade::P3);
    let out = a.handle(Input::ConsentGiven(mem_core::model::ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Courier,
        grade: PrivacyGrade::P3,
        scope: mem_core::model::ConsentScope::Once,
    }));
    assert!(out.contains(&Output::CourierQueued { message: msg }));
    assert!(!out.iter().any(|o| matches!(o, Output::Transmit { .. })));
    meet(&mut a, &mut c);
    assert_eq!(c.agent.courier_status().2, 1);
    assert_eq!(a.agent.privacy_log().len(), 1, "the downgrade is logged");
}

#[test]
fn direct_delivery_with_proof_is_p1_and_a_forged_proof_is_not() {
    let (mut a, mut b, _c, b_id, a_id) = apart();
    a.send(b_id, "direct").unwrap();
    meet(&mut a, &mut b);
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.channels[0].cla, ClaId::Iroh);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P1);

    // A stranger replaying A's bundle with a made-up proof is labelled couriers.
    let (mut a2, mut b2, _c2, b2_id, a2_id) = apart();
    let msg = a2.send(b2_id, "replayed").unwrap();
    let bytes = mem_invariants::transmitted(&a2.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap()
    .1;
    let peer = PeerHandle(42);
    b2.handle(Input::EncounterStarted {
        peer,
        cla: ClaId::Iroh,
    });
    b2.handle(Input::EncounterMessage {
        peer,
        msg: mem_core::EncounterMsg::Hello {
            version: mem_proto::encounter::ENCOUNTER_VERSION,
            ephemeral_id: [1; 16],
        },
    });
    b2.handle(Input::EncounterMessage {
        peer,
        msg: mem_core::EncounterMsg::Bundles(vec![mem_proto::encounter::BundleEntry {
            bytes,
            proof: Some([0xEE; 16]),
        }]),
    });
    let got = &b2.agent.messages(a2_id)[0];
    assert_eq!(got.channels[0].cla, ClaId::Courier);
}

#[test]
fn a_peer_without_a_summary_gets_no_courier_traffic() {
    // A build without courier support never sends `summary`: it is not a courier and is handed nothing.
    let (mut a, _b, _c, b_id, _) = apart();
    a.send(b_id, "x").unwrap();
    let peer = PeerHandle(9);
    a.handle(Input::EncounterStarted {
        peer,
        cla: ClaId::Iroh,
    });
    let out = a.handle(Input::EncounterMessage {
        peer,
        msg: mem_core::EncounterMsg::Hello {
            version: mem_proto::encounter::ENCOUNTER_VERSION,
            ephemeral_id: [2; 16],
        },
    });
    let mut all = out;
    all.extend(a.handle(Input::EncounterMessage {
        peer,
        msg: mem_core::EncounterMsg::Tags(vec![[7; 16]; 8]),
    }));
    assert!(!all.iter().any(|o| matches!(
        o,
        Output::Encounter {
            msg: mem_core::EncounterMsg::Bundles(_),
            ..
        } | Output::Forward { .. }
    )));
}

/// A courier that carried a bundle for days still hands it over. The recipient
/// no longer advertises that day's tag, so the courier offers it and the recipient claims it.
#[test]
fn a_bundle_carried_for_days_reaches_its_recipient_through_an_offer() {
    let (mut a, mut b, mut c, b_id, a_id) = apart();
    a.send(b_id, "three days later").unwrap();
    meet(&mut a, &mut c);
    assert_eq!(c.agent.courier_status().2, 1);
    for d in [&mut b, &mut c] {
        d.advance(3 * 24 * 60 * 60 * 1000);
        d.tick();
    }
    let (out_c, _) = meet(&mut c, &mut b);
    assert!(out_c.iter().any(|o| matches!(
        o,
        Output::Encounter {
            msg: mem_core::EncounterMsg::Offer(_),
            ..
        }
    )));
    assert_eq!(texts(&b, a_id), ["three days later"]);
}

/// A flooder hands `c` one batch of `n` junk bundles of 16 KiB (random tags, maximum copies) in
/// its own encounter.
fn flood(c: &mut Device, peer: u64, seed: u8, n: u8) {
    use mem_proto::encounter::{BundleEntry, ENCOUNTER_VERSION};
    use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, RendezvousTag};
    let peer = PeerHandle(peer);
    c.handle(Input::EncounterStarted {
        peer,
        cla: ClaId::Iroh,
    });
    c.handle(Input::EncounterMessage {
        peer,
        msg: mem_core::EncounterMsg::Hello {
            version: ENCOUNTER_VERSION,
            ephemeral_id: [seed; 16],
        },
    });
    for i in 0..n {
        let junk = Bundle::new(BundleParams {
            bundle_id: BundleId::from_bytes([
                seed, i, 0xFF, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            ]),
            kind: BundleKind::Message,
            dst_tag: RendezvousTag::from_bytes([seed ^ i; 16]),
            lifetime_s: BundleKind::Message.max_lifetime_s(),
            age_ms: 0,
            hops_remaining: 5,
            copies: 64,
            pow: None,
            payload: vec![0xAB; 16 * 1024],
            purge_commitment: [seed, i, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7],
        })
        .unwrap();
        c.handle(Input::EncounterMessage {
            peer,
            msg: mem_core::EncounterMsg::Bundles(vec![BundleEntry::bare(junk.encode())]),
        });
    }
    c.handle(Input::EncounterEnded { peer });
}

/// Bundles from a flood (an arrival batch above 256 KiB) are never handed on, so
/// junk cannot travel on beside honest bundles and get them evicted downstream. A batch of honest
/// size on the same device is still sprayed.
#[test]
fn flood_bundles_are_never_handed_on() {
    let (mut c, mut d) = (Device::new(30), Device::new(31));
    flood(&mut c, 90, 1, 20); // 320 KiB: a flood
    flood(&mut c, 91, 2, 2); // 32 KiB: honest-sized
    assert_eq!(c.agent.courier_status().2, 22);
    meet(&mut c, &mut d);
    let passed = d.agent.courier_entries();
    assert_eq!(
        passed.len(),
        2,
        "only the honest-sized batch spreads: {passed:?}"
    );
}

/// Everything carried shares one spray allowance per encounter (64 KiB), served
/// smallest arrival batch first, so a spammer that keeps its batches small gets little through
/// and a lone honest hand-off goes first.
#[test]
fn carried_bundles_share_one_spray_allowance_smallest_batch_first() {
    let (mut c, mut d) = (Device::new(32), Device::new(33));
    flood(&mut c, 90, 1, 12); // 192 KiB: under the flood threshold
    flood(&mut c, 91, 2, 4); // 64 KiB
    flood(&mut c, 92, 3, 1); // 16 KiB: the smallest batch
    meet(&mut c, &mut d);
    let passed = d.agent.courier_entries();
    let bytes: u64 = passed.iter().map(|m| u64::from(m.size)).sum();
    assert!(bytes <= 64 * 1024, "{bytes} bytes sprayed in one encounter");
    assert!(
        passed.iter().any(|m| m.key.bundle_id[0] == 3),
        "the smallest batch goes first"
    );
    assert!(
        passed.iter().all(|m| m.key.bundle_id[0] != 1),
        "the largest batch waits"
    );
}
