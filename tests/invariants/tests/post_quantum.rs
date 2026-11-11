//! Post-quantum suite 2 (X-Wing). The X-Wing key travels in the first messages after
//! pairing; from then on every message to that contact is suite 2, automatically. Stated limits:
//! the first message is suite 1; contact requests and identity-fallback key updates stay suite 1.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::{ClaId, Input, Output};
use mem_invariants::{Device, pair, transmitted};
use mem_proto::Bundle;
use mem_proto::payload::Payload;

/// Seals `text` from `from` to `to` as a QR bundle and returns its bytes.
fn sealed(from: &mut Device, to: mem_core::model::ContactId, text: &str) -> Vec<u8> {
    let msg = from.send(to, text).unwrap();
    let out = from.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    transmitted(&out).unwrap().1
}

fn suite(bundle: &[u8]) -> u8 {
    Payload::decode(Bundle::decode(bundle).unwrap().payload())
        .unwrap()
        .suite_id
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

#[test]
fn the_xwing_key_travels_first_then_everything_is_suite_2() {
    let (mut a, mut b) = (Device::new(201), Device::new(202));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();

    // A's first message: suite 1 (B's X-Wing key is not known yet), carrying A's X-Wing key.
    let first = sealed(&mut a, b_id, "hello");
    assert_eq!(suite(&first), 1);
    assert!(delivered(&deliver(&mut b, first)));

    // B learned it: B's reply is suite 2, and carries B's X-Wing key.
    let reply = sealed(&mut b, a_id, "hi");
    assert_eq!(suite(&reply), 2);
    assert_eq!(
        Payload::decode(Bundle::decode(&reply).unwrap().payload())
            .unwrap()
            .enc
            .len(),
        mem_crypto::XWING_ENC_LEN
    );
    assert!(delivered(&deliver(&mut a, reply)), "suite 2 opens");

    // A learned B's key: A's next message is suite 2 as well.
    let next = sealed(&mut a, b_id, "post-quantum");
    assert_eq!(suite(&next), 2);
    let out = deliver(&mut b, next);
    assert!(delivered(&out));
    assert_eq!(b.agent.messages(a_id).last().unwrap().text, "post-quantum");
}

/// Once the contact confirmed our X-Wing key, messages stop carrying it: a key update puts the
/// inner payload in the 4 KiB bucket, a short suite-2 message without one stays under 2 KiB.
#[test]
fn the_key_stops_travelling_once_confirmed() {
    let (mut a, mut b) = (Device::new(203), Device::new(204));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let first = sealed(&mut a, b_id, "one");
    assert!(first.len() > 4096, "carries the key: {}", first.len());
    deliver(&mut b, first);
    // B's reply acknowledges A's first message (piggybacked ack).
    let reply = sealed(&mut b, a_id, "two");
    deliver(&mut a, reply);
    let later = sealed(&mut a, b_id, "three");
    assert_eq!(suite(&later), 2);
    assert!(later.len() < 2048, "no key any more: {}", later.len());
    assert!(delivered(&deliver(&mut b, later)));
}

/// Transmitted bundle sizes in `out`.
fn sent_sizes(out: &[Output]) -> Vec<usize> {
    out.iter()
        .filter_map(|o| match o {
            Output::Transmit { bytes, .. } => Some(bytes.len()),
            _ => None,
        })
        .collect()
}

/// A contact who only receives never sends a message that could acknowledge our
/// key; a suite-2 bundle sealed to our X-Wing key proves they hold it, so our acks stop carrying
/// it (1.2 KB each) once one arrives.
#[test]
fn a_suite_2_bundle_confirms_our_key() {
    use mem_core::ACK_DELAY_MS;
    use mem_invariants::{encounter, pump};
    let (mut a, mut b) = (Device::new(209), Device::new(210));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    encounter(&mut a, &mut b, ClaId::Iroh);

    // A writes; B only acknowledges. B's first ack carries B's key (B learned A's from m1).
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "m1".into(),
    });
    pump(&mut a, &mut b, out, Vec::new());
    b.advance(ACK_DELAY_MS);
    let ack1 = b.tick();
    let first = sent_sizes(&ack1);
    assert_eq!(first.len(), 1, "{ack1:?}");
    assert!(first[0] > 4096, "the first ack carries B's key: {first:?}");
    pump(&mut a, &mut b, Vec::new(), ack1);

    // A now seals to B's X-Wing key; that confirms it, so B's next ack is small.
    let out = a.handle(Input::UserSend {
        contact: b_id,
        text: "m2".into(),
    });
    pump(&mut a, &mut b, out, Vec::new());
    b.advance(ACK_DELAY_MS);
    let ack2 = b.tick();
    let second = sent_sizes(&ack2);
    assert_eq!(second.len(), 1, "{ack2:?}");
    assert!(second[0] < 2048, "no key any more: {second:?}");
}
