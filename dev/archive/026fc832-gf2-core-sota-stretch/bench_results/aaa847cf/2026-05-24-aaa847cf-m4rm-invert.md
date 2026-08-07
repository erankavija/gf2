# M4RI-style invert path for BitMatrix — evidence

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `aaa847cf` (M4RI-style invert path for BitMatrix) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Closes A8 rows | 44 (n=64), 45 (n=256), 46 (n=1024) of scorecard `2026-05-08-2cfc4372-sota-scorecard.md` |
| Design note | `dev/active/aaa847cf-m4rm-invert-design.md` |
| Raw CSV | `dev/bench_results/2026-05-24-aaa847cf-m4rm-invert.csv` |

## Success criteria (verbatim from `jit issue show aaa847cf`)

- **[hard]** Implement an M4RM-table-augmented invert path in BitMatrix **OR** document why gf2-core's existing invert is structurally beyond catch-up.
- **[hard]** If the implementation lands, GF(2) invert at n in {64, 256, 1024} comes within 1.5x of M4RI `mzd_invert_m4ri`.
- **[hard]** Bit-exact equality with the existing reference invert; no regression on existing tests.
- **[hard]** Filed under epic 97bf0879 umbrella amendment A8 as the named follow-up for the 3 GF(2) invert FAIL cells.

## 1. Methodology (verbatim from `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 6, as quoted in `2026-05-24-a70b1c70-phase0-controls.md` § 1)

> All Wave-6B benchmarks were run on:
>
> - **CPU:** AMD Ryzen 9 5900X (Zen 3), 12c/24t, 3.7 GHz base / 4.6 GHz boost. AVX2 + BMI2 + VAES + VPCLMULQDQ. No AVX-512.
> - **Kernel:** Linux 7.0.3-arch1-1.
> - **Isolation:** `taskset -c 6-11 nice -n -5` (CCX1 pinned: cores 6-11, SMT siblings 18-23). Agent and parent shell on CCX0 (cores 0-5). Sequential trials (no concurrent benches).
> - **Toolchain:** rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1.
> - **Frequency governor:** powersave (no root to flip). Per-core boost enabled; reaches 4.6 GHz under load. Transient thermal ramps produce 1-2% per-iteration variance, handled by 5-trial median.
> - **Reference:** fflas-ffpack 2.5.0 + Givaro 4.2.0 in pinned container (`gf2-bench:ref`, sha256 in `benchmarks/image.lock`). Container built from Debian bookworm-20260421-slim. All container measurements are single-threaded (pinned-image protocol per `dev/plans/sota_reference_acceptance_protocol.md` § 5).

The recipe was followed verbatim. Reference: this evidence uses **M4RI 20260122** (`pkg-config --modversion m4ri = 20260122`) via the canonical
`benchmarks/reference/m4ri_bench` binary (built per `benchmarks/reference/Makefile`).
The gf2-core path runs from the same in-tree criterion bench
(`crates/gf2-core/benches/matrix_inversion.rs`) used in the original scorecard.

Cargo / shell invocations per trial:

```bash
# gf2-core (criterion median of 100 samples, 2 s measurement window):
taskset -c 6-11 nice -n -5 \
  target/release/deps/matrix_inversion-<hash> --bench \
    --warm-up-time 1 --measurement-time 2 'matrix_inversion/(64|256|1024)$'

# M4RI 20260122 (200 timed iterations after 3 warmup iterations):
taskset -c 6-11 nice -n -5 \
  benchmarks/reference/m4ri_bench --warmup 3 --iters 200 --seed <42 + trial>
```

5 trials per cell, distinct seeds across trials. Wall-time is recorded as
the criterion median (gf2) and the `mean_ns` field emitted by `m4ri_bench`
(M4RI). The reported per-cell number is the **median of medians** across
the 5 trials, the same aggregation policy used in `2026-05-24-a70b1c70`
§§ 3.2 and 4.2.

**Quiet host confirmed.** Pre-bench `pgrep -af 'cargo|rustc'` showed only
the in-tree `jit-server` daemons (which idle on the JIT data-dir and do
not run compute), the agent shell on CCX0, and no cargo or rustc build
processes. No competing cargo / rustc / benchmark ran during the 5-trial
windows.

## 2. Algorithm summary

`crates/gf2-core/src/alg/gauss.rs::invert_m4ri` runs Gauss–Jordan
elimination on the `n × 2n` augmented matrix `[A | I]` in column blocks of
width `k = default_block_size_invert(n)` (k = 4 for n ≤ 512, k = 8 for
n > 512). For each block it (i) finds `k` pivots inside the block, swapping
rows and clearing pivot bits within the k-pivot stripe, (ii) builds a
2ᵏ-entry Gray-code table of XOR combinations of the pivot rows restricted
to the trailing word suffix (`first_block_word..stride_words`), then
(iii) applies the table to every other row — both above the pivot stripe
(the Gauss–Jordan twist) and below — via a single `row_xor_slice_from`
per non-stripe row. The right half of `[A | I]` is then extracted as the
inverse with a word-aligned or shifted copy (handles `n % 64 ≠ 0` without
introducing padding-bit leakage; the result is tail-masked).
`pub fn invert` dispatches to this path for n ≥ `INVERT_M4RI_THRESHOLD = 8`
and to the unchanged scalar Gauss–Jordan (`invert_scalar`) below that
threshold, where the table setup is the dominant cost.

## 3. Per-cell results

### 3.1 Raw trials (criterion / m4ri_bench medians)

