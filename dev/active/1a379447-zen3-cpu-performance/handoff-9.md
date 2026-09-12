# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 11

**Date:** 2026-09-12T18:15Z
**Session number:** 11
**Prior handoffs:** [session 1](handoff.md), [session 2](handoff-2.md), [session 3](handoff-3.md), [session 4](handoff-4.md), [session 6 halt](handoff-5.md), [session 7](handoff-6.md), [session 9](handoff-7.md), [session 10](handoff-8.md)

## Current state

- Epic: `1a379447`, backlog (dependency-blocked container; assignee `agent:jit-execution-lead`).
- Wave in progress: wave 2 of 7 (wave-3 issues `1c602857` and `12fdeb5b` run alongside because their deps are done).
- Children: `jit issue status` is authoritative. Closed this session: `3be770d5`, `6fb89a3c`. In progress: `04b85d10`, `6c6b09b1`, `1c602857`, `12fdeb5b` (all waiting on the overnight window), `a203a23c` (gates), `428f2f6b` (worker running at handoff time). Ready, not started: `53c5a8c0`, `c04dd4ac`, `07ca8585`, and the new bugs `2c487595`, `706a8f93`.
- Active claims: no leases. Assignments: lead on `04b85d10`, `a203a23c`; `agent:worker` on `6c6b09b1`, `1c602857`, `12fdeb5b`, `428f2f6b`. Every session-11 worker was a native subagent named `w11-<id>`; a new session starts fresh.
- Open escalations: none. One invoker decision this session: append `[Cassagne2019]` to `3be770d5`'s Notes (`c034bbd8`).
- **The overnight window is armed.** Transient user timer `gf2-bench-window-20260913` runs [run-window.sh](bench-window/run-window.sh) at 2026-09-13 04:00 Europe/Helsinki (01:00Z) over the committed [queue.tsv](bench-window/queue.tsv): seven jobs, about 71 minutes. Every job was pre-flighted (worktree present, launcher executable, cargo on PATH, no `.done` collision). Follow with `GF2_WINDOW_UNIT=gf2-bench-window-20260913 bash dev/active/1a379447-zen3-cpu-performance/bench-window/follow-window.sh`; state under `.agents/bench-window/`. **First thing next session: collect it** the way session 10 prescribed: per campaign, its own execution log (`cell-start` = `cell-complete` = the addendum's cell count, declared pairs per cell, terminal `complete`) and its receipt's evaluator verdict, never the job rc.
- Progress file: [progress.json](progress.json).

### Per-issue state

| Issue | Branch / worktree | State and next step |
|---|---|---|
| `3be770d5` | **done**; worktree reclaimed | Merged at `6055e0c7` and `5ac7d50c`; 56 artifacts linked; four gates pass. |
| `6fb89a3c` | **done**; worktree reclaimed | Freeze-script double-count fixed (`fad06995`, merged `28f96566`); four gates pass. |
| `a203a23c` | `worktree-agent-a203a23c` at `e00195fc` in `.agents/worktrees/agent-a203a23c`; merged at `b6fa4719`, links at `f9640aaa` | doc-review passed. **cargo-ci failed on the five `gf2-sim::permanent_rare_event_artifacts` 15 s timeouts under host contention** (three codex reviews and a worker build were running); re-run it on a quiet host. code-review and research-review were running at handoff time: read `.jit/gate-runs`. If all pass, close and reclaim. |
| `428f2f6b` | `worktree-agent-428f2f6b` in `.agents/worktrees/agent-26465e6c-v3` (reused directory) | Worker `w11-428f2f6b` was still running at handoff (head `ff4409ea` or later). Next: read its branch, lead-review (inventories, generators reproducing byte for byte, corrections.md), merge, gates. If uncommitted work remains, lead-preserve it on the branch first. |
| `04b85d10` | `worktree-agent-04b85d10-v3` at `8c089201` in `.agents/worktrees/agent-04b85d10-v3` | Rework round 2 delivered (resolution table in the session-11 transcript and summarized in progress.json): mislabeled layout/count v3 cells preserved as falsified in `dev/bench_results/04b85d10/receipt-evidence-record.json`; the harness builds the declared codes with a guard; `summarize-receipts.py` projects prose from the record; v4 count and layout pilots frozen and **queued overnight**. After the pilots: freeze the confirmations from the committed pilot receipts (second attempt alpha 0.05/6 shared with the v3 cells; untested), second window, then lead review and gates. Rework counter is 2 of 2. **Not merged.** |
| `6c6b09b1` | `worktree-agent-6c6b09b1-v3` at `6e907460` in `.agents/worktrees/agent-6c6b09b1` | Three v4 pilots committed and verified; three v4 confirmations frozen at six cells each (P-20) and **queued overnight**; smoke passed. After the window: commit the receipts **together with the three modified family ledgers**, complete findings, merge, gates. Expect `fail` outcomes in the pairwise confirmation (comparator-gap inversion, see Traps). **Not merged.** |
| `1c602857` | `worktree-agent-1c602857` at `3c35eee2` in `.agents/worktrees/agent-1c602857` | Launcher exports cargo on PATH, `build` mode added, smoke reached all six cells, main merged in. Pilot **queued overnight** (5 min floor). Then freeze the confirmation from the committed pilot receipt, second window, review, gates. **Not merged.** |
| `12fdeb5b` | `worktree-agent-12fdeb5b` at `208e8783` in `.worktrees-local/agent-eda07788-v3` (reused directory) | Survey delivered: AFF3CT and srsRAN pinned, 39 matched arms over 24 configurations bit-exact, pilot frozen, smoke accepted. Pilot **queued overnight** (20 min). Then freeze the confirmation (at most six cells), second window, review, gates. Artifact table in [pending-links/12fdeb5b-links.tsv](pending-links/12fdeb5b-links.tsv). **Not merged.** |

## What just happened

- Collected the 2026-09-12 08:36Z window from each campaign's own log: 6c6b09b1's three v4 pilots (17/14/9 cells) and 3be770d5's two v4 confirmations (2/6 cells) complete and Accepted; 3be770d5's nine-session profile ran but its summary failed on a one-sample listing mismatch; 1c602857's pilot died in 1 s (`ionice: failed to execute cargo`, no `~/.cargo/bin` on the unit's PATH).
- Dispatched, four at a time, native subagents by difficulty (opus for surveys and the measurement-validity rework, sonnet for the bounded fixes): 3be770d5, 6c6b09b1, 04b85d10 (rework 2), a203a23c (rework 1), then 1c602857, 428f2f6b, 12fdeb5b, 6fb89a3c.
- 3be770d5: the profile mismatch was a parser defect (perf's `[unknown] ([unknown])` lines dropped while `--stats` counted them; now an `unmapped-ip` category with the exact-total rule kept and a self-test). The independent Terra xhigh review failed on one finding (the lever table's `1/(1-s_lo)` presented as a realized "speedup at least"); the rework labelled it an Amdahl ceiling and made the refutation rule share-based. The review gates then failed at tier 1 on the `cites:Cassagne2019` label without a text mention; the invoker approved appending the key. Closed.
- 6fb89a3c: ported the freeze-script fix from 3e59cb9a; all three frozen addenda reproduce byte for byte; closed.
- Filed: `2c487595` (version the addendum schema alongside predecessors), `706a8f93` (record achieved scheduling priority in receipts; resolves the session-10 open question with option A because `surfaced_pitfalls` names epic REQ-01/06), `94ab019f` (tuning_conservative_cfg.rs feature gate, outside the epic), and `f69c9b87` in the just-in-time tracker (asset scanner reads demangled Rust symbols as paths). The just-in-time session fixed and reinstalled that one within the hour; the previously unlinkable `popcnt-attribution.txt` is now linked to 04b85d10.
- Pruned the pre-epic worktrees `baseline-check` and `cargo-budget`; reclaimed `agent-3be770d5` and `agent-6fb89a3c-v3`; reused `agent-26465e6c-v3` for 428f2f6b and `.worktrees-local/agent-eda07788-v3` for 12fdeb5b. Worktrees: 8, disk about 60 GB free, cache pool about 170 GB.
- Wrote and armed the overnight queue; fixed the 04b85d10 lines with an `env PATH=` prefix after pre-flight showed its launcher does not export cargo.

