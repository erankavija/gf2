# Investigation: closing the integrated LDPC gaps against AFF3CT

> **Diátaxis Type:** Reference

Read-only planning investigation for `55c3e433`, by Codex `gpt-6-astra` at high effort, 2026-11-01. It extends `dev/active/1a379447-zen3-cpu-performance/1362381c/aff3ct-gap-investigation.md`. Prospective effects and durations are estimates.

The planning lead checked these claims against source: the numerical contract's operation order, `min_sum_check_row`, the QC prototype's cap-zero path, `LdpcCode::from_quasi_cyclic`, the ledger's one-confirmation-per-candidate rule, the tail-support limit and the FI-3 ledger. The compile status of the QC prototype and the adopted graph are recorded in [plan.md](plan.md).

## Claim classification

Citation prefixes used below:

- `P/` = `dev/active/1a379447-zen3-cpu-performance/`
- `S/` = `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/`
- `A/` = `.agents/ext/c077a88b/aff3ct/include/`

These are source findings. Prospective performance effects remain estimates.

| Claim | Classification | Source finding |
|---|---|---|
| AFF3CT channel-after-sum accumulation | **Confirmed** | `_initialize_var_to_chk` accumulates incoming messages from zero, then adds `Y_N[v]`; `_compute_post` uses the same order. `A/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx:219`, `:273`. |
| AFF3CT sign-bit signs | **Confirmed for check updates** | `Update_rule_MS::compute_chk_node_in/out` uses `std::signbit`; its syndrome checker instead compares posteriors with zero. `A/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx:62`, `:89`; `A/Tools/Code/LDPC/Syndrome/LDPC_syndrome.hxx:59`. |
| AFF3CT no final-cap syndrome check | **Confirmed** | `_decode` excludes the last iteration from syndrome testing. Reaching the cap triggers posterior computation, without another syndrome test. `A/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx:189`. |
| AFF3CT branchless min/max | **Unverifiable as a machine-code claim** | The source uses nested `std::min/std::max`, without explicit reduction branches. Whether the measured executable implements these branchlessly requires assembly evidence. `A/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx:65`. |
| AFF3CT XOR-sign reduction | **Confirmed** | Both reduction and exclusion use XOR. `A/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx:64`, `:89`. |
| AFF3CT normalizes two minima once per check | **Confirmed** | `Update_rule_NMS::end_chk_node_in` computes normalized `MS.min2` and `MS.min1`; outputs select the cached constants. `A/Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hxx:100`; `A/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx:88`. |
| AFF3CT variable-major streaming | **Confirmed, specifically for variable/posterior passes** | Those passes advance contiguous message pointers by variable degree. Check updates access those arrays through `transpose`. `A/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx:210`, `:237`, `:266`. |
| AFF3CT direct syndrome from posteriors | **Confirmed** | `LDPC_syndrome::check_soft` reads `Y_N[H[c][v]]` and accumulates parity without a hard-bit array. `A/Tools/Code/LDPC/Syndrome/LDPC_syndrome.hxx:45`. |
| gf2 argmin through conditional comparisons | **Confirmed** | `min_sum_check_row` maintains `min1`, `min2`, and `arg1` using strict `<` comparisons. This establishes source structure, not emitted branch counts. `crates/gf2-coding/src/ldpc/min_sum.rs:111`. |
| gf2 per-output normalization | **Confirmed** | Each output calls `MinSumRule::apply`; normalized output is `alpha * (sign * magnitude)`. `crates/gf2-coding/src/ldpc/min_sum.rs:26`, `:136`. |
| gf2 indirect check-major access during variable update | **Confirmed** | `variable_node_update` traverses `var_edge_to_check_edge`, then indexes both message arrays. `crates/gf2-coding/src/ldpc/core.rs:628`. |
| AFF3CT extra posterior pass | **Confirmed** | `_compute_post` is separate from `_initialize_var_to_chk`. gf2 computes beliefs and outgoing variable messages together. This does not by itself establish a performance disadvantage. `A/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx:185`, `:191`, `:261`; `crates/gf2-coding/src/ldpc/core.rs:633`. |
| Payloads `8E+4N`, `8E+5N`, additional lanes `64E+65N` | **Confirmed as array-payload accounting** | For one f32 AFF3CT frame: two edge arrays and one posterior array. gf2 adds an N-byte `Vec<bool>` hard-decision buffer. Lane storage adds two `F32x8` edge arrays, two `F32x8` variable arrays, and one byte per variable. These exclude graph/index storage, scratch capacity, packed codewords, allocation overhead, and harness buffers. The batch decoder also owns its single-frame decoder. `A/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx:29`; `crates/gf2-coding/src/ldpc/core.rs:540`; `crates/gf2-coding/src/ldpc/batch.rs:185`; `crates/gf2-kernels-simd/src/ldpc_lanes.rs:17`. |

