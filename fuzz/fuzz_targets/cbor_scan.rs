//! The bounded CBOR scanner must never panic, for any limits.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::cbor::{DecodeLimits, scan};

fuzz_target!(|input: (u16, &[u8])| {
    let (max_container_len, data) = input;
    let limits = DecodeLimits {
        max_input_len: data.len(),
        max_container_len: u64::from(max_container_len),
    };
    let _ = scan(data, &limits);
});
