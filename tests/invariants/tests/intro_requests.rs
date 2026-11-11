//! Contact requests without meeting: quarantine until approval, proof of work,
//! the per-encounter verification cap, blocking, and the opt-in intro tag.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{MessageKind, RejectReason};
use mem_core::{ClaId, EncounterMsg, Input, Output};
use mem_crypto::intro::{intro_tag, pow_valid, week_for_unix_ms};
use mem_invariants::{Device, LINK_PEER, encounter, pump, transmitted};
use mem_proto::{Bundle, BundleParams, Pow};

/// B allows requests through nearby devices.
fn accepting(seed: u8) -> Device {
    let mut d = Device::new(seed);
    d.handle(Input::SetAcceptRequestsNearby { enabled: true });
    d
}

/// The newest intro bundle A queued for `contact`, as bytes, by showing it as a QR code.
fn intro_bytes(a: &mut Device, contact: mem_core::model::ContactId) -> Vec<u8> {
    let msg = a
        .agent
        .messages(contact)
        .iter()
        .rev()
        .find(|m| m.kind == MessageKind::ContactRequest)
        .unwrap()
        .id;
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    transmitted(&out).unwrap().1
}

fn requests_arrived(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::ContactRequest { .. }))
        .count()
}

/// The same bundle with another proof of work.
fn with_pow(bytes: &[u8], pow: Pow) -> Vec<u8> {
    let b = Bundle::decode(bytes).unwrap();
    Bundle::new(BundleParams {
        bundle_id: b.bundle_id(),
        kind: b.kind(),
        dst_tag: b.dst_tag(),
        lifetime_s: b.lifetime_s(),
        age_ms: b.age_ms(),
        hops_remaining: b.hops_remaining(),
        copies: b.copies(),
        pow: Some(pow),
        payload: b.payload().to_vec(),
        purge_commitment: *b.purge_commitment(),
    })
    .unwrap()
    .encode()
}

/// A nonce that does *not* satisfy the difficulty for this bundle.
fn bad_pow(bytes: &[u8], now: u64) -> Pow {
    let b = Bundle::decode(bytes).unwrap();
    let week = week_for_unix_ms(now);
    (0u64..10_000)
        .map(u64::to_be_bytes)
        .find(|n| !pow_valid(b.bundle_id().as_bytes(), b.dst_tag().as_bytes(), week, n, 4))
        .map(|n| Pow::new(n, 4))
        .unwrap()
}

#[test]
fn a_request_stays_in_quarantine_until_approved() {
    let (mut a, mut b) = (Device::new(1), accepting(2));
    let b_id = a.request(&b, Some("hi, it's Ann")).unwrap();
    assert!(a.agent.contact(b_id).unwrap().requested);
    let view = &a.agent.messages(b_id)[0];
    assert_eq!(
        (view.kind, view.text.as_str()),
        (MessageKind::ContactRequest, "hi, it's Ann")
    );
    // Nothing else can be sent before they answer.
    assert!(
        a.handle(Input::UserSend {
            contact: b_id,
            text: "too early".into()
        })
        .contains(&Output::Rejected(RejectReason::AwaitingApproval))
    );

    // They meet on Wi-Fi: B advertises its intro tag, A delivers the request.
    let (_, out_b) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert_eq!(requests_arrived(&out_b), 1);
    // Not a contact, no message, only a quarantined request with an unverified name.
    assert_eq!(b.agent.contacts().len(), 0);
    let req = &b.agent.requests()[0];
    assert_eq!(req.note.as_deref(), Some("hi, it's Ann"));
    assert_eq!(
        req.via.cla,
        ClaId::Courier,
        "no proof of who handed it over"
    );

    // B approves; its ack (on the normal tags, after the ack delay) confirms A's side.
    let ob = b.handle(Input::ApproveRequest(req.id));
    let a_id = b.agent.contacts()[0].id;
    assert_eq!(b.agent.requests().len(), 0);
    b.advance(3_000);
    let mut ob2 = ob;
    ob2.extend(b.tick());
    pump(&mut a, &mut b, Vec::new(), ob2);
    assert!(!a.agent.contact(b_id).unwrap().requested);

    // Now they are ordinary contacts.
    let oa = a.handle(Input::UserSend {
        contact: b_id,
        text: "hello".into(),
    });
    pump(&mut a, &mut b, oa, Vec::new());
    assert_eq!(b.agent.messages(a_id).last().unwrap().text, "hello");
}

