# MEM license

Required Notice: Copyright 2026 aBitCipher

MEM is source-available software. It is not open source.

| Part | License |
|---|---|
| `spec/` (protocol schemas and test vectors) | [Creative Commons Attribution 4.0 International](spec/LICENSE) |
| `crates/cla-audio/vendor/ggwave/` | Its own license (MIT), see `crates/cla-audio/vendor/ggwave/LICENSE` |
| Portions of `app/packages/mem_netmap/` adapted from graph_flow (each such file says so in its first lines) | Their own license (MIT), see `app/packages/mem_netmap/THIRD_PARTY_NOTICES.md` |
| Everything else | [PolyForm Strict License 1.0.0](LICENSE-PolyForm-Strict-1.0.0.md), with the additional permission for security research below |

In short, for everything under the PolyForm Strict License: you may read, build and run the
software for noncommercial purposes. You may not distribute it, and you may not make changes or
new works based on it, except as the additional permission allows. The license text, not this
summary, is what counts.

## Additional permission for security research

The licensor grants this permission in addition to the PolyForm Strict License 1.0.0. It does not
change that license's text and does not limit any right that license already gives you.

1. **Private changes.** To find, confirm or demonstrate security or privacy problems in the
   software, you may make changes to it and new works based on it (patches, test harnesses, fuzz
   targets, instrumented builds), and build and run them, for any purpose including paid audit
   work. You may not distribute the changed software or run it as a service for others.
2. **Reports.** You may publish your findings, and include in them the excerpts of the software
   and the patches or proofs of concept needed to explain them.
3. **Comparing builds.** You may build the software to compare your result with a published
   release.
4. **No trademark rights.** This permission gives no right to the MEM name or logo.

Please report vulnerabilities privately first, as [SECURITY.md](SECURITY.md) describes.

## Other uses

Anything else, including redistribution, modified versions, commercial use and running a hub
commercially, needs written permission from the licensor. Ask before you build on it.
