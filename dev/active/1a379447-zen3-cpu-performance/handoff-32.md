# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 36

**Date:** 2026-10-13
**Session number:** 36
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-31.md` in this directory

## Current state

- Epic: `1a379447` — state: backlog in the tracker (derived; work is in progress)
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 125 done, 3 in_progress, 36 backlog/ready, 26 rejected. Direct children: 26 of 28 done; open `817fc47d` (after `63bad95d`) and `1362381c` (after story `ed3d490e`).
- Active claims: `63bad95d` (`agent:worker`, four confirmations queued), `4252085a` and `cba35553` (`agent:worker`, no worker running; both wait for the invoker).
- Open escalations: `cba35553` REQ-03 amendment; `4252085a` rework round 4.
- Progress file: `progress.json` in this directory

## What just happened

- `05c9065e` done: `dev/bench_results/ed3d490e/preparation/peak-memory.json`, two lead reworks, code-review round 1 failed on three findings, round 2 passed; verdict `reviews/05c9065e-r2.md`. It added `arm::child_environment`, `arm::validation_launch` and `benchmark-ab-runner launch` to `tuning-campaign-support`, and the shared projection `survey/arm_build.py`.
- `0a77ecfc` done: declaration, protocol, manifest, producer reports, rehearsal record and queue line under `dev/active/ed3d490e-zen3-ldpc-frame-simd/calibration/`; code-review round 1 failed on script headers, round 2 passed; verdict `reviews/0a77ecfc-r1.md`. `20af7e6e` is ready.
- `4252085a` merged three times (`df7603594`, `b30832568`, `d9420f9de`) and still fails code-review: each round names another field the check compares outside the closed identity set (whole build-record digest; the executable entry's `inputs`; `linked_aff3ct` at `record-intra-quality.py:66`). cargo-ci passes at the round-3 tree; research-review passed at round 2 and is not rerun.
- Finding of `4252085a`: the recorded bundles carry LLRs at half the BPSK closed form (`quality-intra/llr-scale-probe.json`). Bug `11bfbfd9` filed outside the epic for the `gf2-sim` test frame source.
- Invoker rulings (interview, 2026-10-12): `4252085a` round 3 authorized; the INTRA arm multiplies recorded LLRs by two (plan DEC-13); keep the staged `aff3ct-throughput-arm`; reject `e430f035` and drop its citations (`1a4182001`); the source-evidence check stays outside cargo-ci.
- `cba35553` created and merged (`d9420f9de`): the INTRA arm takes `GF2_AFF3CT_LLR_FACTOR` from its catalogue entry; INTRA package rebuilt; `build-identity.json`, the quality record and `peak-memory.json` regenerated. Not gated: see the open question.
- Window timer `gf2-bench-window-20261013` stays armed for 04:00 EEST with five jobs: four `63bad95d` confirmations and the `0a77ecfc` campaign.
- Reclaimed `agent-4252085a` and `agent-0a77ecfc-r1`.

## What to do next

- [ ] Read `.agents/bench-window/window.log` loads, then each campaign's own execution log. Collect the four `63bad95d` confirmations as `handoff-31.md` describes, and integrate `63bad95d` in its merge order.
- [ ] Collect the `0a77ecfc` campaign from its own log in `agent-0a77ecfc`; a job that exited 75 stays queued (re-arm with the invoker's approval). Then dispatch `20af7e6e` in that worktree.
- [ ] On the invoker's answers: amend `cba35553` REQ-03 and run its three gates on main; dispatch `4252085a` round 4 in `agent-ed3d490e-run` as one audit of every field the check compares.
- [ ] After `63bad95d` and `20af7e6e` merge and before the story's first freeze: run `survey/record-gf2-arms-build.py` in `agent-ed3d490e-run`, then `record-intra-quality.py record` and `record-peak-memory.py record`, and re-register the records.
- [ ] After the `0a77ecfc` evidence is collected: trim the comments of `calibration/run-campaign.sh`, `rehearsal/private-lock.sh` and `rehearsal/untimed-windows.patch` with one rehearsal rerun.
- [ ] Then `5d590bac` (after `4252085a`), `817fc47d` (after `63bad95d`), and `1362381c` after the story.

## Traps — do not repeat these

Traps of `handoff-31.md` and earlier remain in force.

- **Do NOT write a criterion stricter than the convention it names** (handoff-28, hit again). The lead's `cba35553` REQ-03 says the catalogue names executables by digest; `arms.json` names none for workspace arms. The issue is merged and cannot be gated until the invoker amends it.
- **Do NOT answer an identity finding field by field.** `4252085a` failed three code-reviews on three different compared fields. At the first such finding, order one audit of every compared field against the closed set of `behavioral-evidence-validity` (handoff-16 gave this rule for harness findings).
- **Do NOT ask a recorder for a check mode beyond what a criterion names.** The lead's brief for `05c9065e` asked for one; code-review failed its relaunch envelope as an unnamed acceptance checker.
- **Do NOT let a leaf change `dev/tools/tuning-campaign-support` without sequencing its siblings.** `05c9065e` did; `0a77ecfc` then merged main twice to re-pin its manifest, and the gf2 arms' build-record inputs moved.
- **Do NOT rebuild a workspace arm in another worktree and expect its pinned digest.** The executables embed the checkout path (`4252085a` round 2); copy the pinned files with `cp -p` and verify the digests.
- **Do NOT take a worker's "done" as its last word, and do not merge before its stop is confirmed.** Three workers reported before reading queued messages; `05c9065e` committed once more after the lead's stop. Check `git merge-base --is-ancestor <commit> <branch>` and the tip before acting.
- **Do NOT `cd` in the lead shell, even to read.** It re-anchored the session once at start.
- **Do NOT stop a gate task and assume every `jit` process is yours.** `jit gate evaluate b069e8ef` of another session ran on the host; identify a process by its id and arguments before acting.
- **Do NOT read three `permanent_rare_event_artifacts` timeouts at load 28 as a tree defect.** The rerun behind a load guard passed.
- **`jit issue reject --reason` writes a `resolution:<token>` label**; `jit issue update --state rejected --reason` is refused.

## Open questions needing invoker input

- Question: amend `cba35553` REQ-03?
  - Context: the lead wrote "the build identity record and the catalogue name the rebuilt INTRA executables by digest"; only the build record names them, and it pins the catalogue by digest.
  - Options: amend to "The build identity record names the rebuilt INTRA executables by digest and pins the catalogue that declares the factor, written by its committed recorder."; make a recorder write digests into `arms.json`.
  - Recommendation: amend.
- Question: authorize `4252085a` rework round 4?
  - Context: three reworks spent; the same finding class three times; round 3 names `linked_aff3ct` among the compared fields.
  - Options: authorize one audit round; take over manually; reject the issue.
  - Recommendation: authorize the audit round.
- Question: does the workspace arm build become independent of the checkout path?
  - Context: the story's arm digests reproduce only in `.agents/worktrees/agent-ed3d490e-run`.
  - Options: file a task for a path-remapped build; keep the worktree as the build tree.
  - Recommendation: keep the worktree until the story's evidence is collected.

## Reference artefacts

- Epic: `jit issue show 1a379447`; story: `jit issue show ed3d490e`; plan decision DEC-13 in `dev/active/ed3d490e-zen3-ldpc-frame-simd/plan.md`
- Worker brief: `dev/active/1a379447-zen3-cpu-performance/worker-brief.md` (the registry governs its provenance lines)
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`; state `.agents/bench-window/`
- Lead verdicts of this session: `reviews/05c9065e-r2.md`, `reviews/0a77ecfc-r1.md`
- Story records: `dev/bench_results/ed3d490e/preparation/` (`build-identity.json`, `peak-memory.json`, `quality-intra/`); calibration: `dev/active/ed3d490e-zen3-ldpc-frame-simd/calibration/`
- Rulings: `progress.json` `escalations` and `session_scope`
