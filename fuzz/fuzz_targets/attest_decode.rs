//! Attestation objects arrive from QR codes shown by anyone and from relying
//! parties' pages: the decoders must never panic, and anything they accept must be the
//! canonical encoding of what they return. The first byte picks the decoder.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::attest::{AttestOffer, AttestRequest, AttestStatement, Attestation, dearmor};

fuzz_target!(|data: &[u8]| {
    let Some((&pick, input)) = data.split_first() else {
        return;
    };
    match pick % 5 {
        0 => {
            if let Ok(r) = AttestRequest::decode(input) {
                assert_eq!(r.encode().ok().as_deref(), Some(input), "non-canonical {r:?}");
            }
        }
        1 => {
            if let Ok(o) = AttestOffer::decode(input) {
                assert_eq!(o.encode().ok().as_deref(), Some(input), "non-canonical {o:?}");
            }
        }
        2 => {
            if let Ok(s) = AttestStatement::decode(input) {
                assert_eq!(s.encode().ok().as_deref(), Some(input), "non-canonical {s:?}");
            }
        }
        3 => {
            if let Ok(a) = Attestation::decode(input) {
                assert_eq!(a.encode().ok().as_deref(), Some(input), "non-canonical {a:?}");
            }
        }
        _ => {
            if let Ok(text) = core::str::from_utf8(input) {
                let _ = dearmor(text);
            }
        }
    }
});
