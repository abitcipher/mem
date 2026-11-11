//! Sound channel: pairing cards as fountain frames over ggwave.
//!
//! Sound never carries bundles. Each phone plays its own card as a loop of small fountain frames
//! (`spec/frame.cddl`, object kind 1, symbol size 64 bytes, at most [`MAX_FRAME_LEN`] bytes per
//! ggwave transmission) and listens for the other's. Any `K` distinct frames rebuild the card, so
//! lost or garbled transmissions only cost time. Grade P0: everyone within earshot hears that
//! phones exchanged something and can record the public card, as with a pairing QR code.
//!
//! Audio is 16-bit mono PCM at [`SAMPLE_RATE`]. The platform plays and records it; this crate only
//! turns frames into samples and back.

#![allow(
    unsafe_code,
    reason = "FFI to the vendored ggwave C API; every call goes through `with_ggwave` under one lock"
)]

mod ffi;

use std::ffi::{c_int, c_void};
use std::sync::{Mutex, Once, PoisonError};

use mem_fountain::{Assembler, Progress, Sender};
use mem_proto::frame::{Frame, ObjectKind, TransferId};

/// Sample rate of all PCM in and out, Hz.
pub const SAMPLE_RATE: u32 = 48_000;
/// Fountain symbol size for cards.
pub const SYMBOL_LEN: u16 = 64;
/// ggwave's largest variable-length payload; every frame fits.
pub const MAX_FRAME_LEN: usize = 140;
/// Playback volume passed to ggwave (0–100; ggwave advises at most 50).
const VOLUME: c_int = 50;
/// Samples handed to ggwave per decode call (its default analysis frame).
const DECODE_CHUNK: usize = 1024;
/// The same in bytes, as ggwave takes it.
const DECODE_CHUNK_BYTES: c_int = 2048;
/// ggwave's largest decoded payload (`GGWave::kMaxDataSize`).
const MAX_DECODED: usize = 256;
const MAX_DECODED_C: c_int = 256;

/// Which sound to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Audible tones (ggwave "audible fast", about 5.8 s per frame): works on every phone.
    Audible,
    /// Near-ultrasound (ggwave "ultrasound fast"): quieter to people, but some phone speakers
    /// and microphones cut these frequencies.
    Ultrasound,
}

impl Profile {
    const fn protocol(self) -> c_int {
        match self {
            Self::Audible => ffi::PROTOCOL_AUDIBLE_FAST,
            Self::Ultrasound => ffi::PROTOCOL_ULTRASOUND_FAST,
        }
    }
}

/// Sound channel failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
    /// ggwave could not start (all its instances in use) or failed to encode.
    #[error("the sound encoder failed")]
    Ggwave,
    /// The card is too large or empty for fountain frames.
    #[error("the contact code cannot be framed for sound")]
    Framing,
}

static GGWAVE: Mutex<()> = Mutex::new(());
static SETUP: Once = Once::new();

/// Runs `f` while holding the one ggwave lock: its C API keeps instances in a global table.
fn with_ggwave<T>(f: impl FnOnce() -> T) -> T {
    let _guard = GGWAVE.lock().unwrap_or_else(PoisonError::into_inner);
    SETUP.call_once(|| {
        // SAFETY: global setup under the lock, before any instance exists: silence ggwave's
        // logging and decode only the two protocols MEM sends (fewer false detections). Never
        // enable the unconfigured "custom" protocols: ggwave divides by zero on them.
        unsafe {
            ffi::ggwave_setLogFile(std::ptr::null_mut());
            for p in 0..ffi::PROTOCOL_COUNT {
                let on = c_int::from(
                    p == ffi::PROTOCOL_AUDIBLE_FAST || p == ffi::PROTOCOL_ULTRASOUND_FAST,
                );
                ffi::ggwave_rxToggleProtocol(p, on);
            }
        }
    });
    f()
}

fn parameters(mode: c_int) -> ffi::Parameters {
    // SAFETY: returns a plain struct by value.
    let mut p = unsafe { ffi::ggwave_getDefaultParameters() };
    #[allow(clippy::cast_precision_loss, reason = "48 000 is exact in f32")]
    let rate = SAMPLE_RATE as f32;
    p.payload_length = -1; // variable length, with sound markers
    p.sample_rate_inp = rate;
    p.sample_rate_out = rate;
    p.sample_rate = rate;
    p.sample_format_inp = ffi::SAMPLE_FORMAT_I16;
    p.sample_format_out = ffi::SAMPLE_FORMAT_I16;
    p.operating_mode = mode;
    p
}

