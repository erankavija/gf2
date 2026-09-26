# Logical-buffer profile attribution (jit:18a87159)

Source: 9 repetitions under `profile-unroll2/rep-*`, 10 frozen profile cases, driver and host identities in `profile-unroll2/host.txt`.

Each figure is the median over the repetitions, with the order-statistic interval in brackets. A per-call figure divides a whole-process counter by the call count the driver observed in that same pass, so it carries the pass's own start-up and fixture construction as well as its measured operations.

## Per-call cost and instruction mix

| Case | ns/call | cycles/call | instructions/call | IPC | branches/call | branch-miss rate |
|---|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 8.102 [8.088, 8.114] | 38.90 [38.84, 38.99] | 143.00 [143.00, 143.00] | 3.676 [3.668, 3.682] | 27.00 [27.00, 27.00] | 0.0000 [0.0000, 0.0000] |
| `xor-9w-a64-warm@public-xor-a` | 8.189 [8.155, 8.212] | 39.29 [39.17, 39.31] | 158.00 [158.00, 158.00] | 4.022 [4.019, 4.034] | 30.00 [30.00, 30.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-warm@public-xor-a` | 11.266 [11.223, 11.303] | 53.90 [53.81, 54.00] | 227.01 [227.01, 227.01] | 4.211 [4.204, 4.218] | 34.00 [34.00, 34.00] | 0.0000 [0.0000, 0.0000] |
| `xor-8w-o8-warm@public-xor-a` | 8.159 [8.141, 8.177] | 39.19 [39.14, 39.23] | 143.00 [143.00, 143.00] | 3.649 [3.645, 3.653] | 27.00 [27.00, 27.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-streaming@public-xor-a` | 31.109 [30.669, 31.753] | 154.28 [152.25, 156.81] | 232.22 [232.14, 232.31] | 1.505 [1.481, 1.525] | 34.26 [34.25, 34.26] | 0.0000 [0.0000, 0.0001] |
| `row-xor-8w-full-warm@row-xor-a` | 9.479 [9.427, 9.531] | 45.73 [45.66, 45.84] | 134.00 [134.00, 134.00] | 2.930 [2.923, 2.935] | 29.00 [29.00, 29.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-9w-full-warm@row-xor-a` | 10.053 [10.035, 10.089] | 48.65 [48.61, 48.76] | 149.00 [149.00, 149.00] | 3.063 [3.056, 3.065] | 32.00 [32.00, 32.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-warm@row-xor-a` | 13.302 [13.294, 13.417] | 63.90 [63.89, 64.59] | 218.01 [218.01, 218.01] | 3.412 [3.375, 3.412] | 36.00 [36.00, 36.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-8w-tail63-warm@row-xor-a` | 9.414 [9.393, 9.469] | 45.60 [45.51, 45.62] | 134.00 [134.00, 134.00] | 2.939 [2.938, 2.945] | 29.00 [29.00, 29.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-streaming@row-xor-a` | 30.444 [29.694, 30.487] | 149.91 [146.40, 150.38] | 223.10 [222.94, 223.10] | 1.488 [1.484, 1.523] | 36.25 [36.24, 36.25] | 0.0000 [0.0000, 0.0001] |

## Memory traffic

| Case | L1 loads/call | L1 load-miss rate | LLC loads/call | LLC load-miss rate | calls per pass |
|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 46.00 [46.00, 46.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 123731967 [123731967, 123731967] |
| `xor-9w-a64-warm@public-xor-a` | 48.01 [48.00, 48.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 123731967 [123731967, 123731967] |
| `xor-64w-a64-warm@public-xor-a` | 94.00 [94.00, 94.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 89128959 [89128959, 89128959] |
| `xor-8w-o8-warm@public-xor-a` | 49.00 [49.00, 49.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 123731967 [123731967, 123731967] |
| `xor-64w-a64-streaming@public-xor-a` | 93.54 [93.51, 93.70] | 0.2122 [0.2122, 0.2124] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 32505855 [31981567, 33030143] |
| `row-xor-8w-full-warm@row-xor-a` | 37.20 [37.19, 37.21] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 106954751 [105906175, 106954751] |
| `row-xor-9w-full-warm@row-xor-a` | 37.50 [37.50, 38.22] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 99614719 [99614719, 100663295] |
| `row-xor-64w-full-warm@row-xor-a` | 104.00 [104.00, 104.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 75497471 [75497471, 75497471] |
| `row-xor-8w-tail63-warm@row-xor-a` | 34.00 [34.00, 34.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 106954751 [105906175, 108003327] |
| `row-xor-64w-full-streaming@row-xor-a` | 94.10 [92.77, 94.58] | 0.2132 [0.2118, 0.2145] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 33030143 [33030143, 34078719] |

## Observed route

| Case | selected path |
|---|---|
| `xor-8w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=8/layout=a64/src%64=0/dst%64=0/working-set=128B` |
| `xor-9w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=9/layout=a64/src%64=0/dst%64=0/working-set=256B` |
| `xor-64w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=64/layout=a64/src%64=0/dst%64=0/working-set=1024B` |
| `xor-8w-o8-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=8/layout=o8/src%64=8/dst%64=8/working-set=128B` |
| `xor-64w-a64-streaming@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=64/layout=a64/src%64=0/dst%64=0/working-set=67108864B` |
| `row-xor-8w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=8w/shape=full/base%64=16/dst:src%64=16:16,16:16,16:16,16:16,16:16,16:16,16:16,16:16/working-set=4096B` |
| `row-xor-9w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=9w/shape=full/base%64=0/dst:src%64=8:0,0:8,8:0,0:8,8:0,0:8,8:0,0:8/working-set=4608B` |
| `row-xor-64w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=64w/shape=full/base%64=16/dst:src%64=16:16,16:16,16:16,16:16,16:16,16:16,16:16,16:16/working-set=32768B` |
| `row-xor-8w-tail63-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=8w/shape=tail63/base%64=0/dst:src%64=0:0,0:0,0:0,0:0,0:0,0:0,0:0,0:0/working-set=4096B` |
| `row-xor-64w-full-streaming@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=64w/shape=full/base%64=48/dst:src%64=48:48,48:48,48:48,48:48,48:48,48:48,48:48,48:48/working-set=67108864B` |

## Sampled symbol shares

Symbols the flat sample report attributes at or above its own percent limit, as the median of their per-repetition shares.

| Case | symbol | median share | repetitions |
|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::drive` | 40.04% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 21.87% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_buffer_harness::fixture::XorBanks::pair` | 20.31% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::measure::{{closure}}` | 15.24% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::fns::xor_fn` | 2.47% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::drive` | 35.54% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 27.86% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_core::matrix::BitMatrix::row_xor` | 26.27% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::measure::{{closure}}` | 10.20% | 9 |
