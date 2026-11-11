//! No silent downgrade: a below-floor send needs a matching ConsentRecord.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::format_collect,
    reason = "test code"
)]

use mem_core::model::{ConsentBasis, ConsentRecord, ConsentScope, OfferId, RejectReason};
use mem_core::{ClaId, Input, Output, PrivacyGrade};
use mem_invariants::{Device, pair, transmitted};

fn offer_of(out: &[Output]) -> mem_core::model::ChannelOffer {
    out.iter()
        .find_map(|o| match o {
            Output::ConsentRequired(offer) => Some(offer.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn below_floor_send_is_blocked_without_consent() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "secret plan").unwrap();

    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Share,
    });
    assert!(
        transmitted(&out).is_none(),
        "share (P4) must not transmit below floor P3"
    );
    let offer = offer_of(&out);
    assert_eq!(offer.grade, PrivacyGrade::P4);
    assert_eq!(offer.floor, PrivacyGrade::P3);
}

#[test]
fn mismatched_or_unknown_consent_is_refused() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "x").unwrap();
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Share,
    }));

    // Fabricated offer id.
    let forged = ConsentRecord {
        offer: OfferId(9_999),
        message: msg,
        cla: ClaId::Share,
        grade: PrivacyGrade::P4,
        scope: ConsentScope::Once,
    };
    let out = a.handle(Input::ConsentGiven(forged));
    assert_eq!(out, vec![Output::Rejected(RejectReason::ConsentMismatch)]);

    // Right offer, wrong grade (user was shown something else).
    let wrong = ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Share,
        grade: PrivacyGrade::P2,
        scope: ConsentScope::Once,
    };
    let out = a.handle(Input::ConsentGiven(wrong));
    assert!(transmitted(&out).is_none());
    // The offer is consumed: retrying with the same id fails as well.
    let right = ConsentRecord {
        grade: PrivacyGrade::P4,
        ..wrong
    };
    assert!(transmitted(&a.handle(Input::ConsentGiven(right))).is_none());
}

#[test]
fn once_consent_transmits_once_and_is_logged() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "via telegram").unwrap();
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Share,
    }));

    let out = a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Share,
        grade: offer.grade,
        scope: ConsentScope::Once,
    }));
    let (_, bytes) = transmitted(&out).unwrap();
    let log = a.agent.privacy_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].basis, ConsentBasis::Once);

    // The receiver records the honest P4 provenance.
    b.handle(Input::Received {
        cla: ClaId::Share,
        bytes,
    });
    assert_eq!(
        b.agent.messages(a_id)[0].channels[0].grade,
        PrivacyGrade::P4
    );

    // `Once` does not carry over to the next message.
    let msg2 = a.send(b_id, "again").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg2,
        cla: ClaId::Share,
    });
    assert!(transmitted(&out).is_none());
}

#[test]
fn standing_rule_applies_until_revoked_and_expires() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "1").unwrap();
    let offer = offer_of(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Share,
    }));
    let until = a.now + 60_000;
    a.handle(Input::ConsentGiven(ConsentRecord {
        offer: offer.id,
        message: msg,
        cla: ClaId::Share,
        grade: offer.grade,
        scope: ConsentScope::ThisContact {
            until_unix_ms: Some(until),
        },
    }));
    let rule = a.agent.rules()[0].id;

    // Within the rule: transmits without asking, and is still logged.
    let msg2 = a.send(b_id, "2").unwrap();
    assert!(
        transmitted(&a.handle(Input::RequestTransmit {
            message: msg2,
            cla: ClaId::Share
        }))
        .is_some()
    );
    assert_eq!(
        a.agent.privacy_log()[0].basis,
        ConsentBasis::ExistingRule(rule)
    );

    // After expiry: asks again.
    a.now = until;
    let msg3 = a.send(b_id, "3").unwrap();
    assert!(
        transmitted(&a.handle(Input::RequestTransmit {
            message: msg3,
            cla: ClaId::Share
        }))
        .is_none()
    );

    // The next tick removes the expired rule from the list; the log keeps its send.
    a.tick();
    assert_eq!(a.agent.rules().len(), 0);
    assert_eq!(a.agent.privacy_log().len(), 2);
    let again = offer_of(&a.handle(Input::RequestTransmit {
        message: msg3,
        cla: ClaId::Share,
    }));
    a.handle(Input::ConsentGiven(ConsentRecord {
        offer: again.id,
        message: msg3,
        cla: ClaId::Share,
        grade: again.grade,
        scope: ConsentScope::ThisContact {
            until_unix_ms: None,
        },
    }));
    let rule = a.agent.rules()[0].id;

    // Revoked: asks again.
    a.handle(Input::RevokeRule(rule));
    let msg4 = a.send(b_id, "4").unwrap();
    assert!(
        transmitted(&a.handle(Input::RequestTransmit {
            message: msg4,
            cla: ClaId::Share
        }))
        .is_none()
    );
}

