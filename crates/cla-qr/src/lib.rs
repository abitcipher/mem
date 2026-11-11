//! Animated QR channel, `cla-qr`. Grade P0.
//!
//! - **Send:** an object (bundle or pairing card) becomes an endless stream of fountain-coded
//!   frames ([`mem_fountain`]); each frame is rendered as a QR bit matrix in byte mode, ECC M.
//!   The UI only paints the matrix (no image encoding).
//! - **Receive:** the camera's luma (Y) plane goes to [`Scanner::scan_luma`], which finds a QR
//!   code with `rxing` (pure Rust, also builds for Wasm), feeds frames to the assembler and
//!   reports progress until the object is complete. Static text QR codes (`memcard1:`/`mem1:`
//!   from v0.1) are passed through as text.
//!
//! Everything a camera sees is hostile: the decoders are bounded and never panic.

use mem_fountain::frame::{Frame, MAX_FRAME_LEN, ObjectKind};
use mem_fountain::{Assembler, Progress, Sender};
use qrcode::bits::Bits;
use qrcode::{Color, EcLevel, QrCode, Version};
use rxing::common::HybridBinarizer;
use rxing::qrcode::QRCodeReader;
use rxing::{
    BarcodeFormat, BinaryBitmap, DecodeHints, Luma8LuminanceSource, RXingResultMetadataType,
    RXingResultMetadataValue, Reader,
};

/// Symbol size of [`Pace::Fast`], the initial value. The frame is 406 bytes, a version
/// 15 QR code at ECC M (77 modules).
pub const DEFAULT_SYMBOL_LEN: u16 = 384;

/// How densely and how quickly an animated transfer is shown: a user setting.
/// A fixed-focus webcam cannot follow version 15 at 8 frames/s, so a
/// sparser, slower code is offered. The receiver adapts by itself: every frame declares its
/// symbol size, and a change of pace starts a parallel transfer with the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pace {
    /// Large modules, slow: for weak or fixed-focus cameras and screens with glare.
    /// Version 8 (49 modules), 5 frames/s, about 0.6 KiB/s.
    Easy,
    /// The default. Version 12 (65 modules), 6 frames/s, about 1.5 KiB/s.
    #[default]
    Normal,
    /// Dense and quick, for good cameras. Version 15 (77 modules), 8 frames/s, about 3 KiB/s.
    Fast,
}

impl Pace {
    /// Fountain symbol size in bytes.
    #[must_use]
    pub const fn symbol_len(self) -> u16 {
        match self {
            Self::Easy => 128,
            Self::Normal => 256,
            Self::Fast => DEFAULT_SYMBOL_LEN,
        }
    }

    /// Frames shown per second.
    #[must_use]
    pub const fn frames_per_second(self) -> u8 {
        match self {
            Self::Easy => 5,
            Self::Normal => 6,
            Self::Fast => 8,
        }
    }
}
/// Largest camera image accepted, in pixels.
pub const MAX_IMAGE_PIXELS: usize = 4096 * 4096;
/// Images larger than this on their short side are downscaled 2× before decoding.
const DOWNSCALE_ABOVE: u32 = 900;

/// Errors from this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum QrError {
    /// The data does not fit a QR code at ECC M.
    #[error("data does not fit in a QR code")]
    TooLarge,
    /// The object cannot be fountain-coded.
    #[error("fountain: {0}")]
    Fountain(#[from] mem_fountain::FountainError),
}

/// A QR code as a square bit matrix, row-major, 1 = dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrMatrix {
    /// Modules per side.
    pub width: u32,
    /// `width * width` entries of 0 or 1.
    pub modules: Vec<u8>,
}

impl QrMatrix {
    fn from_code(code: &QrCode) -> Option<Self> {
        Some(Self {
            width: u32::try_from(code.width()).ok()?,
            modules: code
                .to_colors()
                .into_iter()
                .map(|c| u8::from(c == Color::Dark))
                .collect(),
        })
    }
}