## What to do next

- [ ] **Collect the window first** (see Current state). Then, per issue: 04b85d10, 12fdeb5b and 1c602857 freeze their confirmations from the committed pilot receipts and queue a second window; 6c6b09b1 commits receipts plus ledgers and completes findings.
- [ ] Finish `a203a23c`: re-run cargo-ci on a quiet host (`JIT_AGENT_ID=agent:jit-execution-lead CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host jit gate evaluate a203a23c cargo-ci < /dev/null`), read the code-review and research-review results, close if green, reclaim `agent-a203a23c`.
- [ ] Review `428f2f6b`'s branch; merge and gate.
- [ ] Link the pending artifact tables under [pending-links/](pending-links/) when each issue closes; 04b85d10 also relabels `tables.md` and `run-consumer-campaigns.sh` (labels in its table).
- [ ] Rebuild the eda07788 arms (`dev/active/eda07788/survey/nr-derate-build.sh` against `.agents/ext/c077a88b/aff3ct`) before `c04dd4ac` dispatches; the derived build was deleted (Traps).
- [ ] Ready and unstarted: `53c5a8c0`, `c04dd4ac`, `07ca8585` (unblocked by 3be770d5), bugs `2c487595` and `706a8f93`.
- [ ] Pass `GF2_WINDOW_UNIT` to `follow-window.sh` or update its default (it still names the 20260912b unit).

