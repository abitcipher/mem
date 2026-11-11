# Security

MEM has had no external security audit yet. Do not rely on it for high-risk communication.

## Reporting a vulnerability

Please report privately before publishing, and give us a reasonable time to fix the problem.
Use the private vulnerability reporting of the hosting platform this repository is on.

A useful report says which version or commit is affected, what an attacker needs (a courier, a
hub, a nearby device, a contact), what they learn or can do, and how to reproduce it.

## Auditing

The [license](LICENSE.md) lets you build, run, change privately and instrument the software for
security research, and publish what you find. Good starting points:

- `spec/`: the wire format as CDDL, with test vectors;
- `crates/mem-proto`, `crates/mem-crypto`, `crates/mem-core`: decoders, cryptography and the
  protocol agent, with no I/O;
- `tests/invariants/`: the security properties as tests;
- `fuzz/`: fuzz targets for every decoder.
