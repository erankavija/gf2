# Terminal-invariant audit of the ed3d490e manifest

> **Diátaxis Type:** Reference

Planning record of `2133d15f`. It applies the terminal invariant of the
planning skill (`.agents/skills/jit-planning-lead/SKILL.md`, "Terminal
invariant") and the plan-review prompt (`contrib/gates/plan-review-prompt.md`)
to the manifest the audit reads, one row per leaf of that manifest, and names
the leaves of [breakdown.json](breakdown.json) that replace each failing one.

The standard is the one the plan-review gate applies: one protocol family per
leaf, one tool per leaf, one evidence mechanism per leaf, code apart from
permanent documentation, deletion apart from caller migration. A unit below is
anything that standard counts as independently deliverable or independently
testable. A leaf with one unit passes; a leaf with more is split into the
fewest leaves that pass, or loses its odd unit to an existing leaf of the same
kind where a new leaf would be a single-file edit with one criterion.

## Audit

| Leaf | Units | Result |
|---|---|---|
| `decoder-numerical-contract-page` | reference page; rustdoc edit in a code file | move: the rustdoc citation goes to `ldpc-lane-batch-decoder` |
| `lane-kernel-portable` | one kernel module with its suite | pass |
| `decode-selector-family` | selector family with codec; typed route decision | split: `decode-selector-family`, `decode-route-decision` |
| `batch-soft-decoder-trait` | trait with default paths; shared contract suite over five decoders | split: `batch-soft-decoder-trait`, `batch-decoder-contract-suite` |
| `ldpc-lane-batch-decoder` | one decoder type with its unit tests | pass |
| `batch-conformance-suite` | one suite | pass |
| `lane-kernel-avx2` | one kernel file with its required assembly artefact | pass |
| `batch-decoder-avx2-route` | one backend selection, existing suites on one more route | pass |
| `batch-worker-pool` | one adapter | pass |
| `decode-batch-test-callers` | one caller family | pass |
| `decode-batch-bench-callers` | one caller family | pass |
| `decode-batch-howto-page` | one page passage | pass |
| `decode-batch-removal` | one deletion | pass |
| `nr-rate-matched-batch-decode` | one implementer | pass |
| `dvb-t2-concat-batch-decode` | one implementer | pass |
| `sim-batch-decode-stage` | one stage type | pass |
| `sim-cpu-ldpc-stage-batch` | one stage | pass |
| `sim-nr-decode-stage-batch` | one stage | pass |
| `sim-dvb-t2-stage-batch` | one stage | pass |
| `sim-bler-sweep-batch` | one binary | pass |
| `sim-batch-determinism` | worker-count determinism; checkpoint resume identity | split: `sim-batch-worker-determinism`, `sim-batch-resume-identity` |
| `gf2-benchmark-arms` | one arm family | pass |
| `aff3ct-benchmark-arms` | arm set with one new arm; INTRA quality record | split: `aff3ct-benchmark-arms`, `intra-arm-quality-record` |
| `benchmark-arms-validation` | build identity; quality validation; peak memory; runner smoke | split: `arms-build-identity`, `arms-quality-validation`, `arms-peak-memory-record`, `arms-runner-smoke` |
| `campaign-tooling` | generator; launcher; smoke runner | split: `addendum-generator`, `campaign-launcher`, `plan-smoke-runner` |
| `single-worker-family-preparation` | one protocol family | pass |
| `multicore-family-preparation` | one protocol family | pass |
| `batch-sweep-family-preparation` | one protocol family | pass |
| `matched-family-preparation` | one protocol family | pass |
| `fastest-compatible-family-preparation` | one protocol family | pass |
| `before-after-pilot-collection` | three protocol families | split: `single-worker-pilot-collection`, `multicore-pilot-collection`, `batch-sweep-pilot-collection` |
| `comparator-pilot-collection` | two protocol families | split: `matched-pilot-collection`, `fastest-compatible-pilot-collection` |
| `before-after-confirmation-collection` | two protocol families | split: `single-worker-confirmation-collection`, `multicore-confirmation-collection` |
| `comparator-confirmation-collection` | one protocol family | pass, as `matched-confirmation-collection` |
| `coding-tuning-producer` | one tool with its harness test | pass |
| `campaign-driver-coding-owner` | driver; composer | split: `profile-composer-coding-owner`, `campaign-driver-coding-owner` |
| `campaign-validator-coding-owner` | one tool | pass |
| `decoder-calibration-preparation` | one campaign's declaration set | pass |
| `decoder-calibration-collection` | one campaign's stage; a repository test | move: the envelope test goes to `decoder-dispatch-verification` |
| `selector-holdout-preparation` | one arm; one protocol family | split: `tuned-profile-arm`, `selector-holdout-preparation` |
| `selector-holdout-pilot-collection` | one protocol family | pass |
| `selector-holdout-confirmation-collection` | one protocol family | pass |
| `decoder-dispatch-verification` | tests under the committed envelope | pass |
| `batch-decoding-reference-pages` | permanent pages of one subject | pass |
| `lane-profile-preparation` | profiling executable; summarizer rules; series launcher; case file with queue line | split: `lane-profile-executable`, `profile-summarizer-lane-rules`, `lane-profile-launcher`, `lane-profile-preparation` |
| `lane-profile-collection` | one series | pass |
| `outcome-publication` | table generator; findings record | split: `outcome-tables`, `outcome-publication` |

## Leaves at the limit

- `ldpc-lane-batch-decoder` carries the decoder type, its allocation
  assertion, its route reporter and one rustdoc citation. They are one type's
  contract and are tested in one module.
- `decoder-calibration-preparation` carries a declaration, a protocol
  amendment, a producing manifest, untimed producer records and a queue line.
  They are one campaign's premeasurement set, the shape of a family
  preparation leaf.
- `decoder-dispatch-verification` carries the route observation tests and the
  envelope decode test. Both run under the committed envelope and neither is
  more than one test file.
- `batch-decoding-reference-pages` edits three permanent pages on one subject.
- Each `*-family-preparation` leaf and `selector-holdout-preparation` carries
  a family declaration, its addendum, a ledger arithmetic record, a smoke
  record and a queue line: one family's freeze.
- Each freezing `*-pilot-collection` leaf verifies a pilot and freezes that
  family's confirmation, one family's stage transition.
