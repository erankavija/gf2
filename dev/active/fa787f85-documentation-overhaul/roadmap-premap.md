# Roadmap pre-map: deleted roadmaps vs code and jit

Pre-map for the four planned roadmap-verification tasks under `fa787f85`.
Sources: `git show 61c3f0a6a^:ROADMAP.md`,
`git show 61c3f0a6a^:crates/gf2-core/ROADMAP.md`,
`git show 791beb2b0^:crates/gf2-coding/ROADMAP.md`. Every planned, unchecked
or future item gets one row. Completed items are listed only where the roadmap
left a follow-up open.

Classes: `DONE` (in code; file/module cited), `TRACKED` (open or done jit issue;
short id cited; `archived` is a closed container), `DROPPED` (obsolete; reason),
`GAP` (untracked relevant work). `Conf` is `high` unless the mapping rests on a
title match or an inference; `low` rows need verification first.

Evidence shorthand: jit states as of 2026-10-01 (`jit query all --json`,
894 issues). Paths are workspace-relative.

## 1. ROADMAP.md (workspace)

### 1.1 Planned milestones

| Item | Class | Evidence | Conf |
|---|---|---|---|
| M17 GPU/FPGA BP prototypes | DONE | GPU: `crates/gf2-kernels-hip/hip/ldpc_bp.hip`, `crates/gf2-sim/src/gpu/ldpc_bp.rs`; epic `806eb14e` archived, kernel `a930be7f`, decision `86a363aa`. FPGA half: TRACKED `adc75ba7` ready. | high |
| M18 QAM soft-decision demapping | DONE | `crates/gf2-coding/src/modem/` (`gray_qam_mapper.rs`, `fast_gray_qam_demapper.rs`, `gpu_demapper.rs`); `d4851c3d` archived, `52112411`, `db1dda70`. | high |
| M19 End-to-end DVB-T2 FEC + BICM simulation | DONE | `crates/gf2-coding/src/dvb_t2_bicm_harness.rs`, gf2-sim DVB-T2 preset `5d0a3fad`; chain `003e4088`, FER run `e4849f07`, campaign `2928ccce` archived. | high |
| M20 Competitive benchmarking vs Magma/Sage/AFF3CT | TRACKED | `d77176e5` ready (Magma/Sage/AFF3CT); done parts: `18e69a1a` (aff3ct/IT++), `64c88ae4` (fflas-ffpack/M4RI), `01ae4c20` SOTA report; `1362381c` backlog (end-to-end results). | high |
| M21 GRAND universal decoder | DONE | `crates/gf2-coding/src/grand/` (`orbgrand.rs`, `sogrand.rs`); `6efb756b` archived, `d5dc78e8`, `f03ea0fd`. | high |
| M22 5G polar CRC-aided SCL | TRACKED | `b81c239c` backlog, `b94e2821` backlog, `e748fab4` ready, `21eb03e8` backlog. No polar code module in `crates/gf2-coding/src`. | high |
| M23 Neural-aided BP | TRACKED | `9a5662ff` ready. | high |
| M24 SDR / GNU Radio blocks | TRACKED | `56467231` ready (C FFI), `b8baca8a` backlog (OOT module), `21922c59` backlog, `dfca71b8` backlog, `dd153981` backlog, `fcb09e6f` ready. | high |

### 1.2 Key dependencies (cross-crate)

| Item | Class | Evidence | Conf |
|---|---|---|---|
| GF(2^m) -> BCH algebraic decoding | DONE | `crates/gf2-core/src/gf2m`, `crates/gf2-coding/src/bch/`. | high |
| Primitive polynomials -> BCH field construction | DONE | `crates/gf2-core/src/primitive_polys.rs`, `bch/spec.rs`. | high |
| Sparse matrices -> LDPC BP | DONE | `crates/gf2-core/src/sparse.rs`, `ldpc/edge_layout.rs`. | high |
| Polynomial arithmetic -> BCH syndromes | DONE | `bch/core.rs`; `70972f06` FieldPoly. | high |
| Rank/select -> sparse graph ops | DONE | `crates/gf2-core/src/bitvec.rs` rank/select. | high |
| Polar transforms -> 5G polar research | TRACKED | transform DONE (`bitvec.rs::polar_transform`); consumer is M22 (`b81c239c`). | high |

