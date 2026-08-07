# gf2 Technical Debt Assessment (2026-06-30)

## Executive summary

**71 confirmed findings** across 9 dimensions.

| Severity | Count |
|----------|-------|
| Critical | 1 |
| High | 6 |
| Medium | 44 |
| Low | 20 |

| Dimension | Crit | High | Med | Low | Total |
|-----------|------|------|-----|-----|-------|
| doc-drift | 0 | 3 | 12 | 5 | 20 |
| doc-completeness | 0 | 0 | 11 | 6 | 17 |
| test-conventions | 0 | 1 | 10 | 3 | 14 |
| ssot-duplication | 0 | 0 | 8 | 1 | 9 |
| separation-of-concerns | 0 | 0 | 2 | 2 | 4 |
| bitwise-arithmetic | 0 | 1 | 0 | 2 | 3 |
| msrv | 0 | 0 | 1 | 1 | 2 |
| unsafe-isolation | 0 | 1 | 0 | 0 | 1 |
| tail-masking | 1 | 0 | 0 | 0 | 1 |

**Highest-leverage themes**

1. **Tail-masking correctness hole.** `BitVec::from_words` is the only public constructor that omits `mask_tail()`, silently corrupting popcount, rank/select, equality, and all downstream bitwise operations for any caller that supplies words with high bits set above `len_bits % 64`. Single-line fix with immediate correctness impact across the entire crate.

2. **Documentation drift** (20 findings, 3 high). Stale crate names in CONTRIBUTING.md, a compile-failing API name in a presentation (`from_entries` vs `from_triplets`), false FFI safety bounds (F_3 n=64 claim in four sites), missing crates in README/CONTRIBUTING tables, and wrong test commands throughout user-facing docs.

3. **Test-convention debt** (14 findings, 1 high). `SimulationRunner` tests in `modem_analysis_integration.rs` run without `#[ignore]` and will exceed the 5 s CI budget on every run. Widespread bare `#[ignore]` without reason strings (20+ sites in `gf2-coding`) and missing `test_` prefixes across `gf2-kernels-simd` and `gf2-sim`.

4. **SSOT duplication** (9 findings). Eight independent duplications: Gaussian elimination reimplemented twice inline in `nr_5g/mod.rs` and once in `drm.rs`; `lcm_poly` duplicates a `pub(crate)` core function; F_7 LUT builders duplicated between `gf2-kernels-simd` and `gf2-algebra`; `mean_sigma` copy-pasted into four bin files; raw HIP FFI re-declared in test helpers.

5. **Doc-completeness gaps** (17 findings). Systemic missing `# Examples`, `# Complexity`, `# Panics`, and `# Safety` sections across all production crates. The crate-wide `#![allow(clippy::missing_safety_doc)]` in `gf2-kernels-simd/src/lib.rs` papers over the root cause rather than fixing it.

---

## Findings by dimension

### tail-masking

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| Critical | `crates/gf2-core/src/bitvec.rs:204` | `BitVec::from_words` accepts caller-supplied words without calling `mask_tail()`; every other constructor that could produce dirty padding calls `mask_tail()`; downstream bitwise ops, popcount, rank/select, and equality produce wrong results for any caller that passes a word with high bits set. | Call `mask_tail()` before returning; remove the caller-must-ensure note from the doc once the invariant is internally enforced. |

### unsafe-isolation

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| High | `crates/gf2-kernels-simd/src/x86/fp_generic.rs:155,229,266,296` | Four `pub unsafe fn` (`fp_montgomery_mul4`, `fp_montgomery_batch_mul`, `fp_montgomery_batch_add`, `fp_montgomery_batch_sub`) have no `# Safety` doc section and no `// SAFETY:` comments before `get_unchecked` / raw-pointer dereferences; a file-level `#![allow(clippy::missing_safety_doc)]` silences Clippy enforcement. | Remove the `#![allow]`; add a `# Safety` doc section to all four functions stating AVX2 availability and input-validity preconditions; add `// SAFETY:` comments at each unsafe operation site. |

