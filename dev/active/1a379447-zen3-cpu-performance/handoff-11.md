# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 14

**Date:** 2026-09-13T18:05Z (draft), closing section 2026-09-13T18:45Z
**Session number:** 14
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md), [session 7](handoff-6.md), [session 9](handoff-7.md), [session 10](handoff-8.md), [session 11](handoff-9.md), [session 13](handoff-10.md). Session 12 wrote no handoff.

## Current state

- Epic: `1a379447`, backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 3 of 7 (07ca8585 from wave 4 runs alongside). `jit issue status` is authoritative.
- `1d4fd63d`: merged into main at 0bce899b (worker branch 8ca2b3f4 plus lead commits 49d838a7, 6cde1a6e, a531b17b); gates asm-artefact-present, research-review, code-review, doc-review passed; cargo-ci: see the closing section. Review [reviews/1d4fd63d-r1.md](reviews/1d4fd63d-r1.md); 68 artifacts linked (dfe5eacc).
- Running at draft time, native subagents `w14-<id>` (opus; a new session starts them fresh): `5cbb6545`, `53c5a8c0`, `07ca8585` (rework round 1, in the reused `.agents/worktrees/agent-1c602857` directory), `19513245` (new worktree). Invoker instruction 18:00Z: no new work this session; let them finish; write the handoff.
- Ready, not started: planning nodes `8d8be934` (story 2037941f) and `8ed3ac58` (story c04dd4ac); bugs `2c487595`, `706a8f93`; task `94bbe5d7`. Outside the epic: `8e53235b` (word-wise BCH codeword assembly; depends on 1d4fd63d).
- Active claims: `agent:worker` on the four running issues; `1d4fd63d` still assigned `agent:worker` (the lead's re-claim was refused; gates evaluated regardless).
- Open escalations: none blocking. One open question (non-blocking) below.
- Progress file: [progress.json](progress.json).

### Per-issue state

| Issue | Branch / worktree | State and next step |
|---|---|---|
| `1d4fd63d` | merged; worktree `.agents/worktrees/agent-1d4fd63d` | Close once cargo-ci passes (closing section); then `reclaim-worker-worktree.sh 1d4fd63d`. Candidate lane `avx2-ymm6` not selected under the frozen rule; `63bad95d` owns the selector. |
| `5cbb6545` | `worktree-agent-5cbb6545` in `.agents/worktrees/agent-5cbb6545` | Both confirmations committed (popcount: accepted, qualifies=false, w4 fail, w16384-vs-libpopcnt not-material; fused: accepted, qualifies=false, two cells inconclusive at the 1.04 margin). Worker retained the established count below the SIMD threshold (006e456d), re-recorded asm for the merged tree, was writing findings/tables, then cargo-ci. Next: worker report → lead review (all tiers) → merge `--no-ff` → gates → links from the scratchpad table (re-request from the branch if gone) → close. |
| `53c5a8c0` | `worktree-agent-53c5a8c0` in `.agents/worktrees/agent-53c5a8c0` | Both confirmations committed (qualifies=false each). Worker committed release asm with generators, tables generator and tables.md, one rustdoc fix in the batch module (measured short-batch boundary), and was running the 24-row profile session (full-host mutex per row), then findings and cargo-ci. Reported causes: poly-16w ~97x is real (no wide carry-less kernel above nine words; 16 words fall to the portable schoolbook); raw-batch-1024-composed ~0.08 is the declared composed-cost artefact (gf2x has no batch entry point). Next as for 5cbb6545. Its freezer extension (`--holdout-cells`, `--purpose`) merges before 07ca8585's (`--worthwhile-speedup`). |
| `07ca8585` | `worktree-agent-07ca8585` in `.agents/worktrees/agent-1c602857` | Rework round 1 against [reviews/07ca8585-r1.md](reviews/07ca8585-r1.md). Worker built a standalone arms workspace (`dev/active/07ca8585/survey/arms/`, own target dir) so the pinned after arm stays byte-identical; isolated check-node arms (gf2 `EdgeLayout` + `min_sum_check_row`; AFF3CT `Update_rule_NMS` via a new `a3u_*` shim) with a committed bit-identity parity receipt; fixed-iteration cells against a committed untimed corpus `preparation/quality-fixed/`; pilots and frozen confirmations of both granularities (224387e7); the profile re-sampling series gets a launcher and a queue line; GPU byte-identity tests after the timed work. Next: worker report → re-review (Tier 1.5 against r1) → the four earlier queue lines plus the new ones run in a benchmark window → adoption decided by the confirmations → merge, gates, links, close. |
| `19513245` | `worktree-agent-19513245` in `.agents/worktrees/agent-19513245` | Worker committed prototype and arm scaffold, frozen pilot addenda, launcher and pre-timing evidence; six campaigns (three families, pilot then confirmation, ~15 min) were running in-session. Next as for 5cbb6545. |
| `2037941f`, `c04dd4ac` | no worktree | Brackets scaffolded (planning `8d8be934`/`8ed3ac58`, breakdown `30c3aef1`/`16560579`). Next: jit-planning-lead flow per story (investigator → manifest-first synthesizer → adversarial reviewer → plan-review gate), then jit-breakdown, coverage-preview and breakdown-review. Plan dirs resolve to `dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations` and `dev/active/c04dd4ac-zen3-shifts-and-permutations` (not yet created). c04dd4ac's leaves need the eda07788 AFF3CT arms rebuilt first (session-11 trap). |

## What just happened

- Resumed after session 13's usage-limit death (13:26Z); only 07ca8585 had reported. Recovered the four session-13 dispatch prompts from its transcript; committed the pending 07ca8585 links table (f753f8db).
- 5cbb6545's popcount confirmation had completed after its worker died and sat uncommitted; the continuation worker verified and committed it (c7daaaba).
- Dispatched four opus workers in reused worktrees (5cbb6545, 53c5a8c0 continuations; 07ca8585 rework round 1; 19513245 fresh via the dispatch script). Prompts were committed under `prompts/session-14/` and removed again on the invoker's ruling (65a1aecd): prompts are ephemeral.
- 1d4fd63d lead review PASS (Tier 2 through 3); lead named owners for its four follow-ups (49d838a7), created task `8e53235b` outside the epic, merged at 0bce899b. research-review failed at tier 1 (cites labels absent from the issue text); invoker chose the 3be770d5 repair for all ten affected issues (576ab16c). doc-review F1 (dispatch rustdoc said this issue's receipt put the incumbent first) fixed in 6cde1a6e; F2 (report unlinked) fixed by linking 68 artifacts (dfe5eacc); second-round doc-review F1 (BitMatrix::transpose rustdoc described the PSHUFB lane as inspection-only) and code-review F1 (run-validation.sh bypassed `cargo-budget.sh --test`) fixed in a531b17b with the record regenerated; all three reviews then passed. cargo-ci gate timed out at 900 s under host contention (worker builds); re-run after a manual `./scripts/cargo-ci.sh` warm-up.
- Verified for the invoker that no worker delegates to codex: the four subagent transcripts contain no codex tool call; the only codex processes are the invoker's own five-day-old session.

## What to do next

- [ ] Close `1d4fd63d` if cargo-ci passed (closing section); otherwise re-run `jit gate evaluate 1d4fd63d cargo-ci` on a quiet host right after a manual `./scripts/cargo-ci.sh`. Commit state; reclaim `agent-1d4fd63d`.
- [ ] Collect the four workers' branches: review each per `lead-review-protocol.md` (07ca8585 with Tier 1.5 against r1), merge one at a time (`--no-ff`; 53c5a8c0 before 07ca8585 for the freezer), run reviews then cargo-ci, link artifacts from each worker's scratchpad table (`/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/22859b91-e3f8-4d1b-90d5-b1bdb621c2bc/scratchpad/<id>-links.tsv`; re-request from the branch if gone), close, reclaim.
- [ ] File the two follow-ups the 53c5a8c0 worker reported (not filed this session on the invoker's no-new-work instruction): (1) no wide carry-less kernel above nine words, so `clmul_wide` at every larger width falls to the portable bit-serial schoolbook (the whole of the poly-16w result); (2) `field_vec.dot_chunk_len` is never calibrated against short vectors and the dispatched dot product zeroes 8 KiB of chunk scratch per call (the memset share in its profile). The 5cbb6545 and 19513245 reports may name more.
- [ ] Queue the 07ca8585 campaigns (four lines on its branch plus the profile re-sampling line) into `bench-window/queue.tsv` on main, check each launcher exports `~/.cargo/bin`, arm a window (invoker chooses the time; precedent 04:00 Europe/Helsinki; `systemd-run --user` needs `XDG_RUNTIME_DIR=/run/user/1000` and `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`), collect from each campaign's own log.
- [ ] Then the two planning nodes (jit-planning-lead flow), then `94bbe5d7` (freezer prose; after both freezer extensions land), `2c487595`, `706a8f93` (sonnet-sized).
- [ ] Rebuild the eda07788 arms before c04dd4ac's leaves dispatch.

## Traps — do not repeat these

- **Do NOT commit dispatch prompts.** Invoker ruling this session: prompts are ephemeral; keep copies in the session scratchpad only. The session-13 suggestion to commit them beside `worker-brief.md` is void.
- **Do NOT run tracker writes concurrently with a gate evaluation.** A `jit issue update` batch running while `jit gate evaluate 1d4fd63d code-review` was in flight produced "Failed to restore recovery serialization after external process" and no run record; the gate had to be re-run. Serialize: links and updates first, then gates.
- **Do NOT expect the cargo-ci gate to fit its 900 s runner timeout while four workers build.** It timed out with no output at a531b17b. Warm the caches with a manual `./scripts/cargo-ci.sh` (no timeout) and evaluate the gate immediately after, or wait for a quiet host.
- **Do NOT dispatch a research-review-gated issue whose `cites:` labels its text lacks** (third occurrence). Every open epic issue now carries `Source references: [Key].`; a newly created issue needs it too. The gate fails in 133 ms, before any reviewer runs.
- **Do NOT write a bare `echo ====` or `$B:path` in this zsh.** A bare `====` word is looked up as a command and aborts the rest of a `&&` chain; `$B:crates/...` is parsed as a history modifier. Write `echo '===='` and `"${B}:crates/..."`.
- **Do NOT gate a validation runner that calls `cargo-budget.sh cargo test` without `--test`.** code-review rejects the record as produced through an unserialized path (`@/invariant/test-tier-budgets`); regenerate the record through the corrected runner.
- **Do NOT let a report name "the invoker" as the owner of an open decision.** Reviewers read it as a deferred item without an owning issue; name the issue (63bad95d for selector calibration here) and raise the question in the handoff.
- **Do NOT match a process by its full command line in a wait or check, and do not quote such a pattern in a heredoc.** The harness rejects the whole command, even when the pattern only appears in text being written. Match the executable name (`ps -eo etimes,comm,args`) or poll a file or commit.
- Unresolved traps from [session 13](handoff-10.md#traps--do-not-repeat-these) and earlier remain in force (jit profile projections, partial-feature clippy failures are not a broken main, `--family-description`, stories are not tasks, no `cd` into worktrees, `/bin/ls` not `ls -t`, zsh `path` loop variable, `codex exec < /dev/null`, comparator builds under `.agents/ext/<issue>` of the checkout that built them, window launchers export `~/.cargo/bin`, `main...<branch>` for branch files, worker reports under 3500 characters).

## Open questions needing invoker input

- Question: adopt the `avx2-ymm6` transpose lane despite the non-qualifying receipt, or leave it to 63bad95d's selector calibration?
  - Context: three kernel cells pass materially, all others not-worse, ahead of M4RI and Bitshuffle; two consumer cells frozen as improvement record not-material; no further confirmatory attempt exists for the family.
  - Options: (a) leave the published lane unchanged, 63bad95d calibrates; (b) amend the contract's qualification rule; (c) adopt on the evidence as recorded.
  - Recommendation: (a).
- Question: when to run the next benchmark window for 07ca8585's confirmations and profile re-sampling (about 85 minutes queued once its rework lands).

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [worker brief](worker-brief.md); [progress](progress.json); reviews under [reviews/](reviews/) (`1d4fd63d-r1.md`, `07ca8585-r1.md` this session); pending link tables under [pending-links/](pending-links/).
- Protocol v4: `dev/active/f547c394/{protocol.md,amendment-v4.md,addendum.schema.json}`; canonical freezer `dev/active/c7113c5a/survey/freeze-confirmation.py` (two unmerged extensions on the 53c5a8c0 and 07ca8585 branches).
- Window: [run-window.sh](bench-window/run-window.sh), [queue.tsv](bench-window/queue.tsv), [follow-window.sh](bench-window/follow-window.sh).
- Story brackets: planning `8d8be934` (2037941f), `8ed3ac58` (c04dd4ac); jit-planning-lead references under `.agents/skills/jit-planning-lead/references/`.
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`; leak baseline `/tmp/lead-pre-dispatch-latest.txt` (session 14, taken before the 19513245 dispatch).
- Gate records: `.jit/gate-runs/`.

## Closing section

- **Invoker rulings after the draft (18:15Z onward):** the host must not be locked during an active session; all measurement runs in an overnight slot; locking is disabled; every running measurement was killed; the workers must stop right away and terminate gracefully; the overnight run is armed.
- **Lock discipline (07fb19ee on main):** `scripts/cargo-budget.sh` runs its command unlocked and unbudgeted unless `GF2_BENCH_WINDOW=1`; `dev/scripts/ccx1-bench-flock.sh` refuses (exit 3) outside the window; `bench-window/run-window.sh` exports the flag; AGENTS.md and worker-brief.md state the rule; the ten-minute in-session allowance is withdrawn. Worker worktrees that have not merged main still carry the old wrappers; the window exports the flag, so they run there unchanged.
- **Killed:** the 53c5a8c0 profile session (10 of 24 rows recorded, resumable per row) and the 19513245 vector confirmation (twice; its checkpoints persist and `window` resumes it). Its control confirmation had completed (receipt uncommitted in the worktree at kill time; the worker was told to commit it).
- **Window armed:** transient user timer `gf2-bench-window-20260914`, 2026-09-14 04:00 Europe/Helsinki (01:00Z), `--setenv=PATH=~/.cargo/bin:...`, working directory the primary checkout. Queue on main (`bench-window/queue.tsv`, 18 lines: 8 collected earlier and skipped by their `.done` keys, 10 new): 07ca8585 single-worker and comparator single-worker confirmations, multicore and comparator multicore pilots, check-node and fixed-iteration confirmations, profile re-sampling series; 19513245 vector and matrix confirmations; 53c5a8c0 profile remainder. About 155 estimated minutes. **First thing next session: collect it** with `bench-window/follow-window.sh` (`GF2_WINDOW_UNIT=gf2-bench-window-20260914`), judging each campaign from its own execution log and receipt, then freeze/summarize per issue (07ca8585's confirmations were frozen by the worker; 19513245's were frozen from committed pilots; the profile needs `summarize-profile.py` on the worker branch).
- **1d4fd63d cargo-ci:** failed at 89735528 on the known contention flake (`accepted_complete_experiment_measures_defaults_and_reopens_only_its_27_leaves`, TIMEOUT 15 s under load 11); the manual `./scripts/cargo-ci.sh` at the same tree passed every other step. Re-run the gate on a quiet host (unlocked now), then close and reclaim.
- **Workers:** told at 18:40Z to commit everything (wip commits allowed), write their links tables, report under 2000 characters and halt. Their final reports, if received before the session ended, are appended below; otherwise the branches and worktrees hold the state and `git status` in each worktree shows what was left uncommitted.
