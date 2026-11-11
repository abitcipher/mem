//! `Inner::decode`, `decode_text` and `decode_ack_ids` must never panic, and anything they accept
//! must be canonical (including field 10 and the ack content type).
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::inner::{Inner, decode_ack_ids, decode_text, encode_ack_ids, encode_text, unpad};

fuzz_target!(|data: &[u8]| {
    if let Ok(inner) = Inner::decode(data) {
        assert_eq!(inner.encode(), data, "accepted a non-canonical inner");
        assert!(inner.fields.acks.len() <= mem_proto::inner::MAX_ACK_IDS);
    }
    if let Ok(ids) = decode_ack_ids(data) {
        assert_eq!(encode_ack_ids(&ids).unwrap(), data, "non-canonical ack content");
    }
    if let Ok(text) = decode_text(data) {
        assert_eq!(encode_text(&text).unwrap(), data, "non-canonical text");
    }
    let _ = unpad(data);
});
