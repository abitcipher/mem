//! Local-network encounters (Iroh part, sans-IO): nearby nodes exchange messages through an encounter, the
//! outbox flushes automatically only within the consent gate, and provenance records the
//! network channel and its grade.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "test code"
)]

use mem_core::model::{DeliveryStatus, RejectReason};
use mem_core::{ClaId, EncounterMsg, Input, MIN_ADVERTISED_TAGS, Output, PrivacyGrade};
use mem_invariants::{Device, LINK_PEER, encounter, pair, pump};

fn delivered(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::Deliver { .. }))
        .count()
}

#[test]
fn queued_message_flushes_when_contact_comes_into_range() {
    let (mut a, mut b) = (Device::new(31), Device::new(32));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // Queued while apart: no channel was chosen, nothing transmitted.
    let msg = a.send(b_id, "see you on the Wi-Fi").unwrap();
    assert_eq!(a.agent.messages(b_id)[0].status, DeliveryStatus::Queued);

    let (out_a, out_b) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert!(out_a.iter().any(|o| matches!(
        o,
        Output::Transmit { message, cla: ClaId::Iroh, peer: Some(_), .. } if *message == msg
    )));
    assert_eq!(delivered(&out_b), 1);
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.text, "see you on the Wi-Fi");
    assert_eq!(got.channels[0].cla, ClaId::Iroh);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P1);
    let sent = &a.agent.messages(b_id)[0];
    assert_eq!(sent.status, DeliveryStatus::Sent);
    assert_eq!(sent.channels[0].grade, PrivacyGrade::P1);
    assert_eq!(a.agent.reachable_contacts(), vec![(b_id, ClaId::Iroh)]);

    // While connected, a new message goes out immediately, in both directions.
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "and another".into(),
    });
    let (_, out_b) = pump(&mut a, &mut b, out, Vec::new());
    assert_eq!(delivered(&out_b), 1);
    let out = b.handle(Input::UserSend {
        contact: a_id,
        text: "reply".into(),
    });
    let (out_a, _) = pump(&mut a, &mut b, Vec::new(), out);
    assert_eq!(delivered(&out_a), 1);

    // B's reply carried acks for both of A's messages, so a second encounter resends nothing
    // from A (only unconfirmed messages are resent).
    a.handle(Input::EncounterEnded { peer: LINK_PEER });
    b.handle(Input::EncounterEnded { peer: LINK_PEER });
    let (out_a, _) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert!(!out_a.iter().any(|o| matches!(o, Output::Transmit { .. })));
}

/// A contact whose floor is P0 never gets an automatic P1 send; the user is asked.
#[test]
fn auto_flush_never_goes_below_the_floor() {
    let (mut a, mut b) = (Device::new(33), Device::new(34));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P0,
    });
    let msg = a.send(b_id, "only in person").unwrap();
    let (out_a, out_b) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert!(!out_a.iter().any(|o| matches!(o, Output::Transmit { .. })));
    assert!(
        !out_a
            .iter()
            .any(|o| matches!(o, Output::ConsentRequired(_)))
    );
    assert_eq!(delivered(&out_b), 0);

    // The option is offered as reachable but needing consent.
    let opt = a
        .agent
        .channel_options(msg)
        .into_iter()
        .find(|o| o.cla == ClaId::Iroh)
        .unwrap();
    assert!(opt.reachable && opt.needs_consent);
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Iroh,
    });
    assert!(matches!(out[..], [Output::ConsentRequired(_)]));
}

#[test]
fn network_channel_without_a_peer_is_unreachable() {
    let (mut a, mut b) = (Device::new(35), Device::new(36));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "hello").unwrap();
    let opt = a
        .agent
        .channel_options(msg)
        .into_iter()
        .find(|o| o.cla == ClaId::Iroh)
        .unwrap();
    assert!(!opt.reachable);
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Iroh,
    });
    assert_eq!(out, vec![Output::Rejected(RejectReason::Unreachable)]);
}

