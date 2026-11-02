# AFF3CT gap investigation

Read-only investigation by Codex `gpt-6-astra` at high effort, 2026-11-01, over the FI-3 confirmation in [1358d6ab/outcome.md](../1358d6ab/outcome.md). Effects and sizes are estimates unless a cited artifact records them. No plan has been adopted from it.

**Assessment:** QC-aware intra-frame SIMD is the NR lead, on descriptive results and without a valid confirmation.  
The check update is the only DVB-T2 lead.  
Forced NR lanes recorded wins at six and twelve physical cores against SingleFrame.  
Bandwidth saturation is unproven and does not explain FI-3's SingleFrame losses.  
Evidence and qualifications follow.  
No builds, tests, benchmarks, perf sessions or git writes were run; no files were changed.

## 1. Comparison validity

**Measured:** FI-3 compares equal delivered frame counts: one at `w1-b1`, eight per worker—96 total—at `p12-b8`. All four losing gf2 cells select `SingleFrame`; AFF3CT reports `wave=1 simd=none`. Its `backend=AVX2` label describes the compilation backend, not an INTER/INTRA NMS decoder. Workers occupy CPU 0 or CPUs 0–11, with one dispatcher and no nested pool. Both arms specify f32 flooding NMS, normalization 0.75, cap 50 and syndrome stopping; AFF3CT’s syndrome depth is one. (FI-3 receipt, arms/cells)

**Important scope:** timings repeatedly consume the **first one or eight frames**, identically on every worker; the quality statistics cover 128 frames. Thus those statistics do not establish equal iteration counts within every timed call. ([gf2 selection:486](../../../../dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs) (line 486), `aff3ct-throughput-arm.rs` (line 29))

Parsing, construction and pool startup are outside timing; dispatch, input copying/conversion, decoding and decision extraction are inside. gf2 produces the full codeword before extracting the information window; AFF3CT produces that window directly. Stage probes are separate diagnostics, not deductions from elapsed time. (driver:28 (line 28), [gf2:204](../../../../dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs) (line 204), shim:172 (line 172))

Builds are optimized: gf2 Rust 1.95, native CPU, release/LTO/one codegen unit; AFF3CT GCC 16.2.1, `-O3 -march=native`, with library loop unrolling and no recorded fast-math flag. This is not a portable-versus-native mismatch. (gf2 build:73 (line 73), AFF3CT build:4 (line 4), library flags:20 (line 20))

**Not numerically identical:** pinned sources exist at `.agents/ext/c077a88b/aff3ct`. AFF3CT adds the channel **after** summing messages, uses sign bits, and skips the final-cap syndrome check. gf2 requires channel-first accumulation, comparison-based signs and terminal syndrome evaluation. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 176), `.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx` (line 59), [contract:109](../../../../docs/reference/ldpc-decoder-numerical-contract.md) (line 109))

The receipt's quality records preserve equal DVB mean iterations, while NR mean iterations and bit errors differ between the arms (each cell's `decoder_quality`, over the 128 quality frames). Consequently “matched algorithm” is justified; “identical work/results” is not. These differences can affect gaps, although the recorded aggregate iteration difference does not explain AFF3CT’s advantage. FI-3 receipt

## 2. One-frame gap

**Measured profiles:** integrated gf2 one-frame samples attribute most of each code's time to check-loop work and reduction, then variable updates; the category shares, sample counts and Wilson intervals are in the two linked rows. These are supporting integrated profiles; exact FI-3 profiles provide whole-process counters including setup/warmup, not isolated decoder attribution. (DVB:206 (line 206), NR:432 (line 432), scope:15 (line 15))

