# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 12

**Date:** 2026-09-05T11:41:33+03:00
**Session number:** 12
**Prior handoffs:** `handoff.md`, `handoff-2.md`, `handoff-3.md`,
`handoff-4.md`, `handoff-5.md`, `handoff-6.md`, `handoff-7.md`,
`handoff-8.md`, `handoff-9.md`, `handoff-10.md`, `handoff-11.md`

## Current state

- Epic: `6dc81018` — state: in_progress
- Wave in progress: wave 27 of 28
- Children summary: the epic's direct rollup is 11 done and 1 backlog; the
  remaining nested chain is `a83583e0` in progress followed by direct child
  `dbd8787d` in backlog.
- Active claims: `6dc81018` and `a83583e0` are assigned to
  `agent:jit-execution-lead`; no advisory worker lease remains.
- Open escalations: `a83583e0` needs an invoker decision for the real-host
  admission evidence interface before gates or measurement.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- Recovered one stale JIT claims lock and reconciled every prior handoff with
  the live graph; `eaae1b56` is done and wave 27 is current.
- Stopped the initially dispatched Astra worker immediately when the invoker
  prohibited Astra-class subagents; it made no file change.
- Dispatched a Sol xhigh recovery worker over the inherited `a83583e0` WIP and
  a parallel read-only Terra lifecycle audit.
- Closed four premeasurement blockers: exact `/tmp` stage identity, durable
  pre/post-build source observations, actual staged CLI preflights, and complete
  provenance/budget/environment configuration.
- The first Terra review failed one driver/validator canonical-path mismatch.
  Rework 1 added fail-closed path guards and mutation tests; independent
  re-review passed.
- Focused results pass: support 105/105, core harness 136/136, algebra harness
  11/11, committed-profile 2/2, focused Clippy, Rust formatting, shell/Python
  syntax, validator self-test, and `git diff --check`.
- Committed premeasurement tooling as `ecd85315`, the progress rework record as
  `0e6fdaad`, and the manifest link as `505792b2`.
- Linked `dev/active/a83583e0/producing-build-inputs.json` to `a83583e0`.
- `jit validate` passes with the repository's pre-existing loose-mode warnings.

## What to do next

- [ ] Read the invoker's response to the host-admission decision below.
- [ ] If option A is approved, amend the protocol with the exact canonical JSON
  schema and boundary, implement driver/validator/launcher ingestion and
  fail-closed tests, and record the owner decision in `progress.json`.
- [ ] Run the complete `a83583e0` premeasurement checks and configured gates on
  a clean committed tree; apply the six-tier lead review.
- [ ] Build the exact Rust 1.95 release executables and run only the untimed
  self-check, list-grid, and representative capability preflights.
- [ ] Observe a quiet real host, create the approved admission evidence, and
  execute/resume the 717-cell campaign through the single outer full-host lock.
- [ ] Independently validate and atomically publish the core owner, algebra
  owner, complete envelope, receipt, raw results, execution log, session record,
  and checksum manifest; remove the v3 prepublication boundary in the same
  cutover and re-run all `a83583e0` gates.
- [ ] Close `a83583e0`, advance to wave 28, execute `dbd8787d`, then perform the
  epic completion review, gates, report, transition, and archive.

## Traps — do not repeat these

- **Do not treat a static admission policy, mutex ownership, or a sandbox
  process list as host-idle evidence.** Protocol section 7 explicitly requires
  real-host load, memory, and competing CPU/GPU observations; the missing input
  interface is the current escalation.
- **Do not let the Rust driver accept evidence paths the independent validator
  rejects.** Rework 1 now requires exact canonical, non-symlink locations for
  both build-source observations and all six staged preflight reports before
  locked work.
- **Do not start the campaign from an arbitrary stage or dirty/moving source.**
  The launcher and driver require the exact `/tmp/gf2-a83583e0-<stamp>-<pid>`
  identity plus matching clean pre/post-build HEAD and tree observations.
- **Do not use Astra-class subagents.** The invoker prohibited them during this
  session; use Sol, Terra, or Luna according to task difficulty.
- **Do not run measurement or gates before the host-admission contract is
  approved and implemented.** Changing the reviewed protocol or issue boundary
  without the invoker's approval violates the execution-lead scope rule.
- Prior handoff traps remain in force, especially one outer benchmark lock, no
  Cargo or commits under/during the timed run, immutable historical evidence,
  exact staged executables, durable execution logging, and no schema-token-only
  publication.

## Open questions needing invoker input

- Question: What exact interface carries the real-host admission observations
  required by protocol section 7?
  - Context: The lead must observe actual load, available memory, and competing
    CPU/GPU work, and the driver must record and enforce those facts; the
    protocol rejects sandbox process listings and mutex ownership as sufficient
    but predeclares no schema, path, or attestation semantics.
  - Options: (A) add a canonical lead-produced JSON artifact supplied before
    preparation, validated fail-closed, hashed into immutable campaign identity,
    and recorded in the journal/receipt; (B) make the unsandboxed launcher gather
    and attest the observations directly under a newly defined policy; (C) amend
    the protocol so the existing static policy binding is sufficient.
  - Recommendation: A. It preserves explicit human/lead host observation,
    separates acquisition from enforcement, and gives the validator a durable,
    hash-bound record without weakening the reviewed contract.

## Reference artefacts

- Epic: `jit issue show 6dc81018`
- Current issue: `jit issue show a83583e0`
- Premeasurement protocol: `dev/active/a83583e0/premeasurement-protocol.md`
- Producing manifest: `dev/active/a83583e0/producing-build-inputs.json`
- Driver and validator: `dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs`,
  `dev/scripts/validate-tuning-extent-campaign.py`
- Governing designs: `dev/active/7d824b2f/design.md`,
  `dev/active/3fa7c9d0/design.md`
- Implementation checkpoint: `ecd85315`
- Progress file: `dev/active/6dc81018-field-capability-dispatch/progress.json`