## Traps — do not repeat these

- **Do NOT name a shell loop variable `path` in zsh.** It aliases `PATH`; a `while read -r path ...` loop made every command inside "not found" and silently linked nothing. Use `dpath` or similar.
- **Do NOT run `codex exec` in the background without `< /dev/null`.** It prints "Reading additional input from stdin..." and waits forever; the first 3be770d5 review sat idle until killed. Every gate and review launch this session carries `< /dev/null`.
- **Do NOT kill a process by a full-command-line pattern that also matches your own shell.** The kill took the launching shell with it (exit 144), and the harness now blocks such loops outright. Use the background task id, or match the executable name and age.
- **Do NOT tell a worker that external comparator builds live under the primary checkout.** They live git-ignored under `.agents/ext/<issue>` of whichever checkout built them; eda07788's was inside its worktree. The 12fdeb5b worker, told otherwise, removed `.agents` in its worktree and destroyed the eda07788 AFF3CT arm build (rebuildable; source and library under `.agents/ext/c077a88b` intact). Brief workers to look inside a directory before removing it.
- **Do NOT queue a launcher for the systemd window without checking it exports `~/.cargo/bin`.** The unit's PATH lacks it. 1c602857's pilot died on it in the 08:36Z window; pre-flight found 04b85d10's launcher has the same gap, fixed on the queue line with `env PATH=/home/vkaskivuo/.cargo/bin:/usr/local/bin:/usr/bin`. Check `grep -n 'cargo/bin' <launcher>` for every queued job.
- **Do NOT run cargo-ci gates while three codex reviewers and a worker build share the host.** a203a23c's run lost the five `gf2-sim::permanent_rare_event_artifacts` tests to the 15 s cap; nothing in its diff touches Rust. Stagger: reviews first, cargo-ci when the host is quiet, or budget one re-run.
- **Do NOT dispatch a research-review-gated issue whose `cites:` labels its text does not mention** (session-1 trap, hit again on 3be770d5). Check `jit issue show <id>` labels against the description before the first gate; the repair is an invoker decision.
- **Do NOT present `1/(1-share)` as a predicted speedup.** It is an Amdahl ceiling and must be labelled an estimate; refute an attribution by the post-change share, not by the clock. Brief profiling workers up front.
- **Do NOT expect a subagent to notice its report was dropped or truncated.** Three workers' first reports were dropped (over about 6000 characters) or truncated; ask for resends in pieces under 3500 characters, and tell workers up front to split.
- **Do NOT read a worktree's "can't find crate" or "symbol not found" cargo-ci failures as a tree defect.** Four workers hit stale pool-seeded or pre-merge caches; `cargo clean -p <crate> --profile ci-test` (the profile matters) or removing `target/debug` and `target/ci-test` fixed it. The gate runs on main after the merge anyway.
- **Do NOT use the two-dot form `git diff --name-only main..<branch>` to list a branch's files.** It lists everything on main the branch lacks (544 KB of output here). Use `main...<branch>`.
- Unresolved traps from [session 10](handoff-8.md#traps--do-not-repeat-these) and earlier remain in force; the `jit doc add` demangled-symbol trap is resolved (jit `f69c9b87` fixed 2026-09-12).

## Open questions needing invoker input

- None. (The session-10 question on recording achieved niceness is resolved as bug `706a8f93`, option A, because the pitfall record names epic REQ-01 and REQ-06; veto by rejecting the issue.)

## Reference artefacts

- Epic: `jit issue show 1a379447`; [measurement contract](measurement-contract.md); [progress](progress.json); reviews under [reviews/](reviews/) (`3be770d5-r1.md`, `6fb89a3c-r2.md`).
- Protocol v4: `dev/active/f547c394/{protocol.md,amendment-v4.md,addendum.schema.json}`.
- Window: [run-window.sh](bench-window/run-window.sh), [queue.tsv](bench-window/queue.tsv), [follow-window.sh](bench-window/follow-window.sh); `systemctl --user list-timers | grep gf2` (needs `XDG_RUNTIME_DIR=/run/user/1000` and `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`).
- Pending artifact tables: [pending-links/](pending-links/).
- Scripts: `.agents/skills/jit-execution-lead/scripts/{dispatch-worker-worktree.sh,check-leak-into-main.sh,reclaim-worker-worktree.sh}`.
- Gate records: `.jit/gate-runs/`.
