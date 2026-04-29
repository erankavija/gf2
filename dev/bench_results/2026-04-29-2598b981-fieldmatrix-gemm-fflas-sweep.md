# JIT 2598b981 — GF(p) `FieldMatrix::gemm` fflas-comparable sweep

This is the deferred full-reference-host pass for the GF(p)
delayed-reduction work from `e7ab802d`.  It re-runs the existing
`fieldmatrix_gemm` Criterion path for the GF(p) cells relevant to
`64c88ae4` and compares them with the existing fflas-ffpack reference
harness where that harness has coverage.

## Host and toolchain

| Item | Value |
|---|---|
| Repo commit measured | `a355a2fad29ec328bf69913d38348036c26561af` |
| Branch | `worktree-agent-2598b981` |
| Kernel | `Linux fraktaali 6.19.11-arch1-1 x86_64` |
| CPU | AMD Ryzen 9 5900X 12-Core Processor, 12 cores / 24 threads |
| ISA flags relevant here | `avx2`, `bmi2`, `fma`, `vaes`, `vpclmulqdq` |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)`, LLVM 22.1.2 |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| Rust flags | `RUSTFLAGS='-C target-cpu=native'` (`-Awarnings` added only on extraction reruns) |
| C++ compiler | `g++ (GCC) 15.2.1 20260209` |
| fflas-ffpack | host `pkg-config` reports `2.5.0` |
| Givaro | host `pkg-config` reports `4.2.1` |

Note: the fflas run used the host installation rather than the pinned
container from `dev/bench_results/2026-04-26.md`.  The local fflas
harness was built with the existing `benchmarks/reference/Makefile`,
which uses `-O3 -march=native`; this satisfies the reference-host
availability check but is not bit-for-bit the 2026-04-26 pinned
container (`Givaro 4.2.0` there vs `4.2.1` here).

## Commands

gf2 side, existing Criterion harness:

```bash
RUSTFLAGS='-C target-cpu=native' cargo bench -p gf2-core \
  --bench fieldmatrix_gemm --features rand -- 'gemm/Fp_7/Fp_7/256'

for bench in \
  'gemm/Fp_251/Fp_251/256' \
  'gemm/Fp_65521/Fp_65521/256' \
  'gemm/Fp_M31/Fp_M31/256' \
  'gemm/Fp_M31/Fp_M31/1024' \
  'gemm/Fp_7/Fp_7/1024' \
  'gemm/Fp_251/Fp_251/1024' \
  'gemm/Fp_65521/Fp_65521/1024' \
  'gemm_rect/Fp_7/Fp_7/1024x1024x32' \
  'gemm_rect/Fp_7/Fp_7/1024x1024x8' \
  'gemm_rect/Fp_251/Fp_251/1024x1024x32' \
  'gemm_rect/Fp_251/Fp_251/1024x1024x8' \
  'gemm_rect/Fp_65521/Fp_65521/1024x1024x32' \
  'gemm_rect/Fp_65521/Fp_65521/1024x1024x8' \
  'gemm_rect/Fp_M31/Fp_M31/1024x1024x32' \
  'gemm_rect/Fp_M31/Fp_M31/1024x1024x8'
do
  RUSTFLAGS='-C target-cpu=native -Awarnings' cargo bench -q -p gf2-core \
    --bench fieldmatrix_gemm --features rand -- "$bench"
