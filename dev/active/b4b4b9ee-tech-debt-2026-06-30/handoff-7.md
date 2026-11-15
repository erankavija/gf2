# Handoff: Technical debt assessment remediation (2026-06-30) (b4b4b9ee), session 7

**Date:** 2026-11-15
**Prior handoffs:** `handoff-6.md` (pool findings, traps). Its traps still bind unless resolved below.

## Current state

- Wound down at about 23:00 by the owner's order through the steward (`gf2-a2`): the tracker cutover has priority. This epic takes no dispatch, gate or measurement until the owner resumes it. The lead was `gf2-24`. No push; the history rewrite, the benchmark window and the streams `d77176e5`, `ae03bcd0` and `45ecf975` stay paused. `41517289` stays the owner's action.
- Landed and closed this session: `574ed420` (merge `1ff971676`), `697b9178` (`12f0f331b`), `9c0568ee` (`fb96c74d8`), `d8aee1f4` (`848bf4e95`).
- Landed, still open: `959ab809` REQ-01 (`6e2d5ec59`). Each failed cargo-ci step keeps its full log under `target/cargo-ci-logs/<stamp>-<pid>/` and prints the path; nextest steps also print each failing test, the error blocks and the summary. REQ-02 and REQ-03 are open. The first captured occurrence (recorded in the issue) reads as a nextest cancellation: every started test passed and 19 were not run.
- Filed this session: `959ab809`, `4f395137` (unseeded `quick_test()` simulations), `1452ba69` (converge the wrapper self-test helpers, after `18276ff1`).
- No process of this lead runs. No claim lease is held.

| Issue | Branch head | Done | Missing |
|---|---|---|---|
| `18276ff1` | `90ed7a351` | rework 2, fixture fix, option (a); production fixes in the pool wrapper guardian and the budget gate sampler; REQ-06 attempts 1 and 2 recorded | the case-7 SIGINT fix (issue section "Option (a) review and park"), then REQ-06, final gates, steward approval, `test-budgets` on the open epics |
| `4f395137` | `ac0859cb9` | implementation `6192440ad`; REQ-01 to REQ-03 evidence on the branch; `cargo-ci` passed at the head | `code-review` rerun, land |
| `22bb2dcd` | `f8aec8aab` | main merged, target warm | `cargo-ci`, `code-review` on the landing head; then `90cd64a9` |
| `d73ac633` | `0ac0d915b` | as in `handoff-6.md` | `cargo-ci` |
| `e29fd776` | `5520d7b03` | as in `handoff-6.md` | `cargo-ci`, `doc-review` |
| `9ff05f12` | `b9f2f9a42` | — | `cargo-ci`, `code-review`, `doc-review` |
| `b5782224` | `75f70cab1` | REQ-01 to REQ-03 implemented | `cargo-ci`, `code-review` |
| `903dc74c` | `22f57141d` | unblocked by `697b9178` | merge main, `cargo-kani`, `code-review` |

Each issue records its parked state. The worktrees of `d73ac633`, `e29fd776` and `4f395137` hold uncommitted gate-run records in `.jit/`.

## Dispatch record

| Dispatch | Issue | Vendor | Model / tier | Reviewer and verdict |
|---|---|---|---|---|
| review of parked WIP | `18276ff1` | Codex | gpt-6.1-sol, high | — |
| rework 1 | `18276ff1` | Codex | gpt-6.1-sol, high | Claude Opus 5.5: FAIL |
| rework 2 | `18276ff1` | Claude | Opus 5.5 | code-review gate: REQ-06 evidence only |
| F2 doc move | `18276ff1` | Codex | gpt-6-luna, xhigh | — |
| fake process table | `18276ff1` | Codex | gpt-6.1-sol, high | Claude Opus 5.5: FAIL (FIFO hang), parked |
| fixture fix (steward resumption) | `18276ff1` | Claude | Opus 5.5 | Codex gpt-6.1-sol: PASS |
| option (a) (owner) | `18276ff1` | Codex | gpt-6.1-sol, high | Claude Opus 5.5: FAIL (ignored SIGINT), parked |
| implement, 2 reworks | `959ab809` | Claude | Sonnet 5.5 | Codex gpt-6.1-sol ×3: FAIL |
| final fix (steward) | `959ab809` | Codex | gpt-6.1-sol, high | Claude Sonnet 5.5: PASS |
| implement, evidence | `4f395137` | Claude | Sonnet 5.5 | code-review gate: evidence |
| implement | `d8aee1f4` | Codex | gpt-6.1-sol, high | Claude Sonnet 5.5: PASS |

