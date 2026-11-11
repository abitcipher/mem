//! Mailbox key rotation: weekly keys, a 30-day window for late bundles, keys announced
//! end to end until acknowledged, standalone updates, and holding messages for stale keys.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{ContactId, DeliveryStatus, RejectReason};
use mem_core::{ClaId, Input, Output};
use mem_invariants::{Device, LINK_PEER, encounter, pair, transmitted};
use mem_proto::{Bundle, BundleKind};

const DAY: u64 = 24 * 60 * 60 * 1000;

/// A sealed QR bundle of a new message from `from` to `to`.
fn sealed(from: &mut Device, to: ContactId, text: &str) -> Vec<u8> {
    let msg = from.send(to, text).unwrap();
    transmitted(&from.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap()
    .1
}

fn deliver(to: &mut Device, bytes: Vec<u8>) -> Vec<Output> {
    to.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    })
}

fn delivered(out: &[Output]) -> bool {
    out.iter().any(|o| matches!(o, Output::Deliver { .. }))
}

fn advance(devs: &mut [&mut Device], ms: u64) {
    for d in devs {
        d.advance(ms);
        d.tick();
    }
}

#[test]
fn old_keys_open_late_bundles_for_thirty_days_then_not() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // Sealed to B's first key, then held back.
    let late = sealed(&mut a, b_id, "late");
    let late2 = sealed(&mut a, b_id, "later");
    // B rotates after a week; the old key is retired, not deleted.
    advance(&mut [&mut b], 8 * DAY);
    assert!(delivered(&deliver(&mut b, late)), "retired key still opens");
    // 30 days after retirement it is gone (by then the bundle is also past its lifetime and its
    // tag out of the window: refused either way, never delivered).
    advance(&mut [&mut b], 31 * DAY);
    let out = deliver(&mut b, late2);
    assert!(!delivered(&out));
    assert!(
        out.iter().any(|o| matches!(o, Output::Rejected(_))),
        "{out:?}"
    );
}

#[test]
fn a_new_key_travels_in_messages_until_acknowledged() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // B rotates; B's next message to A announces the new key.
    advance(&mut [&mut a, &mut b], 8 * DAY);
    let first = sealed(&mut b, a_id, "with key");
    assert!(delivered(&deliver(&mut a, first)));
    // A now seals to B's new key: B opens it (and so the announcement worked).
    let reply = sealed(&mut a, b_id, "to the new key");
    assert!(delivered(&deliver(&mut b, reply)));
    // A's reply acknowledged B's message; B's later messages still reach A.
    let next = sealed(&mut b, a_id, "no key now");
    assert!(delivered(&deliver(&mut a, next)));
}

#[test]
fn a_standalone_key_update_goes_out_after_a_quiet_day() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, _a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // B rotates and does not message A for more than a day.
    advance(&mut [&mut a, &mut b], 8 * DAY);
    advance(&mut [&mut a, &mut b], 2 * DAY);
    // Next time they meet over Wi-Fi, B sends a key-update bundle by itself, and A acks it.
    let (_, out_b) = encounter(&mut a, &mut b, ClaId::Iroh);
    let sent_update = out_b.iter().any(|o| match o {
        Output::Transmit { bytes, .. } => {
            Bundle::decode(bytes).is_ok_and(|x| x.kind() == BundleKind::KeyUpdate)
        }
        _ => false,
    });
    assert!(sent_update, "{out_b:?}");
    a.advance(3_000);
    a.tick();
    b.handle(Input::EncounterEnded { peer: LINK_PEER });
    // It never shows up as a message on A.
    assert_eq!(a.agent.messages(b_id).len(), 0);
}

#[test]
fn after_long_silence_key_updates_unlock_held_messages() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    // 38 days without a word: each side's knowledge of the other's key may be deleted
    // (7 days current + 30 retained).
    advance(&mut [&mut a, &mut b], 38 * DAY);
    advance(&mut [&mut a, &mut b], 2 * DAY);
    let msg = a.send(b_id, "held").unwrap();
    let out = a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    assert!(
        out.contains(&Output::Rejected(RejectReason::WaitingForKey)),
        "{out:?}"
    );
    assert_eq!(a.agent.messages(b_id)[0].status, DeliveryStatus::Queued);

    // They meet on Wi-Fi: key updates go to each other's identity key (no deadlock), each learns
    // the other's current key, and A's held message is resealed and delivered.
    encounter(&mut a, &mut b, ClaId::Iroh);
    assert_eq!(
        b.agent.messages(a_id).last().map(|m| m.text.clone()),
        Some("held".into())
    );
}

/// Reading a contact's old card again (the same pairing) keeps the key learned since: it does not
/// fall back to the card's key, which the contact zeroizes 30 days after retiring it, and it is
/// reported as unchanged (no symbols to compare).
#[test]
fn reading_an_old_card_again_keeps_the_newer_key() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    let old_card = b.card();
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    advance(&mut [&mut a, &mut b], 8 * DAY);
    assert!(delivered(&deliver(&mut a, sealed(&mut b, a_id, "new key"))));
    let out = a.handle(Input::ImportCard {
        cla: ClaId::Qr,
        bytes: old_card,
    });
    let unchanged = out.iter().any(|o| {
        matches!(
            o,
            Output::ContactPaired {
                unchanged: true,
                ..
            }
        )
    });
    // By day 39 the card's key is gone at B; the key A learned on day 8 still opens.
    advance(&mut [&mut a, &mut b], 31 * DAY);
    assert!(delivered(&deliver(
        &mut b,
        sealed(&mut a, b_id, "still reaches B")
    )));
    assert!(unchanged, "nothing to compare");
}

/// A card read in person after a remote request upgrades trust: that is a change, so the
/// symbols are shown.
#[test]
fn a_card_that_raises_trust_is_not_unchanged() {
    let (mut a, mut b) = (Device::new(3), Device::new(4));
    a.import(&b, ClaId::Share).unwrap();
    b.import(&a, ClaId::Share).unwrap();
    let out = a.handle(Input::ImportCard {
        cla: ClaId::Qr,
        bytes: b.card(),
    });
    assert!(out.iter().any(|o| matches!(
        o,
        Output::ContactPaired {
            updated: true,
            unchanged: false,
            ..
        }
    )));
}
