# GF(2) echelon/rank target closure — 2026-05-06

| Field | Value |
|---|---|
| JIT issue | `366dbbcd` |
| Host | `fraktaali`-class AMD Ryzen 9 5900X development host |
| Toolchain | `rustc 1.95.0`, release builds with `RUSTFLAGS="-C target-cpu=native"` |
| Target story | `974a85bd` / epic `97bf0879` |

## Target rows and thresholds

The GF(2) echelon target rows are the M4RI `mzd_echelonize_m4ri(A, 1, 0)`
full-RREF rows from `dev/bench_results/2026-04-26-reference.csv:130-135`.
They cover `n ∈ {64, 256, 1024}` in both regimes:

- `uniform`: seed-controlled dense random square matrices;
- `deficient`: rank exactly `n/2`, generated as `L·R` with shared inner
  dimension.

The SOTA window used by the parent story is "within 1.5× of M4RI", so the
gf2-core wall-clock threshold is `1.5 × M4RI wall` for each row.

| n | regime | M4RI wall | gf2 threshold | M4RI throughput |
|---:|---|---:|---:|---:|
| 64 | uniform | 4.932 µs | 7.398 µs | 53.152 Gops/s |
| 64 | deficient | 2.462 µs | 3.693 µs | 106.476 Gops/s |
| 256 | uniform | 42.676 µs | 64.014 µs | 393.130 Gops/s |
| 256 | deficient | 30.824 µs | 46.236 µs | 544.291 Gops/s |
| 1024 | uniform | 603.392 µs | 905.088 µs | 1.780 Tops/s |
| 1024 | deficient | 360.096 µs | 540.144 µs | 2.982 Tops/s |

No M4RI GF(2) echelon row exists at `n=4096` in the pinned reference harness;
`benchmarks/reference/m4ri_bench.c` documents echelon scope as `n=64..1024`.

## Change measured

`gf2_core::alg::rref::rref` now uses a safe M4RI-style blocked RREF schedule for
left-to-right pivots:

1. collect a small block of pivot rows while maintaining identity on the block
   pivot columns;
2. build a Gray-table of pivot-row combinations;
3. clear all block pivot columns in each non-pivot row with at most one row XOR;
4. early-terminate rank-deficient tails once the unreduced suffix is zero.

The public API remains unchanged. No unsafe code was added to `gf2-core`.
Right-to-left pivoting retains the unblocked path for compatibility.

## Criterion target benchmark

Command:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --features test-support,simd --bench rref -- \
  gf2_echelon_target_rows --warm-up-time 1 --measurement-time 2 --sample-size 10
```

The benchmark compares `production_blocked` with `baseline_block1`, the same
implementation forced to block size 1 (no Gray-table batching).

| n | regime | baseline_block1 | production_blocked | speedup | threshold | status |
|---:|---|---:|---:|---:|---:|---|
| 64 | uniform | 9.537 µs | 5.168 µs | 1.85× | 7.398 µs | within threshold |
| 64 | deficient | 4.966 µs | 2.983 µs | 1.67× | 3.693 µs | within threshold |
| 256 | uniform | 332.72 µs | 59.28 µs | 5.61× | 64.014 µs | within threshold |
| 256 | deficient | 165.65 µs | 31.79 µs | 5.21× | 46.236 µs | within threshold |
| 1024 | uniform | 5.904 ms | 775.61 µs | 7.61× | 905.088 µs | within threshold |
| 1024 | deficient | 3.071 ms | 451.65 µs | 6.80× | 540.144 µs | within threshold |

## CSV-emitter cross-check

Command:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo run -p gf2-core --release --features rand,simd \
  --example bench_csv_emitter -- \
  --warmup 2 --iters 10 --output /dev/stdout --filter 'echelon/GF(2)'
```

The hand-rolled CSV emitter uses the same seeds/schema as the 2026-04-26 target
matrix. Mean rows:

| n | regime | gf2 wall | gf2 throughput | gf2/M4RI throughput |
|---:|---|---:|---:|---:|
| 64 | uniform | 5.195 µs | 50.461 Gops/s | 0.949× |
| 64 | deficient | 2.921 µs | 89.745 Gops/s | 0.843× |
| 256 | uniform | 57.749 µs | 290.520 Gops/s | 0.739× |
| 256 | deficient | 30.264 µs | 554.362 Gops/s | 1.019× |
| 1024 | uniform | 809.483 µs | 1.326 Tops/s | 0.745× |
| 1024 | deficient | 456.576 µs | 2.352 Tops/s | 0.789× |

## Validation

- `cargo fmt --all -- --check`
- `RUSTFLAGS="-C target-cpu=native" cargo nextest run -p gf2-core --features test-support,simd --release --profile ci -E 'test(rref)'`
- `RUSTFLAGS="-C target-cpu=native" cargo clippy -p gf2-core --features test-support,simd --release --all-targets -- -D warnings`
- `RUSTFLAGS="-C target-cpu=native" cargo nextest run -p gf2-core --features test-support,simd --release --profile ci` — 1959 passed, 3 skipped.
- `RUSTFLAGS="-C target-cpu=native" cargo rustc -p gf2-core --features simd --release --lib -- --emit=asm -C target-cpu=native`

All targeted GF(2) echelon/rank rows are within the 1.5× threshold. No scope
note is required for the pinned target matrix.
