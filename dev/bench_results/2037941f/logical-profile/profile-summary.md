# Logical-buffer profile attribution (jit:18a87159)

Source: 9 repetitions under `logical-profile/rep-*`, 14 frozen profile cases, driver and host identities in `logical-profile/host.txt`.

Each figure is the median over the repetitions, with the order-statistic interval in brackets. A per-call figure divides a whole-process counter by the call count the driver observed in that same pass, so it carries the pass's own start-up and fixture construction as well as its measured operations.

## Per-call cost and instruction mix

| Case | ns/call | cycles/call | instructions/call | IPC | branches/call | branch-miss rate |
|---|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 7.941 [7.932, 7.968] | 38.15 [38.13, 38.23] | 142.00 [142.00, 142.00] | 3.722 [3.714, 3.725] | 27.00 [27.00, 27.00] | 0.0000 [0.0000, 0.0000] |
| `xor-9w-a64-warm@public-xor-a` | 8.021 [7.984, 8.140] | 38.42 [38.29, 39.08] | 157.00 [157.00, 157.00] | 4.086 [4.017, 4.100] | 30.00 [30.00, 30.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-warm@public-xor-a` | 11.452 [11.335, 11.536] | 54.93 [54.48, 55.39] | 212.01 [212.01, 212.01] | 3.859 [3.828, 3.891] | 34.00 [34.00, 34.00] | 0.0000 [0.0000, 0.0000] |
| `xor-8w-o8-warm@public-xor-a` | 8.005 [7.964, 8.032] | 38.37 [38.29, 38.47] | 142.00 [142.00, 142.00] | 3.700 [3.691, 3.709] | 27.00 [27.00, 27.00] | 0.0000 [0.0000, 0.0000] |
| `xor-64w-a64-streaming@public-xor-a` | 31.050 [30.780, 32.233] | 154.54 [152.93, 159.83] | 217.22 [217.22, 217.44] | 1.406 [1.360, 1.420] | 34.26 [34.26, 34.27] | 0.0000 [0.0000, 0.0000] |
| `xor-8w-a64-warm@resolved-xor` | 8.015 [8.003, 8.019] | 38.57 [38.50, 38.66] | 133.00 [133.00, 133.00] | 3.449 [3.440, 3.454] | 24.00 [24.00, 24.00] | 0.0000 [0.0000, 0.0000] |
| `xor-9w-a64-warm@resolved-xor` | 8.064 [8.036, 8.107] | 38.67 [38.66, 38.77] | 148.00 [148.00, 148.00] | 3.827 [3.818, 3.828] | 27.00 [27.00, 27.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-8w-full-warm@row-xor-a` | 9.512 [9.497, 9.530] | 46.01 [45.98, 46.05] | 133.00 [133.00, 133.00] | 2.891 [2.888, 2.893] | 29.00 [29.00, 29.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-9w-full-warm@row-xor-a` | 10.157 [10.136, 10.223] | 49.12 [49.11, 49.54] | 148.00 [148.00, 148.00] | 3.013 [2.988, 3.014] | 32.00 [32.00, 32.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-warm@row-xor-a` | 13.583 [13.547, 13.827] | 65.33 [65.21, 66.38] | 203.01 [203.01, 203.01] | 3.108 [3.058, 3.113] | 36.00 [36.00, 36.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-8w-tail63-warm@row-xor-a` | 9.486 [9.471, 9.498] | 45.93 [45.88, 45.97] | 133.00 [133.00, 133.00] | 2.896 [2.893, 2.899] | 29.00 [29.00, 29.00] | 0.0000 [0.0000, 0.0000] |
| `row-xor-64w-full-streaming@row-xor-a` | 29.770 [29.154, 30.440] | 147.10 [143.89, 149.96] | 207.94 [207.87, 208.10] | 1.414 [1.388, 1.445] | 36.24 [36.24, 36.25] | 0.0000 [0.0000, 0.0001] |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | 287073.311 [285566.775, 289837.192] | 1379733.65 [1369618.47, 1393134.88] | 6363060.34 [6363026.31, 6363092.21] | 4.612 [4.567, 4.646] | 1675749.97 [1675742.47, 1675758.23] | 0.0064 [0.0062, 0.0064] |
| `nr-construct-bg2-1024-441-z56-46w-warm@nr-construct-a` | 8259680.935 [8233883.358, 8303952.397] | 39832682.12 [39724463.33, 40028578.12] | 213703555.41 [213703492.72, 213732184.62] | 5.365 [5.339, 5.380] | 59434338.11 [59434326.91, 59442284.88] | 0.0010 [0.0009, 0.0010] |

## Memory traffic

| Case | L1 loads/call | L1 load-miss rate | LLC loads/call | LLC load-miss rate | calls per pass |
|---|---|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | 46.00 [46.00, 46.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 127926271 [125829119, 127926271] |
| `xor-9w-a64-warm@public-xor-a` | 48.00 [48.00, 48.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 125829119 [123731967, 125829119] |
| `xor-64w-a64-warm@public-xor-a` | 94.00 [94.00, 94.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 88080383 [87031807, 89128959] |
| `xor-8w-o8-warm@public-xor-a` | 49.00 [49.00, 49.00] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 125829119 [125829119, 125829119] |
| `xor-64w-a64-streaming@public-xor-a` | 94.03 [93.85, 94.14] | 0.2038 [0.2029, 0.2049] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 32505855 [31195135, 32505855] |
| `xor-8w-a64-warm@resolved-xor` | 42.00 [42.00, 42.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 125829119 [125829119, 125829119] |
| `xor-9w-a64-warm@resolved-xor` | 44.00 [44.00, 44.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 125829119 [123731967, 125829119] |
| `row-xor-8w-full-warm@row-xor-a` | 37.21 [37.20, 37.22] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 105906175 [105906175, 106954751] |
| `row-xor-9w-full-warm@row-xor-a` | 37.50 [37.50, 37.51] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 98566143 [98566143, 99614719] |
| `row-xor-64w-full-warm@row-xor-a` | 103.53 [103.52, 103.54] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 74448895 [72351743, 74448895] |
| `row-xor-8w-tail63-warm@row-xor-a` | 34.01 [34.00, 34.01] | 0.0000 [0.0000, 0.0000] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 105906175 [105906175, 106954751] |
| `row-xor-64w-full-streaming@row-xor-a` | 92.30 [92.05, 92.68] | 0.2092 [0.2087, 0.2098] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 34078719 [33030143, 34603007] |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | 2458872.58 [2453140.21, 2469190.13] | 0.0032 [0.0032, 0.0033] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 3519 [3455, 3551] |
| `nr-construct-bg2-1024-441-z56-46w-warm@nr-construct-a` | 77922050.68 [77914100.70, 78001924.63] | 0.1110 [0.1110, 0.1111] | 0.000 [0.000, 0.000] | 0.0000 [0.0000, 0.0000] | 123 [121, 123] |

## Observed route

| Case | selected path |
|---|---|
| `xor-8w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=8/layout=a64/src%64=0/dst%64=0/working-set=128B` |
| `xor-9w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=9/layout=a64/src%64=0/dst%64=0/working-set=256B` |
| `xor-64w-a64-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=64/layout=a64/src%64=0/dst%64=0/working-set=1024B` |
| `xor-8w-o8-warm@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=8/layout=o8/src%64=8/dst%64=8/working-set=128B` |
| `xor-64w-a64-streaming@public-xor-a` | `gf2-core/kernels::ops::xor_inplace/simd/w=64/layout=a64/src%64=0/dst%64=0/working-set=67108864B` |
| `xor-8w-a64-warm@resolved-xor` | `gf2-core/kernels::ops::resolve_xor_inplace/simd/w=8/layout=a64/src%64=0/dst%64=0/working-set=128B` |
| `xor-9w-a64-warm@resolved-xor` | `gf2-core/kernels::ops::resolve_xor_inplace/simd/w=9/layout=a64/src%64=0/dst%64=0/working-set=256B` |
| `row-xor-8w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=8w/shape=full/base%64=16/dst:src%64=16:16,16:16,16:16,16:16,16:16,16:16,16:16,16:16/working-set=4096B` |
| `row-xor-9w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=9w/shape=full/base%64=0/dst:src%64=8:0,0:8,8:0,0:8,8:0,0:8,8:0,0:8/working-set=4608B` |
| `row-xor-64w-full-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=64w/shape=full/base%64=16/dst:src%64=16:16,16:16,16:16,16:16,16:16,16:16,16:16,16:16/working-set=32768B` |
| `row-xor-8w-tail63-warm@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=8w/shape=tail63/base%64=0/dst:src%64=0:0,0:0,0:0,0:0,0:0,0:0,0:0,0:0/working-set=4096B` |
| `row-xor-64w-full-streaming@row-xor-a` | `gf2-core/BitMatrix::row_xor/simd/stride=64w/shape=full/base%64=48/dst:src%64=48:48,48:48,48:48,48:48,48:48,48:48,48:48,48:48/working-set=67108864B` |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2-coding/QuasiCyclicLdpc::nr_5g_rate_matched(2,256,49)/simd/Z=9/dense=378x468/stride=8w/nnz=1773/h-sha256=1cd2ee73fb6ad2bff8557657add5362f508aa877a7ed1c0255f2dd2c54c386a4` |
| `nr-construct-bg2-1024-441-z56-46w-warm@nr-construct-a` | `gf2-coding/QuasiCyclicLdpc::nr_5g_rate_matched(2,1024,441)/simd/Z=56/dense=2352x2912/stride=46w/nnz=11032/h-sha256=ce426676dbecc9d2f56a50a8418d802e1c36c22a9f9ca1cbf9267446ce43debc` |

## Sampled symbol shares

Symbols the flat sample report attributes at or above its own percent limit, as the median of their per-repetition shares.

| Case | symbol | median share | repetitions |
|---|---|---|---|
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::drive` | 39.82% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 20.98% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_buffer_harness::fixture::XorBanks::pair` | 20.51% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `logical_profile::measure::{{closure}}` | 16.04% | 9 |
| `xor-8w-a64-warm@public-xor-a` | `gf2_kernels_simd::x86::avx2::fns::xor_fn` | 2.53% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::drive` | 39.20% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 26.85% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `gf2_core::matrix::BitMatrix::row_xor` | 25.88% | 9 |
| `row-xor-8w-full-warm@row-xor-a` | `logical_profile::measure::{{closure}}` | 8.43% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_coding::ldpc::nr_5g::<impl gf2_coding::ldpc::core::QuasiCyclicLdpc>::nr_5g_rate_matched` | 85.81% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_core::sparse::SpBitMatrix::from_coo` | 1.96% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `hashbrown::map::HashMap<K,V,S,A>::insert` | 1.84% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_coding::ldpc::core::QuasiCyclicLdpc::to_edges` | 1.01% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_core::sparse::SpBitMatrix::transpose` | 0.96% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_core::matrix::BitMatrix::row_xor` | 0.85% | 9 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_kernels_simd::x86::avx2::avx2_xor_into` | 0.67% | 7 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `cfree` | 0.60% | 7 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_core::matrix::BitMatrix::swap_rows` | 0.54% | 4 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `gf2_coding::ldpc::nr_5g::<impl gf2_coding::ldpc::core::QuasiCyclicLdpc>::nr_5g` | 0.53% | 1 |
| `nr-construct-bg2-256-49-z9-8w-warm@nr-construct-a` | `malloc` | 0.53% | 5 |
