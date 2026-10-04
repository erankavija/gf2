# Performance evidence catalog for the docs/reference evidence page

Scope: committed evidence that can back performance claims on the planned
`docs/reference` performance-evidence page (epic fa787f85, REQ-06, D-48;
invariant `benchmark-backed-performance`). Each candidate claim area lists the
best receipt, which of the six REQ-06 fields it records in-file (hardware,
build flags, dataset/workload, baseline, measurement date, commit), the headline
number with its baseline, and the gaps. The second part lists every
performance claim in permanent docs (`README.md`, crate READMEs,
`crates/*/docs/`, `docs/`) with its backing status. Paths are repository
relative. Catalog date: 2026-10-01.

Coverage: all 145 zen3 receipts, all loose `dev/bench_results/*.md`, the
tuning-profile and permanent-campaign receipts, gf2-sim and DVB-T2 receipts
(current and archived), and every performance statement in `README.md`,
crate READMEs, `crates/*/docs/*.md`, `benchmarks/README.md` and `docs/`.
Not covered: contents of raw `dev/benchmarks/*-criterion.txt` and
`*-perf-stat.txt`, `dev/archive/6efb756b-grand/`, `dev/studies/`,
`dev/simulation_results/`, rustdoc prose in `crates/*/src/` beyond the two
`lib.rs` lines in section 4.6, and `crates/*/docs/archive/`.

Field legend in tables: H hardware, F build flags/toolchain, W dataset or
workload, B baseline, D date, C commit SHA. `+` recorded in-file, `~` partial
or indirect (e.g. "standard development machine", command line only, content
digest instead of SHA), `-` absent.

## 1. Evidence classes

| Class | Location | H | F | W | B | D | C | Notes |
|---|---|---|---|---|---|---|---|---|
| zen3 campaign receipts (`zen3-benchmark-receipt-v1`) | `dev/bench_results/<issue>/<campaign>/receipt.json` | + | + | + (`plan.json`, arms) | control arm | + (`observed_utc`) | ~ content digests, no SHA | Schema in `dev/active/f547c394/protocol-v1.md`, `dev/tools/tuning-campaign-support/src/protocol.rs`. Section 2 lists each receipt. |
| Loose evidence reports | `dev/bench_results/*.md` (17) | + header | + command | + | + | + | - | Target of `relocate-bench-narrative` (REQ-14, D-49); links must follow the move. |
| Tuning-profile receipts | `dev/benchmarks/tuning_profiles/*.md` (+ `.sha256`) | + | + | + | + | + | ~ some (`gf2-a83583e0-...md` records producing commit `21790510c0aa`) | Selector non-regression, host calibration. |
| Permanent-campaign receipts | `dev/benchmarks/permanent_campaign/*.md` + `.csv` | + | + | + | + | + | + (`batched-f3-avx2-provenance-fixed.md`: clean revision `88474a74ceee`) | Only class with all six fields in one file. |
| gf2-sim receipts | `dev/benchmarks/gf2-sim/*.md`, `dev/archive/f9717e7e-gf2-sim/benchmarks/gf2-sim/*.md` | + | ~ command | + | + | + | ~ (`9e983ae26e`, `ec30b3e1` in prose; GPU BCH receipt has none) | Sections 3.6, 3.7. |
| DVB-T2 AWGN campaign | `dev/benchmarks/dvb_t2_awgn/*.csv`, `dev/archive/2928ccce-dvb-t2-awgn-campaign/` | ~ | ~ | + | - | ~ | - | Conformance curves, not throughput. |
| Raw criterion / perf-stat dumps | `dev/benchmarks/<id>-criterion.txt`, `*-perf-stat.txt` | - | - | ~ | - | - | - | Usable only via the report that cites them. |
| Crate-local tables | `dev/archive/legacy/crates/gf2-core/docs/BENCHMARKS.md`, `dev/archive/legacy/crates/gf2-core/benches/*_results.md`, `dev/archive/legacy/crates/gf2-core/docs/KERNEL_OPTIMIZATION.md` | ~ | ~ | + | + | - | ~ (`179be78`, `e94eb23` short SHAs in benches md) | Section 4 rates each claim. |

## 2. zen3 receipt.json inventory

145 receipts under `dev/bench_results/<issue>/<campaign>/receipt.json`
(excluding copies nested under `inputs/`). 144 carry
`schema: "zen3-benchmark-receipt-v1"`; `07ca8585/device-conformance-r1` is
`ldpc-device-conformance-v1` (GPU LDPC correctness, RX 6950 XT, hipcc 7.2,
2026-09-15, no timing, the only receipt that records a commit:
`source_identity.revision_informational` = `3e62efc0…`).

### 2.1 What the schema guarantees (`dev/tools/tuning-campaign-support/src/receipt.rs`, `deny_unknown_fields`)

