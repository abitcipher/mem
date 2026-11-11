# MEM protocol specification

Normative CDDL (RFC 8610) for everything that crosses a channel. These files are the
machine-checkable form of the wire format, and `mem-proto` must match them.

| File | Contents |
|---|---|
| `envelope.cddl` | Outer bundle envelope (visible to carriers) |
| `inner.cddl` | Decrypted inner payload |
| `payload.cddl` | HPKE payload wrapper |
| `card.cddl` | Pairing card |
| `intro.cddl` | Contact request content |
| `group.cddl` | Group content types inside the encrypted payload |
| `device.cddl` | Device certificates, device lists and device setup |
| `encounter.cddl` | Encounter protocol messages |
| `frame.cddl` | Fountain-coded transfer frame (animated QR, sound); object kinds bundle, card, backup, device setup, connect hint |
| `hint.cddl` | Connect hint |
| `hub.cddl` | Hub protocol messages |
| `hublist.cddl` | Operator-signed hub list |
| `attest.cddl` | In-person attestation for a relying party: request, offer, statement, attestation |
| `backup.cddl` | Encrypted backup file and its contents |
| `vectors/envelope/*.cbor` | Envelope test vectors; checked by `mem-proto/tests/vectors.rs` and validated against `envelope.cddl` in CI |
| `vectors/frame/*.cbor`, `vectors/encounter/*.cbor` | Frame and encounter vectors, validated against their CDDL in CI |
| `vectors/backup/backup.cbor`, `backup.txt` | A backup of a fixed test identity with default Argon2id parameters, and its phrase in both forms; restored by `tests/invariants/tests/backup_restore.rs`, validated against `backup.cddl` in CI. Test-only secret. |
| `vectors/backup/backup-0.2.0-ft8.cbor`, `.txt` | The same identity backed up by builds up to 0.2.0-ft.8, before the default-floor setting; proves older backups still restore. Test-only secret. |
| `vectors/attest/*` | Two fixed identities attesting at `https://basa.ua`: the objects as CBOR and `attest.json` (keys, emoji, text forms); checked by `mem-core` |

License: the schemas and vectors in this directory are under CC BY 4.0 (`LICENSE`).

Encoding rules: deterministic CBOR, integer map keys, definite lengths only, max nesting
depth 8.