### bitwise-arithmetic

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| High | `crates/gf2-core/src/bitvec.rs:1532` | `polar_transform_into` iterates bit-by-bit for all stride values; for stride >= 64 the butterfly reduces to a word-level XOR, giving a ~64x slowdown on large inputs. | Add a fast path that XOR-assigns word slices when stride >= 64 and both segments are word-aligned via `kernels::ops::xor_inplace`; fall back to the bit loop only for stride < 64. |
| Low | `crates/gf2-coding/src/convolutional.rs:92,187`; `crates/gf2-coding/src/crc.rs:300`; `crates/gf2-coding/src/drm.rs:305` | GF(2) parity computed with `count_ones() % 2` and `weight % 2` at four sites instead of the branchless `& 1` idiom. | Replace `% 2 == 1` with `& 1 != 0` and `% 2 != 0` with `& 1 != 0` at each site. |
| Low | `crates/gf2-coding/src/bch/core.rs:685` | `pack_coeff_stream` writes to a raw `Vec<u64>` using the bit-numbering idiom inline while reading via `BitVec::get`, duplicating the convention instead of going through `BitVec::push_bit`/`words()`. | Replace the manual bit-packing loop with a `BitVec` accumulator and call `.words().to_vec()`; or keep the loop and add `// SAFETY: matches gf2_core bit-numbering convention (word=i>>6, bit=i&63)`. |

### test-conventions

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| High | `crates/gf2-coding/tests/modem_analysis_integration.rs` | Five `SimulationRunner` tests with `max_frames` 4,000-8,000 have no `#[ignore]` annotation and will exceed the 5 s CI budget on every run. | Add `#[ignore = "sim: uncoded BER analysis integration, N frames"]` to all five affected tests. |
| Medium | `crates/gf2-core/src/field/axiom_tests.rs:1027` | `test_axioms_gf2m_wide_256_stress` uses bare `#[ignore]` without a reason string, unlike every other ignored test in the file. | Change to `#[ignore = "slow: gf2m_wide_256 axiom stress"]`. |
| Medium | `crates/gf2-kernels-hip/tests/throughput_bench.rs:13` | `bench_gpu_vs_cpu_batch64` uses a `bench_` prefix instead of `test_` and lacks `#[ignore]` despite running 200 total iterations expected to exceed 5 s. | Rename to `test_gpu_cpu_batch64_throughput` and add `#[ignore = "slow: throughput benchmark, exceeds 5s"]`. |
| Medium | `crates/gf2-coding/tests/dvb_t2_ldpc_verification_suite.rs` and 8 other gf2-coding test files | 20+ bare `#[ignore]` across `dvb_t2_ldpc_verification_suite.rs` (x8), `test_vector_parser.rs` (x4), `test_vectors/loader.rs` (x3), `backend_integration.rs`, `ldpc_validation.rs`, `dvb_t2_ldpc_verification.rs`, `nr5g_regression.rs`, `ldpc/core.rs`; `bcjr/mod.rs:968` uses an inline `// ~38s:` comment instead of a string annotation. | Replace every bare `#[ignore]` with `#[ignore = "slow: ..."]` or `#[ignore = "external: ..."]`; convert the inline comment in `bcjr/mod.rs` to a reason string. |
| Medium | `crates/gf2-coding/tests/ldpc_cache_io.rs` | Six `#[ignore]` strings use capital `"Slow:"` prefix instead of the project-standard lowercase `"slow:"`. | Change `"Slow:"` to `"slow:"` in all six annotations. |
| Medium | `crates/gf2-coding/src/bch/core.rs` | No proptest coverage for BCH encode/decode mathematical invariants; `src/bch/extended.rs` and `src/linear.rs` both carry proptest modules. | Add a `mod proptests` block with at minimum: encode-then-decode roundtrip with <= t errors, and syndrome-zero for valid codewords; mark slow variants `#[ignore = "slow: ..."]`. |
| Medium | `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs` | 18 `#[test]` functions in the campaign binary lack the required `test_` prefix (e.g., `parse_decoder_minsum`, `strict_gpu_flag_wires_to_config`). | Rename every `#[test] fn foo_bar()` to `#[test] fn test_foo_bar()` following `test_<operation>_<scenario>`. |
| Medium | `crates/gf2-kernels-simd/src/x86/mersenne.rs` and 8 other kernel files | ~14 `#[test]` functions across kernel files lack the `test_` prefix (e.g., `batch_mul_exact_multiple_of_8`, `panelized_gemm_gf256_square`, `safe_wrappers_match_scalar_word_boundaries`). Pattern is systemic across `mersenne.rs`, `fp65537.rs`, `fp_medium.rs`, `fp_small.rs`, `fp_small_f32.rs`, `gf2m_batch.rs`, `gf2m_gemm.rs`, `gf2m_wide.rs`, `fp_generic.rs`. | Rename all offending functions to `test_<operation>_<scenario>`. |
| Medium | `crates/gf2-kernels-hip/tests/gpu_cpu_crosscheck.rs:272` | Two proptest GPU tests lack `#[ignore = "external: gfx1030 device required"]` and a `#[cfg(feature = "hip")]` gate. | Add the ignore annotation and the cfg gate to the proptest block. |
| Medium | `crates/gf2-kernels-hip/src/lib.rs:1227` | Three in-module GPU tests (`test_gpu_bcjr_hamming74_noiseless`, `test_gpu_bcjr_batch_multiple`, `test_gpu_bcjr_empty_batch`) lack `#[ignore]`. | Add `#[ignore = "external: gfx1030 device required"]` to all three; add the same to `test_demap_on_stream_matches_default_stream` for consistency. |
| Medium | `crates/gf2-core/src/bitvec.rs` | No proptest coverage for `BitVec` mathematical invariants: tail bits zero after every mutating op, NOT double-inverse, shift roundtrip, push/pop roundtrip, `polar_transform` self-inverse. | Add a `proptest!` module covering at minimum those five invariant families. |
| Low | `crates/gf2-core/src/bitvec.rs:1705` | `test_new`, `test_zeros`, `test_ones`, `test_clear` lack the `_<scenario>` suffix; same pattern in `gfpn/quadratic.rs` and `gfpn/cubic.rs`. | Rename to include a scenario: `test_new_produces_empty_bitvec`, `test_zeros_sets_all_bits_false`, etc. |
| Low | `crates/gf2-algebra/src/packed/packed5.rs`, `packed7.rs` | Proptest blocks are cross-check-only (bitwise result matches scalar per-lane); no direct algebraic invariant tests for commutativity, distributivity, or neg-involution on `Packed5`/`Packed7` themselves. | Add direct invariant proptests mirroring the pattern in `scalar.rs:528-558`. |
| Low | `crates/gf2-kernels-simd/src/mersenne.rs` and 6 other kernel modules | Prime-field and polynomial-multiply kernel modules have only deterministic fixed-case tests; no proptest for field-axiom invariants or SIMD-vs-scalar equivalence. | Add `proptest!` suites with `ProptestConfig::with_cases(500)` to each module, following the pattern in `bipedal3.rs:623-680`. |