| REQ-06 field | Guarantee | Where |
|---|---|---|
| Hardware | required | `host` (cpu_model, hostname, os_kernel, cpu_flags, topology, governors, smt, affinity); all 144 record AMD Ryzen 9 5900X on `fraktaali`, kernel 7.2.2 (135) or 7.2.6 (9) |
| Build flags | required toolchain, optional flags | `toolchain` (rustc 1.95.0 in 114, 1.97.0 in 30); per arm `build`, `executable_sha256`, `arguments`, `environment`; `rustflags` null or empty for 282 of 525 arms; C/C++ flags only as free text |
| Dataset / workload | indirect | cell `cell_id` naming plus hash-pinned `addendum` (path, snapshot, sha256) |
| Baseline | required | each cell has `baseline_arm`, `candidate_arm`, `core_arm`; `claimed` (decision + bootstrap interval) is optional and absent in measured-only pilots |
| Date | implicit | `host.observed_utc`; no top-level date field |
| Commit | absent | `source.revision` is informational, empty in all 144; identity rests on `source.producing` SHA-256 hashes of producing files |

Every receipt directory holds an `acceptance-summary.md`; most issue
directories hold a `tables.md` (04b85d10, 07ca8585, 12fdeb5b, 19513245,
1d0da41f, 1d4fd63d, 26465e6c, 3be770d5, 4c1e441f, 53c5a8c0, 5cbb6545,
6c6b09b1, 6fb89a3c, ad2a6a58, c077a88b, c7113c5a; eda07788 has `tables-v3.md`
and `tables-nr-derate.md`; 2037941f has `logical-tables.md` and
`isal-base-gap-*.md`; c04dd4ac has `residual-shift-tables.md`).

### 2.2 Confirmation receipts by claim area

`est` = median baseline time / median candidate time over the cells
(`cells[].claimed.interval.estimate`); above 1 means the candidate is faster.
When the candidate is an external library the ratio reads the other way
(external faster). Decisions: I improved, N not-worse, R regressed, ? inconclusive.
Dates are 2026. Pilots and smokes exist beside every confirmation and are
omitted here except where no confirmation exists.

