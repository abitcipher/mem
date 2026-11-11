//! Deny-by-default contacts and untrusted input: bundles from anyone who is not an
//! approved contact never become messages.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::format_collect,
    reason = "test code"
)]

use mem_core::model::RejectReason;
use mem_core::{ClaId, Input, Output};
use mem_invariants::{Device, pair, transmitted};

#[test]
fn stranger_bundle_is_not_delivered() {
    let (mut a, mut b, mut eve) = (Device::new(1), Device::new(2), Device::new(3));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // Eve has B's card (e.g. it was shared publicly) but B never approved Eve.
    let b_in_eve = eve.import(&b, ClaId::Share).unwrap();
    let msg = eve.send(b_in_eve, "click this link").unwrap();
    let out = eve.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (_, bytes) = transmitted(&out).unwrap();

    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert_eq!(out, vec![Output::Rejected(RejectReason::NotForUs)]);
    assert!(
        b.agent
            .contacts()
            .iter()
            .all(|c| c.fingerprint != eve.agent.fingerprint())
    );
}

#[test]
fn message_for_someone_else_is_not_delivered() {
    let (mut a, mut b, mut c) = (Device::new(1), Device::new(2), Device::new(3));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    pair(&mut a, &mut c, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "for B only").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    assert_eq!(
        c.handle(Input::Received {
            cla: ClaId::Qr,
            bytes
        }),
        vec![Output::Rejected(RejectReason::NotForUs)]
    );
}

#[test]
fn tampered_bundle_is_rejected() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "x").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    // Flip one bit in the ciphertext (end of the bundle).
    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert_eq!(
        b.handle(Input::Received {
            cla: ClaId::Qr,
            bytes: bad
        }),
        vec![Output::Rejected(RejectReason::Undecryptable)]
    );
    // Garbage never panics.
    for junk in [vec![], vec![0xff; 10], vec![0xa0], bytes[..20].to_vec()] {
        let out = b.handle(Input::Received {
            cla: ClaId::Share,
            bytes: junk,
        });
        assert!(matches!(out[..], [Output::Rejected(_)]));
    }
}

#[test]
fn forged_card_is_rejected() {
    let (mut a, b) = (Device::new(1), Device::new(2));
    let mut card = b.card();
    let i = card.len() - 1;
    card[i] ^= 1; // corrupt the signature
    assert_eq!(
        a.handle(Input::ImportCard {
            cla: ClaId::Qr,
            bytes: card
        }),
        vec![Output::Rejected(RejectReason::BadSignature)]
    );
    let own = a.card();
    assert_eq!(
        a.handle(Input::ImportCard {
            cla: ClaId::Qr,
            bytes: own
        }),
        vec![Output::Rejected(RejectReason::OwnCard)]
    );
}

/// A contact code another app hands over is only previewed (checked, not
/// imported) until the user confirms; the preview says whether it is already a contact.
#[test]
fn a_card_preview_never_adds_a_contact() {
    let (mut a, b) = (
        mem_invariants::Device::new(71),
        mem_invariants::Device::new(72),
    );
    let preview = a.agent.card_preview(&b.card()).unwrap();
    assert!(!preview.known);
    assert_eq!(preview.fingerprint, b.agent.fingerprint());
    assert!(a.agent.contacts().is_empty(), "nothing imported");
    a.import(&b, mem_core::ClaId::Share).unwrap();
    assert!(a.agent.card_preview(&b.card()).unwrap().known);
    assert!(a.agent.card_preview(&a.card()).is_err(), "own card");
    assert!(a.agent.card_preview(&[0xa0]).is_err(), "malformed");
}

/// A removed contact (no shared group) is gone: secret, conversation, queued messages; what it
/// sends afterwards is not delivered.
#[test]
fn a_removed_contact_is_gone() {
    let (mut a, mut b) = (Device::new(73), Device::new(74));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.send(b_id, "queued").unwrap();
    a.handle(Input::RemoveContact(b_id));
    assert_eq!(a.agent.contacts().len(), 0);
    assert_eq!(a.agent.messages(b_id).len(), 0);
    let msg = b.send(a_id, "after removal").unwrap();
    let (_, bytes) = transmitted(&b.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    let out = a.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert!(!out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    assert_eq!(a.agent.contacts().len(), 0, "not re-added");
}
