//! Two in-process agents exchange messages through a loopback channel.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::format_collect,
    reason = "test code"
)]

use mem_core::model::{DeliveryStatus, MsgDirection, TrustLevel};
use mem_core::{ClaId, Input, Output, PrivacyGrade};
use mem_invariants::{Device, pair, transmitted};

#[test]
fn pair_by_qr_and_exchange_messages_both_ways() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();

    // Both sides see the same short authentication string.
    let sas_a = a.agent.contact(b_id).unwrap().sas;
    let sas_b = b.agent.contact(a_id).unwrap().sas;
    assert_eq!(sas_a, sas_b);
    assert_eq!(
        a.agent.contact(b_id).unwrap().trust,
        TrustLevel::MetInPerson
    );

    // A -> B over QR (P0, within the default P2 floor: no consent needed).
    let msg = a.send(b_id, "Привіт, B!").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (op, bytes) = transmitted(&out).unwrap();
    a.handle(Input::TransmitResult { op, result: Ok(()) });

    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: bytes.clone(),
    });
    assert!(
        out.iter().any(|o| matches!(o, Output::Deliver { .. })),
        "{out:?}"
    );
    let received = b.agent.messages(a_id);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].text, "Привіт, B!");
    assert_eq!(received[0].direction, MsgDirection::Incoming);
    assert_eq!(received[0].channels[0].grade, PrivacyGrade::P0);

    // Sender view records the channel too.
    let sent = &a.agent.messages(b_id)[0];
    assert_eq!(sent.status, DeliveryStatus::Sent);
    assert_eq!(sent.channels[0].cla, ClaId::Qr);

    // Replaying the same bundle is refused by its purge tombstone, but acknowledged again:
    // only the owed ack is saved, nothing is delivered.
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::Rejected(mem_core::model::RejectReason::Purged)))
            && !out.iter().any(|o| matches!(o, Output::Deliver { .. })),
        "{out:?}"
    );

    // B -> A.
    let reply = b.send(a_id, "Hi A").unwrap();
    let out = b.handle(Input::RequestTransmit {
        message: reply,
        cla: ClaId::Qr,
    });
    let (_, bytes) = transmitted(&out).unwrap();
    a.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(a.agent.messages(b_id).len(), 2);
    // B's reply carried the ack for A's message over QR (piggybacked).
    assert_eq!(a.agent.messages(b_id)[0].status, DeliveryStatus::Delivered);
    // A delivered message leaves the outbox: it can no longer be offered for sending.
    assert_eq!(a.agent.channel_options(msg), vec![]);
}

#[test]
fn messages_survive_snapshot_restore() {
    let (mut a, mut b) = (Device::new(3), Device::new(4));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "persisted").unwrap();

    // Restart A from its snapshot and continue the flow.
    let snap = a.agent.snapshot();
    a.agent = mem_core::Agent::restore(&snap, a.now).unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (_, bytes) = transmitted(&out).unwrap();

    // Restart B too: its contact secret must survive.
    let snap = b.agent.snapshot();
    b.agent = mem_core::Agent::restore(&snap, b.now).unwrap();
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(b.agent.messages(a_id)[0].text, "persisted");
    assert_eq!(
        a.agent.fingerprint(),
        mem_core::Agent::restore(&a.agent.snapshot(), 0)
            .unwrap()
            .fingerprint()
    );
}

#[test]
fn tags_rotate_daily_and_tolerate_one_day_skew() {
    let (mut a, mut b) = (Device::new(5), Device::new(6));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "late").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (_, bytes) = transmitted(&out).unwrap();

    // Receiver clock one day ahead: still accepted (epoch - 1).
    b.now += 86_400_000;
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: bytes.clone(),
    });
    assert!(
        out.iter().any(|o| matches!(o, Output::Deliver { .. })),
        "{out:?}"
    );
    let _ = a_id;
}

/// A bundle keeps the tag of the day it was sealed: it is accepted while it may still be in an
/// outbox or a courier (up to the 30-day lifetime), not just for ±1 day.
#[test]
fn bundles_sealed_days_ago_are_still_accepted() {
    let (mut a, mut b) = (Device::new(31), Device::new(32));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "late but fine").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    b.advance(20 * 24 * 60 * 60 * 1000);
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(b.agent.messages(a_id)[0].text, "late but fine");
}
