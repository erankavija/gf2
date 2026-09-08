# YMM carry-less batch dispatch on Zen 3

## Finding and adoption decision

**The premise was falsified on this Ryzen 9 5900X.** Enabling the YMM lane
makes every declared raw-batch and FieldVec consumer cell slower. The
[confirmatory receipt](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/receipt.json)
is accepted with zero protocol findings and **does not qualify for adoption**.
All cells are `regressed`; no negative, unavailable or inconclusive result is
omitted. The [acceptance summary](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/acceptance-summary.md)
is the authoritative projection of the measurements.

The default bundle retains sequential PCLMULQDQ. `gf2m::detect_x86` selects the
function pointer and its lane tag together at detection time.
`detect_with_clmul_batch_preference(ClmulBatchLane::Ymm)` can select YMM on
AVX2+VPCLMULQDQ hosts without AVX512VL; an unsupported preference falls back to
sequential PCLMULQDQ. Without PCLMULQDQ and SSE4.1 detection returns `None`,
allowing the caller's portable scalar fallback. There is no adoption branch or
per-call feature check inside either multiplication kernel.

The repaired capability predicate and target-feature contract are retained
because the **dispatch/safety contract was wrong independently of which lane
wins**. No result here establishes an optimum or a performance rule for other
microarchitectures. Sequential is the conservative default; YMM remains an
explicit, capability-checked preference.

## Protocol-v3 continuation

The protocol-v3 continuation preserves the negative result and does not create
a new confirmatory performance claim. Its first four-cell exploratory
[pilot](../../bench_results/1d0da41f/v3-pilot/acceptance-summary.md) is accepted
with zero findings, but a noisy six-pair odd-tail interval gives a conservative
resolution of 0.1246014148. That resolution cannot support either frozen
margin. A separately frozen, one-cell, 24-pair
[resolution pilot](../../bench_results/1d0da41f/v3-pilot-r2/acceptance-summary.md)
is accepted with zero findings. Its odd-tail estimate is 0.6054968089 with
interval [0.6040122157, 0.6078261575], giving resolution 0.00388547393; the
original margins remain admissible because 1.00388547393 is below both 1.05
and 1.10.

The resulting five-cell [v3 receipt](../../bench_results/1d0da41f/v3-confirmation/receipt.json)
is structurally accepted, preserves all samples, and again estimates every YMM
cell as slower. Its [acceptance summary](../../bench_results/1d0da41f/v3-confirmation/acceptance-summary.md)
classifies all five cells as `not-confirmatory` under P-20. The retrospective
v1 reservation and the fresh v3 reservation produce ten family comparisons;
the second-attempt family alpha is 0.0083333333 and the per-comparison alpha is
0.0008333333. Ten thousand bootstrap draws therefore provide only about 4.17
expected draws in each tail, below the protocol's fixed minimum of twenty. This limit
is independent of the observed interval widths. The one permitted v3 candidate
attempt is spent, so no timing retry or post-hoc method change is made.

The accepted v1 confirmation remains the governing negative evidence. The v3
continuation independently agrees in direction but is retained as
non-confirmatory evidence of a protocol-budget incompatibility. Neither body of
evidence supports YMM default adoption, and the sequential default remains in
place.

## Question and method

Does correcting the AVX512VL requirement improve raw independent 64-by-64
carry-less products and `FieldVec::<Gf2mElement>::simd_dot_product`? Both arms
compute the complete 128-bit product with canonical little-endian polynomial
bit indexing. The FieldVec consumer packs field elements, dispatches raw
batches, XOR-accumulates the products and reduces the accumulated value.
All of those consumer operations are inside the timed call. Construction of
fixture inputs is outside the timed call and reported as setup telemetry.