The requested 30-entry LDPC history contains relevant routing/interface drift: `7988b2ac5` supplies configured worker counts, `b9559f557` removes inherent batch methods, `b6d87f96e` enables dispatched AVX2 lanes, and `4d8808734` fixes zero-cap results. Current source retains the scalar check/variable arithmetic above. Harnesses must use `BatchSoftDecoder`, pass actual concurrency, and distinguish forced SingleFrame from production routing. (`S/arms/src/lib.rs:16`, `:220`, `:498`; `crates/gf2-coding/src/ldpc/route.rs:30`, `:80`; `crates/gf2-coding/src/ldpc/core.rs:740`.)

## Harness reuse

| Component | Reusable behavior | Missing work |
|---|---|---|
| gf2 arm | `gf2-batch-single-frame-arm` forces `BatchRoute::SingleFrame` through conservative selectors while retaining the production decoder implementation. `gf2-batch-tuned-arm` installs the committed envelope and observes production routes. | Iteration diagnostics; current executable/build identities. Use forced SingleFrame for REQ-01 and the tuned arm for lead preservation and closeout. `S/arms/src/lib.rs:63`, `:178`; `S/arms/tuned-profile/src/lib.rs:46`. |
| AFF3CT arm and shim | `Aff3ctWorker` owns an uncounted clone; `Handle::build` creates a counting prototype. Existing settings select scalar flooding f32 NMS, syndrome depth one. | Post-timing counted replay and diagnostics; no new C++ counter is needed. `P/3be770d5/survey/harness/src/aff3ct.rs:140`, `:366`, `:433`, `:497`; `P/c077a88b/survey/harness/cpp/aff3ct_shim.cpp:113`. |
| QC arm | `qc-decoder-arm` already accepts per-worker batch size and uses `run_cell`. Each worker reuses one `QcDecoder`, converting and decoding frames sequentially. | Register it beside the other arms, expose its existing iteration vector, repair zero-cap behavior, and verify current-tree conformance. `P/f63a2464/survey/arms/src/bin/qc-decoder-arm.rs:35`, `:66`, `:148`. |
| Shared driver | `run_cell` provides pinned workers, setup exclusion, timing windows, empty-dispatch probes, and post-timing placement/quality checks. | A small post-timing observation hook, rather than another driver. `P/3be770d5/survey/harness/src/driver.rs:46`, `:86`, `:105`. |
| FI-3 campaign driver | `make-family.py` projects declarations into plans; `run-family-campaign.py` runs bounded sessions, resumes, finalizes, verifies logs, and evaluates acceptance. | A newly scoped declaration, arm registrations/build records, campaign identities, and explicit search budget. `S/make-family.py:261`, `:335`; `S/run-family-campaign.py:75`. |

