//! E2E by default and no silent downgrade for the P5 plaintext fallback: the text leaves
//! the core unencrypted only after a `Once` consent for that message, never automatically, never
//! under a standing rule, and the message records `Plain · P5` honestly.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{ChannelOffer, ConsentRecord, ConsentScope, DeliveryStatus, RejectReason};
use mem_core::{ClaId, Input, Output, PrivacyGrade};
use mem_invariants::{Device, encounter, pair};

fn offer_of(out: &[Output]) -> ChannelOffer {
    out.iter()
        .find_map(|o| match o {
            Output::ConsentRequired(offer) => Some(offer.clone()),
            _ => None,
        })
        .unwrap()
}

fn plaintext(out: &[Output]) -> Option<String> {
    out.iter().find_map(|o| match o {
        Output::PlaintextReady { text, .. } => Some(text.clone()),
        _ => None,
    })
}

#[test]
fn plain_text_needs_a_once_consent_and_is_logged() {
    let (mut a, mut b) = (Device::new(121), Device::new(122));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "meet at 6, gate B").unwrap();

    // Asking for P5 only produces an offer, graded P5.
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Plain,
    });
    assert_eq!(plaintext(&out), None);
    let offer = offer_of(&out);
    assert_eq!(offer.grade, PrivacyGrade::P5);

    // Any wider scope is refused: no standing rule may cover P5.
    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Plain,
        grade: offer.grade,
        scope: ConsentScope::ThisContact {
            until_unix_ms: None,
        },
    }));
    assert_eq!(plaintext(&out), None);
    assert_eq!(a.agent.rules(), Vec::new());

    // Once: the exact text comes out, once.
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Plain,
    }));
    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Plain,
        grade: offer.grade,
        scope: ConsentScope::Once,
    }));
    assert_eq!(plaintext(&out).as_deref(), Some("meet at 6, gate B"));
    let op = out
        .iter()
        .find_map(|o| match o {
            Output::PlaintextReady { op, .. } => Some(*op),
            _ => None,
        })
        .unwrap();
    a.handle(Input::TransmitResult { op, result: Ok(()) });
    let sent = &a.agent.messages(b_id)[0];
    assert_eq!(sent.status, DeliveryStatus::Sent);
    assert_eq!(sent.channels[0].cla, ClaId::Plain);
    assert_eq!(sent.channels[0].grade, PrivacyGrade::P5);
    let log = a.agent.privacy_log();
    assert_eq!(log[0].cla, ClaId::Plain);
    assert_eq!(log[0].grade, PrivacyGrade::P5);
}

/// Nothing automatic ever produces plain text: not the outbox flush in an encounter, not a tick.
#[test]
fn plain_text_is_never_automatic() {
    let (mut a, mut b) = (Device::new(123), Device::new(124));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.send(b_id, "secret").unwrap();
    let (out_a, _) = encounter(&mut a, &mut b, ClaId::Iroh);
    assert_eq!(plaintext(&out_a), None);
    a.advance(60_000);
    assert_eq!(plaintext(&a.tick()), None);
}

/// A group message's copy (hidden, with group content) never goes as plain text, even with a
/// `Once` consent.
#[test]
fn group_copies_never_go_as_plain_text() {
    let (mut a, mut b) = (Device::new(125), Device::new(126));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.handle(Input::CreateGroup {
        name: "g".into(),
        members: vec![b_id],
    });
    let group = a.agent.groups()[0].id;
    encounter(&mut a, &mut b, ClaId::Iroh);
    b.handle(Input::EncounterEnded {
        peer: mem_invariants::LINK_PEER,
    });
    a.handle(Input::EncounterEnded {
        peer: mem_invariants::LINK_PEER,
    });
    b.handle(Input::AcceptGroupInvitation(group));
    encounter(&mut a, &mut b, ClaId::Iroh);
    a.handle(Input::EncounterEnded {
        peer: mem_invariants::LINK_PEER,
    });
    a.handle(Input::SendGroupText {
        group,
        text: "group only".into(),
    });
    let copy = a.agent.group_messages(group)[0].id;
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: copy,
        cla: ClaId::Plain,
    }));
    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: copy,
        cla: ClaId::Plain,
        grade: offer.grade,
        scope: ConsentScope::Once,
    }));
    assert_eq!(plaintext(&out), None);
    assert!(out.contains(&Output::Rejected(RejectReason::NotPlainText)));
}
