# Logical-buffer profile attribution (jit:18a87159)

Source: 9 repetitions under `profile-unroll4/rep-*`, 10 frozen profile cases, driver and host identities in `profile-unroll4/host.txt`.

Per-call and memory figures are medians over the repetitions with order-statistic intervals in brackets. A per-call figure divides a whole-process counter by the call count the driver observed in that same pass, so it carries the pass's own start-up and fixture construction as well as its measured operations. Symbol-share availability and bounds follow the rules below.

## Per-call cost and instruction mix

| Case | ns/call | cycles/call | instructions/call | IPC | branches/call | branch-miss rate |
|---|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 7.922 [7.888, 7.941] | 38.03 [37.85, 38.07] | 147.00 [147.00, 147.00] | 3.866 [3.861, 3.884] | 28.00 [28.00, 28.00] | 0.0000 [0.0000, 0.0000] |
| `xor-9w-a64-warm@public-xor-a` | 8.160 [8.139, 8.175] | 39.21 [39.12, 39.22] | 162.00 [162.00, 162.00] | 4.132 [4.130, 4.141] | 31.00 [31.00, 31.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-warm@public-xor-a` | 11.022 [10.987, 11.125] | 52.86 [52.54, 53.44] | 203.00 [203.00, 203.00] | 3.841 [3.799, 3.864] | 30.00 [30.00, 30.00] | 0.0000 [0.0000, 0.0000] |
| `xor-8w-o8-warm@public-xor-a` | 7.954 [7.933, 7.998] | 38.16 [38.09, 38.22] | 147.00 [147.00, 147.00] | 3.853 [3.847, 3.859] | 28.00 [28.00, 28.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-streaming@public-xor-a` | 30.413 [30.001, 30.674] | 151.06 [149.19, 152.49] | 208.14 [208.06, 208.14] | 1.378 [1.365, 1.395] | 30.25 [30.25, 30.25] | 0.0001 [0.0000, 0.0001] |
| `row-xor-8w-full-warm@row-xor-a` | 9.502 [9.485, 9.590] | 46.00 [45.92, 46.06] | 138.00 [138.00, 138.00] | 3.000 [2.996, 3.005] | 30.00 [30.00, 30.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-9w-full-warm@row-xor-a` | 10.168 [10.153, 10.219] | 49.27 [49.25, 49.29] | 153.00 [153.00, 153.00] | 3.106 [3.104, 3.107] | 33.00 [33.00, 33.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-warm@row-xor-a` | 13.403 [13.373, 13.434] | 64.44 [64.41, 64.60] | 194.01 [194.01, 194.01] | 3.011 [3.003, 3.012] | 32.00 [32.00, 32.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-8w-tail63-warm@row-xor-a` | 9.450 [9.359, 9.521] | 45.76 [45.21, 45.85] | 138.00 [138.00, 138.00] | 3.016 [3.010, 3.053] | 30.00 [30.00, 30.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-streaming@row-xor-a` | 30.257 [30.104, 31.274] | 149.28 [148.73, 154.02] | 199.02 [199.02, 199.27] | 1.333 [1.294, 1.338] | 32.24 [32.24, 32.26] | 0.0001 [0.0001, 0.0001] |

## Memory traffic

| Case | L1 loads/call | L1 load-miss rate | LLC loads/call | LLC load-miss rate | calls per pass |
|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 46.00 [46.00, 46.02] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 127926271 [127926271, 127926271] |
| `xor-9w-a64-warm@public-xor-a` | 48.00 [48.00, 48.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 123731967 [123731967, 123731967] |
| `xor-64w-a64-warm@public-xor-a` | 94.00 [94.00, 94.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 91226111 [90177535, 91226111] |
| `xor-8w-o8-warm@public-xor-a` | 49.00 [49.00, 49.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 125829119 [125829119, 127926271] |
| `xor-64w-a64-streaming@public-xor-a` | 93.66 [93.56, 93.98] | 0.2119 [0.2116, 0.2129] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 33030143 [33030143, 33554431] |
| `row-xor-8w-full-warm@row-xor-a` | 37.20 [37.19, 37.20] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 105906175 [104857599, 106954751] |
| `row-xor-9w-full-warm@row-xor-a` | 37.50 [37.50, 37.50] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 98566143 [98566143, 98566143] |
| `row-xor-64w-full-warm@row-xor-a` | 104.00 [104.00, 104.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 75497471 [74448895, 75497471] |
| `row-xor-8w-tail63-warm@row-xor-a` | 34.00 [34.00, 34.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 106954751 [105906175, 108003327] |
| `row-xor-64w-full-streaming@row-xor-a` | 94.07 [92.49, 94.64] | 0.2242 [0.2233, 0.2263] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 33554431 [31981567, 33554431] |

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

Symbols the flat sample report attributes at or above its own percent limit. A share is summarized only when every repetition reports that symbol, using the same order-statistic interval as the other tables. Missing rows are censored by report display and are not zero measurements; their all-repetition aggregate is unavailable. The linked raw reports below show which repetitions contain each symbol.

| Case | symbol | share, median [interval] | reports present |
|---|---|---|---:|
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::drive` | 41.61% [41.14%, 42.66%] | 9/9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_buffer_harness::fixture::XorBanks::pair` | 20.75% [20.05%, 20.95%] | 9/9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::measure::{{closure}}` | 17.86% [17.34%, 18.25%] | 9/9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 17.10% [16.68%, 17.46%] | 9/9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::fns::xor_fn` | 2.59% [2.49%, 2.87%] | 9/9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::drive` | 36.88% [35.11%, 37.45%] | 9/9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 26.78% [25.97%, 28.09%] | 9/9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_core::matrix::BitMatrix::row_xor` | 26.22% [25.57%, 26.43%] | 9/9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::measure::{{closure}}` | 10.91% [10.14%, 11.15%] | 9/9 |