**Source compatibility.** The QC source’s `QuasiCyclicLdpc`, `MinSumRule`, and `Llr` calls match current declarations. It has **no `gf2-kernels-simd` dependency or kernel API calls**: its AVX2 implementation resides inside the standalone prototype. Compilation and linking are unverified in this read-only investigation. (`P/f63a2464/survey/prototypes/Cargo.toml:20`; `P/f63a2464/survey/prototypes/src/qc.rs:20`, `:53`, `:395`; `crates/gf2-coding/src/ldpc/core.rs:319`; `crates/gf2-coding/src/ldpc/nr_5g/mod.rs:198`.)

API compatibility does not establish behavioral compatibility. At cap zero, QC copies channel inputs but never initializes `posterior` before testing its syndrome; a reused decoder therefore exposes stale posteriors. The current canonical decoder explicitly copies channel LLRs at cap zero. Also, the QC arm’s corpus check compares dimensions and edge counts, not the full parity-check relation; reuse needs an exact graph comparison outside timing. (`P/f63a2464/survey/prototypes/src/qc.rs:217`, `:231`; `crates/gf2-coding/src/ldpc/core.rs:740`; `P/f63a2464/survey/arms/src/bin/qc-decoder-arm.rs:138`.)

**Iteration evidence:**

- **gf2:** every frame’s `DecodeOutcome` already occupies `BatchWorker.outcomes`; recording it after timing adds no instrumentation inside the decoder. The field is currently private and the driver discards finished states after verification. (`S/arms/src/lib.rs:97`, `:220`; `P/3be770d5/survey/harness/src/driver.rs:105`.)
- **QC:** `QcOutcome.iterations` and `last_iterations()` are exposed. The arm already copies each outcome into `QcWorker.iterations` inside its timed loop, but never emits the vector. Post-timing serialization adds no further timed work; the existing store remains part of this arm’s measured operation. (`P/f63a2464/survey/prototypes/src/qc.rs:140`, `:190`; `P/f63a2464/survey/arms/src/bin/qc-decoder-arm.rs:76`.)
- **AFF3CT:** timed clones have no counter. `Handle::decode_counted` works on the original counting prototype and returns one count per native wave—one frame for this comparator. Replay the exact timed prefix after timing, label counts as replay-derived, and compare replay decisions with timed-worker decisions. Counting inside timed calls changes their work; post-timing replay avoids that. Existing tests compare counted-prototype and clone decisions. (`P/3be770d5/survey/harness/cpp/throughput_shim.cpp:25`; `P/3be770d5/survey/harness/src/aff3ct.rs:427`, `:570`; `P/3be770d5/survey/harness/src/bin/aff3ct-throughput-arm.rs:32`.)

All workers repeatedly consume the same first b1/b8 frames. Thus a worker/frame-indexed count vector plus the repeated-input mapping describes the timed workload; it is not a trace independently observed on every repeated invocation. That distinction must be fixed in the plan. The prepared quality summary is insufficient: worker verification checks per-frame error counts, not iterations or posterior equality. (`S/arms/src/lib.rs:485`; `P/f63a2464/survey/arms/src/bin/qc-decoder-arm.rs:115`; `P/3be770d5/survey/harness/src/workload.rs:200`.)

**Batching and CPU sets.** QC serves b8 with eight sequential single-frame decodes per worker; its eight SIMD lanes represent lifted positions within a frame. Use `physical-cores-6`, `physical-cores-12`, and `logical-cpus-24`, resolved at runtime. Six prefers six physical representatives sharing L3; twelve selects physical representatives; twenty-four uses the available logical-CPU mask. Under the full-affinity topology recorded by FI-3, these resolve to `0–5`, `0–11`, and `0–23`; w1 resolves to CPU 0. They are not the wrapper’s default CCX1 set `6–11`, so the campaign requires `--full-host`. (`P/f63a2464/survey/prototypes/src/qc.rs:3`; `dev/tools/tuning-campaign-support/src/host.rs:334`; `dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-pilot/execution.log:1`; `dev/scripts/ccx1-bench-flock.sh:8`.)

