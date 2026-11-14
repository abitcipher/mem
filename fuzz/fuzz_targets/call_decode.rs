//! The call setup decoder must never panic, and anything it accepts must be the canonical
//! encoding of what it returns.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::call::CallSignal;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = CallSignal::decode(data) {
        assert_eq!(s.encode().ok().as_deref(), Some(data), "non-canonical call setup");
    }
});