### 1.3 Research goals

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Primitive polynomial testing matches/exceeds CAS | DONE | `crates/gf2-core/src/primitive_polys.rs`; `dev/archive/legacy/crates/gf2-core/docs/BENCHMARKS.md` (3-340x vs SageMath). | high |
| GF(2^m) arithmetic via zero-cost abstractions + SIMD | DONE | `crates/gf2-kernels-simd/src/gf2m*.rs`; `2c7548ae`, `577b9e7f`. | high |
| Top-tier in Polynomial Systems Solving benchmarks | DROPPED | No Groebner/system-solving work anywhere; the CAS comparison moved to FieldMatrix linear algebra vs fflas-ffpack/M4RI (`64c88ae4`, `01ae4c20`). | low |
| GRAND for short codes | DONE | `crates/gf2-coding/src/grand/`. | high |
| Neural-aided BP | TRACKED | `9a5662ff` ready. | high |
| Spatially-coupled LDPC, sliding window | TRACKED | `5f4afdf5` ready. | high |
| Polar codes with CRC-aided SCL | TRACKED | `b81c239c` backlog. | high |
| Novel constructions documented and validated | TRACKED | Delivered: `8b1609a8` (RM subcode), `d0e2af0c` (QC-GLDPC+SOGRAND), `crates/gf2-coding/src/product/`; open: `55087229` backlog (PAC codes). | low |
| Rigorous FER curves vs theoretical bounds | TRACKED | Curves DONE (`e4849f07`, `92086311`, `831bfc4a`); bounds: `aed96ef9` ready (finite-blocklength bounds). | high |
| Open reproducible benchmarks | DONE | `dev/benchmarks/`, `a03b2556` container image, `7cd9afdb` pinned artefact. | high |

### 1.4 Open research questions

| Item | Class | Evidence | Conf |
|---|---|---|---|
| GPU LDPC memory- vs compute-bound | DONE | `43fb19e2`, `24c11004`; `dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md`. | high |
| FPGA feasibility for functional Rust -> HDL | TRACKED | `adc75ba7` ready. | high |
| GPU vs multi-core CPU crossover for LDPC | DONE | `9c37ec8c`, `a9e461de` (permanent), `86a363aa`. | high |
| GRAND vs algebraic: when is GRAND faster | GAP | Only `9d8cb409` ready (SOGRAND throughput); no head-to-head GRAND-vs-BCH/OSD comparison issue. | low |
| Normalized/offset min-sum gains | DONE | `crates/gf2-coding/src/ldpc/min_sum.rs`; `c3ea6855`. | high |
| Quantized LLRs 3-8 bit | TRACKED | `d69b964e` ready, `f63a2464` in_progress. | high |
| Structured LDPC encoding avoiding dense matrices | DONE | `crates/gf2-coding/src/ldpc/encoding/ira.rs`; `82dd7384`. | high |
| Neural-aided BP iteration reduction | TRACKED | `9a5662ff` ready. | high |
| End-to-end DVB-T2 latency budget | GAP | Throughput profiled (`3be770d5`, `9fb40c83`), no latency-budget analysis issue. | low |
| SDR: Rust vs GNU Radio C++ throughput | TRACKED | `dfca71b8` backlog (benchmark vs GNU Radio FEC blocks). | high |
| Real-signal validation vs test vectors | TRACKED | `dfca71b8`, `fcb09e6f`. | high |
| CAS crossover Rust+SIMD vs Magma/Sage (m > 32?) | TRACKED | `d77176e5` ready; partial `53c5a8c0`, `73ab8eef` (NTL/FLINT). | high |
| FEC decoders vs AFF3CT/IT++ | DONE | `18e69a1a`, `c077a88b`, `3be770d5`; continuation `1a379447` backlog. | high |
| Karatsuba vs FFT for m > 64 | DONE | `53c5a8c0` crossovers, `e0b6f940` NTT, multi-word `6fb4abad` (`crates/gf2-core/src/gf2m/wide.rs`); additive NTT TRACKED `c7cfd37e`/`24701af9`. | high |
| Shannon gap of practical LDPC decoders | DONE | `e4849f07` curves vs reference, `325e5c89` capacity fix; bounds follow-up `aed96ef9`. | high |
| Finite-length polar vs LDPC, N < 10K | TRACKED | `21eb03e8` backlog (blocked on polar implementation). | high |
| SC-LDPC threshold saturation | TRACKED | `5f4afdf5` ready. | high |

