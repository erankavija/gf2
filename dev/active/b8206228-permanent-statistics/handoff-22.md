# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 26

**Date:** 2026-08-30
**Session number:** 26
**Prior handoffs:** `handoff.md` through `handoff-21.md`. Their unresolved traps remain in force.

## Current state

- Epic `b8206228` remains backlog with all epic gates pending.
- Main is clean at `d014e120`; that commit records the new `6639435f` MAX-rework escalation.
- This session closed `2c1ae813` (accelerator launch-cost table), `3f664839` (preregistered rare-event design), and `8ec10dde` (exact compressed-state propagation). Their configured gates and lead reviews passed. The final `7a816262` frozen-invocation repair also closed before this frontier.
- `73317b2e` is in progress but stopped before edits. Its lease was released. It is blocked on the first owner decision below; no campaign draw, device work, or measurement ran.
- `6639435f` is in progress on clean worktree `.agents/worktrees/agent-6639435f`, branch `worktree-agent-6639435f`, HEAD `49b4d452`. It is not merged. Its lease is `93ca69e9-01e4-4b61-ba64-46264126b767` and was due to expire at 10:52:32+03:00; renew or reacquire it before resuming.
- No Cargo or nextest process is active. No ignored slow test, target/coverage estimating run, campaign run, or GPU work was started.

## What just happened

- The frozen campaign invocation was repaired and revalidated without changing the 63-cell scientific manifest.
- `2c1ae813` committed the exact 15-cell accelerator launch-cost table plus Rust preflight/validation. Every configured gate passed.
- `3f664839` produced the binding rare-event design at `dev/active/3f664839/design.md`, including exact weighting, address partitions, immutable schemas, attempt/checkpoint lineage, no-replace publication, fixed target/coverage allocations, and runtime provenance. Two review cycles closed artifact-schema and execution-provenance findings; all gates passed.
- `8ec10dde` added exact arbitrary-precision compressed-state propagation in `gf2-algebra`; all gates passed. A pre-existing stale rustdoc-link defect is tracked as `049a89af`.
- The high-tier `6639435f` branch now contains ten commits from `570ff55c` through `49b4d452`: exact weighted statistics, the canonical addressed nullspace proposal, closed artifact schemas, strict lineage validation, immutable publication/lifecycle APIs, and deterministic final-receipt regeneration.
- The required `rare_event_artifact_final_receipt_regeneration` test passes in 2.466 s. It covers all 24 target and 27 coverage worker/resume layouts, all fixed boundaries, exact 2,048/19,200 checkpoint-reference sets, distinct execution provenance, stable result payload/digest, distinct outer receipts, and byte-identical regeneration. Focused Rust 1.95 clippy, format, and diff checks pass.
- Independent final audit of publication commit `c05b5f11` returned four HIGH findings after the issue reached its ordinary two-cycle rework limit. The worker stopped after committing the already-started regeneration test; it did not start the thin runner or full CI.

## Owner decisions required

### 1. Frozen campaign order (`73317b2e`)

Hard REQ-12 requires `(q,n)=(7,20)` to reach a terminal state before any other campaign-purpose cell is scheduled. The pinned emitter SHA-256 `2d6edcd9…` accepts only `--q` and sorts the selected q=7 field by increasing `(q,n,shard_id)`, so it necessarily schedules n=4 before n=20. An outside coordinator cannot enforce REQ-12 against that executable.

Options:

1. Authorize a narrowly versioned exact-cell selector in the emitter and re-freeze its executable digest and invocation before any draw. **Recommended.**
2. Amend REQ-12 to the order the field-only emitter can enforce, accepting n=4 before n=20.
3. Leave both unchanged; the coordinator and all three campaign arms remain blocked.

### 2. Publication/lifecycle audit (`6639435f`)

At rework MAX, final audit found:

