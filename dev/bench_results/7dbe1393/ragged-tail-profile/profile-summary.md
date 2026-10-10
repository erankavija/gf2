# Matvec lane profile attribution at ragged strides

Each bracket is the 96.1% order-statistic interval for the median of nine full process passes. Counters include setup, calibration, and output release, divided by the arm's observed timed call count; the campaign receipt remains the authority for operation latency.

| Frozen cell | cycles/call | instructions/call | L1 misses/call | cache misses/call |
|---|---:|---:|---:|---:|
| `lane-r1024-9w-warm.matvec-lane-scalar` | 19552.84 [19420.58, 20859.93] | 116780.79 [115837.22, 116882.11] | 1333.50 [1328.38, 1349.46] | 22.05 [21.74, 22.50] |
| `ragged-r1024-9w-warm.matvec-a` | 25003.32 [24931.13, 25080.77] | 138124.27 [138090.38, 138301.38] | 1282.82 [1282.39, 1290.30] | 21.09 [21.04, 21.13] |
| `ragged-r1024-9w-warm.matvec-lane-parity` | 19303.04 [18904.26, 20125.46] | 89305.75 [89155.91, 89655.59] | 1332.11 [1329.55, 1356.91] | 21.45 [21.42, 22.29] |
| `lane-r1024-10w-warm.matvec-lane-scalar` | 20294.91 [20191.37, 20473.82] | 122855.87 [122776.15, 123184.64] | 1483.01 [1479.34, 1497.35] | 24.35 [24.13, 24.92] |
| `ragged-r1024-10w-warm.matvec-a` | 31211.00 [30886.47, 32127.26] | 164202.81 [163986.44, 164663.88] | 1451.74 [1448.24, 1456.66] | 23.65 [23.61, 23.85] |
| `ragged-r1024-10w-warm.matvec-lane-parity` | 19980.66 [19724.60, 20151.41] | 89727.21 [89628.42, 89865.94] | 1485.69 [1479.28, 1488.99] | 23.67 [23.55, 23.87] |
| `lane-r1024-62w-warm.matvec-lane-scalar` | 80547.04 [79750.10, 81859.71] | 317055.95 [316645.18, 317306.03] | 9012.92 [9000.76, 9038.59] | 319.73 [301.93, 353.16] |
| `ragged-r1024-62w-warm.matvec-a` | 93277.67 [91657.35, 97250.26] | 354075.56 [353147.27, 355639.00] | 9159.95 [9119.88, 9268.37] | 332.75 [315.06, 363.07] |
| `ragged-r1024-62w-warm.matvec-lane-parity` | 40046.87 [38837.65, 42397.26] | 151894.18 [151639.83, 153189.11] | 9052.36 [9002.23, 9195.79] | 372.62 [301.12, 416.49] |
| `lane-r1024-63w-warm.matvec-lane-scalar` | 84791.76 [83514.56, 89733.63] | 323782.27 [323366.83, 325354.50] | 9210.01 [9180.50, 9354.23] | 346.91 [300.24, 380.26] |
| `ragged-r1024-63w-warm.matvec-a` | 96725.21 [96022.87, 99054.05] | 380643.72 [380467.88, 381026.06] | 9356.08 [9326.65, 9397.01] | 372.07 [297.95, 403.30] |
| `ragged-r1024-63w-warm.matvec-lane-parity` | 39834.88 [38342.21, 41658.98] | 151946.19 [150982.69, 152739.96] | 9117.62 [9113.68, 9277.77] | 380.46 [350.61, 436.95] |
| `lane-r1024-66w-warm.matvec-lane-scalar` | 86228.33 [85795.05, 87907.51] | 334566.51 [334278.97, 334995.88] | 9672.86 [9633.74, 9714.86] | 363.26 [336.80, 403.17] |
| `ragged-r1024-66w-warm.matvec-a` | 95820.92 [94942.82, 97687.27] | 369106.59 [368770.87, 370124.79] | 9795.82 [9759.09, 9824.30] | 363.72 [343.40, 375.83] |
| `ragged-r1024-66w-warm.matvec-lane-parity` | 61060.51 [43893.26, 63136.59] | 157181.50 [155643.68, 158518.36] | 9478.82 [9392.89, 9531.00] | 395.93 [384.89, 438.04] |
| `lane-r1024-32w-warm.matvec-lane-scalar` | 50986.62 [49889.49, 74532.95] | 201846.81 [201487.55, 203957.79] | 4745.46 [4724.32, 4796.06] | 107.79 [87.66, 199.57] |
| `ragged-r1024-32w-control-warm.matvec-a` | 64359.57 [38045.84, 71556.59] | 179480.18 [178616.54, 181445.10] | 4629.16 [4609.90, 4712.53] | 186.16 [117.74, 206.98] |
| `ragged-r1024-32w-control-warm.matvec-lane-parity` | 42974.20 [42204.48, 44319.10] | 108285.96 [108009.32, 115551.11] | 4520.89 [4506.54, 4870.55] | 177.42 [168.78, 194.24] |