The next implementation dispatch is Claude.

## Decisions that bind the next lead

- `18276ff1` (steward and owner): REQ-08 option A, with test pools capped at the tokens they reserve and the cap taken from `RAYON_NUM_THREADS`. REQ-05 "issue guidance" means live instructions. A sustained busy host ends `test-budgets` with verdict `error`. The per-epic `test-budgets` requirement is added with `jit gate add` on main at landing. Owner option (a): self-test waits end on events. Option (b), the self-test in the quiet-host path, applies only after a review fails on a timing dependence. The merge goes to the steward.
- `959ab809`: once the safety net exists, an unmatched nextest line shape is advisory.
- `058be3826` (subject "unused") stays on main; its reword is in `45ecf975`'s Notes.
- Commits with no issue carry the scope `jit:none` (AGENTS.md, owner).
- Every main commit, tracker-only ones included, is announced to the carve-out lead first, with main's index empty.

## What happened

- `574ed420` REQ-04 pair 17:08 to 17:38: before cold 832 s, warm 146 s; after cold 702 s, warm 120 s; all rc=0. REQ-03 without shim and sccache rc=0.
- `697b9178`: Kani rerun at the landing head, 3 of 3, because main's `6379e04e` changed `bitvec.rs` after the recorded run.
- `18276ff1` REQ-06 attempt 1 failed on the self-test reading the real `/proc`. Attempt 2 failed on the self-test's 20 s wait; the last review traced that failure to SIGINT being ignored under the harness's `&` launch, not to load. `d8aee1f4` failed four runs of attempt 2 and is now fixed.
- The fixture audit fixed two production defects: a pool-wrapper guardian that blocked forever after a SIGKILLed wrapper, and a budget-gate sampler exit that was read as busy.

## Next

1. Owner resumption only. Then: `4f395137` code-review and land (steward priority); `22bb2dcd` then `90cd64a9`; `18276ff1` SIGINT fix (Claude, per alternation), blocking Codex review, REQ-06 with a harness that starts runs with SIGINT at its default; the rest of the table.

## Traps

- Artifacts outside the tree are in `~/.local/share/gf2/b4b4b9ee-handoff-7/`. They hold the REQ-04 pair driver (`req04/pair.sh`), the REQ-06 driver and sampler (`req06/run6.sh`, `req06/sample.py`; the sampler also counts runnable threads in processes whose environment carries `NEXTEST_EXECUTION_MODE`), the run logs, and the reviewer's SIGINT reproduction (`18276ff1-sigint-repro/`).
- `run6.sh` starts each cargo-ci with `( ... ) &`, so SIGINT is ignored in every child. Restore it (e.g. `trap - INT` in the subshell, or launch through a wrapper that resets the signal) before the next REQ-06 run; otherwise case 7 of the budget-gate self-test hangs.
- Landing scripts (in that directory): they take only the event lines of the issue files they name, keep other sessions' pending lines, refuse when main's index holds staged changes, and refuse a nonconforming subject. The first version committed a peer's claim events under the subject "unused".
- A bare file name in issue prose resolves from the store directory since `28aa2a9d`. Use repository paths, and run `scripts/check-references.py` without a pipe after every issue edit: `| tail` hid a failure once.
- Matching processes by command line finds the dispatch shell whose brief mentions the pattern. Match by `comm` and working directory.
- Main moves often. Merge main, gate, and land in one go; a gated head that lacks the current main reruns its gates.
- A fresh worktree's cargo-ci gate exceeds 900 s at load above about 15; warm it with a direct run first.
- Until `18276ff1` lands, consecutive 15.00 s kills are pool-token waits, not defects.
- A Codex worker ran cargo outside the host shim; every brief must tell workers to keep the shim.
- `git checkout -- .jit` in a worktree is refused by the session classifier when it would discard records; commit gate records instead.

## Open questions for the invoker

None.

## References

- Epic `b4b4b9ee`; `progress.json` in this directory; steward handoff `dev/active/2caf738d-1-0/handoff-4.md`; carve-out plan `dev/active/6ba9be17-tracker-repository/plan.md`.
