# Integrated QC route adoption decision

Decision record for @/issue/3ee00fcf. The decision is **adopt the QC route** under
`adoption-family` in `dev/active/55c3e433-aff3ct-ldpc-gaps/plan.md`.
The candidate carries no check variant. All measured values below belong to one
host: AMD Ryzen 9 5900X, 12 cores, 24 logical CPUs. They decide the registered
rule and support no performance claim beyond it.

## Evidence (REQ-02)

- Campaign `55c3e433-integrated-adoption-pilot`, family `55c3e433-integrated-adoption`: verdict Accepted, `qualifies` false, six sessions, twelve pairs per cell. Receipt SHA-256 `ea8df84f85ca82f2563bf68733634c41aad3a33812ef881ac161ea76eee3f447`.
- Campaign `55c3e433-integrated-adoption-confirmation`, the same family: verdict Accepted, `qualifies` true, six sessions, 24 pairs per cell. Receipt SHA-256 `87cfeaa2d11de5b8f03d5f5f6a8f83c0975b3dd2e256d25c692854c21c06a18a`.
- Both campaigns cover all six cells. Their receipts, acceptance summaries and append-only execution logs are preserved outside the tracked tree, together with the freeze derivation, frozen addendum, window output, build records, declaration copy and untimed smoke record.
- The canonical freezer, `experiments/aff3ct-ldpc-gaps/freeze-confirmation.py`, derives a widest relative half-width of 0.012162 on the 5900X pilot and freezes measurement resolution at 0.02. The frozen worthwhile speedup is 1.25 and equivalence margin is 1.1. There is one confirmation attempt.
- The build records identify the baseline arm `gf2-batch-pre-change` by executable SHA-256 `82f3c72aef7a40221937466161be00508b286830e84ce62d8709316512588dd8` and the candidate arm `gf2-batch-qc-candidate` by executable SHA-256 `9ba08077b76a3b4c62d6740b125c7feb64136b23d4740813748e5f9db5d6b4e6`. Both use Rust 1.95, release builds and `-C target-cpu=native`.

Every cited SHA-256 is independently computed with `sha256sum`. The receipt
digests match the expected values; executable digests match the build records.

## Rule application

On the 5900X host, the canonical evaluator reports `qualifies` true, with no
findings. The six confirmation outcomes are Pass. The following values are the
acceptance summary's rounded estimates and intervals on that host.
Speedup is median baseline time divided by median candidate time, as defined by
`speedup_of_medians` in `dev/tools/tuning-campaign-support/src/abtest.rs`.
Each paired-bootstrap interval has confidence 0.9958333333333333
(99.58333333333333%; the Markdown summary displays 0.9958).

| Confirmation cell (5900X) | Objective | Speedup estimate | Interval | Decision | Outcome |
|---|---|---:|---|---|---|
| `dvb-t2-r12-w1-b1` | non-regression | 1.0000 | [0.9973, 1.0019] | NotWorse | Pass |
| `dvb-t2-r12-w1-b8` | non-regression | 0.9972 | [0.9937, 0.9992] | NotWorse | Pass |
| `dvb-t2-r12-p12-b8` | non-regression | 1.0018 | [0.9957, 1.0075] | NotWorse | Pass |
| `nr-bg1-z384-w1-b1` | improvement | 9.2531 | [9.2155, 9.2925] | Improved | Pass |
| `nr-bg1-z384-w1-b8` | non-regression | 0.9851 | [0.9652, 0.9903] | NotWorse | Pass |
| `nr-bg1-z384-p12-b8` | improvement | 8.3011 | [8.1455, 8.3328] | Improved | Pass |

REQ-01's no-campaign branch does not apply because the QC route is delivered.
REQ-04's negative or inconclusive branch does not apply because the confirmation
evaluator reports `qualifies` true. The pilot alone does not select production.

## Complexity budget

The frozen budget is `max_new_unsafe_kernels` 1 and `max_added_source_lines` 1500.
The delivery adds one unsafe kernel,
`crates/gf2-kernels-simd/src/x86/ldpc_qc.rs`.

The measured addition is **907 production Rust source lines**, or **1,984 including
tests**. Neither `adoption-family` nor the frozen declaration states whether the
allowance counts test source. The total including tests exceeds 1,500. After the
confirmation, the owner ruled on 2026-11-04 that `max_added_source_lines` binds
production source, because the allowance derives from the delivery's estimated size
as a maintenance cost. Under that ruling the delivery is within budget, at 907 of
1,500 lines. Under a count that includes tests, it would exceed the budget by 484
lines. The measurement sums the added column of `git diff --numstat 87ec416d3 HEAD`
for Rust source files under `crates/`. Production excludes `tests/` directories
and additions inside `#[cfg(test)]` modules. Zero-context diff hunk positions
identify 137 added lines inside the test module of
`crates/gf2-coding/src/ldpc/route.rs`; the other added source lines outside
`tests/` are production. Counts include comments and blank lines; JSON profiles
and the assembly listing are not Rust source. These counts differ from the
handoff's 898 and 1,882 by 9 and 102, respectively.

