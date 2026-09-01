# Handoff — issue `389aa4de` owner escalation (2026-09-01)

## Current state

- Epic `6dc81018` remains in wave 25; `06ba0418` is done.
- Issue `389aa4de` is assigned to `agent:codex`; lease
  `cdfebbb3-620f-4307-b0f0-49bb2b9b7765` remains active.
- Worktree branch `worktree-agent-389aa4de` is clean at unmerged commit
  `c746072b895dabc9e699118c516071a115b89274`.
- Main is clean at `501fe8b2` in this handoff.

## Valid measured evidence

Claude executed the coordinated host-owning calibration at clean producing
revision `501fe8b28f351ac05e52cccd96ce6f0ae9898cdf`. The successful third attempt
ran 5 executions by 5 repetitions at 250 ms across 45 grid points, producing
2,250 raw samples and 108 verified Karatsuba child observations. Three
independent audits reproduced the selected values `4`, `31`, `383`, `1024`,
and `512`, the exact 5-measured/32-omitted codec result, all hashes, and both
owner-wrapper identities. The evidence is valid; the blocker is test-contract
quality in its publication change.

## Open escalation

Two approved protocol retries exhausted the execution-lead rework limit:

1. remove unsupported `--release` from `cargo bench --no-run`;
2. read raw executable paths with `cat` instead of parsing them as JSON, and
   make the preflight fail closed.

Attempt 3 succeeded and publication commit `c746072b` passes focused Rust 1.95
tests, the complete baked slice, formatting, and workspace clippy. Independent
pre-merge code review nevertheless found three blocking test defects:

1. `tuning_profile_committed.rs` derives the 37-field inventory from the
   committed conservative artifact instead of
   `CoreTuningCodec::encode_body(&CoreTuning::CONSERVATIVE)`.
2. `tuning::baked` hashes the measured owner and separately asserts the literal
   4, but does not strictly reopen the owner and relationally compare
   `SIMD_MIN_WORDS` to its measured field.
3. `tuning_repository_envelopes.rs` repeats all five measured core literals
   instead of comparing the complete typed section and measurement to the
   authoritative core owner.

The invoker must choose targeted guidance with a counter reset, manual lead
takeover, or rejection. The lead recommends the targeted reset: all three
changes are narrow test-only corrections, preserve the measured artifacts and
production behavior, and directly enforce REQ-02, REQ-04, and
`@/inv/semantic-test-assertions`.

After approval, amend the worker branch, rerun its focused checks and
independent review, then announce and merge under
`/tmp/gf2-main-leads.lock` before running the configured JIT gates. Coordinate
every main merge and JIT write with Claude through
`/home/vkaskivuo/Projects/forum-poc/forum.sh`.

## Invoker decision

The invoker selected option A. The rework counter is reset, and one narrow
test-only retry is authorized for the three findings above. This is guided
retry 1 after the reset. It changes neither the measured artifacts nor
production behavior. Re-run focused tests, the baked slice, formatting,
clippy, artifact byte checks, and independent pre-merge review before merging.

## Post-merge gate finding

Guided retry 1 passed independent review and merged at `561b0d71`. The exact
merged tree passes `./scripts/cargo-ci.sh` with 5,299 tests and the configured
code-review gate with zero findings. The configured doc-review gate found two
blocking single-source-prose defects: `KERNEL_OPTIMIZATION.md` and
`QUALITY_AUDIT_REPORT.md` both say the new receipt records the direct-backend
versus dispatcher distinction under a section named **“What this receipt does
not claim”**, but the receipt has no such section.

Rework 2 after the reset is limited to making those citations resolve to one
explicit authoritative scope statement in the receipt. It must not alter the
measured owner, complete envelope, raw log, checksum manifest, harness, or
production behavior. Re-run formatting, link/heading checks, the checksum
manifest, independent documentation review, and the configured doc-review
gate. Any further blocking finding exhausts the reset rework allowance and
requires invoker escalation.

## Rework 2 review result

Worker commit `66123a77` adds a resolving **“SIMD pilot scope”** heading and
repoints both permanent documents. It is clean and unmerged. Independent
review found one blocking factual defect: the new scope paragraph says the
older 8-word result used `ops::xor_inplace`, but
`benches/simd_vs_scalar.rs` directly calls `ScalarBackend::xor` and the
detected SIMD backend, just as the current calibration harness does. The
replacement explanation therefore invents a call-path distinction.

The reset rework allowance is exhausted. Do not merge `66123a77`. The invoker
must choose targeted guidance with another counter reset, manual lead
takeover, or rejection. The lead recommends one documentation-only reset:
remove the false dispatcher distinction and state only the evidence-backed
protocol and producing-revision scope of the two result sets, then rerun
independent documentation review and the configured doc-review gate.

## Second invoker decision

The invoker selected option A. The counter is reset and one
documentation-only retry is authorized. Preserve the resolving **“SIMD pilot
scope”** heading and both links, but replace the false `ops.rs` distinction
with only evidence-backed scope: the 2026-09-01 value comes from the receipt's
preregistered fixed-window protocol at its pinned producing revision, while
the older 8-word result is reported separately from the Criterion
`simd_vs_scalar` benchmark and is not reproduced or reconciled by this
receipt. Do not claim a call-path difference or invent a causal
reconciliation. Measured artifacts and production behavior remain immutable.
