# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 10

**Date:** 2026-09-12T08:25Z
**Session number:** 10
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md), [session 7](handoff-6.md), [session 9](handoff-7.md)

## Current state

- Epic: `1a379447`, backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 2 of 7.
- Children: 5 done, 1 in_progress, 5 ready, 7 backlog, 0 rejected. Plus four
  non-child in-flight issues (`04b85d10`, `6fb89a3c`, `3e59cb9a` done, `3be770d5`).
  `jit issue status` and [progress.json](progress.json) are authoritative.
- Active claims: no leases. Lead assignment on `04b85d10`, `6fb89a3c`,
  `26465e6c`; `agent:worker` on `6c6b09b1`, `3be770d5`, `3e59cb9a`. Every
  session-10 worker has stopped; revive by name (`w10-<id>`).
- Open escalations: none. Four invoker decisions this session, recorded in
  progress.json.
- **No benchmark window is armed.** The invoker deferred it to after this
  session and will set the time. [queue.tsv](bench-window/queue.tsv) holds
  seven jobs, about 215 estimated minutes.

### Closed this session

| Issue | Notes |
|---|---|
| `3e59cb9a` | DVB-T2 warm-pass repair and re-measurement. All four gates pass first time; lead review PASS. Found two defects the v4 merge created — see Traps. |
| `26465e6c` | Rework round 1 merged, then the lead swept the narrative v4 invalidated and regenerated `reevaluation.txt` against the v4 evaluator: every verdict, finding and summary comparison unchanged, the two evaluator digest lines the whole diff. Four gates pass, two independent Terra xhigh reviews return zero findings. |
| `bdc507a3` | Protocol v4. Three gates pass; independent review passes with one advisory that is exactly `a203a23c`. REQ-04 verified independently by the lead: 59/59 identical verdicts and summaries. Six artifacts linked. |
| `a387825e` | CCX1 lock-descriptor fix. Two doc-review rework rounds, both on the lead's own wording (see Traps). Three gates pass; independent review clean. Three artifacts linked. |

### Per-issue state

| Issue | Branch / worktree | State and next step |
|---|---|---|
| `04b85d10` | merged | Nine-session repeated profile committed and independently verified. **Does not close: code-review round 2 returned three blocking findings (see What to do next).** Rework counter is 2 of 2, so a third failure escalates. Five artifacts linked; `popcnt-attribution.txt` could not be linked (see Traps). |
| `6fb89a3c` | merged at `19af1d61` | Three corrected re-measurements committed and verified. **Does not close: see the freeze-script finding below.** Gates need re-running after that is fixed. 13 artifacts linked, 6 relabelled. |
| `3e59cb9a` | **done** | All four gates pass; lead review PASS. 15 artifacts linked. Its one deferred-items match is `157c305c`, already filed. |
| `a203a23c` | merged at `76812a4e` | All three criteria MET per the worker; cargo-ci, code-review and doc-review pass; **research-review FAILED on one blocking finding, so it does not close** (see What to do next). 9 artifacts linked. Restored 9 of 14 receipts; repository-wide 57/62 reproduce, up from 48. |
| `6c6b09b1` | `worktree-agent-6c6b09b1-v3` at `361e7929` in `.agents/worktrees/agent-6c6b09b1` | Wire fix and v4 migration done; three pilots queued. Everything downstream waits on the window. Do not merge before the window runs. |
| `3be770d5` | `worktree-agent-3be770d5` at `95447b2a` in `.agents/worktrees/agent-3be770d5` | Three pilots committed; profile series void and preserved; two v4 confirmations and a profile re-run queued. REQ-02 and REQ-03 wait on the window. Do not merge before the window runs. |
| `1c602857` | `worktree-agent-1c602857` in `.agents/worktrees/agent-1c602857` | Conformance and routing done; pilot queued. REQ-04 needs **two** windows: pilot, then commit the receipt and freeze, then confirmation. |

## What just happened

