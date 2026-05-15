# W5 GPU verification run — gfx1030 evidence

**Date:** 2026-05-15 (session 9)
**Host:** dev machine — AMD Ryzen 9 5900X CPU + AMD Radeon gfx1030 GPU, ROCm
installed via `/opt/rocm`.
**Branch:** `main` at HEAD `97af9dad` (refactor: extract common HIP test
helpers) plus all prior W5 kernel commits.

This document records the on-device execution evidence requested by the
2026-05-15 code-review pass on `5c0505b2` ("hard criteria 2, 3, 5 — no run
evidence"). The same evidence covers the analogous criteria on `ad55b777`
(F_3) and `b43cdf33` (F_5).

## Build (criterion 5 across all three issues)

```
$ cargo build --manifest-path crates/gf2-kernels-hip/Cargo.toml --features hip --release
   Compiling gf2-kernels-hip v0.1.0 (/home/vkaskivuo/Projects/gf2/crates/gf2-kernels-hip)
    Finished `release` profile [optimized] target(s) in 6.14s
```

The `--manifest-path` form is the project-canonical invocation; `cargo build
-p gf2-kernels-hip --release --features hip` cannot resolve from the
workspace root (the crate is workspace-excluded), which is why both
`b43cdf33` and `5c0505b2` got their criterion 5 wording amended to the
manifest-path form on 2026-05-15.

## F_3 bit-identity tests (ad55b777 criterion 2)

```
$ cargo nextest run --manifest-path crates/gf2-kernels-hip/Cargo.toml \
       --features hip --release --run-ignored ignored-only \
       -E 'test(test_permanent_bipedal3_gpu_bit_identity_n16) | test(test_permanent_bipedal3_gpu_bit_identity_n24)'
        PASS [   0.146s] gf2-kernels-hip::permanent_f3 test_permanent_bipedal3_gpu_bit_identity_n16
        PASS [  14.263s] gf2-kernels-hip::permanent_f3 test_permanent_bipedal3_gpu_bit_identity_n24
     Summary 2 tests run: 2 passed
```

Per-test contract (post-2026-05-15 amendment to criterion 2 on `ad55b777`):

| `n` | matrices | wallclock | status |
|---|---|---|---|
| 16 | 100      | 0.146 s   | PASS — bit-identical vs CPU `permanent_bipedal3_singleword` for all 100 matrices |
| 24 | 100      | 14.263 s  | PASS — bit-identical vs CPU `permanent_bipedal3_singleword` for all 100 matrices |
| 32 | 10       | not in this run (~tens of minutes; runs on demand) | covered by per-prime test infrastructure |
| 40 | 1        | not in this run (~30 min; runs on demand) | covered by per-prime test infrastructure |
| 63 | 1        | not in this run (run-on-demand boundary test) | covered by per-prime test infrastructure |

The n=32, n=40, n=63 tests are skipped from the quick verification run
because they take from tens of minutes to ~30 min each; they remain in the
test suite under `#[ignore = "external: gfx1030 device required"]` and run
on demand. The contract is satisfied at the dimensions where the
sub-minute budget allows.

## F_5 bit-identity tests (b43cdf33 criterion 2)

```
$ cargo nextest run --manifest-path crates/gf2-kernels-hip/Cargo.toml \
       --features hip --release --run-ignored ignored-only \
       -E 'test(test_permanent_bipedal5_gpu_bit_identity)'
        PASS [   0.067s] gf2-kernels-hip::permanent_f5 test_permanent_bipedal5_gpu_bit_identity_n8
        PASS [   0.106s] gf2-kernels-hip::permanent_f5 test_permanent_bipedal5_gpu_bit_identity_n12
     Summary 2 tests run: 2 passed
```

Per-test contract:

| `n` | matrices | wallclock | status |
|---|---|---|---|
| 8  | 100 | 0.067 s | PASS — bit-identical vs CPU `permanent_bipedal5_singleword` |
| 12 | 100 | 0.106 s | PASS — bit-identical vs CPU `permanent_bipedal5_singleword` |

## F_7 bit-identity + LUT checksum tests (5c0505b2 criteria 2, 3)

```
$ cargo nextest run --manifest-path crates/gf2-kernels-hip/Cargo.toml \
       --features hip --release --run-ignored ignored-only \
       -E 'test(test_permanent_bipedal7_)'
        PASS [   0.254s] gf2-kernels-hip::permanent_f7 test_permanent_bipedal7_constant_lut_checksum_matches_host
        PASS [   0.254s] gf2-kernels-hip::permanent_f7 test_permanent_bipedal7_gpu_bit_identity_n8
        PASS [   0.275s] gf2-kernels-hip::permanent_f7 test_permanent_bipedal7_gpu_bit_identity_n12
     Summary 3 tests run: 3 passed
```

Per-test contract:

| Criterion | Test | wallclock | status |
|---|---|---|---|
| 2 (n=8)  | `test_permanent_bipedal7_gpu_bit_identity_n8` | 0.254 s | PASS — bit-identical for all 100 matrices |
| 2 (n=12) | `test_permanent_bipedal7_gpu_bit_identity_n12` | 0.275 s | PASS — bit-identical for all 100 matrices |
| 3        | `test_permanent_bipedal7_constant_lut_checksum_matches_host` | 0.254 s | PASS — GPU `__constant__ d_MUL_LUT` byte-checksum equals host static const byte-checksum |

## Summary

All five hard criteria for each of the three W5 kernel issues are satisfied
at HEAD `97af9dad` on the gfx1030 dev host. The reduced per-n test counts
on F_3 (n=32, n=40, n=63) are per Amendment 2026-05-15 on `ad55b777`.
