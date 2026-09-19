# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 22

**Date:** 2026-09-19T15:55Z
**Session number:** 22
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-16.md` in this directory (sessions 16 to 20 left no handoff; their record is `progress.json` `notes`).

## Current state

- Epic: `1a379447` — state: backlog (claimed by `agent:jit-execution-lead`; containers stay backlog until their subtree is terminal).
- Wave in progress: 3 of 7 in `progress.json` `waves`, run as a rolling wave of at most four workers.
- Children summary (label `epic:zen3-cpu-performance`): 43 done, 6 in_progress, 18 backlog/ready, 2 rejected.
- Active claims (`agent:worker` unless noted): `ad2a6a58`, `4c1e441f`, `f63a2464` (confirmations queued for the window, trees pinned); `9fb40c83` (`agent:work-9fb40c83`, gates owed, invoker decision pending); `e1f9a78f` (gates owed, next round not dispatched).
- Open escalations: one, `9fb40c83` research-review F3 (see Open questions).
- Progress file: `progress.json` in this directory reflects the above.
- Benchmark window `gf2-bench-window-20260920` is armed for 2026-09-20 04:00 Europe/Helsinki (01:00Z). Its live lines: `f63a2464` ×3 confirmations (`agent-9fb40c83`, 54 min), `ad2a6a58` confirmation (`agent-0a357f94`, 8 min), `4c1e441f` confirmation (`agent-c04dd4ac`, 6 min). Every 2026-09-19 job carries a done marker. The three `e1f9a78f` dense lines stay commented out.
- Worktrees: `agent-0a357f94`, `agent-c04dd4ac`, `agent-9fb40c83` (pinned on the pre-rewrite history on purpose, each with post-window commits the lead has already replayed onto main); `agent-b64dc9c4` (18a87159, rebased onto main, merged; reclaim when 18a87159 closes); `agent-e1f9a78f` and `agent-9fb40c83-r3` (reset to main, free); the foreign `agent-02b8137c-run`. `agent-85fc5ff4` reclaimed.
- No worker is running. Main is clean; the leak check is clean.

## What just happened

- Window of 2026-09-19 collected: nine jobs, eight campaigns verified complete from their own execution logs (18a87159 17/17/6 cells plus a nine-repetition profile; f63a2464 2/3/2; ad2a6a58 10; 4c1e441f 8), every pilot receipt accepted. `ad2a6a58`'s rc=1 was its launcher's private terminal check; the lead marked the job done after verifying the log.
- `9fb40c83` gates on c011e69a2: cargo-ci PASS, code-review PASS (zero findings), research-review FAIL round 4 on F3 only (F1, F2 closed). Rework count 3: invoker decision required.
- `d45aff82` DONE: rework 2 (staged smoke mode on the runner's own session loop, `SessionDispatch` enum, pause/resume/refusal test) merged at b556c449b; cargo-ci, code-review, doc-review pass with zero findings; review `reviews/d45aff82-r2.md`. Unblocks `e1f9a78f`, `00dd43c3`, `9fb40c83` closure and the three measured issues' dependency edges.
- `4c1e441f`: pilot merged (57ded10db), confirmation frozen (6 of 8 cells) and queued; 13 artifacts linked. `ad2a6a58`: pilot merged (e736999ee), confirmation frozen (6 of 10 cells; equivalence margin 1.15 -> 1.3 by the freezer's sanctioned option, cold cells set the resolution 0.29) and queued; 18 artifacts linked. `f63a2464`: three QC pilots merged (3c6c8274c); Q and L stop on S2, QC enters its three confirmations, queued; 33 artifacts linked. All three trees stay pinned; commits were replayed onto the rewritten main through scratch `replay-<id>` branches (`git rebase --onto main <pinned tip>`), main's side taken for files main had already converged (ad2a6a58 launcher, closure, smoke script).
- `18a87159`: post-window round merged (da6ca8163, 3e70bba29): three baseline records in `logical-baselines.md`, interval tables, profile summary, confirmatory budget for bc091474 (six cells per family), REQ-09 negative. Gates: cargo-ci PASS; code-review PASS (zero findings), research-review FAIL round 1 on F1 (the three receipts and the profile omit their exact executed top-level invocations: the `window` command, the runner and `perf` invocations; REQ-01, REQ-04, REQ-07, @/inv/claims-trace-to-artifacts). Rework 1 owed; verdict in `reviews/18a87159-r1.md`.
- Bug `5ac79460` filed and wired ahead of `bc091474`: the shared `verify-campaign-log.py` resolves an omitted exploratory `pilot_pairs` to `confirmatory_pairs` against the protocol's `pilot_min_pairs` rule (the logical harness writes `pilot_pairs: null`). Not dispatched (invoker: no new work this session).
- Schema index `dev/active/f547c394/schema-versions.json` regenerated twice on main (65, then with the logical pilots) because each receipt-bearing merge stales it.
- Stopped a broken sccache server (TMPDIR inside a deleted temp dir, zero cache hits); the window runner replaces such servers itself.
- Messaged the just-in-time session (`just-in-time-4f`) with the lock-contention report and a fix request (shorter critical section, bounded wait instead of the 5 s timeout, re-acquire on verdict recording).
- Five opus dispatches, no rework rounds, no stalls.

## What to do next

- [ ] Collect the 2026-09-20 window: for each of the five lines read `.agents/bench-window/window.log` and the campaign's own `execution.log` and acceptance summary, never the job rc. Then the final round per A/B issue in its pinned worktree: outcome prose (REQ-06/REQ-08 verdict per cell, direction agreement with the pinned prior receipt), converge the private smoke on `benchmark-ab-runner smoke` over both plans (fix `dev/scripts/smoke-campaign-arms.sh`'s unconditional `--label pilot` at the source; add the finding count to `ad2a6a58` tables' Source line), then `git rebase --onto main <pinned tip>` in the worktree (the tips are 711e4fa7d, 58bcfe84b, 504f14167 plus this session's replayed commits; resolve toward main's shared forms), cargo-ci, gates cargo-ci, code-review, research-review. `f63a2464` also applies S4 per ledger and converges `qc-arm-smoke` and `ldpc-plan-check`.
- [ ] `9fb40c83`: resolve the open question below, then either dispatch round 4 (launcher collects counters and hot symbols in every repetition; v4-r3 profile session as a window line; per-repetition order-statistic intervals) or record the invoker's decision on the issue and re-run research-review. It closes only after that; `d45aff82` is done so no dependency remains.
- [ ] `e1f9a78f`: dispatch its round in `agent-e1f9a78f` (reset to main): dense launcher's smoke uses `benchmark-ab-runner smoke <plan> --stage <dir>` for REQ-03's logging and checkpoint/resume clause, re-point the `dense-parity-conformance.md` rows that cite the runner's resume test, then gates cargo-ci, code-review, doc-review (Tier 1.5 table: six prior code-review rounds, findings listed in `progress.json` `notes` and the gate runs). On PASS close it and dispatch `c73ffa25`, which re-keys the three held dense queue lines.
- [ ] `00dd43c3`: dispatch (opus) in a fresh pinned worktree created by the dispatch script; it starts on the shared smoke and derives the residual arm's label from `residual_shift_route()` (`crates/gf2-core/benches/shifts.rs` hard-codes `bitvec-residual-scalar-*`).
- [ ] `5ac79460` (sonnet, `agent-9fb40c83-r3`): fix the checker's pair rule, tests, keep every committed campaign passing; it gates `bc091474`.
- [ ] `18a87159` rework 1 of 2 (opus, `agent-b64dc9c4`, branch already on main): close research-review F1 by recording each campaign's exact executed invocation beside its receipt (the launcher's `window` command line as run by the window unit, from `.agents/bench-window/window.log` and the launcher's own log, plus the runner and `perf` invocations of the profile session) in a committed, regenerable record the harness writes at run time for future runs and that the baseline record cites; receipts and evidence bytes stay immutable. Then re-run research-review only (cargo-ci and code-review passed on the merged tree; re-run them if Rust or tool code changes), Tier review, close, reclaim the worktree (`LEAD_CACHE_DIRS=none`).
- [ ] Reclaim `agent-9fb40c83-r3` when `9fb40c83` closes.

## Traps — do not repeat these

- **Do NOT merge a pinned worker branch into main, and do NOT rebase it in place while a confirmation is queued.** Replay it: `git branch -f replay-<id> <branch>`, a scratch worktree, `git rebase --onto main <pinned tip>`, merge the replay branch, delete it. The pinned worktree keeps its tree for the window (`scratchpad/replay-pinned.sh` of this session did this; the script is not in the repository).
- **Do NOT expect a pinned tree's launcher, closure or smoke script to match main's.** Main converged the axpy family on shared generators (2d95c4d4f) after the pin; the worker re-did that work and its commit conflicted. Resolve toward main's form and let the pinned tree keep its own; tell the next worker to read main's copy with `git show main:<path>`.
- **Do NOT run the merged-tree cargo-ci while workers run their own suites.** A load-guarded background start passed the guard and then hit load 40 from two worker suites; four tests timed out at the 15 s cap. Re-run when no worker builds; the run's exit code is not evidence otherwise.
- **Do NOT merge a receipt-bearing branch without regenerating the schema index.** `dev/active/f547c394/schema-versions.json` counts pinning receipts and cargo-ci's `addendum-schema-versions` step reads HEAD; regenerate with `check-addendum-schema-versions.py --write`, commit, re-check. A worker on a pinned tree bumps it too; drop that bump at replay.
- **Do NOT trust an sccache server started under a gate or a temporary directory.** It keeps `TMPDIR=/tmp/tmp.*/scratch` after the directory is gone and every cold compile fails; `sccache --stop-server`, then let the next cargo run spawn one.
- **Do NOT reclaim a reused worktree by its directory name alone.** The reclaim script keys the branch on the directory (`worktree-agent-85fc5ff4`) while the checked-out branch was `worktree-agent-d45aff82`; check `git -C <wt> rev-parse --abbrev-ref HEAD` and `git rev-list --count main..<that branch>` yourself before `LEAD_RECLAIM_FORCE=1`.
- **Do NOT use the shared `verify-campaign-log.py` on a plan with null `pilot_pairs` until 5ac79460 lands.** It resolves the omission to the confirmatory count and rejects a correct exploratory campaign.
- **Do NOT chain `timeout N jit gate evaluate ...` with an env prefix in one command.** The auto-mode classifier refused that shape; the plain `jit gate evaluate <id> <gate>` ran.
- Traps of `handoff-16.md` and its predecessors remain in force (history-rewrite tag, no un-rebased merges of old-history branches, `.done` markers, `jit doc check-links` after renames, gate staggering, zsh word splitting, no process-pattern kills, load above 8).

## Open questions needing invoker input

- Question: `9fb40c83` research-review round 4 keeps F3 (retained-session hardware-counter and hot-symbol tables carry n = 1 and no interval; only the first profile repetition collected counters). Rework count is 3, past the limit (session-21 ruling 4). Which way?
  - Context: `scratchpad` draft `escalation-9fb40c83.md` of this session, reproduced in `progress.json` `escalations`.
  - Options: (1) authorize round 4: launcher collects counters and hot symbols in every repetition, a v4-r3 profile session as a window line, per-repetition order-statistic intervals; (2) record an overrule decision on the issue (the tables are descriptive, n = 1 labelled) and re-run the gate; (3) drop the two tables and cite the raw perf files, at the cost of REQ-03's attribution.
  - Recommendation: option 1; it is the only route that keeps REQ-03 and satisfies @/inv/uncertainty-reported literally, and the window mechanism is armed nightly.
- Question: `f63a2464`'s decision record caps "confirmatory campaigns" at one per family, and QC has three ledgers; the worker queued three confirmations under the ledger reading (recorded in findings.md Limits). Confirm the reading before the window, or hold the three lines by commenting them out.
  - Recommendation: confirm the ledger reading; it is the arithmetic the frozen record performs and the one the pilots already spent.

## Reference artefacts

- Epic: `jit issue show 1a379447`; measurement contract `measurement-contract.md`; worker brief `worker-brief.md`.
- Reviews this session: `reviews/d45aff82-r2.md`.
- Shared smoke: `dev/tools/tuning-campaign-support/src/arm.rs` (`smoke`, `validate_cell`), `src/bin/benchmark-ab-runner.rs` (`SessionDispatch`, `smoke --stage`), `tests/protocol_contracts.rs`.
- A/B families: `dev/active/ad2a6a58/` (confirmation addendum, `confirmation-derivation-axpy.txt`), `dev/active/4c1e441f/` (`confirmation-derivation-dense-product.txt`), `dev/active/f63a2464/` (`findings.md`, `freeze-confirmations.sh`, three confirmation addenda).
- Logical baselines: `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/logical-baselines.md`, `dev/bench_results/2037941f/logical-tables.md`, `logical-profile/profile-summary.md`.
- DVB profile (9fb40c83): `dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-interleave-profile-tables.md:114` (F3 tables), `dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-dynamic-profile/`.
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`, state `.agents/bench-window/`, unit `gf2-bench-window-20260920`.
- History rewrite: `subject-rewrite-sha-map.tsv`, tag `archive/main-before-subject-rewrite`.
