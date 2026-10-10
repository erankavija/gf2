# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 33

**Date:** 2026-10-11T00:00Z
**Session number:** 33
**Prior handoffs:** `handoff.md`, then `handoff-2.md` through `handoff-28.md` in this directory.

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`).
- Wave in progress: 4 of 7; story `ed3d490e` runs its own layers inside it.
- Direct children: 21 of 26 done or rejected; open `bfc2ceab`, `7dbe1393`, `8e53235b` (in progress), `817fc47d`, `1362381c` (backlog). `63bad95d` is in progress below `1362381c`.
- Story `ed3d490e`: `e4e2ba52`, `395fe4e2`, `a96a4cd6`, `feb60d0d` closed this session; `bad1aea8` in rework 1; `aff59c3c` blocked on an escalation.
- Active claims, each `agent:worker`: `bfc2ceab`, `63bad95d`, `7dbe1393`, `8e53235b`, `aff59c3c`, `bad1aea8`.
- Open escalations: eight, listed under "Open questions needing invoker input".
- Benchmark window: timer `gf2-bench-window-20261011` armed for 2026-10-11 04:00 Europe/Helsinki. One new job: the `8e53235b` pilot, whose queue line starts the launcher only below a one-minute load of 2 and otherwise exits 75 and stays queued.
- Pinned worktrees, none merged into main: `agent-bfc2ceab` (`c6869fc6d`), `agent-63bad95d-fam` (`a257dd63b`), `agent-63bad95d-run` (`d808af2f3`), `agent-63bad95d-m4ri` (`998a0a3d4`), `agent-63bad95d-pm4ri` (`1aad07d0e`), `agent-7dbe1393` (`ca918074f`), `agent-8e53235b` (`d0dcfbcb2`).
- `jit validate` fails with five `profile-ownership` errors that commit `2b4d4a4ba` of epic `b4b4b9ee` introduced.
- Progress file: `progress.json` in this directory reflects the above.

## What just happened

- The 2026-10-09 window ran its seven jobs to a terminal `complete` while the session of epic `b4b4b9ee` merged and gated on main; `.agents/bench-window/window.log` records the load at each job start. Every family it measured is refused by its freezer or inconclusive:
  - `63bad95d` native ladder: confirmation committed, three cells pass and three are inconclusive, the confirmatory attempt is spent; `63bad95d/disturbance-record.md` on the family branch.
  - `63bad95d` dispatch overhead: second pilot committed, freeze refused, pilot budget spent.
  - `63bad95d` native and portable M4RI: pilots committed, both `resolution-insufficient`.
  - `bfc2ceab`: second pilot committed, freeze refused, no pilot trial left; the cost is recorded as exploratory and REQ-04 is unmet as written.
  - `7dbe1393`: profile session and pilot committed, pilot `resolution-insufficient`; the candidate kernel stays unselected.
- `e4e2ba52` closed: the two inherent LDPC batch functions are removed.
- `395fe4e2` closed after one rework: the resume tests interrupt for real and bound batches to the processed frames.
- `a96a4cd6` closed after one rework: the three decode stages gain `with_selectors` and `with_portable_lanes` (lead decision; the stages exposed no route control), and the tests assert the route each case takes.
- `feb60d0d` closed after two reworks: the arms manifest cites the `harness-path-dependencies` exception; the build recorder decides from content only.
- `8e53235b` stage 1 on its branch: `BitVec::write_bits`, a selectable word-wise assembly with production unchanged, family `8e53235b-bch-codeword-assembly` frozen and smoked, pilot queued.
- `bad1aea8` merged at `a82eea0a2`; cargo-ci and research-review pass; code-review failed on an undersized-buffer check before the AFF3CT FFI call; rework dispatched.
- Seven merged worktrees reclaimed without harvest.

## What to do next

- [ ] Collect the 2026-10-11 window: `.agents/bench-window/window.log`, then the `8e53235b` campaign's own execution log. A job exit of 75 means the load guard held the pilot back; it stays queued.
- [ ] `bad1aea8`: review the rework on `worktree-agent-bad1aea8-r2`, merge, cargo-ci on a quiet host, then code-review; research-review again if a record changed.
- [ ] Apply the invoker's rulings to `63bad95d`, `bfc2ceab`, `7dbe1393` and `aff59c3c`; each ruling's branch work is listed in `progress.json` under the issue's status.
- [ ] Before any of the pinned measurement branches merges: reword the 72-character subject `96d44eded` on `worktree-agent-63bad95d-pm4ri`; regenerate `f547c394/schema-versions.json` on the merged tree; merge order `worktree-agent-7dbe1393`, then `worktree-agent-63bad95d-m4ri`, then `worktree-agent-63bad95d-pm4ri`.
- [ ] File the follow-ups recorded in `progress.json` `surfaced_pitfalls` for session 33 (the `checkpoint_compat` resume tests that clear the completion flag; the stale `04b85d10` findings and launchers; the `permanent_rare_event_artifacts` load timeouts under `@/issue/f21b25ac`). Read `.jit/reference/content-standards.md` first and wire each to the issue that surfaced it.
- [ ] Reconcile the pitfall on `ed3d490e` REQ-09 (no production checkpointed campaign uses the batch decode stage) before the story's gates.
- [ ] Dispatch the next ready story issues as they unblock (`jit graph deps 1a379447 --depth 0`).
- [ ] Reclaim each pinned worktree when its issue closes (`LEAD_CACHE_DIRS=none`).

## Traps — do not repeat these

- **Do NOT trust a window's receipts without reading who else used the host.** Another epic's session ran through the 2026-10-09 window and spent a confirmatory attempt and five pilot trials. Read `window.log`'s load at each job start and `git log` over the window before dispatching collection work.
- **Do NOT evaluate code-review before cargo-ci.** The reviewer reads `cargo-ci=pending` as missing build evidence and fails the criterion (`e4e2ba52` round 1).
- **Do NOT start a cargo-ci gate while any worker builds, a rework worker included.** `feb60d0d` and `395fe4e2` each failed once on 15-second timeouts in `gf2-sim::permanent_rare_event_artifacts` at loads of 16 to 30; the same tree passed on a quiet host. The chain that worked: every merged issue's cargo-ci back to back with no worker running, then the reviews.
- **Do NOT send a merge or commit and the gate that depends on it in one tool block.** It happened twice this session; the gate record's commit had to be checked afterwards.
- **Do NOT commit with `git commit -a` on main.** It swept the tracker files of a gate run into a test commit; the commit was split before anything else landed.
- **Do NOT write `progress.json` with `ensure_ascii=True`.** The file is `indent=1`, `ensure_ascii=False`; the wrong flag rewrote every non-ASCII line.
- **Do NOT give a worker a criterion that needs route control without checking the type exposes it.** `a96a4cd6`'s first worker stopped because no `gf2-sim` stage could be put on a lane route.
- **Do NOT accept a test named for a route without an assertion of the route.** `a96a4cd6`'s sweep tests named lane routes and ran sum-product, which always takes the single-frame route.
- **Do NOT let a generator decide from Git history.** `feb60d0d`'s first rework computed "unaffected" from the paths commits touched; `@/invariant/behavioral-evidence-validity` failed it.
- **Do NOT arm the window timer in the same breath as other work.** The permission classifier denied the first `systemd-run`; it ran after the invoker said to try again. Ask early.
- **Do NOT read a duplicate worker report as news.** Each worker's report arrives twice, as a message and as its idle notice.
- All traps of [handoff-28](handoff-28.md) and earlier remain in force.

## Open questions needing invoker input

- Question: may the stages disturbed in the 2026-10-09 window be repeated once on a quiet host under new family versions, or do they stand as measured?
  - Context: `63bad95d` native ladder (confirmatory attempt spent), `63bad95d` dispatch overhead (both pilots spent), `63bad95d` native and portable M4RI (single pilots, `resolution-insufficient`), `7dbe1393` (single pilot, `resolution-insufficient`, all rows favour the candidate). The protocol's voided-attempt rule covers procedural defects before a result is read, which a contended host is not.
  - Options: (A) one repeat each under a new family version citing the recorded load; (B) stand as measured.
  - Recommendation: A; the disturbance came from another session of the same invoker and is recorded independently of the results.
- Question: how is `bfc2ceab` REQ-04 met?
  - Options: (A) the invoker records the exploratory check cost as accepted in `decisions.json`; (B) a new family for a clean confirmation; (C) amend REQ-04.
  - Recommendation: A, naming both disturbances.
- Question: does `matvec_parity_kernel` of `7dbe1393` stay in the tree unselected while its family is undecided?
  - Recommendation: hold the branch unmerged until the first question is answered.
- Question: should `bench-window/run-window.sh` apply a load guard to every job?
  - Context: the `8e53235b` queue line carries one; nothing else keeps other sessions off the host.
  - Recommendation: yes.
- Question: who repairs `jit validate`?
  - Context: `2b4d4a4ba` (`jit:b4b4b9ee`) changed the reviewer models in `.jit/gates.toml` without the `sim-research` package source; `repo-validate` gates the story and the epic.
  - Recommendation: epic `b4b4b9ee`, through `packages/sim-research`.
- Question (from session 32, still open): in what form may a workspace crate depend on `dev/tools/tuning-campaign-support`?
  - Options: (A) one `[workspace.dependencies]` entry at the root; (B) move the crate under `crates/`; (C) extend the named exception to owner-producer dev-dependencies.
  - Recommendation: A if the reviewer admits a root-relative path, otherwise C.
- Question: is the stage API added by `a96a4cd6` (`with_selectors`, `with_portable_lanes` on three `gf2-sim` stages) acceptable inside that issue?
  - Context: the story plan listed one touched test file; the lead decided it as a routine implementation choice.
- Question: `bad1aea8`'s INTRA crate carries its own AFF3CT handle beside the pinned `3be770d5` one. File it as a named exception to `@/invariant/convention-convergence` with a tracking issue?
  - Recommendation: yes; the `3be770d5` handle is digest-pinned.

## Reference artefacts

- Epic: `jit issue show 1a379447`; progress: `progress.json`; worker brief `worker-brief.md`; measurement contract `measurement-contract.md`.
- Reviews of this session: `reviews/e4e2ba52-r1.md`, `reviews/395fe4e2-r2.md`, `reviews/a96a4cd6-r2.md`, `reviews/feb60d0d-r3.md`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`.
- Disturbed families, on their branches: `63bad95d/disturbance-record.md`, `63bad95d/campaign-findings.md`, `63bad95d/campaigns/63bad95d-profile-dispatch-overhead-r2-freeze-refusal.txt` (`worktree-agent-63bad95d-fam`); `63bad95d/native-m4ri-pilot-resolution.md` (`worktree-agent-63bad95d-m4ri`); `63bad95d/portable-m4ri-pilot-resolution.md` (`worktree-agent-63bad95d-pm4ri`); `bfc2ceab/confirmation-freeze-refusal-spmm-check.json`, `bfc2ceab/decision-table.md` (`worktree-agent-bfc2ceab`); `7dbe1393/pilot-resolution.md`, `7dbe1393/survey/freeze-refusal.txt` (`worktree-agent-7dbe1393`).
- Word-wise assembly, on `worktree-agent-8e53235b`: `8e53235b/measurement-plan.md`, `8e53235b/addendum-v4-codeword-assembly-pilot.json`, `dev/bench_results/8e53235b/run-codeword-assembly.sh`.
- Story `ed3d490e`: `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`; arms `survey/arms/`, catalogue `survey/arms.json`, records `feb60d0d/` and `bad1aea8/` under that directory; tests `crates/gf2-sim/tests/batch_decode_worker_determinism.rs`, `crates/gf2-sim/tests/checkpoint_compat.rs`.
- Link tables the workers delivered for the unmerged branches, to apply with `jit doc add` after each merge: `pending-links/63bad95d-fam-links.tsv`, `pending-links/63bad95d-m4ri-links.tsv`, `pending-links/63bad95d-pm4ri-links.tsv`, `pending-links/bfc2ceab-links.tsv`, `pending-links/7dbe1393-links.tsv`, `pending-links/8e53235b-links.tsv`.