**Smallest harness change:** retain the two-arm campaign model, comparing SingleFrame–QC and SingleFrame–AFF3CT at each of five NR configurations. Add post-timing iteration diagnostics to the existing arm lifecycle, using the runner’s existing stderr journal. Register QC and the check variants in the existing catalogue/build machinery; reuse bundles and quality records only after current conformance succeeds. No new timing framework, result schema, or C++ counting mechanism is required. (`S/make-family.py:269`; `S/arms.json:17`, `:93`; `P/3be770d5/survey/harness/src/workload.rs:143`; `S/record-peak-memory.py:166`.)

## Check-update path

The production single-frame path is:

`LdpcBatchDecoder::decode_wave_per_frame` → `LdpcDecoder::decode_codeword_into` → `check_node_update` → `check_node_update_min_sum` → `min_sum_check_row`.

Both losing-code routes reach this same implementation. The FI-3 cases are AList-based LDPC decoder workloads, not complete DVB concatenated or NR rate-matching pipelines. (`crates/gf2-coding/src/ldpc/batch.rs:362`; `crates/gf2-coding/src/ldpc/core.rs:749`, `:616`, `:605`; `S/arms/src/lib.rs:488`; `S/families/1362381c-ldpc-aff3ct-integration.json:72`.)

The smallest isolated experiment changes only `min_sum_check_row`, building separately pinned arm executables while retaining the same decoder, variable update, stopping logic, and harness. The function is directly imported by `core.rs`; there is no existing runtime check-update strategy selector. A permanent strategy framework is unnecessary for a bounded comparison. (`crates/gf2-coding/src/ldpc/core.rs:605`; `crates/gf2-coding/src/ldpc/min_sum.rs:96`.)

The binding numerical clauses are:

- Comparison signs: both zeros positive, NaN negative.
- Strict minima comparisons, first-position tie behavior, skipped NaN magnitudes, and infinity when all excluded magnitudes are NaN.
- Normalized output evaluated as `alpha * (sign * magnitude)` in that order.
- Degree-zero/one behavior, channel-first unreassociated variable sums, and terminal syndrome semantics.

(`docs/reference/ldpc-decoder-numerical-contract.md:48`, `:58`, `:72`, `:84`, `:111`, `:135`.)

**Normalizing unsigned minima before signing does not satisfy the written operation-order clause**, even if bit equivalence is argued for the allowed parameter domain. REQ-05 supplies no waiver. A contract-preserving alternative is to cache the four signed results `alpha * (+1 * min1/min2)` and `alpha * (-1 * min1/min2)`, then select an already computed result. Its speed effect is an **estimate requiring measurement**, especially for low-degree checks. (`docs/reference/ldpc-decoder-numerical-contract.md:77`; `crates/gf2-coding/src/ldpc/min_sum.rs:29`.)

Branch reduction can remain within the contract through comparison/select operations. A literal transplant of AFF3CT’s min/max fold is insufficient: NaN handling, infinity initialization, strict ties, and comparison-derived signs must survive. In particular, naïvely nesting Rust `min/max` around NaN can introduce a spurious second minimum. Emitted branchlessness remains an assembly question. (`crates/gf2-coding/src/ldpc/min_sum.rs:113`; `A/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx:53`, `:65`; `docs/reference/ldpc-decoder-numerical-contract.md:63`.)

Existing coverage has distinct roles:

- `ldpc_check_update_contract.rs` checks row output bits against a scalar reference, including ties, signed zero, non-finites, and low degrees. Its whole-decoder comparisons check codewords, iterations, and flags—not posterior bits. (`crates/gf2-coding/tests/ldpc_check_update_contract.rs:56`, `:88`, `:112`, `:413`.)
- `ldpc_batch_contract.rs` compares posterior bits against `LdpcDecoder`, including special-value frames. Its reference must remain the unchanged canonical implementation while a candidate is evaluated; comparing two routes that both contain the replacement is insufficient. (`crates/gf2-coding/tests/ldpc_batch_contract.rs:112`, `:140`, `:347`, `:393`.)
- The QC prototype already has canonical-posterior and scalar/AVX2 special-value comparisons, but these do not cover the zero-cap defect. (`P/f63a2464/survey/prototypes/tests/contracts.rs:87`, `:164`.)

