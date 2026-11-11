//! End-to-end delivery acks and retransmission. A message is `Delivered` only on a
//! verified ack; until then it is retried over automatic channels, within the consent gate,
//! and duplicates are refused but acknowledged again.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "test code"
)]

use mem_core::model::{DeliveryStatus, MsgId, PeerHandle};
use mem_core::{
    ACK_DELAY_MS, ClaId, EncounterMsg, Input, MAX_SENDS_PER_ENCOUNTER, MIN_MESSAGE_LIFETIME_S,
    Output, PrivacyGrade, RETRANSMIT_AFTER_MS,
};
use mem_invariants::{Device, LINK_PEER, encounter, pair, pump};

fn delivered(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::Deliver { .. }))
        .count()
}

/// Transmissions of `message` in `out`.
fn sends_of(out: &[Output], message: MsgId) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::Transmit { message: m, .. } if *m == message))
        .count()
}

/// Lets `dev` believe its transmissions succeeded, but nothing reaches the other side: a
/// write into a dead connection.
fn lose(dev: &mut Device, out: &[Output]) {
    for o in out {
        if let Output::Transmit { op, .. } = o {
            dev.handle(Input::TransmitResult {
                op: *op,
                result: Ok(()),
            });
        }
    }
}

fn status(dev: &Device, contact: mem_core::model::ContactId, i: usize) -> DeliveryStatus {
    dev.agent.messages(contact)[i].status
}

/// Regression: a message written into a dead connection is reported as
/// handed over, never arrives, and must not be lost. A new encounter resends it; it is delivered
/// once and becomes `Delivered` only when the ack arrives.
#[test]
fn message_lost_in_a_dead_connection_is_resent_and_confirmed() {
    let (mut a, mut b) = (Device::new(71), Device::new(72));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "new look".into(),
    });
    let msg = a.agent.messages(b_id)[0].id;
    assert_eq!(sends_of(&out, msg), 1);
    lose(&mut a, &out);
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Sent);
    assert_eq!(b.agent.messages(a_id), vec![]);

    // B restarted: A still holds the stale encounter (handle 1) when B's new one (handle 2)
    // arrives. A prefers the newer encounter and resends at once.
    let fresh = PeerHandle(2);
    b.handle(Input::EncounterEnded { peer: LINK_PEER });
    let ob = b.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla: ClaId::Iroh,
    });
    let mut oa = a.handle(Input::EncounterStarted {
        peer: fresh,
        cla: ClaId::Iroh,
    });
    // A's hello and tags reach B on its new connection.
    for o in &oa {
        if let Output::Encounter { msg, .. } = o {
            b.handle(Input::EncounterMessage {
                peer: LINK_PEER,
                msg: msg.clone(),
            });
        }
    }
    for o in &ob {
        if let Output::Encounter { msg, .. } = o {
            oa.extend(a.handle(Input::EncounterMessage {
                peer: fresh,
                msg: msg.clone(),
            }));
        }
    }
    let resent: Vec<Vec<u8>> = oa
        .iter()
        .filter_map(|o| match o {
            Output::Transmit {
                message,
                peer: Some(p),
                bytes,
                ..
            } if *message == msg && *p == fresh => Some(bytes.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(resent.len(), 1, "{oa:?}");

    // B gets it once, and owes an ack.
    let out_b = b.handle(Input::EncounterMessage {
        peer: LINK_PEER,
        msg: EncounterMsg::Bundles(
            resent
                .into_iter()
                .map(mem_proto::encounter::BundleEntry::bare)
                .collect(),
        ),
    });
    assert_eq!(delivered(&out_b), 1);
    assert_eq!(b.agent.messages(a_id)[0].text, "new look");

    // No message to carry it: after ACK_DELAY_MS B sends a standalone ack, and A confirms.
    b.advance(ACK_DELAY_MS);
    let acks = b.tick();
    let ack_bytes: Vec<Vec<u8>> = acks
        .iter()
        .filter_map(|o| match o {
            Output::Transmit {
                cla: ClaId::Iroh,
                bytes,
                ..
            } => Some(bytes.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ack_bytes.len(), 1, "{acks:?}");
    let out_a = a.handle(Input::EncounterMessage {
        peer: fresh,
        msg: EncounterMsg::Bundles(
            ack_bytes
                .into_iter()
                .map(mem_proto::encounter::BundleEntry::bare)
                .collect(),
        ),
    });
    assert!(out_a.iter().any(|o| matches!(
        o,
        Output::StatusChanged { message, status: DeliveryStatus::Delivered } if *message == msg
    )));
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Delivered);
    // Confirmed: never sent again, on this or any later encounter.
    a.advance(RETRANSMIT_AFTER_MS);
    assert_eq!(sends_of(&a.tick(), msg), 0);
    assert_eq!(a.agent.channel_options(msg), vec![]);
}

/// A lost ack is healed: the retransmitted copy is refused before decryption but
/// acknowledged again, and the recipient still shows the message once.
#[test]
fn lost_ack_is_healed_by_retransmission() {
    let (mut a, mut b) = (Device::new(73), Device::new(74));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "hello".into(),
    });
    let msg = a.agent.messages(b_id)[0].id;
    // Delivered to B ...
    let (_, out_b) = pump(&mut a, &mut b, out, Vec::new());
    assert_eq!(delivered(&out_b), 1);
    // ... but B's standalone ack is lost.
    b.advance(ACK_DELAY_MS);
    let acks = b.tick();
    assert_eq!(
        acks.iter()
            .filter(|o| matches!(o, Output::Transmit { .. }))
            .count(),
        1
    );
    lose(&mut b, &acks);
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Sent);

    // A retries after RETRANSMIT_AFTER_MS; B refuses the copy but acks it again.
    a.advance(RETRANSMIT_AFTER_MS);
    b.advance(RETRANSMIT_AFTER_MS);
    let retry = a.tick();
    assert_eq!(sends_of(&retry, msg), 1);
    let (_, out_b) = pump(&mut a, &mut b, retry, Vec::new());
    assert_eq!(delivered(&out_b), 0);
    assert_eq!(b.agent.messages(a_id).len(), 1);
    b.advance(ACK_DELAY_MS);
    let acks = b.tick();
    pump(&mut a, &mut b, Vec::new(), acks);
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Delivered);
}

