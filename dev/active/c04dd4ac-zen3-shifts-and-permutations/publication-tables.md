# Shift and permutation publication tables

> **Diátaxis Type:** Reference

## Accepted receipt authority

The receipt SHA-256 equals the acceptance summary's pinned digest. Protocol and addendum digests verify against the receipt-local snapshots. The current working tree and Git revision do not decide whether a measured receipt remains valid. Host and toolchain are observed in each receipt's sessions.

| Family / receipt | receipt SHA-256 | protocol snapshot | addendum snapshot | verdict / role | runtime host | toolchain |
|---|---|---|---|---|---|---|
| Residual workload: `dev/bench_results/c04dd4ac/residual-shift-profile/receipt.json` | `bb70db8ce001425b1cbd8d4b3d144beb04cba5331fb53a9a7dc27e2f60d45616` | `dev/active/f547c394/protocol.md` v4 `047b8f395637642852da8066efbf33eafa81da5d9f1296e0631a986e2b7604bb` | `7e7289e083b351d3a5c6778e90e07de019c634a49b92f606ee9c921abf6e3e41` | accepted / pilot | fraktaali / AMD Ryzen 9 5900X 12-Core Processor | `rustc 1.95.0 (59807616e 2026-04-14)` |
| BMI2 confirmation: `dev/bench_results/00dd43c3/v4-r1-confirmation/receipt.json` | `73d408e8b050aa3cc0695a9942e9ba8c52243ca60a908160c87ea6c746f98031` | `dev/active/f547c394/protocol.md` v4 `e0f67e4e8b80ae2baadcd93291dd086b033285d20521980b8daf218eab1401ad` | `e71ffbbf696d768639b315856d632903e694aa5da6e76fc49eef66a211012d89` | accepted / confirmation | fraktaali / AMD Ryzen 9 5900X 12-Core Processor | `rustc 1.95.0 (59807616e 2026-04-14)` |
| DVB packed consumer: `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-pilot/receipt.json` | `be4d9ad84207513d80a6c4e9b5d0b2b47ed106b6d73adbe5f8aa1100762833ef` | `dev/active/f547c394/protocol.md` v4 `047b8f395637642852da8066efbf33eafa81da5d9f1296e0631a986e2b7604bb` | `47865f3f83bba6dc085d91fcfc828cc0424a0869289a0505564b548ab2a3d869` | accepted / pilot | fraktaali / AMD Ryzen 9 5900X 12-Core Processor | `rustc 1.95.0 (59807616e 2026-04-14)` |
| NR LLR de-rate: `dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/receipt.json` | `e1041cde377fa5742b8f613530647222bcfede7c2f47710711f6c832669de03e` | `dev/active/f547c394/protocol.md` v3 `1d42ac3c965c816f4488a5524d17aa05fdea15f6011c6806a10754561f9f3c1c` | `ca2c9c97daec55c46a6257d50a41ff1d8cfe9c83ef34dcf316a8c519db205133` | accepted / confirmation | fraktaali / AMD Ryzen 9 5900X 12-Core Processor | `rustc 1.97.0 (2d8144b78 2026-07-07)` |

## Residual workload profile

Exploratory residual-versus-word-offset comparisons use different operations. Their ratios classify material workload cost and cannot select an implementation.
Per-arm estimates are upper medians with order-statistic uncertainty intervals across the paired calls (coverage 0.968750). The accepted ratio uses its separately reported confidence interval.

Source: `dev/bench_results/c04dd4ac/residual-shift-profile/receipt.json` and `dev/bench_results/c04dd4ac/residual-shift-profile/acceptance-summary.json`; protocol `047b8f395637642852da8066efbf33eafa81da5d9f1296e0631a986e2b7604bb`; pairs and confidence are in each row.

