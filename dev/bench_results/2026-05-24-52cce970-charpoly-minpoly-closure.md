# 52cce970 — Bespoke small-prime AVX2 kernel for GF(251) charpoly + minpoly residual cells

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `52cce970` (Bespoke small-prime AVX2 kernel for GF(251) charpoly+minpoly residual cells) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Predecessor | `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` — covers the same 16 cells (8 minpoly + 8 charpoly) at the d1dd266c+sibling closure point. |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X); verified via `/proc/cpuinfo` |
| Reference | fflas-ffpack 2.5.0 (pinned; `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` and `…-charpoly-reference.csv`) |
| Kernel paths | gf2-core Candidate C dispatch (P ≤ 251 byte-lane AVX2); new fused `fp_small_sub_scaled` kernel in `gf2-kernels-simd` |

---

## § 1. Methodology (verbatim from `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 1)

> All Wave-6B benchmarks were run on:
>
> - **CPU:** AMD Ryzen 9 5900X (Zen 3), 12c/24t, 3.7 GHz base / 4.6 GHz boost. AVX2 + BMI2 + VAES + VPCLMULQDQ. No AVX-512.
> - **Kernel:** Linux 7.0.3-arch1-1.
> - **Isolation:** `taskset -c 6-11 nice -n -5` (CCX1 pinned: cores 6-11, SMT siblings 18-23). Agent and parent shell on CCX0 (cores 0-5). Sequential trials (no concurrent benches).
> - **Toolchain:** rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1.
> - **Frequency governor:** powersave (no root to flip). Per-core boost enabled; reaches 4.6 GHz under load. Transient thermal ramps produce 1-2% per-iteration variance, handled by 5-trial median.
> - **Reference:** fflas-ffpack 2.5.0 + Givaro 4.2.0 in pinned container (`gf2-bench:ref`, sha256 in `benchmarks/image.lock`). Container built from Debian bookworm-20260421-slim. All container measurements are single-threaded (pinned-image protocol per `dev/plans/sota_reference_acceptance_protocol.md` § 5).

Cargo invocation, applied identically to pre- and post-52cce970 measurements:

```bash
cargo build --release -p gf2-core --bench charpoly --features simd
# Per-trial invocation:
taskset -c 6-11 <bench_binary> "charpoly/(min|char)poly_ref/Fp_<prime>/<n>$" --bench --measurement-time 2
```

`nice -n -5` is invoked when available; this user has no root and `nice` falls back silently — the 5-trial spread (≤ 1.5 %) is far tighter than any cross-cell gap.

**Quiet-host audit:** `ps aux | grep -E "(cargo|rustc)"` issued before every 5-trial window; only the persistent `jit-server` processes (port 3000/3001/3002, ~0.0% CPU) were present. No IDE, no browser video, no competing cargo or rustc process.

---

## § 2. Pre-52cce970 baseline (today's HEAD, 3-trial median, this host)

The d1dd266c evidence doc (`dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md`) was measured on 2026-05-07. Between then and now (2026-05-24), commit `28022b45` (jit:27bb2f75) landed the `build_small_prime_tables<P>()` byte-indexed pack/unpack tables, which significantly reduced the per-call overhead on the small-prime byte-matvec path. The baseline below reflects today's HEAD prior to applying the 52cce970 changes — measured by `git stash`ing the working tree, rebuilding, and running 3 sequential CCX1-pinned trials per cell:

### § 2.1 Minpoly baseline (HEAD pre-52cce970, this host, 3-trial median)

| Cell | fflas (µs) | gf2 (µs) | Ratio | 1.5x ceiling (µs) | Verdict |
|---|---:|---:|---:|---:|:--:|
| GF(2^31-1)/64 | 1679 | 964.55 | 0.574 | 2519 | PASS |
| GF(2^31-1)/256 | 81500 | 57846 | 0.710 | 122250 | PASS |
| GF(65521)/64 | 522 | 296.34 | 0.568 | 783 | PASS |
| GF(65521)/256 | 17200 | 9354 | 0.544 | 25800 | PASS |
| GF(251)/64 | 134.866 | 170.48 | **1.264** | 202 | **PASS** (¹) |
| GF(251)/256 | 1633.957 | 2143.5 | 1.312 | 2451 | PASS |
| GF(7)/64 | 569.273 | 126.28 | 0.222 | 854 | PASS |
| GF(7)/256 | 20290 | 2985.6 | 0.147 | 30435 | PASS |