#[test]
fn raising_the_floor_allows_without_consent_lowering_requires_it_for_qr_never() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // Strictest floor: P0. QR is P0 so it is still allowed.
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P0,
    });
    let msg = a.send(b_id, "x").unwrap();
    assert!(
        transmitted(&a.handle(Input::RequestTransmit {
            message: msg,
            cla: ClaId::Qr
        }))
        .is_some()
    );
    // Floor P4: the user has chosen to allow P4 without asking.
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P4,
    });
    assert!(
        transmitted(&a.handle(Input::RequestTransmit {
            message: msg,
            cla: ClaId::Share
        }))
        .is_some()
    );
    // No privacy-log entry: P4 was within the user's floor.
    assert_eq!(a.agent.privacy_log(), vec![]);
}

#[test]
fn p5_is_never_accepted_as_a_floor() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    for input in [
        Input::SetFloor {
            contact: b_id,
            floor: PrivacyGrade::P5,
        },
        Input::SetDefaultFloor {
            floor: PrivacyGrade::P5,
        },
    ] {
        let out = a.handle(input);
        assert!(
            out.contains(&Output::Rejected(RejectReason::InvalidFloor)),
            "{out:?}"
        );
    }
    assert_eq!(a.agent.contact(b_id).unwrap().floor, PrivacyGrade::P3);
    assert_eq!(a.agent.default_floor(), PrivacyGrade::P3);
}

#[test]
fn default_floor_applies_to_new_contacts_only_and_survives_restart() {
    let (mut a, mut b, mut c) = (Device::new(1), Device::new(2), Device::new(3));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.handle(Input::SetDefaultFloor {
        floor: PrivacyGrade::P0,
    });
    a.agent = mem_core::Agent::restore(&a.agent.snapshot(), a.now).unwrap();
    assert_eq!(a.agent.default_floor(), PrivacyGrade::P0);

    let (c_id, _) = pair(&mut a, &mut c, ClaId::Qr).unwrap();
    assert_eq!(a.agent.contact(b_id).unwrap().floor, PrivacyGrade::P3);
    assert_eq!(a.agent.contact(c_id).unwrap().floor, PrivacyGrade::P0);

    // The new contact's floor governs consent offers.
    let msg = a.send(c_id, "only face to face").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Share,
    });
    assert!(transmitted(&out).is_none());
    assert_eq!(offer_of(&out).floor, PrivacyGrade::P0);
}

/// No silent downgrade for every channel, present and future: with the strictest floor (P0), asking to send a
/// message over any channel graded below it never lets the message out (no transmission, mail,
/// courier hand-off or plain text) without consent: it asks, showing the channel's real grade.
/// A new channel cannot skip the consent gate unnoticed.
#[test]
fn every_channel_below_the_floor_asks_first() {
    for cla in ClaId::ALL {
        let (mut a, mut b) = (Device::new(1), Device::new(2));
        let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
        a.handle(Input::SetChannelEnabled { cla, enabled: true });
        a.handle(Input::SetFloor {
            contact: b_id,
            floor: PrivacyGrade::P0,
        });
        let msg = a.send(b_id, "for P0 only").unwrap();
        let out = a.handle(Input::RequestTransmit { message: msg, cla });
        let escaped = out.iter().any(|o| {
            matches!(
                o,
                Output::Transmit { .. }
                    | Output::Forward { .. }
                    | Output::CourierQueued { .. }
                    | Output::EmailSend { .. }
                    | Output::PlaintextReady { .. }
            )
        });
        if cla.grade() == PrivacyGrade::P0 {
            continue;
        }
        assert!(!escaped, "{cla:?} ({:?}) sent below floor P0", cla.grade());
        let asked = out.iter().any(|o| matches!(o, Output::ConsentRequired(_)));
        assert!(asked, "{cla:?}: did not ask");
        if let Some(offer) = out.iter().find_map(|o| match o {
            Output::ConsentRequired(offer) => Some(offer),
            _ => None,
        }) {
            assert_eq!(
                offer.grade,
                cla.grade(),
                "{cla:?}: offer shows the real grade"
            );
            assert_eq!(offer.floor, PrivacyGrade::P0);
        }
    }
}