| Cell | scaling | pairs | baseline median [uncertainty interval] | candidate median [uncertainty interval] | ratio [confidence interval] | decision | outcome |
|---|---|---:|---|---|---|---|---|
| `left-small-r1-w64` | single-core-latency | 6 | 4.94 [4.93, 4.94] ns/call | 6.22 [6.20, 6.23] ns/call | 0.7941 [0.7918, 0.7958] at 0.9750 | regressed | pilot |
| `left-lane-crossing-r7-w64` | single-core-latency | 6 | 8.29 [8.26, 8.75] ns/call | 6.34 [6.33, 6.36] ns/call | 1.3086 [1.3040, 1.3453] at 0.9750 | improved | pilot |
| `left-byte-residual-r8-w64` | single-core-latency | 6 | 46.59 [46.39, 47.82] ns/call | 11.61 [11.54, 11.81] ns/call | 4.0083 [3.9417, 4.0908] at 0.9750 | improved | pilot |
| `left-resident-r63-w64` | single-core-latency | 6 | 653.80 [653.21, 654.26] ns/call | 117.43 [116.94, 117.85] ns/call | 5.5712 [5.5543, 5.5821] at 0.9750 | improved | pilot |
| `left-streaming-r65-w64` | sustained-throughput | 6 | 94,713.77 [94,358.25, 94,808.07] Mibit/s | 383,569.98 [380,594.87, 384,475.41] Mibit/s | 4.0476 [4.0173, 4.0710] at 0.9750 | improved | pilot |
| `right-small-r1-w64` | single-core-latency | 6 | 4.55 [4.53, 4.73] ns/call | 7.00 [6.97, 7.04] ns/call | 0.6505 [0.6471, 0.6639] at 0.9750 | regressed | pilot |
| `right-lane-crossing-r7-w64` | single-core-latency | 6 | 7.51 [7.46, 7.51] ns/call | 6.18 [6.16, 6.45] ns/call | 1.2131 [1.1869, 1.2161] at 0.9750 | not-worse | pilot |
| `right-byte-residual-r8-w64` | single-core-latency | 6 | 45.92 [45.60, 45.97] ns/call | 10.57 [10.47, 11.38] ns/call | 4.3438 [4.1741, 4.3824] at 0.9750 | improved | pilot |
| `right-resident-r63-w64` | single-core-latency | 6 | 652.63 [651.93, 653.85] ns/call | 68.65 [68.46, 72.89] ns/call | 9.5077 [9.1922, 9.5216] at 0.9750 | improved | pilot |
| `right-streaming-r65-w64` | sustained-throughput | 6 | 94,804.14 [94,587.72, 94,876.58] Mibit/s | 408,936.68 [389,660.76, 410,596.73] Mibit/s | 4.2602 [4.1442, 4.3331] at 0.9750 | improved | pilot |

## BMI2 route confirmation

The baseline holds the shipped scalar funnel; the candidate selects its BMI2 route for the same residual operation. The accepted ratio interval decides each confirmatory outcome.
Per-arm estimates are upper medians with order-statistic uncertainty intervals across the paired calls (coverage 0.999997). The accepted ratio uses its separately reported confidence interval.

Source: `dev/bench_results/00dd43c3/v4-r1-confirmation/receipt.json` and `dev/bench_results/00dd43c3/v4-r1-confirmation/acceptance-summary.json`; protocol `e0f67e4e8b80ae2baadcd93291dd086b033285d20521980b8daf218eab1401ad`; pairs and confidence are in each row.

