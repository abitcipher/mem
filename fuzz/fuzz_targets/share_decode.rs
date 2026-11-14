//! The share decoders must never panic, and anything they accept must be the canonical
//! encoding of what they return.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::share::{ShareCatalog, ShareRef};

fuzz_target!(|data: &[u8]| {
    if let Ok(c) = ShareCatalog::decode(data) {
        assert_eq!(c.encode().ok().as_deref(), Some(data), "non-canonical catalog");
    }
    if let Ok(r) = ShareRef::decode(data) {
        assert_eq!(r.encode().as_slice(), data, "non-canonical ref");
    }
});
