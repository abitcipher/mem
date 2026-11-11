//! The email bridge: P4, so every send asks unless the user made a standing rule for
//! that contact; provenance says email, P4; the app password is never printed.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{
    ChannelOffer, ConsentRecord, ConsentScope, ContactId, DeliveryStatus, MsgId, RejectReason,
};
use mem_core::{ClaId, EmailAccount, Input, Output, PrivacyGrade};
use mem_invariants::{Device, pair};

fn account() -> EmailAccount {
    EmailAccount {
        address: "ana@example.org".into(),
        imap_host: "imap.example.org".into(),
        imap_port: 993,
        smtp_host: "smtp.example.org".into(),
        smtp_port: 465,
        username: "ana@example.org".into(),
        password: zeroize::Zeroizing::new("app-password-123".into()),
    }
}

fn email_on(dev: &mut Device, contact: ContactId) {
    dev.handle(Input::SetChannelEnabled {
        cla: ClaId::Email,
        enabled: true,
    });
    assert!(
        !dev.handle(Input::SetEmailAccount(Some(account())))
            .iter()
            .any(|o| matches!(o, Output::Rejected(_)))
    );
    dev.handle(Input::SetContactEmail {
        contact,
        address: Some("bo@example.net".into()),
    });
}

fn sends(out: &[Output]) -> Vec<(mem_core::model::OpId, MsgId, String, Vec<u8>)> {
    out.iter()
        .filter_map(|o| match o {
            Output::EmailSend {
                op,
                message,
                to,
                bytes,
            } => Some((*op, *message, to.clone(), bytes.clone())),
            _ => None,
        })
        .collect()
}

fn offer_of(out: &[Output]) -> ChannelOffer {
    out.iter()
        .find_map(|o| match o {
            Output::ConsentRequired(offer) => Some(offer.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn email_is_off_without_an_account() {
    let mut a = Device::new(131);
    assert!(!a.agent.channel_enabled(ClaId::Email));
    a.handle(Input::SetChannelEnabled {
        cla: ClaId::Email,
        enabled: true,
    });
    assert_eq!(a.handle(Input::EmailConnected), Vec::new());
    let bad = EmailAccount {
        address: "not an address".into(),
        ..account()
    };
    assert!(
        a.handle(Input::SetEmailAccount(Some(bad)))
            .contains(&Output::Rejected(RejectReason::InvalidEmailAccount))
    );
}

/// Nothing goes by email automatically below the floor; a `Once` consent sends exactly
/// that message to the typed address, and the result is recorded as email, P4.
#[test]
fn every_email_send_asks_unless_a_rule_allows_it() {
    let (mut a, mut b) = (Device::new(132), Device::new(133));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    email_on(&mut a, b_id);
    a.handle(Input::EmailConnected);
    let msg = a.send(b_id, "by mail").unwrap();
    assert!(
        sends(&a.tick()).is_empty(),
        "never automatic without a rule"
    );

    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Email,
    }));
    assert_eq!(offer.grade, PrivacyGrade::P4);
    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Email,
        grade: offer.grade,
        scope: ConsentScope::Once,
    }));
    let s = sends(&out);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].2, "bo@example.net");
    a.handle(Input::TransmitResult {
        op: s[0].0,
        result: Ok(()),
    });
    let sent = &a.agent.messages(b_id)[0];
    assert_eq!(sent.status, DeliveryStatus::Sent);
    assert_eq!(sent.channels[0].cla, ClaId::Email);
    assert_eq!(sent.channels[0].grade, PrivacyGrade::P4);
    assert_eq!(a.agent.privacy_log()[0].cla, ClaId::Email);

    // The recipient imports the mail's bundle: provenance email, P4.
    let out = b.handle(Input::Received {
        cla: ClaId::Email,
        bytes: s[0].3.clone(),
    });
    assert!(out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.text, "by mail");
    assert_eq!(got.channels[0].cla, ClaId::Email);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P4);
}

/// A standing rule for the contact lets the outbox go by email by itself, each message once.
#[test]
fn a_standing_rule_sends_automatically_and_once() {
    let (mut a, mut b) = (Device::new(134), Device::new(135));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    email_on(&mut a, b_id);
    a.handle(Input::EmailConnected);
    let first = a.send(b_id, "one").unwrap();
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: first,
        cla: ClaId::Email,
    }));
    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: first,
        cla: ClaId::Email,
        grade: offer.grade,
        scope: ConsentScope::ThisContact {
            until_unix_ms: None,
        },
    }));
    let s = sends(&out);
    a.handle(Input::TransmitResult {
        op: s[0].0,
        result: Ok(()),
    });
    // The next message goes on its own.
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "two".into(),
    });
    let s = sends(&out);
    assert_eq!(s.len(), 1, "sent under the rule");
    a.handle(Input::TransmitResult {
        op: s[0].0,
        result: Ok(()),
    });
    // And nothing is sent twice.
    a.advance(60_000);
    assert_eq!(sends(&a.tick()).len(), 0);
    a.handle(Input::EmailDisconnected);
    assert_eq!(sends(&a.handle(Input::EmailConnected)).len(), 0);
}

#[test]
fn no_address_means_unreachable_and_the_password_is_never_printed() {
    let (mut a, mut b) = (Device::new(136), Device::new(137));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    email_on(&mut a, b_id);
    a.handle(Input::SetContactEmail {
        contact: b_id,
        address: None,
    });
    a.handle(Input::EmailConnected);
    let msg = a.send(b_id, "x").unwrap();
    let opt = a
        .agent
        .channel_options(msg)
        .into_iter()
        .find(|o| o.cla == ClaId::Email)
        .unwrap();
    assert!(!opt.reachable);
    assert!(
        a.handle(Input::SetContactEmail {
            contact: b_id,
            address: Some("nope".into()),
        })
        .contains(&Output::Rejected(RejectReason::InvalidEmailAddress))
    );
    // Neither the account nor the input that carries it prints the password.
    let shown = format!(
        "{:?} {:?}",
        account(),
        Input::SetEmailAccount(Some(account()))
    );
    assert!(!shown.contains("app-password-123"), "{shown}");
    a.restart().unwrap();
    assert_eq!(
        a.agent.email_account().map(|x| x.address),
        Some("ana@example.org".into())
    );
}
