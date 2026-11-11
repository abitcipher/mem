# Changelog

All notable changes to MEM. Versions follow [SemVer](https://semver.org/); before 1.0 the wire
format may change between minor versions.

## [Unreleased]

### Added — encrypted backup and recovery phrase

- **Recovery phrase:** turning on backups shows a generated phrase, either 12 English words (BIP-39)
  or 9 Latin syllables (proquints, language-neutral; the default for the Ukrainian UI).
  - You confirm it by typing 3 of the words.
  - Either form opens every backup, because keys come from the 128-bit secret, not the words.
  - A single mistyped syllable letter is corrected when only one fix fits.
  - The phrase screens block screenshots and screen recording (`FLAG_SECURE`), checked on the
    emulator.
- **Backup:** Argon2id (64 MiB, 3 passes) then XChaCha20-Poly1305 over the padded state, with or
  without message history.
  - It can be saved as a `.membackup` file through the system picker (no storage permission),
    sent through another app (with a disclosure), or shown as an animated QR to a new phone
    (64 KiB, "without history" if larger).
  - MEM keeps no server copy.
- **Restore:** from a QR scan, a file or pasted text. It is a move: same identity, contacts and
  keys.
  - Restoring over an identity that has contacts asks first, and keeps the old state encrypted as
    `replaced-<time>.bin`; nothing is deleted.
- **Automatic backups:** off by default, every N days (1–90) or after each new contact, into a
  folder you pick once. Cloud-synced folders are allowed, with a note about what the provider sees.
- **Fixed:** the runtime now persists state changes made by clock ticks (expiry, acks).
- **Engineering:**
  - crates `argon2` (MIT/Apache-2.0) and `bip39` (CC0-1.0);
  - `spec/backup.cddl`, a published vector, and a CI validation step;
  - fuzz target `backup_decode`;
  - invariant tests for restore equivalence, both phrase forms, wrong phrases and hostile KDF
    parameters;
  - a test that only two FFI functions carry phrase words;
  - the Linux end-to-end test turns backups on through the UI and restores a fresh profile.

### Added — delivery confirmations

- **No more silent message loss.** A message is `Delivered` (two ticks) only when the recipient's
  signed, encrypted ack arrives, sent after the message is stored.
  - Until then it stays in the outbox and is resent at every Wi-Fi encounter, and every 30 s
    within one (at most 3 sends per encounter).
  - Over QR and share, the chat says "Not confirmed yet — tap to send again".
  - Fixes a message written into a dead Wi-Fi connection being marked Sent and lost.
- **How acks travel:** piggybacked in the next message (inner field 10, any channel), or as a
  standalone ack bundle (kind 1, content type 2) over Wi-Fi within the floor.
  - A retransmitted copy is still refused by its tombstone, but acknowledged again from a
    bounded delivered index.
- **Faster failure detection on Wi-Fi:** QUIC keep-alive 3 s, idle timeout 10 s. Sending prefers the
  newest encounter with a contact.
- **Retry period:** the message lifetime defaults to 30 days, and is a setting in Profile within
  1–30 days.
- **Wire format:** inner field 10 and content type 2 are new. Builds before this change cannot
  read messages that carry them; update all devices together.
- **Engineering:**
  - invariant tests `tests/invariants/tests/delivery_acks.rs`;
  - fuzz target `inner_decode`;
  - the runtime ticks every second while a peer is connected.

### Added — animated QR and Wi-Fi

- **Animated QR (P0)**:
  - Messages of any size up to the 64 KiB bundle cap travel as a loop of fountain-coded QR frames
    (RaptorQ, RFC 6330), 8 frames/s, QR version 15.
  - The receiver's camera can miss frames and catch them in any order; any K frames complete the
    transfer. A typical message needs 1–2 frames, and the maximum is about 21 s.
  - Frame format: `spec/frame.cddl`.
- **Scanning in Rust**:
  - The camera feeds its luma plane to `rxing` in the core.
  - This replaces `mobile_scanner`, which uses Google ML Kit and reports usage to Google, contrary
    to the P0 exposure profile.
  - Pairing-card QR codes (`memcard1:`) still work.
- **Wi-Fi delivery (Iroh, P1)**:
  - Phones on the same local network find each other by mDNS and connect directly over QUIC.
  - There are no relays, no n0 discovery and no port mapping, and the transport key is random on
    every start.
  - Queued messages go out by themselves when the contact comes into range, but only when the
    channel is within the contact's privacy floor (or a standing rule allows it). Automatic sends
    never ask and never go below the floor.
  - A message typed while the contact is nearby is sent immediately.
- **Encounter protocol, subset**: `hello`, `tags`, `bundles` and `bye`.
  - Only today's receiving tags are advertised, padded with random decoys and sorted, so tags of
    different days stay unlinkable.
  - The per-encounter quota is 256 bundles.
- **App**:
  - A Wi-Fi status icon, and a "nearby" mark on contacts and in the chat header.
  - A **Channels** screen showing each channel's exposure profile, with a switch for Wi-Fi delivery.
  - "Wait for Wi-Fi" in the channel picker.
  - Live updates when messages arrive over the network.
  - `Wi-Fi · P1` provenance badges.
  - en/uk strings.
- **Android permissions**: network state, Wi-Fi state, multicast (with a foreground-only multicast
  lock) and camera.
- **Engineering**:
  - New crates: `mem-fountain`, `cla-qr`, `cla-iroh`.
  - `mem-runtime::Node`: Tokio, the Iroh channel, a clock and an event stream.
  - New fuzz targets: `frame_decode`, `encounter_decode`, `fountain_assemble` and `qr_luma`.
  - Vectors for frames and encounter messages, validated against CDDL in CI.
  - `deny.toml` gains narrow licence exceptions for Iroh's transitive dependencies:
    CDLA-Permissive-2.0 (CA root data) and Unlicense (wasm-only crates). The `paste` "unmaintained"
    advisory is ignored; it is a build-time macro with no vulnerability.

### Changed — wire format (not compatible with 0.1.0 bundles)

- Every bundle now carries a `purge_commitment` (envelope field 10, bound into the HPKE AAD). The
  encrypted inner payload carries the matching `purge_token` (field 9, covered by the signature).
- The receiver verifies that the token matches the commitment; a mismatch means nothing is
  delivered. After delivery it keeps a tombstone (30 days, quota of 32,768). Later copies of the
  same bundle are refused before decryption. Tombstones survive restarts.
- Bundles made by 0.1.0 are rejected as malformed. Pairing cards are unchanged, so contacts stay
  paired after the update. Messages still queued in a 0.1.0 outbox are dropped on upgrade.
- Snapshots from 0.1.0 still load. The purge key is created on the first send.

## [0.1.0] — 2026-10-15 — experimental

First usable build. **Experimental: do not rely on it for high-risk communication.** No
external security review has been done.

### What works

- **Identity**: Ed25519 identity, X25519 mailbox key, base32 fingerprint. Created on first launch.
- **Pairing**: exchange signed contact cards by scanning a QR code (Android) or
  pasting text (any platform). Both devices show the same 6-emoji short authentication string
  to compare. Contacts record whether you met in person or paired remotely.
- **End-to-end encrypted 1:1 text messages**: sign-then-encrypt with the recipient's key in the
  signed data. HPKE (RFC 9180, X25519 / HKDF-SHA256 / ChaCha20-Poly1305), with the envelope
  bound as associated data. Padding buckets hide message length. Daily-rotating blinded
  rendezvous tags. The envelope carries no identities or timestamps.
- **Two channels**:
  - **QR code on screen (P0)**: the receiver scans it. Needs no network and no other app.
  - **Share as text (P4)**: an armored `mem1:` string sent through any app (Signal, WhatsApp,
    email…) and pasted into MEM → Receive.
- **Consent before downgrade**: sending below the contact's privacy floor
  (default P2) shows a disclosure generated from the channel's exposure profile. That covers who
  observes, what they learn, and what stays private. You can allow once, or create a standing
  permission for the contact (with an extra confirmation). Every below-floor send goes into the
  **Privacy log**. Permissions are revocable.
- **Honest provenance**: each message shows which channel carried it and its grade. Text
  you paste in is shown as P4, because MEM cannot know how it reached you.
- **Deny by default**: only bundles that decrypt and verify as coming from an approved
  contact become messages.
- **Local storage**: one snapshot, encrypted with XChaCha20-Poly1305 and written atomically.
  Android backup and device transfer are disabled.
- **Languages**: English and Ukrainian.
- **Platforms**: Android (arm64, armv7, x86_64) and Linux desktop. Linux has no camera scanning;
  paste instead.

### Known gaps

| Gap | Impact |
| --- | --- |
| One mailbox key, never rotated | A key compromise exposes all past messages (no forward-secrecy window) |
| Storage key in an app-private file, not Android Keystore | A rooted device or forensic access to app data can read the history |
| Pairing nonce is long-lived | Re-pairing yields the same secret unless you tap "New pairing code" |
| No delivery acknowledgements | "Sent" means handed to a channel, not received |
| Single-frame QR only (≈ 820-byte bundle) | Long messages need the text channel |
| No network, courier, hub, BLE or Iroh delivery | Every message needs a user action |
| No backup/restore, single device | Losing the device loses the identity and history |
| Wire format was experimental | v0.1.0 bundles are not accepted by later versions |
| Android release is signed with the debug key | Not suitable for store distribution; Android won't accept it as an update to a differently signed build |

### Engineering

- Rust workspace: `mem-proto` (bounded, canonical-only CBOR decoders, fuzzed), `mem-crypto`,
  `mem-cla`, `mem-core` (sans-IO agent), `mem-runtime`, `mem-ffi` (flutter_rust_bridge 2.13).
- 71 Rust tests, including the cross-crate invariant tests. Plus a Flutter integration test
  driving the real app end to end.
