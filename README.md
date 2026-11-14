<p align="center"><img src="assets/brand/logo.svg" width="166" height="166" alt="MEM logo"></p>

## MEM — Mesh Messenger

End-to-end encrypted messages between people who have approved each other, over **any** medium
that is available: an animated QR code on a screen, the local Wi-Fi network, another messaging
app, and later radio, couriers and our own hub.

> **Delivery is our job; the risk choice is yours.** MEM prefers the most private channel that
> works. If only a less private one is available, MEM tells you exactly what it would expose and
> sends only with your consent.


### Try it

1. Install the APK on two Android phones, or run the Linux build on a desktop.
2. **Add contact**: one phone shows *My code*, the other scans it under *Scan or paste*. Then
   swap. Compare the six emoji on both screens.
3. Open the chat and type. If your contact is on the same Wi-Fi (a green Wi-Fi mark next to
   their name), the message goes straight away (P1). Otherwise choose how it travels:
   - **QR code on screen (P0)**: an animated code; the other phone opens *Receive* and points
     the camera at it until it completes.
   - **Wait for Wi-Fi**: it is sent automatically the next time you are both on the same network.
   - **Sharing through another app (P4)**: MEM first shows what that app will learn. Then you
     send the `mem0:…` text through any messenger, and your contact pastes it into *Receive*.
4. *Channels* (Wi-Fi icon) shows what each channel exposes and turns Wi-Fi delivery off.
   *Privacy log* lists every message that went below a contact's privacy floor.

### Build

Requirements: Rust (pinned by `rust-toolchain.toml`), Flutter 3.47+, and for Android the SDK
and NDK.

```sh
cargo test --workspace                 # core, crypto, protocol and invariant tests
cd app
flutter test                           # widget tests
flutter test integration_test -d linux # end-to-end test of the real app (needs a display;
                                       # MEM_IT_SKIP_LAN=1 skips the part needing multicast)
flutter build linux --release
flutter build apk --release --split-per-abi
```

After changing `crates/mem-ffi/src/api`, regenerate the bindings with
`flutter_rust_bridge_codegen generate` (version 2.13.0) in `app/`.

### Repository

- `crates/`: the Rust core (sans-IO protocol, crypto, runtime, FFI).
- `app/`: the Flutter application.
- `spec/`: CDDL schemas and test vectors.
- `hub/`: the store-and-forward hub (Elixir).
- `tests/invariants/`: cross-crate tests of the security invariants.

**Status: experimental.** See
[CHANGELOG.md](CHANGELOG.md) for what works and the known gaps. No external security audit yet.


### License

MEM is source-available, not open source. See [LICENSE.md](LICENSE.md).

- The protocol specification in `spec/` is under CC BY 4.0: implement it freely.
- Everything else is under the PolyForm Strict License 1.0.0 with an additional permission for
  security research: you may read, build, run and audit it for noncommercial purposes, but not
  redistribute it or build other products on it without written permission.

To report a vulnerability, see [SECURITY.md](SECURITY.md). To contribute, see
[CONTRIBUTING.md](CONTRIBUTING.md).
