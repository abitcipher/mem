//! `Frame::decode` must never panic, and anything it accepts must be canonical.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::frame::Frame;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = Frame::decode(data) {
        assert_eq!(frame.encode(), data, "accepted a non-canonical encoding");
    }
});