### 1.5 Long-term vision

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Competitive CAS for binary field research | TRACKED | `d77176e5` ready, `2caf738d` backlog (1.0 release). | high |
| Publication-worthy novel constructions | TRACKED | see 1.3 row; `55087229`. | low |
| Industry-standard open FEC benchmark suite | TRACKED | `c077a88b` done, `1362381c` backlog. | high |
| Educational tool with pedagogical examples | TRACKED | `315f4de5`, `5f3d0ff9`, `be331e20`, `0056e853` ready; epic `fa787f85`. | high |

### 1.6 Publication and validation

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Technical reports on implementations | DONE | `01ae4c20`, `24c11004`, `a9ab0a4f`; `dev/benchmarks/*/README.md`. | high |
| Open-source benchmark suites | DONE | `6ed7f050`, `dev/benchmarks/`. | high |
| Conference/journal submissions (ISIT, ICC, Trans. IT) | GAP | No issue tracks paper writing or submission. | low |
| DVB-T2 bit-exact compliance | DONE | `4cdaf1c5` (TP04->TP07a, 202/202), `dev/archive/legacy/crates/gf2-coding/docs/DVB_T2.md`. | high |
| 5G NR polar vs 3GPP vectors | TRACKED | `b81c239c` backlog (5G NR LDPC vectors DONE: `dd22a099`, `acf9b11a`). | high |
| Decode real-world DVB-T2 captures | TRACKED | `fcb09e6f` ready, `dfca71b8` backlog. | high |
| Compete with commercial SDR implementations | DROPPED | No commercial comparator in reach; superseded by open-source baselines `1a379447`, `18e69a1a`. | low |
| Benchmarks reproducible with published code | DONE | `a03b2556`, `7cd9afdb`. | high |
| Methodology documentation | DONE | `5102d87a`, `1d6043e8` acceptance protocol. | high |
| Comparison with commercial tools (Magma) | TRACKED | `d77176e5` ready. | high |
| Data and FER curves available | DONE | `152388f4` reference data, `dev/benchmarks/`. | high |

### 1.7 Contributing areas

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Benchmarking on Intel/AMD/ARM | TRACKED | `363556e6` done (AVX2-only sweep), `194b902a` ready (NEON), `4b5d8948` ready (AVX-512). No Intel-host issue. | low |
| Novel decoding algorithms | TRACKED | `9a5662ff`, `5f4afdf5`, `55087229`, `e15041f8`. | high |
| SIMD kernel optimization | DONE | `crates/gf2-kernels-simd/`; epics `97bf0879`, `026fc832` archived. | high |
| GPU/FPGA acceleration experiments | TRACKED | GPU DONE (`806eb14e`); FPGA `adc75ba7`. | high |
| Standard codes: 5G NR | TRACKED | LDPC DONE `dd22a099`; polar `b81c239c`. | high |
| Standard codes: DVB-S2X | GAP | No issue mentions DVB-S2/S2X. | low |
| Property-based tests for new algorithms | TRACKED | `3d522ba0`, `799af6fe`, `493a5330`, `b146e998` backlog. | high |
| Integration tests with real-world signals | TRACKED | `dfca71b8`. | high |
| Educational examples with decoding traces | TRACKED | `315f4de5` ready. | high |
| Research notes documenting experiments | DONE | `dev/research/`, `dev/studies/`. | high |
| Performance analysis and optimization guides | DONE | `dev/archive/legacy/crates/gf2-coding/docs/SIMD_PERFORMANCE_GUIDE.md`, `dev/archive/legacy/crates/gf2-core/docs/KERNEL_OPTIMIZATION.md`; sweep `f357b3dc` backlog. | high |

