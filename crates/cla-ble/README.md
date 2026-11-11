# cla-ble — Bluetooth LE

Rust side of the BLE channel. The radio itself is the `mem_radio` plugin (`app/plugins/mem_radio`,
Kotlin); this crate reassembles the byte stream and maps links to peer handles.

- **Grade:** P0 (`ClaId::Ble`).
- **Exposure:** nearby people with a BLE scanner learn that a MEM device is here (the fixed
  service UUID), when it exchanges something, and that two devices were close. No identity: the
  OS randomises the address and the 4-byte link nonce changes at least every 15 minutes. Content
  is end-to-end encrypted; sizes are padded.
- **MTU:** a stream, not packets: L2CAP CoC on Android 10+, otherwise GATT chunks of
  `ATT MTU − 3` bytes (20–512). Bundles up to 64 KiB.
- **Throughput:** not measured yet (needs two phones).
- **Platforms:** Android 7+ (Android 6–11 need the location permission and Location on to scan).
  iOS later.
- **Background:** only while "Stay reachable in the background" is on, at low power.
- **Battery:** continuous scanning and advertising while on; low-power modes in the background.
- **Per encounter:** at most 64 courier bundles / 1 MiB.
