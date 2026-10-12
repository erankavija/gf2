# Handoff — Maximize Zen 3 CPU throughput against open-source baselines (1a379447) — session 35

**Date:** 2026-10-12
**Session number:** 35
**Prior handoffs:** `handoff.md`, `handoff-2.md` … `handoff-30.md` in this directory

## Current state

- Epic: `1a379447` — state: backlog in the tracker (derived; work is in progress)
- Wave in progress: wave 4 of 7 (story `ed3d490e` runs its own layers inside it)
- Children summary (label `epic:zen3-cpu-performance`): 123 done, 1 in_progress, 38 backlog/ready, 26 rejected. Direct children: 26 of 28 done; open `817fc47d` (after `63bad95d`) and `1362381c` (after story `ed3d490e`).
- Active claims: `63bad95d` (`agent:worker`, no worker running; four confirmations queued).
- Open escalations: none.
- Progress file: `progress.json` in this directory

## What just happened

- Window of 2026-10-12 collected: six jobs, each accepted with no finding on a quiet host.
- Done this session, every gate passed on the merged tree: `aff59c3c`, `bfc2ceab`, `cb8f7c36` (filed and closed), `958639ec`, `8e53235b`, `7dbe1393`, `824cde7c`, `d4185e9e`, `07024d65`, `dd50ea11`, `1ca04967`. Lead verdicts are under `reviews/`.
- `8e53235b`: confirmation accepted and qualifying; the packed BCH kernel assembles codewords word-wise through `packed_place_codeword`.
- `7dbe1393`: the repeat family ends `resolution-insufficient`; production keeps its kernel and `matvec_parity_kernel` stays unselected.
- `63bad95d`: repeat pilots of four families committed on their branches; four confirmations frozen, smoked and queued. Native-ladder-v2 freezes margin 1.08 at resolution 0.07, as its pilot addendum's rationale states.
- Timer `gf2-bench-window-20261013` armed for 2026-10-13 04:00 EEST (invoker approved).
- Invoker rulings: `bfc2ceab` rework round 3 authorized; `d4185e9e` rework round 3 authorized after re-review under the new rules.
- Invariants reduced from 37 to 33 with the invoker, commit `54b4540f6`: `behavioral-evidence-validity` absorbs `runtime-observed-provenance` and names a closed identity set; `claims-trace-to-artifacts` absorbs `uncertainty-reported` and `external-claims-cited`; `campaign-resumability` absorbs `campaign-execution-logging`; `no-dev-path-coupling` covers run-time lookup and leaves build-system paths outside. Review prompts follow in `bad5c7ce3` (`sim-research` 1.2.6).
- Filed outside the epic: `1ce7e8a5` (BCH proof model against the word-wise assembly; depends on `8e53235b`).
- Reclaimed eleven worktrees. Remaining: `agent-63bad95d-fam`, `-m4ri`, `-pm4ri`, `-run`, `agent-ed3d490e-run`, and three of other epics.

## What to do next

- [ ] Read `.agents/bench-window/window.log` loads, then each campaign's own execution log; dispatch collection of the four `63bad95d` confirmations in their worktrees (`agent-63bad95d-fam` twice, `-m4ri`, `-pm4ri`). A line that exited 75 stays queued: re-arm the timer with the invoker's approval.
- [ ] Integrate `63bad95d` after the confirmations: merge order `worktree-agent-63bad95d-m4ri`, then `-pm4ri` (it contains the native branch; both changed `freeze-m4ri-confirmation.sh` the same way), then `-fam`. Expect conflicts in the dense harness `cells.rs` (main locates addenda and ledgers through `locate_live` since `7dbe1393`) and in `f547c394/schema-versions.json` (take ours, then `check-addendum-schema-versions.py --write`). The driver and `tuning_calibration.rs` are behaviour sources of its producing manifest and changed on main. Then: restore `ple.scalar_base_max_cols` 24, correct `calibration-plan.md` section 3 and `survey/planned-families.json`, record the REQ-11 exception citing `@/issue/817fc47d`, fix the comment at `crates/gf2-core/benches/tuning_calibration.rs:5431` that names the retired `runtime-observed-provenance`, then gates.
- [ ] Dispatch the ready story leaves: `4252085a` then `05c9065e` in `agent-ed3d490e-run` (serial, both touch the arms); `0a77ecfc` in a fresh worktree. The brief of `0a77ecfc` must include an untimed end-to-end run of driver and validator over a complete coding stage before any queue line.
- [ ] Regenerate `dev/bench_results/ed3d490e/preparation/build-identity.json` after the last merge that touches `dev/tools/tuning-campaign-support` and before the story's first freeze: the arms link that crate.
- [ ] Keep `agent-ed3d490e-run`: it holds the staged `target/ldpc-throughput/release/aff3ct-throughput-arm` the story's catalogue pins.
- [ ] After `63bad95d`: `817fc47d`, `807ddab2`. After the story: `1362381c`.
- [ ] Apply `pending-links/63bad95d-fam-links.tsv`, `-m4ri-links.tsv` and `-pm4ri-links.tsv` after the `63bad95d` merges; they hold this session's rows too. Skip frozen addenda and freezer records.

