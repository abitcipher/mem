//! Connect hints arrive by sound and QR from anyone nearby: the decoder must never
//! panic, and anything it accepts must be the canonical encoding of what it returns.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::hint::ConnectHint;

fuzz_target!(|data: &[u8]| {
    if let Ok(h) = ConnectHint::decode(data) {
        assert_eq!(h.encode().ok().as_deref(), Some(data), "non-canonical {h:?}");
    }
});
