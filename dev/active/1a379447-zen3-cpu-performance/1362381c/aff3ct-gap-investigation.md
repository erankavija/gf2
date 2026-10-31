# AFF3CT gap investigation

Read-only investigation by Codex `gpt-6-astra` at high effort, 2026-11-01, over the FI-3 confirmation in [1358d6ab/outcome.md](../1358d6ab/outcome.md). Effects and sizes are estimates unless a cited artifact records them. No plan has been adopted from it.

**Assessment:** NR’s one-frame and multicore gaps merit reopening QC-aware intra-frame SIMD; its descriptive results are strong, although adoption is unconfirmed.  
DVB-T2’s best lead is a cheaper, contract-preserving check update; complete closure remains uncertain.  
Multicore lanes deserve selective investigation: NR already won at six and twelve physical cores.  
Bandwidth saturation is plausible, but neither proven nor the explanation for FI-3’s SingleFrame losses.  
Preserve the measured one-worker, eight-frame wins. Evidence and qualifications follow.  
No builds, tests, benchmarks, perf sessions or git writes were run; no files were changed.

## 1. Comparison validity

**Measured:** FI-3 compares equal delivered frame counts: one at `w1-b1`, eight per worker—96 total—at `p12-b8`. All four losing gf2 cells select `SingleFrame`; AFF3CT reports `wave=1 simd=none`. Its `backend=AVX2` label describes the compilation backend, not an INTER/INTRA NMS decoder. Workers occupy CPU 0 or CPUs 0–11, with one dispatcher and no nested pool. Both arms specify f32 flooding NMS, normalization 0.75, cap 50 and syndrome stopping; AFF3CT’s syndrome depth is one. ([FI-3 receipt, arms/cells](../../../../dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/receipt.json))

**Important scope:** timings repeatedly consume the **first one or eight frames**, identically on every worker; the quality statistics cover 128 frames. Thus those statistics do not establish equal iteration counts within every timed call. ([gf2 selection:486](../../../../dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs) (line 486), [AFF3CT selection:29](../../../../dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/harness/src/bin/aff3ct-throughput-arm.rs) (line 29))

Parsing, construction and pool startup are outside timing; dispatch, input copying/conversion, decoding and decision extraction are inside. gf2 produces the full codeword before extracting the information window; AFF3CT produces that window directly. Stage probes are separate diagnostics, not deductions from elapsed time. ([driver:28](../../../../dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/harness/src/driver.rs) (line 28), [gf2:204](../../../../dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs) (line 204), [shim:172](../../../../dev/active/1a379447-zen3-cpu-performance/c077a88b/survey/harness/cpp/aff3ct_shim.cpp) (line 172))

Builds are optimized: gf2 Rust 1.95, native CPU, release/LTO/one codegen unit; AFF3CT GCC 16.2.1, `-O3 -march=native`, with library loop unrolling and no recorded fast-math flag. This is not a portable-versus-native mismatch. ([gf2 build:73](../../../../dev/bench_results/ed3d490e/preparation/build-identity.json) (line 73), [AFF3CT build:4](../../../../dev/bench_results/3be770d5/preparation/build-identity.json) (line 4), [library flags:20](../../../../dev/bench_results/c077a88b/v3-preparation/build-identity.json) (line 20))

**Not numerically identical:** pinned sources exist at `.agents/ext/c077a88b/aff3ct`. AFF3CT adds the channel **after** summing messages, uses sign bits, and skips the final-cap syndrome check. gf2 requires channel-first accumulation, comparison-based signs and terminal syndrome evaluation. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 176), `.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx` (line 59), [contract:109](../../../../docs/reference/ldpc-decoder-numerical-contract.md) (line 109))

The receipt preserves equal DVB mean iterations, but NR means differ: gf2 30.671875 versus AFF3CT 30.7109375; NR bit errors are 677 versus 644. Consequently “matched algorithm” is justified; “identical work/results” is not. These differences can affect gaps, although the recorded aggregate iteration difference does not explain AFF3CT’s advantage. [FI-3 receipt](../../../../dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/receipt.json)

## 2. One-frame gap