| Cell | scaling | pairs | baseline median [uncertainty interval] | candidate median [uncertainty interval] | ratio [confidence interval] | decision | outcome |
|---|---|---:|---|---|---|---|---|
| `left-byte-residual-r8-w64` | single-core-latency | 24 | 49.01 [48.89, 49.11] ns/call | 29.66 [29.61, 30.05] ns/call | 1.6521 [1.6488, 1.6547] at 0.9958 | improved | pass |
| `left-resident-r63-w64` | single-core-latency | 24 | 659.05 [658.08, 661.21] ns/call | 337.49 [337.00, 338.85] ns/call | 1.9528 [1.9485, 1.9560] at 0.9958 | improved | pass |
| `left-streaming-r65-w64` | sustained-throughput | 24 | 94,212.31 [93,480.40, 94,515.56] Mibit/s | 167,607.00 [160,582.90, 171,148.83] Mibit/s | 1.7775 [1.7321, 1.7896] at 0.9958 | improved | pass |
| `right-byte-residual-r8-w64` | single-core-latency | 24 | 47.98 [47.81, 48.28] ns/call | 29.90 [29.65, 30.11] ns/call | 1.6048 [1.5990, 1.6124] at 0.9958 | improved | pass |
| `right-resident-r63-w64` | single-core-latency | 24 | 657.92 [656.90, 660.56] ns/call | 339.37 [338.72, 341.47] ns/call | 1.9386 [1.9329, 1.9415] at 0.9958 | improved | pass |
| `right-streaming-r65-w64` | sustained-throughput | 24 | 94,376.30 [94,131.66, 94,614.97] Mibit/s | 180,103.12 [170,438.08, 181,202.49] Mibit/s | 1.9082 [1.8369, 1.9170] at 0.9958 | improved | pass |

## DVB-T2 packed whole-consumer comparison

The one-frame BitPackedBatch boundary includes xdsopl PCTITL unpack, destructive-input copy with output allocation, and pack. Values are medians and order-statistic intervals across each exploratory cell's paired calls (coverage 0.968750). The acceptance decision remains exploratory.

Source: `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-pilot/receipt.json`, `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-pilot/acceptance-summary.json`; protocol `047b8f395637642852da8066efbf33eafa81da5d9f1296e0631a986e2b7604bb`.

| Cell | pairs | gf2 stage ns/call [interval] | xdsopl packed ns/call [interval] | decision |
|---|---:|---|---|---|
| `dvb-t2-qam16-r12-normal-warm-sim-stage-gap` | 6 | 184,608 [184,012, 185,160] | 429,012 [428,550, 432,087] | regressed |
| `dvb-t2-qam64-r12-normal-warm-sim-stage-gap` | 6 | 184,992 [184,644, 185,689] | 430,916 [428,080, 432,789] | regressed |
| `dvb-t2-qam16-r12-short-warm-sim-stage-gap` | 6 | 12,195 [12,179, 12,231] | 34,973 [34,903, 35,050] | regressed |
| `dvb-t2-qam64-r12-short-warm-sim-stage-gap` | 6 | 12,170 [12,140, 12,190] | 30,921 [30,876, 31,502] | regressed |
| `dvb-t2-qam16-r12-normal-streaming-sim-stage-gap` | 6 | 215,160 [214,828, 215,316] | 434,739 [433,556, 435,765] | regressed |

### xdsopl conversion and remaining work

Each part has its own median and order-statistic interval over the same pairs. Remainder includes PCTITL and the untimed-separately packed-batch wrap; it is not a PCTITL-only measurement. The gf2 stage has zero unpack, copy, and pack at this boundary; its whole cost appears above.

| Cell | pairs | unpack ns/call [interval] | input copy and output allocation [interval] | pack ns/call [interval] | remainder ns/call [interval] |
|---|---:|---|---|---|---|
| `dvb-t2-qam16-r12-normal-warm-sim-stage-gap` | 6 | 27,092 [26,997, 27,321] | 83,064 [82,764, 83,354] | 200,256 [199,672, 201,193] | 119,330 [118,525, 120,284] |
| `dvb-t2-qam64-r12-normal-warm-sim-stage-gap` | 6 | 27,021 [26,966, 27,081] | 82,783 [82,483, 83,030] | 198,719 [196,504, 199,628] | 123,050 [121,998, 123,380] |
| `dvb-t2-qam16-r12-short-warm-sim-stage-gap` | 6 | 6,799 [6,795, 6,825] | 1,400 [1,397, 1,413] | 11,116 [11,080, 11,152] | 15,659 [15,621, 15,690] |
| `dvb-t2-qam64-r12-short-warm-sim-stage-gap` | 6 | 6,823 [6,818, 6,830] | 1,412 [1,400, 1,427] | 11,192 [11,111, 11,780] | 11,515 [11,476, 11,536] |
| `dvb-t2-qam16-r12-normal-streaming-sim-stage-gap` | 6 | 27,100 [27,043, 27,275] | 83,127 [82,682, 83,427] | 205,141 [203,904, 205,775] | 119,588 [118,815, 120,560] |