- Diagnosed the three `6c6b09b1` pilots that died in 13 ms in the 2026-09-12 window: its arm crate decoded the runner's A/B pair position (`"baseline"`/`"candidate"`) as the protocol's `CellRole` enum. Fixed with contract tests that scan the runner's own `ArmRequest` declaration.
- Added a standing worker rule: an untimed end-to-end smoke through the real `benchmark-ab-runner`, reaching a result line from every arm the plan names, is mandatory before any window-job line. It caught the identical defect class in `1c602857` before that one cost a window.
- Merged `a387825e` and `bdc507a3` to main, cargo-ci green on each, flock test passing on the merged tree. Then merged `26465e6c`, `04b85d10` and `6fb89a3c`.
- Swept an epic-wide staleness the v4 merge created: five findings documents labelled a link "protocol version 3" pointing at a document that now reads version 4.
- Pruned six stale worktrees and reclaimed two more after their issues closed; free disk 38G to 65-78G, worktrees 17 to 12.
- Hit the Codex weekly usage limit at 07:05Z, which failed two gate suites with "Agent produced no output". The invoker reset it.

## What to do next

- [ ] **Fix `dev/active/6fb89a3c/survey/freeze-remeasure.py` before closing `6fb89a3c`.** Its `pilot_history()` globs `dev/bench_results/6fb89a3c/*/execution.log` and filters only on family id and exploratory role, with no skip for the campaign the addendum itself drives, while `main()` still adds `+1` for "this campaign". Its three re-measurement receipts are now committed, so each campaign is counted twice and the committed script no longer reproduces the committed addenda. The fix is `3e59cb9a`'s, at commit `5c260134` on `worktree-agent-3e59cb9a`: skip a campaign whose `campaign-start` record names this addendum as its own path. Not reproducible from main — the arm executables are gitignored — so verify inside the worktree.
- [ ] File an issue for the schema-versioning defect: protocol v4 replaced `dev/active/f547c394/addendum.schema.json` in place rather than versioning it alongside. `addendum-v1.schema.json` is preserved; v2 and v3 are not, so the only surviving copies of the v3 schema are inside receipt `inputs/` snapshots, and anything validating a frozen v3 addendum against the shared path fails on two const constraints. `bdc507a3` is already closed, so this needs its own issue.
- [ ] **Rework `04b85d10` (round 2 of 2 — a third failure escalates).** code-review returned three issue-impact blocking findings, two of them measurement-validity rather than wording. F1: `dev/active/04b85d10/addendum-bit-storage-layout-v3-confirmation.json:11` labels degree-14 short-frame BCH measurements as normal-frame, so the frozen addendum's declared workload does not match the receipt. F2: the n=64800 whole-consumer receipt reports conversion costs from a short-code setup and a 32448-bit proxy rather than the declared normal-frame consumer (`survey/gf2-side/src/lib.rs:781` in the count-v3-confirmation producing snapshot). F3 (medium): `survey/summarize-receipts.py:93` hard-codes receipt-specific provenance and correction prose instead of deriving it from structured receipt evidence. Full verdict in `.jit/gate-runs/`; it cites REQ-01, REQ-02, REQ-03 and `@/invariant/benchmark-backed-performance`.
- [ ] **Rework `a203a23c` (round 1).** research-review F1, high, blocking: tagged commit `76812a4e` reports the checker's 23.1 s to 2.59 s and the restoration generator's 60.2 s to 10.1 s reductions with no committed benchmark artifact, invocation, hardware or provenance. References `@/invariant/claims-trace-to-artifacts` and `@/invariant/benchmark-backed-performance`. The lead caused this by asking for the cost measurement without saying where the artifact recording it would live. Fix by committing an artifact that carries the invocation, host and timings, and pointing the claim at it. Its other three gates pass.
- [ ] `428f2f6b` is now unblocked: `3e59cb9a` closed, so the eda07788 findings are free for its citation pass.
- [ ] Ask the invoker for the window time, then arm the timer over the committed queue.
- [ ] After the window: `3be770d5` (profile re-run first), `6c6b09b1` (three pilots), `1c602857` (pilot, then freeze, then a second window for the confirmation).
- [ ] Remaining ready work, not started: `428f2f6b` (after `3e59cb9a` closes — both edit the eda07788 findings), `12fdeb5b`, `53c5a8c0`. Briefs for all three are written; recreate from this handoff if the scratchpad is gone.
- [ ] File the jit asset-scanner bug (see Traps).

## Traps — do not repeat these

