# 12907582 drift pre-audit: crate roots and top module docs

Scope: `//!` blocks of `src/lib.rs` and `src/*/mod.rs` in gf2-core, gf2-coding,
gf2-algebra, gf2-sim, gf2-kernels-simd, gf2-kernels-hip, gf2-stats, plus the
crate `Cargo.toml` feature tables they describe. Read-only; verified against
code on branch `worktree-agent-ab61dd5f4d07bfb81` (2026-10-01). Narrative and
issue-id designators are out of scope (moved to the tersification sweep).

Verdicts: `correct`, `wrong` (contradicts code), `stale` (was true, code moved
on, or omits current surface), `unverifiable` (no code evidence either way).

## Summary

| Crate | wrong | stale | notes |
|---|---|---|---|
| gf2-core | 5 | 6 | kernels/compute docs claim AVX-512/NEON/GPU feature that do not exist |
| gf2-coding | 1 | 3 | root omits most of the public module surface |
| gf2-algebra | 0 | 6 | all plan paths moved to `dev/archive/ae82bd73-gf2-algebra-permanent/plans/` |
| gf2-sim | 0 | 1 | module map omits `snr_checkpoint` |
| gf2-kernels-simd | 2 | 3 | root claims AVX-512F support and all-safe API |
| gf2-kernels-hip | 0 | 4 | root describes the crate as BCJR-only, gfx1030-only |
| gf2-stats | 0 | 0 | |

Bare TODO markers without a JIT id: 10 in code (6 locations), 5 in contributor
prose under `crates/*/docs/`.

