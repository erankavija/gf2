# Markowitz-degree sparse RREF — evidence (`jit:5ce13bae`)

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `5ce13bae` (Markowitz-degree pivot selection for sparse RREF) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Umbrella reference | `97bf0879` amendment A8 (10 sparse-elim FAIL cells from `2026-05-08-2cfc4372-sota-scorecard.md` § 4.4) |
| Predecessor evidence | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` § 4 #5 (gf2-core uniformly 0.38x-0.51x of LinBox) |
| Design note | `dev/active/5ce13bae-markowitz-design.md` |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X, 12c/24t); verified via `/proc/cpuinfo` |
| Toolchain | rustc 1.95.0 (59807616e 2026-04-14); cargo 1.95.0 |
| Reference | LinBox 1.7.1 `GaussDomain::NoReordering` (`pkg-config --modversion linbox` = 1.7.1) baselines reused verbatim from `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` |
| Raw CSV | `dev/bench_results/2026-05-24-5ce13bae-markowitz-sparse-rref.csv` (5 trials × 10 cells) |

---

## 1. Methodology (verbatim recipe from `dev/bench_results/2026-05-24-a70b1c70-phase0-controls.md` § 1)

> All Wave-6B benchmarks were run on:
>
> - **CPU:** AMD Ryzen 9 5900X (Zen 3), 12c/24t, 3.7 GHz base / 4.6 GHz boost. AVX2 + BMI2 + VAES + VPCLMULQDQ. No AVX-512.
> - **Kernel:** Linux 7.0.3-arch1-1.
> - **Isolation:** `taskset -c 6-11 nice -n -5` (CCX1 pinned: cores 6-11, SMT siblings 18-23). Agent and parent shell on CCX0 (cores 0-5). Sequential trials (no concurrent benches).
> - **Toolchain:** rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1.
> - **Frequency governor:** powersave (no root to flip). Per-core boost enabled; reaches 4.6 GHz under load. Transient thermal ramps produce 1-2% per-iteration variance, handled by 5-trial median.
> - **Reference:** fflas-ffpack 2.5.0 + Givaro 4.2.0 in pinned container (`gf2-bench:ref`, sha256 in `benchmarks/image.lock`). Container built from Debian bookworm-20260421-slim. All container measurements are single-threaded (pinned-image protocol per `dev/plans/sota_reference_acceptance_protocol.md` § 5).

The recipe was followed verbatim with one host-environment exception: `nice -n -5` was not applied because this session does not have root privilege to raise process priority. CCX1 pinning via `taskset -c 6-11` was applied for every trial; this is the load-bearing isolation control. Per-trial invocation (Cargo example, not Criterion bench harness because the existing `bench_sparse_csv_emitter` already wires all 10 cells with byte-identical inputs to the LinBox reference):

```bash
cargo build --release -p gf2-coding --example bench_sparse_csv_emitter --features bench-csv
for trial in 1 2 3 4 5; do
  taskset -c 6-11 ./target/release/examples/bench_sparse_csv_emitter \
    --filter "sparse-elim" --warmup 1 --iters 3 \
    --output /tmp/markowitz-bench/trial${trial}.csv
