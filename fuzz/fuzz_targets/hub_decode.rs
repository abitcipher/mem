//! `HubMsg::decode` runs on the hub (through the NIF) and on every client: it must
//! never panic, and anything it accepts must be the canonical encoding of what it returns.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::hub::HubMsg;

fuzz_target!(|data: &[u8]| {
    if let Ok(m) = HubMsg::decode(data) {
        assert_eq!(m.encode().ok().as_deref(), Some(data), "non-canonical {m:?}");
    }
});
