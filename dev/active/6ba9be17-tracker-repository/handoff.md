# Handoff: Move the tracker data into its own repository (6ba9be17), session 1

**Date:** 2026-11-15
**Prior handoffs:** none in this directory. Lead `gf2-7d` led the epic before this session; its state is in `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-6.md` and in the handover below.

## Current state

- Lead `gf2-e3` under steward `gf2-96`, from 01:17 local. At 01:35 the owner ordered every session to hand off. Wave 1 of 6 is gated but nothing has landed. The epic is `backlog` and unclaimed. Progress: [progress.json](progress.json).
- Both wave-1 issues are `in_progress` and assigned to `agent:gf2-7d` on main with no lease. gf2-7d asked the next lead to reassign them with `jit issue assign <id> agent:<lead>` on main.
- Nothing is pushed. Worktree `target/` directories were removed (5.8 GB each).

| Issue | Branch head | Gates | Next |
|---|---|---|---|
| `6d5ba9fb` gate checkers through the data root | `worktree-agent-6d5ba9fb` at `372cfb032` (main merged at `e1bc4b5d1`) | cargo-ci passed at `e1bc4b5d1` (run after a 900 s timeout at `44d8f5e69`, cold `target/`, no verdict); code-review (Codex) passed at `e1bc4b5d1`, 0 findings. Blocking Claude review: see below. Lead review PASS. | Land: merge main into the branch (tracker and `dev/` only changed since, so cargo-ci inputs should be identical; check with `jit gate evaluate`, which reuses a verdict for identical inputs), merge into main by hand, close. Then dispatch `3fb4accb`. |
| `28aa2a9d` reference checker against a mounted store | `worktree-agent-28aa2a9d` at `f07572f92` (main merged at `52ecbbd6b`) | cargo-ci passed at `52ecbbd6b`; code-review (Codex) FAILED at `52ecbbd6b`: F1 (REQ-02) and F2 (REQ-04). The lead confirmed both by reading the code. | Rework 1 of 2. The brief is written; dispatch it at a clean start. |

- Blocking Claude review of `6d5ba9fb`, which is recorded only here and in gf2-7d's notes. Copy it into the issue at landing: "Blocking Claude review (Claude Sonnet 5.5, beside the Codex gate): PASS at 44d8f5e69; REQ-01 to REQ-08 met; REQ-02 checked with the real jit against scratch roots (checkout and store unchanged by sha256 snapshot). Advisory: docs-mechanical exits 2 when JIT_DATA_DIR is outside any git repository (documented contract)."
- `28aa2a9d` rework findings, from the code-review run at `52ecbbd6b`:
  - F1: the issue-prose fault filter and `resolves()` in `scripts/check-references.py` (about lines 416 to 428) consult `trees.on_branch` for `.jit/` targets. A tracker file missing from the store therefore passes when some branch head holds it. Tracker references must resolve against the store only, and the fix needs self-test coverage.
  - F2: `check()` (about line 990) compares `Path.absolute()`, which keeps `..` and symlinks. `JIT_DATA_DIR=../<checkout>/.jit` then skips the issue prose. Compare with `resolve()` and cover a `..` spelling and a symlinked spelling.
- Rework 1 was dispatched to Codex gpt-6-luna (xhigh) at 01:35 and stopped within seconds by the hand-off order; it made no edit. The full rework brief (template, verdict, issue, return contract) was in the session scratchpad, which is not kept. Rebuild it from these findings and `jit gate status 28aa2a9d code-review --all --status failed --json`. Under vendor alternation that dispatch counts as Codex's. A Codex rework needs a blocking Claude review beside the Codex gate.
- `3fb4accb` (wave 2) is undispatched; it waits for `6d5ba9fb` to land. The steward requires the strongest non-Fable Claude tier for it, which is also the next vendor in the alternation. The design choices the lead settled for its brief:
  - The primary checkout is the parent of the absolute `--git-common-dir`.
  - The worktree identity is the basename of the linked worktree's git dir.
  - "A store of its own" means `<toplevel>/.jit` exists in a linked worktree. Before the cutover every linked worktree has one, so the entry point refuses there by design.
  - The no-argument form prints POSIX `export` lines that are safe to `eval`; with a command it `exec`s.
  - `JIT_DATA_DIR` and `JIT_WORKTREE_ROOT` are always recomputed from git (contract `tracker-root-resolution`); only `JIT_AGENT_ID` passes through.
  - REQ-07 is `root = ".."` under `[worktree]`.
  - The worker runs only focused `CARGO_CI_STEPS` checks.

## What happened