#[test]
fn the_intro_tag_is_advertised_only_with_the_setting_on() {
    let mut b = Device::new(2);
    let card = mem_proto::card::PairingCard::decode(&b.card()).unwrap();
    let tag = intro_tag(
        &mem_crypto::IdentityPublic::from_bytes(&card.fields.identity_pk).unwrap(),
        week_for_unix_ms(b.now),
    );
    let tags = |d: &mut Device| -> Vec<[u8; 16]> {
        let out = d.handle(Input::EncounterStarted {
            peer: LINK_PEER,
            cla: ClaId::Iroh,
        });
        d.handle(Input::EncounterEnded { peer: LINK_PEER });
        out.into_iter()
            .find_map(|o| match o {
                Output::Encounter {
                    msg: EncounterMsg::Tags(t),
                    ..
                } => Some(t),
                _ => None,
            })
            .unwrap()
    };
    assert!(!tags(&mut b).contains(&tag));
    b.handle(Input::SetAcceptRequestsNearby { enabled: true });
    assert!(tags(&mut b).contains(&tag));
}

#[test]
fn switching_requests_on_asks_the_hub_at_once() {
    // Regression: a request waiting at the hub arrived only after a reconnect.
    let mut b = Device::new(2);
    let card = mem_proto::card::PairingCard::decode(&b.card()).unwrap();
    let tag = intro_tag(
        &mem_crypto::IdentityPublic::from_bytes(&card.fields.identity_pk).unwrap(),
        week_for_unix_ms(b.now),
    );
    b.handle(Input::SetChannelEnabled {
        cla: ClaId::Hub,
        enabled: true,
    });
    // Connected, and done with whatever the first plan fetched.
    let mut pending = b.handle(Input::HubConnected);
    for _ in 0..8 {
        if !pending.iter().any(|o| matches!(o, Output::HubFetch { .. })) {
            break;
        }
        pending = b.handle(Input::HubFetchDone);
    }
    let out = b.handle(Input::SetAcceptRequestsNearby { enabled: true });
    let asked = out.iter().any(|o| match o {
        Output::HubFetch { prefixes } => prefixes.iter().any(|p| p[..] == tag[..2]),
        _ => false,
    });
    assert!(asked, "{out:?}");
}

#[test]
fn a_request_pasted_or_scanned_arrives_without_the_setting() {
    // The setting only controls advertising; a request handed over in person or pasted
    // still reaches the requests screen.
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let b_id = a.request(&b, None).unwrap();
    let bytes = intro_bytes(&mut a, b_id);
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(requests_arrived(&out), 1);
    assert_eq!(b.agent.requests()[0].via.cla, ClaId::Qr);
}

#[test]
fn bad_or_weak_proofs_of_work_are_refused() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let b_id = a.request(&b, None).unwrap();
    let good = intro_bytes(&mut a, b_id);
    let invalid = with_pow(&good, bad_pow(&good, b.now));
    let weak = {
        let p = Bundle::decode(&good).unwrap().pow().copied().unwrap();
        with_pow(&good, Pow::new(p.nonce(), 3))
    };
    for bytes in [invalid, weak] {
        let out = b.handle(Input::Received {
            cla: ClaId::Qr,
            bytes,
        });
        assert_eq!(out, [Output::Rejected(RejectReason::BadProofOfWork)]);
    }
    assert_eq!(b.agent.requests().len(), 0);
    // The genuine one still works (nothing was tombstoned by the failures).
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: good,
    });
    assert_eq!(requests_arrived(&out), 1);
}

