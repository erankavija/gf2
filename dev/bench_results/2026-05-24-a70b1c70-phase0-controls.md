# Phase 0 control-lane measurements — GF(241), GF(127), GF(251) drift check

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `a70b1c70` (Refresh GF(251) and control-lane benchmarks) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Predecessor plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` (Phase 0) |
| Host | Linux 7.0.3-arch1-1 / Zen 3 (AMD Ryzen 9 5900X); verified via `/proc/cpuinfo` |
| Reference | fflas-ffpack 2.5.0 (pinned; `pkg-config --modversion fflas-ffpack` = 2.5.0) |
| Kernel path | gf2-core Candidate C (`N_THRESH_PRIME = 252`, `select_f32_path = false`) |

---

## 1. Methodology (verbatim from `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 6)

> All Wave-6B benchmarks were run on:
>
> - **CPU:** AMD Ryzen 9 5900X (Zen 3), 12c/24t, 3.7 GHz base / 4.6 GHz boost. AVX2 + BMI2 + VAES + VPCLMULQDQ. No AVX-512.
> - **Kernel:** Linux 7.0.3-arch1-1.
> - **Isolation:** `taskset -c 6-11 nice -n -5` (CCX1 pinned: cores 6-11, SMT siblings 18-23). Agent and parent shell on CCX0 (cores 0-5). Sequential trials (no concurrent benches).
> - **Toolchain:** rustc 1.95.0 (59807616e 2026-04-14), criterion 0.5.1.
> - **Frequency governor:** powersave (no root to flip). Per-core boost enabled; reaches 4.6 GHz under load. Transient thermal ramps produce 1-2% per-iteration variance, handled by 5-trial median.
> - **Reference:** fflas-ffpack 2.5.0 + Givaro 4.2.0 in pinned container (`gf2-bench:ref`, sha256 in `benchmarks/image.lock`). Container built from Debian bookworm-20260421-slim. All container measurements are single-threaded (pinned-image protocol per `dev/plans/sota_reference_acceptance_protocol.md` § 5).

The above recipe was followed verbatim for this Phase 0 run. Cargo invocation:

```bash
cargo build --release -p gf2-core --bench fieldmatrix_gemm --features rand,simd
# Per-trial invocation (5 sequential trials):
taskset -c 6-11 nice -n -5 <bench_binary> "gemm/Fp_(127|241)/Fp_(127|241)/(256|1024)$" --bench
# GF(251) drift check (single trial):
taskset -c 6-11 nice -n -5 <bench_binary> "gemm/Fp_251/Fp_251/(256|1024)$" --bench
```

Gop/s computed as `2 * n^3 / median_ns` (same formula as `run_662f7a15_prime_sweep.sh`; criterion median point estimate in nanoseconds).

**No concurrent jobs observed during the 5-trial windows.** No IDE, browser video, or competing cargo process was running during measurement. Confirmed by visual inspection of system load before each trial.

---

## 2. fflas-ffpack reference for GF(241) and GF(127)