- Took over from `gf2-7d`. Its handover: both worktrees cut from main `747cae404` (after `a33af036b`); `6d5ba9fb` implemented by Codex gpt-6.1-sol (high), `28aa2a9d` by Claude Opus 5.5. `28aa2a9d` worker notes: the self-test was red before the implementation but was committed with it; with the data root under the root, untracked unignored store files are now scanned; a data root outside any git repository falls back to all files on disk.
- `6d5ba9fb`: the first cargo-ci (gf2-7d, 01:11:40) timed out at 900 s with no output. The pool had no `cargo-pool-run` waiters and no refill after 01:10:54. After the main merge (one conflict in the issue file, resolved with main's copy) cargo-ci passed in 3 min and code-review passed.
- `28aa2a9d`: main merged; cargo-ci passed, code-review failed (above).
- Agreed with `gf2-00` (45ecf975 stream) on `2a4c2684`; see References.
- Pre-existing drift on main, outside this epic: `contrib/gates/code-review-prompt.md` differs from `packages/sim-research/assets/live/contrib/gates/code-review-prompt.md`. Commit `8610e44cc` added the "immutable main history is advisory" paragraph to the projection only. Reported to the steward; it needs a tracked issue (`@/inv/no-deferred-defects`).

## Next

1. Take a main hold, reassign both issues to the new lead, then land `6d5ba9fb`:
   - merge main into the branch;
   - re-evaluate cargo-ci at the merged head;
   - merge by hand into main. `.jit/events.jsonl` has `merge=union`; stage only your own lines, because main carries other sessions' uncommitted lines;
   - copy the worktree's `.jit/gate-runs/6d5ba9fb-*` into main's store;
   - move the issue to done;
   - run `jit validate`, `python3 -B scripts/check-references.py` and `python3 scripts/check-tracked-budget.py`.
2. Dispatch `28aa2a9d` rework 1 (Codex), then a blocking Claude review, the Codex gate, and the landing.
3. Dispatch `3fb4accb` (Claude Opus tier) in a new worktree from main after step 1. Then wave 3: `daa33be8` (strongest Claude tier) and `6310679b`. When both land, give `gf2-00` the interfaces it needs for `2a4c2684`.
4. Waves 4 and 5 as in the plan. `6889e6e6` waits for an owner decision on timing: the rewrite is paused (steward, 01:22).

## Traps

- Host rules tonight: at most two cargo-ci runs host-wide, plus one from the jit project's session. If a gate nears 900 s with three runs going, drop to one. After any kill or timeout, run `grep -i refill /tmp/cargo-pool-1000/log` and check `ps` for `cargo-pool-run` waiters with idle CPU. Do not touch the pool, its FIFO or its daemon. `18276ff1` and `574ed420` are parked, so the 15 s per-test kill stays and a killed test runner can leak a pool token.
- A fresh worktree has a cold `target/`; its first cargo-ci can exceed 900 s. A warm pre-run is allowed after a timeout; a rerun at a merged head took 3 min.
- `jit gate evaluate` in a worktree leaves `.jit/events.jsonl` and the issue file modified, and they conflict with main on merge. Commit them as `chore(jit:<id>): record gate runs` (done on both branches), or discard them before merging main. On an issue-file conflict take main's copy, then re-evaluate.
- A code-review run before cargo-ci passes reads the missing CI evidence; run it after.
- The Bash tool's working directory follows a `cd` into a worktree; use subshells or `git -C`.
- The user-level `land.sh` does not fit; merge by hand.
- Earlier traps in `dev/active/b4b4b9ee-tech-debt-2026-06-30/handoff-6.md` still bind: commit subjects (closed type list, `jit:<id>` scope, at most 71 characters), zsh word splitting, kills by working directory.

## Decisions that bind the next lead

- Every owner decision recorded in [plan.md](plan.md) (D-01 to D-14, the narrowed REQ-06, the amended REQ-04, the `resolvable-references` wording) binds as written. Do not re-litigate them.
- Parked branches of other streams are carried through the rewrite as rewritten refs (owner, 2026-11-15, through the steward). The cutover tool's branch reconciliation (`e610f5c4`) handles them, and `gf2-00`'s design records it.
- `gf2-00` implements `2a4c2684` once `3fb4accb` and `6310679b` land (steward, 01:22). It needs:
  - from `6310679b`: the `tracker-bootstrap` and `tracker-snapshot` usage, and bootstrap working in a primary checkout that is a fresh clone of a rewritten mirror;
  - from `3fb4accb`: the `tracker-env` variables;
  - from `e610f5c4`: a scratch tracker repository path and a bundle.
  The lead added three rehearsal checks, and `gf2-00` accepted them:
  - the lease and lock directory sits under `.jit/.git`, and `jit claim list` is unchanged across the reseat;
  - the cutover's step-6 snapshot precedes the rewrite's step-1 bundle;
  - the parked-branch precondition is recorded as the owner's decision.
- Agents: tier by difficulty, never Fable; alternate Claude and Codex per dispatch; the reviewer comes from the other vendor; a Codex change gets a blocking Claude review. `3fb4accb`, `daa33be8`, `7854cfad` and `e610f5c4` go to the strongest non-Fable Claude tier. Next implementation dispatch: Claude (`3fb4accb`), after the Codex rework of `28aa2a9d`.
- `7854cfad` goes to the steward before its merge (gate-set change). With `18276ff1` parked it has no sibling to sequence with.
- `e610f5c4` and `6b9e6b14` run on scratch clones. Nothing touches the real store or history before `6889e6e6`.

## Open questions for the invoker

None.

## References

- [plan.md](plan.md), [breakdown.json](breakdown.json), [investigation.md](investigation.md); charter D-8 and D-9 in `dev/research/2caf738d-charter.md`.
- The `2a4c2684` design and the rewrite rehearsal script are kept by `gf2-00` outside the repository; ask `gf2-00` for their location.