### doc-completeness

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| Medium | `crates/gf2-kernels-simd/src/lib.rs:1` | Crate-wide `#![allow(clippy::missing_safety_doc)]` papers over missing `# Safety` sections; 14 submodule files duplicate the same allow, suggesting it was added to silence rather than fix the gap. | Remove the crate-level `#![allow]`; add proper `# Safety` doc sections to all affected `pub unsafe fn`; remove the redundant per-file allows. |
| Medium | `crates/gf2-kernels-simd/src/lib.rs:83` | `LogicalFns` struct exposes 15 public function-pointer fields with no individual `///` doc comments; the struct-level comment only states "identical semantics to the scalar implementation." | Add a `///` doc comment to each of the 15 fields describing slice shape, preconditions, and semantics. |
| Medium | `crates/gf2-core/src/matrix.rs:987` | `BitMatrix::find_pivot_row` has no `# Examples` and no `# Complexity`; `to_sparse` has no `# Complexity` despite being O(rows x cols). | Add a runnable `# Examples` to `find_pivot_row`; add `# Complexity O(rows - start_row)` to `find_pivot_row` and `# Complexity O(rows x cols)` to `to_sparse`. |
| Medium | `crates/gf2-core/src/sparse.rs:890` | `SpBitMatrix::from_dense` and `to_dense` have single-sentence docs with no `# Examples`, `# Complexity`, or `# Panics`. | Add all three sections; note that `# Panics` delegates from `BitMatrix::get`. |
| Medium | `crates/gf2-coding/src/grand/orbgrand.rs:852,863` | `ln_1_plus_exp` and `log_sum_exp` missing `# Arguments`, `# Examples`, `# Complexity`; the immediately-following `log_prob_parity` has all four sections. | Add all three sections to each function; both are O(1). |
| Medium | `crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs:374` | `RuEncodingMatrices::generator()` panics for non-systematic codes but has no `# Panics` section. | Add `# Panics` (panics if `self.is_systematic()` is false) and a `# Examples` doctest. |
| Medium | `crates/gf2-coding/src/modem/analysis.rs:829` | `per_bit_mi_histogram_bits` missing `# Arguments` despite the immediately-following `gmi_bits` having it. | Add `# Arguments` documenting the `stats` parameter. |
| Medium | `crates/gf2-algebra/src/packed/packed5.rs:1410` | `Packed5Matrix::PartialEq::eq` has a single-line description with no `# Examples`; `Debug::fmt` has no doc comment at all; sibling types `Bipedal3Matrix` and `Packed7Matrix` carry full doc blocks. | Add description, `# Examples`, and `# Complexity` to both impls, mirroring `Bipedal3Matrix`. |
| Medium | `crates/gf2-sim/src/presets/dvb_t2.rs:411` | Six builder-chain methods (`decoder`, `demap`, `channel`, `parallelism`, `seed`, `checkpoint_dir`) carry `# Arguments` but no `# Examples`. | Add a `no_run` `# Examples` snippet to each. |
| Medium | `crates/gf2-sim/src/stages/mod.rs:274` | Four stage constructors (`DvbT2Encode::new`, `BitInterleave::new`, `BitDeinterleave::new`, `DvbT2Decode::new`) carry `# Arguments` but no `# Examples`. | Add a brief `# Examples` block to each constructor. |
| Medium | `crates/gf2-kernels-hip/src/lib.rs:411` and `launch_ldpc_bp.rs`, `launch_bch_syndrome.rs`, `launch_chacha20_awgn.rs` | ~20 public getter methods across `GpuBcjrBatch`, `GpuLdpcBp`, `GpuBchSyndrome`, `GpuChaChaAwgn`, and related types have no `# Examples`. | Add `# Examples` blocks with `no_run` to all affected getters, following the pattern in `src/host/streams.rs`. |
| Low | `crates/gf2-core/src/bitvec.rs:847,873` | `find_first_set` and `find_last_set` missing `# Complexity`; siblings `find_first_one` and `find_first_zero` at lines 908/948 have it. | Add `# Complexity O(n) where n is the number of words, with early exit on first non-zero word` to both. |
| Low | `crates/gf2-sim/src/connector.rs` | `Connector::new` has `# Arguments` but no `# Examples` despite the struct-level doc carrying a full example. | Add a `# Examples` block to the constructor. |
| Low | `crates/gf2-sim/src/executor/results.rs:59` | `SnrPointResult::from_counters` missing `# Examples`. | Add a short counter sequence example with assertions on `fer` and `errors`. |
| Low | `crates/gf2-sim/src/parallel/mod.rs:259` | `WorkerCounters::fer()` and `mean_iters()` missing `# Examples`. | Add a `WorkerCounters::default()` + `record_frame()` then assert-ratio example to each. |
| Low | `crates/gf2-coding/src/ldpc/encoding/richardson_urbanke.rs:113`; `src/traits.rs:279,316`; `src/ldpc/nr_5g/mod.rs:1306` | Four public items annotate `# Examples` with `` ```ignore ``, so they are never compiled or executed by `cargo test --doc`. | Replace `` ```ignore `` with `` ```no_run `` for GPU-dependent examples, or make examples self-contained and compilable. |
| Low | `crates/gf2-coding/src/ldpc/nr_5g/mod.rs:1362,1385` | `Nr5gRateMatchedDecoder::with_scale` and `with_algorithm` missing `# Examples`; all sibling methods on the type carry doctests. | Add a compilable doctest to each using `QuasiCyclicLdpc::nr_5g_rate_matched`. |