**Measured profiles:** integrated gf2 one-frame samples attribute DVB approximately 41% to check-loop work, 33% to reduction and 22% to variable updates; NR approximately 47%, 34%, 18%. Sample counts are only 378/279, with broad intervals. These are supporting integrated profiles; exact FI-3 profiles provide whole-process counters including setup/warmup, not isolated decoder attribution. ([DVB:206](../../../../dev/active/1a379447-zen3-cpu-performance/78f0aafd/decoder-profile/profile.md) (line 206), [NR:432](../../../../dev/active/1a379447-zen3-cpu-performance/78f0aafd/decoder-profile/profile.md) (line 432), [scope:15](../../../../dev/active/1a379447-zen3-cpu-performance/78f0aafd/report.md) (line 15))

AFF3CT uses branchless min/max and XOR-sign reduction, normalizes two minima once per check, streams variable-major messages and checks syndrome directly from posteriors. gf2 tracks an argmin through conditional comparisons, applies normalization per output and indirectly accesses check-major messages during variable updates. **Inference:** check instruction/control dependencies are the strongest common target; layout is secondary. (`.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx` (line 59), `.agents/ext/c077a88b/aff3ct/include/Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hxx` (line 100), [gf2 reduction:111](../../../../crates/gf2-coding/src/ldpc/min_sum.rs) (line 111), [variable update:628](../../../../crates/gf2-coding/src/ldpc/core.rs) (line 628))

AFF3CT actually performs an additional posterior accumulation pass; gf2 combines posterior and outgoing-message computation. Therefore “AFF3CT simply does less arithmetic” is inadequate. Its older profiles separately attribute approximately 14% to posterior computation. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 261), [profile:203](../../../../dev/bench_results/3be770d5/v4-r1-steady-profile/profile.md) (line 203))

## 3. Multicore gap

**Source-derived payloads**, excluding graphs, indices, capacities and harness buffers: AFF3CT owns `8E+4N` bytes of messages/posteriors; gf2 SingleFrame approximately `8E+5N`; lanes add `64E+65N`. The receipt reports lane allocations of 18,727,136 bytes/DVB worker and 9,463,296/NR worker—even when SingleFrame runs. Those inactive allocations are not proof of decode traffic. (`.agents/ext/c077a88b/aff3ct/include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx` (line 29), [single:537](../../../../crates/gf2-coding/src/ldpc/core.rs) (line 537), [batch:185](../../../../crates/gf2-coding/src/ldpc/batch.rs) (line 185), [receipt](../../../../dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/receipt.json))

**Contrary evidence:** forced NR lanes already beat SingleFrame at six/twelve cores; both codes lose at 24 logical CPUs. The blanket worker cutoff is conservative, not a universal crossover finding. Lane instruction counts remain stable while IPC collapses; indexed dependent loads appear in assembly. This supports stalls, but does not distinguish bandwidth, cache capacity, latency or SMT contention. ([outcomes:102](../../../../dev/active/1a379447-zen3-cpu-performance/6230e0b6/findings.md) (line 102), [counters:403](../../../../dev/bench_results/f434ddd3/profile/profile.md) (line 403), [assembly:253](../../../../crates/gf2-kernels-simd/src/x86/asm/ldpc_lanes.asm.txt) (line 253))

**Inference:** smaller lane state could win. Sharing immutable graph/layout data reduces duplication; sharing mutable messages across independent frames cannot preserve independent state. Cooperative processing of fewer waves is another hypothesis, with synchronization costs. Neither is established by the existing profiles.

## 4. Already tried

`07ca8585` already removed edge searches, per-edge allocations and repeated leave-one-out reductions; these are not fresh opportunities. ([findings:58](../../../../dev/active/1a379447-zen3-cpu-performance/07ca8585/findings.md) (line 58))

QC timings descriptively report speedups over their canonical baseline of **8.9059× latency**, **8.6440×/8.2970×/6.0823×** at 6/12/24 workers. Direct comparator latency is **2.923 ms QC versus 15.112 ms AFF3CT**. These are recorded measurements, not valid confirmation or transferable FI-3 speedups. C-09 invalidates the family verdict because its campaign budget was exceeded. ([timings:54](../../../../dev/bench_results/f63a2464/timing-tables.md) (line 54), [multicore:127](../../../../dev/bench_results/f63a2464/timing-tables.md) (line 127), [comparator:202](../../../../dev/bench_results/f63a2464/timing-tables.md) (line 202), [C-09:182](../../../../dev/active/1a379447-zen3-cpu-performance/f63a2464/corrections.md) (line 182))

