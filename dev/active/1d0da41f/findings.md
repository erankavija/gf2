# YMM carry-less batch dispatch on Zen 3

## Finding and adoption decision

**The premise was falsified on this Ryzen 9 5900X.** Enabling the YMM lane
makes every declared raw-batch and FieldVec consumer cell slower. The
[confirmatory receipt](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/receipt.json)
is accepted with no protocol finding and **does not qualify for adoption**
(tables "Protocol-v1 confirmation", source line and "Findings"). Every cell
decides `regressed` (same section, `Decision` column); no negative, unavailable
or inconclusive result is omitted. The
[acceptance summary](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/acceptance-summary.md)
is the authoritative record of the measurements, and
[`survey/summarize.py`](survey/summarize.py) projects it and every other
committed receipt of this issue into the
[generated tables](../../bench_results/1d0da41f/tables.md). This report carries
the argument and cites the evidence: it states no measured or derived value.
Every figure lives in `tables.md`, in the receipts and summaries themselves, or
in the evidence files named below, and each pointer here names the `tables.md`
section and row that carries it.

A number appears in this report only when it identifies a cell, names a
workload size, names a protocol or addendum constant that fixes the design
before measurement, or belongs to a citation key, version pin or commit id.

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
a new confirmatory performance claim. Its first exploratory
[pilot](../../bench_results/1d0da41f/v3-pilot/acceptance-summary.md) covers the
four single-core cells and is accepted (its finding count is on the Source
line of the tables' "Protocol-v3 exploratory pilots" section for `v3-pilot`),
but its odd-tail cell
draws an interval wide enough to put the conservative resolution above both
frozen margins (tables "Protocol-v3 exploratory pilots", row
`raw-batch-odd-tail-65-1core`; "Measurement resolution", row "v3 exploratory
pilot", whose `Frozen margins admissible` cell reads no). A separately frozen,
one-cell
[resolution pilot](../../bench_results/1d0da41f/v3-pilot-r2/acceptance-summary.md)
at the protocol's maximum pilot pair count is accepted (its finding count is
on that section's Source line for `v3-pilot-r2`), and the
resolution it derives leaves the original margins admissible: one plus that
resolution stays below both the equivalence margin and the worthwhile-speedup
threshold the family froze (tables "Measurement resolution", row "v3 resolution
pilot"; "Frozen margins and declared resolution", row "Protocol-v3
confirmation").

The resulting five-cell [v3 receipt](../../bench_results/1d0da41f/v3-confirmation/receipt.json)
is structurally accepted, preserves all samples, and again estimates every YMM
cell as slower (tables "Protocol-v3 confirmation", `Speedup [interval]`
column). Its [acceptance summary](../../bench_results/1d0da41f/v3-confirmation/acceptance-summary.md)
classifies every cell as `not-confirmatory` under P-20 (same section, `Outcome`
column and "Findings"). The retrospective v1 reservation and the fresh v3
reservation together set the family's reserved comparison count and its second
attempt, and the attempt alpha that follows leaves the expected bootstrap draws
per tail below the protocol's fixed minimum (tables "Protocol-v3 confirmation"
→ "Family accounting (P-20)", whose `tail condition` row reads fails). That
limit is independent of the observed interval widths. The one permitted v3
candidate attempt is spent, so no timing retry or post-hoc method change is
made.

The accepted v1 confirmation remains the governing negative evidence. The v3
continuation independently agrees in direction but is retained as
non-confirmatory evidence of a protocol-budget incompatibility. Neither body of
evidence supports YMM default adoption, and the sequential default remains in
place.

A fresh checkout does not reproduce the v1 confirmation's accepted verdict. Its
saved plan predates the typed `producing_manifest` field, so the evaluator
compares the survey manifest the receipt pins against the historical shared
manifest that omission selects and reports a P-23 campaign-start mismatch; the
[re-evaluation record](../a203a23c/receipt-reevaluation.json) states both
manifest paths beside the verdict. Every file that receipt pins is committed,
its measurements and its committed summary are unchanged, and the three v3
receipts above reproduce their verdicts. The conclusion the v1 confirmation
governs is therefore unaffected — the v3 continuation reaches it independently —
while the v1 verdict itself no longer re-derives from the repository.

## Question and method

Does correcting the AVX512VL requirement improve raw independent 64-by-64
carry-less products and `FieldVec::<Gf2mElement>::simd_dot_product`? Both arms
compute the complete 128-bit product with canonical little-endian polynomial
bit indexing. The FieldVec consumer packs field elements, dispatches raw
batches, XOR-accumulates the products and reduces the accumulated value.
All of those consumer operations are inside the timed call. Construction of
fixture inputs is outside the timed call and reported as setup telemetry
(tables "Protocol-v1 confirmation" → "Untimed conversion diagnostics").

The [shared measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md),
[receipt-pinned protocol v1](../../bench_results/1d0da41f/2026-09-08-1d0da41f-clmul-dispatch-confirmation/inputs/protocol.md) and
[frozen family](addendum-ymm-clmul-dispatch.json) govern this result. The family
bytes are those the receipt pins by digest (`receipt.addendum.sha256`), and its
resolution evidence is the distinct committed exploratory pilot receipt the
addendum names (`effect.resolution_evidence`; tables "Frozen margins and
declared resolution", row "Protocol-v1 confirmation").
The runner captures that pilot at `inputs/resolution-evidence/receipt.json`
before the opening campaign record. No addendum amendment, timing override,
post-hoc threshold, repeated confirmatory trial or sample exclusion is used.

The [executed commands](evidence/reproduction.md) record the build, lock, resume
and finalization sequence. The bounded launcher is [run-clmul-ab.sh](../../bench_results/1d0da41f/run-clmul-ab.sh),
which gives one session a bounded number of cells; the campaign completed across
the session count its receipt records (tables "Protocol-v1 confirmation", source
line and "Sessions and host"). Sessions run under
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

Both gf2 libraries and the adapter carry the version their Cargo manifests
declare ([`survey/ymm-clmul-arm/Cargo.toml`](survey/ymm-clmul-arm/Cargo.toml)).
The two arm builds use the toolchain the receipt observed (tables "Protocol-v1
confirmation" → "Sessions and host"), release optimization, thin LTO, one
codegen unit, no native ISA flags, and the adapter's locked dependency graph.
Library license metadata is pinned by the root and crate Cargo manifests.
The baseline source archive is
[survey/baseline-source.tar.gz](survey/baseline-source.tar.gz); its content digest
is in the receipt's producing-input manifest. The candidate source is saved in
`inputs/producing/crates/` inside the receipt. Relative to that baseline, the
production arithmetic changes are confined to the raw-batch dispatch/safety
repair; the FieldVec changes are tests.

The baseline arm is the predicate that requires AVX512VL and the candidate is
the repaired predicate. Each executes one dispatch path in every execution of
every cell — the baseline `pclmulqdq-scalar-xmm`, the candidate
`avx2+vpclmulqdq-ymm` — and each arm's executable digest is joined to its
receipt in the tables ("Arm executables"; "Protocol-v1 confirmation" → "Per-arm
call time and selected path", `Selected path` column).

The receipt's runtime observations identify the host, its CPU model, SMT state
and governor (tables "Protocol-v1 confirmation" → "Sessions and host"). The
latency cells resolve one CPU and the streaming cell resolves the six CPUs of
one core complex (same section, `CPUs` column), yet **declares and observes one
worker** (`Workers observed` column): it is a single-worker sustained
measurement on a six-core affinity mask, not a six-worker scaling result.
Twelve-core and 24-logical-CPU scaling are outside this single-worker family,
not silently measured or inferred. No external raw-product arm is declared, and
no gf2x build is attempted; a long-polynomial multiply is not this independent
product operation.

## Confirmatory measurements

Each cell runs the protocol's confirmatory pair count of fresh paired
executions, with the protocol's window count and window target per arm execution
(`settings.confirmatory_pairs`, `settings.windows_per_execution` and
`settings.window_target_ms` in the receipt). The point estimate is the ratio of
baseline and candidate medians, and the paired bootstrap interval uses the
frozen resample count at the family-corrected per-comparison confidence (tables
"Protocol-v1 confirmation", source line). No window is flagged in any cell
(`Flagged windows` column). The candidate-over-baseline columns are algebraic
transformations of the same estimates and endpoints, not new samples.

Every cell decides `regressed`: the confidence interval of each places YMM
outside the frozen non-regression margin and below the raw-cell
worthwhile-speedup threshold (tables "Protocol-v1 confirmation", `Speedup
[interval]` and `Decision` columns, against "Frozen margins and declared
resolution", row "Protocol-v1 confirmation"). The consumer cell
`fieldvec-dot-1024-1core` regresses as well as the isolated kernel cells, so the
frozen rule retains the established sequential implementation. This is a
measured rejection of YMM adoption, not an inconclusive absence of improvement.
The final detect-time plumbing is correctness-tested; its exact overhead is not
separately timed and no additional speedup is claimed for it.

## Preserved exploratory falsification

The [pilot summary](../../bench_results/1d0da41f/2026-09-07-1d0da41f-clmul-dispatch-pilot/acceptance-summary.md)
covers the four single-core cells at the protocol's minimum pilot pair count,
with percentile-bootstrap intervals at its own family confidence and no flagged
window (tables "Protocol-v1 exploratory pilot"). It is exploratory and
contributes no samples to confirmation. Its toolchain differs from the
confirmation's, which both receipts record (tables "Protocol-v1 exploratory
pilot" and "Protocol-v1 confirmation" → "Sessions and host", toolchain lines).
Its historical producer snapshot covers the protocol tooling; the independent
confirmation supplies the complete arm/library source closure. The pilot arm
binaries are not independently reconstructed here. Its direction agrees with
confirmation in every cell it shares (tables "Protocol-v1 exploratory pilot",
`Decision` column), and its samples are never pooled with confirmation.

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
is committed together with the final SIMD source change.

The YMM loop still builds and extracts its two lanes and retains its indexed
bounds checks. That instruction mix is consistent with the regression; it does
not isolate a causal bottleneck. No hardware-counter profiling, per-core frequency trace, packing rewrite,
unrolling, fusion or long-polynomial optimization is performed. The finite search
establishes no winning crossover beyond the declared sizes.

The shared raw-batch operand suite runs the selected YMM lane, sequential lane
and default bundle against the portable scalar oracle. It covers random and
adversarial operands, zero, the high bit, complete 128-bit outputs, empty
batches, odd lengths, non-vector tails and the repository's 0/1/63/64/65 word
boundaries. The safe preferences also preserve slice-length panics. FieldVec
coverage includes random scalar equivalence, adversarial GF(2^8), GF(2^16) and
GF(2^32) operands, chunk boundaries and its documented empty-input panic.
[Preflight](evidence/preflight-tests.txt) and
[adoption tests](evidence/adoption-tests.txt) record the focused checks.
[Portable consumer checks](evidence/portable-tests.txt) pass every selected test
with the core SIMD feature disabled, and its nextest summary line carries the
counts; [evidence/cargo-ci.txt](evidence/cargo-ci.txt) records the repository CI
contract.

## Diagnostic limitation retained with the evidence

In the frozen measurement adapter, `conversion.unpack_ns` times an untimed
whole-consumer preflight, not an isolated unpack operation. Those raw diagnostic
values are preserved (tables "Protocol-v1 confirmation" → "Untimed conversion
diagnostics") and must not be summed with packing or interpreted as standalone
reduction costs. The primary paired timing windows already include the complete
consumer; the adoption decision uses only those windows. The working adapter
reports zero separate unpack cost because the consumer returns a field element
directly, and removes that redundant preflight. This diagnostic correction has
no fresh confirmation: the measured producer remains explicitly versioned by its
receipt-local source snapshot, and the losing candidate is not adopted.
Reproduction must use that snapshot's adapter and libraries, not the
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

| Criterion | Outcome | Evidence |
|---|---|---|
| REQ-01 | MET; negative confirmation accepted and baseline retained | Frozen receipt, input snapshots, acceptance summary, generated tables and this interpretation |
| REQ-07 | MET; available YMM explicitly selectable without AVX512VL | Original failing evidence; `gf2m.rs`; repaired target-feature predicate |
| REQ-08 | MET | Shared raw-batch suites, FieldVec consumer suites and test logs |
| REQ-09 | MET; every declared cell regresses | Confirmatory raw samples, execution log and checkpoints; tables "Protocol-v1 confirmation", `Decision` column |
| REQ-10 | MET; assembly committed with the final SIMD source change (`c9e498b6`) | Rust 1.95 assembly and scope audit; no arithmetic-body changes |

The repository CI contract passes, and its step summary carries the verdict and
the main test tier's counts ([evidence/cargo-ci.txt](evidence/cargo-ci.txt)).
The assembly pairing check is recorded in [evidence/asm-gate.txt](evidence/asm-gate.txt).
