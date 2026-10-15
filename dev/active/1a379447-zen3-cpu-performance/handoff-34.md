# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 38

**Date:** 2026-10-15
**Session number:** 38
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-33.md` in this directory

## Current state

- Epic: `1a379447` — state: backlog in the tracker (derived; work is in progress)
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 158 done, 5 in_progress, 9 backlog/ready, 26 rejected. Direct epic children open: `1362381c`, `e5f8b3ae` (new); the rest sit under story `ed3d490e`.
- Active claims: `32bfe9f8`, `a03daf3f`, `7eddc5e2` (`agent:jit-execution-lead`; confirmations measured, collection not dispatched); `859bc8cb`, `37aa7254` (`agent:worker`, blocked on the invoker).
- Open escalations: `859bc8cb` REQ-02, `37aa7254` (below; `progress.json` `escalations`, 2026-10-15).
- Session rule (invoker, 2026-10-15): no benchmark window policy this session; timed runs ran in-session through `bench-window/run-window.sh` on a quiet host. The rule binds this session only.
- Progress file: `progress.json` in this directory

## What just happened

- Done, all gates on main: `9eb6f13c`, `ec4e71ca` (lead added `ec4e71ca/cause.md`, `reproduce.sh`, `spawn-delay.c`), `d11a45d4` (lead added `d11a45d4/record.md`), `17e08faa`, `4976531f`, `69dd39f1` (shared dev crate `dev/tools/thread-allocations`), `d93a1c7e`, `5afa8d76` (two doc reworks), `5d590bac` (two reworks; runner `smoke --stage` re-exports, `atomic_replace`), `a4476c6a`, `4d8fd613` (`survey/make-family.py`), `dbea4e10` (`survey/run-family-campaign.py`; atomic receipt in `finalize`), `3f3efdcf` (`ldpc-batch-profile`), `2a69b276` (`survey/smoke-family.py`), `c103a288` (`gf2-batch-tuned-arm`, native build), `2a3b479d` (shared `3be770d5/survey/profile-series.sh`; `repository_files.py file` takes a path suffix; two reworks), `f434ddd3`, the five freezes `a91e7cef` (`trial_ledger::next_attempt`, `benchmark-acceptance --next-attempt`, `survey/record-confirmatory-bound.py`), `a7ae2623`, `9bcc7e73`, `493dba5a`, `caabc4e1`, and the five pilot collections `d0d2c1bd`, `093ae4d0`, `fb65bb3f`, `87a6bb36`, `a80062a1`.
- Window runs in-session: 04:37-05:13Z profile series (9 sessions) and five pilots, each complete and Accepted; 06:15Z onward the three confirmations (`d0d2c1bd`, `093ae4d0`, `87a6bb36` queue lines): each ran 6 cells to a terminal complete record and was Accepted (`qualifies=false` on all three; read the per-cell outcomes from the receipts); job output in `.agents/bench-window/` keys b3c8ec809e78c13f, c1edd7a52c50baad, cc4f92d6e72c3944.
- Filed `e5f8b3ae` (tuned arm's ~150-line runner copy; tracked exception, converges after `ed3d490e`), cited in the arm's source.
- Lead fixes on main: `69dd39f1` code-review F1/F2 (`8e3876ef2`), `a4476c6a` and `2a69b276` docstring trims, worker-brief resolution alpha wording.
- Reclaimed: `agent-9eb6f13c`, `-ec4e71ca`, `-d11a45d4`, `-17e08faa`, `-69dd39f1`, `-4976531f`, `-d93a1c7e`, `-815703a1`, `-5afa8d76`, `-4d8fd613`, `-dbea4e10`, `-2a69b276`.

## What to do next

- [ ] Collect the three confirmations: dispatch one worker on a brief like the pilot collections' (verify each log, commit receipt and snapshots, show the acceptance re-run reproduces byte for byte in a committed record, record per-cell outcomes as evaluated, no rerun after a non-passing outcome) (issues `32bfe9f8`, `a03daf3f`, `7eddc5e2`; worktree `agent-ed3d490e-run`, new branch at main). Their stages and receipts sit uncommitted in that worktree. Then gate each (cargo-ci, code-review, research-review).
- [ ] Settle the two open questions below, then: `859bc8cb` (branch `worktree-agent-859bc8cb`, `d27193d4b`, unmerged) and `37aa7254` (nothing committed).
- [ ] After `859bc8cb`: `3bb47302` summarizes the profile series. Its raw output is uncommitted in `agent-ed3d490e-run/dev/bench_results/f434ddd3/profile/`; commit it there with the summary.
- [ ] After `37aa7254`: `67517a7b`, `7695aaee`; then `7211ae53` (tables), `6230e0b6`, story `ed3d490e`; then `e5f8b3ae`, `1362381c`, epic completion.
- [ ] Reclaim `agent-0a77ecfc`, `agent-63bad95d-run`, `agent-f434ddd3-r1`, `agent-2a3b479d`, `agent-859bc8cb` (after its ruling) once nothing reads them.

## Traps — do not repeat these

Traps of `handoff-33.md` and earlier remain in force, except these, which are retired:

- **cargo-ci repeats and HEAD ordering.** `cargo-ci` declares `[gates.inputs]` in `.jit/gates.toml`, so an evaluation over unchanged inputs reuses the prior verdict across issues and HEAD moves, and `.jit/` writes do not invalidate it. Gate several issues with `jit gate evaluate-many cargo-ci <ids>...`. Gate only merged, committed trees: three checks read HEAD or the index, which the digest does not see.
- **Lock staggering.** jit queues on lock contention with a 60 s default wait (`JIT_LOCK_TIMEOUT`); the 5 s bootstrap-lock timeout no longer exists.
- **Reading `.jit/issues/*.json` for gates.** `jit issue show <id> --json` lists them under `gates`; `gates_required` was removed from that shape.

- **Do NOT end a gate chain's `&&` on `| tail`.** `jit gate evaluate ... | tail -1 && jit gate evaluate ... code-review` started code-review on a failed cargo-ci; the pipe returned tail's status. Use `scratchpad/gate-chain.sh`'s pattern: capture `$?` before filtering.
- **Do NOT loop over an unquoted `$files` in zsh.** `for f in $files` ran once on the whole string and linked nothing; three gates started on unlinked evidence. Pipe into `while read -r f`.
- **Do NOT start a gate right after a merge that touches `tuning-campaign-support`.** The calibration pin `ed3d490e/calibration/producing-build-inputs.sha256` goes stale; run `make-calibration-producing-inputs.py --check` after every merge and repin before gating.
- **Restart sccache after stopping a gate task.** A stopped gate left a server with a deleted temp dir; every build step of the next run failed in under a second.
- **Do NOT let a family freeze or arm task in the build tree leave cited records stale.** `smoke.json`, `validation.jsonl` and `peak-memory.json` cite `build-identity.json` by digest; any arm or recorder change must rerun those recorders (`3f3efdcf`, `c103a288` follow-ups).
- **Do NOT accept a recorder in this story without its resumability audit.** `5d590bac`, `dbea4e10` and `2a3b479d` each failed code-review on campaign-resumability one path at a time; ask for every interruption point in the first brief.
- **Do NOT dispatch two workers that both need the build tree `agent-ed3d490e-run`.** Arm digests reproduce only there; serialize on branches, or give a worker its own worktree that reads the build tree's binaries read-only (`f434ddd3` rework).
- **A byte-for-byte regeneration criterion fails when the interpreter or a moved path enters the output** (`859bc8cb`: CPython 3.14.7 to 3.14.8, `dev/active/3be770d5` moved). Handoff-33's trap on such criteria applies to planned issues too; flag them at dispatch.

## Open questions needing invoker input

- Question: amend `859bc8cb` REQ-02?
  - Context: the committed profile summaries cannot regenerate byte for byte on this host even with the unmodified summarizer (pinned executable gone, generator path moved, CPython version changed); old and new summarizer give byte-identical outputs.
  - Options: A amend REQ-02 to old-vs-new byte identity; B rebuild and re-pin the executable and regenerate committed summaries; C reject and drop lane categories.
  - Recommendation: A.
- Question: how does `37aa7254` (decode selector holdout) proceed?
  - Context: REQ-01's `selector-calibration` pilot conflicts with protocol validation (holdout cells spend the first attempt); no portable arms exist (`c103a288` built a native tuned arm); `make-family.py` lacks a tuning-profile field.
  - Options: A amend REQ-01 to the 63bad95d pattern (`freeze-confirmation.py --purpose/--holdout-cells`) and file a task for the two portable arms in their own target dir; B spend the first attempt in the pilot; C reject the holdout chain.
  - Recommendation: A.

## Reference artefacts

- Epic: `jit issue show 1a379447`; story: `jit issue show ed3d490e`
- Rulings and pitfalls: `progress.json` `escalations`, `surfaced_pitfalls` (session 38 entries)
- Story tools: `dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/` (`make-family.py`, `smoke-family.py`, `run-family-campaign.py`, `record-arm-*.py`, `record-confirmatory-bound.py`)
- Families: `survey/families/*.json`; addenda under this directory's `<issue>/`; receipts under `dev/bench_results/ed3d490e/<family>/`
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`; state `.agents/bench-window/`
