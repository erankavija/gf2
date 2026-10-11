# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 34

**Date:** 2026-10-11
**Session number:** 34
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-29.md` in this directory

## Current state

- Epic: `1a379447` — state: in_progress
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 112 done, 6 in_progress, 44 backlog/ready, 26 rejected
- Active claims: `d4185e9e`, `aff59c3c`, `bfc2ceab` (rework in flight, claimed by the lead for gating), `7dbe1393`, `8e53235b`, `63bad95d` (waiting for the window)
- Open escalations: none
- Progress file: `progress.json` in this directory

## What just happened

- Invoker rulings of 2026-10-11, all applied and recorded in `progress.json`: repeat each load-disturbed family once; M4RI pair as a second pilot in the existing family (`v4-r2`); `bfc2ceab` REQ-04 exploratory cost accepted; `7dbe1393` merges with `matvec_parity_kernel` unselected; load guard per queue line only; stage builders stay public and state the contract; AFF3CT INTRA arm converges on the `3be770d5` handle before measuring; story arms build in `agent-ed3d490e-run`; `tuning-campaign-support` as one root workspace dependency; the remaining `dev/` path is a tracked exception.
- Invoker steer: simplest, least protocol-intensive approach; the focus is HPC results.
- `packages/sim-research/manifest.toml` 1.2.5 carries the reviewer models; `jit validate` passes.
- Filed: `81e955b0`, `1ca04967`, `151af028`, `07024d65`, `754208a0`, `e430f035` (exception to `no-dev-path-coupling`, outside the epic, depends on `aff59c3c`).
- `81e955b0` (route-equivalence sentence on the twelve stage builders): cargo-ci, code-review, doc-review passed; done.
- `151af028` (checkpoint resume interrupt through the heartbeat; no resume defect found): cargo-ci, code-review passed; done.
- `d4185e9e` merged (`56351f104`); cargo-ci passed; code-review and research-review FAILED with F1 native header identities missing, F2 inherited compiler environment uncontrolled, F3 missing pinned arm accepted. Rework 1 sent to `w-d4185e9e-s34` in `agent-ed3d490e-run`.
- `aff59c3c` rework 1 merged (`528aaea60`); cargo-ci passed; code-review FAILED with F1 root `dev/` path, F2 full-equality identity comparison in `tuning_calibration.rs:1471`, F3 comments restating names. Rework 2 (last allowed) sent to `w-aff59c3c-s34` in `agent-aff59c3c-r2`; F1 resolves by a comment citing `@/issue/e430f035`.
- `bfc2ceab` merged (`e2748cae7`, `d70cf1dc1`); cargo-ci passed; code-review FAILED with F1 PAM contract test detection-only (`entry_conditions.rs:441`), F2 arm manifests reach the support crate by directory climb. Rework 2 sent to `w-bfc2ceab-s34` in `agent-bfc2ceab`; F2 resolves by the exception comment unless the manifest is digest-pinned. The two findings of the earlier failed review are confirmed resolved.
- `7dbe1393` merged (`6c6e57870`), stays in progress for its repeat pilot.
- `8e53235b`: pilot verified (word-wise ahead on all five cells, exploratory); confirmation frozen and queued. Branch unmerged at `75280f153`.
- `63bad95d` repeats prepared on three unmerged branches: fam `6d2106d1a`, m4ri `74ba66ed2`, pm4ri `a84073607`.
- Window `gf2-bench-window-20261012` armed for 2026-10-12 04:00 EEST; jobs listed in `progress.json` `benchmark_window`.

## What to do next

- [ ] Read `.agents/bench-window/window.log` loads and each campaign's own execution log; dispatch collection for `8e53235b` (confirmation), `7dbe1393`, and the four `63bad95d` pilots. A line that exited 75 stays queued: re-arm the timer.
- [ ] Collect the three rework branches (check `git log main..<branch>` and a clean worktree first; a worker may have stopped uncommitted): `worktree-agent-ed3d490e-run` (`d4185e9e`), `worktree-agent-aff59c3c-r2`, `worktree-agent-bfc2ceab`. Review each resolution table against the findings in `jit gate status <id> code-review`, merge, then cargo-ci and the failed reviews. `aff59c3c` and `bfc2ceab` are at 2 rework attempts: another failure escalates.
- [ ] After `bfc2ceab` merges: run `bfc2ceab/survey/make-inventory.py` and the `2037941f` producing-closure `--check` commands.
- [ ] After `d4185e9e` is done: dispatch `1ca04967` in `agent-ed3d490e-run`, then `4252085a` in the same worktree; then `5d590bac`, `a4476c6a` as they unblock. `aff59c3c` done unblocks the `958639ec` chain. `6230e0b6` now waits only on its other dependencies.
- [ ] File a bug for the failing `6e87c436/survey/make-source-evidence.py --check` on main (`crates/gf2-core/src/matrix.rs` anchor missing after the `7dbe1393` merge).
- [ ] Write `reviews/81e955b0-r1.md` and `reviews/151af028-r1.md` from the gate records; the lead verdicts were not written down this session.
- [ ] Apply the links tables in the session-34 scratchpad for `8e53235b` and `63bad95d-*` after their merges (regenerate from the branches if the scratchpad is gone).
- [ ] Reclaim `agent-81e955b0` and `agent-151af028` with `LEAD_CACHE_DIRS=none .agents/skills/jit-execution-lead/scripts/reclaim-worker-worktree.sh`.
- [ ] Remaining direct epic children: `1362381c`, `817fc47d`, `07024d65` (after `8e53235b`), `807ddab2`, story `ed3d490e`.

## Traps — do not repeat these

Traps of `handoff-29.md` and earlier remain in force.

- **Do NOT assume an idle worker has reported.** `w-aff59c3c-s34` went idle with four edited manifests uncommitted and no report; it finished only after a nudge. Check the worktree's `git status` and `git log main..` when a worker is idle without a report.
- **Do NOT treat an invoker ruling on form as a review pass.** The ruled root workspace dependency still failed `no-dev-path-coupling` because the path spells `dev/`. A literal Cargo path under `dev/` needs the exception citation `@/issue/e430f035`.
- **Do NOT ask a worker for "a new family version" of a dense-harness family.** The families are a compiled enum; a new one needs harness code. The M4RI pair repeats as run id `v4-r2` in the existing family.
- **Do NOT merge main into a branch with a queued run.** `8e53235b`, the three `63bad95d` branches and `7dbe1393` measure the tree their pilot measured.
- **Do NOT expect `jit dep add` between two direct epic children to succeed.** It is refused under transitive reduction; the lead holds such orders (`07024d65` after `8e53235b`). Use `--reduce` when adding an edge that makes another redundant.
- **Do NOT merge a kernel change without running `bfc2ceab/survey/make-inventory.py`.** It failed on main after `7dbe1393` added `and_parity_fn` without a verdict in `survey/review.py`.
- **Do NOT resolve a `schema-versions.json` merge conflict by hand.** Take ours, then `python3 -B dev/scripts/check-addendum-schema-versions.py --write`.
- **Do NOT edit `.jit/gates.toml` to change a reviewer model.** The values project from `packages/sim-research/manifest.toml`; edit it, bump the version, `jit profile upgrade --profile path:packages/sim-research`.
- **Do NOT create an issue outside the epic without a dependency.** `jit validate` fails on an isolated issue (`754208a0`, `e430f035` each needed one edge).
- **Do NOT `cd` in the lead shell.** Start every command with `cd /home/vkaskivuo/Projects/gf2 &&`; a `cd` into a subdirectory re-anchored later commands twice.
- **Do NOT run review gates of one issue in the same chain as its cargo-ci on a loaded host.** The five cargo-ci runs back to back raised the load to 37 and still passed; start the chain only with no worker building.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show 1a379447`; story: `jit issue show ed3d490e`
- Worker brief: `dev/active/1a379447-zen3-cpu-performance/worker-brief.md`
- Measurement contract: `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md`
- Window: `dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh`, `queue.tsv`; state `.agents/bench-window/`
- Rulings: `progress.json` `escalations`; `bfc2ceab/decisions.json` `later_rulings`
- Review of `aff59c3c` round 1: `reviews/aff59c3c-r1.md`; later findings in `jit gate status <id> code-review`
- Build identity record: `dev/bench_results/ed3d490e/preparation/build-identity.json`