## 2. crates/gf2-core/ROADMAP.md

### 2.1 Phase 14 next steps

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Benchmark poly utilities vs SageMath/NTL (optional) | DONE | `73ab8eef` (NTL/FLINT), `53c5a8c0`; `dev/archive/legacy/crates/gf2-core/docs/POLY_UTILITIES_PERFORMANCE.md`. | low |
| Migrate `poly_from_exponents` into gf2-coding BCH | DONE | gf2-coding roadmap Technical Debt row marks it complete; `bch/spec.rs` uses core polynomials. | high |

### 2.2 Phase 2 wide buffer optimization

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Higher unrolling factors 8x/16x | DROPPED | `42a22210` rejected (Measure the XOR unroll portfolio) under breakdown `30c3aef1`; scalar kernel stays at `UNROLL = 4` (`crates/gf2-core/src/kernels/scalar/logical.rs`). | high |
| BitSlice zero-copy operations | DROPPED | `3f57a279`, `552fae3b`, `4497e541` rejected (zero-copy outcome not selected). | high |
| Specialized mid-range 8-64 word kernels | TRACKED | `2037941f` backlog, `2ad3a3e0` backlog (evidence synthesis); planning `8d8be934` done. | high |
| Profiling plan: real workloads, unroll microbench, decision >15% | DONE | `04b85d10` (profile bit-storage costs in production consumers), `30c3aef1` breakdown done. | high |

### 2.3 Other planned phases

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Phase 6b SIMD polar transforms, cache blocking N > 8K | TRACKED | `b3766524` ready, `07a1813a` backlog, `cdb951cc` ready (rename). | high |
| Phase 10 GF(p^m) arithmetic | DONE | `crates/gf2-core/src/gfp`, `crates/gf2-core/src/gfpn/`; `e095a100` archived, `350bff7f`, `3f4b946c`. | high |

### 2.4 Future directions

| Item | Class | Evidence | Conf |
|---|---|---|---|
| AVX-512 backend | TRACKED | `4b5d8948` ready, `f8d230ef` ready, `b59fa661`/`c7c0e991` backlog; partial code `crates/gf2-kernels-simd/src/x86/bipedal_avx512.rs`. | high |
| ARM NEON for AArch64 | TRACKED | `194b902a` ready; `a8cdcfe3` rejected (PMULL); stubs only in `crates/gf2-core/src/kernels/aarch64.rs`. | high |
| GPU acceleration | DONE | `crates/gf2-kernels-hip/`; FieldMatrix GPU `16283d6f` ready. | high |
| Batch polynomial operations | DONE | `a7c81834`, `bdf95060`, `2e7db385`. | high |
| Extended field degrees m > 64 | DONE | `crates/gf2-core/src/gf2m/wide.rs`; `6fb4abad`, `7c954fb5`. | high |
| SOTA polynomial factorization | GAP | Only irreducibility certificates (`19fe9394`) and minimal polynomials (`6aac5c44`); no factorization issue. | low |
| Novel sparse matrix algorithms | DONE | `5ce13bae` Markowitz pivoting, `cbf576d1` Cuthill-McKee, `eb57f944`. | high |
| Hardware-optimized implementations | DONE | tuning profiles `220cab0b`, SOTA epics `97bf0879`/`026fc832` archived. | high |