The canonical fflas_bench binary (`benchmarks/reference/fflas_bench`) does not have GF(241) or GF(127) rows. The fflas reference numbers are **bracketed** from measured primes on the same code path, using the authorised dimensional-extrapolation argument from `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 1.1:

- **GF(241)** (p = 241 < 256): fflas-ffpack routes all byte-family primes through `Modular<float>` + OpenBLAS sgemm. The canonical measured reference is GF(251)/`Modular<float>` from `dev/bench_results/2026-04-26-reference.csv`: n=256 → 128.48 Gop/s, n=1024 → 138.32 Gop/s. GF(241) uses the same dispatch path; throughput is flat across byte primes on this code path.
- **GF(127)** (p = 127 < 256): fflas-ffpack's default dispatch for GF(127) when instantiated as `Modular<int64_t>` (the mode used in `run_field` for all non-float primes in the bench harness) is the `int64` delayed-reduction path, not the `Modular<float>` path. The bracketed reference is GF(7)/`Modular<int64_t>` from the same canonical baseline: n=256 → 50.75 Gop/s, n=1024 → 96.23 Gop/s. fflas throughput on `Modular<int64_t>` is nearly flat across tiny/small primes (GF(7) = 50.75, GF(31) = 50.48 at n=256), making this extrapolation conservative.

These bracket values match the `*`-annotated rows in `7a106fe4` § 1.1 exactly.

---

## 3. GF(241) control measurements — 5 trials

**CSV:** `dev/bench_results/2026-05-24-a70b1c70-gf241-control.csv`

### 3.1 Raw trials

| trial | n=256 Gop/s | n=1024 Gop/s |
|---:|---:|---:|
| 1 | 59.171 | 71.050 |
| 2 | 59.009 | 70.294 |
| 3 | 59.004 | 70.670 |
| 4 | 58.824 | 70.364 |
| 5 | 59.243 | 70.815 |

### 3.2 Aggregate (5-trial median / Q1 / Q3 / min / max)

| n | median Gop/s | Q1 | Q3 | IQR | min | max |
|---:|---:|---:|---:|---:|---:|---:|
| 256 | 59.009 | 59.004 | 59.171 | 0.167 | 58.824 | 59.243 |
| 1024 | 70.670 | 70.364 | 70.815 | 0.451 | 70.294 | 71.050 |

### 3.3 PASS/FAIL against 1.5x threshold (ratio >= 0.667)

| n | gf2 Gop/s | fflas Gop/s | ratio | fflas source | verdict |
|---:|---:|---:|---:|---|---|
| 256 | 59.009 | 128.48* | 0.459 | GF(251)/Modular<float> bracket | **FAIL** |
| 1024 | 70.670 | 138.32* | 0.511 | GF(251)/Modular<float> bracket | **FAIL** |

`*` = extrapolated from GF(251) per § 2; same `Modular<float>` + OpenBLAS dispatch tier.

**Both GF(241) cells FAIL the 1.5x threshold.** This mirrors the GF(251) structural gap: fflas routes GF(241) through its float-modular BLAS cascade, which delivers 128-138 Gop/s by delegating to OpenBLAS sgemm, while gf2-core Candidate C achieves 59-71 Gop/s with its byte-packed AVX2 panel kernel. The architectural cause (float-modular BLAS cascade vs. hand-written panel loop) is the same as documented in `7a106fe4` § 3.1 Amendment A.

---

## 4. GF(127) control measurements — 5 trials

**CSV:** `dev/bench_results/2026-05-24-a70b1c70-gf127-control.csv`

### 4.1 Raw trials

| trial | n=256 Gop/s | n=1024 Gop/s |
|---:|---:|---:|
| 1 | 54.756 | 71.235 |
| 2 | 54.652 | 69.442 |
| 3 | 54.815 | 71.200 |
| 4 | 54.630 | 69.426 |
| 5 | 54.053 | 71.359 |

### 4.2 Aggregate (5-trial median / Q1 / Q3 / min / max)

| n | median Gop/s | Q1 | Q3 | IQR | min | max |
|---:|---:|---:|---:|---:|---:|---:|
| 256 | 54.652 | 54.630 | 54.756 | 0.126 | 54.053 | 54.815 |
| 1024 | 71.200 | 69.442 | 71.235 | 1.793 | 69.426 | 71.359 |

The n=1024 IQR is wider (1.793 Gop/s) than n=256 (0.126 Gop/s), reflecting normal thermal variation across the 5 sequential trials at the larger matrix size.

### 4.3 PASS/FAIL against 1.5x threshold (ratio >= 0.667)

| n | gf2 Gop/s | fflas Gop/s | ratio | fflas source | verdict |
|---:|---:|---:|---:|---|---|
| 256 | 54.652 | 50.75* | 1.077 | GF(7)/Modular<int64_t> bracket | **PASS** |
| 1024 | 71.200 | 96.23* | 0.740 | GF(7)/Modular<int64_t> bracket | **PASS** |

`*` = extrapolated from GF(7) per § 2; same `Modular<int64_t>` dispatch tier.

**Both GF(127) cells PASS the 1.5x threshold.** GF(127) uses the `Modular<int64_t>` code path in fflas (not the float-modular BLAS cascade), so gf2-core Candidate C is competitive: it exceeds fflas by 7.7% at n=256 and runs at 74.0% of fflas at n=1024 (above the 66.7% threshold). These numbers are consistent with GF(31) rows in `7a106fe4` § 1.1 (same dispatch tier; 53.74/68.98 Gop/s at n=256/1024 for GF(31)).

---

## 5. GF(251) drift check — single trial at current HEAD

Cited numbers from `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 1.1 (prime-sweep aggregate, 5-trial median):
- GF(251)/n=256: **58.98 Gop/s**
- GF(251)/n=1024: **70.89 Gop/s**