/// Renders binary data as one byte-mode segment at ECC M in the smallest version that fits.
/// Byte mode keeps the bytes intact for the decoder (other modes would re-encode them).
///
/// # Errors
///
/// [`QrError::TooLarge`] if no version fits.
pub fn render_bytes(data: &[u8]) -> Result<QrMatrix, QrError> {
    for v in 1..=40 {
        let mut bits = Bits::new(Version::Normal(v));
        if bits.push_byte_data(data).is_ok()
            && bits.push_terminator(EcLevel::M).is_ok()
            && let Ok(code) = QrCode::with_bits(bits, EcLevel::M)
        {
            return QrMatrix::from_code(&code).ok_or(QrError::TooLarge);
        }
    }
    Err(QrError::TooLarge)
}

/// Renders text (e.g. an armored card) with automatic mode selection, as in v0.1.
#[must_use]
pub fn render_text(text: &str) -> Option<QrMatrix> {
    QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .ok()
        .and_then(|c| QrMatrix::from_code(&c))
}

/// The sending side of one transfer: an endless sequence of QR frames.
#[derive(Debug)]
pub struct Animation {
    sender: Sender,
}

impl Animation {
    /// Prepares `object` for animated display.
    ///
    /// # Errors
    ///
    /// [`QrError::Fountain`] if the object is empty or over 64 KiB.
    pub fn new(kind: ObjectKind, object: &[u8]) -> Result<Self, QrError> {
        Self::with_symbol_len(kind, object, DEFAULT_SYMBOL_LEN)
    }

    /// As [`Animation::new`] with an explicit symbol size.
    ///
    /// # Errors
    ///
    /// See [`Animation::new`].
    pub fn with_symbol_len(
        kind: ObjectKind,
        object: &[u8],
        symbol_len: u16,
    ) -> Result<Self, QrError> {
        Ok(Self {
            sender: Sender::new(kind, object, symbol_len)?,
        })
    }

    /// Minimum number of frames the receiver must catch.
    #[must_use]
    pub const fn frames_needed(&self) -> u32 {
        self.sender.source_symbols()
    }

    /// Frame `n` of the endless stream as an encoded frame.
    #[must_use]
    pub fn frame_bytes(&self, n: u32) -> Vec<u8> {
        self.sender.frame(n)
    }

    /// Frame `n` of the endless stream as a QR matrix.
    ///
    /// # Errors
    ///
    /// [`QrError::TooLarge`] cannot happen for frames within the symbol limits.
    pub fn frame(&self, n: u32) -> Result<QrMatrix, QrError> {
        render_bytes(&self.sender.frame(n))
    }
}

/// What a QR code contained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// Binary data from byte-mode segments (a frame, if it decodes as one).
    Bytes(Vec<u8>),
    /// Text, for QR codes that are not frames.
    Text(String),
}

/// What one camera image achieved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanEvent {
    /// No QR code found, or one that is neither a frame nor text.
    Nothing,
    /// A text QR code (e.g. a `memcard1:` card from v0.1).
    Text(String),
    /// A frame of a transfer in progress.
    Progress {
        /// Distinct frames collected.
        received: u32,
        /// Frames needed (source symbols).
        needed: u32,
    },
    /// A transfer completed.
    Complete {
        /// What it carried.
        kind: ObjectKind,
        /// The object.
        object: Vec<u8>,
    },
    /// A frame of a transfer that already completed.
    AlreadyComplete,
}

/// The receiving side: finds QR codes in camera images and reassembles transfers.
#[derive(Debug, Default)]
pub struct Scanner {
    assembler: Assembler,
}

impl Scanner {
    /// A fresh scanner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes one camera luma plane (`row_stride >= width` bytes per row) and feeds what it
    /// finds. Only the centred square of the image is searched.
    pub fn scan_luma(
        &mut self,
        luma: &[u8],
        width: u32,
        height: u32,
        row_stride: u32,
    ) -> ScanEvent {
        match decode_luma(luma, width, height, row_stride) {
            Some(p) => self.feed(p),
            None => ScanEvent::Nothing,
        }
    }

    /// Feeds an already decoded QR payload.
    pub fn feed(&mut self, payload: Payload) -> ScanEvent {
        match payload {
            Payload::Bytes(bytes) => match self.assembler.push(&bytes) {
                Ok(Progress::Partial { received, needed }) => {
                    ScanEvent::Progress { received, needed }
                }
                Ok(Progress::Complete { kind, object }) => ScanEvent::Complete { kind, object },
                Ok(Progress::AlreadyComplete) => ScanEvent::AlreadyComplete,
                Err(_) => match String::from_utf8(bytes) {
                    Ok(text) => ScanEvent::Text(text),
                    Err(_) => ScanEvent::Nothing,
                },
            },
            Payload::Text(text) => ScanEvent::Text(text),
        }
    }

