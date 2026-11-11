# Vendored ggwave

- Upstream: https://github.com/ggerganov/ggwave
- Tag `ggwave-v0.4.3`, commit `a38e38b7373f9adf45baf737a104206664b225a1` (2026-03-21).
- Files copied unchanged: `LICENSE` (MIT, Georgi Gerganov), `include/ggwave/ggwave.h`,
  `src/ggwave.cpp`, `src/fft.h`, `src/reed-solomon/{gf,poly,rs}.hpp` and its `LICENSE` (MIT,
  Mike Lubinets).
- Used only through the C API in `src/ffi.rs` (ADR-0015). Update by copying a newer tag the same
  way and recording it here.
463a87b0f9e9c23ccf82e4cae3ea50fc4653ed63fca5e3e148f6513cf03d38ae  LICENSE
ed03a28e7299a2d686bf28d747b177a1ead059e0563e11807ee6e96fa9e3dda0  include/ggwave/ggwave.h
97404255fe603511e7123efa1bf41b25fd7bdc80e837ac4742a3170fab287a0f  src/ggwave.cpp
1ec89c52f15a9bb25badbb6dde4612d5aedb445d946a7f571c4363523903de55  src/fft.h
4b0f806c4bda5dfee0a01343b664e83d2064a198c1c7cc69b3ae36d8dfba3055  src/reed-solomon/gf.hpp
5bf6ddd498c8be8f49960f331c883449ce7bbd8a40bb9c969fc6e0c441cc731b  src/reed-solomon/LICENSE
9c8fa3b3099144009002a8543a11d756e26d633ac005f1e07ce0e5eff900f020  src/reed-solomon/poly.hpp
b1e42030ed26981c2074152a839d2fa3006af97925f687052e8497e02497c2b7  src/reed-solomon/rs.hpp