## DVB-T2 path and BICM composition

Each path contributes one median-of-windows observation per session. The table gives the across-session upper median and order-statistic uncertainty interval (coverage 0.960938). Allocation counts are exact per-call censuses. Separate-process shares describe composition only; the paired receipt above decides materiality.

Source: `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r3-dynamic-profile/rep-NN/cases.json`; runtime host and invocation: `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r3-dynamic-profile-provenance.md` (SHA-256 `eb381c9a11d95c26b669549fa46b9a306dc9d31be33f34ba91baa9f7b236013f`).

| Path | sessions | ns/call [interval] | allocations/call | bytes/call |
|---|---:|---|---:|---:|
| `qam16-r12-normal-direct` | 9 | 191,633 [191,372, 192,054] | 1 | 8104 |
| `qam16-r12-normal-sim-stage` | 9 | 189,720 [189,599, 190,265] | 2 | 8192 |
| `qam16-r12-normal-xdsopl-packed` | 9 | 254,796 [254,463, 254,955] | 4 | 785704 |
| `qam16-r12-normal-bicm-channel` | 9 | 1,111,770 [1,108,887, 1,114,556] | 32 | 2341856 |
| `qam64-r12-normal-direct` | 9 | 190,360 [190,175, 190,542] | 1 | 8104 |
| `qam64-r12-normal-sim-stage` | 9 | 190,075 [189,890, 190,249] | 2 | 8192 |
| `qam64-r12-normal-xdsopl-packed` | 9 | 258,789 [258,613, 259,227] | 4 | 785704 |
| `qam64-r12-normal-bicm-channel` | 9 | 1,323,765 [1,321,319, 1,327,005] | 32 | 2451272 |
| `qam16-r12-short-direct` | 9 | 13,549 [13,537, 13,558] | 1 | 2032 |
| `qam16-r12-short-sim-stage` | 9 | 13,564 [13,544, 13,600] | 2 | 2120 |
| `qam16-r12-short-xdsopl-packed` | 9 | 34,421 [34,368, 34,548] | 4 | 196432 |
| `qam16-r12-short-bicm-channel` | 9 | 172,416 [171,197, 173,378] | 32 | 586184 |
| `qam64-r12-short-direct` | 9 | 13,553 [13,535, 13,579] | 1 | 2032 |
| `qam64-r12-short-sim-stage` | 9 | 13,570 [13,559, 13,598] | 2 | 2120 |
| `qam64-r12-short-xdsopl-packed` | 9 | 30,416 [30,405, 30,569] | 4 | 196432 |
| `qam64-r12-short-bicm-channel` | 9 | 199,178 [198,885, 202,546] | 32 | 614600 |

## NR de-rate-matching no-win

AFF3CT Puncturer_5G::depuncture is the operation-equivalent external arm inside its adapter. The accepted protocol-v3 confirmation reports the corrected confidence interval and outcome. Adapter unpack and pack are descriptive per-execution upper medians with order-statistic uncertainty intervals over the pairs (coverage 0.999997); they do not decide the verdict.

Source: `dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/receipt.json`, `dev/bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/acceptance-summary.json`; protocol `1d42ac3c965c816f4488a5524d17aa05fdea15f6011c6806a10754561f9f3c1c`. The full conversion and runtime-host breakdown is `dev/bench_results/eda07788/tables-nr-derate.md`.