### msrv

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| Medium | `crates/gf2-kernels-simd/Cargo.toml` | No `rust-version` field; all peer crates declare `rust-version = "1.95"`; without it, `cargo check --locked` silently accepts older toolchains for this crate. | Add `rust-version = "1.95"` to `[package]`. |
| Low | `CLAUDE.md` | MSRV section omits `gf2-algebra` and `gf2-sim` (both declare `rust-version = "1.95"`) and does not note that `gf2-kernels-simd` has no pin. | Update the sentence to list all enforcing crates and flag `gf2-kernels-simd` as unpinned. |

### ssot-duplication

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| Medium | `crates/gf2-core/src/kernels/x86.rs` | Unused `has_avx2/avx512f/pclmulqdq/bmi2()` detection stubs duplicate `gf2-kernels-simd::detect()`; three bare `// TODO:` kernel stubs have never been implemented. | Delete `kernels/x86.rs` and its `mod.rs` entry, or replace stubs with delegation to `gf2-kernels-simd` and remove the TODO stubs. |
| Medium | `crates/gf2-algebra/src/packed/packed5.rs`, `packed7.rs` | `Packed5Matrix` and `Packed7Matrix` silently omit `row()`, `to_row_major()`, and `transpose()` that `Bipedal3Matrix` provides; `lib.rs:31` lists all three as part of the matrix API, making the omission from F_5/F_7 an undocumented asymmetry. | Add all three methods to both types, or explicitly document the asymmetry as intentional in each struct-level doc comment. |
| Medium | `crates/gf2-kernels-hip/tests/common/mod.rs:18` | Test helper re-declares raw `hipMalloc`, `hipFree`, `hipMemcpy`, `hipDeviceSynchronize` FFI bindings instead of using the crate's `DeviceBuffer` RAII wrappers and `ffi.rs`. | Replace the `extern "C"` block with `crate::host::DeviceBuffer`, `copy_from_host`, `copy_to_host`, and `crate::ffi::hip_device_synchronize`. |
| Medium | `crates/gf2-sim/src/bin/gpu_awgn_throughput.rs:155` and 3 other bin files | `mean_sigma(xs: &[f64]) -> (f64, f64)` is copy-pasted verbatim into four bin files; `parallel_throughput.rs` inlines the same three-step computation without a wrapper. | Add `pub(crate) fn mean_sigma` to `gf2-sim/src/observability.rs` (or a new `src/bench_utils.rs`) and replace all four private copies. |
| Medium | `crates/gf2-coding/src/drm.rs:624` | `DrmCode::systematic_form` reimplements full Gaussian elimination over `BitMatrix`; `gf2_core::alg::rref::rref` with `pivot_from_right=false` provides the identical operation. | Replace the function body with `rref(&g, false)`; retain only the post-RREF column-rearrangement logic. |
| Medium | `crates/gf2-coding/src/ldpc/nr_5g/mod.rs:753` | `compute_mother_encoding` contains two literal copies of the same 28-line pivot-find / row-swap / full-column-eliminate kernel applied to different column ranges; `richardson_urbanke.rs` in the same crate correctly calls `gf2_core::alg::rref::rref`. | Extract a private `fn rref_column_pass(work, cols, current_row, pivot_cols)` helper and call it twice. |
| Medium | `crates/gf2-coding/src/bch/core.rs:225` | `BchCode::lcm_poly` duplicates `gf2_core::field::charpoly::poly_lcm` (identical gcd-then-quotient logic); `poly_lcm` is `pub(crate)` and unreachable from `gf2-coding`. | Promote `poly_lcm` to `pub fn FieldPoly::lcm` in `gf2_core/src/field/poly.rs`; remove `BchCode::lcm_poly`; have `charpoly::poly_lcm` delegate to it. |
| Medium | `crates/gf2-kernels-simd/src/bipedal/packed7.rs:37` | F_7 nibble-pair LUT builders (`build_add7_lut`, `build_sub7_lut`, `build_mul7_lut`) are duplicated verbatim between `gf2-kernels-simd` and `gf2-algebra`; a comment in the file acknowledges the duplication and its rationale. | Consolidate in `gf2-kernels-simd`, pub-export, and have `gf2-algebra` import them, making `gf2-kernels-simd` the SSOT. |
| Low | `crates/gf2-sim/src/bin/ldpc_bler_sweep.rs:177` | Private `esn0_db_to_sigma(f64) -> f64` duplicates `gf2_sim::channels::es_n0_db_to_sigma_f64` with only the return type differing. | Remove the local copy; use `f64::from(crate::channels::es_n0_db_to_sigma_f64(esn0_db))`, or add a `f64`-returning variant to `channels/mod.rs`. |

