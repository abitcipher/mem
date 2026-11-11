//! `Bundle::decode` must never panic, and anything it accepts must be canonical.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::Bundle;

fuzz_target!(|data: &[u8]| {
    if let Ok(bundle) = Bundle::decode(data) {
        assert_eq!(bundle.encode(), data, "accepted a non-canonical encoding");
    }
});