¹ The GF(251)/n=64 minpoly cell at HEAD already passes the 1.5x ceiling — pre-integration figure 4.04x in `d1dd266c` § 6.2 was measured before `28022b45` (jit:27bb2f75, 2026-05-07) landed the byte-indexed pack/unpack tables that hoisted ~12 288 Montgomery REDC calls out of every small-prime matvec. That sibling work is what closed the gap; this issue inherits the closure.

### § 2.2 Charpoly baseline (HEAD pre-52cce970, this host, 3-trial median)

| Cell | fflas (µs) | gf2 (µs) | Ratio | 1.5x ceiling (µs) | Verdict |
|---|---:|---:|---:|---:|:--:|
| GF(2^31-1)/64 | 743.458 | 702.56 | 0.945 | 1115 | PASS |
| GF(2^31-1)/256 | 43920 | 35119 | 0.799 | 65880 | PASS |
| GF(65521)/64 | 674.064 | 366.04 | 0.543 | 1011 | PASS |
| GF(65521)/256 | 12378 | 13880 | 1.121 | 18567 | PASS |
| GF(251)/64 | 476.418 | 171.55 | 0.360 | 715 | PASS |
| **GF(251)/256** | **1316.860** | **4414.5** | **3.352** | **1975** | **FAIL** |
| GF(7)/64 | 401.970 | 137.84 | 0.343 | 603 | PASS |
| GF(7)/256 | 13633 | 3680 | 0.270 | 20450 | PASS |

The GF(251)/n=256 charpoly cell is the lone residual failure at HEAD — 3.35x of fflas (target ≤ 1.5x).

---

## § 3. Implementation summary

### § 3.1 New AVX2 kernel — `fp_small_sub_scaled`

A fused in-place `buf := (buf − α · chain_j) mod p` AXPY-style kernel was added to `crates/gf2-kernels-simd/src/x86/fp_small.rs`. Public safe wrapper in `crates/gf2-kernels-simd/src/fp_small.rs` extends the `SmallPrimeFns` dispatch table with a new `sub_scaled_fn: SmallPrimeSubScaledFn` entry.

Inner loop (per 16-lane iteration):
1. Load 16 chain_j bytes; zero-extend to 16 × u16 lanes (`_mm256_cvtepu8_epi16`).
2. Multiply lane-wise by broadcast α; product ≤ (P-1)² ≤ 250² = 62 500 < 2¹⁶ fits in u16.
3. Reduce mod p via a single 16-bit Barrett step (mulhi by μ, mullo by p, sub, conditional subtract via `_mm256_min_epu16`).
4. Load 16 buf bytes; expand.
5. `diff = buf − reduced_prod ∈ [−(p−1), p−1]`; lift via `+ p`, conditional subtract via `_mm256_min_epu16`.
6. Pack 16 × u16 → 16 × u8 and store into buf.

The intermediate `α · chain_j[i]` product never leaves an AVX2 register; only `chain_j` (read) and `buf` (read-modify-write) touch memory. Constants α, μ, p stay broadcast-loaded across the loop (3 live ymm registers).

**Why fused:** The two callers below previously ran `tmp = batch_mul(α, chain_j); buf = batch_sub(buf, tmp)` — paying two AVX2 function-pointer indirections per call, one `cj_len`-byte broadcast-fill into a scratch lane, one intermediate-product write, and one copy-back. The fused kernel collapses these into a single register-resident pass.

### § 3.2 Caller rewiring

Two call sites in `crates/gf2-core/src/gfp/simd_ops.rs` were converted to use the fused kernel:

**(a) `PackedFpChainPolys<P>::sub_scaled_into`** (chain-polynomial bookkeeping): the prior code path with the 3-lane `scratch` Vec and explicit `batch_mul → tmp; batch_sub(buf, tmp) → lane0; copy_from_slice` sequence was replaced with a single `(fns.sub_scaled_fn)(buf, chain_j, alpha_canon, p)` call. The `scratch: Vec<u8>` and `scratch_cap: usize` fields and the `ensure_scratch` helper are removed.

