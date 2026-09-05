# Handoff — Reconcile field capability traits and profile-driven kernel dispatch (6dc81018) — session 13

**Date:** 2026-09-05T11:47:33+03:00
**Session number:** 13
**Prior handoffs:** `handoff.md` through `handoff-12.md`

## Current state

- Epic `6dc81018` remains in progress at wave 27 of 28.
- Direct rollup remains 11 done and 1 backlog. The remaining nested chain is
  `a83583e0` in progress, followed by direct child `dbd8787d` in backlog.
- `a83583e0` premeasurement tooling and rework 1 are committed and independently
  reviewed. Configured gates and the timed campaign have not run.
- The prior host-admission escalation is resolved: the invoker selected option C.
  No open escalation remains.
- No Astra-class subagent may be used. The planned wave-28 model is changed to
  Sol xhigh; future work uses Sol, Terra, or Luna according to difficulty.

## What just happened

- The invoker selected the least-friction option C: amend protocol section 7 so
  the existing exact static host-admission policy binding is sufficient.
- This decision rejects both a new lead-produced live-host JSON artifact and a
  new unsandboxed launcher-attestation interface.
- A Sol xhigh implementation dispatch and Terra high read-only audit dispatch
  were started, then immediately interrupted when the invoker directed handoff
  and stop. Neither dispatch changed a file.
- No protocol, implementation, test, gate, measurement, publication, or JIT
  issue-state change was made after the decision.
- The repository was clean before this handoff/progress checkpoint.

## What to do next

- [ ] Amend `dev/active/a83583e0/premeasurement-protocol.md` section 7 so exact
  static policy binding is the complete host-admission contract. Remove the
  requirements for lead-observed live load/memory/competing-work evidence and
  driver-recorded live observations.
- [ ] Audit the Rust driver, Python validator, launcher, and tests for matching
  semantics. Keep the existing exact `HostAdmissionPolicy` binding fail-closed;
  make only changes required for a truthful, internally consistent contract.
- [ ] Obtain independent non-Astra review of the protocol amendment and any code
  changes. Record the decision in the issue history as needed.
- [ ] Run all configured `a83583e0` premeasurement checks and gates on a clean
  committed tree, then apply the six-tier lead review.
- [ ] Build and preflight the exact Rust 1.95 release executables, then execute
  or resume the 717-cell campaign through the single outer full-host lock.
- [ ] Independently validate and atomically publish all required owners,
  envelope, receipt, raw results, log, session record, and checksums; complete
  `a83583e0` gates and transition it to done.
- [ ] Execute wave 28 `dbd8787d` with a non-Astra agent, then complete the epic's
  holistic gates, completion report, transition, and archive.

## Traps — do not repeat these

- **Do not re-open the host-admission choice without a new contradiction.** The
  invoker explicitly chose static policy binding for lower friction.
- **Do not claim that live host load, memory, or competing-work observations are
  recorded after option C.** Amend the protocol truthfully; do not retain prose
  promising evidence the tooling does not produce.
- **Do not weaken unrelated controls.** Exact `/tmp` stage identity, clean and
  stable source, canonical non-symlink evidence paths, staged CLI preflights,
  one outer lock, no Cargo under the lock, durable execution logging, immutable
  historical evidence, and fail-closed validation remain mandatory.
- **Do not use Astra-class subagents.** Use Sol, Terra, or Luna only.
- Prior handoff traps remain in force, especially driver/validator agreement,
  atomic publication, v3 prepublication-boundary removal, and separate code/JIT
  state commits.

## Open questions needing invoker input

- None. Option C is approved. The next lead should implement it directly.

## Reference artefacts

- Epic: `jit issue show 6dc81018`
- Current issue: `jit issue show a83583e0`
- Protocol: `dev/active/a83583e0/premeasurement-protocol.md`
- Producing manifest: `dev/active/a83583e0/producing-build-inputs.json`
- Driver: `dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs`
- Validator: `dev/scripts/validate-tuning-extent-campaign.py`
- Governing design: `dev/active/7d824b2f/design.md`
- Implementation checkpoint: `ecd85315`
- Prior durable checkpoint: `623f5568` and JIT link `f82e31f2`
- Progress file: `dev/active/6dc81018-field-capability-dispatch/progress.json`