## gf2-core

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-core/src/lib.rs:3-5` | Powers "coding theory and compression tooling" in gf2-coding | wrong | no `compress` symbol anywhere in `crates/gf2-coding/src` | Delete "and compression". |
| `crates/gf2-core/src/lib.rs:7-11` | Core types are `BitVec`, `BitMatrix`, `SpBitMatrix` | stale | re-exports also `BitSlice`, `BitSliceMut`, `SpBitMatrixDual`, `SpBitMatrixBlockCsr`, `SparseBitMatrix`, `RowPermutation` (`lib.rs:75-80`) | Add the missing re-exports to the list. |
| `crates/gf2-core/src/lib.rs:9` | `BitVec` backed by `Vec<u64>` | correct | `bitvec.rs`; `get` returns `bool` (`bitvec.rs:353`) | |
| `crates/gf2-core/src/lib.rs:11` | `SpBitMatrix` is CSR | correct | `sparse.rs:71-76` (`indptr`, `indices`) | |
| `crates/gf2-core/src/lib.rs:15-17` | u64 words, little-endian bit order, `word = i >> 6`, tail masking | correct | consistent with `io/mod.rs:52` and `bch/mod.rs:100-102` | |
| `crates/gf2-core/src/compute/mod.rs:16` | ComputeBackend does "matmul, RREF, batch encode/decode" | stale | `compute/backend.rs:94-208`: `matmul`, `rref`, `matvec`, `matvec_transpose`, `batch_matvec`, `batch_matvec_transpose`; no encode/decode | Replace with "matmul, RREF, matvec, batch matvec". |
| `crates/gf2-core/src/compute/mod.rs:17` | `GpuBackend (future)` | unverifiable | no such type | Drop or keep as explicit non-promise. |
| `crates/gf2-core/src/compute/mod.rs:23` | kernels `Backend` impls: `ScalarBackend`, `SimdBackend` | correct | `kernels/mod.rs:34`, `kernels/simd/mod.rs:17` | |
| `crates/gf2-core/src/compute/mod.rs:32` | Feature `gpu`: GPU backend via HIP/ROCm (opt-in) | wrong | `crates/gf2-core/Cargo.toml` `[features]`: `simd`, `rand`, `parallel`, `visualization`, `io`, `tuning-profile`, `test-support`; no `gpu`; no `cfg(feature = "gpu")` in `src/` | Remove the `gpu` bullet. |
| `crates/gf2-core/src/compute/mod.rs:51` | `gf2-core = { version = "0.2", features = ["parallel"] }` | wrong | `crates/gf2-core/Cargo.toml:3` `version = "0.1.0"` | Use `"0.1"` or a path dependency. |
| `crates/gf2-core/src/kernels/mod.rs:5` | SIMD: "AVX2/AVX-512/NEON acceleration (optional, runtime detected)" | wrong | `gf2-kernels-simd/src/x86/mod.rs:44-49` dispatches AVX2 only; `gf2-kernels-simd/src/lib.rs:138-145` returns `None` off x86 | "AVX2 on x86/x86_64 (runtime detected, `simd` feature)". |
| `crates/gf2-core/src/kernels/mod.rs:6-7` | GPU planned for this layer; FPGA planned | unverifiable | no code | Drop FPGA; keep GPU only if an issue exists. |
| `crates/gf2-core/src/kernels/mod.rs:14` | "Backend-specific modules: SIMD, GPU, FPGA implementations" | stale | `kernels/mod.rs:16-30`: `backend`, `ops`, `scalar`, `simd` (feature), `x86`/`aarch64` (detection stubs only) | List `simd` plus the detection modules. |
| `crates/gf2-core/src/kernels/simd/mod.rs:3-4` | Backend "uses AVX2/AVX-512 (x86) or NEON (ARM)" | wrong | `kernels/simd/mod.rs:29` hard-codes `name: "avx2"`; dispatch as above | "uses AVX2 on x86/x86_64". |
| `crates/gf2-core/src/kernels/x86.rs:1-9`, `kernels/aarch64.rs:1-7` | "This module will contain optimized implementations ... Currently only feature detection stubs" | stale | kernels live in `gf2-kernels-simd`; only `has_pclmulqdq` is used (`benches/tuning_calibration.rs:11779`) | "Feature-detection helpers; kernels live in `gf2-kernels-simd`." |
| `crates/gf2-core/src/field/mod.rs:50-52` | Lagrange interpolation, balanced product tree + batch GCD, NTT "land in sibling tasks" | stale | `field/poly_interpolate.rs:1`, `field/poly.rs:90` (`build_subproduct_tree`), `poly.rs:1720` (`batch_gcd`), `field/ntt` (named at `field/mod.rs:34`) | Delete the sentence. |
| `crates/gf2-core/src/field/mod.rs:43` | `Gf2mPoly_<V>` is a `pub type` alias of `FieldPoly` | correct | `gf2m/field.rs:2348` | |
| `crates/gf2-core/src/gf2m/mod.rs:3` | "re-exported from the field submodule for backward compatibility" | stale | `gf2m/mod.rs:8,23`: private `mod field; pub use field::*` inside `gf2m`, unrelated to `crate::field` | "Items are defined in the private `field` submodule and re-exported here." |
| `crates/gf2-core/src/gfp/mod.rs:14-15,23-28` | Mersenne `n ≥ 31`, Proth `n ≥ 24`, `P = 2` bitwise, Goldilocks separate | correct | `gfp/mod.rs:76-91` | |
| `crates/gf2-core/src/io/mod.rs:22` | Type tags 1..4 | not checked | `io/format.rs` `TypeTag` | |
| `crates/gf2-core/src/io/mod.rs:77,82` | SpBitMatrix payload is COO `(row, col)` pairs | correct | `io/sparse.rs:93,179` | |
| `crates/gf2-core/src/io/mod.rs:107-108` | Compression and checksum flags "not implemented yet" (legacy formats) | correct | no `blake3` use in `io/{bitvec,matrix,sparse}.rs`; only flag setters in `io/format.rs:52-70` | |
| `crates/gf2-core/src/gfpn/mod.rs:11-21` | Listed types | correct | `gfpn/mod.rs:71-75` | |
| `crates/gf2-core/src/tuning/mod.rs`, `alg/mod.rs` | | correct | doctest-backed / trivial | |

## gf2-coding

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-coding/src/lib.rs:9` | "includes both block codes and streaming (convolutional) codes" | stale | `convolutional.rs:1` "skeleton for future implementation"; only `new/state/constraint_length/rate` (`convolutional.rs:54-160`), no encode/decode | "Block codes; the convolutional module is a skeleton." |
| `crates/gf2-coding/src/lib.rs:13-43` | Public surface = `LinearBlockCode`, `SyndromeTableDecoder`, `bch`, convolutional, `llr` | stale | `lib.rs:73-98` also exports `ldpc` (DVB-T2 QC-LDPC), `gldpc`, `product`, `bcjr`, `grand`, `osd`, `transform`, `crc`, `drm`, `modem`, `channel`, `fading`, `info_theory`, `simulation`, `dvb_t2_bicm_harness`, `tuning` | Add a module map covering the exported modules. |
| `crates/gf2-coding/src/lib.rs:26-27` | BCH constructed over any supported base field; decodes binary codes | correct | `bch/mod.rs:5-6`, `bch/core.rs:430` | |
| `crates/gf2-coding/src/lib.rs:36-38` | Convolutional types are skeletons | correct | as above | |
| `crates/gf2-coding/src/lib.rs:92` (code comment) | "SIMD detection is now handled internally in llr.rs via once_cell::Lazy" | correct | `llr.rs:386` | |
| `crates/gf2-coding/src/bch/mod.rs` | Workflow, layouts, decoder guarantees, HIP batch syndrome | correct (spot-checked) | doctest `bch/mod.rs:192-219`; `Cargo.toml` `hip` feature comment; `bch/dvb_t2/mod.rs:344` | Complexity table not re-derived. |
| `crates/gf2-coding/src/product/mod.rs:30-31` | Built-in `ProductComponent` impls: `ExtendedBchComponent`, `CrcCode` | stale | `product/mod.rs:318,336,354` adds `DrmCode` | Add `DrmCode`. |
| `crates/gf2-coding/src/modem/mod.rs:44` | Example link `https://github.com/openamateur/gf2/...` | wrong | `git remote`: `github.com/erankavija/gf2` | Use the repo-relative path `crates/gf2-coding/examples/modem_gray_qam_preset.rs`. |
| `crates/gf2-coding/src/modem/mod.rs:26` | Gray QAM orders `2, 4, 16, 64, 256` | not checked | `modem/presets.rs` | |
| `crates/gf2-coding/src/bcjr/mod.rs:7` | `O(n · 2^(n-k))` | correct | states `= 1 << (n-k)` (`gf2-kernels-hip/src/lib.rs:409`) | |
| `gldpc/mod.rs`, `transform/mod.rs` | doctest-backed; no target/feature claims | correct | | |
| `crates/gf2-coding/Cargo.toml` defaults `simd`, `sim-observability` | not described in `lib.rs` | — | | Optional: mention in a Features section. |