/// One epoch of tags, padded with decoys to a power of two, sorted.
#[test]
fn advertised_tags_are_padded_and_sorted() {
    let (mut a, mut b) = (Device::new(37), Device::new(38));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let out = a.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla: ClaId::Iroh,
    });
    let tags = out
        .iter()
        .find_map(|o| match o {
            Output::Encounter {
                msg: EncounterMsg::Tags(t),
                ..
            } => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(tags.len(), MIN_ADVERTISED_TAGS);
    assert!(tags.windows(2).all(|w| w[0] < w[1]));
    assert!(matches!(
        out[0],
        Output::Encounter {
            msg: EncounterMsg::Hello { version: 1, .. },
            ..
        }
    ));
}

#[test]
fn unpaired_peer_gets_nothing_and_protocol_violations_end_the_encounter() {
    let (mut a, mut b) = (Device::new(39), Device::new(40));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.send(b_id, "for b only").unwrap();
    // A stranger on the same network.
    let mut c = Device::new(41);
    let (out_a, out_c) = encounter(&mut a, &mut c, ClaId::Iroh);
    assert!(!out_a.iter().any(|o| matches!(o, Output::Transmit { .. })));
    assert_eq!(delivered(&out_c), 0);

    // Bundles before hello: cut off.
    let mut d = Device::new(42);
    d.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla: ClaId::Iroh,
    });
    let out = d.handle(Input::EncounterMessage {
        peer: LINK_PEER,
        msg: EncounterMsg::Bundles(vec![mem_proto::encounter::BundleEntry::bare(vec![0])]),
    });
    assert!(out.contains(&Output::EndEncounter { peer: LINK_PEER }));
}

#[test]
fn per_encounter_bundle_quota() {
    let mut a = Device::new(43);
    a.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla: ClaId::Iroh,
    });
    a.handle(Input::EncounterMessage {
        peer: LINK_PEER,
        msg: EncounterMsg::Hello {
            version: 1,
            ephemeral_id: [0; 16],
        },
    });
    let mut cut = false;
    for _ in 0..=(mem_core::PEER_BUNDLE_QUOTA / 16) {
        let out = a.handle(Input::EncounterMessage {
            peer: LINK_PEER,
            msg: EncounterMsg::Bundles(vec![
                mem_proto::encounter::BundleEntry::bare(vec![0xa0]);
                16
            ]),
        });
        if out.contains(&Output::Rejected(RejectReason::Quota)) {
            assert!(out.contains(&Output::EndEncounter { peer: LINK_PEER }));
            cut = true;
            break;
        }
    }
    assert!(cut);
}

#[test]
fn disabled_channel_is_never_used() {
    let (mut a, mut b) = (Device::new(44), Device::new(45));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.handle(Input::SetChannelEnabled {
        cla: ClaId::Iroh,
        enabled: false,
    });
    let msg = a.send(b_id, "x").unwrap();
    let out = a.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla: ClaId::Iroh,
    });
    assert_eq!(out, vec![Output::EndEncounter { peer: LINK_PEER }]);
    assert!(
        a.agent
            .channel_options(msg)
            .iter()
            .all(|o| o.cla != ClaId::Iroh)
    );
    // The setting survives a restart.
    let snap = a.agent.snapshot();
    let restored = mem_core::Agent::restore(&snap, a.now).unwrap();
    assert!(!restored.channel_enabled(ClaId::Iroh));
    assert!(restored.channel_enabled(ClaId::Qr));
}

/// Tags rotate at midnight: peers learn the new tags and the sender's window still matches a
/// receiver whose clock is a little behind.
#[test]
fn epoch_rollover_readvertises() {
    let (mut a, mut b) = (Device::new(46), Device::new(47));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);
    a.now += 86_400_000;
    let out = a.handle(Input::Tick {
        wall_unix_ms: a.now,
    });
    // (The harness ticks before every input, so the rollover shows up here.)
    assert!(out.iter().any(|o| matches!(
        o,
        Output::Encounter {
            msg: EncounterMsg::Tags(_),
            ..
        }
    )));
    // B's clock still says yesterday; A's message still gets there.
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "after midnight".into(),
    });
    let (_, out_b) = pump(&mut a, &mut b, out, Vec::new());
    assert_eq!(delivered(&out_b), 1);
    assert_eq!(b.agent.messages(a_id)[0].text, "after midnight");
}
