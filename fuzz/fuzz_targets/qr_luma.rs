//! QR detection on hostile camera images must never panic. The first 4 bytes choose the
//! dimensions (at most 255 × 255, plus stride padding); the rest is the image.
#![no_main]

use cla_qr::Scanner;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let [w, h, pad, _, image @ ..] = data else {
        return;
    };
    let (w, h) = (u32::from(*w), u32::from(*h));
    let stride = w + u32::from(*pad % 16);
    let _ = Scanner::new().scan_luma(image, w, h, stride);
});
