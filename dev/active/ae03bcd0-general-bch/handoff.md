# Handoff — Harden and generalize BCH codes over finite fields (ae03bcd0) — session 1

**Date:** 2026-08-31T22:05+03:00
**Session number:** 1
**Prior handoffs:** none

## Current state

- Epic: `ae03bcd0` — state: backlog (container; blocked until subtree terminal). Claimed by agent:jit-execution-lead.
- Wave in progress: wave 1 of 15 (plan in `progress.json`).
- Children summary: bracket (2aa2279e, e3169db4) done pre-session; this session closed 3 of wave 1 — `ad6778e8` (ext-design), `3243bc1f` (property tests), `8f51d6cf` (LCM promotion) all done, gates green, merged to main. `4e732b56` (baseline survey) is merged and doc-review-passed but **rework-pending** on research-review. `88ca7d2f` re-slotted to wave 11 (CCX1-gated).
- Active claims: `4e732b56` and `ae03bcd0` by agent:jit-execution-lead. Release/re-claim as needed (`jit issue release <id> <reason>` first; direct re-claim is refused).
- Open escalations: none.
- Progress file: `progress.json` here (current).

## What just happened

- Coordinated with a parallel codex lead (epic b8206228) via the forum bus (`~/Projects/forum-poc/forum.sh`, identity `agent:claude`, peer `agent:codex`): shared advisory lock `/tmp/gf2-main-leads.lock` around every main/JIT write batch + forum notice first. Peer's `gf2-predraw-validation3` is armed for 2026-09-01 02:00:30 EEST — **CCX1 embargo from 01:30 until codex reports it terminal**.
- Owner-approved DAG change: story `3931ac6f` retargeted onto the epic (`--reduce`), plan.md D-03 amended; tree now renders the design chain under the epic.
- `ad6778e8`: Opus architect produced `extension-design.md` (renamed from ext-design.md to match the manifest); doc-review passed R2 after dropping the lead-caused stale path entry. Lead decisions recorded in progress.json (R-01 → 19fe9394 dispatch, R-03 footprint additions, R-04 conway-registry tower rule).
- `3243bc1f`, `8f51d6cf`: implemented (Luna / native Sonnet), merged sequentially with cargo-ci on each merged tree; all gates green; done.
- `4e732b56`: survey complete and merged — baselines selected (W1: bchlib primary, AFF3CT secondary; W2: M4RI elimination primary, AFF3CT basis-encode secondary; gaps 5949×/1110×), contract fixed, 128-cell receipt committed, six citekeys registered (`Etsi2015` replaced the pattern-invalid ETSI302755). doc-review passed R3 (two root-absolute-link rounds). research-review R1 returned **FAIL, 9 findings (F1/F2/F4 high+blocking, F5 medium+blocking, 5 advisory)** — verdict preserved in `review-4e732b56-research-r1.json` because jit discarded it (see Traps). Gate later stopped deliberately per owner instruction; **no verdict is recorded in .jit** — the gate must be re-run after rework.
- Infra: two codex startup deadlocks diagnosed (killed, no worktree damage); jit gate-retry defect root-caused to `with_mutation_attempts` (gate_check.rs:903) and reported to the `just-in-time-c5` session with the owner's directive to remove automatic retry. Untracked accidental `__pycache__` from the survey branch.

## What to do next

- [ ] Confirm the jit fix landed (message `just-in-time-c5` / check `~/.cargo/bin/jit` build) before running any long AI gate.
- [ ] Dispatch rework for `4e732b56` (attempt 1) to the still-live teammate `survey-4e732b56` (full context) or a fresh agent with `references/rework-prompt-template.md`. All 9 findings in `review-4e732b56-research-r1.json` must be addressed (no-argue; advisories included). None needs the bench host: F2's matrix-equality check is correctness-only and the built baselines live in worktree `agent-4e732b56`. F1 needs exact invocations + per-stage revision/host linkage in the receipt; F4 a committed retrieval receipt for the kodo exclusion; F5 committed `m4ri_config.h`/linkage evidence.
- [ ] Re-run `jit gate evaluate 4e732b56 research-review` after rework (fresh window; probe reviewer liveness within ~3 min). On PASS: mark done, then wave-1 close: `check-leak-into-main.sh`, reclaim worktrees `agent-ad6778e8`, `agent-3243bc1f`, `agent-8f51d6cf` (NOT `agent-4e732b56` — see Traps), advance `current_wave` to 2.
- [ ] Dispatch wave 2: `c3d5cea5` extension-trait (hard → native Opus; footprint per extension-design.md module layout, includes `field/axiom_tests.rs` per R-03) and `7a3a6738` bch-api-design (hard → Sol xhigh per alternation; producer of bchspec-model/block-code-traits contracts).
- [ ] Wave 11 note stands: `88ca7d2f` runs in the next CCX1 window codex grants; must land before cutover-benches (wave 12) merges.
- [ ] Carry the survey's three downstream notes into dispatch prompts: genmatrix-perf (basis-vector route is the wrong algorithm — replace, don't tune), avx2-batch-kernels (interleaved-decay open question — check generated code first), perf-receipts (re-establish absolutes on the kept governor).