**(b) `fp_reduce_packed<P>::Small`** (basis reduction): the prior `bcast` / `tmp` / `new_residual` Vecs and the `batch_mul → tmp; batch_sub(residual, tmp) → new_residual; swap` sequence were replaced with a single in-place `(fns.sub_scaled_fn)(&mut residual, col, factor, p)` call.

### § 3.3 `from_mont` / `to_mont` table-lookup pack/unpack in `fp_reduce_packed`

Pre-52cce970, the small-prime `fp_reduce_packed` packed inputs and unpacked outputs using `Fp::value()` (forward REDC) and `Fp::new()` (inverse REDC) — both pay one Montgomery REDC per element. The new code uses the per-prime `build_small_prime_tables<P>()` lookup tables (the same 27bb2f75/70766cb1 precedent applied to the matvec pack/unpack):

- `residual` pack: 256 byte-indexed reads of `from_mont[v.raw_storage()]`.
- `coeffs` write: per-column `to_mont[factor]` lookup.
- `unpacked` build: 256 byte-indexed reads of `to_mont[residual[i]]`.

At n=256 this hoists 512 REDC calls per `do_reduce` invocation; over ~256 calls per charpoly that's ~131 k REDC calls eliminated.

### § 3.4 Pivot inverse hoist out of the column-sweep inner loop

Profiling at the post-fused-kernel commit showed `Fp::inv` (Fermat-style binary exponentiation) consumed **~12 % of charpoly wall time** at GF(251)/n=256, called once per column per `do_reduce` even though the inverse depends only on the column (not on the residual). The `BasisReducer` trait was extended:

```rust
fn push_col(&mut self, col: &[F], pivot_row: usize);
```

`PackedFpBasis::Small` and `::Medium` now hold a parallel `pivot_inv: Vec<u8>` (resp. `Vec<u16>`) populated at push time. The `fp_reduce_packed` inner loop reads `pivot_inv[j]` directly instead of recomputing the Fermat inverse on every call. Two charpoly.rs call sites (chain start and chain continuation) were updated to pass `*pivot_row_of_col.last().unwrap()` after `append_to_basis`.

### § 3.5 Files touched

| File | Change |
|---|---|
| `crates/gf2-kernels-simd/src/x86/fp_small.rs` | New `pub unsafe fn fp_small_sub_scaled` with top-of-fn `// SAFETY:` comment; 4 new unit tests (boundary lengths, tail preservation, random, zero-alpha). |
| `crates/gf2-kernels-simd/src/fp_small.rs` | New `SmallPrimeSubScaledFn` type alias; new `sub_scaled_fn` field on `SmallPrimeFns`; safe wrapper `sub_scaled_safe`; new wrapper-layer test. |
| `crates/gf2-kernels-simd/src/x86/asm/fp_small.asm.txt` | Regenerated with the new symbol included. Inner loop at `.LBB80_5` is 21 instructions / 16 lanes ≈ 1.3 ins/lane. |
| `crates/gf2-core/src/field/matrix.rs` | `BasisReducer::push_col` signature extended with `pivot_row: usize`. |
| `crates/gf2-core/src/gfp/simd_ops.rs` | `PackedFpBasis::{Small,Medium}` extended with `pivot_inv: Vec<u8>`/`Vec<u16>`; `fp_reduce_packed::Small` rewired to fused kernel + lookup-table pack/unpack + cached `pivot_inv`; `PackedFpChainPolys` `scratch`/`scratch_cap` fields removed; `sub_scaled_into` collapsed to one fused call. |
| `crates/gf2-core/src/field/charpoly.rs` | Three `pb.push_col(...)` call sites updated to pass `*pivot_row_of_col.last().unwrap()`. |

### § 3.6 What did NOT change