/// Within one encounter a message is sent at most `MAX_SENDS_PER_ENCOUNTER` times, then waits
/// for the next encounter, so a recipient that cannot store it is not flooded.
#[test]
fn retransmission_is_capped_per_encounter() {
    let (mut a, mut b) = (Device::new(75), Device::new(76));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "into the void".into(),
    });
    let msg = a.agent.messages(b_id)[0].id;
    let mut total = sends_of(&out, msg);
    lose(&mut a, &out);
    for _ in 0..5 {
        a.advance(RETRANSMIT_AFTER_MS);
        let out = a.tick();
        total += sends_of(&out, msg);
        lose(&mut a, &out);
    }
    assert_eq!(total, usize::from(MAX_SENDS_PER_ENCOUNTER));
    // The next encounter sends it again.
    a.handle(Input::EncounterEnded { peer: LINK_PEER });
    b.handle(Input::EncounterEnded { peer: LINK_PEER });
    let (out_a, _) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert_eq!(sends_of(&out_a, msg), 1);
}

/// Acks and retransmissions follow the floor. With a P0 floor, Wi-Fi (P1) carries neither;
/// the ack waits and rides on the next message the user sends by QR (piggyback).
#[test]
fn acks_and_retries_never_go_below_the_floor() {
    let (mut a, mut b) = (Device::new(77), Device::new(78));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    for (dev, contact) in [(&mut a, b_id), (&mut b, a_id)] {
        dev.handle(Input::SetFloor {
            contact,
            floor: PrivacyGrade::P0,
        });
    }
    // A sends by QR (user action, P0 is within the floor).
    let msg = a.send(b_id, "by hand").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (op, bytes) = mem_invariants::transmitted(&out).unwrap();
    a.handle(Input::TransmitResult { op, result: Ok(()) });
    assert_eq!(
        delivered(&b.handle(Input::Received {
            cla: ClaId::Qr,
            bytes
        })),
        1
    );

    // On Wi-Fi together: no retransmission of A's message and no standalone ack (P1 < P0).
    let (out_a, out_b) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert_eq!(sends_of(&out_a, msg), 0);
    for dev in [&mut a, &mut b] {
        dev.advance(RETRANSMIT_AFTER_MS);
    }
    let (ta, tb) = (a.tick(), b.tick());
    for out in [&out_a, &out_b, &ta, &tb] {
        assert!(
            !out.iter().any(|o| matches!(
                o,
                Output::Transmit {
                    cla: ClaId::Iroh,
                    ..
                }
            )),
            "{out:?}"
        );
        assert!(!out.iter().any(|o| matches!(o, Output::ConsentRequired(_))));
    }
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Sent);

    // B's reply by QR carries the ack.
    let reply = b.send(a_id, "got it").unwrap();
    let out = b.handle(Input::RequestTransmit {
        message: reply,
        cla: ClaId::Qr,
    });
    let (_, bytes) = mem_invariants::transmitted(&out).unwrap();
    a.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Delivered);
}

/// The retry period is the message lifetime: a user setting clamped to 1–30 days. An
/// unconfirmed message that outlives it is reported as failed and leaves the outbox.
#[test]
fn unconfirmed_message_expires_after_its_lifetime() {
    let (mut a, mut b) = (Device::new(79), Device::new(80));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // Below the minimum: clamped up to one day.
    a.handle(Input::SetMessageLifetime { seconds: 60 });
    let msg = a.send(b_id, "short-lived").unwrap();
    a.advance(u64::from(MIN_MESSAGE_LIFETIME_S) * 1000 - 1);
    a.tick();
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Queued);
    a.advance(1);
    let out = a.tick();
    assert!(out.iter().any(|o| matches!(
        o,
        Output::StatusChanged { message, status: DeliveryStatus::Failed } if *message == msg
    )));
    assert_eq!(a.agent.channel_options(msg), vec![]);
}

/// The delivered index and owed acks survive a restart: a copy arriving after the recipient
/// restarted is still acknowledged.
#[test]
fn re_ack_survives_a_restart() {
    let (mut a, mut b) = (Device::new(81), Device::new(82));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "before restart".into(),
    });
    pump(&mut a, &mut b, out, Vec::new());
    // B's ack goes into a dying connection; then B restarts.
    b.advance(ACK_DELAY_MS);
    let acks = b.tick();
    lose(&mut b, &acks);
    let snap = b.agent.snapshot();
    b.agent = mem_core::Agent::restore(&snap, b.now).unwrap();
    a.handle(Input::EncounterEnded { peer: LINK_PEER });
    encounter(&mut a, &mut b, ClaId::Iroh);
    // A's resend reached B (in the encounter above); B owes the ack again and sends it.
    for dev in [&mut a, &mut b] {
        dev.advance(ACK_DELAY_MS);
    }
    let acks = b.tick();
    pump(&mut a, &mut b, Vec::new(), acks);
    assert_eq!(status(&a, b_id, 0), DeliveryStatus::Delivered);
}