## gf2-algebra

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-algebra/src/lib.rs:13` | Unsafe-isolation invariant "(CLAUDE.md §Architecture, point 3)" | stale | `CLAUDE.md` delegates to `AGENTS.md`; `AGENTS.md:56` "## Architecture boundaries" | Cite `AGENTS.md` §Architecture boundaries. |
| `crates/gf2-algebra/src/lib.rs:22` | `Bipedal3` = 64 lanes | correct | `packed/bipedal3.rs:93-96` (`mag: u64, sgn: u64`), `:494` `LANES = 64` | |
| `crates/gf2-algebra/src/lib.rs:25-26` | `Bipedal3Vec` = two parallel `Vec<u64>` with mask-tail invariant | correct | `packed/bipedal3.rs:1553-1557` | |
| `crates/gf2-algebra/src/lib.rs:29-30` | `Bipedal3Matrix` column-major `Vec<Bipedal3Vec>` | correct | `packed/bipedal3.rs:2921-2926` | |
| `crates/gf2-algebra/src/lib.rs:34-50` | `gray_code_iter`, `permanent_ryser`, `permanent_mod3_reference`, `permanent_bipedal3/5/7`, `permanental_rank_status` exist | correct | `gray.rs:129`, `permanent/ryser.rs:92`, `reference.rs:90`, `bipedal3.rs:180`, `bipedal5.rs:108`, `bipedal7.rs:110`, `rank.rs:194` | |
| `crates/gf2-algebra/src/lib.rs:42-44` | Single-word fast path `n ≤ 63`, then multi-word | correct | `permanent/bipedal3.rs:195` `if n <= 63`; matches HIP bound (`gf2-kernels-hip/src/permanent/mod.rs:95-98`, 0b45a5fa done) | |
| `crates/gf2-algebra/src/lib.rs:57-59` | Link `../../../dev/plans/6e20133d/d1a_gf2_algebra_boundary.md` | stale | file is `dev/archive/ae82bd73-gf2-algebra-permanent/plans/6e20133d/d1a_gf2_algebra_boundary.md` | Fix the path (or drop the relative link). |
| `crates/gf2-algebra/src/lib.rs:61-69` | Module map: `packed`, `permanent`, `gray`, `parallel`, `gpu` | stale | `lib.rs:89` `pub mod tuning` (and `testutil` under `test-support`) missing | Add `tuning` row. |
| `crates/gf2-algebra/src/lib.rs:68-69` | `parallel` default on, `gpu` default off | correct | `Cargo.toml` `default = ["simd", "parallel", "f5", "f7"]` | |
| `crates/gf2-algebra/src/lib.rs:73-77` | `scripts/check-feature-matrix.sh` covers the 64-cell matrix | correct (path is crate-relative) | `crates/gf2-algebra/scripts/check-feature-matrix.sh` exists | Say `crates/gf2-algebra/scripts/...`. |
| `crates/gf2-algebra/src/lib.rs:81-84` | See-also paths under `dev/plans/...` | stale | all four now under `dev/archive/ae82bd73-gf2-algebra-permanent/plans/{,6e20133d,9fe275d3,4fced99b}/` | Update paths. |
| `crates/gf2-algebra/src/permanent/mod.rs:9-12,33` | Plan paths `dev/plans/ae82bd73-.../gf2_algebra_permanent.md`, `dev/plans/9fe275d3/...`, `dev/plans/6e20133d/...` | stale | same relocation | Update paths. |
| `crates/gf2-algebra/src/permanent/mod.rs:19-20` | F_5 `n ≤ Packed5::LANES = 64`, F_7 `n ≤ Packed7::LANES = 16` | correct | `packed/packed5.rs:440`, `packed/packed7.rs:216` | |
| `crates/gf2-algebra/src/packed/mod.rs:9` | `dev/plans/9fe275d3/d1b_packed_field_api.md` | stale | `dev/archive/ae82bd73-gf2-algebra-permanent/plans/9fe275d3/d1b_packed_field_api.md` | Update path. |
| `crates/gf2-algebra/Cargo.toml:2-3` (comment) | `dev/plans/d1c_feature_gate_matrix.md §8.1` | stale | no file with that name in the tree; the decision is `.../plans/4fced99b/d1c_feature_matrix.md` | Update path. |

## gf2-sim

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-sim/src/lib.rs:41` | `Pipeline::dvb_t2` and `Pipeline::nr_5g` builders | correct | `presets/dvb_t2.rs:791`, `presets/nr_5g.rs:1023` | |
| `crates/gf2-sim/src/lib.rs:57-70` | Seven examples listed | correct | `crates/gf2-sim/examples/` has exactly those seven | |
| `crates/gf2-sim/src/lib.rs:75` | Byte-identity across worker counts `{1, 2, 4, 8, 24}` | correct | `tests/parallel_determinism.rs:4`, `tests/common/mod.rs:15` | |
| `crates/gf2-sim/src/lib.rs:82-104` | Module map | stale | `lib.rs:133` `pub mod snr_checkpoint` (re-exported `lib.rs:170-174`) is absent from the map | Add `snr_checkpoint` row. |
| `crates/gf2-sim/src/lib.rs:92` | `PipelineConfig` has `From<&SimulationConfig>` | correct | `config.rs:125` | |
| `crates/gf2-sim/src/lib.rs:104` | `gpu`: `feature = "hip"` | correct | `gpu/mod.rs:10-14,23,513`; `Cargo.toml` `hip` | |
| `crates/gf2-sim/src/lib.rs:110` | Design doc at `dev/archive/f9717e7e-gf2-sim/active/ec530af9/ec530af9-pipeline-design.md` | correct | file exists | |
| `crates/gf2-sim/src/executor/mod.rs:15` | `Pipeline::run_checkpointed` | correct | `pipeline.rs:304` | |
| `graph`, `parallel`, `stages`, `checkpoint`, `channels`, `presets`, `permanent_*` mod docs | | correct | doctests / listed items exist | |

