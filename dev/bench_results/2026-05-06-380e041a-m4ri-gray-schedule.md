# M4RI-style Gray-table schedule prototype — 2026-05-06

| Field | Value |
|---|---|
| JIT issue | `380e041a` |
| Host | `fraktaali`, AMD Ryzen 9 5900X, Linux 7.0.3-arch1-1 |
| Toolchain | rustc 1.95.0, cargo 1.95.0 |
| Governor | `powersave` at measurement time; frequency/host not pinned |
| Raw criterion log | `2026-05-06-380e041a-m4ri-gray-schedule-criterion.txt` |

## Prototype

The prototype is `gf2_core::alg::m4rm::multiply_with_table_schedule_for_test`, exposed only for tests / `test-support`. It keeps the existing safe M4RM Gray-table builder and register-tiled row update, but decouples the table scheduler from production's fixed `64 KiB, max k=8` policy. The criterion bench `crates/gf2-core/benches/m4rm_gray_schedule.rs` compares production against `max_k=10` schedules with target Gray-table budgets of 64, 128, 256, and 512 KiB.

For square 4096 matrices these budgets select k=7, 8, 9, and 10 respectively; production selects k=7 because of the max-k cap. The k=9 row is the main M4RI-style candidate: a wider Gray-code panel halves the number of panels while keeping the table to 256 KiB.

## Correctness

New unit coverage compares the prototype with production `m4rm::multiply` on deterministic boundary shapes covering zero rows, single-bit matrices, row-tile thresholds, and column word boundaries around 63/64/65 and 128/129 bits. The prototype therefore remains bit-exact against the existing BitMatrix multiplication path for these representative shapes.

Targeted validation command:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo nextest run -p gf2-core --features test-support,simd --release --profile ci \
  -E 'test(m4ri_style_schedule) or test(register_tiled_multiply_matches_naive_with_wide_remainders)'
```

Result: 3 tests passed.

## Criterion methodology

Command:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --features test-support,simd --bench m4rm_gray_schedule -- \
  m4rm_gray_schedule --warm-up-time 1 --measurement-time 2 --sample-size 10
```

The table below uses the middle estimate from Criterion's confidence interval. Rows are same-session comparisons, so the speedup column is the load-bearing comparison. Absolute Gops/s are included for orientation only; they are not pinned-container numbers and should not be mixed directly with older benchmark sessions.

| n | schedule | selected k | time | speedup vs production | throughput |
|---:|---|---:|---:|---:|---:|
| 1024 | production 64 KiB max8 | 8 | 0.902 ms | 1.000× | 2.380 Tops/s |
| 1024 | 64 KiB max10 | 9 | 0.895 ms | 1.008× | 2.399 Tops/s |
| 1024 | 128 KiB max10 | 10 | 1.031 ms | 0.875× | 2.082 Tops/s |
| 1024 | 256 KiB max10 | 10 | 1.028 ms | 0.878× | 2.088 Tops/s |
| 1024 | 512 KiB max10 | 10 | 1.042 ms | 0.866× | 2.061 Tops/s |
| 2048 | production 64 KiB max8 | 8 | 5.405 ms | 1.000× | 3.179 Tops/s |
| 2048 | 64 KiB max10 | 8 | 5.461 ms | 0.990× | 3.146 Tops/s |
| 2048 | 128 KiB max10 | 9 | 5.485 ms | 0.985× | 3.132 Tops/s |
| 2048 | 256 KiB max10 | 10 | 5.368 ms | 1.007× | 3.200 Tops/s |
| 2048 | 512 KiB max10 | 10 | 5.396 ms | 1.002× | 3.184 Tops/s |
| 4096 | production 64 KiB max8 | 7 | 35.854 ms | 1.000× | 3.833 Tops/s |
| 4096 | 64 KiB max10 | 7 | 36.345 ms | 0.986× | 3.782 Tops/s |
| 4096 | 128 KiB max10 | 8 | 35.081 ms | 1.022× | 3.918 Tops/s |
| 4096 | 256 KiB max10 | 9 | 33.792 ms | 1.061× | 4.067 Tops/s |
| 4096 | 512 KiB max10 | 10 | 34.018 ms | 1.054× | 4.040 Tops/s |

## Interpretation

The M4RI-style wider-panel idea is bit-exact and gives a measurable large-size win, but only a modest one in this safe prototype:

- At n=4096, the best row is k=9 / 256 KiB, 6.1% faster than same-session production.
- k=10 is slightly worse than k=9 at n=4096, and oversized tables are clearly worse at n=1024.
- At n=1024 and n=2048, same-session gains are at or below ~1%, i.e. not enough to justify a blanket policy.

Against the pinned M4RI reference from the predecessor profile (6.273 Tops/s at n=4096), the best uncontrolled absolute row here is still about 1.54× slower. Because this run is not pinned and production itself measures much faster than older pinned criterion rows, that ratio is only directional. The same-session conclusion is stronger: table scheduling alone does not deliver the missing factor; it recovers only single-digit percent at the main large size.

## Recommendation for `8e305c21`

Do not land the prototype as the sole production improvement. A production task may still use an adaptive large-size policy around k=9 / ~256 KiB as a small component, gated by same-session benchmarks and probably only for n around 4096+, but closing the GF(2) M4RI gap needs additional work in the update kernel / table layout / cache scheduling beyond simply widening the Gray-code table.
