# Dense baseline profile attribution

Each bracket is the 96.1% order-statistic interval for the median of nine full process passes. Counters include setup, calibration, and output release, divided by the arm's observed timed call count; the campaign receipt remains the authority for operation latency.

| Frozen cell | cycles/call | instructions/call | L1 misses/call | cache misses/call |
|---|---:|---:|---:|---:|
| `and-popcnt-8w-warm` | 39.19 [39.09, 41.45] | 151.92 [151.86, 152.79] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] |
| `and-popcnt-64w-warm` | 93.24 [92.48, 93.76] | 346.30 [346.26, 347.26] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] |
| `matvec-r1024-8w-warm` | 19827.52 [19791.83, 19845.40] | 97460.85 [97374.49, 97573.09] | 1189.48 [1188.31, 1190.75] | 18.94 [18.56, 19.22] |
| `matvec-r1024-64w-warm` | 83581.32 [82334.15, 85750.60] | 290085.02 [289768.36, 293048.23] | 9328.19 [9239.50, 9359.84] | 377.64 [319.77, 409.30] |
| `matvec-r1024-64w-streaming` | 118211.37 [117531.14, 119879.89] | 289562.24 [289452.89, 289748.49] | 9191.71 [9159.58, 9249.54] | 308.20 [305.76, 312.02] |
| `matvec-r1024-8w-scalar-reference-warm` | 17621.51 [17582.15, 17701.42] | 107177.64 [107131.87, 107234.08] | 1175.54 [1174.27, 1177.89] | 20.56 [20.06, 29.78] |

## Sampled cycle attribution

Each symbol share and count is the median and order-statistic interval over nine independent cycle-sampling passes. The table retains symbols whose median sampled share reaches 1%; every report and annotation remains beside the table.

| Frozen cell | symbol | share (%) | samples/pass | total samples/pass |
|---|---|---:|---:|---:|
| `and-popcnt-8w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 57.33 [56.45, 58.44] | 1634.00 [1610.00, 1655.00] | 2845.00 [2832.00, 2850.00] |
| `and-popcnt-8w-warm` | `dense-arm: dense_arm::run::{{closure}}` | 27.80 [27.19, 29.21] | 792.00 [769.00, 833.00] | 2845.00 [2832.00, 2850.00] |
| `and-popcnt-8w-warm` | `dense-arm: tuning_campaign_support::timing::time_calls` | 13.74 [13.30, 13.97] | 391.00 [379.00, 398.00] | 2845.00 [2832.00, 2850.00] |
| `and-popcnt-64w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 78.37 [77.85, 79.95] | 2247.00 [2229.00, 2328.00] | 2876.00 [2868.00, 2912.00] |
| `and-popcnt-64w-warm` | `dense-arm: dense_arm::run::{{closure}}` | 14.72 [13.28, 15.26] | 418.00 [389.00, 439.00] | 2876.00 [2868.00, 2912.00] |
| `and-popcnt-64w-warm` | `dense-arm: tuning_campaign_support::timing::time_calls` | 5.92 [5.70, 6.38] | 170.00 [164.00, 183.00] | 2876.00 [2868.00, 2912.00] |
| `matvec-r1024-8w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 58.58 [57.46, 59.03] | 1649.00 [1622.00, 1671.00] | 2822.00 [2815.00, 2827.00] |
| `matvec-r1024-8w-warm` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 30.44 [29.62, 31.62] | 857.00 [835.00, 894.00] | 2822.00 [2815.00, 2827.00] |
| `matvec-r1024-8w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 8.65 [8.49, 9.07] | 245.00 [240.00, 256.00] | 2822.00 [2815.00, 2827.00] |
| `matvec-r1024-64w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 77.71 [76.78, 78.74] | 2249.00 [2208.00, 2300.00] | 2852.00 [2847.00, 2947.00] |
| `matvec-r1024-64w-warm` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 18.26 [17.63, 18.97] | 525.00 [508.00, 552.00] | 2852.00 [2847.00, 2947.00] |
| `matvec-r1024-64w-warm` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 2.30 [2.19, 2.49] | 67.00 [64.00, 71.00] | 2852.00 [2847.00, 2947.00] |
| `matvec-r1024-64w-streaming` | `dense-arm: gf2_kernels_simd::x86::avx2::avx2_and_popcnt` | 74.87 [74.60, 75.63] | 2203.00 [2191.00, 2232.00] | 2939.00 [2897.00, 2981.00] |
| `matvec-r1024-64w-streaming` | `dense-arm: gf2_core::matrix::BitMatrix::matvec_simd` | 16.40 [15.97, 16.75] | 489.00 [468.00, 493.00] | 2939.00 [2897.00, 2981.00] |
| `matvec-r1024-64w-streaming` | `[unknown]: 0xffffffffa6f12633` | 1.70 [1.55, 1.77] | 49.00 [46.00, 52.00] | 2939.00 [2897.00, 2981.00] |
| `matvec-r1024-64w-streaming` | `dense-arm: gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn` | 1.50 [1.27, 1.80] | 44.00 [37.00, 53.00] | 2939.00 [2897.00, 2981.00] |
| `matvec-r1024-8w-scalar-reference-warm` | `dense-arm: gf2_core::matrix::BitMatrix::matvec` | 97.23 [96.94, 97.61] | 2724.00 [2698.00, 2738.00] | 2797.00 [2779.00, 2810.00] |

Raw counter passes, sampled symbol reports, commands, host facts, and annotated instruction samples are retained beside this table.