AFF3CT uses branchless min/max and XOR-sign reduction, normalizes two minima once per check, streams variable-major messages and checks syndrome directly from posteriors. gf2 tracks an argmin through conditional comparisons, applies normalization per output and indirectly accesses check-major messages during variable updates. **Inference:** check instruction/control dependencies are the strongest common target; layout is secondary. (`.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx` (line 59), `.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hxx` (line 100), [gf2 reduction:111](../../../../crates/gf2-coding/src/ldpc/min_sum.rs) (line 111), [variable update:628](../../../../crates/gf2-coding/src/ldpc/core.rs) (line 628))

AFF3CT actually performs an additional posterior accumulation pass; gf2 combines posterior and outgoing-message computation. Therefore “AFF3CT simply does less arithmetic” is inadequate. Its older profiles attribute a separate share to posterior computation, stated there with its interval. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 261), profile:203 (line 203))

## 3. Multicore gap

**Source-derived payloads**, for $E$ graph edges and $N$ variable nodes, excluding graphs, indices, capacities and harness buffers: AFF3CT owns $8E + 4N$ bytes of messages/posteriors; gf2 SingleFrame approximately $8E + 5N$; lanes add $64E + 65N$. Each `selected_path` of the receipt reports the lane workspace per worker, even when SingleFrame runs. Those inactive allocations are not proof of decode traffic. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 29), [single:537](../../../../crates/gf2-coding/src/ldpc/core.rs) (line 537), [batch:185](../../../../crates/gf2-coding/src/ldpc/batch.rs) (line 185), receipt)

**Contrary evidence:** forced NR lanes already beat SingleFrame at six/twelve cores; both codes lose at 24 logical CPUs. The blanket worker cutoff is conservative, not a universal crossover finding. Lane instruction counts remain stable while IPC collapses; indexed dependent loads appear in assembly. This supports stalls, but does not distinguish bandwidth, cache capacity, latency or SMT contention. (outcomes:102 (line 102), counters:403 (line 403), [assembly:253](../../../../crates/gf2-kernels-simd/src/x86/asm/ldpc_lanes.asm.txt) (line 253))

Lane-state size and wave cooperation remain untested by the existing profiles; `@/issue/55c3e433` holds the compact-lane decision under its REQ-04.

## 4. Already tried

`07ca8585` already removed edge searches, per-edge allocations and repeated leave-one-out reductions; these are not fresh opportunities. (findings:58 (line 58))

The QC timing tables report descriptive latency, multicore and comparator rows, each with its pair count and interval. They are recorded measurements, not valid confirmation or transferable FI-3 speedups. C-09 invalidates the family verdict because its campaign budget was exceeded. (timings:54 (line 54), multicore:127 (line 127), comparator:202 (line 202), C-09:182 (line 182))

The DVB QC prototype is unavailable: its required partition is absent. Quantized and layered adoption is closed under this contract. Their quality-screen failures do **not** prove universal inferiority: layered decoding improved aggregate punctured-cell errors while losing individual previously successful frames; quantized failures depended on code/scale. (findings:54,133 (line 133))

## 5. Tracked experiments

Planned work is in the tracker: `@/issue/55c3e433` owns these gaps; pilot `@/issue/1d787ed3` compares the single-frame route, the QC prototype and AFF3CT on NR with counted iterations; `@/issue/a72f2b3f` compares check-update variants on both codes; `@/issue/4555dfb3` delivers a QC core only on a go decision. Variants outside the [numerical contract](../../../../docs/reference/ldpc-decoder-numerical-contract.md) stay out of production.

## 6. Limits

No evidence establishes an AVX2 ceiling. Hardware bounds are unmeasured (accounting limits).

**Open unknowns:** timed-subset iteration equality, current executable versus FI-3 performance, actual memory bandwidth, and workspace causality.

**Records needing careful correction:** `07ca8585`’s disappearing-reduction/dominant-loop claims are contradicted by recategorization; blanket “multicore lanes lose” contradicts NR results; fixed-wave sweeps do not isolate policy cost; QC’s comparator `fail` means AFF3CT was slower, not QC. (recategorization:50 (line 50), R-04:341 (line 341), decision direction:231 (line 231))