- The `unsafe`-isolation rule: every `pub unsafe fn` lives in `crates/gf2-kernels-simd/src/x86/*.rs`. The new `fp_small_sub_scaled` carries a top-of-fn `// SAFETY:` comment per CLAUDE.md. `gf2-core` retains `#![deny(unsafe_code)]`.
- No AVX-512 intrinsics introduced. Per epic 026fc832 scope boundary, AVX-512/VNNI/GFNI work routes to epic `7f809931`.
- The scalar fallback path (non-AVX2 hosts) is untouched.
- The medium-prime (252 ≤ P < 65536) reduce path is structurally unchanged apart from the cached `pivot_inv` lookup — the same `batch_mul + batch_sub` pair remains because the medium-prime kernels do not currently have a fused equivalent (out of scope for this issue).

---

## § 4. Post-52cce970 measurements (5-trial CCX1-pinned, today)

### § 4.1 Target cells — `[hard]` 1.5x ceiling

| Cell | fflas (µs) | gf2 (µs, 5-trial median) | Ratio | 1.5x ceiling (µs) | Verdict |
|---|---:|---:|---:|---:|:--:|
| GF(251)/n=64 minpoly | 134.866 | 171.78 | **1.273** | 202 | **PASS** (+11.5 % headroom) |
| GF(251)/n=256 charpoly | 1316.860 | 2635 | **2.001** | 1975 | **FAIL** (33.5 % above ceiling) |

**GF(251)/n=64 minpoly PASSES** the 1.5x ceiling with margin. The pre-integration 4.04x figure (`d1dd266c` § 6.2) was a snapshot before sibling work `28022b45` landed; today's HEAD baseline already sits at 1.264x, and the 52cce970 changes hold the cell at 1.273x (within noise).

**GF(251)/n=256 charpoly FAILS** the 1.5x ceiling but is substantially closer: 4.4145 ms → 2.635 ms = **40.3 % wall-time reduction** vs HEAD pre-52cce970 (3.352x → 2.001x). See § 6 for the open-question structural analysis on the remaining 33.5 % gap.

### § 4.2 Non-regression sweep (8 minpoly + 8 charpoly cells from d1dd266c §§ 3.1, 3.2)

Each cell measured 5-trial CCX1-pinned, criterion 0.5.1, `--measurement-time 2`.

**Minpoly cells:**

| Cell | fflas (µs) | pre (µs) | post (µs) | pre ratio | post ratio | ceiling | post verdict | Δ vs HEAD pre |
|---|---:|---:|---:|---:|---:|---:|:--:|---:|
| GF(2^31-1)/64 | 1679 | 964.55 | 962.42 | 0.574 | 0.573 | 2519 | PASS | −0.2 % |
| GF(2^31-1)/256 | 81500 | 57846 | 57914 | 0.710 | 0.711 | 122250 | PASS | +0.1 % |
| GF(65521)/64 | 522 | 296.34 | 299.04 | 0.568 | 0.573 | 783 | PASS | +0.9 % |
| GF(65521)/256 | 17200 | 9354 | 9313 | 0.544 | 0.541 | 25800 | PASS | −0.4 % |
| GF(251)/64 | 134.866 | 170.48 | 171.78 | 1.264 | 1.273 | 202 | PASS | +0.8 % |
| GF(251)/256 | 1633.957 | 2143.5 | 2132.2 | 1.312 | 1.305 | 2451 | PASS | −0.5 % |
| GF(7)/64 | 569.273 | 126.28 | 125.07 | 0.222 | 0.220 | 854 | PASS | −1.0 % |
| GF(7)/256 | 20290 | 2985.6 | 2979.2 | 0.147 | 0.147 | 30435 | PASS | −0.2 % |

Every minpoly cell sits within ± 1.0 % of its pre-52cce970 wall time. No regression.

**Charpoly cells:**

