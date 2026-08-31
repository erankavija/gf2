# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization

Rendered by `baseline-survey/make-receipt.py` from the run directory; every
figure below is read out of the committed CSVs at render time.

| Field | Value |
|---|---|
| Issue | `4e732b56` |
| Run timestamps (UTC) | 2026-08-31T17:02:00Z, 2026-08-31T17:18:55Z |
| gf2 revision | `3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e` |
| Host | AMD Ryzen 9 5900X 12-Core Processor |
| Cores pinned | CCX1 via `dev/scripts/ccx1-bench-flock.sh` (`taskset -c 6-11`, `nice -n -5`) |
| Governor | powersave |
| Kernel | Linux fraktaali 7.1.11-arch1-1 #1 SMP PREEMPT_DYNAMIC Fri, 28 Aug 2026 03:36:07 +0000 x86_64 GNU/Linux |
| C compiler | gcc (GCC) 16.2.1 20260810 |
| C++ compiler | g++ (GCC) 16.2.1 20260810 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07) |

## Baseline pins

| Pin | Value |
|---|---|
| aff3ct tag | `v4.7.0` |
| aff3ct commit | `e8a65c5047262d97a15563b9edc961f69b2792cc` |
| bchlib tag | `v2.1.3` |
| bchlib commit | `8d0656ab8f37e734428635501738d360ad80eebd` |
| m4ri version | `20260122` |
| m4ri tarball sha256 | `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` |
| itpp release | `4.3.1 (tarball sha256 50717621c5dfb5ed22f8492f8af32b17776e6e06641dfe3a3a8f82c8d353b877)` |
| itpp soname | `8.2.1` |

## Reference build configuration

* `aff3ct library:  -O3 -march=native -funroll-loops -O3 -DNDEBUG -std=gnu++11 -fPIC`
* `aff3ct defines:  -DAFF3CT_EXT_STRINGS -DAFF3CT_MULTI_PREC -DAFF3CT_POLAR_BIT_PACKING -DMIPP_ENABLE_BACKTRACE -DSPU_COLORS -DSPU_STACKTRACE`
* `m4ri library: CFLAGS='-O3 -march=native -fPIC'`

## Measured cells

Throughput is information bits per second for W1 and matrix bits per second
for W2. `Trials` is the number of independent trials the cell's wall budget
allowed; a cell marked *estimate* was projected from a measured per-unit cost
and was never run at that size.

| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |
|---|---|---|---|---|---|---|---|---|---|
| W1 | B1 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 89.97 | 83.55 | 90.91 | 8.2% |
| W1 | B1 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 4.16 | 3.84 | 4.54 | 16.8% |
| W1 | B1 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 4.50 | 4.47 | 4.52 | 1.2% |
| W1 | B1 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 3.85 | 3.53 | 3.89 | 9.4% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 102.97 | 93.55 | 103.69 | 9.8% |
| W1 | B1 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 391.32 | 384.31 | 401.08 | 4.3% |
| W1 | B1 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 7.00 | 6.64 | 7.01 | 5.2% |
| W1 | B1 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 6.79 | 6.63 | 6.86 | 3.3% |
| W1 | B1 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 5.54 | 5.14 | 5.60 | 8.3% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 102.53 | 101.39 | 102.56 | 1.1% |
| W1 | B1 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 394.19 | 390.69 | 400.56 | 2.5% |
| W1 | B1 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 5.91 | 5.90 | 5.92 | 0.3% |
| W1 | B1 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 5.90 | 5.89 | 5.92 | 0.5% |
| W1 | B1 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.25 | 6.24 | 6.25 | 0.2% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 99.90 | 99.70 | 100.26 | 0.6% |
| W1 | B1 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 370.17 | 368.92 | 371.09 | 0.6% |
| W1 | B1 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 6.14 | 6.11 | 6.16 | 0.9% |
| W1 | B1 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 6.13 | 6.03 | 6.17 | 2.2% |
| W1 | B1 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 6.22 | 6.19 | 6.23 | 0.7% |
| W1 | B2 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 86.82 | 86.06 | 88.26 | 2.5% |
| W1 | B2 | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 1993.27 | 1626.21 | 2044.15 | 21.0% |
| W1 | B2 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.77 | 1.74 | 1.80 | 3.1% |
| W1 | B2 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.75 | 1.74 | 1.76 | 1.5% |
| W1 | B2 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.73 | 0.73 | 0.74 | 0.8% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 87.33 | 85.02 | 87.72 | 3.1% |
| W1 | B2 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 180.73 | 177.18 | 182.89 | 3.2% |
| W1 | B2 | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 2036.31 | 1966.40 | 2239.01 | 13.4% |
| W1 | B2 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.95 | 1.94 | 1.97 | 1.4% |
| W1 | B2 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.97 | 1.96 | 1.98 | 0.8% |
| W1 | B2 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.67 | 0.67 | 0.67 | 0.1% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 78.96 | 78.07 | 79.40 | 1.7% |
| W1 | B2 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 178.75 | 175.33 | 180.87 | 3.1% |
| W1 | B2 | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 2259.42 | 2248.81 | 2266.24 | 0.8% |
| W1 | B2 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.88 | 1.88 | 1.91 | 1.5% |
| W1 | B2 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.90 | 1.89 | 1.91 | 1.1% |
| W1 | B2 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.66 | 0.66 | 0.5% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 75.98 | 75.31 | 76.39 | 1.4% |
| W1 | B2 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 159.77 | 153.12 | 161.80 | 5.4% |
| W1 | B2 | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 2240.90 | 2235.24 | 2251.43 | 0.7% |
| W1 | B2 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 1.90 | 1.89 | 1.91 | 0.9% |
| W1 | B2 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 1.90 | 1.90 | 1.91 | 0.9% |
| W1 | B2 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.66 | 0.65 | 0.66 | 0.6% |
| W1 | B3 | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 112.56 | 111.36 | 113.28 | 1.7% |
| W1 | B3 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.25 | 3.21 | 3.27 | 2.0% |
| W1 | B3 | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.24 | 3.23 | 3.28 | 1.4% |
| W1 | B3 | 1 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.35 | 0.35 | 0.35 | 0.9% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 98.37 | 98.10 | 99.17 | 1.1% |
| W1 | B3 | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 443.08 | 432.58 | 451.59 | 4.3% |
| W1 | B3 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.51 | 3.49 | 3.57 | 2.3% |
| W1 | B3 | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.49 | 3.47 | 3.51 | 1.1% |
| W1 | B3 | 16 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.37 | 0.36 | 0.37 | 0.7% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 97.85 | 96.57 | 97.94 | 1.4% |
| W1 | B3 | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 437.37 | 432.09 | 439.87 | 1.8% |
| W1 | B3 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.39 | 3.37 | 3.41 | 1.3% |
| W1 | B3 | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.42 | 3.41 | 3.42 | 0.5% |
| W1 | B3 | 256 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.36 | 0.36 | 0.36 | 0.4% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 96.72 | 96.63 | 97.67 | 1.1% |
| W1 | B3 | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 432.48 | 430.62 | 434.12 | 0.8% |
| W1 | B3 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 3.43 | 3.42 | 3.43 | 0.4% |
| W1 | B3 | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 3.43 | 3.42 | 3.43 | 0.4% |
| W1 | B3 | 4096 | itpp 4.3.1+soname8.2.1 | `poly-remainder-gfx` | 7 | 0.39 | 0.36 | 0.39 | 8.4% |
| W1 | T2N | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.61 | 38.52 | 38.63 | 0.3% |
| W1 | T2N | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4636.90 | 4188.23 | 4671.61 | 10.4% |
| W1 | T2N | 1 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.67 | 0.67 | 0.68 | 1.2% |
| W1 | T2N | 1 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.67 | 0.69 | 2.2% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.67 | 38.57 | 38.79 | 0.6% |
| W1 | T2N | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.15 | 40.20 | 43.72 | 8.6% |
| W1 | T2N | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4650.60 | 4647.78 | 4654.58 | 0.1% |
| W1 | T2N | 16 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.68 | 0.67 | 0.68 | 0.3% |
| W1 | T2N | 16 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.67 | 0.68 | 1.1% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.39 | 38.23 | 38.53 | 0.8% |
| W1 | T2N | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.39 | 41.22 | 41.95 | 1.8% |
| W1 | T2N | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4644.85 | 4640.69 | 4646.95 | 0.1% |
| W1 | T2N | 256 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch` | 7 | 0.68 | 0.68 | 0.68 | 0.1% |
| W1 | T2N | 256 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-loop` | 7 | 0.68 | 0.68 | 0.68 | 0.9% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 38.37 | 38.31 | 38.39 | 0.2% |
| W1 | T2N | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 41.57 | 41.47 | 41.81 | 0.8% |
| W1 | T2N | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4672.27 | 4638.78 | 4680.73 | 0.9% |
| W1 | T2N | 4096 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `encode-batch-projected` | *estimate* | 0.65 | — | — | — |
| W1 | T2S | 1 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 45.04 | 44.83 | 45.21 | 0.8% |
| W1 | T2S | 1 | bchlib v2.1.3 | `table-remainder` | 7 | 4575.20 | 4552.78 | 4604.07 | 1.1% |
| W1 | T2S | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.78 | 0.78 | 0.79 | 1.3% |
| W1 | T2S | 1 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.79 | 0.78 | 0.79 | 1.4% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.87 | 42.82 | 43.22 | 0.9% |
| W1 | T2S | 16 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 51.99 | 51.85 | 52.28 | 0.8% |
| W1 | T2S | 16 | bchlib v2.1.3 | `table-remainder` | 7 | 4567.90 | 4551.30 | 4588.83 | 0.8% |
| W1 | T2S | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.77 | 0.77 | 0.78 | 1.1% |
| W1 | T2S | 16 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.77 | 0.76 | 0.78 | 1.7% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.85 | 41.88 | 43.11 | 2.9% |
| W1 | T2S | 256 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 68.57 | 68.40 | 69.21 | 1.2% |
| W1 | T2S | 256 | bchlib v2.1.3 | `table-remainder` | 7 | 4544.95 | 4533.26 | 4547.55 | 0.3% |
| W1 | T2S | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 7 | 0.77 | 0.76 | 0.77 | 0.9% |
| W1 | T2S | 256 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 7 | 0.76 | 0.76 | 0.76 | 0.2% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-scalar` | 7 | 42.62 | 42.54 | 42.69 | 0.4% |
| W1 | T2S | 4096 | aff3ct v4.7.0 | `lfsr-simd-inter` | 7 | 50.78 | 50.59 | 50.82 | 0.4% |
| W1 | T2S | 4096 | bchlib v2.1.3 | `table-remainder` | 7 | 4533.24 | 4529.20 | 4559.91 | 0.7% |
| W1 | T2S | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-batch` | 3 | 0.76 | 0.76 | 0.76 | 0.1% |
| W1 | T2S | 4096 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `encode-loop` | 3 | 0.76 | 0.76 | 0.76 | 0.1% |
| W2 | B1 | 5 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 241.94 | 138.89 | 250.00 | 45.9% |
| W2 | B1 | 5 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 6.26 | 6.22 | 6.29 | 1.2% |
| W2 | B1 | 5 | m4ri 20260122 | `echelonize` | 7 | 234.38 | 138.89 | 241.95 | 44.0% |
| W2 | B1 | 5 | m4ri 20260122 | `genmatrix-rref` | 7 | 234.38 | 87.21 | 250.00 | 69.5% |
| W2 | B1 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 108.20 | 72.89 | 108.66 | 33.1% |
| W2 | B1 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 140.61 | 112.78 | 147.92 | 25.0% |
| W2 | B2 | 64 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 156.22 | 142.17 | 156.46 | 9.1% |
| W2 | B2 | 64 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 6.92 | 6.91 | 6.98 | 1.1% |
| W2 | B2 | 64 | m4ri 20260122 | `echelonize` | 7 | 1443.70 | 856.48 | 1456.63 | 41.6% |
| W2 | B2 | 64 | m4ri 20260122 | `genmatrix-rref` | 7 | 1304.66 | 1177.97 | 1306.75 | 9.9% |
| W2 | B2 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2035.28 | 1945.84 | 2040.35 | 4.6% |
| W2 | B2 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 2580.92 | 2467.47 | 2596.51 | 5.0% |
| W2 | B3 | 223 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 119.72 | 117.39 | 120.78 | 2.8% |
| W2 | B3 | 223 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 7 | 7.29 | 7.23 | 7.47 | 3.3% |
| W2 | B3 | 223 | m4ri 20260122 | `echelonize` | 7 | 1477.40 | 964.14 | 1562.22 | 40.5% |
| W2 | B3 | 223 | m4ri 20260122 | `genmatrix-rref` | 7 | 1703.57 | 1599.58 | 1720.05 | 7.1% |
| W2 | B3 | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 2391.62 | 2363.89 | 2410.81 | 2.0% |
| W2 | B3 | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 3091.06 | 3015.34 | 3141.13 | 4.1% |
| W2 | T2N | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 84.66 | 84.24 | 85.04 | 0.9% |
| W2 | T2N | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 93.62 | 90.06 | 93.87 | 4.1% |
| W2 | T2N | 32208 | aff3ct v4.7.0 | `basis-encode-pack` | 4 | 41.23 | 41.17 | 41.27 | 0.2% |
| W2 | T2N | 32208 | gf2 08e42f096a75309d35255d831782ae38d291f632 | `generator-matrix-projected` | *estimate* | 0.66 | — | — | — |
| W2 | T2N | 32208 | m4ri 20260122 | `echelonize` | 7 | 175.22 | 172.27 | 176.90 | 2.6% |
| W2 | T2N | 32208 | m4ri 20260122 | `genmatrix-rref` | 7 | 446.82 | 430.33 | 447.35 | 3.8% |
| W2 | T2S | 256 | m4ri 20260122 | `matmul-m4rm` | 7 | 339.91 | 336.56 | 340.18 | 1.1% |
| W2 | T2S | 4096 | m4ri 20260122 | `matmul-m4rm` | 7 | 457.52 | 454.15 | 460.09 | 1.3% |
| W2 | T2S | 7032 | aff3ct v4.7.0 | `basis-encode-pack` | 7 | 48.28 | 48.23 | 48.33 | 0.2% |
| W2 | T2S | 7032 | gf2 3eaa7c7530e8762303e8962e7ee38c5f6eb60f0e | `generator-matrix` | 3 | 1.58 | 1.58 | 1.58 | 0.1% |
| W2 | T2S | 7032 | m4ri 20260122 | `echelonize` | 7 | 788.77 | 786.37 | 793.32 | 0.9% |
| W2 | T2S | 7032 | m4ri 20260122 | `genmatrix-rref` | 7 | 1754.17 | 1747.76 | 1768.44 | 1.2% |