| Cell | pairs | speedup [confidence interval] | decision | outcome | AFF3CT unpack ns [uncertainty interval] | AFF3CT pack ns [uncertainty interval] |
|---|---:|---|---|---|---|---|
| `nr-bg2-n256-k121-gap-native-vs-aff3ct` | 24 | 0.3491 [0.3472, 0.3504] at 0.9958 | regressed | fail | 71 [70, 74] | 62 [61, 63] |
| `nr-bg2-n1024-k400-gap-native-vs-aff3ct` | 24 | 0.3428 [0.3418, 0.3476] at 0.9958 | regressed | fail | 145 [143, 147] | 105 [104, 108] |
| `nr-bg2-n1440-k720-gap-native-vs-aff3ct` | 24 | 0.3197 [0.3186, 0.3208] at 0.9958 | regressed | fail | 189 [185, 194] | 137 [134, 146] |
| `nr-bg1-n1320-k1056-gap-native-vs-aff3ct` | 24 | 0.3263 [0.3251, 0.3274] at 0.9958 | regressed | fail | 167 [165, 177] | 115 [114, 117] |
| `nr-bg1-n2560-k2048-gap-native-vs-aff3ct` | 24 | 0.3292 [0.3282, 0.3312] at 0.9958 | regressed | fail | 376 [371, 386] | 365 [359, 369] |
| `nr-bg1-n2560-k2048-control-portable-vs-native` | 24 | 0.9992 [0.9905, 1.0031] at 0.9958 | not-worse | not-material | 0 [0, 0] | 0 [0, 0] |

## Content-pinned dispositions

The exploratory profile's receipt and summary match their context pins. The confirmation qualifies, records pass in both directions and no disqualifying cell. The predeclared retention rule in the pilot addendum and the completed outcome document select the BMI2 route. The lane-crossing pilot row stays exploratory; it is distinct from the unimplemented AVX2 nomination.

| Source | SHA-256 | role |
|---|---|---|
| `dev/active/00dd43c3/profile-context.json` | `9afdec8e2ce4a7f80361f314606f49c392399889a13e867212e5655c615b3d38` | profile context pin |
| `dev/active/00dd43c3/pilot-addendum.json` | `8712cd79a9d511d9bbc9b8701aa3241c85279959c3272f15c0c56f08a3df0d2f` | frozen retention rule |
| `dev/active/00dd43c3/confirmation-derivation.txt` | `36ee11f4502b047d43c11ad65aff4ab7a11b42033e2e92703430b9abecc0e4a2` | retained and dropped pilot cells |
| `dev/active/00dd43c3/confirmation-outcome.md` | `01ce89392397f2ad886b8a780a4d0634891e6ec6e6a49635ace0721d54263d19` | completed retention disposition |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/shift-profile.md` | `4a21c61059ac28b8fdc90007c595e0ff8495dabfc3aa8f8ba29338af86f2fce8` | material workload disposition |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/shift-feasibility-record.md` | `1738ab405e6da15b3b102dfc550050e88a70866aebde767ed623f7736ed02cc8` | Rust 1.95 and scalar-fallback feasibility |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-interleave-profile.md` | `7a29558048008ea9d1d608acf0ff6b0f3e5a1a05f4f45b697ab5f6d9de832ce5` | DVB not-material disposition |
| `dev/active/eda07788/findings.md` | `6a165de79f5e3a31665878dbf406ee6148f1ad25c1806b02aba7d1c49373d9c6` | NR no-win and unavailable mappings |
| `dev/active/c04dd4ac-zen3-shifts-and-permutations/investigation.md` | `a6c634fa1bdee3da38dd06cbe8f26c2898feb8ae869cd0eba6caca6c176dcde9` | consumer and comparator survey |