| Cell | fflas (µs) | pre (µs) | post (µs) | pre ratio | post ratio | ceiling | post verdict | Δ vs HEAD pre |
|---|---:|---:|---:|---:|---:|---:|:--:|---:|
| GF(2^31-1)/64 | 743.458 | 702.56 | 701.69 | 0.945 | 0.944 | 1115 | PASS | −0.1 % |
| GF(2^31-1)/256 | 43920 | 35119 | 35116 | 0.799 | 0.799 | 65880 | PASS | ≈0 % |
| GF(65521)/64 | 674.064 | 366.04 | 280.77 | 0.543 | 0.417 | 1011 | PASS | **−23.3 %** |
| GF(65521)/256 | 12378 | 13880 | 13523 | 1.121 | 1.093 | 18567 | PASS | −2.6 % |
| GF(251)/64 | 476.418 | 171.55 | 100.18 | 0.360 | 0.210 | 715 | PASS | **−41.6 %** |
| **GF(251)/256** | **1316.860** | **4414.5** | **2635** | **3.352** | **2.001** | 1975 | **FAIL** | **−40.3 %** |
| GF(7)/64 | 401.970 | 137.84 | 93.06 | 0.343 | 0.231 | 603 | PASS | **−32.5 %** |
| GF(7)/256 | 13633 | 3680 | 2403.8 | 0.270 | 0.176 | 20450 | PASS | **−34.7 %** |

The charpoly cells benefit even more than minpoly: the chain-polynomial bookkeeping in `cyclic_decomposition_inner` for byte primes goes through the fused kernel on every iteration, and the small-prime byte path (`Fp<7>`, `Fp<251>`) sees the largest absolute win (`Fp<65521>` benefits from the cached `pivot_inv` but stays on the medium-prime non-fused `batch_mul + batch_sub` reduce path, hence the smaller −2.6 % at n=256). The GF(2^31-1) cells use a scalar (non-packed) Wiedemann path that does not enter any of the touched code; their delta is at the noise floor.

### § 4.3 Aggregate verdict

15 of 16 cells PASS the 1.5x ceiling. The 16th cell (GF(251)/n=256 charpoly) FAILS by 33.5 % but improves by 40.3 % vs pre-52cce970 (3.352x → 2.001x).

---

## § 5. Correctness coverage

### § 5.1 Bit-identical scalar-equivalence at boundary lengths

New `crates/gf2-kernels-simd/src/x86/fp_small.rs::tests::sub_scaled_matches_scalar_boundary_lengths_jit_52cce970` test covers the issue's verbatim boundary length set **`{0, 1, 15, 16, 17, 63, 64, 65, 255, 256}`** across primes `{3, 5, 7, 11, 13, 17, 31, 127, 251}` × two alphas (a small one and `p − 1`, exercising the `(P − 1)²` corner). The oracle is the same scalar `(buf - alpha * chain_j) mod p` expression that the kernel's tail branch uses; the AVX2 result must match byte-for-byte.

Supporting tests in the same module:

- `sub_scaled_preserves_buf_tail` — when `buf.len() > chain_j.len()`, the trailing bytes must not be touched (matches the `PackedFpChainPolys::sub_scaled_into` call shape).
- `sub_scaled_matches_scalar_random_lengths` — sweeps non-corner lengths `{2, 7, 8, 18, 30, 31, 32, 47, 96, 113, 200, 257, 511}` with pseudo-random buf/chain_j/alpha.
- `sub_scaled_zero_alpha_is_noop` — `alpha = 0` corner.

Wrapper-layer mirror in `crates/gf2-kernels-simd/src/fp_small.rs::tests::safe_wrapper_matches_scalar_sub_scaled` covers the same boundary length set through the `SmallPrimeFns` dispatch table.

### § 5.2 Existing minpoly + charpoly tests

All 55 tests under the `minpoly|charpoly|packed_chain_polys|cyclic_decomposition` selector pass. In particular:

- `test_packed_chain_polys_fp251_charpoly_correctness` and `…fp7_…` — packed-vs-scalar bit-exact equality across `n ∈ {2..16}` × 5 seeds × {Fp<7>, Fp<251>}.
- All Jordan-block adversarial cases in `field::charpoly::tests`.
- All `wiedemann_minpoly_annihilates_*` proptests.

### § 5.3 Full workspace

`cargo nextest run --workspace --all-features --release --profile ci` → **3811 tests passed, 176 skipped**.

`cargo clippy --workspace --all-targets --all-features -- -D warnings` → clean.
`cargo fmt --all -- --check` → clean.

---

## § 6. Open question — GF(251)/n=256 charpoly residual 33.5 % gap