## gf2-kernels-simd

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-kernels-simd/src/lib.rs:9` | "Supported (x86_64): AVX2, AVX-512F (experimental)" | wrong | runtime dispatch is AVX2 only (`x86/mod.rs:44-49`); `x86/bipedal_avx512.rs:1-7,17` is a compile-time-gated `unimplemented!()` stub behind `target_feature = "avx512f"` (`x86/mod.rs:16-20`); per-bundle targets used: `avx2` (135), `avx2,fma` (8), `bmi2` (3), `sse4.1`, `popcnt`, `pclmulqdq` (1 each) | "Supported: x86/x86_64 AVX2 (some bundles also need BMI2, PCLMULQDQ, FMA, SSE4.1, POPCNT, detected per bundle). AVX-512F: compile-time stub only, no kernels." |
| `crates/gf2-kernels-simd/src/lib.rs:10` | "AArch64 NEON planned" | unverifiable | `detect()` returns `None` off x86 (`lib.rs:138-145`); no aarch64 code | Drop unless an issue exists. |
| `crates/gf2-kernels-simd/src/lib.rs:4-5` | "All public APIs are safe and return plain function pointers that operate on `&mut [u64]` / `&[u64]`" | wrong | 71 `pub unsafe fn` (e.g. `bipedal/mod.rs:121-196`, `bipedal/lanes.rs:18`); `#![allow(clippy::missing_safety_doc)]` (`lib.rs:1`); bundles over `f32`/`f64` (`llr.rs:16-55`, `modem.rs`) | "Detection entry points are safe and return function-pointer bundles; `bipedal::avx2` and lane modules expose `unsafe fn` with documented preconditions; operand types vary by module." |
| `crates/gf2-kernels-simd/src/lib.rs:2` | "SIMD-accelerated logical kernels for gf2-core" | stale | `lib.rs:15-36`: also GF(p) (`fp_*`, `mersenne`, `fp65537`), GF(2^m) (`gf2m*`), `bch_encode`, `llr`, `modem`, `bipedal` F_3/F_5/F_7; consumers gf2-coding and gf2-algebra | Widen the one-line summary. |
| `crates/gf2-kernels-simd/Cargo.toml:25-27` | Features `avx2`, `avx512`: "toggles for building specific backends" | stale | no `cfg(feature = "avx2")` / `"avx512"` in this crate or gf2-core | Remove the two features or document them as no-ops. |
| `crates/gf2-kernels-simd/src/bipedal/mod.rs:14` | `dev/plans/c7542983/r4_simd_batching_decision.md` | stale | `dev/archive/ae82bd73-gf2-algebra-permanent/plans/c7542983/r4_simd_batching_decision.md` | Update path. |
| `crates/gf2-kernels-simd/src/bipedal/mod.rs:44` | "today only `Avx2Lane`; future AVX-512 / AArch64" | correct | `lanes.rs`; `bipedal/mod.rs:73` | |
| `crates/gf2-kernels-simd/src/bipedal/mod.rs:29-34` | AVX2 entry points in `x86::bipedal_avx2{,_packed5,_packed7}` | correct | `x86/mod.rs:10-15` | |

