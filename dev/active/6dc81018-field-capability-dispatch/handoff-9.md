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