| trial | gf2 n=64 (µs) | gf2 n=256 (µs) | gf2 n=1024 (ms) | m4ri n=64 (µs) | m4ri n=256 (µs) | m4ri n=1024 (ms) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 5.7701 | 75.005 | 1.4258 | 8.938 | 71.995 | 1.1464 |
| 2 | 5.7095 | 75.084 | 1.4383 | 9.067 | 71.786 | 1.1212 |
| 3 | 5.7086 | 75.306 | 1.4527 | 8.951 | 71.483 | 1.1198 |
| 4 | 5.7553 | 75.158 | 1.4822 | 9.284 | 72.353 | 1.1238 |
| 5 | 6.2441 | 75.072 | 1.4606 | 9.090 | 73.117 | 1.1614 |

### 3.2 Aggregate (5-trial median of medians)

| n | gf2 median | M4RI median | ratio (gf2 / m4ri) | gap to 1.5x | verdict |
|---:|---:|---:|---:|---:|:---:|
| 64 | 5.755 µs | 9.067 µs | **0.635×** | 0.86× headroom | **PASS** |
| 256 | 75.084 µs | 71.995 µs | **1.043×** | 0.46× headroom | **PASS** |
| 1024 | 1.4527 ms | 1.1238 ms | **1.293×** | 0.21× headroom | **PASS** |

**All three criterion cells PASS the [hard] 1.5x ceiling.**

### 3.3 Versus the original FAIL state

Pre-rework numbers from scorecard § 2.3 (rows 210–212):

| n | scorecard gf2 | new gf2 | speedup |
|---:|---:|---:|---:|
| 64 | 40.450 µs | 5.755 µs | **7.0×** |
| 256 | 871.210 µs | 75.084 µs | **11.6×** |
| 1024 | 24.727 ms | 1.4527 ms | **17.0×** |

The 1024 speedup of 17× matches the predicted `log₂(1024) = 10` asymptotic
factor closely; the additional factor (~1.7×) comes from the Gray-table
SIMD path being shared with `m4rm::build_gray_table_flat` (already
AVX2-dispatched on this host).

## 4. Correctness validation

| Test | Location | Result |
|---|---|---|
| `test_invert_m4ri_matches_scalar_identity` | `crates/gf2-core/src/alg/gauss.rs` | PASS — boundary identity equality at n ∈ {0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129} |
| `test_invert_m4ri_matches_scalar_random` | `crates/gf2-core/src/alg/gauss.rs` | PASS — random invertible inputs at n ∈ {1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 200, 256} agree bit-exact with `invert_scalar` |
| `test_invert_m4ri_round_trips_via_multiply` | `crates/gf2-core/src/alg/gauss.rs` | PASS — A × A⁻¹ = I |
| `test_invert_m4ri_singular_returns_none` | `crates/gf2-core/src/alg/gauss.rs` | PASS — zero and duplicate-row inputs return None |
| `test_invert_dispatch_matches_explicit_paths` | `crates/gf2-core/src/alg/gauss.rs` | PASS — public `invert` matches `invert_scalar` below threshold and `invert_m4ri` at/above |
| `test_invert_non_square_returns_none` | `crates/gf2-core/src/alg/gauss.rs` | PASS — non-square inputs return None on all 3 paths |
| `test_invert_m4ri_bit_exact_with_scalar_on_boundaries` | `crates/gf2-core/tests/inversion.rs` | PASS — boundary identity oracle |
| `prop_invert_m4ri_equals_gauss` | `crates/gf2-core/tests/inversion.rs` (proptest, 256 cases default) | PASS — random GF(2) matrices at n ∈ {1, 7, 8, 9, 15, 16, 31, 32, 63, 64, 65, 127, 128, 129} agree bit-exact with the scalar reference, including both invertible and singular inputs |
| All existing `tests/inversion.rs` tests | `crates/gf2-core/tests/inversion.rs` | PASS — 12/12 pre-existing invert tests pass unchanged (they call the public `invert` symbol, which now dispatches to `invert_m4ri` for n ≥ 8) |

Full gf2-core test suite: **2029 tests passed, 11 skipped (slow/ignored), 0 failed.**

## 5. Open questions

None. All three criterion cells passed with comfortable headroom (n=64
ratio 0.635× is 0.86× below the 1.5× ceiling; n=256 ratio 1.043× is 0.46×
below; n=1024 ratio 1.293× is 0.21× below). The smallest headroom is at
n=1024 — if M4RI's wall jitters down by ~15% on a different host we'd be
at the boundary, but on the canonical CCX1-pinned 5900X measurement the
cell PASSes.

## 6. Files touched

- `crates/gf2-core/src/alg/gauss.rs` — replaced the prior single-path
  `invert` with a dispatcher; added `invert_m4ri`, `invert_scalar`,
  `INVERT_M4RI_THRESHOLD`, the block-elimination helpers
  (`find_block_pivot_invert`, `eliminate_block_full`,
  `block_table_index_invert`, `default_block_size_invert`), and inline
  unit tests for boundaries, random matrices, singular inputs, dispatch,
  and non-square inputs.
- `crates/gf2-core/tests/inversion.rs` — added `proptest!` block
  (`prop_invert_m4ri_equals_gauss`) and boundary-identity check.

No changes outside `gf2-core`. No new unsafe code (the Gray-table builder
and row-XOR kernels inherit the existing AVX2 dispatch through
`crate::kernels::ops::resolve_xor_inplace`).
