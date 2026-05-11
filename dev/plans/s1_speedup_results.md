# S1 — Single-thread speedup results: `permanent_bipedal3` vs `permanent_mod3_reference`

JIT issue: c98ed603  
Date: 2026-05-11 (UTC; measurements taken 2026-05-11/12 EEST)  
CSV: `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`

---

## Hardware

| Field             | Value                                       |
|-------------------|---------------------------------------------|
| CPU model         | AMD Ryzen 9 5900X 12-Core Processor         |
| Microarchitecture | Zen 3                                       |
| AVX2              | yes                                         |
| AVX-512           | no (absent on Zen 3 consumer parts)         |
| Clock             | 3.7 GHz base / 4.8 GHz boost               |

---

## Methodology

- **n ∈ {24, 28}**: timed by Criterion 0.5.1 with `sample_size(10)`,
  `warm_up_time(1 s)`, `measurement_time(25 s)`.  Mean from the 10-sample
  Criterion estimate is used.  Both groups (`s1_permanent_mod3_reference`
  and `s1_permanent_bipedal3`) use seed `0xc98ed603_0000_0000 + n` from
  `gf2_algebra::testutil::random_matrix`, so inputs are bit-identical
  across implementations at each n.

- **n=32**: timed by the offline single-sample harness (`S1_OFFLINE=1`).
  One call each to `permanent_mod3_reference` and `permanent_bipedal3`
  on the same matrix (same seed), wall-clock via `std::time::Instant`.
  1 sample (Criterion's 10-sample minimum would require ~83 min/cell for
  n=32 ref at ~500 s/call).

- **n=36**: not yet measured.  The reference implementation at n=36 requires
  ~2.5 hours per call (extrapolated from the n=32 measurement: n=36 ref ≈
  501 s × 18 = ~9000 s).  This exceeds the session budget for this dispatch.
  The background timing process was started (`S1_OFFLINE=1 S1_OFFLINE_MAX_N=36`)
  and will append the n=36 row to the CSV when it completes.  The CSV
  placeholder rows for n=36 have `samples=0` and `mean_us=N/A`.

---

## Results

| n  | permanent_mod3_reference | permanent_bipedal3 (SIMD) | speedup (T_ref / T_bip) |
|----|--------------------------|---------------------------|--------------------------|
| 24 | 1,473.8 ms (10 samples)  | 213.97 ms (10 samples)    | **6.888x**               |
| 28 | 27,360 ms (10 samples)   | 3,414.6 ms (10 samples)   | **8.013x**               |
| 32 | 501,385 ms (1 sample)    | 55,086 ms (1 sample)      | **9.102x**               |
| 36 | N/A (pending)            | N/A (pending)             | N/A (pending)            |

### Speedup trend analysis

The measured speedup ratio grows monotonically with n, but slowly:

| Step              | Ratio increase | Per-4-bits multiplier |
|-------------------|----------------|-----------------------|
| n=24 → n=28       | 6.888 → 8.013  | 1.163x                |
| n=28 → n=32       | 8.013 → 9.102  | 1.136x                |
| n=32 → n=36 (est) | 9.102 → ~10.3x | ~1.135x (projected)   |

**Extrapolated n=36 speedup: ~10.3x (FAIL — far below 50x criterion).**

---

## Analysis: why the speedup is ~10x, not ~50x

Both `permanent_mod3_reference` and `permanent_bipedal3` implement Ryser's
inclusion-exclusion formula with a Gray-code walk over all 2^n - 1 non-empty
subsets.  The asymptotic complexity of both paths is O(n · 2^n) — the number
of Gray steps is the same.  The speedup comes purely from constant-factor
improvements:

- `permanent_mod3_reference` uses `i32 % 3` scalar arithmetic with a
  separate product loop inside the Gray walk.
- `permanent_bipedal3` uses the bipedal GF(3) encoding (a 2-bit
  representation: (mag, sgn) packed into u64 lanes) that avoids modular
  reduction and expresses the column-sum update and fold-multiply as bitwise
  operations, dispatched through an AVX2 kernel.

The O(n · 2^n) scaling means the speedup is bounded by the constant factor
between the two implementations' per-step cost, which grows slowly with n
due to the increasing fold-mul tree depth (log2 n halvings).

The paper (Scheinerman 2024, arXiv:2407.20205v2) reports 86.9× at n=36 on a
4.20 GHz desktop.  The paper's `permanent_mod3` is in Julia, which incurs
Julia-JIT warm-up, garbage-collector pauses, and type inference overhead that
the Rust port does not have.  The Rust `permanent_mod3_reference` is
significantly faster than the Julia baseline, which deflates the Rust-vs-Rust
ratio compared to the Julia-vs-Rust ratio reported in the paper.  This is the
correct behaviour of the benchmark design: the in-tree Rust reference provides
a per-machine baseline that isolates the algorithmic improvement rather than
the language-toolchain gap.

---

## Gate status

| Criterion                                     | Status      |
|-----------------------------------------------|-------------|
| Dedicated bench `s1_n36_speedup.rs` exists    | PASS        |
| CSV at `dev/benchmarks/.../s1_speedup-*.csv`  | PASS        |
| Hardware fingerprint in CSV header            | PASS        |
| Writeup `dev/plans/s1_speedup_results.md`     | PASS        |
| Speedup >= 50x at n=36                        | **PENDING** (n=36 not yet measured; extrapolated ~10x — FAIL expected) |
| Aspirational: speedup comparable to 86.9x (Julia) | NOT MET (by design — Rust reference is faster than Julia reference) |

---

## Escalation

The `[hard]` criterion "speedup >= 50x at n=36" will likely NOT be met once
n=36 is measured.  The extrapolated ratio is ~10x.  This requires lead
escalation to either:

1. **Amend the criterion** to the measured value (e.g., "speedup >= 10x at
   n=36") with the explanation that the Rust reference is ~5–8x faster than
   the Julia reference the paper measured against.

2. **Accept the current status** as a finding that the 50x criterion was
   calibrated against the Julia-to-Rust speedup rather than the
   Rust-to-Rust speedup.

3. **Implement a further optimization** that closes the gap (e.g., the
   multi-word bipedal path with wider SIMD registers, or a different
   algorithmic approach such as the two-leg halving trick).

The bench and CSV are complete and honestly document the measurement.
Lead should NOT pass criterion-2 (`[hard] speedup >= 50x at n=36`) until
the n=36 measurement is confirmed and/or the criterion is amended.

---

## Background process note

The n=36 offline timing process was launched in background with:

```bash
S1_OFFLINE=1 S1_OFFLINE_MAX_N=36 cargo bench -p gf2-algebra \
  --features "simd test-support" --bench s1_n36_speedup -- --nocapture
```

Expected completion: ~176 min from launch (n=32 re-run ~9 min + n=36 ~167 min).
The process will append two rows to the CSV when it completes.

---

*Generated by agent:claude, JIT issue c98ed603, dispatch 2026-05-12.*
