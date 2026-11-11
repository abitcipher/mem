//! E2E by default and minimal envelope: transmitted bytes contain no plaintext, no
//! identity keys, no names, and only the envelope fields of `spec/envelope.cddl`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::format_collect,
    reason = "test code"
)]

use mem_core::{ClaId, Input};
use mem_invariants::{Device, pair, transmitted};
use mem_proto::Bundle;

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn transmitted_bundle_leaks_no_plaintext_or_identity() {
    let (mut a, mut b) = (Device::new(1), Device::new(2));
    a.handle(Input::SetDisplayName(Some("Alice Uniquename".into())));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let text = "the eagle lands at midnight";
    let msg = a.send(b_id, text).unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();

    assert!(!contains(&bytes, text.as_bytes()), "plaintext in bundle");
    assert!(!contains(&bytes, b"Alice"), "display name in bundle");
    // Card layout: map header, key 0, value 1, key 1, bstr header (0x58 0x20), 32-byte key.
    let a_card = a.card();
    assert_eq!(&a_card[4..6], &[0x58, 0x20]);
    assert!(
        !contains(&bytes, &a_card[6..38]),
        "sender identity key in bundle"
    );
    let b_card = b.card();
    assert!(
        !contains(&bytes, &b_card[6..38]),
        "recipient identity key in bundle"
    );
    assert!(
        !contains(&bytes, msg.as_bytes()),
        "message id (UUIDv7, timestamped) in bundle"
    );

    // The bundle decodes as an envelope with the expected direct-delivery parameters.
    let bundle = Bundle::decode(&bytes).unwrap();
    assert_eq!(bundle.hops_remaining(), 0);
    assert_eq!(bundle.copies(), 1);
    // Padded to a bucket: short texts all have the same size.
    let msg2 = a.send(b_id, "hi").unwrap();
    let (_, bytes2) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg2,
        cla: ClaId::Qr,
    }))
    .unwrap();
    assert_eq!(bytes.len(), bytes2.len(), "padding buckets hide length");
}