## Sampled cycle attribution

Each symbol share and count is the median and order-statistic interval over nine independent cycle-sampling passes. The table retains symbols whose median sampled share reaches 1%; every report and annotation remains beside the table.

| Frozen cell | symbol | share (%) | samples/pass | total samples/pass |
|---|---|---:|---:|---:|
| `lane-r1024-9w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 97.39 [96.77, 97.81] | 2780.00 [2751.00, 2809.00] | 2859.00 [2830.00, 2877.00] |
| `ragged-r1024-9w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 77.57 [77.02, 78.26] | 2126.00 [2091.00, 2135.00] | 2737.00 [2715.00, 2750.00] |
| `ragged-r1024-9w-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 14.10 [13.54, 14.68] | 386.00 [370.00, 399.00] | 2737.00 [2715.00, 2750.00] |
| `ragged-r1024-9w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 6.50 [6.00, 6.82] | 178.00 [165.00, 198.00] | 2737.00 [2715.00, 2750.00] |
| `ragged-r1024-9w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 72.13 [72.08, 73.57] | 2030.00 [2006.00, 2047.00] | 2812.00 [2754.00, 2841.00] |
| `ragged-r1024-9w-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 16.25 [15.65, 16.79] | 459.00 [431.00, 481.00] | 2812.00 [2754.00, 2841.00] |
| `ragged-r1024-9w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 8.68 [8.18, 9.01] | 248.00 [226.00, 256.00] | 2812.00 [2754.00, 2841.00] |
| `lane-r1024-10w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 97.61 [97.20, 97.89] | 2764.00 [2687.00, 2809.00] | 2837.00 [2752.00, 2879.00] |
| `ragged-r1024-10w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 80.02 [79.34, 80.90] | 2242.00 [2189.00, 2304.00] | 2798.00 [2744.00, 2851.00] |
| `ragged-r1024-10w-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 14.63 [13.76, 15.10] | 410.00 [395.00, 463.00] | 2798.00 [2744.00, 2851.00] |
| `ragged-r1024-10w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 3.50 [3.11, 3.79] | 100.00 [87.00, 109.00] | 2798.00 [2744.00, 2851.00] |
| `ragged-r1024-10w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 72.32 [71.32, 73.19] | 2002.00 [1985.00, 2049.00] | 2800.00 [2712.00, 2829.00] |
| `ragged-r1024-10w-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 16.76 [15.89, 17.88] | 464.00 [439.00, 502.00] | 2800.00 [2712.00, 2829.00] |
| `ragged-r1024-10w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 8.35 [7.95, 8.90] | 235.00 [220.00, 247.00] | 2800.00 [2712.00, 2829.00] |
| `lane-r1024-62w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 98.58 [98.28, 98.63] | 2816.00 [2781.00, 2852.00] | 2855.00 [2821.00, 2903.00] |
| `ragged-r1024-62w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 83.51 [83.23, 84.32] | 2431.00 [2115.00, 2502.00] | 2887.00 [2503.00, 2981.00] |
| `ragged-r1024-62w-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 13.22 [12.71, 13.67] | 380.00 [310.00, 397.00] | 2887.00 [2503.00, 2981.00] |
| `ragged-r1024-62w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 1.54 [1.21, 1.96] | 46.00 [34.00, 58.00] | 2887.00 [2503.00, 2981.00] |
| `ragged-r1024-62w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 83.34 [82.49, 83.94] | 2337.00 [1671.00, 2377.00] | 2819.00 [2005.00, 2837.00] |
| `ragged-r1024-62w-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 10.43 [9.86, 10.77] | 294.00 [197.00, 305.00] | 2819.00 [2005.00, 2837.00] |
| `ragged-r1024-62w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 4.11 [3.65, 4.40] | 112.00 [83.00, 123.00] | 2819.00 [2005.00, 2837.00] |
| `lane-r1024-63w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 98.42 [98.24, 98.60] | 2814.00 [2786.00, 2895.00] | 2854.00 [2836.00, 2938.00] |
| `ragged-r1024-63w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 83.84 [83.25, 84.47] | 2438.00 [2414.00, 2478.00] | 2916.00 [2883.00, 2946.00] |
| `ragged-r1024-63w-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 12.91 [11.97, 13.22] | 370.00 [356.00, 382.00] | 2916.00 [2883.00, 2946.00] |
| `ragged-r1024-63w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 1.89 [1.71, 2.02] | 55.00 [49.00, 59.00] | 2916.00 [2883.00, 2946.00] |
| `ragged-r1024-63w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 83.32 [82.78, 84.07] | 2375.00 [2298.00, 2428.00] | 2865.00 [2776.00, 2908.00] |
| `ragged-r1024-63w-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 10.23 [9.11, 10.85] | 290.00 [254.00, 323.00] | 2865.00 [2776.00, 2908.00] |
| `ragged-r1024-63w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 4.29 [3.73, 4.61] | 123.00 [111.00, 129.00] | 2865.00 [2776.00, 2908.00] |
| `lane-r1024-66w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 98.52 [98.39, 98.96] | 2819.00 [2790.00, 2841.00] | 2858.00 [2837.00, 2871.00] |
| `ragged-r1024-66w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 83.95 [82.46, 84.88] | 2454.00 [2383.00, 3309.00] | 2910.00 [2891.00, 3895.00] |
| `ragged-r1024-66w-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 12.71 [12.48, 14.20] | 412.00 [361.00, 450.00] | 2910.00 [2891.00, 3895.00] |
| `ragged-r1024-66w-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 1.60 [1.24, 1.65] | 47.00 [37.00, 51.00] | 2910.00 [2891.00, 3895.00] |
| `ragged-r1024-66w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 85.30 [84.65, 86.71] | 2439.00 [2217.00, 2482.00] | 2814.00 [2599.00, 2851.00] |
| `ragged-r1024-66w-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 7.96 [7.12, 8.67] | 218.00 [203.00, 247.00] | 2814.00 [2599.00, 2851.00] |
| `ragged-r1024-66w-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 3.90 [3.67, 4.39] | 107.00 [103.00, 114.00] | 2814.00 [2599.00, 2851.00] |
| `lane-r1024-32w-warm.matvec-lane-scalar` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_scalar` | 98.39 [98.10, 98.51] | 2854.00 [2676.00, 2936.00] | 2903.00 [2710.00, 2990.00] |
| `ragged-r1024-32w-control-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 77.78 [72.84, 80.61] | 2195.00 [1679.00, 2279.00] | 2826.00 [2296.00, 2860.00] |
| `ragged-r1024-32w-control-warm.matvec-a` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 15.33 [13.62, 20.75] | 437.00 [365.00, 580.00] | 2826.00 [2296.00, 2860.00] |
| `ragged-r1024-32w-control-warm.matvec-a` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 3.82 [3.48, 4.45] | 104.00 [89.00, 127.00] | 2826.00 [2296.00, 2860.00] |
| `ragged-r1024-32w-control-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_parity` | 81.62 [80.84, 83.31] | 2188.00 [2171.00, 2280.00] | 2702.00 [2606.00, 2777.00] |
| `ragged-r1024-32w-control-warm.matvec-lane-parity` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd_parity` | 10.22 [9.38, 10.83] | 271.00 [248.00, 284.00] | 2702.00 [2606.00, 2777.00] |
| `ragged-r1024-32w-control-warm.matvec-lane-parity` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_parity_fn` | 5.34 [4.74, 5.92] | 146.00 [122.00, 170.00] | 2702.00 [2606.00, 2777.00] |

Raw counter passes, sampled symbol reports, commands, host facts, and annotated instruction samples are retained beside this table.