The variants can use the same pilot driver as REQ-01. Two predeclared variants require eight paired cells: each versus unchanged production at DVB and NR w1-b1/p12-b8. No DVB QC arm is implied. (`P/3be770d5/survey/harness/src/driver.rs:46`; `S/make-family.py:269`; `P/f63a2464/corrections.md:151`.)

## Go path

Productization needs more than moving the prototype kernel:

1. **Reusable QC structure and decoder integration.** `LdpcCode::from_quasi_cyclic` expands to edges and discards the QC description. Production routing therefore needs a canonical way to retain or supply applicable QC structure; the AList-based pilot cannot obtain it from `LdpcCode` today. Avoid an NR-only private decoder inside the harness or consumer. (`crates/gf2-coding/src/ldpc/core.rs:14`, `:83`; `P/f63a2464/survey/prototypes/src/qc.rs:36`.)
2. **Canonical selection.** Extend `DecodeSelectors`, `CodingTuning`, and `CodingTuningCodec` in `crates/gf2-coding/src/tuning.rs`, and the route decision represented by `BatchRoute`/`decide_batch_route` in `ldpc/route.rs`. Current selection expresses frame lanes versus SingleFrame, not QC applicability. Preserve configured-worker semantics and the existing w1-b8 lane choice unless evidence supports changing it. (`crates/gf2-coding/src/tuning.rs:112`, `:246`, `:351`; `crates/gf2-coding/src/ldpc/route.rs:53`, `:80`.)
3. **Kernel boundary and fallback.** Move production unsafe code into `gf2-kernels-simd`, with safe capability dispatch and scalar fallback. `LdpcLaneFns::{portable, …}` and `detect()` provide the existing boundary pattern, but their frame-lane layout is not automatically a QC layout. (`P/f63a2464/survey/prototypes/src/qc.rs:258`, `:395`; `crates/gf2-kernels-simd/src/ldpc_lanes.rs:128`, `:147`; `AGENTS.md:60`.)
4. **Shared conformance.** Run `traits::conformance::batch_soft_decoder_contract`, posterior-bit comparisons, zero-cap/reuse cases, special values, both stopping modes, and forced fallback. The generic batch suite checks words/outcomes; posterior equality requires the LDPC-specific coverage too. (`crates/gf2-coding/src/traits.rs:1947`; `crates/gf2-coding/tests/ldpc_batch_contract.rs:140`; `docs/reference/ldpc-decoder-numerical-contract.md:223`.)
5. **Frozen confirmation.** Pin a current pre-change baseline and final candidate, with improvement and non-regression criteria over the declared dispatch domain. Calibrated selectors/final integration require independent holdout evidence. (`P/measurement-contract.md:123`; `P/f547c394/protocol.md:87`.)

Reuse `c7113c5a/survey/freeze-confirmation.py`, the family generator/runner, `benchmark-acceptance --next-attempt`, and the existing append-only ledger. The freezer derives resolution from the accepted pilot and checks margins; it also supports declared holdouts. Confirmation uses 24 fresh pairs, one attempt per candidate identity, and 10,000 bootstrap draws. At the first attempt, the twenty-draw tail requirement permits at most six confirmatory comparisons under the default alpha allocation. Thus the full pilot grid cannot simply become one confirmation. (`P/c7113c5a/survey/freeze-confirmation.py:13`, `:30`, `:43`; `P/1358d6ab/ldpc-aff3ct/window.sh:37`; `P/f547c394/protocol.md:229`, `:287`, `:303`.)

