//! Test helper: decodes ggwave frames from a 16-bit mono 48 kHz WAV recording and prints
//! what it found (frame sizes and fountain transfer ids; cards are public).
//!
//! `cargo run -p cla-audio --example decode_wav -- recording.wav`

#![allow(clippy::print_stdout, reason = "command-line tool")]

use std::fs;

use cla_audio::Decoder;
use mem_proto::frame::Frame;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        println!("usage: decode_wav <file.wav>");
        return;
    };
    let Ok(bytes) = fs::read(&path) else {
        println!("cannot read {path}");
        return;
    };
    // Skip the 44-byte canonical WAV header; samples are little-endian i16.
    let samples: Vec<i16> = bytes
        .get(44..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    let Ok(mut decoder) = Decoder::new() else {
        println!("ggwave did not start");
        return;
    };
    let mut found = 0;
    for (i, chunk) in samples.chunks(4800).enumerate() {
        for payload in decoder.push(chunk) {
            found += 1;
            let at = i * 100;
            match Frame::decode(&payload) {
                Ok(f) => println!(
                    "{at} ms: frame {:?} ({} bytes)",
                    f.transfer(),
                    payload.len()
                ),
                Err(_) => println!("{at} ms: {} bytes, not a MEM frame", payload.len()),
            }
        }
    }
    println!("{found} payloads in {} s", samples.len() / 48_000);
}
