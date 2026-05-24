# 52cce970 — Bespoke AVX2 design note

## Target cells (verbatim from issue)

| Cell | Pre-integration | Post-integration | 1.5x ceiling | Gap |
|---|---:|---:|---:|---:|
| GF(251)/n=64 minpoly | 4.04x | 2.84x | 0.202 ms | 1.9x past ceiling |
| GF(251)/n=256 charpoly | 9.58x | 3.18x | 1.975 ms | 2.1x past ceiling |

## Fresh 2026-05-24 baseline (HEAD, 5-trial CCX1-pinned)

Methodology verbatim per `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 1.
Bench binary: `charpoly-465318bd5a6ce711`, `taskset -c 6-11`, `--measurement-time 3`,
criterion 0.5.1 default 10-sample window per trial.

| Cell | gf2 wall (5-trial median of medians) | fflas wall | Ratio | 1.5x ceiling | Verdict |
|---|---:|---:|---:|---:|:---:|
| GF(251)/n=64 minpoly | 171.11 µs | 134.866 µs | **1.269x** | 0.202 ms | **PASS** |
| GF(251)/n=256 charpoly | 4.384 ms | 1.317 ms | **3.33x** | 1.975 ms | FAIL |

The GF(251)/n=64 minpoly cell **already passes** at HEAD. The pre-integration
4.04x figure from `dev/bench_results/2026-05-07-d1dd266c-minpoly-tuning.md` was
measured before commit `28022b45` (jit:27bb2f75) landed the
`build_small_prime_tables<P>()` byte-indexed pack/unpack tables, which removed
12 288 Montgomery REDC operations per matvec call. That optimisation was the
sibling-task gap-closer for the cell.

Only GF(251)/n=256 charpoly remains as a hard pass/fail gate for this issue.
Per the verbatim issue description, the residual gap is "chain_polys
`sub_scaled_into` per-call function-pointer indirection + Barrett-constant
load. fflas fuses the chain_polys update with the matvec inner loop."

## § 6.4 root-cause analysis (from d1dd266c)

> Charpoly runs `charpoly_cubic` → `cyclic_decomposition` → product of block polys.
> For random matrices the cyclic_decomposition is single-block of length n,
> and the inner loop's polynomial-bookkeeping (each `chain_polys[k]` update
> is `O(k)` field operations × `O(k)` substitutions) is `O(n³)` Montgomery
> muls in the scalar arm.
>
> `5a3dbd5b` replaced the scalar Montgomery polynomial-bookkeeping with
> canonical-byte arithmetic (`PackedFpChainPolys<P>`) using AVX2 `batch_mul`
> + `batch_sub`, eliminating the ~16M Montgomery REDC operations per call
> for `Fp<P>` with `P ≤ 251`. Wall time dropped 12.61 ms → 4.20 ms.
> The remaining constant-factor gap is the per-call AVX2 byte-lane operation
> overhead at the chain_polys boundary.

`sub_scaled_into` (`crates/gf2-core/src/gfp/simd_ops.rs:1913`) is called
~n²/2 times per charpoly with average operand length n/2 (in the single-block
random-matrix case). At n=256, that's ~32 700 calls. Each call currently:

1. Reads `from_mont[alpha.raw_storage()]` → canonical byte. (1 ld)
2. Resizes `self.scratch` to 3 × max_cj_len (one-shot at chain start).
3. Copies `self.polys[j]` (length cj_len) into scratch lane 1. (cj_len B copy)
4. Fills `self.scratch[..cj_len]` with broadcast alpha. (cj_len B memset)
5. Calls `batch_mul_fn` (fn-pointer indirect call) → scratch lane 2 = lane0 × lane1.
6. Calls `batch_sub_fn` (fn-pointer indirect call) → scratch lane 0 = buf − lane2.
7. Copies scratch lane 0 back to buf.

Inside each kernel call:
- AVX2 prologue saves/restores; barrett constant loaded; tail handling on each.
- batch_mul reduces mod p (full Barrett step) for each 16-lane block.
- batch_sub then re-canonicalises after subtract.

Per call overhead (independent of cj_len): ~3 function-pointer prologues + 3 copies + 2 Barrett constants reloaded + 2 separate kernel asm prologues. At small cj_len (chain growing from 0 to n), the per-call constant cost dominates over the SIMD body.

## Design — bespoke fused kernel

### `fp_small_sub_scaled` (new public AVX2 entry point)

Signature:
```rust
#[target_feature(enable = "avx2")]
pub unsafe fn fp_small_sub_scaled(buf: &mut [u8], chain_j: &[u8], alpha: u8, p: u8)
```

Semantics: `buf[i] := (buf[i] - alpha * chain_j[i]) mod p` for `i in 0..chain_j.len()`.

Invariants:
- `chain_j.len() <= buf.len()` (asserted).
- All bytes in `[0, p)`.
- `p` an odd prime in `[3, 251]`.
- `alpha < p` (canonical).

Inner loop (16 lanes per iteration):
1. Load 16 bytes from `chain_j` into `__m128i`, zero-extend to `__m256i` of 16 u16 lanes (`_mm256_cvtepu8_epi16`).
2. Multiply by broadcast `alpha` (16-bit vector): `_mm256_mullo_epi16`. Product ≤ (P−1)² = 62500 < 2¹⁶.
3. Single 16-bit Barrett step → canonical product lane in `[0, p)`. (mulhi by μ; mullo by p; sub.)
4. Load 16 bytes from `buf` (same offset), zero-extend.
5. `diff = buf - reduced_prod` ∈ `[-(p-1), p-1]`. Add p to lift into `[1, 2p-1]`. Conditional subtract → `[0, p)`.
6. Pack 16 u16 → 16 u8 (single 128-bit lane, no permute needed because we use `_mm_packus_epi16` on the 256-bit reduced via `_mm256_extracti128_si256<1>` + low-half).
7. Store 16 bytes back to `buf`.

Scalar tail: classic `(a + (p - mul(alpha, c))) % p` per byte.

This fuses what was previously two distinct AVX2 calls (batch_mul → scratch; batch_sub → scratch) plus two intermediate buffer hops into a single read-modify-write pass.

**Estimated savings per `sub_scaled_into` call:**
- 2 fn-pointer indirections → 1.
- 1 broadcast memset (cj_len B) eliminated — broadcast lives in a register.
- 1 intermediate scratch write + scratch read eliminated — the product is in a register when it's needed for the subtract.
- 1 copy-back step (cj_len B from scratch to buf) eliminated.

At chain length d=128 (mid-decomposition, n=256), each call had ~5 cj_len-sized memory passes; the fused kernel has 2 (one buf read+write, one chain_j read). That's a 2.5× reduction in memory traffic per call on a path that is heavily call-overhead-bound.

### Caller-side wiring

`PackedFpChainPolys::sub_scaled_into` (in `crates/gf2-core/src/gfp/simd_ops.rs`):
- Drops the `scratch` Vec and the `scratch_cap` field. (Confirmed unused elsewhere.)
- Calls a new safe wrapper `fns.sub_scaled_fn(buf, &self.polys[j], alpha_val, P as u8)` exposed through `SmallPrimeFns`.

### Non-AVX2 hosts

`detect_x86` returns `None` already on non-AVX2 hosts, so `try_new` returns `None` and the scalar fallback in `cyclic_decomposition_inner` (the `else` branch at `charpoly.rs:547`) handles it. No new code path needed.

### What we are NOT doing

- **No AVX-512.** Per CLAUDE.md and epic 026fc832 scope boundary, AVX-512 / VNNI / GFNI is routed to epic `7f809931`.
- **No register-blocked panel-iteration rewrite for n=64 minpoly.** That cell already PASSES at 1.269x; the issue is closed by the sibling work and a fresh measurement.

### TDD proptest plan

New proptest in `crates/gf2-kernels-simd/src/x86/fp_small.rs::tests`:
`sub_scaled_matches_scalar`. Boundary lengths verbatim per issue criterion:
`{0, 1, 15, 16, 17, 63, 64, 65, 255, 256}`. Covers primes `{3, 5, 7, 11, 13, 17, 31, 127, 251}` (same set as existing tests in the module). Each (p, len) pair: fixed-seed PRNG inputs, scalar oracle `(buf - alpha * c) mod p`, AVX2 result must match byte-for-byte.

### Register schedule (Zen 3)

Inside the inner loop the kernel uses (per 16-lane iteration):
- `ymm0` — broadcast `alpha` (live across the loop, never reloaded)
- `ymm1` — broadcast `μ` Barrett constant (live across the loop)
- `ymm2` — broadcast `p` (live)
- `ymm3` — `chain_j` 16-byte block expanded to u16
- `ymm4` — product = `_mm256_mullo_epi16(ymm3, ymm0)`
- `ymm5` — q = `_mm256_mulhi_epu16(ymm4, ymm1)`
- `ymm6` — `qp = _mm256_mullo_epi16(ymm5, ymm2)`
- `ymm4` — `r = ymm4 - ymm6`  (reusing ymm4)
- `ymm6` — `r_minus_p = ymm4 - ymm2`
- `ymm4` — `_mm256_min_epu16(ymm4, ymm6)` (canonical reduced product)
- `ymm7` — `buf` block expanded
- `ymm7` — `diff = ymm7 - ymm4`
- `ymm7` — `shifted = diff + ymm2`
- `ymm6` — `minus_p = ymm7 - ymm2`
- `ymm7` — `_mm256_min_epu16(ymm7, ymm6)` (canonical [0, p))
- pack low half + store

Three live constants (alpha, μ, p) stay in ymm0/ymm1/ymm2 across the entire vectorised loop. The remaining ymm3..7 cycle through the per-iteration data. Total dependency chain length per lane ≈ 7 instructions: load → cvt → mul → mulhi → mullo → sub → min → load → sub → add → sub → min → pack → store. Most of these are 1-cycle on Zen 3 except mulhi/mullo (5-cycle latency); Zen 3 can issue two AVX2 mul-class ops per cycle so we expect throughput-bound, not latency-bound, performance at sustained lengths.

## Risk + verification

- Correctness — proptest at boundary lengths {0..256} × 9 primes × random data; existing 3277 workspace tests must still pass.
- ASM artefact — `crates/gf2-kernels-simd/src/x86/asm/fp_small.asm.txt` regenerated with `fp_small_sub_scaled` added to the symbol list.
- Non-regression — measure GF(2^31-1), GF(65521), GF(251), GF(7) at n ∈ {64, 256} for both minpoly and charpoly (the §§ 3.1, 3.2 cells from d1dd266c).
- Quiet-host audit — `ps aux | grep cargo` before each 5-trial window.