done
```

Wall_ns per cell is the mean over 3 iterations (post-warmup) within each trial; the 5-trial median across these per-trial means is the headline number. The Bernoulli matrix support is derived from the same SplitMix64 master seed and `derive_seed("spelim-er", 3, si, 1)` rule that `linbox_sparse_bench.cpp:269-271` uses, so the input is byte-identical to what LinBox measured for the published baseline.

**Quiet-host verification.** Before each trial, `ps -eo pid,comm,stat | grep -E "cargo|rustc|clippy|ld\b|cc1"` returned no rows (other than a benign kernel kworker). No IDE, browser video, or competing cargo process was running during the 5-trial window. The trials completed in approximately 60 seconds wall-clock.

---

## 2. Design summary

The previous straight-line column-sweep sparse RREF (gf2-core HEAD pre-`5ce13bae`) processed columns in ascending order, picking the first un-used row whose leading entry matched the current column. This produced canonical RREF (modulo a pre-existing bug in `FieldMatrix::rref` — see § 6) but did nothing to control fill-in during elimination.

LinBox `GaussDomain::NoReordering` uses a Markowitz-degree priority queue: at each step it picks the (row, col) pair minimising `(row_nnz - 1) * (col_nnz - 1)` — the structural upper bound on the size of the resulting Schur-complement block. This minimises fill-in across the elimination, keeping intermediate sparse representations compact.

The Markowitz pivot selection introduced here adds a canonical-RREF constraint absent from LinBox's algorithm. Because the canonical RREF pivot column set is uniquely determined (leftmost linearly-independent columns), the implementation iterates pivots in ascending column order, and within each column picks the un-used row with minimum `row_nnz` (col_nnz is identical across candidates at a fixed column, so the Markowitz product collapses to "minimise row_nnz"). The `row_nnz` array is maintained incrementally during each axpy — re-scanning the matrix would destroy the speedup. Dependent rows whose `row_nnz` drops to zero drop out of pivot search automatically, mirroring LinBox's early-out on dependent rows.

Both target methods (`SpBitMatrix::rref` and `SparseFieldMatrix::rref`) were rewritten in this discipline; their public APIs are unchanged. Full design rationale: `dev/active/5ce13bae-markowitz-design.md`.

---

## 3. Per-cell PASS/FAIL table (5-trial median, 1.5x threshold against LinBox baselines)

LinBox baselines: `dev/bench_results/2026-05-04-47698404-sparse-reference.csv` (LinBox 1.7.1 `GaussDomain::NoReordering`, byte-identical seed walk).

Pre-Markowitz numbers: `dev/bench_results/2026-05-04-47698404-sparse.csv` (gf2-core HEAD at `0d6ca3b6`, straight-line sparse Gauss-Jordan).

The contract for `[hard]` criterion 2 ("Cross-ratios... come within 1.5x of LinBox `GaussDomain::NoReordering`") is `ratio = LinBox_wall / gf2_wall >= 1/1.5 = 0.667`, i.e. gf2-core throughput >= (2/3) of LinBox throughput.

| Field | n | gf2 wall (ns, median) | IQR (ns) | LinBox wall (ns) | pre-Markowitz wall (ns) | ratio vs LinBox | speedup vs pre | verdict |
|---|---:|---:|---:|---:|---:|---:|---:|:---:|
| GF(2) | 256 | 5,708,398 | 177,630 | 4,465,081 | 9,593,009 | **0.782** | 1.68x | **PASS** |
| GF(2) | 1024 | 306,179,171 | 541,060 | 228,006,362 | 505,269,928 | **0.745** | 1.65x | **PASS** |
| GF(7) | 256 | 15,924,471 | 19,143 | 8,217,269 | 21,423,416 | **0.516** | 1.35x | **FAIL** |
| GF(7) | 1024 | 715,128,227 | 760,067 | 478,156,753 | 1,112,513,938 | **0.669** | 1.56x | **PASS** |
| GF(251) | 256 | 11,796,536 | 12,493 | 7,127,422 | 16,761,361 | **0.604** | 1.42x | **FAIL** |
| GF(251) | 1024 | 485,371,155 | 506,380 | 363,645,551 | 776,362,225 | **0.749** | 1.60x | **PASS** |
| GF(65521) | 256 | 10,950,393 | 13,503 | 6,871,125 | 15,643,864 | **0.627** | 1.43x | **FAIL** |
| GF(65521) | 1024 | 459,827,134 | 310,797 | 363,647,751 | 717,241,325 | **0.791** | 1.56x | **PASS** |
| GF(2^31-1) | 256 | 10,513,626 | 13,550 | 7,570,109 | 16,209,991 | **0.720** | 1.54x | **PASS** |
| GF(2^31-1) | 1024 | 427,719,445 | 109,410 | 366,749,558 | 754,420,032 | **0.857** | 1.76x | **PASS** |

**Aggregate:** 7 of 10 cells PASS the 1.5x contract; 3 of 10 FAIL (all at n=256, all GF(p) — see § 5 open question).

The Markowitz path delivers a uniform **1.35x-1.76x speedup over the pre-Markowitz baseline** across all 10 cells. The relative gap to LinBox closes from the uniform 0.38x-0.51x reported in `2026-05-04-47698404-sparse-scorecard.md` § 3 (sparse-elim table) to 0.52x-0.86x — a substantial improvement, with PASS verdicts on every n=1024 cell and 2 of 5 n=256 cells.

---

## 4. Correctness validation

The Markowitz path is bit-exact with the canonical RREF reference. Validation chain:

1. **Existing sparse-rref tests (`gf2-core` `cargo nextest`)** — All 22 pre-existing tests (`*rref*` in `crates/gf2-core/src/sparse.rs::tests` and `crates/gf2-core/src/field/sparse_matrix.rs::tests`) PASS without modification. These test against `dense_rref_reference` / `FieldMatrix::rref` on their own seed-shape choices. Run command + result:
   ```
   $ cargo nextest run -p gf2-core --release -E 'test(sparse) and test(rref)' --profile ci
   Summary [   0.323s] 22 tests run: 22 passed, 1951 skipped
   ```

2. **New Markowitz-specific tests (`jit:5ce13bae`)** — 9 new tests added; all pass:
   - `crates/gf2-core/src/sparse.rs`:
     - `proptest_rref_markowitz_byte_equality` (a 256-case proptest covering rows ∈ [0, 65], cols ∈ [0, 65], entry_count ∈ [0, 200], with idempotence + dense-reference byte-equality assertions).
   - `crates/gf2-core/src/field/sparse_matrix.rs`:
     - `test_rref_markowitz_1x1_single_entry_fp7` (1x1 corner)
     - `test_rref_markowitz_tall_deficient_fp7` (8x4 rank-deficient)
     - `test_rref_markowitz_wide_fp7` (4x12 wide)
     - `test_rref_markowitz_word_boundary_n64_fp7` (64x64, very sparse)
     - `test_rref_markowitz_word_boundary_n65_fp65521` (65x65 over GF(65521))
     - `test_rref_markowitz_sweep_fp7` (32 seeds × 7 shapes × 5 densities for Fp<7>; includes 0x0, 1x1, 3x5, 5x3, 8x8, 15x17, 24x24)
     - `test_rref_markowitz_sweep_fp65521` (16 seeds × 4 shapes × 3 densities for Fp<65521>)
     - `test_rref_markowitz_sweep_g8` (16 seeds × 3 shapes × 3 densities for `Gf2mWide<1, AES>`)

3. **Independent oracle.** The GF(p) sweep tests use an in-test `direct_rref_reference_fp` (and `_g8`) — a textbook column-by-column Gauss-Jordan with above-pivot elimination producing canonical RREF. This is independent of both the Markowitz path under test and the dense `FieldMatrix::rref` reference. See § 6 for why this independent oracle was necessary.

4. **Full gf2-core / workspace suites.**
   ```
   $ cargo nextest run -p gf2-core --release --all-features --profile ci
   Summary [   0.985s] 2021 tests run: 2021 passed, 11 skipped

   $ cargo nextest run --workspace --all-features --release --profile ci
   Summary [   3.929s] 3798 tests run: 3798 passed, 176 skipped
   ```

5. **Formatter + clippy.**
   ```
   $ cargo fmt --all -- --check    # clean after a final fmt pass
   $ cargo clippy --workspace --all-targets --all-features -- -D warnings
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.92s   # zero warnings
   ```

The Markowitz path's correctness is independently witnessed by: (a) the textbook direct Gauss-Jordan oracle, (b) the dense reference on inputs where the dense reference is itself canonical, and (c) the GF(2) sparse rref's idempotence (`RREF(RREF(M)) == RREF(M)`).

---

## 5. Open question — 3 n=256 GF(p) FAIL cells

| Field | n | gf2 wall (ns) | LinBox wall (ns) | ratio | gap to 0.667 |
|---|---:|---:|---:|---:|---:|
| GF(7) | 256 | 15,924,471 | 8,217,269 | 0.516 | -0.151 |
| GF(251) | 256 | 11,796,536 | 7,127,422 | 0.604 | -0.063 |
| GF(65521) | 256 | 10,950,393 | 6,871,125 | 0.627 | -0.040 |

These 3 cells deliver 1.35x-1.43x speedups over the pre-Markowitz baseline but still don't quite close the 1.5x gap to LinBox at n=256. The pattern is uniform: all 3 are GF(p) (not GF(2)), all at the smaller `n=256` size with the denser regime `density=3.91e-2`. At `n=1024` (with the sparser `density=9.77e-3`), every cell PASSes including these three primes.

Plausible root causes:

1. **Per-pivot constant overhead.** At n=256 density 3.9%, the matrix has ~2,621 non-zeros and rank ~256. The Markowitz pivot search is O(m) per pivot, contributing ~256 * 256 = 65,536 row scans for the full elimination. At n=1024 density 0.98%, ~10,485 nnz and rank ~1024, contributing ~1024 * 1024 = 1,048,576 row scans — but the elimination work scales as O(rank * fill-in * row_weight), so the pivot-search overhead is a smaller fraction at n=1024.

2. **GF(p) scalar arithmetic.** GF(7), GF(251), GF(65521) all use Montgomery `Fp::mul` per element via `core::ops::Mul`. LinBox `GaussDomain<Modular<int64_t>>` uses delayed-reduction MAC. At small n, the constant-factor overhead of Montgomery per multiply dominates.

3. **PriorityQueue vs argmin scan.** LinBox uses an actual priority queue keyed on Markowitz product across (row, col) pairs; this implementation does a linear `argmin` scan over un-used rows, which is O(m) per pivot. Switching to a priority queue would reduce the per-pivot search to O(log m) but adds maintenance overhead at each axpy (each row whose row_nnz changes needs a queue update).

This is **not a correctness gap**; it is a performance gap of 0.04-0.15 below threshold on 3 of 10 cells. The 1.5x contract on those 3 cells is the only `[hard]` item not yet met by this issue. Closing them would likely require one or more of:

- (a) Lazy-reduction MAC for GF(p) axpy inner loops (small-prime path).
- (b) Sparse priority queue for pivot search at small n.
- (c) Block elimination at small n (combine multiple pivots per pass).

Each is a separate, scoped implementation effort. The lead should decide whether to amend the `[hard]` contract to `[aspirational]` for these 3 cells (justification: 1.35x-1.43x speedup achieved; n=256 dense regime is structurally harder for any general sparse method; same 3 cells fail under any plausible Markowitz-only intervention), or to file follow-up issues for (a)-(c) under the same 97bf0879 amendment A8.

---

## 6. Open question — pre-existing `FieldMatrix::rref` canonical divergence

During Markowitz test development, a corner-case input (15×17 GF(7), density 0.05, seed=1) was discovered where `FieldMatrix::rref` (the dense PLE-based RREF in `crates/gf2-core/src/field/ple.rs`) produces a row-echelon form that is NOT the canonical RREF: it picks a non-leftmost pivot column set on inputs where rank-deficient PLE recursion exhibits a specific row-permutation pattern. Concretely on the reproducer seed:

- Canonical RREF (uniquely determined): pivots `{0, 1, 2, 3, 5, 6, 7, 10, 15, 16}`.
- `FieldMatrix::rref` output: pivots `{0, 1, 2, 3, 5, 6, 7, 13, 15, 16}`.

Both are RREFs of *some* row-equivalent matrix, but only the first is the canonical RREF of the input. The Markowitz sparse path correctly produces the canonical form; the dense PLE path does not.

This is **out of scope for `5ce13bae`** (which only touches `SpBitMatrix::rref` and `SparseFieldMatrix::rref`). The pre-existing tests that compare sparse vs dense do not catch this bug because the chosen test seeds avoid the failure pattern. The new Markowitz sweep tests circumvent the dense reference by using an in-test `direct_rref_reference_fp` oracle (textbook column-by-column Gauss-Jordan) so the byte-equality assertions remain rigorous.

A separate JIT issue should be filed against `FieldMatrix::rref` to investigate. A diagnostic reproducer is preserved as inline `direct_rref_reference_fp`/`_g8` references in `crates/gf2-core/src/field/sparse_matrix.rs` (test module), available to any follow-up worker via `cargo nextest run -p gf2-core --release -E 'test(test_rref_markowitz_sweep_fp7)'`. Outside the scope of this evidence package; recorded here as an open finding for the lead's triage.

---

## 7. Files

| Path | Description |
|---|---|
| `crates/gf2-core/src/sparse.rs` | `SpBitMatrix::rref` Markowitz pivot selection (GF(2)) |
| `crates/gf2-core/src/field/sparse_matrix.rs` | `SparseFieldMatrix::rref` Markowitz pivot selection (generic `F: FiniteField`) |
| `dev/active/5ce13bae-markowitz-design.md` | Design note: algorithm + complexity argument |
| `dev/bench_results/2026-05-24-5ce13bae-markowitz-sparse-rref.csv` | Raw 5-trial benchmark CSV (50 rows = 5 × 10 cells) |
| `dev/bench_results/2026-05-24-5ce13bae-markowitz-sparse-rref.md` | This evidence document |

---

## 8. Quality gates

| Gate | Command | Result |
|---|---|:---:|
| `cargo fmt` | `cargo fmt --all -- --check` | PASS (clean after fmt pass) |
| `cargo clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS (zero warnings) |
| gf2-core tests | `cargo nextest run -p gf2-core --release --all-features --profile ci` | 2021/2021 PASS |
| Workspace tests | `cargo nextest run --workspace --all-features --release --profile ci` | 3798/3798 PASS |
| Sparse-rref subset | `cargo nextest run -p gf2-core --release -E 'test(sparse) and test(rref)' --profile ci` | 31/31 PASS |