| Claim area | Receipt (`dev/bench_results/…`) | Date | Flags | Baseline > candidate | Decisions | est median [min–max] |
|---|---|---|---|---|---|---|
| LDPC decode vs aff3ct, steady state, multicore | `3be770d5/v4-r1-3be770d5-ldpc-steady-multicore-confirmation` | 09-12 | ext, native `target-cpu=native` | gf2 NMS f32 > aff3ct flooding (DVB-T2 r1/2, NR BG1 Z384) | I6 | 15.67 [12.07–18.19] |
| LDPC decode vs aff3ct, single worker | `3be770d5/v4-r1-3be770d5-ldpc-steady-single-worker-confirmation` | 09-12 | same | same | I2 | 15.12 [12.93–17.32] |
| LDPC decode vs aff3ct after check-node update, single worker | `07ca8585/v4-r1-07ca8585-ldpc-update-comparator-single-worker-confirmation` | 09-14 | same | gf2 after > aff3ct flooding | I2 | 1.82 [1.74–1.91] |
| LDPC fixed-iteration vs aff3ct | `07ca8585/v4-r1-07ca8585-ldpc-update-fixed-iteration-confirmation` | 09-14 | same | gf2 after > aff3ct flooding | I2 | 2.05 [1.90–2.21] |
| LDPC check-node kernel vs aff3ct | `07ca8585/v4-r1-07ca8585-ldpc-update-checknode-confirmation` | 09-14 | same | gf2 checknode NMS > aff3ct checknode | I2 | 2.42 [2.28–2.56] |
| LDPC update before vs after (gf2 only), 1 worker | `07ca8585/v4-r1-07ca8585-ldpc-update-single-worker-confirmation` | 09-14 | native | gf2 before > gf2 after | I2 | 8.37 [6.76–9.99] |
| LDPC matched algorithm (earlier, superseded by 3be770d5) | `c077a88b/v3-r1-c077a88b-ldpc-matched-algorithm-confirmation` | 09-08 | same | gf2 NMS > aff3ct flooding | N1 R1 | 0.55 [0.14–0.96] |
| LDPC QC intra-frame (pilots only) | `f63a2464/v4-r1-f63a2464-ldpc-qc-*-pilot` (three pilots) | 09-19 | native | gf2 NMS > QC intra-frame; QC > aff3ct | measured only | — |
| NR rate-matched LDPC encode vs srsRAN / aff3ct | `12fdeb5b/2026-09-13-12fdeb5b-nr-encode-confirmation` | 09-13 | ext, native | gf2 native > srsRAN ×4, > aff3ct ×2 (BG1/BG2, n 256–8448) | I5 R1 | 20.77 [0.48–151.31] |
| NR LLR de-rate-matching vs aff3ct | `eda07788/2026-09-10-eda07788-nr-derate-confirmation` | 09-10 | cp `x86-64`; ext, native | gf2 native > aff3ct ×5 (BG2 n 256–1440) | N1 R5 | 0.34 [0.32–1.00] (aff3ct faster) |
| DVB-T2 bit interleave vs xdsopl | `eda07788/2026-09-08-eda07788-dvb-t2-v3-confirmation`; `c04dd4ac/dvb-interleave-profile/v4-r2-pilot` | 09-08; 09-17 | cp `x86-64`; ext, native | gf2 > xdsopl (QAM16/64 r1/2) | N2 R4; N5 R5 | 0.49 [0.46–1.00]; 0.75 (xdsopl faster) |
| GF(2^m) polynomial multiplication vs gf2x | `c7113c5a/v3-r1-baselines-confirmation`; `c7113c5a/v3-r1-host-targeting-confirmation` | 09-10 | ext, native; tp `x86-64-v3` | gf2 native > gf2x | I7 R4; I2 R2 | 166.46 [0.15–866.36]; 144.53 [0.63–304.35] |
| GF(2^m) wide polynomial vs gf2x (field 256/571) | `53c5a8c0/2026-09-13-53c5a8c0-polynomial-confirmation` | 09-13 | ext, native | gf2 long product > gf2x | I2 N1 R3 | 0.97 [0.08–97.07] |
| GF(2^m) clmul crossover (element vs batch path) | `53c5a8c0/2026-09-13-53c5a8c0-crossover-confirmation` | 09-13 | cp | element path > batch path | I4 N1 R1 | 1.92 [0.75–2.36] |
| Public clmul wide dispatch | `1c602857/2026-09-13-1c602857-public-clmul-confirmation` | 09-13 | cp | portable path > public path (4w/9w) | I3 N3 | 33.29 [1.00–113.79] |
| ymm clmul dispatch (regression, rejected) | `1d0da41f/v3-confirmation` | 09-08 | cp | baseline > candidate (raw batch 8/65/512) | R5 | 0.60 [0.58–0.79] |
| GF(2^8) matrix product vs M4RIE / ISA-L | `6c6b09b1/v4-r1-matrix-product-confirmation` | 09-13 | ext, native (ISA-L v2.32.1 gcc `-O3 -march=native`) | gf2 element/wide > m4rie, > isal (n=256) | I6 | 31.57 [9.04–361.40] |
| GF(2^8) pairwise vs M4RIE / gf-complete | `6c6b09b1/v4-r1-pairwise-control-confirmation` | 09-13 | same | gf2 batch > m4rie, > gfcomplete (4k/128k) | N2 R4 | 0.76 [0.53–1.13] |
| GF(2^8) region axpy vs ISA-L (comparator gap) | `6c6b09b1/v4-r1-region-axpy-confirmation` | 09-13 | same | gf2 element/wide > isal (4k/128k/8m) | I6 (gap in ISA-L's favour) | 679.02 [469.52–2259.84] |
| GF(2^8) shipped table paths vs scalar | `19513245/r1-matrix-confirmation`, `r1-vector-confirmation`, `r1-control-confirmation`; `4c1e441f/r1-dense-product-pilot`; `ad2a6a58/r1-axpy-pilot` | 09-13/14, 09-19 | cp | current/scalar > prototype/table | I6; I6; I3; I8; I10 | 6.33; 30.56; 2.32; 5.73; 49.69 |
| Popcount route selection | `5cbb6545/2026-09-13-5cbb6545-popcount-confirmation`; `26465e6c/v3-popcount-confirmation` | 09-13; 09-10 | cp, ext | legacy > resolved dispatch; production > nibble-LUT / libpopcnt / Mula | I3 N2 R1; I16 N18 ?8 R17 | 1.08 [0.91–1.34]; 0.99 [0.31–2.56] |
| Fused AND+popcount consumers | `5cbb6545/2026-09-13-5cbb6545-fused-confirmation`; `26465e6c/v3-and-popcnt-confirmation` | 09-13; 09-10 | cp | legacy > resolved fused; and-fused > scalar control / two-pass | I4 ?2; R6 | 1.23 [0.96–2.98]; 0.50 [0.20–0.84] |
| Bit-storage consumers (count, layout, logical) | `04b85d10/2026-09-10-04b85d10-{count,layout,logical}-v3-confirmation` | 09-10 | cp | scalar > simd backend; transpose scalar > detected; dispatched > resolved | I4 N1; I4 N1; I2 N2 | 1.20; 1.93; 1.09 |
| BitVec residual shift (BMI2 route) | `00dd43c3/v4-r1-confirmation`; `c04dd4ac/residual-shift-profile` | 09-27; 09-17 | cp | residual scalar > residual gated; production > word-aligned control | I6; I7 N1 R2 | 1.84 [1.60–1.95]; 4.03 [0.65–9.51] |
| XOR / logical buffers vs ISA-L `xor_gen_base` | `6fb89a3c/v3-r1-6fb89a3c-logical-confirmation`; `2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot` | 09-10; 09-26 | cp `x86-64`; ext C `-O3 -march=native` | gf2 logical > isal base | R9; R12 | 0.05 [0.04–0.20]; 0.16 (ISA-L faster) |
| XOR unroll variants, matvec, NR construction (A/A and cfg sweeps) | `2037941f/2037941f-{logical-isolated-xor,logical-public-row-xor}/{bc091474-u2,bc091474-u4,v4-r1}-pilot`, `2037941f/2037941f-{dense-allocated-matvec,dense-isolated-fused-parity,logical-nr-construction}/v4-r1-pilot` | 09-19/26 | cp, `--cfg gf2_xor_unroll{2,4}` | a > b | N dominant | 0.98–1.02 |
| 64-bit block transpose vs M4RI / bitshuffle | `1d4fd63d/2026-09-13-1d4fd63d-external-confirmation`; `6fb89a3c/v3-r1-6fb89a3c-transpose-confirmation` | 09-13; 09-10 | cp, ext | gf2 ymm6 > m4ri, > bitshuffle | R2; I1 N1 R6 | 0.19 [0.14–0.24]; 0.49 (external faster) |
| Transpose lane selection (gf2 only) | `1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-confirmation` | 09-13 | cp | production > lane avx2 ymm6 | I3 N3 | 1.16 [1.02–1.25] |
| BCH generator matrix vs M4RI | `6fb89a3c/v3-r1-6fb89a3c-bch-confirmation` | 09-10 | cp `x86-64`; ext | gf2 bch genmatrix > m4ri (b1–b3) | R3 | 0.13 [0.06–0.20] (M4RI faster) |
| Protocol bring-up (xor-fold, decoder pipeline smokes) | `f547c394/*` (8 receipts) | 09-06/08 | cp | baseline > candidate | mixed | 0.75–1.53 |

Pilots without confirmation (measured only, no claim): `07ca8585/v4-r1-07ca8585-ldpc-update-comparator-multicore-pilot`,
`07ca8585/v4-r1-07ca8585-ldpc-update-multicore-pilot`, `3be770d5/v3-r1-*-pilot`, `f63a2464/*`,
`c077a88b/2026-09-08-*`, `26465e6c/2026-09-0{7,8}-*`, `6c6b09b1/2026-09-08-…-byte-field-pilot`.

## 3. Candidate claim areas

Every older file that names hardware names the same host: AMD Ryzen 9 5900X
(Zen 3, AVX2, no AVX-512), GPU AMD Radeon RX 6950 XT (gfx1030, ROCm 7.2.4).
Ratios are stated as the source states them.

| Area | Best evidence | Headline (vs baseline) | H | F | W | B | D | C | Gaps |
|---|---|---|---|---|---|---|---|---|---|
| GF(2^m) scalar / SIMD field kernels | `dev/archive/e095a100-gfpm-arithmetic/active/e095a100-completion-report.md`; zen3 receipts `dev/bench_results/1c602857/` (public clmul), `6c6b09b1/` (byte field) | AVX2+VPCLMULQDQ `Gf2mWide<4>` 6.4× vs scalar; SoA SIMD 7.91× at N=1000 | ~ ("Zen 3") | - | ~ | scalar | + (2026-04) | ~ branch tip `e00d775b` | Figures live in report prose; no standalone kernel receipt; zen3 receipts in section 2 are the reproducible replacement. |
| GF(p^m) kernels | same report | no separate figure | | | | | | | No standalone GF(p^m) benchmark exists. |
| GF(2^m) FieldMatrix GEMM vs M4RIE / NTL | `dev/archive/97bf0879-gf2-core-sota-performance/bench_results/2026-05-07-d82c00a3-gf2m-parity-evidence.md` | GF(2^16) 148.5× / 35.6× / 0.614× vs M4RIE at n=64/256/1024; GF(2^32) 5.7–6.7× vs NTL `mat_GF2E`; GF(2^8) 0.015–0.393× vs M4RIE (passes only under an `[aspirational]` amendment) | + | + (`target-cpu=native`) | + | + (M4RIE 20250128, NTL 11.6.0) | + | - | No commit; GF(2^8) result is a documented shortfall. |
| GF(p) FieldMatrix GEMM vs FFLAS-FFPACK | `dev/bench_results/2026-05-06-7a106fe4-gfp-parity-evidence.md`; `dev/archive/babcf05e-gf2-core-ppc-spiral/bench_results/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md`; n=4096: `2026-05-28-98336ab4-fgemm-n4096*.csv` + `run_98336ab4_fgemm_n4096_bench.sh` | 7a106fe4: GF(7) 0.578 / 0.679 / 0.708 of fflas wall time at n=64/256/1024; n=4096 all six cells pass at ≤1.5× (GF(251) 1.466×, GF(65521) 1.283×) | + (CCX1 pinning) | 2598b981 + ; 7a106fe4 and 98336ab4 - | + | + (fflas 2.5.0, Givaro 4.2.0, image sha in `benchmarks/image.lock`) | + | 2598b981 + (`a355a2fad29e`); others - | Flags and commit missing on the current evidence; small-n cells amended. |
| Cross-family dense LA scorecard (FFLAS, M4RI, M4RIE, NTL, LinBox) | `dev/archive/026fc832-gf2-core-sota-stretch/bench_results/2026-05-28-b0fa00af-sota-scorecard-final.md`; reference CSVs `dev/bench_results/2026-05-04-*-reference.csv` + `-host.txt` + `-perf-stat.txt` | Every cell PASS, AMENDED or EXCLUDED; GF(2) matmul 1.213× (n=64), 1.070× (n=256) of M4RI; GF(251) invert n=1024 3.217× (amended) | + (`host.txt`: lscpu) | - | + | + (all five libraries with versions) | + (2026-05-28) | ~ (`93dc5125`, `6c31fb87` in completion report) | Several PASS results are amendments; GF(2) pluq/solve_left unimplemented at that HEAD; extension-field GEMM design-only. |
| GF(2) BitMatrix GEMM vs M4RI (Strassen) | `dev/archive/babcf05e-gf2-core-ppc-spiral/bench_results/2026-04-29-strassen-matmul-crossover.md`; `2026-05-06-380e041a-m4ri-gray-schedule-criterion.txt`; `2026-05-28-bdf60780-matmul-gf2-smalln.csv` | Forced Strassen 0.83–0.91× of M4RM at n=2048–8192; auto dispatch n=1024 0.42× of M4RI | - | + | + | + | + | - | Supersedes `BENCHMARKS.md` "5–7× slower" but no host or commit. |
| LDPC decode CPU | `dev/archive/f9717e7e-gf2-sim/benchmarks/gf2-sim/cpu-foundation-receipts.md`, `parallelism-receipts.md`, `baseline-single-thread.md`; `dev/benchmarks/gf2-sim/baseline-single-thread.csv`; zen3 receipts `dev/bench_results/3be770d5/`, `07ca8585/`, `f63a2464/` | 21.44 ± 0.22 fps at 24 threads = 13.22× over 1.6216 fps single-thread (DVB-T2 r1/2 16-QAM) | + | - | + | + | + (2026-06-08) | + (`9e983ae26e`, `ec30b3e1`; CSV has a per-row `commit_sha` column) | No RUSTFLAGS. |
| LDPC decode GPU (HIP) | `dev/archive/f9717e7e-gf2-sim/benchmarks/gf2-sim/gpu-stages-receipts.md`; `dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md` | GPU decode 28.98× CPU-24T (639.10 / 22.06 fps), 253.51× CPU-1T | + | ~ (hipcc `-O3 --offload-arch=gfx1030`, no RUSTFLAGS) | + | + | + (2026-06-09..18) | + (`f3f0aaa5`, `cba9e8d9`) | Receipts missing from `dev/benchmarks/gf2-sim/` though its README lists them. |
| gf2-sim hybrid executor | `dev/archive/2928ccce-dvb-t2-awgn-campaign/benchmarks/gf2-sim/hybrid-executor-receipts.md` | CPU+GPU 123.03 ± 9.16 fps = 5.74× CPU-24T | + | - | + | + | + | ~ | Lives under the DVB-T2 archive, not the gf2-sim one. |
| gf2-sim 5G NR real-time | `dev/archive/f9717e7e-gf2-sim/benchmarks/gf2-sim/5g-nr-realtime.md` | 17.45 ± 0.03 Mbps, ~11.5× short of the 200 Mbps target (target amended) | + (host load recorded) | - | + | + (TS 38.214 ≈ 91.7 Mbps) | + | ~ ("merged HEAD" in prose) | Headline is a shortfall; projections (50–83 Mbps) are estimates. |
| BCH syndrome GPU | `dev/benchmarks/gf2-sim/gpu-bch-syndrome-receipt.md` | GPU 7267.9 fps vs best CPU 82.7 fps (1T) = 87.88×; 625× vs 24T is context only (24T path is Arc-contended) | + | ~ (`--release`, hipcc `-O3`) | + (DVB-T2 Normal r1/2, 1024 frames, 5 repeats) | + | + (2026-06-17) | - | No commit SHA. |
| BCH / OSD | `dev/bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md`; `dev/archive/b7157be6-osd/active/b7157be6-completion-report.md`; correctness under `dev/simulation_results/osd-ebch-128-64/` | eBCH(128,64) OSD: 12.40× at 24 workers (51.6% efficiency), 6.28× at 8, byte-identical output across worker counts; reproduces Fossorier 1994 order-2 curve | + | - (rustc 1.97.0 only) | + (120 000 blocks, Eb/N0 1.55 dB) | 1 worker | + | + (`1e2d4885`) | No RUSTFLAGS. |
| Permanents F_3 (CPU) | `dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md` (+ `.csv`); `backend-selection-v1.md`; historical `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv` | Batched AVX2 3.57–6.17× over scalar at n=8..28; S1 historical 9.42× (n=32), 10.64× (n=36), ~6.9× (n=24) over `permanent_mod3_reference` | + | + (rustc 1.95.0, binary SHA-256) | + (seed, fixtures) | + | + (2026-08-10) | + (`88474a74ceee`) | Only area with all six fields. S1 n=32/36 rest on one sample; S3, S5 marked non-authoritative by `gf2_algebra_permanent/README.md`. |
| Permanents GPU | `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`; `dev/studies/b488f02c/feasibility-study.md` | GPU 28.65× / 30.32× over sequential AVX2 at n=24/28, M=256; 0.46× / 0.44× against best CPU path | + | - | + | + | + | ~ (`13b9143a`, harness uncommitted) | Corroboration only. |
| DVB-T2 conformance | `dev/archive/2928ccce-dvb-t2-awgn-campaign/benchmarks/dvb_t2_awgn/CLOSURE.md`; curves `dev/benchmarks/dvb_t2_awgn/curve_*.csv`; byte-identity `dev/benchmarks/gf2-sim/dvb-t2-regression-receipts.md`; aff3ct `dev/benchmarks/gf2-sim/comparison/README.md` | Gap to ETSI TS 102 831 Table 44 at FER 1e-4: 16-QAM 0.117 / 0.167 / 0.224 dB, 64-QAM 0.51–0.61 dB (gate amended to ≤0.5 / ≤0.65 dB); aff3ct v4.4.0 agreement 0.016 dB (DVB-T2), 0.003 dB (5G NR) at FER 1e-2 | + (GPU) | - | + (seed 42) | ETSI / aff3ct | + (2026-06-13) | - | Conformance, not throughput; regression receipt names RX 6900 XT while all other files name RX 6950 XT. |
| SIMD bit kernels (gf2-core) | `dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md`, `2026-09-01-eaae1b56.md`, `2026-08-20-host-calibration.md`; raw `dev/benchmarks/19bc3199-*`, `3168d114-*`, `cad241e6-*`, `jit-*-{matvec,mul}.txt` | `bit_backend.simd_min_words` selected value 4 | + | + | + | + | + | + (`21790510c0aa`) | Threshold evidence, not a speedup headline; raw criterion dumps carry no provenance. |

Directory notes: `dev/bench_results/54fd3f0b-sota-sparse-fieldmatrix/` is
empty; `dev/bench_results/97bf0879-gf2-core-sota-performance/` holds only
`themes/`; `dev/archive/d4851c3d-modem-framework` and
`dev/archive/b7157be6-osd` have no bench files of their own;
`dev/archive/6efb756b-grand/active/6efb756b-completion-report.md` (commit
`6199244`, 2026-04) was not mined.

## 4. Performance claims in permanent docs

Status: **backed** = a committed receipt records at least hardware, workload,
baseline and date and the page can link it; **partial** = evidence exists but
lacks two or more REQ-06 fields or predates the current code path;
**unbacked** = no committed evidence found; **estimate** = page itself marks it
as projected or target.

### 4.1 `README.md`

| Line | Claim | Status | Evidence |
|---|---|---|---|
| 31 | gf2-algebra has "fast matrix permanents" on CPU and GPU | qualitative | `dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md` |
| 135 | debug mode is 10–100× slower on SIMD / simulation code | unbacked (rule of thumb) | none |

### 4.2 `crates/gf2-core/README.md`

| Line | Claim | Status | Evidence |
|---|---|---|---|
| 125 | 3.4–3.6× SIMD speedup for bulk logical ops and popcount on operands >512 bytes | partial | `dev/archive/legacy/crates/gf2-core/docs/BENCHMARKS.md` Phase 3 table ("x86_64 with AVX2", no date/commit); `dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md` (simd_min_words=4, host, commit) remeasures the threshold, not the 3.4–3.6× figure; zen3 popcount receipts `dev/bench_results/5cbb6545/`, `26465e6c/` |
| 155 | BENCHMARKS.md gives performance vs M4RI / NTL / FLINT | partial | see 4.4 |

### 4.3 `crates/gf2-algebra/README.md`

| Line | Claim | Status | Evidence |
|---|---|---|---|
| 14–20 | Batched AVX2 F_3 permanent leads scalar by 3.57×–6.17× at n=8..28; single-matrix AVX2 is 2.86×–3.35× slower than scalar | backed (all six fields) | `dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md` (+ `.csv`); Ryzen 9 5900X, rustc 1.95.0, revision `88474a74ceee`, 2026-08-10 |
| 23–29 | Historical AVX2 dispatcher 6.9×/8.0×/10.6× over `permanent_mod3_reference` at n=24/28/36 | partial, labelled historical | `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`; host named, no commit, predates current dispatcher |
| 31–49 | GPU M=256 batch 28.65×/30.32× over sequential AVX2 (RX 6950 XT) | corroboration only, page says so | `dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`; no harness SHA |
| 224 | debug 10–100× slower | unbacked (rule of thumb) | none |
| lib.rs:40 | reference oracle used as the "50× speedup denominator" | partial | same S1 csv |

### 4.4 `dev/archive/legacy/crates/gf2-core/docs/`

| File:line | Claim | Status | Evidence |
|---|---|---|---|
| `BENCHMARKS.md:7` | matvec beats M4RI 2.7–58×, matvec_transpose 3.6–23.6×; 36–63× over initial implementation | partial | in-file Phase 13 tables; "standard development machine", no date/commit/flags |
| `BENCHMARKS.md:74-133` | 3–340× (up to 931×) faster than SageMath for primitive polynomials, GF(2^m) ops, sparse matvec | partial | in-file tables only; no hardware, date, commit; SageMath version absent |
| `BENCHMARKS.md:185-199` | 13–18× faster than NTL 11.6.0 for m≤16; 1.9× at m=32; 0.5× at m=64 | partial | in-file table; library versions given, no hardware/date/commit |
| `BENCHMARKS.md:201-219` | M4RI 20250128 is 5–7× faster for M4RM multiply | partial, likely stale | in-file; superseded by later GF(2) GEMM work (`dev/bench_results/2026-05-06-380e041a-m4ri-gray-schedule-criterion.txt`, `2026-05-28-bdf60780-matmul-gf2-smalln.csv`) |
| `BENCHMARKS.md:285-300` | Gauss–Jordan inversion 2.2–2.6× slower than M4RI | partial | in-file; "standard development machine" |
| `BENCHMARKS.md:337-491` | RREF phases: 32–40%, 6–61%, 22–64% improvements | partial | in-file; no hardware/date/commit |
| `BENCHMARKS.md` Phase 3 (~line 535) | SIMD 1.49× at 8 words, 3.43–3.56× at 64–1024 words; peak 97 GiB/s vs 28 GiB/s | partial | "x86_64 with AVX2"; no date/commit |
| `README.md:21-23` | SIMD 3.4–3.6×; RREF 150–170× faster than naive | partial / unbacked | 150–170× figure has no table in BENCHMARKS.md |
| `KERNEL_OPTIMIZATION.md:197-199,318-332,654-657` | same SIMD 3.4–3.6× and 97 vs 28 GiB/s; dispatch overhead ~0.8 ns | partial | duplicates BENCHMARKS.md Phase 3; cites `gf2-a83583e0-...md` for simd_min_words=4 |
| `KERNEL_OPTIMIZATION.md:90` | 40–80% faster than bit-twiddling alternatives | unbacked | none |
| `GF2M.md:12,91-93` | table multiply 10× faster than schoolbook; 17×/13×/1.9× vs NTL; 114 000×/70 000×/6 500× vs SageMath | partial | same numbers as BENCHMARKS.md Phase 9.4 |
| `POLY_UTILITIES_PERFORMANCE.md:9-10,68-70,198-213,241-242` | 13–18× vs NTL; 100–1000× vs SageMath; per-function 50–500× | estimate / unbacked | document is a design plan; figures are targets and "expected", no measurements committed |
| `POLAR_IMPLEMENTATION_PLAN.md:66,150,199` | FHT 12×/28×/81× vs naive at N=64/256/1024 | unbacked | no committed table or receipt found |
| `QUALITY_AUDIT_PLAN.md:236,293-296` | SIMD 2.57× avg, 3.4–3.5× peak; BitVec 8–17 GiB/s | partial | audit narrative; no receipt |
| `dev/archive/legacy/crates/gf2-core/benches/BENCHMARK_RESULTS.md` | Montgomery vs naive 1.8×/2.7×/3.0×/2.6× (Mersenne-61) | partial | commit `179be78`; "AMD Ryzen / Intel x86-64" (ambiguous host), no date/flags |
| `dev/archive/legacy/crates/gf2-core/benches/field_matrix_fusion_results.md` | fused vs eager ≈1.00–1.02 | partial | commit `e94eb23`, Mersenne-31, single core; host model absent |
| `dev/archive/legacy/crates/gf2-core/benches/strassen_threshold_results.md` | Winograd ≥1.2× over classical at threshold | partial | see file; also `dev/archive/babcf05e-gf2-core-ppc-spiral/bench_results/2026-04-29-strassen-matmul-crossover.md` |

### 4.5 `crates/gf2-coding/README.md` and `dev/archive/legacy/crates/gf2-coding/docs/`

| File:line | Claim | Status | Evidence |
|---|---|---|---|
| `README.md:101` | word-level (64×) × SIMD (4–8×) ≈ 256–512× over naive Gaussian elimination for LDPC preprocessing | unbacked (arithmetic estimate) | none |
| `README.md:131` | ~530 MB cache, ~13 min one-time preprocessing, cached encoders load in <16 ms | unbacked | none committed |
| `README.md:155` | debug 10–100× slower | unbacked (rule of thumb) | none |
| `LDPC_PERFORMANCE.md:13-16,35` / `PARALLELIZATION.md:47-48,152,244` / `DVB_T2.md:15-17,155-157` | LDPC encode 3.85 Mbps, decode 8.29 Mbps parallel (6.7× on 24 cores, 202 blocks), BCH >100 Mbps | partial, stale | no receipt; "24-core CPU" only; zen3 LDPC receipts (`dev/bench_results/07ca8585/`, `f63a2464/`, `3be770d5/`) and gf2-sim receipts (`cpu-foundation-receipts.md`: 21.44 fps at 24 threads) measure different workloads |
| `LDPC_PERFORMANCE.md:52-70` | f32 LLR 5%/9% faster per block | partial | in-file only |
| `LDPC_PERFORMANCE.md:103-148`, `PARALLELIZATION.md:157-203` | 2–4×, 1.5–2×, 5–10× targets; 20–50 Mbps expected | estimate (marked target/expected) | none |
| `SIMD_PERFORMANCE_GUIDE.md:7,30,57-58,217-224` | SIMD 4–8×; 256–512× vs bit-level; RREF 5–10 s → 1–2 s; cache generation 60–120 s → 16 s; 5.3×/5.5×/5.5× on DVB-T2 matrices | unbacked | approximate figures, no receipt, no hardware |
| `DVB_T2.md:127-157` | real-time requirement 31.4 / 50 Mbps vs current 3.85 / 8.29 Mbps | partial | requirement is derived from the standard; current figures see above |

### 4.6 Other permanent surfaces

| File | Claim | Status |
|---|---|---|
| `crates/gf2-kernels-simd/README.md:63-84` | `criterion-1.5x` gate: geomean ≥1.5× vs pinned baseline | gate definition, not a claim; evidence lives in `dev/bench_results/19bc3199-*`, `dev/benchmarks/*-criterion.txt` |
| `crates/gf2-stats/README.md` | none | — |
| `crates/gf2-sim` (no README; `src/lib.rs`) | none | — |
| `benchmarks/README.md` | defines `throughput_ops` and the external-library protocol; no figures | — |
| `docs/lean4-verification-pipeline.md` | none | — |
| `crates/gf2-core/src/lib.rs:286` | Mersenne backend gives 2× the throughput of generic Montgomery | partial; `dev/archive/legacy/crates/gf2-core/benches/BENCHMARK_RESULTS.md` and `dev/bench_results/2026-05-05-3d06224c-mersenne-baseline.csv` |

## 5. Gaps across the board

- No evidence class except the permanent-campaign receipts records all six
  REQ-06 fields in one file; zen3 receipts lack a commit SHA (content digests
  only; `source.revision` empty in all 144) and loose reports lack one
  entirely. The evidence page must pin the commit itself (the commit that
  added the receipt) for every link.
- zen3 receipts record `rustflags` as null or empty for 282 of 525 arms, so
  the page must state flags per arm from the receipt's `build` kind
  (conservative-portable, native, tuned-portable, external) rather than assume
  a single flag set; C/C++ flags for external arms are free text only.
- Several zen3 confirmations are regressions against external libraries
  (ISA-L XOR 0.05×, M4RI BCH genmatrix 0.13×, M4RI/bitshuffle transpose
  0.19–0.49×, aff3ct NR de-rate-matching 0.34×, xdsopl interleave 0.49×);
  the page should present these alongside the wins (LDPC decode 15×, NR
  encode 20×, GF(2^8) matrix product 31×, polynomial multiplication 166×).
- The same host (AMD Ryzen 9 5900X, `fraktaali`) measured everything;
  cross-host claims have no evidence.
- The gf2-coding throughput figures (3.85 / 8.29 Mbps, 6.7×) repeat across
  four files with no receipt; the page should either replace them with the
  zen3 LDPC receipts and gf2-sim fps receipts or label them stale.
- `BENCHMARKS.md` comparisons with SageMath, NTL and M4RI carry library
  versions but no host, date, flags or commit; the later external-library
  protocol (`benchmarks/README.md`, `dev/bench_results/2026-05-04-*-reference.csv`)
  is the reproducible replacement for the M4RI/NTL/FLINT rows.
- Polar FHT (81×), RREF 150–170×, SIMD_PERFORMANCE_GUIDE timings and the
  gf2-coding cache-generation timings have no committed evidence.
- `relocate-bench-narrative` (REQ-14) moves `dev/bench_results/*.md`; links
  from the evidence page must target the post-move paths or pin the commit.

## 6. Correction: direction of comparator estimates

The estimate column of §2.2 is the ratio of arm medians, median(gf2) / median(comparator), so an estimate above 1 means the comparator is faster. Sections 2.2 and 5 read several of these cells in the opposite direction. The original text stands above; this section records the contradiction (`@/inv/falsification-preserved`), established while authoring `docs/reference/performance-evidence.md` (1fca3739) from the receipts' acceptance summaries.

| Cell | Catalog reading | Receipt reading |
|---|---|---|
| LDPC decode, fixed-iteration and check-node update vs AFF3CT | gf2 faster | AFF3CT faster; estimates 1.74–2.57 |
| NR rate-matched LDPC encode vs srsRAN | gf2 faster | srsRAN faster at every size; estimates 6.89–151 |
| NR LLR de-rate-matching vs AFF3CT | "aff3ct faster" | gf2 faster; estimates 0.32–0.35 |
| ISA-L XOR, M4RI BCH and DVB-T2 interleave cells listed as regressions | gf2 slower | gf2 faster where the estimate is below 1 |

`docs/reference/performance-evidence.md` states each claim from its receipt, not from this catalog.