The [shared measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md),
[receipt-pinned protocol v1](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/inputs/protocol.md) and
[frozen family](addendum-ymm-clmul-dispatch.json) govern this result. The family
bytes remain identical to commit `9e6ebad6`, SHA-256
`23732531a6e468b570a5af518eb349771e0e51c33a606caaaf093615d5973a36`.
Its resolution evidence is the distinct committed exploratory pilot receipt,
SHA-256 `da17c31c357951f4082e2b9c57eb581672309e0ef01ef2ac8a6ab0e251f0fbc0`.
The runner captures that pilot at `inputs/resolution-evidence/receipt.json`
before the opening campaign record. No addendum amendment, timing override,
post-hoc threshold, repeated confirmatory trial or sample exclusion is used.

The [executed commands](evidence/reproduction.md) record the build, lock, resume
and finalization sequence. The bounded launcher is [run-clmul-ab.sh](../../bench_results/1d0da41f/run-clmul-ab.sh).
The completed campaign uses three sessions with at most two cells each under
`dev/scripts/ccx1-bench-flock.sh --full-host`, setting `CARGO_CI_NO_LOCK=1`.
Each session releases the exclusive mutex. The shared journal and checkpoint
store resume completed cells without repetition; the log is announced before
measurement. The wrapper's attempted priority increase is denied by the host;
priority adjustment is best-effort and the exclusive lock is observed.

The canonical runner accepts an optional family producing-input manifest.
[producing-inputs.json](producing-inputs.json) extends the shared tooling
closure with the arm adapter, candidate libraries, baseline source archive,
Cargo manifests/locks, configuration and launcher. It uses the existing
`ProducingInputs::capture_to` mechanism; no private provenance or campaign
framework is introduced. The receipt-local snapshots, opening facts and saved
plan bind exact content before the first sample. Git locators are informational.
Acceptance revalidates successfully after the production selection changes,
using those immutable inputs rather than the current checkout's contents.

## Pinned arms and host

Both gf2 libraries and the adapter are version 0.1.0. The two arm builds use
Rust 1.95.0 (`59807616e`, 2026-04-14), release optimization, thin LTO, one
codegen unit, no native ISA flags, and the adapter's locked dependency graph.
Library license metadata is pinned by the root and crate Cargo manifests.
The baseline source archive is
[survey/baseline-source.tar.gz](survey/baseline-source.tar.gz); its content digest
is in the receipt's producing-input manifest. The candidate source is saved in
`inputs/producing/crates/` inside the receipt. Relative to that baseline, the
production arithmetic changes are confined to the raw-batch dispatch/safety
repair; the FieldVec changes are tests.

| Arm | Selected path in every execution | Executable SHA-256 |
|---|---|---|
| Baseline: predicate requiring AVX512VL | `pclmulqdq-scalar-xmm` | `91373c57a51ab10a9b48b8dc3b9d3398a3cbb2498f9dadf8f650d6ec788c9203` |
| Candidate: repaired predicate | `avx2+vpclmulqdq-ymm` | `c89f97fd654158fb364625d43d7e375d49ef95b37f39270425e14eb544bc3116` |

Runtime observations in the receipt identify `AMD Ryzen 9 5900X 12-Core Processor`,
SMT enabled and the `powersave` governor. Latency cells run on CPU 0. The
streaming cell resolves CPUs 0–5 but **declares and observes one worker**; it
is a single-worker sustained measurement on a six-core affinity mask, not a
six-worker scaling result. Twelve-core and 24-logical-CPU scaling are outside
this single-worker family, not silently measured or inferred. No external
raw-product arm is declared, and no gf2x build is attempted; a long-polynomial
multiply is not this independent-product operation.

## Confirmatory measurements

Each row contains 24 fresh paired executions, with five 100-ms timing windows
per arm execution (240 retained windows per cell). The point estimate is the
ratio of baseline and candidate medians. Paired bootstrap intervals use 10000
resamples and 99% confidence per comparison, controlling the five-comparison
family at 95%. Every cell has zero flagged windows. The reciprocal columns are
algebraic transformations of the same estimates and intervals, not new samples.