## Files

| File | SHA-256 | Bytes |
|---|---|---|
| `2026-08-31-4e732b56-generator-agreement.txt` | `ccd3474e8296a67e…` | 170 |
| `2026-08-31-4e732b56-small-aff3ct-perf-stat.txt` | `c5444ff8a39b37c9…` | 3213 |
| `2026-08-31-4e732b56-small-aff3ct.csv` | `0c9410478f7898d8…` | 18979 |
| `2026-08-31-4e732b56-small-aff3ct.log` | `c06bf9f3852c4dc4…` | 2420 |
| `2026-08-31-4e732b56-small-bchlib.csv` | `ae2f68ed79a55681…` | 5046 |
| `2026-08-31-4e732b56-small-bchlib.log` | `b5ef3a892b1d30bd…` | 919 |
| `2026-08-31-4e732b56-small-gf2.csv` | `ed806f9235b0dd41…` | 27312 |
| `2026-08-31-4e732b56-small-gf2.log` | `38967f1be0f4decc…` | 2961 |
| `2026-08-31-4e732b56-small-host.txt` | `48d874f6602133c5…` | 6947 |
| `2026-08-31-4e732b56-small-itpp.csv` | `144d658f7610b981…` | 8175 |
| `2026-08-31-4e732b56-small-itpp.log` | `d73de26d9321da47…` | 1053 |
| `2026-08-31-4e732b56-small-m4ri.csv` | `1a65df2601a97b07…` | 9406 |
| `2026-08-31-4e732b56-small-m4ri.log` | `c1f653058eae71da…` | 989 |
| `2026-08-31-4e732b56-t2n-aff3ct.csv` | `68dac5879e0c5d61…` | 4962 |
| `2026-08-31-4e732b56-t2n-aff3ct.log` | `2b23d6cdad68bcc3…` | 780 |
| `2026-08-31-4e732b56-t2n-bchlib-perf-stat.txt` | `41549b5083a6a249…` | 1393 |
| `2026-08-31-4e732b56-t2n-bchlib.csv` | `471e8b8d959eb79b…` | 2708 |
| `2026-08-31-4e732b56-t2n-bchlib.log` | `a4262934c26f034c…` | 470 |
| `2026-08-31-4e732b56-t2n-gf2.csv` | `9f0b2209b3cdc731…` | 5468 |
| `2026-08-31-4e732b56-t2n-gf2.log` | `37359a00592290c9…` | 939 |
| `2026-08-31-4e732b56-t2n-host.txt` | `fdbe9cf0695132e6…` | 6938 |
| `2026-08-31-4e732b56-t2n-itpp.csv` | `eebab57153ce9b42…` | 90 |
| `2026-08-31-4e732b56-t2n-itpp.log` | `d2f9b2a8b59c9a6c…` | 275 |
| `2026-08-31-4e732b56-t2n-m4ri.csv` | `1a9b71ddc4d9e46c…` | 2683 |
| `2026-08-31-4e732b56-t2n-m4ri.log` | `6d3eff1f308e6301…` | 409 |
| `generators.txt` | `612d5c84ee02ff69…` | 551 |

## Reproduction

```
dev/active/4e732b56/baseline-survey/fetch-build.sh
dev/active/4e732b56/baseline-survey/run-survey.sh dev/bench_results/4e732b56 <codes> <prefix>
dev/active/4e732b56/baseline-survey/make-receipt.py dev/bench_results/4e732b56
```