done
```

fflas side, existing reference harness:

```bash
make -C benchmarks/reference fflas_bench
timeout 240s benchmarks/reference/fflas_bench --warmup 0 --iters 1
```

The fflas command was intentionally bounded.  It completed, but the
`GF(2^31-1)`, `n=4096` fgemm cell emitted the harness budget warning:
`observed=67622303357_ns budget=30000000000_ns`.

## Square GF(p) cells

Throughput units are conventional GEMM Gop/s (2*m*k*n operations per cell) for both gf2 and fflas. Criterion prints these as Gelem/s because the harness passes ops_gemm(...) through Throughput::Elements; the value is still the same 2*m*k*n / wall normalizer used by the CSV harness.  The `published gf2` column is the 64c88ae4 gf2-side
baseline from `dev/bench_results/2026-04-26.md`.  `gf2 / fflas` uses the
host fflas run from this evidence document.  `gf2 / published fflas` uses
the pinned-container numbers from `2026-04-26.md`.

| field | shape | published gf2 | post-`e7ab802d` gf2 | gf2 speedup | host fflas | gf2 / host fflas | published fflas | gf2 / published fflas |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| GF(7) | 256³ | 0.528056 | 3.7077 | 7.02× | 15.04527 | 0.246× | 50.752 | 0.073× |
| GF(251) | 256³ | 0.592692 | 3.7037 | 6.25× | 130.2326 | 0.028× | 128.480 | 0.029× |
| GF(65521) | 256³ | 0.589577 | 3.6952 | 6.27× | 6.286519 | 0.588× | 31.615 | 0.117× |
| GF(2^31-1) | 256³ | 0.595291 | 3.6963 | 6.21× | 1.299062 | 2.845× | 2.126 | 1.739× |
| GF(7) | 1024³ | 0.526728 | 3.8376 | 7.29× | 52.70892 | 0.073× | 96.233 | 0.040× |
| GF(251) | 1024³ | 0.590341 | 3.8377 | 6.50× | 165.7883 | 0.023× | 138.317 | 0.028× |
| GF(65521) | 1024³ | 0.557376 | 3.8415 | 6.89× | 24.27351 | 0.158× | 43.381 | 0.089× |
| GF(2^31-1) | 1024³ | 0.589567 | 3.8223 | 6.48× | 1.560911 | 2.449× | 2.341 | 1.633× |

Criterion timing details:

| field | shape | Criterion median wall |
|---|---:|---:|
| GF(7) | 256³ | 9.0499 ms |
| GF(251) | 256³ | 9.0598 ms |
| GF(65521) | 256³ | 9.0807 ms |
| GF(2^31-1) | 256³ | 9.0779 ms |
| GF(7) | 1024³ | 559.59 ms |
| GF(251) | 1024³ | 559.57 ms |
| GF(65521) | 1024³ | 559.02 ms |
| GF(2^31-1) | 1024³ | 561.83 ms |

## Rectangular GF(p) cells

The Rust harness covers the rectangular `64c88ae4` cells.  The current
fflas reference harness does not emit rectangular fgemm rows, so the
reference column remains a harness-scope gap rather than a failed
measurement.

| field | shape | published gf2 | post-`e7ab802d` gf2 | gf2 speedup | fflas reference |
|---|---:|---:|---:|---:|---|
| GF(7) | 1024×1024×32 | 0.524526 | 3.8814 | 7.40× | not covered by current fflas harness |
| GF(7) | 1024×1024×8 | 0.505915 | 3.8755 | 7.66× | not covered by current fflas harness |
| GF(251) | 1024×1024×32 | 0.528017 | 3.8794 | 7.35× | not covered by current fflas harness |
| GF(251) | 1024×1024×8 | 0.535256 | 3.8719 | 7.23× | not covered by current fflas harness |
| GF(65521) | 1024×1024×32 | 0.532509 | 3.8743 | 7.28× | not covered by current fflas harness |
| GF(65521) | 1024×1024×8 | 0.533569 | 3.8690 | 7.25× | not covered by current fflas harness |
| GF(2^31-1) | 1024×1024×32 | 0.626623 | 3.8671 | 6.17× | not covered by current fflas harness |
| GF(2^31-1) | 1024×1024×8 | 0.643570 | 3.8596 | 6.00× | not covered by current fflas harness |

## Deferred cells

| cell | status | reason |
|---|---|---|
| gf2 GF(p) square 4096³ | deferred to nightly/slow | Criterion would run a 3 s warmup plus 10 samples; at the measured ~3.8 Gop/s this is about 36 s per iteration and several minutes per field. |
| fflas GF(2^31-1) square 4096³ | measured but budget-exceeded | Existing harness emitted `early_exit` after one 67.622 s iteration, above its 30 s per-cell reference-host budget. |
| fflas rectangular 1024×1024×{32,8} | deferred to harness extension/nightly | `benchmarks/reference/fflas_bench.cpp` currently enumerates square fgemm only. |

## Within-10× result

The post-`e7ab802d` gf2 numbers are materially faster than the published
64c88ae4 gf2-side baseline: roughly 6.0×–7.7× on every measured GF(p)
cell in this sweep.

Against the pinned 2026-04-26 fflas reference, the aspirational
"within 10× fflas for n ≥ 256" target is met for:

- GF(65521), 256³: 0.117× of fflas (just inside 10×);
- GF(2^31-1), 256³ and 1024³: faster than fflas in both measured cells.

It is not met for GF(7) or GF(251) in the measured square cells.  On the
host fflas installation used for this run, GF(65521) and GF(2^31-1) are
within 10× at all measured n ≥ 256 cells, while GF(7) is within 10× at
256³ only and GF(251) remains outside 10×.

## Validation / caveats

- No implementation changes were made.
- No ignored or slow tests were run.
- The fflas reference was available on this host through `pkg-config`,
  but it was not the pinned container environment.  Ratios against both
  the local host fflas run and the published pinned-container fflas
  baseline are therefore shown explicitly.
- The local fflas run printed `SINGULAR MATRIX` diagnostics from later
  factorisation cells in the all-operation reference harness; the fgemm
  rows above completed and were emitted before/around those diagnostics.