    /// Drops transfers in progress.
    pub fn reset(&mut self) {
        self.assembler.reset();
    }
}

/// Finds one QR code in the centred square of a luma image and returns its payload.
///
/// # Security
///
/// Bounded: images over [`MAX_IMAGE_PIXELS`] or with an inconsistent stride are ignored, the
/// payload is capped by the QR format itself (< 3 KiB), and `rxing` errors are swallowed.
#[must_use]
pub fn decode_luma(luma: &[u8], width: u32, height: u32, row_stride: u32) -> Option<Payload> {
    let (crop, side) = center_square(luma, width, height, row_stride)?;
    let reader_input = Luma8LuminanceSource::new(crop, side, side).ok()?;
    let mut bitmap = BinaryBitmap::new(HybridBinarizer::new(reader_input));
    let hints = DecodeHints {
        PossibleFormats: Some([BarcodeFormat::QR_CODE].into_iter().collect()),
        TryHarder: Some(true),
        ..DecodeHints::default()
    };
    let result = QRCodeReader.decode_with_hints(&mut bitmap, &hints).ok()?;
    if let Some(RXingResultMetadataValue::ByteSegments(segments)) = result
        .getRXingResultMetadata()
        .get(&RXingResultMetadataType::BYTE_SEGMENTS)
    {
        let bytes: Vec<u8> = segments.concat();
        if !bytes.is_empty() && bytes.len() <= MAX_FRAME_LEN && Frame::decode(&bytes).is_ok() {
            return Some(Payload::Bytes(bytes));
        }
    }
    let text = result.getText();
    (!text.is_empty()).then(|| Payload::Text(text.to_owned()))
}