Reopen NR QC under a new reviewed scope. Keep the existing DVB QC prototype unavailable: its required partition is absent. Keep quantized/layered adoption closed under this contract. Their quality-screen failures do **not** prove universal inferiority: layered decoding improved aggregate punctured-cell errors while losing individual previously successful frames; quantized failures depended on code/scale. ([findings:54,133](../../../../dev/active/1a379447-zen3-cpu-performance/f63a2464/findings.md) (line 133))

## 5. Ranked candidates

All sizes and prospective effects below are **estimates**. Require Rust 1.95, posterior-bit conformance, tested scalar fallback, and unsafe implementation only in `gf2-kernels-simd`.

| Priority / target | Change, evidence and estimated effect | Location, risk and falsification |
|---|---|---|
| **1: NR, both gaps** | QC intra-frame vectors across lifted positions; contiguous rotated variable updates preserve accumulation order. Existing descriptive timings suggest enough headroom, but no defensible current-tree multiplier. | [Prototype:247](../../../../dev/active/1a379447-zen3-cpu-performance/f63a2464/survey/prototypes/src/qc.rs) (line 247); roughly 600–1,200 lines across coding layout/routing and SIMD kernels. Contract-preserving; integration/order risk. Compare current SingleFrame, QC and AFF3CT at b1/w1 and b8/6/12/24. Kill on posterior mismatch or failure to close NR gaps. |
| **2: DVB first; both codes** | Specialize check rules/degrees; reduce branches and repeated normalization while preserving NaN/zero/tie semantics. **Estimate:** halving the sampled check-update cost yields approximately 1.6–1.7× overall, not enough alone for DVB latency. | `min_sum.rs:96`, `core.rs:605`; roughly 150–400 lines. Contract-preserving only after special-value verification. Compare existing, scalar-specialized and vector-across-checks arms in all four losing cells. Kill variants with mismatches or no worthwhile gain. |
| **3: multicore** | Reopen NR routing selectively; prototype reconstructing variable-to-check messages from retained beliefs/incoming messages, eliminating one lane message array. **Estimate:** saves `32E` bytes/worker; speed effect unknown. | `batch.rs:130`, lane kernels; roughly 400–900 lines. High initialization/masking risk; preserve first-iteration semantics. Compare SingleFrame, original lanes and compact lanes at fixed 6/12/24 workers, b8. Kill compactness hypothesis if traffic/stalls and throughput do not improve. |
| **4: residual generic gap** | Variable-major storage using canonical mappings; direct soft syndrome and reuse terminal syndrome result. **Estimate:** halving variable-update cost offers roughly 1.1–1.2×; syndrome savings alone are small. | [layout:49](../../../../crates/gf2-coding/src/ldpc/edge_layout.rs) (line 49), `core.rs:628,656,761`; roughly 250–600 lines. Contract-preserving; risks shifting random accesses into checks. Compare layouts with identical arithmetic at b1 and p12/b8; kill on net regression. |

Quantization, layering, reassociated sums or AFF3CT sign semantics require a separately reviewed contract and new BER/FER evidence—not an optimization waiver. ([contract:20](../../../../docs/reference/ldpc-decoder-numerical-contract.md) (line 20))

## 6. Limits and next decisions

No evidence establishes an AVX2 ceiling. NR looks promising; DVB closure remains uncertain. Leaving DVB’s residual gap alone becomes reasonable if contract-preserving check/layout candidates fail and matched decoder-only profiles establish the remaining cost. Hardware bounds are currently unmeasured. ([accounting limits:25](../../../../dev/active/1a379447-zen3-cpu-performance/78f0aafd/report.md) (line 25))

**Open unknowns:** timed-subset iteration equality, current executable versus FI-3 performance, actual memory bandwidth, and workspace causality.

**First experiments:** (1) current SingleFrame/QC/AFF3CT comparison with counted timed-subset iterations; (2) isolated contract-preserving check variants; (3) fixed-worker original/compact-lane comparison, including six cores and SMT.

**Records needing careful correction:** `07ca8585`’s disappearing-reduction/dominant-loop claims are contradicted by recategorization; blanket “multicore lanes lose” contradicts NR results; fixed-wave sweeps do not isolate policy cost; QC’s comparator `fail` means AFF3CT was slower, not QC. ([recategorization:50](../../../../dev/active/1a379447-zen3-cpu-performance/92c53589/record.md) (line 50), [R-04:341](../../../../dev/active/1a379447-zen3-cpu-performance/6230e0b6/findings.md) (line 341), [decision direction:231](../../../../dev/active/1a379447-zen3-cpu-performance/f63a2464/decision-record.md) (line 231))