| Cell | Pairs | Baseline/candidate time ratio | Confidence interval | Candidate/baseline time ratio | Confidence interval |
|---|---:|---:|---|---:|---|
| `raw-batch-small-8-1core` | 24 | 0.7934 | [0.7792, 0.8117] | 1.2604 | [1.2320, 1.2834] |
| `raw-batch-odd-tail-65-1core` | 24 | 0.6052 | [0.6041, 0.6064] | 1.6524 | [1.6492, 1.6553] |
| `raw-batch-l1-512-1core` | 24 | 0.5854 | [0.5833, 0.5872] | 1.7083 | [1.7029, 1.7144] |
| `fieldvec-dot-1024-1core` | 24 | 0.7732 | [0.7683, 0.7780] | 1.2933 | [1.2854, 1.3017] |
| `raw-batch-l1-512-streaming-6core` | 24 | 0.5856 | [0.5827, 0.5872] | 1.7076 | [1.7029, 1.7162] |

All confidence intervals place YMM outside the frozen 1.05 non-regression
margin, and below the raw-cell 1.10 worthwhile-speedup threshold. The consumer
regresses as well as the isolated kernel, so the frozen rule retains the
established sequential implementation. This is a measured rejection of YMM
adoption, not an inconclusive absence of improvement. The final detect-time
plumbing is correctness-tested; its exact overhead is not separately timed and
no additional speedup is claimed for it.

## Preserved exploratory falsification

The [pilot summary](../../bench_results/1d0da41f/2026-09-07-1d0da41f-clmul-dispatch-pilot/acceptance-summary.md)
contains six pairs per cell, 95% percentile-bootstrap intervals and zero flagged
windows. It is exploratory and contributes no samples to confirmation.
The pilot records Rust 1.97.0, whereas confirmation uses Rust 1.95.0. Its
historical producer snapshot covers the protocol tooling; the independent
confirmation supplies the complete arm/library source closure. The pilot arm
binaries are not independently reconstructed here. Its direction agrees with
confirmation, and its samples are never pooled with confirmation.

| Cell | Pairs | Baseline/candidate time ratio | Confidence interval | Candidate/baseline time ratio | Confidence interval |
|---|---:|---:|---|---:|---|
| `raw-batch-small-8-1core` | 6 | 0.8054 | [0.7947, 0.8235] | 1.2417 | [1.2144, 1.2584] |
| `raw-batch-odd-tail-65-1core` | 6 | 0.6082 | [0.6067, 0.6152] | 1.6441 | [1.6254, 1.6483] |
| `raw-batch-l1-512-1core` | 6 | 0.5812 | [0.5801, 0.5815] | 1.7206 | [1.7197, 1.7238] |
| `fieldvec-dot-1024-1core` | 6 | 0.7725 | [0.7699, 0.7748] | 1.2945 | [1.2906, 1.2989] |

## Safety, assembly and correctness

The [original failing evidence](evidence/req07-failing-test.txt) preserves the
unreachable-YMM defect. [Default-selection failing evidence](evidence/default-failing-test.txt)
shows the subsequent adoption defect: production selected YMM despite the
confirmatory rejection. The corrected default-selection test expects XMM;
the explicit-preference test still proves that this AVX512VL-free host reaches
YMM through safe detection.

The exact YMM contract is AVX2 + VPCLMULQDQ + PCLMULQDQ + SSE4.1. AVX2 covers
lane construction/extraction; VPCLMULQDQ covers the YMM multiply; PCLMULQDQ and
SSE4.1 cover the odd tail through `clmul_u64`. Both the runtime predicate and
`#[target_feature]` declaration name all four. Sequential code requires
PCLMULQDQ and SSE4.1. `clmul_u64_scalar` has no CPU-feature requirement.

The [Rust 1.95 assembly](../../../crates/gf2-kernels-simd/src/x86/asm/clmul.asm.txt)
is generated by the established `dev/scripts/regen-asm.sh`. `-C link-dead-code`
retains the standalone scalar symbol for inspection without enabling any ISA
feature; target CPU is unset. It contains `vpclmulqdq` with YMM operands,
sequential XMM `pclmulqdq`, the YMM function's XMM tail, the detect-time selector,
and the scalar bit-scan/shift/XOR loop. No AVX512 instruction or ZMM register
is required. [Assembly and scope audit](evidence/assembly-audit.json) pins the
command, source/assembly digests and unchanged arithmetic bodies. The assembly
must be committed together with the final SIMD source change.