### separation-of-concerns

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| Medium | `crates/gf2-core/src/bitvec.rs:1466` | Four `polar_transform*` methods implement the Arikan (2009) polar-code channel kernel, a coding-theory-specific operation, on the lowest-level primitive type in the workspace. | Move to `gf2-coding` as free functions or an extension trait; if a generic GF(2) butterfly primitive is useful in core, rename it neutrally (e.g., `hadamard_butterfly`). |
| Medium | `crates/gf2-kernels-hip/Cargo.toml:24` | `gf2-kernels-hip` dev-depends on both `gf2-algebra` and `gf2-coding`, which depend on `gf2-kernels-hip` in production via the `hip` feature; two reversed edges in the test layer. | Relocate `permanent_f{3,5,7}` cross-check tests to `gf2-algebra` and QAM demapper tests to `gf2-coding`, where the reference types already live. |
| Low | `crates/gf2-kernels-simd/Cargo.toml:34` | `gf2-kernels-simd` dev-depends on `gf2-algebra` (which depends on `gf2-kernels-simd` in production), inverting the layer order in the test layer; the comment documents this as intentional. | Move proptest cross-checks that use `Packed5`/`Packed7` scalar references into `gf2-algebra`'s own test suite; remove the `gf2-algebra` dev-dep from `gf2-kernels-simd`. |
| Low | `crates/gf2-core/src/sparse.rs:558` | `deterministic_ldpc_like_fixture` embeds "ldpc" in the name of a `#[doc(hidden)]` function exported from `gf2-core`, coupling core vocabulary to a higher-level coding scheme. | Rename to `deterministic_sparse_fixture`, matching the neutral naming of the sibling `deterministic_sparse_bitvec_fixture`. |

### doc-drift