## Traps — do not repeat these

Traps of `handoff-30.md` and earlier remain in force, except where this session changed the rule they describe: the exception comment for Cargo paths under `dev/` (handoff-30) is void, and a provenance finding blocks only on a closed-set item or a number that could be wrong.

- **Do NOT tell a worker to leave `dev/bench_results` untouched.** The lead's brief for `07024d65` said so and the worker left two broken launchers; the criterion protects receipts and their `inputs/` snapshots, and live launchers may change.
- **Do NOT predict a reviewer's disposition to the invoker.** The lead said `d4185e9e` F1 would turn advisory under the new rules; it stayed blocking because the toolchain version is a closed-set item.
- **Do NOT write "restore the first commit's value" with a `git log | tail -1` recipe.** For `d4185e9e` it named a pre-merge branch commit; name the merge commit on main.
- **Do NOT read "Agent produced no output" as a verdict.** Two code-review runs (`07024d65`, `1ca04967`) ended that way and passed on the repeat.
- **Do NOT read `sccache: Failed to create temp dir` or two `permanent_rare_event_artifacts` timeouts as a tree defect.** Both occurred once; the server restart and a rerun on a quieter host passed.
- **Do NOT place a file in the main tree while a gate chain runs.** The gate driver refuses a dirty tree; draft in the scratchpad.
- **Do NOT merge a change to `tuning-campaign-support` and assume the arms build record still holds.** Every arm links it; the record's four digests changed twice today.
- **Do NOT expect a worker to reuse the links file name of an earlier dispatch safely.** The `7dbe1393` and `8e53235b` rework workers overwrote the first workers' tables; apply a table before dispatching the rework.
- **Do NOT edit only the registry when retiring an invariant.** `jit validate` failed on an `enforces:` label of another epic's issue and `docs-mechanical` on seventeen citations; sweep both, and leave receipt snapshots and archives alone.
- **Do NOT expect `jit item show @/issue/<id>` to resolve.** Issue addresses resolve through `jit issue show`; `docs-mechanical` checks them.

## Open questions needing invoker input

- Question: does the story reuse the staged `aff3ct-throughput-arm`, or is the comparator re-pinned after a rebuild on the converged handle?
  - Context: after `1ca04967` the staged executable builds only from a revision before `328b60f8d`.
  - Options: keep and copy the staged file; re-pin in a later issue.
  - Recommendation: keep; the closed identity set pins the executable digest and the source revision is the rebuild pointer.
- Question: does `e430f035` (move `tuning-campaign-support` under `crates/`) stay open?
  - Context: `no-dev-path-coupling` no longer covers Cargo manifest paths, so the issue has no invariant basis; the exception comments citing it remain in the tree.
  - Options: reject it and remove the comments; keep it as a layout change.
  - Recommendation: no strong preference.
- Question: does `6e87c436/survey/make-source-evidence.py --check` become a step of `scripts/cargo-ci.sh`?
  - Context: `cb8f7c36` made the check independent of later trees; it is run by hand.
  - Options: add a step; leave it out.
  - Recommendation: leave it out.

## Reference artefacts

- Epic: `jit issue show 1a379447`; story: `jit issue show ed3d490e`
- Worker brief: `dev/active/1a379447-zen3-cpu-performance/worker-brief.md` (its provenance lines predate commit `54b4540f6`; the registry governs)
- Window: `bench-window/run-window.sh`, `bench-window/queue.tsv`; state `.agents/bench-window/`
- Lead verdicts of this session: `reviews/*-r*.md` for the eleven closed issues
- Build identity record: `dev/bench_results/ed3d490e/preparation/build-identity.json`
- Rulings: `progress.json` `escalations` and `session_scope`