- **Do NOT verify a runner/arm wire contract by code reading.** `6c6b09b1` did, and all three of its queued pilots died in 13 ms, costing a whole window slot. `1c602857` ran the mandatory smoke instead and caught the same class of defect — explicit `null`s where the runner skips the field, rejected by the canonical round trip — before it cost anything. Two independent surveys, same trap.
- **Do NOT trust a queued job's exit code as evidence it measured anything.** `3be770d5`'s 45-minute profile job exited 0 while all 342 of its cases had failed and written no record: the harness logged each case's status and continued, and nothing checked that a session left a usable case.
- **Do NOT build a completeness guard on the terminal record's `measured_cells`.** `benchmark-ab-runner` increments it after `measure_cell` regardless of outcome, and a resumed session that finds every cell checkpointed completes with zero. A guard built on it passes an all-inapplicable campaign and **fails a legitimately resumed one**. Count `cell-complete` records carrying pairs instead. Found only because the untested branch was exercised against a synthesized fixture.
- **Do NOT commit on main while an AI gate is running.** The gate record's commit then no longer names the tree the reviewer read. The lead did this once at `5daab50e`; the change was outside the reviewed issue's footprint so the verdict stood, but do not rely on that.
- **Do NOT write a doc comment that asserts the failure mode as current behaviour.** `a387825e`'s doc-review rejected the lead's first rework for saying a daemon keeps the mutex until it exits — which is exactly what the wrapper's unlock prevents — and for saying the daemon retains the turnstile, which it never receives (`ccx1-bench-flock.sh:100` closes that descriptor in the child). Verify a comment against the implementation before submitting it, not after the reviewer rejects it.
- **Do NOT assume a generator that reproduced its output still does once its campaign lands.** A freeze script that scans committed campaign receipts while producing an artifact a campaign then receipts counts itself twice. Every input is committed, so it looks deterministic until its own output's campaign is committed. Check by re-running with the frozen timestamp and reading `git status`.
- **Do NOT expect `jit doc add` to accept a `.txt` containing a demangled Rust symbol.** The asset scanner reads `memchr::arch::x86_64::memchr::count_raw::find_avx2` as a relative path and refuses it as "not lexically confined", for every doc type. Affects committed disassembly and profiler dumps.
- **Do NOT send a worker report over about 6000 characters.** The drain drops it whole rather than truncating; `3be770d5`'s first final report was lost entirely. Brief workers to split.
- **Do NOT run two independent cross-reviews per issue without watching the budget.** Four gate suites plus four independent Terra xhigh reviews exhausted the account's weekly Codex limit mid-session, failing two gate suites with "Agent produced no output" — which records as a gate FAILURE indistinguishable from a finding.
- **Do NOT ask a worker for a performance number without saying where the artifact recording it will live.** The lead asked `a203a23c` what its new CI steps cost; the worker measured, improved them tenfold, and put the before/after timings in a commit message. research-review then failed the issue on an unsupported performance claim. The measurement was right and the request was right; what was missing was the artifact the claim had to point at.
- **Do NOT trust a worktree's pre-seeded `target/` without suspicion when the compiler contradicts cargo.** A pool-seeded tree reported `can't find crate for sha2` and `serde_json` from rustc while both `.rlib` and `.rmeta` were present, readable and correct-length and cargo reported them Fresh. `cargo clean --release` over the affected crate's dependencies fixed it in 40 s. The dispatch script hardlinks pool entries, so a stale pool entry propagates to every worktree seeded after it.
- **Do NOT re-freeze finished work to satisfy a tool that a protocol bump broke.** When protocol v4 made a frozen v3 addendum unvalidatable, the fix was to validate against the version-3 snapshot the receipt pins and assert that snapshot's digest against `receipt.addendum_schema.sha256` — not to regenerate the frozen artifact.
- Unresolved traps from [session 9](handoff-7.md#traps--do-not-repeat-these) and earlier remain in force.

## Open questions needing invoker input

- Question: when should the next benchmark window run?
  - Context: seven jobs, about 215 minutes, are committed in queue.tsv; the invoker deferred the window to after session 10.
  - Options: A) a 04:00 Europe/Helsinki slot as in prior sessions; B) another time.
  - Recommendation: A, and run it before the next session so its results are ready to collect.

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [progress](progress.json).
- Protocol v4: `dev/active/f547c394/{protocol.md,amendment-v4.md,addendum.schema.json}`.
- Window: [run-window.sh](bench-window/run-window.sh), [queue.tsv](bench-window/queue.tsv); `systemctl --user list-timers | grep gf2`.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.
- Gate records: `.jit/gate-runs/`.