| Severity | File | Issue | Remediation |
|----------|------|-------|-------------|
| High | `README.md` | `gf2-sim` is entirely absent from the workspace layout table despite being a full workspace member with its own `Pipeline`/`Stage`/`Connector` infrastructure. | Add a `gf2-sim` row describing the CPU+GPU FEC simulation pipeline crate with features `hip` and `llr-f64`. |
| High | `docs/presentations/bb85c68a-fieldmatrix/talk.html:250` | Slide 16 uses `SparseFieldMatrix::from_entries(3, 6, entries)`; no such method exists in the codebase; the actual constructor is `from_triplets`. | Replace `from_entries` with `from_triplets` on slide 16. |
| High | `crates/gf2-kernels-hip/src/permanent/mod.rs:33,57,269,324` | Four F_3 FFI doc sites claim `permanent_bipedal3_singleword` supports n=64 via a u128 counter; the function asserts `n <= 63` and panics for n=64; a `#[should_panic]` test confirms this; the parallel F_5 docs (lines 85-86) were correctly updated but the F_3 docs were not. | Replace all four stale sentences with the correct bound (n <= 63) using the same language used for the F_5 update; remove the false u128 counter claim. |
| Medium | `CLAUDE.md` | Architecture parenthetical lists "AARCH64" as a supported SIMD target; only x86 and bipedal subdirs exist in `gf2-kernels-simd`; `gf2-core/src/kernels/aarch64.rs` contains only TODO stubs. | Change the parenthetical to "(AVX2/AVX512)" or "(AVX2/AVX-512F experimental; AARCH64 planned)". |
| Medium | `CONTRIBUTING.md:43` | Project structure tree lists only 3 crates; omits `gf2-algebra`, `gf2-sim`, `gf2-kernels-hip`, `proofs/`, and `dev/`. | Expand the tree to include all workspace members and top-level directories. |
| Medium | `crates/gf2-algebra/README.md:84` | Comment says "16 lanes per u64-pair" for `Packed7`; `Packed7` is a single `u64`. | Change to "16 lanes per u64". |
| Medium | `README.md` | Workspace table attributes GPU LDPC BP to `gf2-coding --features hip`; no GPU LDPC BP code exists in `gf2-coding`; it lives in `gf2-sim/src/gpu/ldpc_bp.rs`. | Remove "LDPC BP" from the `gf2-coding` hip feature description; add a `gf2-sim` row noting its `hip` feature enables GPU LDPC BP and other GPU stages. |
| Medium | `README.md:132` | Developing section shows `cargo test --workspace --all-features --release`; the project mandates `cargo nextest run --workspace --all-features --release --profile ci` with a 5 s per-test hard kill. | Replace with the nextest command and add a note about the `ci` profile's time limit. |
| Medium | `CONTRIBUTING.md:28,31,222` | Three `cargo test` invocations omit `--release` and use `cargo test` instead of `cargo nextest run`; debug-mode tests are 10-100x slower. | Add `--release` and switch to `cargo nextest run --profile ci` at all three sites. |
| Medium | `CONTRIBUTING.md:139` | Doc-comment example uses `use gf2::BitVec`; the crate name is `gf2_core`; no `gf2` crate exists in the workspace. | Change to `use gf2_core::BitVec;`. |
| Medium | `crates/gf2-algebra/src/permanent/mod.rs:15` | Module status says "F_5 covers `n <= Packed5::LANES = 64`"; the actual safe bound is n <= 63, enforced by `gray_code_iter`'s assert. | Change to "F_5 covers `n <= 63` (one below Packed5::LANES, bounded by gray_code_iter)". |
| Medium | `crates/gf2-algebra/src/permanent/bipedal5.rs:8,25` | Lines 8 and 25 claim the single-word path limit is "n <= LANES = 64"; lines 10, 21, 28 in the same file correctly state n <= 63. | Change both header and prose to n <= 63 and explain the off-by-one (a 64-column walk requires iterating 2^64 Gray steps). |
| Medium | `crates/gf2-kernels-simd/src/lib.rs:9` | Crate doc claims "AVX-512F (experimental)"; all AVX-512 functions in `x86/bipedal_avx512.rs` are `unimplemented!()` stubs; `detect_x86()` never dispatches to them. | Change to "Supported: AVX2. AVX-512F kernels are stubbed but not yet implemented." |
| Medium | `docs/presentations/ae82bd73-gf2-algebra-permanent/talk.html:125` | Slide 8 says Ryser's formula is "proven on the `FiniteField` trait"; the actual proof uses Mathlib `CommRing` / `ZMod 3`; the Charon-extracted Rust binding was descoped (user-approved Option 3, 2026-05-17). | Update slide 8 to say "proven over Mathlib CommRing / ZMod 3 (abstract, bounded n <= 63); direct Charon-extracted Rust binding descoped (Option 3)". |
| Medium | `docs/presentations/6efb756b-grand-sogrand/talk.html:slide 28` | Slide 28 and slides 9/11 cite `pub(crate)` or private module paths (`orbgrand::LogisticWeightPatternIter`, `sogrand::compute_block_apps`, `sogrand::log_cap_minus_exp`, etc.) as if they were reachable public API. | Replace with crate-public equivalents where they exist; annotate others as "internal source-file path, not public API". |
| Low | `crates/gf2-algebra/src/lib.rs:15` | Crate-level docs embed internal wave/task designators (`W2 complete`, `W4-T18/T20`, `T2/T3/T4/T5/T6/T7/T8/T9 all landed`) in the entry point visible via `cargo doc`. | Remove the `# Status` section or replace with a brief `# Completeness` note; move wave attribution to the commit history or an internal CHANGELOG. |
| Low | `README.md` | Features table omits `gf2-coding`'s default-on `sim-observability` feature (per-SNR JSON checkpoints, SIGINT flush, ChaCha20 RNG seek). | Add a `sim-observability` row to both the root README and `crates/gf2-coding/README.md` features tables. |
| Low | `docs/presentations/6efb756b-grand-sogrand/talk.html:362` | Slide 26 quotes `cargo test --workspace --all-features --release` and a stale test count of 2816. | Replace with `cargo nextest run --workspace --all-features --release --profile ci`; drop or update the hardcoded count. |
| Low | `crates/gf2-sim/src/frame_sim.rs:17` | Comment says the channel stages are "not yet on main"; they landed with `db9836e4`; `lib.rs:113` already exports `pub mod channels`. | Remove the stale rationale; state only the actual reason (ChaCha20 RNG version mismatch with `gf2-coding`). |
| Low | `crates/gf2-core/src/compute/cpu.rs:101` | `// TODO: Add parallel version when parallel feature is enabled` has been open since initial scaffolding with no tracking issue and no reference to a JIT ticket. | Wire up a rayon-parallel matmul under `#[cfg(feature = "parallel")]`, or replace the comment with a JIT issue ID so the deferred work is traceable. |