#[test]
fn a_courier_stops_checking_proofs_after_the_first_invalid_one() {
    let (mut a, b, mut c) = (Device::new(1), Device::new(2), Device::new(3));
    let b_id = a.request(&b, None).unwrap();
    // A spray copy, as couriers receive them (a direct send has no hops left).
    let good = Bundle::decode(&intro_bytes(&mut a, b_id))
        .unwrap()
        .with_relay(4, 2)
        .unwrap()
        .encode();
    let invalid = with_pow(&good, bad_pow(&good, c.now));
    let hand_over = |c: &mut Device, bundles: Vec<Vec<u8>>| {
        c.handle(Input::EncounterStarted {
            peer: LINK_PEER,
            cla: ClaId::Iroh,
        });
        c.handle(Input::EncounterMessage {
            peer: LINK_PEER,
            msg: EncounterMsg::Hello {
                version: 1,
                ephemeral_id: [9; 16],
            },
        });
        for bytes in bundles {
            c.handle(Input::EncounterMessage {
                peer: LINK_PEER,
                msg: EncounterMsg::Bundles(vec![mem_proto::encounter::BundleEntry::bare(bytes)]),
            });
        }
        c.handle(Input::EncounterEnded { peer: LINK_PEER });
    };
    // Invalid first: the valid one in the same encounter is not even checked.
    hand_over(&mut c, vec![invalid, good.clone()]);
    assert_eq!(c.agent.courier_status().2, 0);
    // A fresh encounter gets a fresh budget.
    hand_over(&mut c, vec![good]);
    assert_eq!(c.agent.courier_status().2, 1);
}

#[test]
fn a_blocked_sender_is_ignored() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let b_id = a.request(&b, None).unwrap();
    let bytes = intro_bytes(&mut a, b_id);
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    let req = b.agent.requests()[0].id;
    b.handle(Input::BlockRequest(req));
    assert_eq!(b.agent.requests().len(), 0);

    // Three days later A asks again: silently dropped.
    a.advance(73 * 60 * 60 * 1000);
    b.advance(73 * 60 * 60 * 1000);
    a.tick();
    a.request(&b, Some("please")).unwrap();
    let bytes = intro_bytes(&mut a, b_id);
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(requests_arrived(&out), 0);
    assert_eq!(b.agent.requests().len(), 0);
}

#[test]
fn requests_from_the_previous_card_work_but_not_older() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let old_card = b.card();
    b.handle(Input::NewPairingCode);
    let b_id = a.request_card(old_card, None).unwrap();
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: intro_bytes(&mut a, b_id),
    });
    assert_eq!(requests_arrived(&out), 1, "one code back is still accepted");

    let (mut a2, mut b2) = (Device::new(5), Device::new(6));
    let stale_card = b2.card();
    b2.handle(Input::NewPairingCode);
    b2.handle(Input::NewPairingCode);
    let id = a2.request_card(stale_card, None).unwrap();
    let out = b2.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: intro_bytes(&mut a2, id),
    });
    assert_eq!(out, [Output::Rejected(RejectReason::NotForUs)]);
}

#[test]
fn requests_survive_a_restart_and_can_be_approved_after() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let b_id = a.request(&b, Some("later")).unwrap();
    let bytes = intro_bytes(&mut a, b_id);
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    b.restart().unwrap();
    a.restart().unwrap();
    assert!(a.agent.contact(b_id).unwrap().requested);
    let first = b.agent.requests()[0].id;
    assert_eq!(b.agent.requests()[0].note.as_deref(), Some("later"));
    // A request arriving after the restart gets its own id (ids restart, requests persist).
    let mut c = Device::new(3);
    let b_in_c = c.request(&b, None).unwrap();
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: intro_bytes(&mut c, b_in_c),
    });
    let ids: Vec<_> = b.agent.requests().iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    b.handle(Input::ApproveRequest(first));
    assert_eq!(b.agent.contacts().len(), 1);
}

#[test]
fn a_request_travels_through_a_courier() {
    let (mut a, mut b, mut c) = (Device::new(1), accepting(2), Device::new(3));
    a.request(&b, Some("via a friend")).unwrap();
    encounter(&mut a, &mut c, ClaId::Iroh);
    for d in [&mut a, &mut c] {
        d.handle(Input::EncounterEnded { peer: LINK_PEER });
    }
    assert_eq!(c.agent.courier_status().2, 1, "C carries the request");
    let (_, out_b) = encounter(&mut c, &mut b, ClaId::Iroh);
    assert_eq!(requests_arrived(&out_b), 1);
    assert_eq!(b.agent.contacts().len(), 0);
}