### 2.5 Phase 12 File I/O deferred items

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Compression support | DROPPED | Flag bit reserved in `crates/gf2-core/src/io/header.rs` but no codec; roadmap marked "not needed"; no issue. | low |
| Checksum verification | DONE | BLAKE3 payload checksum in `crates/gf2-core/src/io/mod.rs`; `e992c5b0`. | high |

### 2.6 Roadmap priorities

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Phase 2 profiling decision | TRACKED | `2037941f` backlog (see 2.2). | high |
| Extended SIMD (AVX-512, NEON) | TRACKED | see 2.4. | high |
| Research algorithms from downstream usage | DONE | `3be770d5`, `04b85d10` monitored consumers; SOTA epics. | low |

## 3. crates/gf2-coding/ROADMAP.md

### 3.1 Primary goal and phases C3, C5, C9

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Primary goal: DVB-T2 FER over AWGN | DONE | `e4849f07`, `2928ccce` archived; `crates/gf2-coding/src/bin/sim_runner.rs`. | high |
| C3 Quantization strategies (fixed-point LLRs) | TRACKED | `d69b964e` ready, `f63a2464` in_progress. | high |
| C5 Irregular LDPC (degree distribution) | TRACKED | `6ed3c78b` ready. | high |
| C5 Normalized/offset min-sum | DONE | `ldpc/min_sum.rs`; `c3ea6855`. | high |
| C9 BCH throughput benchmarks | DONE | `88ca7d2f` baseline receipt; `benches/bch_parallel.rs`. | high |

### 3.2 Phases C4 and C6 (duplicated section)

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Syndrome table optimization (compressed mapping) | GAP | No issue; soft-decision baselines (`312200e4` OSD, GRAND) displaced it but nothing records the drop. | low |
| Berlekamp-Massey for BCH | DONE | `crates/gf2-coding/src/bch/core.rs` (C9). | high |
| Chien search in GF(2^m) | DONE | `bch/core.rs`. | high |
| SOVA for convolutional codes | TRACKED | `e15041f8` ready; BCJR SISO exists (`crates/gf2-coding/src/bcjr/`). | high |

### 3.3 Phase C7 polar and modern codes

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Bhattacharyya bit-channel reliability sorting | TRACKED | `e748fab4` ready. | high |
| Information/frozen bit selection | TRACKED | `e748fab4` ready. | high |
| Rate-compatible polar families | GAP | Not named in `e748fab4`/`b81c239c`; 5G rate matching would cover part. | low |
| Fast Hadamard-like transforms on BitMatrix | DONE | `crates/gf2-core/src/bitvec.rs::polar_transform*`; SIMD `b3766524` ready. | high |
| SC decoder | TRACKED | `b94e2821` backlog. | high |
| SCL decoder with path metrics | TRACKED | `b94e2821` backlog. | high |
| CRC-aided SCL | TRACKED | `b81c239c` backlog. | high |
| Efficient factor graph representation | GAP | No issue; likely absorbed by `b94e2821`. | low |
| FER simulation over AWGN for polar | TRACKED | `21eb03e8` backlog. | high |
| Shannon limit comparison | TRACKED | `21eb03e8`. | high |
| Rate vs Eb/N0 characterization | TRACKED | `21eb03e8`. | low |
| Capacity-approaching verification across N | TRACKED | `21eb03e8`. | low |
| Comparison with LDPC | TRACKED | `21eb03e8`. | low |
| Reuse AWGN/LLR framework, rank/select integration | TRACKED | Inherent in `21eb03e8`/`b94e2821`; framework DONE (`simulation.rs`, `llr.rs`). | low |

### 3.4 Phase C8 compression experiments

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Bit-level transforms (RLE, delta, XOR chaining) | TRACKED | `0405c452` ready. | high |
| Entropy modeling playground | GAP | Not named in `0405c452`. | low |
| Comparative raw vs transformed benchmarks | TRACKED | `0405c452` (inferred). | low |