/// Encodes one payload (at most [`MAX_FRAME_LEN`] bytes) as PCM.
///
/// # Errors
///
/// [`AudioError::Ggwave`] if ggwave cannot start or encode.
pub fn encode(payload: &[u8], profile: Profile) -> Result<Vec<i16>, AudioError> {
    if payload.is_empty() || payload.len() > MAX_FRAME_LEN {
        return Err(AudioError::Framing);
    }
    let len = c_int::try_from(payload.len()).map_err(|_| AudioError::Framing)?;
    with_ggwave(|| {
        // SAFETY: under the lock; the instance is freed before returning.
        let instance = unsafe { ffi::ggwave_init(parameters(ffi::MODE_TX)) };
        if instance < 0 {
            return Err(AudioError::Ggwave);
        }
        let result = (|| {
            let pay = payload.as_ptr().cast::<c_void>();
            // SAFETY: query mode (2 = size in samples) writes nothing.
            let samples = unsafe {
                ffi::ggwave_encode(
                    instance,
                    pay,
                    len,
                    profile.protocol(),
                    VOLUME,
                    std::ptr::null_mut(),
                    2,
                )
            };
            let n = usize::try_from(samples).map_err(|_| AudioError::Ggwave)?;
            if n == 0 {
                return Err(AudioError::Ggwave);
            }
            let mut out = vec![0i16; n];
            // SAFETY: `out` holds exactly the queried number of i16 samples.
            let written = unsafe {
                ffi::ggwave_encode(
                    instance,
                    pay,
                    len,
                    profile.protocol(),
                    VOLUME,
                    out.as_mut_ptr().cast::<c_void>(),
                    0,
                )
            };
            if usize::try_from(written).ok() != Some(n * 2) {
                return Err(AudioError::Ggwave);
            }
            Ok(out)
        })();
        // SAFETY: frees the instance created above, still under the lock.
        unsafe { ffi::ggwave_free(instance) };
        result
    })
}

/// A ggwave receiver. Feed it PCM; it returns every payload it recognises. ggwave has at most
/// four instances in a process (receivers plus a short-lived one per [`encode`]), so keep one
/// pairing session at a time.
pub struct Decoder {
    instance: c_int,
    pending: Vec<i16>,
}

impl Decoder {
    /// Starts a receiver.
    ///
    /// # Errors
    ///
    /// [`AudioError::Ggwave`] if ggwave has no free instance.
    pub fn new() -> Result<Self, AudioError> {
        // SAFETY: under the lock.
        let instance = with_ggwave(|| unsafe { ffi::ggwave_init(parameters(ffi::MODE_RX)) });
        if instance < 0 {
            return Err(AudioError::Ggwave);
        }
        Ok(Self {
            instance,
            pending: Vec::new(),
        })
    }

    /// Feeds captured samples; returns the payloads completed by them (usually none).
    pub fn push(&mut self, samples: &[i16]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(samples);
        let mut found = Vec::new();
        let whole = self.pending.len() - self.pending.len() % DECODE_CHUNK;
        for chunk in self.pending[..whole].as_chunks::<DECODE_CHUNK>().0 {
            let mut out = [0u8; MAX_DECODED];
            // SAFETY: under the lock; `chunk` and `out` are valid for the sizes given.
            let n = with_ggwave(|| unsafe {
                ffi::ggwave_ndecode(
                    self.instance,
                    chunk.as_ptr().cast::<c_void>(),
                    DECODE_CHUNK_BYTES,
                    out.as_mut_ptr().cast::<c_void>(),
                    MAX_DECODED_C,
                )
            });
            if let Ok(n) = usize::try_from(n)
                && n > 0
                && n <= MAX_DECODED
            {
                found.push(out[..n].to_vec());
            }
        }
        self.pending.drain(..whole);
        found
    }

    /// Whether a transmission is coming in right now (someone else is playing): wait before
    /// playing our own.
    #[must_use]
    pub fn receiving(&self) -> bool {
        // SAFETY: under the lock; reads this decoder's own instance.
        with_ggwave(|| unsafe { ffi::mem_ggwave_rx_receiving(self.instance) }) == 1
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: frees this decoder's own instance, under the lock.
        with_ggwave(|| unsafe { ffi::ggwave_free(self.instance) });
    }
}

/// What listening produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// Nothing new.
    Nothing,
    /// Part of the other phone's object.
    Progress {
        /// Distinct frames collected.
        received: u32,
        /// Frames usually needed.
        needed: u32,
    },
    /// The other phone's complete object, of the session's kind (a card is verified by the core on
    /// import; a connect hint is decoded and checked by the runtime).
    Complete(Vec<u8>),
}