**Estimated window cost:** approximately 10–15 minutes for a comparable six-cell confirmation with prepared binaries; add roughly 5–10 minutes if a fresh pilot is needed. Selector holdouts or additional before/after comparisons add cells. The source basis is FI-3’s approximately eight-minute confirmation and four-minute pilot, not the nominal 24 seconds per confirmation cell. (`dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-confirmation/execution.log:1`, `:1221`; corresponding `pilot/execution.log:1`, `:645`.)

## Closeout

The smallest closeout is a six-row projection: four losing cells plus both w1-b8 cells. Each losing row cites a final confirmed comparison, a measured limit, or a preserved negative/inconclusive experiment. A rejected candidate does not require another campaign merely to manufacture a successful result; a changed production route does require fresh evidence. (`.jit/issues/55c3e433-843f-4079-adc5-4530ff3e8946.json:8`; `P/measurement-contract.md:48`, `:114`.)

For changed production, reuse FI-3’s six-cell workload declaration and tuned arm, with the same matched AFF3CT comparator. Its ratio is **gf2 time / AFF3CT time**: `pass` means AFF3CT establishes a material advantage, while `fail` can mean gf2 is faster. Preserve those evaluator outcomes and explain direction rather than relabeling them. (`S/families/1362381c-ldpc-aff3ct-integration.json:95`; `P/1358d6ab/outcome.md:7`, `:20`.)

The existing family **does not provide a clean fresh rerun merely because the tree changes**:

- Its campaign IDs and output directories are fixed; the launcher resumes them and skips completed stages.
- Its window script reuses existing frozen confirmation files.
- Executables must match supplied build identities.
- Its ledger already reserves the confirmation. The same AFF3CT candidate cannot receive another confirmation reservation under the same protocol simply because the gf2 baseline changes.
- A family rename cannot erase trial history.

(`S/run-family-campaign.py:75`, `:113`; `P/1358d6ab/ldpc-aff3ct/window.sh:40`; `dev/bench_results/1358d6ab/ldpc-aff3ct-integration/ledger.jsonl:2`; `P/f547c394/protocol.md:135`, `:162`.)

Plan a genuinely scoped route-adoption/current-integration question with explicit candidate identities, history, and budget before measurement. If the intended question remains the existing family’s question, resolve its spent budget rather than silently starting another ledger. Source-only or documentation drift does not itself invalidate an unchanged executable’s receipt. (`P/f547c394/protocol.md:162`; `P/measurement-contract.md:13`.)

Existing reporting is sufficient: `dev/scripts/campaign_tables.py` projects committed summaries and paths; `1358d6ab/make-outcome.py` already formats intervals, margins, decisions, interpretations, and FER. Its campaign inventory is fixed, so reuse its shared projection functions or parameterize the inventory instead of copying outcome prose. (`dev/scripts/campaign_tables.py:2`, `:30`; `P/1358d6ab/make-outcome.py:27`, `:93`.)

Reuse traps remain binding: no admitted “fastest quality-compatible” AFF3CT arm, no DVB QC arm, no established inter-frame latency conclusion from the QC study, and no valid QC confirmation verdict under the exceeded family budget. Separate ledgers do not multiply an allowed campaign count. (`P/f63a2464/corrections.md:19`, `:151`, `:166`, `:182`.)

## Window fit

`run-window.sh` exports `GF2_BENCH_WINDOW=1`, executes TSV jobs sequentially from their worktree roots, and skips successful job keys. Estimated minutes are scheduling information, not an enforced timeout. Campaign checkpoints and execution logs supply resumability; a queue job’s exit code alone does not establish valid measurements. (`P/bench-window/run-window.sh:13`, `:36`, `:69`; `P/bench-window/follow-window.sh:9`.)

The plan should reference this queue-line form, with literal tabs:

```text
<issue>	<repo-relative-worktree>	40	bash dev/active/55c3e433-aff3ct-ldpc-gaps/<worker>/window.sh
```

