//! The BLE stream reassembler (`cla-ble`) must never panic, and how the radio cut the
//! stream must not change what comes out: any chunking gives the same messages, or the same
//! first error, as the whole stream at once.
#![no_main]

use cla_ble::Deframer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&chunk, stream)) = data.split_first() else {
        return;
    };
    let chunk = usize::from(chunk).max(1);
    let whole = Deframer::default().push(stream);
    let mut d = Deframer::default();
    let mut got = Vec::new();
    let mut failed = None;
    for piece in stream.chunks(chunk) {
        match d.push(piece) {
            Ok(m) => got.extend(m),
            Err(e) => {
                failed = Some(e);
                break;
            }
        }
    }
    match (whole, failed) {
        (Ok(all), None) => assert_eq!(all, got),
        (Err(a), Some(b)) => assert_eq!(a, b),
        (w, f) => panic!("chunking changed the outcome: {w:?} vs {f:?}"),
    }
});
