//! The fountain assembler must never panic on any sequence of frames. The input is split
//! into length-prefixed chunks, each fed as one frame; valid-looking frames are also derived
//! from the input so the decoder itself gets exercised.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_fountain::Assembler;
use mem_fountain::frame::{Frame, ObjectKind, TransferId};

fuzz_target!(|data: &[u8]| {
    let mut a = Assembler::new();
    let mut rest = data;
    while let [len, tail @ ..] = rest {
        let n = usize::from(*len).min(tail.len());
        let (chunk, next) = tail.split_at(n);
        rest = next;
        let _ = a.push(chunk);
        // A well-formed frame with fuzzed header values and symbol bytes.
        if chunk.len() >= 8 {
            let object_len = u32::from(u16::from_le_bytes([chunk[0], chunk[1]])) + 1;
            let esi = u32::from(u16::from_le_bytes([chunk[2], chunk[3]]));
            let symbol_len = 64 + usize::from(chunk[4] % 4) * 64;
            let mut symbol = chunk[5..].to_vec();
            symbol.resize(symbol_len, chunk[5]);
            if let Ok(f) = Frame::new(
                TransferId([chunk[6]; 8]),
                ObjectKind::Bundle,
                object_len,
                esi,
                symbol,
            ) {
                let _ = a.push(&f.encode());
            }
        }
    }
});