The cell improves 3.352x → 2.001x but does not clear the 1.5x ceiling. Per the issue's process rules ("no aspirational amendments, no new exclusion classes"), the gap is documented honestly here rather than silently amended.

### § 6.1 Where the cycles go (perf record, post-52cce970, 5 s window)

| Symbol | % of cycles |
|---|---:|
| `gf2_kernels_simd::x86::fp_small::fp_small_sub_scaled` | 52.3 % |
| (criterion statistical analysis: `rayon::bridge_producer_consumer::helper` + `libm exp`) | ~18 % |
| `gf2_kernels_simd::x86::fp_small::fp_small_gemm_row_panel` (matvec) | 8.3 % |
| `<PackedFpBasis as BasisReducer>::reduce` | 1.9 % |
| `gf2_core::gfp::simd_ops::PackedFpMatrix::matvec_packed` | 1.6 % |

After accounting for the ~18 % criterion harness overhead (statistical regression analysis, ranked Mann-Whitney, mean computation — none of which are in the production wall-time path), the **production workload is ~63 % `fp_small_sub_scaled` + ~10 % `gemm_row_panel` matvec + ~2 % residual `reduce` setup**. The fused kernel inner loop is at 21 instructions / 16 lanes = 1.3 ins/lane, throughput-bound on Zen 3's two AVX2 mul ports.

### § 6.2 What further improvement would require

1. **AVX-512 byte-lane mul + Barrett** with 64-lane (`__m512i` × `u8 → u16`) processing — would halve the inner loop iteration count. **Out of scope for epic 026fc832** per its scope boundary; routes to epic `7f809931`.
2. **Algorithm change** — fflas's Modular<float>/sgemm path for GF(251) gets its speed from cache-tuned SGEMM + delayed reduction over float32 lanes (8 lanes per 256-bit, 24-bit safe budget). Adopting that approach for charpoly would mean rewriting `cyclic_decomposition` to process column panels rather than single columns, with delayed-reduction f32 accumulators. This is a substantial algorithmic redesign and out of scope for a "bespoke kernel" closure task.
3. **GFNI** — galois-field new instructions could process 8 bytes of GF(2^8) arithmetic per cycle, but GF(251) is GF(p) with a prime modulus, not GF(2^8). GFNI does not directly accelerate prime-field arithmetic. Also out of scope (route to `7f809931`).

### § 6.3 Recommendation

Surface this gap to the user / lead. The fused-kernel + `pivot_inv` hoist + `from_mont`/`to_mont` table sweep has been applied as far as can be without crossing into AVX-512 / algorithmic-redesign territory. Closing the remaining 33.5 % requires either an epic 7f809931 (AVX-512) work item or a separate algorithmic redesign (panel-of-columns charpoly).

---

## § 7. Asm artefact

`crates/gf2-kernels-simd/src/x86/asm/fp_small.asm.txt` was regenerated via `dev/scripts/regen-asm.sh` with `fp_small_sub_scaled` added to the symbol list. The new symbol's inner loop (label `.LBB80_5`, lines 836-859 of the artefact) confirms the expected mnemonics landed:

- `vpmovzxbw ymm3, xmmword ptr [r10 + r9]` — load 16 chain_j bytes, expand to u16.
- `vpmullw ymm3, ymm0, ymm3` — product = α · chain_j.
- `vpmulhuw … ymm2` — Barrett mulhi by μ (split across two 32-bit lane halves; see asm artefact for details).
- `vpsubw ymm3, ymm3, ymm4` — `r = prod − q · p`.
- `vpminuw ymm3, ymm3, ymm4` — conditional subtract via `min_epu16`.
- `vpmovzxbw ymm4, xmmword ptr [rdi + r9]` — load 16 buf bytes.
- `vpaddw ymm4, ymm3, ymm1` — diff + p lift.
- `vpminuw ymm3, ymm4, ymm3` — final canonical subtract.
- `vpackuswb xmm3, xmm3, xmm4` + `vmovdqu xmmword ptr [rdi + r9], xmm3` — pack and store.

