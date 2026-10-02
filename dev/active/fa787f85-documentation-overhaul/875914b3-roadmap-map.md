# gf2-coding roadmap map

Source revision and file: 791beb2b0^:crates/gf2-coding/ROADMAP.md. Every row cites that file at the stated line. Items marked complete in the roadmap are listed only where an open follow-up remains.

Dispositions: Tracked (issue short ID), Delivered (code path or delivering issue), Obsolete (reason), Newly filed (exact issue title created by `875914b3-file-issues.sh`).

| Item | Citation | Disposition |
|---|---|---|
| Primary goal | 791beb2b0^:crates/gf2-coding/ROADMAP.md:3-6 | Delivered: crates/gf2-coding/src/dvb_t2_bicm_harness.rs, crates/gf2-coding/src/bin/sim_runner.rs; FER run e4849f07 |
| C3 quantization strategies (fixed-point LLRs) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:32 | Tracked: d69b964e, f63a2464 |
| C4 syndrome table optimization (compressed mapping) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:35 | Obsolete: SyndromeTableDecoder (crates/gf2-coding/src/linear.rs) serves small educational codes whose syndrome space fits a hash map; BCH uses Berlekamp-Massey and soft-decision decoding uses OSD and GRAND, so no consumer needs a compressed table |
| C4 Berlekamp-Massey for BCH | 791beb2b0^:crates/gf2-coding/ROADMAP.md:36 | Delivered: crates/gf2-coding/src/bch/core.rs |
| C4 Chien search | 791beb2b0^:crates/gf2-coding/ROADMAP.md:37 | Delivered: crates/gf2-coding/src/bch/core.rs |
| C4 SOVA | 791beb2b0^:crates/gf2-coding/ROADMAP.md:38 | Tracked: e15041f8 |
| C5 irregular LDPC codes | 791beb2b0^:crates/gf2-coding/ROADMAP.md:48 | Tracked: 6ed3c78b |
| C5 normalized/offset min-sum | 791beb2b0^:crates/gf2-coding/ROADMAP.md:49 | Delivered: crates/gf2-coding/src/ldpc/min_sum.rs; c3ea6855 |
| C6 syndrome table optimization (duplicate of C4) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:52 | Obsolete: same item as line 35 |
| C6 Berlekamp-Massey (duplicate of C4) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:53 | Delivered: crates/gf2-coding/src/bch/core.rs |
| C6 Chien search (duplicate of C4) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:54 | Delivered: crates/gf2-coding/src/bch/core.rs |
| C6 SOVA (duplicate of C4) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:55 | Tracked: e15041f8 |
| C7 bit-channel reliability sorting | 791beb2b0^:crates/gf2-coding/ROADMAP.md:61 | Tracked: e748fab4 |
| C7 information/frozen bit selection | 791beb2b0^:crates/gf2-coding/ROADMAP.md:62 | Tracked: e748fab4 |
| C7 rate-compatible polar families | 791beb2b0^:crates/gf2-coding/ROADMAP.md:63 | Tracked: e748fab4 (description names rate-compatible families) |
| C7 fast Hadamard-like transforms | 791beb2b0^:crates/gf2-coding/ROADMAP.md:64 | Delivered: crates/gf2-core/src/bitvec.rs (polar_transform); SIMD variant tracked by b3766524 |
| C7 SC decoder | 791beb2b0^:crates/gf2-coding/ROADMAP.md:67 | Tracked: b94e2821 |
| C7 SCL decoder with path metrics | 791beb2b0^:crates/gf2-coding/ROADMAP.md:68 | Tracked: b94e2821 |
| C7 CRC-aided SCL | 791beb2b0^:crates/gf2-coding/ROADMAP.md:69 | Tracked: b81c239c |
| C7 efficient factor graph representation | 791beb2b0^:crates/gf2-coding/ROADMAP.md:70 | Tracked: b94e2821 (description names factor graph representation) |
| C7 polar FER over AWGN | 791beb2b0^:crates/gf2-coding/ROADMAP.md:73 | Tracked: 21eb03e8 |
| C7 Shannon limit comparison | 791beb2b0^:crates/gf2-coding/ROADMAP.md:74 | Tracked: 21eb03e8 |
| C7 rate vs Eb/N0 characterization | 791beb2b0^:crates/gf2-coding/ROADMAP.md:75 | Tracked: 21eb03e8 |
| C7 capacity-approaching behavior across block lengths | 791beb2b0^:crates/gf2-coding/ROADMAP.md:76 | Tracked: 21eb03e8 |
| C7 comparison with LDPC codes | 791beb2b0^:crates/gf2-coding/ROADMAP.md:77 | Tracked: 21eb03e8 |
| C7 reuse AWGN channel and simulation framework | 791beb2b0^:crates/gf2-coding/ROADMAP.md:80 | Delivered: crates/gf2-coding/src/channel.rs, crates/gf2-coding/src/simulation.rs |
| C7 leverage LLR operations | 791beb2b0^:crates/gf2-coding/ROADMAP.md:81 | Delivered: crates/gf2-coding/src/llr.rs |
| C7 evaluate rank/select integration | 791beb2b0^:crates/gf2-coding/ROADMAP.md:82 | Obsolete: an optional evaluation with no stated consumer; rank/select primitives live in gf2-core and the polar decoders of b94e2821 operate on LLR arrays and the frozen set |
| C8 bit-level transforms (run-length, delta, XOR chaining) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:85 | Tracked: 0405c452 |
| C8 entropy modeling playground | 791beb2b0^:crates/gf2-coding/ROADMAP.md:86 | Tracked: 0405c452 (description names the adaptive frequency coder) |
| C8 raw vs transformed benchmarks | 791beb2b0^:crates/gf2-coding/ROADMAP.md:87 | Tracked: 0405c452 |
| C9 BCH throughput benchmarks | 791beb2b0^:crates/gf2-coding/ROADMAP.md:96 | Delivered: crates/gf2-coding/benches/bch_parallel.rs; baseline receipt 88ca7d2f |
| C10 remaining 11 DVB-T2 table data files | 791beb2b0^:crates/gf2-coding/ROADMAP.md:111 | Delivered: crates/gf2-coding/src/ldpc/dvb_t2/ (twelve table_*.txt files) |
| C10.6.5 full test-vector validation | 791beb2b0^:crates/gf2-coding/ROADMAP.md:126 | Delivered: crates/gf2-coding/tests/dvb_t2_ldpc_verification.rs; 4cdaf1c5 |
| C10.6.7 throughput gap to the 10-50 Mbps target | 791beb2b0^:crates/gf2-coding/ROADMAP.md:144 | Tracked: 1a379447 |
| C10.6.7 CPU profiling of hotspots | 791beb2b0^:crates/gf2-coding/ROADMAP.md:144 | Delivered: crates/gf2-coding/benches/profile_ldpc_encode.rs; 3be770d5 |
| C11.1 Option A: normalized min-sum, early termination | 791beb2b0^:crates/gf2-coding/ROADMAP.md:228 | Delivered: crates/gf2-coding/src/ldpc/min_sum.rs; c3ea6855; further decoder improvements tracked by 0fc3c9d0 |
| C11.1 Option B: GPU prototype | 791beb2b0^:crates/gf2-coding/ROADMAP.md:229 | Delivered: crates/gf2-sim/src/gpu/ldpc_bp.rs, crates/gf2-kernels-hip/hip/ldpc_bp.hip |
| C11.1 Option C: full FEC chain | 791beb2b0^:crates/gf2-coding/ROADMAP.md:230 | Delivered: crates/gf2-coding/src/dvb_t2_bicm_harness.rs; 003e4088 |
| C11.3 M3 throughput vs batch size | 791beb2b0^:crates/gf2-coding/ROADMAP.md:278 | Delivered: 9c37ec8c |
| C11.3 M3 GPU vs 24-core crossover | 791beb2b0^:crates/gf2-coding/ROADMAP.md:279 | Delivered: 9c37ec8c, 86a363aa |
| C11.3 M3 GPU utilization profile | 791beb2b0^:crates/gf2-coding/ROADMAP.md:280 | Delivered: a9284086, 43fb19e2 |
| C11.3 M3 feasibility study document | 791beb2b0^:crates/gf2-coding/ROADMAP.md:281 | Delivered: dev/archive/806eb14e-hip-gpu-prototype/active/806eb14e-feasibility-report.md (24c11004); the roadmap path docs/GPU_FEASIBILITY_STUDY.md does not exist |
| C11.3 go/investigate/abandon criteria | 791beb2b0^:crates/gf2-coding/ROADMAP.md:283-286 | Delivered: decision 86a363aa |
| C11.4 extend HIP across LDPC, BCH, Viterbi | 791beb2b0^:crates/gf2-coding/ROADMAP.md:293 | Delivered for LDPC (a930be7f), BCH syndrome (9012f8a0), BCJR (795c156e); Viterbi tracked by 92acd7b5 |
| C11.4 Structure-of-Arrays memory layout | 791beb2b0^:crates/gf2-coding/ROADMAP.md:294 | Delivered: 43fb19e2 |
| C11.4 CUDA backend | 791beb2b0^:crates/gf2-coding/ROADMAP.md:295 | Obsolete: the roadmap's own banner (lines 251-261) supersedes CUDA and Vulkan with the HIP backend |
| C11.4 automatic backend selection with CPU fallback | 791beb2b0^:crates/gf2-coding/ROADMAP.md:296 | Tracked: 92acd7b5 |
| C11.4 occupancy and bandwidth tuning | 791beb2b0^:crates/gf2-coding/ROADMAP.md:297 | Delivered for 5G NR LDPC: 23d3525f; remainder tracked by 92acd7b5 |
| C11.4 500-1000 Mbps LDPC target | 791beb2b0^:crates/gf2-coding/ROADMAP.md:299 | Tracked: 92acd7b5 |
| C11.5.1 FPGA survey, resource analysis, latency estimate, cost-benefit | 791beb2b0^:crates/gf2-coding/ROADMAP.md:306-309 | Tracked: adc75ba7 |
| C11.5.2 FPGA platform, Viterbi in Verilog/VHDL, PCIe DMA, CPU benchmark | 791beb2b0^:crates/gf2-coding/ROADMAP.md:312-315 | Tracked: adc75ba7 (description covers the prototype phase) |
| C11.5 1-10 Gbps, under 10 us latency target | 791beb2b0^:crates/gf2-coding/ROADMAP.md:317 | Tracked: adc75ba7 |
| Tier 2 live reception, 50-200 Mbps CPU SIMD | 791beb2b0^:crates/gf2-coding/ROADMAP.md:324 | Tracked: 1a379447 |
| Tier 3 professional, 200-1000 Mbps GPU | 791beb2b0^:crates/gf2-coding/ROADMAP.md:325 | Tracked: 92acd7b5 |
| Tier 4 broadcast, 1-10 Gbps FPGA | 791beb2b0^:crates/gf2-coding/ROADMAP.md:326 | Tracked: adc75ba7 |
| RQ1 GPU LDPC memory- or compute-bound, crossover | 791beb2b0^:crates/gf2-coding/ROADMAP.md:330 | Delivered: 43fb19e2, 9c37ec8c |
| RQ2 CSR vs ELLPACK for GPU | 791beb2b0^:crates/gf2-coding/ROADMAP.md:331 | Delivered: 43fb19e2 |
| RQ3 8-bit LLRs on GPU | 791beb2b0^:crates/gf2-coding/ROADMAP.md:332 | Tracked: f63a2464, d69b964e |
| RQ4 FPGA unrolling factor | 791beb2b0^:crates/gf2-coding/ROADMAP.md:333 | Tracked: adc75ba7 |
| RQ5 Berlekamp-Massey on GPU | 791beb2b0^:crates/gf2-coding/ROADMAP.md:334 | Delivered: 9012f8a0, aeab2ee4; decision in 24c11004 |
| C10.7 FER simulation over AWGN | 791beb2b0^:crates/gf2-coding/ROADMAP.md:345 | Delivered: e4849f07 |
| C10.7 live DVB-T2 reception demo with SDR hardware | 791beb2b0^:crates/gf2-coding/ROADMAP.md:354 | Tracked: fcb09e6f |
| C13 unified error handling, Result types | 791beb2b0^:crates/gf2-coding/ROADMAP.md:359 | Tracked: 1b929ce5 |
| C13 streaming vs batch trait unification | 791beb2b0^:crates/gf2-coding/ROADMAP.md:360 | Tracked: 3931ac6f |
| C13 doc examples with syndrome and decoding traces | 791beb2b0^:crates/gf2-coding/ROADMAP.md:361 | Obsolete: elementary decoder-trace examples fall outside the researcher audience; the research tutorial set is carried by cdba4e71 |
| C12.1 C FFI layer (LDPC, BCH, Viterbi, safety wrappers, header, C test) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:369-375 | Tracked: 56467231 |
| C12.2 GNU Radio OOT module (blocks, GRC, flowgraphs, tests, docs) | 791beb2b0^:crates/gf2-coding/ROADMAP.md:378-385 | Tracked: b8baca8a, 21922c59 |
| C12.3 DVB-T2 conformance test vectors | 791beb2b0^:crates/gf2-coding/ROADMAP.md:388 | Delivered: crates/gf2-coding/tests/dvb_t2_chain_tp07a.rs; 4cdaf1c5 |
| C12.3 benchmark vs GNU Radio FEC blocks | 791beb2b0^:crates/gf2-coding/ROADMAP.md:389 | Tracked: dfca71b8 |
| C12.3 RTL-SDR/HackRF captured signals | 791beb2b0^:crates/gf2-coding/ROADMAP.md:390 | Tracked: dfca71b8, fcb09e6f |
| C12.3 BER/FER comparison curves | 791beb2b0^:crates/gf2-coding/ROADMAP.md:391 | Tracked: dfca71b8 |
| C12.4 LuaRadio FFI blocks | 791beb2b0^:crates/gf2-coding/ROADMAP.md:394 | Tracked: dd153981 |
| C12.4 SDRangel plugin | 791beb2b0^:crates/gf2-coding/ROADMAP.md:395 | Tracked: dd153981 |
| C12.4 gr-satellites contributions | 791beb2b0^:crates/gf2-coding/ROADMAP.md:396 | Tracked: dd153981 (description names gr-satellites) |
| C12.4 Python bindings via PyO3 | 791beb2b0^:crates/gf2-coding/ROADMAP.md:397 | Tracked: dd153981 |
| Debt: consolidate expensive LDPC doctests | 791beb2b0^:crates/gf2-coding/ROADMAP.md:408 | Tracked: a0a29512 (each example is removed or retained compiling), 2596b143 (retained examples pass in the CI doctest step) |
| Open question: data structures for extremely sparse H | 791beb2b0^:crates/gf2-coding/ROADMAP.md:411 | Delivered: crates/gf2-coding/src/ldpc/edge_layout.rs; 3a37e0f6, f1a896f0 |
| Open question: switch from table-based to algebraic decoding | 791beb2b0^:crates/gf2-coding/ROADMAP.md:412 | Newly filed: Measure the table-based versus algebraic decoding crossover |
| Open question: GPU offload feasibility for LDPC | 791beb2b0^:crates/gf2-coding/ROADMAP.md:413 | Delivered: decision 86a363aa |
| Open question: compression transforms vs error-correction ordering | 791beb2b0^:crates/gf2-coding/ROADMAP.md:414 | Newly filed: Study compression-transform ordering against error-correction coding |
| Open question: SDR float vs fixed-point LLRs | 791beb2b0^:crates/gf2-coding/ROADMAP.md:415 | Tracked: d69b964e |