The YMM loop still builds and extracts its two lanes and retains its indexed
bounds checks. That instruction mix is consistent with the regression; it does
not isolate a causal bottleneck. No hardware-counter profiling, per-core frequency trace, packing rewrite,
unrolling, fusion or long-polynomial optimization is performed. The finite search
establishes no winning crossover beyond the declared sizes.

The shared raw-batch operand suite runs the selected YMM lane, sequential lane
and default bundle against the portable scalar oracle. It covers random and
adversarial operands, zero, the high bit, complete 128-bit outputs, empty
batches, odd lengths, non-vector tails and 0/1/63/64/65 boundaries. The safe
preferences also preserve slice-length panics. FieldVec coverage includes random
scalar equivalence, adversarial GF(2^8), GF(2^16) and GF(2^32) operands, chunk
boundaries and its documented empty-input panic. [Preflight](evidence/preflight-tests.txt)
and [adoption tests](evidence/adoption-tests.txt) record the focused checks.
[Portable consumer checks](evidence/portable-tests.txt) pass all eight selected
tests with the core SIMD feature disabled;
[evidence/cargo-ci.txt](evidence/cargo-ci.txt) records the repository CI contract.

## Diagnostic limitation retained with the evidence

In the frozen measurement adapter, `conversion.unpack_ns` times an untimed
whole-consumer preflight, not an isolated unpack operation. Those raw diagnostic
values are preserved and must not be summed with packing or interpreted as
standalone reduction costs. The primary paired timing windows already include
the complete consumer; the adoption decision uses only those windows. The
working adapter reports zero separate unpack cost because the consumer returns
a field element directly, and removes that redundant preflight. This diagnostic
correction has no fresh confirmation: the measured producer remains explicitly
versioned by its receipt-local source snapshot, and the losing candidate is not
adopted. Reproduction must use that snapshot's adapter and libraries, not the
post-decision working tree. The frozen family is not re-used for another trial.

## CI prerequisites

Full CI exposes unrelated prerequisites, recorded in
[evidence/ci-prerequisite-failures.txt](evidence/ci-prerequisite-failures.txt).
The GPU allocation test gates only an explicitly unavailable HIP device and
compares dispatcher metadata with the detected target instead of assuming one
host model. Other detection failures still fail. The composer Cargo.lock is
supplied before its provenance test, and the observation test requires its
profile codec feature. A match-guard simplification resolves Rust 1.95 Clippy's
existing `collapsible_match` error without changing acceptance behavior.
No GPU allocation or kernel execution is verified in this sandbox.

## Criterion outcomes

| Criterion | Working-tree outcome | Evidence |
|---|---|---|
| REQ-01 | MET; negative confirmation accepted and baseline retained | Frozen receipt, input snapshots, acceptance summary and this interpretation |
| REQ-07 | MET; available YMM explicitly selectable without AVX512VL | Original failing evidence; `gf2m.rs`; repaired target-feature predicate |
| REQ-08 | MET | Shared raw-batch suites, FieldVec consumer suites and test logs |
| REQ-09 | MET; all declared cells regress | Confirmatory raw samples, three-session execution log and checkpoints |
| REQ-10 | MET in files; lead must commit source and assembly together | Rust 1.95 assembly and scope audit; no arithmetic-body changes |

Commits, formal gate evaluation and tracker/document linking belong to the lead.
The full repository CI contract passes with exit 0; its main test tier reports
5959 passed, 0 failed and 248 skipped. Every configured CI step passes. The
verbatim verdict is recorded in [evidence/cargo-ci.txt](evidence/cargo-ci.txt).
The temporary lead checkpoint passes the assembly pairing gate, as recorded in
[evidence/asm-gate.txt](evidence/asm-gate.txt); the final merge requires its own gate run.