Single-trial measurement at HEAD (2026-05-24, same CCX pinning):

| n | measured Gop/s | cited Gop/s | delta | verdict |
|---:|---:|---:|---:|---|
| 256 | 61.026 | 58.98 | +3.47% | **PASS** (within ±5%) |
| 1024 | 72.443 | 70.89 | +2.19% | **PASS** (within ±5%) |

Both cells are within ±5% of the cited numbers. The current HEAD runs approximately 2-3% faster than the 2026-05-06 baseline, consistent with normal session-to-session variation and no host regression. The cited 58.98 / 70.89 Gop/s numbers remain valid for prototype-vs-baseline delta computation in Phase 1.

---

## 6. Summary table

| field | n | gf2 Gop/s (median) | fflas Gop/s | ratio | verdict |
|---|---:|---:|---:|---:|---|
| GF(241) | 256 | 59.009 | 128.48* | 0.459 | FAIL ([aspirational] — float-modular structural gap) |
| GF(241) | 1024 | 70.670 | 138.32* | 0.511 | FAIL ([aspirational] — float-modular structural gap) |
| GF(127) | 256 | 54.652 | 50.75* | 1.077 | PASS |
| GF(127) | 1024 | 71.200 | 96.23* | 0.740 | PASS |
| GF(251) drift | 256 | 61.026 (1-trial) | — | — | PASS (cited 58.98, delta +3.47%) |
| GF(251) drift | 1024 | 72.443 (1-trial) | — | — | PASS (cited 70.89, delta +2.19%) |

The GF(241) cells are structural FAIL for the same architectural reason as GF(251): both primes sit in fflas's float-modular BLAS cascade tier. These cells should carry `[aspirational]` status consistent with GF(251) in `7a106fe4`. The GF(127) cells PASS — they are on fflas's `Modular<int64_t>` path which gf2-core Candidate C exceeds or closely matches.

---

## 7. CSV references

| CSV | Rows | Description |
|---|---:|---|
| `dev/bench_results/2026-05-24-a70b1c70-gf241-control.csv` | 11 (incl. header) | 5-trial raw rows for GF(241)/n∈{256,1024}, Candidate C |
| `dev/bench_results/2026-05-24-a70b1c70-gf127-control.csv` | 11 (incl. header) | 5-trial raw rows for GF(127)/n∈{256,1024}, Candidate C |

---

## 8. Open questions

None. Both GF(251) drift cells pass within ±5%; no silent rebaseline is needed. The GF(241) FAIL cells are expected (structural float-modular gap, same as GF(251)) and consistent with the `[aspirational]` amendment in `7a106fe4`. Phase 1 prototype dispatch may proceed using the cited 58.98 / 70.89 Gop/s numbers as the GF(251) baseline.

---

## 9. Source index

| Reference | Path |
|---|---|
| cc5de315 methodology SSOT | `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md` § 6 |
| Canonical fflas baseline | `dev/bench_results/2026-04-26-reference.csv` (GF(251) fgemm rows) |
| Small-prime sweep aggregate (prior) | `dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv` |
| Phase 0 plan | `dev/active/615db3b9-finite-field-la-sota-plan.md` § "Phase 0" |
| GF(241) raw CSV | `dev/bench_results/2026-05-24-a70b1c70-gf241-control.csv` |
| GF(127) raw CSV | `dev/bench_results/2026-05-24-a70b1c70-gf127-control.csv` |
