//! Test peer (not shipped): a headless MEM node on the dev hub, driven by a command file.
//!
//! `cargo run -p mem_ffi --example hubpeer -- <state-dir> <hub-url> [name]`; append commands to
//! `<state-dir>/cmd`, one per line:
//! - `card` prints this node's card text
//! - `request <raw-screencap> <note>` reads a `memcard1:` QR from an `adb exec-out screencap`
//!   (raw RGBA) file and sends that card a contact request
//! - `contacts`, `msgs <id-prefix>`, `send <id-prefix> <text>`

#![allow(clippy::unwrap_used, clippy::print_stdout, reason = "test tool")]

use std::time::Duration;

use cla_qr::{Payload, decode_luma};
use mem_ffi::api::{MemNode, open_node};

fn find(node: &MemNode, prefix: &str) -> Option<String> {
    node.contacts()
        .ok()?
        .into_iter()
        .map(|c| c.id)
        .find(|id| id.starts_with(prefix))
}

/// The first QR text in a raw screencap: 12- or 16-byte header (width, height, format[, space]),
/// then RGBA rows.
fn qr_text(path: &str) -> Option<String> {
    let raw = std::fs::read(path).ok()?;
    let w = u32::from_le_bytes(raw[0..4].try_into().ok()?);
    let h = u32::from_le_bytes(raw[4..8].try_into().ok()?);
    let pixels = (w * h * 4) as usize;
    let rgba = &raw[raw.len() - pixels..];
    let luma: Vec<u8> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000) as u8
        })
        .collect();
    // The decoder reads a centred square (it is built for camera frames): slide a square window
    // down the screenshot so a code anywhere on it is centred in one of them.
    let side = w.min(h);
    let rows = (side * w) as usize;
    (0..=h - side)
        .step_by((side / 4).max(1) as usize)
        .find_map(|top| {
            let start = (top * w) as usize;
            match decode_luma(&luma[start..start + rows], w, side, w)? {
                Payload::Text(t) => Some(t),
                Payload::Bytes(b) => String::from_utf8(b).ok(),
            }
        })
}

fn run(node: &MemNode, line: &str) {
    let mut parts = line.splitn(3, ' ');
    let cmd = parts.next().unwrap_or_default();
    let a = parts.next().unwrap_or_default();
    let b = parts.next().unwrap_or_default();
    match cmd {
        "card" => println!(
            "CARD {}",
            node.own_card().map(|c| c.text).unwrap_or_default()
        ),
        "request" => match qr_text(a) {
            Some(card) => println!(
                "REQUEST {:?}",
                node.send_contact_request(card, Some(b.into()))
            ),
            None => println!("REQUEST no QR found in {a}"),
        },
        "contacts" => {
            for c in node.contacts().unwrap_or_default() {
                println!(
                    "CONTACT {} {:?} requested={}",
                    c.id, c.display_name, c.requested
                );
            }
        }
        "msgs" => match find(node, a) {
            Some(id) => {
                for m in node.messages(id).unwrap_or_default() {
                    println!(
                        "MSG out={} {:?} [{}] from={} channels={:?}",
                        m.outgoing,
                        m.status,
                        m.text,
                        &m.sender_id[..m.sender_id.len().min(8)],
                        m.channels
                            .iter()
                            .map(|c| c.grade.as_str())
                            .collect::<Vec<_>>()
                    );
                }
            }
            None => println!("MSGS no contact {a}"),
        },
        "send" => match find(node, a) {
            Some(id) => println!(
                "SEND {:?}",
                node.send_text(id, b.into()).map(|s| s.message_id)
            ),
            None => println!("SEND no contact {a}"),
        },
        "" => {}
        other => println!("unknown command {other}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("state dir");
    let hub = args.next().expect("hub url");
    let name = args.next().unwrap_or_else(|| "Hub peer".into());
    let node = open_node(dir.clone()).unwrap();
    node.set_display_name(Some(name)).unwrap();
    node.set_lan_enabled(false).unwrap();
    node.set_hub_url(Some(hub)).unwrap();
    node.set_hub_enabled(true).unwrap();
    println!(
        "READY {}",
        node.profile().map(|p| p.fingerprint).unwrap_or_default()
    );
    let cmd = std::path::Path::new(&dir).join("cmd");
    loop {
        if let Ok(text) = std::fs::read_to_string(&cmd)
            && !text.is_empty()
        {
            std::fs::write(&cmd, "").unwrap();
            for line in text.lines() {
                println!("> {line}");
                run(&node, line.trim());
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
