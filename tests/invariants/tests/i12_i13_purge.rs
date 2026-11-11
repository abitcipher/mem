//! Purged stays purged and token presentation is not identity.
//!
//! v0.1 has one admission path, direct receipt by the recipient; courier and hub admission paths
//! arrive in Phases 3 and 4 and must be added here.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "test code"
)]

use mem_core::model::RejectReason;
use mem_core::{ClaId, Input, Output};
use mem_invariants::{Device, pair, transmitted};
use mem_proto::Bundle;

fn delivered_bundle() -> (Device, Device, Vec<u8>) {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "once only").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    let out = b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: bytes.clone(),
    });
    assert!(out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    (a, b, bytes)
}

#[test]
fn every_bundle_carries_a_commitment_bound_into_the_envelope() {
    let (_, b, bytes) = delivered_bundle();
    let bundle = Bundle::decode(&bytes).unwrap();
    assert!(b.agent.is_purged(bundle.purge_commitment()));
}

#[test]
fn delivered_bundle_is_refused_on_every_later_copy() {
    let (_, mut b, bytes) = delivered_bundle();
    // The same bytes again, e.g. a second spray copy via another channel.
    assert_refused(&b.handle(Input::Received {
        cla: ClaId::Share,
        bytes: bytes.clone(),
    }));
    // A copy aged by a courier (age is outside the AAD) is still the same purged bundle.
    let aged = Bundle::decode(&bytes)
        .unwrap()
        .with_age_ms(5_000)
        .unwrap()
        .encode();
    assert_ne!(aged, bytes);
    assert_refused(&b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: aged,
    }));
}

#[test]
fn tombstone_check_happens_before_decryption() {
    let (_, mut b, bytes) = delivered_bundle();
    // Corrupt the ciphertext: an unpurged bundle would be Undecryptable, a purged one is refused
    // earlier without spending any decryption work.
    let mut bad = bytes;
    let i = bad.len() - 20;
    bad[i] ^= 1;
    assert_refused(&b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: bad,
    }));
}

#[test]
fn tombstones_survive_restart() {
    let (_, mut b, bytes) = delivered_bundle();
    assert_eq!(b.store.tombstones.len(), 1, "written to the database");
    b.restart().unwrap();
    assert_refused(&b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    }));
}

#[test]
fn tombstones_expire_after_the_maximum_bundle_lifetime() {
    let (_, mut b, bytes) = delivered_bundle();
    let commitment = *Bundle::decode(&bytes).unwrap().purge_commitment();
    // Any input after 30 days prunes on the next tombstone write; deliver another message then.
    b.now += 31 * 24 * 60 * 60 * 1000;
    let mut c = Device::new(9);
    c.now = b.now;
    let (b_in_c, _) = pair(&mut c, &mut b, ClaId::Qr).unwrap();
    let msg = c.send(b_in_c, "later").unwrap();
    let (_, later) = transmitted(&c.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: later,
    });
    assert!(!b.agent.is_purged(&commitment));
}

/// A token arriving by gossip proves nothing about who presents it. The sender who hears
/// its own message's token from a stranger stops spraying it, but does not mark it delivered:
/// only an end-to-end ack does.
#[test]
fn presenting_a_token_does_not_identify_the_recipient() {
    use mem_core::model::{DeliveryStatus, PeerHandle};
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "gossiped").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    b.handle(Input::Received {
        cla: ClaId::Qr,
        bytes: bytes.clone(),
    });
    let commitment = *Bundle::decode(&bytes).unwrap().purge_commitment();
    let token = b.store.tombstones[&commitment].token;

    // A stranger (not B) presents the token to A.
    let stranger = PeerHandle(77);
    a.handle(Input::EncounterStarted {
        peer: stranger,
        cla: ClaId::Iroh,
    });
    a.handle(Input::EncounterMessage {
        peer: stranger,
        msg: mem_core::EncounterMsg::Hello {
            version: mem_proto::encounter::ENCOUNTER_VERSION,
            ephemeral_id: [5; 16],
        },
    });
    a.handle(Input::EncounterMessage {
        peer: stranger,
        msg: mem_core::EncounterMsg::PurgeTokens(vec![token]),
    });
    assert!(
        a.agent.is_purged(&commitment),
        "A learns and keeps the token"
    );
    let status = a
        .agent
        .messages(b_id)
        .into_iter()
        .find(|m| m.id == msg)
        .unwrap()
        .status;
    assert_ne!(status, DeliveryStatus::Delivered);
}

/// The copy is refused and nothing is delivered. A copy of a bundle this device delivered
/// may also save a renewed ack, which admits nothing.
fn assert_refused(out: &[Output]) {
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::Rejected(RejectReason::Purged)))
            && out
                .iter()
                .all(|o| matches!(o, Output::Rejected(_) | Output::Persist(_))),
        "{out:?}"
    );
}