/// One side of an exchange by sound: plays our object, listens for the other's of the same kind.
/// Pairing exchanges cards; connecting exchanges connect hints.
pub struct SoundPairing {
    kind: ObjectKind,
    sender: Sender,
    own: TransferId,
    next: u32,
    profile: Profile,
    decoder: Decoder,
    assembler: Assembler,
}

impl SoundPairing {
    /// Prepares our `card` (encoded `spec/card.cddl`) for playing and starts listening.
    ///
    /// # Errors
    ///
    /// [`AudioError`] if the card cannot be framed or ggwave cannot start.
    pub fn new(card: &[u8], profile: Profile) -> Result<Self, AudioError> {
        Self::for_object(ObjectKind::Card, card, profile)
    }

    /// Prepares an object of `kind` (a card or a connect hint) and starts listening for the other
    /// phone's object of the same kind.
    ///
    /// # Errors
    ///
    /// [`AudioError`] if the object cannot be framed or ggwave cannot start.
    pub fn for_object(
        kind: ObjectKind,
        object: &[u8],
        profile: Profile,
    ) -> Result<Self, AudioError> {
        let sender = Sender::new(kind, object, SYMBOL_LEN).map_err(|_| AudioError::Framing)?;
        Ok(Self {
            kind,
            own: sender.transfer(),
            sender,
            next: 0,
            profile,
            decoder: Decoder::new()?,
            assembler: Assembler::new(),
        })
    }

    /// The next frame of our card as PCM, to play once. Frames go on forever (fountain code).
    ///
    /// # Errors
    ///
    /// [`AudioError::Ggwave`] if encoding fails.
    pub fn next_burst(&mut self) -> Result<Vec<i16>, AudioError> {
        let frame = self.sender.frame(self.next);
        self.next = self.next.wrapping_add(1);
        encode(&frame, self.profile)
    }

    /// Frames in one round of our card: the number a listener usually needs.
    #[must_use]
    pub const fn frames_per_round(&self) -> u32 {
        self.sender.source_symbols()
    }

