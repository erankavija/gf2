# Dense-parity production outcome (jit:6e87c436)

> **Diátaxis Type:** Research

**Verdict: no change.** The [dense-parity decision record](../50f0bd42/outcome.md)
selects no adoption: the fusion portfolio of the
[baseline and portfolio outcome](../c73ffa25/outcome.md) holds no candidate,
and both confirmatory cells of the M4RI [AlbrechtBard2026] comparator family
record decision `regressed` and outcome `fail`. The fused AND-popcount route
remains the production route of `BitMatrix::matvec`. No production source,
confirmation addendum, holdout campaign, or timed run follows from that
decision.

## Why the fused AND-popcount route remains selected

- **No fusion candidate.** The
  [portfolio decision](../c73ffa25/outcome.md#portfolio-decision) finds the
  measured work inside the existing fused kernel body and identifies no
  distinct full-count fusion within the frozen
  [cost boundary](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md#operations-and-cost-boundaries).
  The generic carry-save comparator stays the preserved
  [non-qualifying result](../../5cbb6545/findings.md), and the scalar
  four-accumulator route stays a reference arm.
- **No external gap.** The
  [confirmation acceptance summary](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/acceptance-summary.md)
  records `m4ri-gap-65x512-warm` and `m4ri-gap-65x4096-warm` as confirmatory,
  `regressed`, `fail`, at the pair count and corrected confidence its rows
  state. The compared arm is M4RI, so both rows fail the comparator-gap
  objective in gf2's favour. The reading holds for the fresh whole-consumer
  boundary, where packing and unpacking are charged to the M4RI arm; it makes
  no claim about isolated `mzd_mul` throughput.
- **No adoption objective.** The
  [decision](../50f0bd42/outcome.md#decision) states that a comparator outcome
  authorizes no production change in either direction.

## Preserved record

The complete fusion and M4RI record stays committed as its producers wrote it:

- The [isolated fused-parity](../../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/acceptance-summary.md)
  and [allocated matvec](../../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/acceptance-summary.md)
  pilot summaries, whose rows are exploratory with outcome `pilot`; the
  allocated summary keeps its `inconclusive` and `regressed` rows beside the
  `not-worse` ones.
- The [M4RI pilot summary](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/acceptance-summary.md),
  every row exploratory, `regressed`, `pilot`, and the two failing
  confirmatory rows above. The generated
  [tables](../../../bench_results/2037941f/m4ri-gap-tables.md) carry each
  cell's interval, pair count, selected paths, and setup costs.
- The unavailable-row companions of the
  [pilot](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/unavailable-rows.tsv)
  and the
  [confirmation](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/unavailable-rows.tsv),
  which record each predeclared unqualified shape as `unavailable` with zero
  samples and zero comparisons.
- The family's append-only
  [ledger](../../../bench_results/2037941f/dense-matvec-vs-m4ri-ledger.jsonl),
  whose last line is the one confirmatory reservation.

## Measured tree and current tree

The dense receipts describe the tree they measured. The
[production drift record](survey/production-drift.json) shows that the current
`gf2-core` and `gf2-kernels-simd` sources are not byte-identical to that tree:
other issues changed them after the campaigns. The record classifies every
measured file and names the commits behind each code difference. On the dense
`matvec` path the code differences are the tail masking of
`BitVec::from_words` (jit:79873126) and the baked `bit_backend` threshold with
its profile owner (jit:dbd8787d, jit:a83583e0); the comment sweeps that touch
`matrix.rs` and the AVX2 kernel source leave their code text equal under the
record's classifier rule.

The [route comparison](survey/route-comparison.json) settles whether that
drift moves the selection the receipts measured. For every cell arm of the
four dense receipts, the path the current tree reports through the harness's
route witness equals the one path the measured pairs report; the record's
`differing_cell_arms` fields carry the count. The arm executables differ in
digest between the two trees, and the record lists both digests per row. This
task takes no timing on the current tree, so the receipts' intervals remain
statements about the measured tree.

## Criterion and evidence

| Criterion | Disposition and evidence |
|---|---|
| REQ-01 — outcome reflected, or behaviour and selection unchanged | The no-adoption branch applies. The `task_change` object of the [drift record](survey/production-drift.json) lists no production path changed against this task's anchor. The [source ledger](survey/source-evidence.json) pins the public method, the stride selector, the conservative and baked thresholds, the detected-bundle fallback, and the fused bundle call. The [route comparison](survey/route-comparison.json) and the baked route witness in the [verification record](verification.md#route-selection) show the same selection for every measured cell and for the baked configuration. |
| REQ-02 — shared scalar/SIMD suites | The [verification record](verification.md#semantic-suites) lists the Rust 1.95 commands, logs, and the suite coverage of each named case. The same suites run with and without the `simd` feature. This task adds the deterministic boundary-shape test and the word-offset test to the shared suites; both are test code. |
| REQ-03 — isolated unsafe code, assembly, Rust 1.95 build | This task adds or modifies no unsafe code and no SIMD symbol, so no new annotated assembly exists; `./scripts/asm-artefact-present.sh` passes with no SIMD source changed. The [source ledger](survey/source-evidence.json) locates the fused kernel and its unsafe call in `gf2-kernels-simd` and the `unsafe_code` denial in `gf2-core`; it also records that the fused bundle entry's call site carries no `SAFETY` comment of its own, while the neighbouring carry-save entries state the detection contract. Rust 1.95 builds the path in the [focused suites](verification.md#semantic-suites) and in the release arms of the [route smokes](verification.md#route-selection). The measured baseline's annotated release assembly is the committed [kernel](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/dense-baseline/simd/avx2-fused.asm.txt) and [matvec](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/dense-baseline/simd/simd-matvec.asm.txt) listing, whose [index](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/dense-baseline/simd/index.txt) names the arm executable by the digest the baseline receipts record. |
| REQ-04 — holdout samples for any production change | No production change exists, so a paired holdout has no after arm and no holdout receipt applies. |
| REQ-05 — worthwhile allocated whole-`matvec` benefit, or no acceptance with contradictory evidence committed | No change is proposed or accepted. The losing, inconclusive, and unavailable rows listed under *Preserved record* stay committed and unchanged. |
| REQ-06 — receipt identity | No change receipt is emitted. The four dense receipts ([isolated](../../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/receipt.json), [allocated](../../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/receipt.json), [M4RI pilot](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/receipt.json), [M4RI confirmation](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/receipt.json)) pin the contract, protocol, addendum schema, campaign addendum, producing-input manifest and per-file source digests, arm executables, ledger, execution log, and checkpoints by SHA-256, with byte snapshots under each receipt's `inputs/` directory, and record the observed host, toolchain, and lock. Each receipt's launcher log beside it holds the exact invocation. The [profile log](../../../bench_results/2037941f/dense-baseline-profile-r2/execution.log) pins the profile's arm, tool, and addendum digests, each cell directory beside it holds its commands, and the assembly index above pins the disassembled executable. |

## Scope of this verdict

The verdict covers the frozen dense-parity portfolio and the qualified M4RI
comparison. It adds no performance claim: every interval, decision, and
outcome cited here is the committed evaluator output of the measured tree.
