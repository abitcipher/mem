//! `EncounterMsg::decode` must never panic. Accepted messages of the kinds this version acts on
//! must be canonical; `hello` may carry unknown `caps` (ignored for forward compatibility, so
//! re-encoding drops them) and must round-trip to the same value.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::encounter::EncounterMsg;

fuzz_target!(|data: &[u8]| {
    match EncounterMsg::decode(data) {
        Ok(msg @ EncounterMsg::Hello { .. }) => {
            let again = msg
                .encode()
                .ok()
                .and_then(|b| EncounterMsg::decode(&b).ok());
            assert_eq!(again.as_ref(), Some(&msg), "hello does not round-trip");
        }
        Err(_) => {}
        Ok(msg) => assert_eq!(
            msg.encode().ok().as_deref(),
            Some(data),
            "accepted a non-canonical encoding"
        ),
    }
});