## gf2-kernels-hip

| File:line | Claim | Verdict | Evidence | Minimal correction |
|---|---|---|---|---|
| `crates/gf2-kernels-hip/src/lib.rs:3` | "provides GPU-accelerated batch BCJR decoding" | stale | also `host` (33), `launch_chacha20_awgn` (39), `launch_ldpc_bp` (48), `launch_bch_syndrome` (62, `hip`), `permanent` (73, `hip`), `GpuGrayQamDemapper` (662); `Cargo.toml` `description = "... BCJR batch decoder"` | List the kernel families; fix the manifest description. |
| `crates/gf2-kernels-hip/src/lib.rs:9` | "tested with ROCm 7.2" | unverifiable | | |
| `crates/gf2-kernels-hip/src/lib.rs:10` | Requires "gfx1030 ISA (RX 6000 series)" | stale | `build.rs:10` compiles blobs for `gfx1030` (mandatory), `gfx1100`, `gfx1200`, `gfx90a`, `gfx940`, `gfx942` (best-effort); `kernels/gfx*/` dirs; `host/arch.rs` `GfxTarget` | "gfx1030 is the required CI target; gfx1100/1200/90a/940/942 blobs are built best-effort and selected at run time." |
| `crates/gf2-kernels-hip/src/lib.rs:69-72` | `permanent`: "placeholder scaffold. Populated by downstream issues ad55b777, b43cdf33, 5c0505b2" | stale | `permanent/mod.rs:3-5` "all fully implemented"; dispatchers at `permanent/mod.rs:2255,2353,2457` | "Per-prime permanent kernels (F_3, F_5, F_7)." |
| `crates/gf2-kernels-hip/src/lib.rs:14-24` | Example API shape | correct | `lib.rs:407` `new(h_cols, n, k, max_batch)`, `lib.rs:491-494` `decode_batch -> (Vec<Vec<f32>>, Vec<Vec<f32>>)` | |
| `crates/gf2-kernels-hip/src/permanent/mod.rs:95-98,...` | `1 <= n <= 63` bound | correct | matches `gf2-algebra/src/permanent/bipedal3.rs:195` (0b45a5fa) | Verified, not re-fixed. |
| `crates/gf2-kernels-hip/src/permanent/mod.rs:14-16` | Compiled only under `hip`; `.hip` sources via `build.rs` | correct | `lib.rs:73`; `build.rs` | |
| `crates/gf2-kernels-hip/src/host/mod.rs:7-11` | Submodules and types | correct | `host/mod.rs:27-38` | |
| `crates/gf2-kernels-hip/Cargo.toml:11-13` | `hip` feature "Enable per-prime permanent kernels ... (gfx1030)" | stale | also gates `launch_bch_syndrome` (`lib.rs:62`); multi-arch per `build.rs:10` | "Enables the BCH syndrome and permanent kernels; requires hipcc." |