---

## 9. Amendment — 2026-05-24 (user-approved)

The two open questions from §§ 5 and 6 were triaged by the lead and resolved by the user on 2026-05-24:

**§ 5 — three n=256 GF(p) cells short of 1.5x.** GF(7)/GF(251)/GF(65521) × n=256 are amended from `[hard]` to `[aspirational]` in 5ce13bae's issue description with architectural cause recorded (per-pivot O(m) argmin scan + Montgomery REDC at small dense n; achievable 0.516 / 0.604 / 0.627 vs target 0.667). All three deliver 1.35x-1.43x speedup over the pre-Markowitz baseline; uniform improvement holds across all 10 cells. Aggregate amended contract: 7/10 [hard] PASS + 3/10 [aspirational] PASS at ≥1.35x. Closing the residual gap is deferred to a future scoped follow-up (lazy-reduction MAC, sparse priority queue, or block elimination at small n) — not in 5ce13bae's scope. Precedent for the amendment: `7a106fe4` GF(7)/GF(31)/n=64 (same small-n constant-overhead pattern, since closed by `27bb2f75`).

**§ 6 — pre-existing FieldMatrix::rref bug.** The dense-RREF non-canonical-pivot bug discovered during Markowitz test development is filed as a separate JIT task **`bd9c6e13`** ("Fix non-canonical RREF in FieldMatrix::rref dense PLE path"), wired as a dependency of epic `026fc832` per user approval 2026-05-24. The Markowitz path under 5ce13bae is unaffected — its tests use the `direct_rref_reference_fp` / `_g8` textbook oracle as an independent canonical-RREF reference; the bug lives entirely in the dense PLE path. The reproducer (15×17 GF(7)/seed=1/density=0.05) and the expected vs observed pivot sets are documented in `bd9c6e13`'s description.
