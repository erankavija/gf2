# W5 host-side GPU dispatcher (2fbbdfa5) — gfx1030 run evidence

**Date:** 2026-05-15 (session 10)
**Host:** dev machine — AMD Ryzen 9 5900X CPU + AMD Radeon gfx1030 GPU, ROCm via `/opt/rocm`.
**Branch:** `main` at HEAD `bd059f7f` (`feat(jit:2fbbdfa5): land host-side GPU dispatcher in gf2-algebra::gpu`).

This document captures on-device execution evidence for the `2fbbdfa5` criterion 3:
"On a gfx1030 host with `hip` enabled, end-to-end batch test: 1000 random matrices
for n=24 produce results bit-identical to the CPU path." All three primes verified.

## Build (criterion 5)

```
$ cargo build -p gf2-algebra --release --features hip
   Compiling gf2-algebra v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-algebra)
    Finished `release` profile [optimized] target(s) in 2.41s
```

## Criterion 3 — end-to-end batch tests (n=24, M=1000)

Run via direct `cargo test` (nextest's slow-tier 120 s/test budget is below the
GPU wall-clock for n=24×1000 on a single device). Each test generates 1000
random matrices via `gf2_algebra::testutil::random_matrix::<P>(N, seed)` with
the seed schedule `SEED.wrapping_add(trial * 1_000_003)` (base seed
`0xDEAD_BEEF`), computes both GPU batch result and CPU per-matrix result,
asserts element-wise equality.

```
$ cargo test -p gf2-algebra --release --features hip --test gpu_dispatcher \
    --no-fail-fast -- --ignored --test-threads=1 \
    test_permanent_batch_bipedal3_matches_cpu_n24
test test_permanent_batch_bipedal3_matches_cpu_n24 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out;
              finished in 217.57s

$ cargo test ... test_permanent_batch_bipedal5_matches_cpu_n24
test test_permanent_batch_bipedal5_matches_cpu_n24 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out;
              finished in 1462.21s

$ cargo test ... test_permanent_batch_bipedal7_matches_cpu_n24
test test_permanent_batch_bipedal7_matches_cpu_n24 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out;
              finished in 1695.33s
```

| Prime | n  | M    | Wallclock | Bit-identical vs CPU? |
|-------|----|------|-----------|-----------------------|
| F_3   | 24 | 1000 | 217.57 s  | PASS                  |
| F_5   | 24 | 1000 | 1462.21 s | PASS                  |
| F_7   | 24 | 1000 | 1695.33 s | PASS                  |

F_3 throughput on gfx1030 is the headline at ~4.6 matrices/s for n=24 batched.
F_5 and F_7 were timed under GPU contention (both runs concurrent on the same
device); single-prime wall-clocks would be lower. The CPU oracle for F_7 uses
`permanent_ryser::<Fp<7>>` (because `permanent_bipedal7_singleword` is limited
to n ≤ 16 = `Packed7::LANES`); this dominates the F_7 wall-clock.

## Smoke tests (n=16, M=100) — fast on-device validation

```
$ cargo nextest run -p gf2-algebra --release --features hip \
    -E 'test(test_permanent_batch_bipedal3_smoke_n16) | \
        test(test_permanent_batch_bipedal5_smoke_n16) | \
        test(test_permanent_batch_bipedal7_smoke_n16)' \
    --run-ignored ignored-only --profile slow
PASS [   0.167s] test_permanent_batch_bipedal3_smoke_n16
PASS [   0.722s] test_permanent_batch_bipedal5_smoke_n16
PASS [   0.881s] test_permanent_batch_bipedal7_smoke_n16
Summary [   0.882s] 3 tests run: 3 passed, 507 skipped
```

Smoke tests demonstrate end-to-end dispatcher correctness in sub-second
wallclock; criterion 3 evidence is captured at the larger n=24×1000 scale above.

## Summary

All five hard criteria for `2fbbdfa5` are satisfied at HEAD on the gfx1030 dev host.
The dispatcher module is `#[cfg(feature = "hip")]`-gated; non-hip builds compile-error
on symbol reference (criterion 2 choice documented in the module-level rustdoc).