1. path-based symlink/type checks can be swapped between validation and use; publication needs stable directory descriptors plus descriptor-relative no-follow/beneath operations;
2. the public generic publisher does not bind destination names to payload kinds/phases, allowing lifecycle bypass;
3. recovery trusts caller-authored liveness snapshots with no observation time instead of collecting PID/start-token/boot identity from the OS at the recovery barrier;
4. `ValidatedCheckpoint` proves valid bytes but not durable publication, so a terminal can claim a never-published checkpoint and wedge filesystem-derived resume.

Options:

1. Authorize one bounded high-tier rework covering all four findings, followed by a fresh independent review and every configured gate. **Recommended.**
2. Repair only typed publication, OS liveness, and durable checkpoint reconstruction, while explicitly excluding concurrent symlink-swap resistance from the threat model.
3. Leave `c05b5f11` unchanged; the thin production runner and issue completion remain blocked.

## What to do next

- [ ] Obtain explicit owner decisions for both escalations. Do not infer authorization from the request to resume the epic.
- [ ] If `6639435f` option 1 is approved, resume the existing clean worktree/branch with a high-tier worker. Keep one bounded rework covering all four findings as one coherent filesystem/lifecycle boundary; then implement the reusable orchestration/runtime-observation API and make `src/bin/permanent_rare_event.rs` a one-frozen-config-argument delegate. A decode-only placeholder is not acceptable.
- [ ] Require adversarial symlink-swap tests, exact destination-to-payload/phase tests, OS-observed and timestamped PID-reuse/live-child tests, and filesystem-reconstructed checkpoint/attempt tests. Keep fixture constructors behind `test-support`.
- [ ] Run the focused release suite, Rust 1.95 clippy/fmt/doc, raw `./scripts/cargo-ci.sh`, then configured `cargo-ci`, `code-review`, and `doc-review`. Only after fresh lead review passes should the branch merge and `6639435f` close.
- [ ] If `73317b2e` option 1 is approved, repair and re-freeze before any draw, then re-dispatch the coordinator. Its first observable campaign action must be q=7,n=20 alone through terminal state. Campaign arms remain downstream of that standing acceptance layer.
- [ ] After the campaign arms and deterministic finalization close, proceed topologically through wave 12 (`53d8e438`, `b05c908b`, `f74e327d`) and wave 13 (`0665e7be`, `b5d45bcd`), then run the epic gates and holistic review.

## Traps — do not repeat these

- **Do not merge or cherry-pick the `6639435f` branch yet.** It is a clean, coherent review branch but final audit is red and the production runner is absent.
- **Do not weaken the four audit findings locally.** Rework is at MAX; any correction requires the owner-authorized counter reset recorded in `progress.json`.
- **Do not use caller-supplied `Option<PID>` values as liveness proof.** Recovery must own a current OS observation and bind its observation time.
- **Do not let byte-valid fixture handles enter production lifecycle APIs.** Terminals and resumes must reconstruct exact published checkpoint state from strict directories.
- **Do not expose an untyped publication escape hatch.** Destination grammar and payload kind/phase must be one checked semantic operation.
- **Do not rely on `symlink_metadata` followed by path reopen for hostile/concurrent publication parents.** Use stable directory handles and descriptor-relative no-follow/beneath resolution.
- **Do not run Cargo/nextest concurrently against the shared target.** All substantial tests remain release mode. Do not opt into ignored coverage/campaign tests during ordinary implementation.
- **Do not start any campaign draw before both the exact-cell selection and frozen digest/invocation decisions are resolved.** The raw campaign remains empty.
- Preserve every committed measurement receipt and frozen artifact. Never edit `dev/active/b8206228-permanent-statistics/breakdown.json`.

## Reference artifacts

- Epic: `jit issue show b8206228`
- Open implementation issues: `jit issue show 73317b2e`, `jit issue show 6639435f`
- Progress and escalations: `dev/active/b8206228-permanent-statistics/progress.json`
- Rare-event design: `dev/active/3f664839/design.md`
- Frozen campaign: `dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/`
- Accelerator costs: `dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv`
- Importance worktree: `.agents/worktrees/agent-6639435f` at `49b4d452`
- Main escalation commit: `d014e120`