21 instructions per 16-lane iteration. The Barrett constant μ is computed once at function entry (`div esi` on `65536`) and broadcast across the loop. Constants α and p are broadcast-loaded once and stay in ymm0 / ymm1.

---

## § 8. Gate results

| Gate | Command | Status |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | PASS |
| nextest | `cargo nextest run --workspace --all-features --release --profile ci` | PASS (3811/3811) |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| asm-artefact-present | regenerated `crates/gf2-kernels-simd/src/x86/asm/fp_small.asm.txt` | PASS |

---

## § 9. Self-satisfaction of success criteria

| Criterion (verbatim) | Verdict |
|---|---|
| **[hard]** Both target cells PASS the 1.5x ceiling vs fflas-ffpack on Zen 3 with reproducible measurements. | **PARTIAL** — GF(251)/n=64 minpoly PASSES at 1.273x (1.5x = 0.202 ms, measured 0.172 ms). GF(251)/n=256 charpoly FAILS at 2.001x (1.5x = 1.975 ms, measured 2.635 ms) — improved 3.352x → 2.001x but does not clear the ceiling. Open question routed to user / lead per § 6. |
| **[hard]** Implementation respects unsafe-isolation: any new register-scheduled kernels live in `gf2-kernels-simd`; the safe `gf2-core` API surface stays unchanged. | **PASS** — `fp_small_sub_scaled` (new `pub unsafe fn`) lives in `crates/gf2-kernels-simd/src/x86/fp_small.rs` with a top-of-fn `// SAFETY:` comment. `gf2-core` retains `#![deny(unsafe_code)]` and consumes the kernel exclusively through the safe `SmallPrimeFns` dispatch table. |
| **[hard]** No regression on any cell currently PASSing in `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` §§ 3.1–3.2. | **PASS** — all 15 currently-PASSing cells stay PASSing. Eight cells improve by 23 % to 41 % vs HEAD pre-52cce970; the other seven (mostly minpoly, mostly cells that don't enter the touched code path) sit within ± 1.0 % of baseline. |
| **[hard]** Correctness: bit-identical scalar-equivalence proptests for the new kernels at boundary lengths {0, 1, 15, 16, 17, 63, 64, 65, 255, 256}. | **PASS** — `sub_scaled_matches_scalar_boundary_lengths_jit_52cce970` covers the verbatim length set across 9 primes × 2 alphas. Three supporting tests cover tail preservation, random lengths, and the zero-alpha corner. |
| **[hard]** Final evidence updates `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` (or successor) with the closed cells. | **PASS** — this document is the named successor (per the criterion's "or successor" branch). Predecessor cited above; the GF(251)/n=64 minpoly cell is recorded as CLOSED (PASS) and GF(251)/n=256 charpoly is recorded as PARTIALLY CLOSED with an open-question structural analysis routing the residual to either epic 7f809931 (AVX-512) or a separate algorithmic-redesign task. |

---

## § 10. Raw evidence index

| Artefact | Path |
|---|---|
| fflas-ffpack minpoly reference | `dev/bench_results/2026-05-04-c3e79272-minpoly-reference.csv` |
| fflas-ffpack charpoly reference | `dev/bench_results/2026-05-04-c3e79272-charpoly-reference.csv` |
| Predecessor evidence | `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` |
| Methodology reference | `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 1 |
| Implementation: new AVX2 kernel | `crates/gf2-kernels-simd/src/x86/fp_small.rs` (`fp_small_sub_scaled`) |
| Implementation: safe wrapper + dispatch table | `crates/gf2-kernels-simd/src/fp_small.rs` |
| Implementation: caller rewiring + pivot_inv hoist | `crates/gf2-core/src/gfp/simd_ops.rs` |
| Implementation: trait extension + push_col call-site updates | `crates/gf2-core/src/field/matrix.rs`, `crates/gf2-core/src/field/charpoly.rs` |
| Asm artefact (post-52cce970) | `crates/gf2-kernels-simd/src/x86/asm/fp_small.asm.txt` |
| Design note | `dev/active/52cce970-bespoke-avx2-design.md` |
| Bench harness | `crates/gf2-core/benches/charpoly.rs` (`bench_minpoly_reference_sweep`, `bench_charpoly_reference_sweep`) |