---

## Documentation drift

The 20 doc-drift findings fall into five groups.

**User-facing reference docs (high priority, immediate fix needed).** The FieldMatrix presentation (`bb85c68a`) uses `SparseFieldMatrix::from_entries()`, a method that does not exist anywhere in the codebase; the real constructor is `from_triplets`. The FFI safety docs in `gf2-kernels-hip/src/permanent/mod.rs` at four separate sites assert that `permanent_bipedal3_singleword` supports n=64 via a u128 counter, when the function asserts `n <= 63` and panics; the equivalent F_5 docs were correctly updated on 2026-05-15, but the F_3 docs were not synchronized. `README.md` omits `gf2-sim` entirely from the workspace layout table.

**Test-command misinformation in CONTRIBUTING.md and README.md.** `README.md` line 132 shows `cargo test`; `CONTRIBUTING.md` lines 28, 31, and 222 show `cargo test --workspace` without `--release`; `CONTRIBUTING.md` line 139 imports from `gf2::BitVec` (no such crate exists; the name is `gf2_core`). Developers following these instructions will run debug-mode builds at 10-100x normal cost or encounter import errors.

**Permanent-count bound confusion.** Three findings propagate the same confusion between `Packed5::LANES = 64` (the storage capacity of one word) and the safe iteration bound of n <= 63 enforced by `gray_code_iter`'s assert at `gray.rs:128`. Sites affected: `gf2-algebra/src/permanent/mod.rs:15`, `gf2-algebra/src/permanent/bipedal5.rs:8` and `:25`, and the four F_3 HIP FFI doc sites (which additionally state a false u128 counter claim).

**Project structure drift.** `CLAUDE.md` lists "AARCH64" as a supported SIMD target when only TODO stubs exist; `CONTRIBUTING.md` shows a 3-crate project tree while the workspace has 6 crates plus `proofs/` and `dev/`; `gf2-algebra/README.md` describes `Packed7` as "16 lanes per u64-pair" when `Packed7` is a single `u64`; `gf2-kernels-simd/src/lib.rs` claims AVX-512F is experimentally supported when all AVX-512 functions are `unimplemented!()` stubs and the runtime dispatch never selects them.