    /// Whether the other phone (or anyone) is playing right now: wait before the next burst.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.decoder.receiving()
    }

    /// Feeds captured PCM. Our own frames (heard through our own microphone) and anything not of
    /// this session's kind are ignored.
    pub fn listen(&mut self, samples: &[i16]) -> Heard {
        let mut heard = Heard::Nothing;
        for payload in self.decoder.push(samples) {
            let Ok(frame) = Frame::decode(&payload) else {
                continue;
            };
            if frame.transfer() == self.own || frame.kind() != self.kind {
                continue;
            }
            match self.assembler.push(&payload) {
                Ok(Progress::Complete { kind, object }) if kind == self.kind => {
                    return Heard::Complete(object);
                }
                Ok(Progress::Partial { received, needed }) => {
                    heard = Heard::Progress { received, needed };
                }
                _ => {}
            }
        }
        heard
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    /// ggwave has four instances per process; the tests below would exceed that in parallel.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A card-sized object (the real one is about 158 bytes without a name, 225 with).
    fn card(seed: u8) -> Vec<u8> {
        (0..200u8)
            .map(|i| i.wrapping_mul(31).wrapping_add(seed))
            .collect()
    }

    #[test]
    fn every_frame_fits_one_transmission() {
        let s = Sender::new(ObjectKind::Card, &card(1), SYMBOL_LEN).unwrap();
        for n in 0..32 {
            assert!(s.frame(n).len() <= MAX_FRAME_LEN, "frame {n}");
        }
    }

    #[test]
    fn a_frame_survives_encoding_and_decoding() {
        let _serial = serial();
        for profile in [Profile::Audible, Profile::Ultrasound] {
            let payload = b"MEM sound frame 0123456789";
            let pcm = encode(payload, profile).unwrap();
            let mut d = Decoder::new().unwrap();
            let mut got = d.push(&pcm);
            got.extend(d.push(&vec![0i16; 48_000]));
            assert_eq!(got, vec![payload.to_vec()], "{profile:?}");
        }
    }

    /// Two phones in one room: each plays its frames in turn, each hears both (its own too), with
    /// noise and with some transmissions lost; both end up with the other's card.
    #[test]
    fn two_phones_exchange_cards_through_a_noisy_room() {
        let _serial = serial();
        let (ca, cb) = (card(1), card(2));
        let mut a = SoundPairing::new(&ca, Profile::Audible).unwrap();
        let mut b = SoundPairing::new(&cb, Profile::Audible).unwrap();
        let mut noise = 12_345u32;
        let mut room = |pcm: &[i16]| -> Vec<i16> {
            pcm.iter()
                .map(|&s| {
                    noise = noise.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    #[allow(clippy::cast_possible_wrap, reason = "small noise")]
                    let n = ((noise >> 16) % 1200) as i16 - 600;
                    (s / 2).saturating_add(n)
                })
                .collect()
        };
        let (mut a_got, mut b_got) = (None, None);
        for turn in 0..24 {
            let speaker_is_a = turn % 2 == 0;
            let burst = if speaker_is_a {
                a.next_burst().unwrap()
            } else {
                b.next_burst().unwrap()
            };
            // Every fifth transmission is lost entirely.
            if turn % 5 == 4 {
                continue;
            }
            let mut heard = room(&burst);
            heard.extend(room(&vec![0i16; 9_600]));
            if let Heard::Complete(c) = a.listen(&heard) {
                a_got = Some(c);
            }
            if let Heard::Complete(c) = b.listen(&heard) {
                b_got = Some(c);
            }
            if a_got.is_some() && b_got.is_some() {
                break;
            }
        }
        assert_eq!(a_got, Some(cb));
        assert_eq!(b_got, Some(ca));
    }

    /// While the other phone plays, `busy` says so; after its transmission it is quiet again.
    #[test]
    fn busy_while_someone_else_plays() {
        let _serial = serial();
        let mut a = SoundPairing::new(&card(5), Profile::Audible).unwrap();
        let mut b = SoundPairing::new(&card(6), Profile::Audible).unwrap();
        let burst = b.next_burst().unwrap();
        assert!(!a.busy());
        a.listen(&burst[..burst.len() / 2]);
        assert!(a.busy(), "mid-transmission");
        a.listen(&burst[burst.len() / 2..]);
        a.listen(&vec![0i16; 24_000]);
        assert!(!a.busy(), "after it");
    }

    #[test]
    fn a_phone_ignores_its_own_card() {
        let _serial = serial();
        let mut a = SoundPairing::new(&card(3), Profile::Audible).unwrap();
        for _ in 0..6 {
            let mut burst = a.next_burst().unwrap();
            burst.extend(vec![0i16; 9_600]);
            assert_eq!(a.listen(&burst), Heard::Nothing);
        }
    }

    /// Connecting by sound: a hint fits one frame, so one transmission each way is
    /// enough; a phone connecting ignores a card played nearby, and the other way round.
    #[test]
    fn hints_take_one_frame_and_do_not_mix_with_cards() {
        let _serial = serial();
        let (ha, hb) = (vec![0xa1; 54], vec![0xb2; 54]);
        let mut a =
            SoundPairing::for_object(ObjectKind::ConnectHint, &ha, Profile::Audible).unwrap();
        let mut b =
            SoundPairing::for_object(ObjectKind::ConnectHint, &hb, Profile::Audible).unwrap();
        assert_eq!(a.frames_per_round(), 1);
        let mut pairing = SoundPairing::new(&card(7), Profile::Audible).unwrap();
        let mut from_a = a.next_burst().unwrap();
        from_a.extend(vec![0i16; 9_600]);
        assert_eq!(b.listen(&from_a), Heard::Complete(ha));
        assert_eq!(
            pairing.listen(&from_a),
            Heard::Nothing,
            "a card session ignores hints"
        );
        let mut from_b = b.next_burst().unwrap();
        from_b.extend(vec![0i16; 9_600]);
        assert_eq!(a.listen(&from_b), Heard::Complete(hb));
        let mut card_burst = pairing.next_burst().unwrap();
        card_burst.extend(vec![0i16; 9_600]);
        assert_eq!(
            a.listen(&card_burst),
            Heard::Nothing,
            "a hint session ignores cards"
        );
    }

    #[test]
    fn a_card_frame_takes_a_few_seconds() {
        let _serial = serial();
        let s = Sender::new(ObjectKind::Card, &card(4), SYMBOL_LEN).unwrap();
        let pcm = encode(&s.frame(0), Profile::Audible).unwrap();
        #[allow(clippy::cast_precision_loss, reason = "seconds for a test message")]
        let secs = pcm.len() as f64 / f64::from(SAMPLE_RATE);
        // About 5.8 s for an 83-byte frame with "audible fast".
        assert!((4.0..8.0).contains(&secs), "{secs} s");
    }
}