### 3.5 Phase C10 residual items

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Remaining 11 DVB-T2 table data files | DONE | 12 files `crates/gf2-coding/src/ldpc/dvb_t2/table_*.txt`. | high |
| Full LDPC test-vector validation (ignored tests) | DONE | C10.6.6 in the same roadmap; `4cdaf1c5`. | high |
| CPU profiling of encoding hotspots | DONE | C10.6.8; `3be770d5`. | high |
| C10.7 FER simulation over AWGN | DONE | `e4849f07`, `152388f4`, `fd73e8a8`. | high |
| C10.7 Live DVB-T2 reception demo with SDR | TRACKED | `fcb09e6f` ready. | high |

### 3.6 Phase C11 parallel computing

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Week 8+ Option A: normalized min-sum, early termination | DONE | `c3ea6855`; further `0fc3c9d0` backlog. | high |
| Week 8+ Option B: GPU prototype | DONE | `806eb14e` archived. | high |
| Week 8+ Option C: full FEC chain | DONE | `003e4088`. | high |
| C11.3 M3 throughput vs batch size | DONE | `9c37ec8c`. | high |
| C11.3 M3 GPU vs 24-core crossover | DONE | `9c37ec8c`, `86a363aa`. | high |
| C11.3 M3 GPU utilization profile | DONE | `a9284086`, `43fb19e2`. | high |
| C11.3 M3 `docs/GPU_FEASIBILITY_STUDY.md` | DONE | Delivered as `dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md` (`24c11004`); roadmap path never created. | high |
| C11.3 go/investigate/abandon decision | DONE | `86a363aa`. | high |
| C11.4 Extend HIP across LDPC, BCH, Viterbi | TRACKED | LDPC `a930be7f`, BCH syndrome `9012f8a0`, BCJR `795c156e` done; Viterbi-on-GPU has no issue (GAP). Production rollout explicitly downstream of `806eb14e`. | low |
| C11.4 SoA memory layout for coalescing | DONE | `43fb19e2` (QC layout, fp16), `23d3525f`. | low |
| C11.4 CUDA backend | DROPPED | Explicit non-goal in `806eb14e` and roadmap banner. | high |
| C11.4 Automatic backend selection with CPU fallback | TRACKED | gf2-sim hybrid executor `75c22fa8`, `ed575f15` OOM fallback done; production multi-backend selection is a stated non-goal of `806eb14e`; `bfd1aa89`/`d77519a3` rejected. | low |
| C11.4 Occupancy/bandwidth tuning, 500-1000 Mbps target | TRACKED | `23d3525f` done (5G NR real-time); DVB-T2 GPU target `1a379447`-adjacent; no explicit Mbps-target issue. | low |
| C11.5.1 FPGA survey, resource analysis, latency estimate, cost-benefit | TRACKED | `adc75ba7` ready. | high |
| C11.5.2 FPGA platform, Verilog Viterbi, PCIe DMA, benchmark | GAP | Conditional on `adc75ba7`; no prototype issue. | low |
| Tier 2: 50-200 Mbps CPU SIMD | TRACKED | `1a379447` backlog, `3be770d5` done. | high |
| Tier 3: 200-1000 Mbps GPU | TRACKED | `23d3525f` done (NR), `806eb14e`. | low |
| Tier 4: 1-10 Gbps FPGA | TRACKED | `adc75ba7`. | high |
| RQ1 GPU memory/compute bound, crossover | DONE | `9c37ec8c`, `43fb19e2`. | high |
| RQ2 CSR vs ELLPACK for GPU | DONE | `43fb19e2` QC layout study. | low |
| RQ3 8-bit LLRs on GPU | TRACKED | `f63a2464` in_progress, `d69b964e`. | high |
| RQ4 FPGA unrolling factor | TRACKED | `adc75ba7`. | high |
| RQ5 BCH Berlekamp-Massey on GPU | DONE | `9012f8a0`, `aeab2ee4`; decision in `24c11004`. | high |

### 3.7 Phase C13 polish

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Unified error handling, Result types | TRACKED | `1b929ce5` ready. | high |
| Streaming vs batch trait unification | TRACKED | `3931ac6f` backlog. | high |
| Doc examples with visual syndrome/decoding traces | TRACKED | `315f4de5` ready. | high |