/// Copies the centred square of the image into a contiguous buffer, downscaling 2× when it is
/// large. Returns `None` for inconsistent dimensions.
fn center_square(luma: &[u8], width: u32, height: u32, stride: u32) -> Option<(Vec<u8>, u32)> {
    let (w, h, row) = (
        usize::try_from(width).ok()?,
        usize::try_from(height).ok()?,
        usize::try_from(stride).ok()?,
    );
    if w == 0 || h == 0 || row < w || w.checked_mul(h)? > MAX_IMAGE_PIXELS {
        return None;
    }
    // The last row need not be padded to the full stride.
    if luma.len() < row.checked_mul(h - 1)?.checked_add(w)? {
        return None;
    }
    let side = w.min(h);
    let (left, top) = ((w - side) / 2, (h - side) / 2);
    let step = if u32::try_from(side).ok()? > DOWNSCALE_ABOVE {
        2
    } else {
        1
    };
    let out_side = side / step;
    let mut out = Vec::with_capacity(out_side * out_side);
    for oy in 0..out_side {
        let at = (top + oy * step) * row;
        for ox in 0..out_side {
            let i = at + left + ox * step;
            let px = if step == 1 {
                *luma.get(i)?
            } else {
                let sum = u16::from(*luma.get(i)?)
                    + u16::from(*luma.get(i + 1)?)
                    + u16::from(*luma.get(i + row)?)
                    + u16::from(*luma.get(i + row + 1)?);
                u8::try_from(sum / 4).ok()?
            };
            out.push(px);
        }
    }
    Some((out, u32::try_from(out_side).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paints a matrix like a phone screen seen by a camera: white quiet zone, `scale` pixels
    /// per module, placed off-centre in a larger grey frame with a row stride.
    fn photograph(m: &QrMatrix, scale: usize, frame_w: usize, frame_h: usize) -> (Vec<u8>, u32) {
        let stride = frame_w + 16;
        let mut img = vec![90u8; stride * frame_h];
        let side = (m.width as usize + 8) * scale;
        let (x0, y0) = ((frame_w - side) / 2 + 3, (frame_h - side) / 2 - 2);
        for y in 0..side {
            for x in 0..side {
                let (mx, my) = ((x / scale).wrapping_sub(4), (y / scale).wrapping_sub(4));
                let dark = mx < m.width as usize
                    && my < m.width as usize
                    && m.modules[my * m.width as usize + mx] == 1;
                img[(y0 + y) * stride + x0 + x] = if dark { 20 } else { 235 };
            }
        }
        (img, u32::try_from(stride).unwrap())
    }

    fn sample(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| u8::try_from(i * 7 % 251).unwrap())
            .collect()
    }

    #[test]
    fn frame_qr_size_is_in_the_planned_range() {
        let a = Animation::new(ObjectKind::Bundle, &sample(5000)).unwrap();
        let m = a.frame(0).unwrap();
        // Version 15 at ECC M: 77 modules.
        assert_eq!(m.width, 77);
    }

    #[test]
    fn each_pace_renders_its_planned_version() {
        // Modules per side = 17 + 4 × version.
        for (pace, modules) in [(Pace::Easy, 49), (Pace::Normal, 65), (Pace::Fast, 77)] {
            let a =
                Animation::with_symbol_len(ObjectKind::Bundle, &sample(5000), pace.symbol_len())
                    .unwrap();
            for n in [0, 1, 500] {
                assert_eq!(a.frame(n).unwrap().width, modules, "{pace:?} frame {n}");
            }
        }
    }

    #[test]
    fn a_change_of_pace_mid_transfer_still_completes() {
        let obj = sample(1500);
        let fast =
            Animation::with_symbol_len(ObjectKind::Bundle, &obj, Pace::Fast.symbol_len()).unwrap();
        let easy =
            Animation::with_symbol_len(ObjectKind::Bundle, &obj, Pace::Easy.symbol_len()).unwrap();
        let mut s = Scanner::new();
        // One fast frame, then the sender switches to easy.
        assert!(matches!(
            s.feed(Payload::Bytes(fast.frame_bytes(0))),
            ScanEvent::Progress { .. }
        ));
        let mut done = None;
        for n in 0..easy.frames_needed() {
            if let ScanEvent::Complete { object, .. } = s.feed(Payload::Bytes(easy.frame_bytes(n)))
            {
                done = Some(object);
            }
        }
        assert_eq!(done, Some(obj));
    }

    #[test]
    fn camera_roundtrip_of_an_animated_transfer() {
        let obj = sample(1100);
        let a = Animation::new(ObjectKind::Bundle, &obj).unwrap();
        assert_eq!(a.frames_needed(), 3);
        let mut s = Scanner::new();
        let mut result = None;
        // Skip frames as a camera would; use repair frames only.
        for n in [5, 9, 13] {
            let (img, stride) = photograph(&a.frame(n).unwrap(), 4, 720, 480);
            match s.scan_luma(&img, 720, 480, stride) {
                ScanEvent::Progress { needed, .. } => assert_eq!(needed, 3),
                ScanEvent::Complete { kind, object } => result = Some((kind, object)),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(result, Some((ObjectKind::Bundle, obj)));
    }

    #[test]
    fn large_images_are_downscaled_and_still_decode() {
        let a = Animation::new(ObjectKind::Card, &sample(300)).unwrap();
        let (img, stride) = photograph(&a.frame(0).unwrap(), 10, 1920, 1080);
        let mut s = Scanner::new();
        assert!(matches!(
            s.scan_luma(&img, 1920, 1080, stride),
            ScanEvent::Complete {
                kind: ObjectKind::Card,
                ..
            }
        ));
    }

    #[test]
    fn text_qr_codes_pass_through() {
        let m = render_text("memcard1:abc").unwrap();
        let (img, stride) = photograph(&m, 6, 640, 480);
        assert_eq!(
            Scanner::new().scan_luma(&img, 640, 480, stride),
            ScanEvent::Text("memcard1:abc".into())
        );
    }

    #[test]
    fn hostile_dimensions_are_ignored() {
        let mut s = Scanner::new();
        let img = vec![0u8; 100];
        assert_eq!(s.scan_luma(&img, 0, 10, 10), ScanEvent::Nothing);
        assert_eq!(s.scan_luma(&img, 20, 20, 20), ScanEvent::Nothing); // too short
        assert_eq!(s.scan_luma(&img, 10, 10, 5), ScanEvent::Nothing); // stride < width
        assert_eq!(
            s.scan_luma(&img, u32::MAX, u32::MAX, u32::MAX),
            ScanEvent::Nothing
        );
        assert_eq!(s.scan_luma(&img, 10, 10, 10), ScanEvent::Nothing); // no QR
    }
}