## What lands (REQ-03)

- The committed production envelope is `crates/gf2-coding/data/tuning-profiles/gf2-3ee00fcf-20261104t064432z-4012544.json`. Its two `qc_route_cases` name NR BG1 Z=384 at one worker with one frame and twelve workers with eight frames. Normalized min-sum with attached matching QC structure takes QC at those cases; other cases keep their existing route.
- `dev/tools/tuning-profile-compose` accepts the fixed input `--fixed-selector decode.qc_route_cases CASES_JSON` in `coding-owner` mode. It reads the cases through the owner codec and preserves the calibrated section and measurement provenance while applying that fixed predicate.
- The tuned arm installs this same production envelope through `ENVELOPE` and `install_envelope` in `experiments/aff3ct-ldpc-gaps/src/tuned.rs`. The confirmation candidate's envelope has identical selector values.
- The superseded envelope resides byte-identically at `experiments/aff3ct-ldpc-gaps/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json` for the pinned pre-change arm. Comparison against its blob at `87ec416d3` confirms byte equality; no copy remains in the production profile directory.
- No check variant is carried. `crates/gf2-coding/src/ldpc/min_sum.rs` has no diff against `87ec416d3`, so the check row is unchanged and has no second form or runtime selector.

## Numerical contract (REQ-05)

QC matches the canonical decoder's posterior bit patterns, hard decisions,
iteration counts and syndrome/convergence flags within
`docs/reference/ldpc-decoder-numerical-contract.md`. The conformance suites assert:

- `crates/gf2-coding/tests/ldpc_qc_contract.rs`: posterior-bit, word and outcome equality on portable and detected backends, including special values, zero caps, reuse, both stopping modes and shared frame contracts.
- `crates/gf2-coding/tests/ldpc_decode_dispatch.rs`: the selected structure and worker/frame cases, portable fallback, exclusion of other cases and longer batches, and equality of decoded words and outcomes with the canonical decoder.
- `crates/gf2-coding/tests/ldpc_batch_contract.rs`: per-frame posterior-bit and outcome equality on QC backends, including special values, frame permutations, wave boundaries and rate-matched inputs.
- `crates/gf2-coding/tests/tuning_profile_committed.rs`: installation in a fresh process, exact committed QC cases, executed-route reporting and canonical words/outcomes under that envelope.
- `crates/gf2-kernels-simd/tests/ldpc_qc.rs`: portable and detected kernel outputs match the shared check contract bit for bit, including special values and boundary sizes.

The preserved untimed smoke record reports QC at the two targeted NR cases and
the existing routes elsewhere. The suites pass in `./scripts/cargo-ci.sh`, the
`cargo-ci` gate of @/issue/3ee00fcf, at source revision `890c3e8be`.

## Inputs note

The two prepared quality files are absent from the preserved evidence. They
come from `545a3c814:dev/bench_results/c077a88b/v3-preparation/quality/` and are
byte-identical to the files both pilots use. Comparing `git show` bytes with
the worktree's prepared input root, then hashing each with `sha256sum`, gives:

- `gf2-nms-f32-dvb-t2-r12.json`: SHA-256 `15fd3cb1c0818b6c14780f9ba74961ea9f52d38b216f883ea3b4c28c19af3ca6` for both copies.
- `gf2-nms-f32-nr-bg1-z384.json`: SHA-256 `d93ca58e2b19539ceb7e4cd71cb5e43046dc73390d218d86f4e4b3e06cefa42f` for both copies.

## Result that would have reversed the decision

On the 5900X host, an improvement-cell lower bound below 1.25 reverses adoption
(observed: 9.2155 at NR `w1-b1`, 8.1455 at NR `p12-b8`). A non-regression-cell
lower bound below $1/1.1$ reverses adoption (observed: 0.9973, 0.9937 and 0.9957
at DVB-T2 `w1-b1`, `w1-b8` and `p12-b8`; 0.9652 at NR `w1-b8`).
Any evaluator finding (observed: none) or conformance failure also reverses
adoption (observed: the `cargo-ci` gate passes). So does exceeding the complexity
budget: more than one new unsafe kernel (observed: one), or more than 1,500 added
production source lines under the owner's ruling (observed: 907).