## Traps — do not repeat these

- **Do NOT trust a silent codex exec.** It intermittently deadlocks at startup (futex wait, ~29 threads, no child processes, own `~/.codex/tmp/arg0/*` lock, and — decisive — no session rollout under `~/.codex/sessions/<date>/`). Two identical hangs cost ~75 min this session. Probe for the rollout within ~3 min of every dispatch/gate; on hang, kill and retry or fall back to a native agent. Clustered concurrent codex launches raise the odds.
- **Do NOT run a long AI gate while the repo can move.** `jit gate evaluate` (current installed build) wraps checker execution in an optimistic retry (`with_mutation_attempts`, `~/Projects/just-in-time/crates/jit/src/commands/gate_check.rs:903`); input drift during the run discards the completed verdict unrecorded and silently re-runs the checker. Fix demanded from the `just-in-time-c5` session (owner directive: no automatic retry). Until confirmed fixed: freeze all repo writes during long gates, and salvage any discarded verdict from the codex rollout (as done in `review-4e732b56-research-r1.json`).
- **Do NOT reclaim worktree `agent-4e732b56`.** It holds the built external baselines (AFF3CT v4.7.0, bchlib v2.1.3, M4RI 20260122, IT++ 4.3.1) the rework's F2 matrix-equality check needs; a reclaim forces a full refetch/rebuild.
- **Do NOT `jit issue claim` over an existing assignee.** It refuses; run `jit issue release <id> "<reason>"` first, then claim.
- **Do NOT let workers run `jit doc add` or commit from codex sandboxes without the writable-root flag.** Linked-worktree git metadata lives under `.git/worktrees/<name>/`, outside the codex workspace sandbox; pass `-c sandbox_workspace_write.writable_roots=["<repo>/.git/worktrees/<name>"]` or commit on the worker's behalf. Workers cannot mutate jit state from worktrees at all — the lead links docs post-merge.
- **Do NOT write `](/repo/path)` or backtick `/dev/...` paths in reviewed docs.** doc-review fails root-absolute repo paths one instance per round (two rounds burned this session). Sweep with `grep -nE '\]\(/|\`/dev' <docs>` before requesting the gate.
- **Do NOT assume story→epic dependency edges are containment inversions.** Hierarchy levels disambiguate; the owner confirmed the story→epic edge is a pure waits-for edge (D-03 amendment). Also `jit dep add --reduce` may drop now-redundant edges elsewhere — verify the resolved tree afterward (here it beneficially re-homed `88ca7d2f`).
- **Do NOT treat forum listening as optional.** A lapsed recv loop queued six peer messages unanswered (~35 min of peer blocking). Keep the persistent Monitor loop (`forum.sh recv --as agent:claude` in a `while true` Monitor) up for the whole session.

## Open questions needing invoker input

None. (The jit gate-retry fix was ordered by the invoker and is in the jit session's hands.)

## Reference artefacts

- Epic: `jit issue show ae03bcd0`; plan `dev/active/ae03bcd0-general-bch/plan.md`; manifest `breakdown.json`; investigation `investigation.md`.
- Extension design (contract producer): `dev/active/ae03bcd0-general-bch/extension-design.md`.
- Survey: `dev/active/4e732b56/findings.md`, `dev/active/4e732b56/workload-selection.md`, receipts `dev/bench_results/4e732b56/`, harness `dev/active/4e732b56/baseline-survey/`.
- Preserved review verdict: `dev/active/ae03bcd0-general-bch/review-4e732b56-research-r1.json`.
- Peer coordination: forum bus `~/Projects/forum-poc/forum.sh`, `FORUM_DIR=/tmp/jit-forum`, identities `agent:claude` ↔ `agent:codex`; lock `/tmp/gf2-main-leads.lock`.
- jit defect thread: peer session `just-in-time-c5` (ListAgents), jit source `~/Projects/just-in-time`.
