//! `HubList::decode` reads lists served by any hub: it must never panic, and anything
//! it accepts must be the canonical encoding of what it returns (the signature covers it).
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::hublist::HubList;

fuzz_target!(|data: &[u8]| {
    if let Ok(l) = HubList::decode(data) {
        assert_eq!(l.encode().ok().as_deref(), Some(data), "non-canonical {l:?}");
    }
});
