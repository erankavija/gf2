# gf2-core roadmap map

Source revision: `61c3f0a6a^:crates/gf2-core/ROADMAP.md`, cited below as `R:<line>`.
Scope: open phases under "Planned Phases" (R:196-228), the Phase 14 next steps,
the Phase 12 deferred items, "Future Directions" (R:232-246) and "Roadmap
Priorities" (R:287-296). Dispositions: tracked (issue), delivered (code),
obsolete (reason), newly filed (issue).

| Roadmap item | Citation | Disposition |
|---|---|---|
| Phase 14: benchmark polynomial utilities vs SageMath/NTL | R:163 | Delivered: `crates/gf2-core/docs/POLY_UTILITIES_PERFORMANCE.md`, `crates/gf2-core/benches/polynomial.rs`; NTL/FLINT references `73ab8eef`, `53c5a8c0` (done) |
| Phase 14: replace `poly_from_exponents()` in gf2-coding BCH | R:164 | Delivered: `crates/gf2-coding/src/bch/dvb_t2/generators.rs` builds generators from core polynomials; no `poly_from_exponents` remains outside tests |
| Phase 2: higher unrolling factors 8x/16x | R:206 | Tracked: `2037941f` REQ-02 covers unroll candidates. Factors 2 and 4 are measured with no adoption (`bc091474`, `dev/active/bc091474/pilot-outcome.md`); factors 8 and 16 are outside that portfolio; scalar kernel keeps `UNROLL = 4` (`crates/gf2-core/src/kernels/scalar/logical.rs`) |
| Phase 2: BitSlice zero-copy operations | R:207 | Tracked: `2037941f` REQ-02 covers zero-copy BitSlice use. `a1ad6d4e` (done) selects no candidate and no adoption: no measured consumer exercises BitSlice copying (`dev/active/a1ad6d4e/outcome.md`) |
| Phase 2: specialized mid-range 8-64 word kernels | R:208 | Tracked: `2037941f` (profile and optimize), `2ad3a3e0` (evidence synthesis) |
| Phase 2 plan step 1: profile real workloads (`perf record` on LDPC/BCH examples) | R:217 | Delivered: `04b85d10` (production-consumer bit-storage profile, done); logical-buffer profiles under `dev/bench_results/2037941f/logical-profile/` |
| Phase 2 plan step 2: microbenchmark unroll factors 1x/2x/4x/8x/16x | R:218 | Tracked: `2037941f` REQ-02. Factors 2 and 4 are measured against the baseline in `bc091474` (done, no adoption); factors 8 and 16 are unmeasured |
| Phase 2 plan step 3: test BitSlice zero-copy operations | R:219 | Delivered: `a1ad6d4e` (done), no-candidate outcome in `dev/active/a1ad6d4e/outcome.md`; the question stays in `2037941f` REQ-02 |
| Phase 2 plan step 4: proceed only on >15% speedup on real workloads | R:220 | Tracked: `2037941f` adopts on family-specific confidence-bound effect rules; the story states no fixed 15% hurdle. Evidence synthesis in `2ad3a3e0` |
| Phase 2 recommendation: profile before implementing | R:222 | Delivered: `30c3aef1` (breakdown, done), `04b85d10` |
| Phase 6b: SIMD polar transforms, cache blocking N > 8K | R:224-225 | Tracked: `b3766524` (SIMD), `07a1813a` (stride >= 64 fast path), `cdb951cc` (rename to `hadamard_butterfly`) |
| Phase 10: GF(p^m) arithmetic | R:227-228 | Delivered: `crates/gf2-core/src/gfp/`, `crates/gf2-core/src/gfpn/`; `e095a100` (archived), `350bff7f`, `3f4b946c` |
| Future: AVX-512 backend | R:235 | Tracked: `4b5d8948` (backend), `f8d230ef`, `c7c0e991`, `b59fa661`; partial code `crates/gf2-kernels-simd/src/x86/bipedal_avx512.rs` |
| Future: ARM NEON for AArch64 | R:236 | Tracked: `194b902a`; `crates/gf2-core/src/kernels/aarch64.rs` holds detection stubs only |
| Future: GPU acceleration | R:239 | Delivered: `crates/gf2-kernels-hip/`; FieldMatrix GPU offload tracked in `16283d6f` |
| Future: batch polynomial operations | R:240 | Delivered: `crates/gf2-core/src/gf2m/batch.rs`, `crates/gf2-core/src/gfpn/batch.rs`; `a7c81834`, `bdf95060`, `2e7db385` |
| Future: extended field degrees (m > 64) | R:241 | Delivered: `crates/gf2-core/src/gf2m/wide.rs`; `6fb4abad`, `7c954fb5` |
| Future: state-of-the-art polynomial factorization | R:244 | Newly filed: `12a3c312` (code holds only factor detection in `crates/gf2-core/src/field/irreducibility.rs`) |
| Future: novel sparse matrix algorithms | R:245 | Delivered: `crates/gf2-core/src/sparse.rs`, `crates/gf2-core/src/field/sparse_matrix.rs`; `5ce13bae`, `cbf576d1`, `eb57f944` |
| Future: hardware-optimized implementations | R:246 | Delivered: `crates/gf2-kernels-simd/`; `220cab0b` tuning profiles; SOTA epics `97bf0879`, `026fc832` (archived) |
| Phase 12: compression support | R:272 | Obsolete: the roadmap marks it not needed; the header compression flag in `crates/gf2-core/src/io/format.rs` has no codec and no consumer requires one |
| Phase 12: checksum verification | R:273 | Delivered for `FieldMatrix`: BLAKE3 payload checksum in `crates/gf2-core/src/io/field_matrix.rs`, `e992c5b0`; the BitVec and BitMatrix `.gf2` formats carry no checksum by that issue's scope |
| Priority 1: Phase 2 profiling decision | R:292 | Tracked: `2037941f` |
| Priority 2: extended SIMD (AVX-512, NEON) | R:293 | Tracked: `4b5d8948`, `194b902a` |
| Priority 3: research algorithms from downstream usage | R:294, R:296 | Delivered: downstream-consumer profiling `3be770d5`, `04b85d10` (done) |
