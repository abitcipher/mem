//! The sound channel: it carries pairing cards only. A card heard in the room means
//! "met in person", as a scanned QR card does; no message can ever go by sound.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{RejectReason, TrustLevel};
use mem_core::{ClaId, Input, Output, PrivacyGrade};
use mem_invariants::Device;

#[test]
fn a_card_heard_in_the_room_is_met_in_person() {
    let (mut a, mut b) = (Device::new(41), Device::new(42));
    let b_id = a.import(&b, ClaId::Audio).unwrap();
    let a_id = b.import(&a, ClaId::Audio).unwrap();
    assert_eq!(
        a.agent.contact(b_id).unwrap().trust,
        TrustLevel::MetInPerson
    );
    assert_eq!(
        b.agent.contact(a_id).unwrap().trust,
        TrustLevel::MetInPerson
    );
    assert_eq!(ClaId::Audio.grade(), PrivacyGrade::P0);
}

#[test]
fn no_message_ever_goes_by_sound() {
    let (mut a, mut b) = (Device::new(43), Device::new(44));
    let b_id = a.import(&b, ClaId::Audio).unwrap();
    b.import(&a, ClaId::Audio).unwrap();
    let msg = a.send(b_id, "not by sound").unwrap();
    assert!(!ClaId::ALL.contains(&ClaId::Audio));
    assert!(
        a.agent
            .channel_options(msg)
            .iter()
            .all(|o| o.cla != ClaId::Audio)
    );
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Audio,
    });
    assert!(
        !out.iter().any(|o| matches!(o, Output::Transmit { .. })),
        "{out:?}"
    );
    assert!(out.contains(&Output::Rejected(RejectReason::TooLargeForChannel)));
}