The launcher delegates bounded sessions to the existing runner, using `ccx1-bench-flock.sh --full-host`; cargo work under that lock uses `CARGO_CI_NO_LOCK=1`. Builds finish before timed work. (`S/run-family-campaign.py:67`, `:81`; `dev/scripts/ccx1-bench-flock.sh:64`; `P/measurement-contract.md:57`.)

**Estimate for REQ-01 + REQ-02:** with twelve pilot pairs and two check variants, REQ-01 contributes ten paired cells and REQ-02 eight. Their nominal timed-window target totals 216 seconds, excluding calibration, setup, probes, replay, and evaluation. FI-3’s six-cell twelve-pair pilot takes approximately four minutes; its p12 DVB cell alone takes approximately 86 seconds. Allow **15–30 minutes**, and queue **40 estimated minutes** with prepared binaries. SMT behavior and current candidate timing are unverified; this is an allowance, not a promised duration. (`P/f547c394/protocol.md:289`; `S/families/1362381c-ldpc-aff3ct-integration.json:36`; `dev/bench_results/1358d6ab/ldpc-aff3ct-integration/1358d6ab-ldpc-aff3ct-integration-pilot/execution.log:219`, `:317`, `:645`.)

## Issues with the criteria

- **REQ-01 has an arm ambiguity at w1-b8.** Forced production SingleFrame is not the tuned production route that owns the existing lead. The pilot needs the former; closeout needs the latter. Otherwise it cannot establish lead preservation. (`S/arms/src/lib.rs:63`; `P/1358d6ab/outcome.md:20`, `:23`.)
- **REQ-01’s “every timed frame” needs an evidence interpretation.** The existing driver repeats a fixed prefix; AFF3CT timed clones expose no iteration counter. Post-timing replay satisfies workload-level accounting only when explicitly identified as replay, not as per-invocation telemetry. (`P/3be770d5/survey/harness/src/driver.rs:86`; `P/3be770d5/survey/harness/src/aff3ct.rs:427`.)
- **REQ-02’s normalization lead conflicts with a literal two-unsigned-minima implementation under REQ-05.** The criterion remains satisfiable through contract-preserving cached signed results or other reductions in repeated work; AFF3CT’s operation order cannot simply be copied. (`docs/reference/ldpc-decoder-numerical-contract.md:77`.)
- **REQ-03 is a conditional delivery obligation.** Creating a follow-up alone does not satisfy a go decision. Closeout must remain blocked until the route and confirmation exist. (`.jit/issues/55c3e433-843f-4079-adc5-4530ff3e8946.json:8`; `P/measurement-contract.md:52`.)
- **The frozen protocol cannot confirm the whole pilot grid as one default family.** Ten NR comparisons already exceed the first-attempt tail-support limit; adding check variants worsens it. Confirmation scope must be selected and frozen explicitly. (`P/f547c394/protocol.md:229`, `:303`; `P/c7113c5a/survey/freeze-confirmation.py:43`.)
- **REQ-04 does not require compact-lane implementation.** A reasoned hold is sufficient; unconditional implementation issues expand scope unnecessarily. REQ-05 is a cross-cutting contract, and REQ-07 is an aspiration assessed by closeout, so neither needs a separate issue. (`.jit/issues/55c3e433-843f-4079-adc5-4530ff3e8946.json:8`.)
- **MSRV feasibility remains a breakdown constraint.** Existing Rust 1.95 probe evidence supports only its recorded probes, not arbitrary new intrinsic variants or current-tree compilation. This read-only investigation cannot supply new compile evidence. (`dev/bench_results/f63a2464/intrinsic-feasibility/feasibility.json:3`; `P/measurement-contract.md:108`.)

No hard criterion is inherently unsatisfiable after these interpretations are fixed. The two concrete conflicts are unsigned-minima normalization versus the written numerical contract, and treating the spent FI-3/QC campaigns as fresh reusable confirmation budgets.