**Internal tracking language in public docs.** `gf2-algebra/src/lib.rs` embeds wave/task designators (`W2 complete`, `T2/T3/T4/T5/T6/T7/T8/T9 all landed`) in crate-level docs visible via `cargo doc`. The `6efb756b` GRAND presentation cites private module paths (`orbgrand::LogisticWeightPatternIter`, `sogrand::compute_block_apps`) as if they were callable public API; these are `pub(crate)` or private and unreachable from downstream code.

---

## Recommended remediation order

1. **Fix `BitVec::from_words` tail-masking invariant** (`bitvec.rs:204`). Call `mask_tail()` before returning; remove the caller-must-ensure note. One-line fix with immediate correctness impact across all consumers of the public API who pass unmasked words.

2. **Add `#[ignore]` to the five `SimulationRunner` tests in `modem_analysis_integration.rs`**. These tests exceed the 5 s CI budget on every run. Each needs `#[ignore = "sim: uncoded BER analysis integration, N frames"]`.

3. **Fix the three high-severity doc-drift items**: replace `from_entries` with `from_triplets` in `bb85c68a` slide 16; correct all four F_3 HIP FFI doc sites to n <= 63; add the `gf2-sim` row to the README workspace table.

4. **Add `# Safety` docs and `// SAFETY:` comments to the four `pub unsafe fn` in `fp_generic.rs`** and remove the `#![allow(clippy::missing_safety_doc)]` from `lib.rs` and the 14 submodule files where it appears.

5. **Fix the `polar_transform_into` performance regression** (`bitvec.rs:1532`). Add a word-level XOR fast path for stride >= 64 via `kernels::ops::xor_inplace`; fall back to the bit-by-bit loop only for stride < 64.

6. **Canonicalize the five SSOT duplications of algorithmic logic**: (a) replace `DrmCode::systematic_form` with `gf2_core::alg::rref::rref`; (b) extract `rref_column_pass` in `nr_5g/mod.rs`; (c) promote `poly_lcm` to `pub fn FieldPoly::lcm` and remove `BchCode::lcm_poly`; (d) consolidate F_7 LUT builders in `gf2-kernels-simd` and import from `gf2-algebra`; (e) add `pub(crate) fn mean_sigma` to `gf2-sim/src/observability.rs` and replace the four private copies.

7. **Replace raw HIP FFI re-declarations in `tests/common/mod.rs`** with `DeviceBuffer` RAII wrappers and `crate::ffi::hip_device_synchronize`.

8. **Bulk-fix test-convention violations**: (a) rename ~32 `#[test]` functions lacking the `test_` prefix across `gf2-kernels-simd` and `gf2-sim/bin`; (b) convert 20+ bare `#[ignore]` to tagged forms in `gf2-coding`; (c) fix capital `"Slow:"` to `"slow:"` in `ldpc_cache_io.rs`; (d) add `#[ignore = "external: gfx1030 device required"]` to the three in-module GPU tests in `gf2-kernels-hip`.

9. **Add `rust-version = "1.95"` to `gf2-kernels-simd/Cargo.toml`** and update the MSRV sentence in `CLAUDE.md` to list all enforcing crates.

10. **Fix medium doc-drift in CONTRIBUTING.md and README.md**: replace all `cargo test` with `cargo nextest run --release --profile ci`; fix `use gf2::BitVec` to `use gf2_core::BitVec`; expand the project structure tree.

11. **Fix the permanent n <= 63 bound documentation** in `gf2-algebra/src/permanent/mod.rs:15` and `bipedal5.rs:8,25`; correct CLAUDE.md AARCH64 claim; fix `gf2-algebra/README.md` Packed7 lane count.

12. **Address the `polar_transform*` separation-of-concerns issue**: move the four methods to `gf2-coding`, or rename them neutrally in `gf2-core` if a generic GF(2) butterfly primitive belongs in the base layer.

13. **Add proptest coverage** for: (a) `BitVec` mathematical invariants; (b) BCH encode/decode roundtrip; (c) algebraic invariants directly on `Packed5` and `Packed7`; (d) field-axiom and SIMD-vs-scalar equivalence in prime-field kernel modules.

14. **Resolve doc-completeness medium items in a single sweep**: `# Panics` on `RuEncodingMatrices::generator()`; `# Arguments`/`# Examples`/`# Complexity` on `LogicalFns` fields, `ln_1_plus_exp`, `log_sum_exp`, `Packed5Matrix::Debug`, the six `gf2-sim` builder methods, the four stage constructors, and the ~20 HIP getter methods.

15. **Remove internal wave/task tracking** from `gf2-algebra/src/lib.rs` crate-level docs; annotate private-module paths in the GRAND presentation as "internal source references, not public API"; update the `gf2-kernels-simd` crate doc to remove the unimplemented AVX-512F claim.