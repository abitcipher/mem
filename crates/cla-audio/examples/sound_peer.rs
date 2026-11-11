//! Test helper: a laptop that pairs by sound the way the app does
//! (`app/lib/screens/sound_pair.dart`). It makes a throwaway identity, plays its card frame by
//! frame through the speaker after a random 0.8–3 s pause and while nobody else is heard
//! playing, and listens all the time. It stops one round after it has the other card, or after
//! three rounds without it, and prints whether the card it heard verifies.
//!
//! `cargo run --release -p cla-audio --example sound_peer -- "Laptop test" audible|ultra`
//!
//! Needs ALSA's `arecord` and `aplay`. With `SOUND_PEER_RAW=<file>` it also keeps everything the
//! microphone heard (raw 16-bit 48 kHz mono). A phone that pairs with it adds a contact with the
//! given name: remove it afterwards.

#![allow(clippy::print_stdout, reason = "command-line tool")]

use std::io::{Read, Write};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cla_audio::{Heard, Profile, SAMPLE_RATE, SoundPairing};
use mem_core::{Agent, Input};
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};

/// The app's limit: at most this many rounds of our card without hearing the other one.
const MAX_ROUNDS: u32 = 3;

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            println!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().collect();
    let name = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "Laptop sound test".into());
    let profile = match args.get(2).map(String::as_str) {
        Some("ultra") => Profile::Ultrasound,
        _ => Profile::Audible,
    };
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("no randomness: {e}"))?;
    let mut rng = ChaCha20Rng::from_seed(seed);
    let (agent, card) = throwaway(&name, &mut rng)?;
    let mut pairing = SoundPairing::new(&card, profile).map_err(|e| e.to_string())?;
    let round = pairing.frames_per_round();
    let start = Instant::now();
    let log = |m: &str| println!("[{:5.1} s] {m}", start.elapsed().as_secs_f32());
    log(&format!(
        "{name}: card {} B, {round} frames per round, {profile:?}",
        card.len()
    ));

    let rate = SAMPLE_RATE.to_string();
    let pcm_args = ["-q", "-t", "raw", "-f", "S16_LE", "-r", &rate, "-c", "1"];
    let (mut rec, rx) = microphone(&pcm_args)?;

    let mut got: Option<u32> = None;
    let mut bursts = 0u32;
    let mut playing: Option<Child> = None;
    let mut next_at = Instant::now() + pause(&mut rng);
    let mut progress = (0, 0);
    let mut finish_at: Option<Instant> = None;
    loop {
        while let Ok(chunk) = rx.try_recv() {
            if got.is_some() {
                continue;
            }
            match pairing.listen(&chunk) {
                Heard::Nothing => {}
                Heard::Progress { received, needed } => {
                    if (received, needed) != progress {
                        progress = (received, needed);
                        log(&format!("heard {received} of {needed}"));
                    }
                }
                Heard::Complete(other) => {
                    got = Some(bursts);
                    match agent.card_preview(&other) {
                        Ok(p) => log(&format!(
                            "got a valid card: name {:?}, fingerprint {}",
                            p.display_name, p.fingerprint
                        )),
                        Err(e) => log(&format!("got a card that does not verify: {e:?}")),
                    }
                }
            }
        }
        if let Some(child) = playing.as_mut()
            && child.try_wait().ok().flatten().is_some()
        {
            playing = None;
            bursts += 1;
            next_at = Instant::now() + pause(&mut rng);
        }
        if let Some(at) = finish_at {
            if Instant::now() >= at {
                break;
            }
        } else if playing.is_none() {
            let done = match got {
                Some(at) => bursts - at >= round,
                None => bursts >= MAX_ROUNDS * round,
            };
            if done {
                log(if got.is_some() {
                    "played one more round; finishing"
                } else {
                    "three rounds without the other card: stopped"
                });
                // Give a transmission that started during our last frame time to arrive.
                finish_at = Some(Instant::now() + Duration::from_secs(8));
            } else if pairing.busy() {
                next_at = next_at.max(Instant::now() + Duration::from_millis(300));
            } else if Instant::now() >= next_at {
                let pcm = pairing.next_burst().map_err(|e| e.to_string())?;
                log(&format!("playing frame {}", bursts + 1));
                playing = Some(play(&pcm_args, &pcm)?);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = rec.kill();
    log(if got.is_some() {
        "result: paired"
    } else {
        "result: no card"
    });
    Ok(got.is_some())
}

/// A new identity named `name`, and its signed card.
fn throwaway(name: &str, rng: &mut ChaCha20Rng) -> Result<(Agent, Vec<u8>), String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let mut agent = Agent::new(u64::try_from(now_ms).map_err(|e| e.to_string())?, rng);
    agent.handle(Input::SetDisplayName(Some(name.to_owned())), rng);
    let card = agent.own_card().map_err(|e| format!("card: {e:?}"))?;
    Ok((agent, card))
}

/// Starts `arecord` and a thread that sends its samples in 100 ms chunks.
fn microphone(pcm_args: &[&str]) -> Result<(Child, mpsc::Receiver<Vec<i16>>), String> {
    let mut rec = Command::new("arecord")
        .args(pcm_args)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("arecord: {e}"))?;
    let mut mic = rec.stdout.take().ok_or("arecord has no output")?;
    let mut raw = match std::env::var("SOUND_PEER_RAW") {
        Ok(path) => Some(std::fs::File::create(path).map_err(|e| e.to_string())?),
        Err(_) => None,
    };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = vec![0u8; 9600];
        while mic.read_exact(&mut buf).is_ok() {
            if let Some(f) = raw.as_mut() {
                let _ = f.write_all(&buf);
            }
            let samples: Vec<i16> = buf
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect();
            if tx.send(samples).is_err() {
                break;
            }
        }
    });
    Ok((rec, rx))
}

/// Plays `pcm` once through `aplay`; the returned child exits when it is done.
fn play(pcm_args: &[&str], pcm: &[i16]) -> Result<Child, String> {
    let mut child = Command::new("aplay")
        .args(pcm_args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("aplay: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("aplay has no input")?;
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    thread::spawn(move || stdin.write_all(&bytes));
    Ok(child)
}

/// The app's random wait before each frame: 0.8–3 s.
fn pause(rng: &mut ChaCha20Rng) -> Duration {
    Duration::from_millis(800 + u64::from(rng.next_u32() % 2200))
}