### 3.8 Phase C12 SDR integration

| Item | Class | Evidence | Conf |
|---|---|---|---|
| C12.1 `src/ffi.rs` C API (7 bullets: LDPC/BCH/Viterbi exposure, safety, header, C test) | TRACKED | `56467231` ready (one issue covers all bullets). | high |
| C12.2 `gr-gf2` OOT module (8 bullets: blocks, GRC, flowgraphs, tests, docs) | TRACKED | `b8baca8a` backlog, `21922c59` backlog. | high |
| C12.3 Conformance test vectors | DONE | `4cdaf1c5`. | high |
| C12.3 Benchmark vs GNU Radio FEC blocks | TRACKED | `dfca71b8` backlog. | high |
| C12.3 RTL-SDR/HackRF captured signals | TRACKED | `dfca71b8`, `fcb09e6f`. | high |
| C12.3 BER/FER comparison curves | TRACKED | `dfca71b8` (inferred). | low |
| C12.4 LuaRadio, SDRangel, PyO3 bindings | TRACKED | `dd153981` backlog. | high |
| C12.4 gr-satellites contributions | GAP | Not named in `dd153981`. | low |

### 3.9 Technical debt and research placeholders

| Item | Class | Evidence | Conf |
|---|---|---|---|
| Consolidate expensive LDPC doctests | TRACKED | `9b3452e9` ready; CI doctest gate `34adff85` in_progress. | high |
| Optimal structures for extremely sparse H | DONE | `ldpc/edge_layout.rs`; `3a37e0f6`, `f1a896f0`. | high |
| Table-based vs algebraic decoding switch point | GAP | No issue. | low |
| GPU offload feasibility for LDPC | DONE | `806eb14e`. | high |
| Compression transforms vs ECC ordering | GAP | Adjacent to `0405c452`, not named. | low |
| SDR float vs fixed-point LLRs | TRACKED | `d69b964e`. | high |

## 4. Counts

| Class | Rows |
|---|---|
| DONE | 65 |
| TRACKED | 75 |
| DROPPED | 6 |
| GAP | 13 |
| Total | 159 |

Low-confidence rows: 33.

## 5. GAP list

Genuinely untracked work surfaced by the three roadmaps. Each needs a verify
step before becoming an issue; confidence is low on all of them.

1. GRAND vs algebraic/OSD head-to-head: when is GRAND faster for short codes (ROADMAP.md 1.4).
2. End-to-end DVB-T2 latency budget (deinterleave + BCH + LDPC + QAM) (1.4).
3. Conference/journal submission track (ISIT, ICC, Trans. IT) (1.6).
4. DVB-S2X standard code implementation (1.7).
5. State-of-the-art polynomial factorization over GF(2^m) (gf2-core 2.4).
6. Syndrome table optimization with compressed mapping (gf2-coding C4/C6); candidate for DROPPED instead.
7. Rate-compatible polar code families (C7).
8. Efficient polar factor graph representation (C7); likely absorbed by `b94e2821`.
9. Entropy modeling playground for compression experiments (C8).
10. Viterbi decoder on the HIP backend (C11.4).
11. FPGA prototype: platform selection, Verilog Viterbi, PCIe DMA (C11.5.2); conditional on `adc75ba7`.
12. gr-satellites contributions (C12.4).
13. Table-based vs algebraic decoding switch point; compression vs ECC ordering (C placeholders).

Secondary notes for the verifiers:

- `docs/GPU_FEASIBILITY_STUDY.md` was never created; the report lives under `dev/archive/806eb14e-hip-gpu-prototype/`. Any surviving doc link to the roadmap path is stale.
- `aarch64.rs` holds detection stubs only; the NEON row is TRACKED, not DONE.
- Compression flag in the I/O header is reserved and unimplemented; confirm the DROPPED verdict with the owner.
