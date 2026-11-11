//! The group content decoders must never panic, and anything they accept must be the
//! canonical encoding of what they return.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::group::{GroupReply, GroupState, GroupText};

fuzz_target!(|data: &[u8]| {
    if let Ok(t) = GroupText::decode(data) {
        assert_eq!(t.encode().ok().as_deref(), Some(data), "non-canonical text");
    }
    if let Ok(s) = GroupState::decode(data) {
        assert_eq!(s.encode().ok().as_deref(), Some(data), "non-canonical state");
    }
    if let Ok(r) = GroupReply::decode(data) {
        assert_eq!(r.encode().ok().as_deref(), Some(data), "non-canonical reply");
    }
});