## gf2-stats

`crates/gf2-stats/src/lib.rs:3-6` names `sampler`, `intervals`, `binomial`,
`accumulator`, `weighted`; all are declared (`lib.rs:10-14`). No feature or
target claims. 0 wrong, 0 stale.

## Bare TODO / FIXME markers (no JIT id within ±4 lines)

Code (candidates for REQ-04):

| Location | Text |
|---|---|
| `crates/gf2-coding/src/llr.rs:433` | `TODO: Add SIMD implementation in gf2-kernels-simd` |
| `crates/gf2-coding/src/llr.rs:454` | `TODO: Add SIMD implementation in gf2-kernels-simd` |
| `crates/gf2-core/src/compute/cpu.rs:101` | `TODO: Add parallel version when `parallel` feature is enabled` |
| `crates/gf2-core/src/kernels/aarch64.rs:21` | `TODO: Add runtime detection when std::arch stabilizes aarch64 feature detection` |
| `crates/gf2-core/src/kernels/aarch64.rs:25-26` | `TODO: Implement NEON kernel`; `TODO: Implement crypto extension based carry-less multiplication` |
| `crates/gf2-core/src/kernels/simd/mod.rs:29` | `name: "avx2", // TODO: Actually detect which variant` |
| `crates/gf2-core/src/kernels/x86.rs:45-47` | `TODO: Implement AVX2 kernel`; `TODO: Implement AVX-512 kernel`; `TODO: Implement PCLMULQDQ-based carry-less multiplication` (all three already exist in gf2-kernels-simd; markers are stale, not open work) |

Contributor prose (REQ-05 territory, classify in the migration manifest):

| Location | Text |
|---|---|
| `dev/archive/legacy/crates/gf2-core/docs/KERNEL_OPTIMIZATION.md:459,552` | `TODO: Phase 6 - Documentation`; `TODO: simd_vs_scalar.rs` |
| `dev/archive/legacy/crates/gf2-core/docs/QUALITY_AUDIT_REPORT.md:214` | `TODO: Validate with cargo +1.80 build` |
| `dev/archive/legacy/crates/gf2-core/docs/POLAR_IMPLEMENTATION_PLAN.md:162,167` | `TODO` status markers |
| `dev/archive/legacy/crates/gf2-core/docs/archive/PHASE11_IMPLEMENTATION_PLAN.md:56,750-753` | `TODO` status legend (archived) |
| `dev/archive/legacy/crates/gf2-coding/docs/archive/QUALITY_AUDIT_*.md:176,496,596` | mentions of TODO/FIXME as audit items (archived) |

No `FIXME`, `XXX`, or `HACK` markers in `crates/`.

## Not covered in this pass

Sub-module `//!` blocks below the top level (e.g. `gf2-core/src/field/*.rs`,
`gf2-coding/src/ldpc/**`, `gf2-sim/src/executor/*.rs`), item-level `///`
docs, complexity tables in `bch/{spec,encode,matrix}.rs`, and
`io/format.rs` type-tag values.
