# Handoff: Move the tracker data into its own repository (6ba9be17), session 3, supporting line

**Date:** 2026-11-16
**Prior handoffs:** [handoff.md](handoff.md) (session 1), [handoff-2.md](handoff-2.md) (session 2).

## Current state

- Lead `gf2-f6` runs the supporting line under steward `gf2-a2`; peer lead `gf2-5c` runs the cutover line (`e610f5c4`, `6b9e6b14`, `6889e6e6`) and holds the epic's claim. Progress: [progress.json](progress.json).
- Landed this session: `7854cfad` (`landed` gate, merge `8c46dae79`).
- Filed: `84022ee3` (rewrite the `code-task` preset as gate keys with the jit binary that reads them, plus three parse-case tests). It runs after the cutover; the host stays on jit 1.0.0 until then (steward, 2026-11-16).
- Parked: `d1eba151` at `348f302d4`; its Parked section states the open finding and the resumption steps.
- `18276ff1` (owner, 2026-11-16): resumed with this lead. The SIGINT fix is committed on its branch at `8233df6d0` (Codex gpt-6.1-sol); its blocking Claude Opus review passed, and its two advisories are recorded in the issue. The six-run REQ-06 measurement and the final gates wait until after the cutover. The REQ-06 driver `req06/run6.sh` under the `b4b4b9ee-handoff-7` artifacts now launches each run with SIGINT at its default.
- `2a4c2684`: unassigned; it waits for `e610f5c4` and runs after the cutover.
- Port: applied on main in `97b8e84a9` (19 open-issue branches, 21 fields, 36 event lines); the 30 run records those gate records name were copied into main's store. `e610f5c4` and `6b9e6b14` land through their own merges.
- Nothing is pushed.

## What happened

- Took the supporting line from `gf2-bf` and `gf2-24`.
- `7854cfad`: code-review passed at `b1c997544`, cargo-ci at the merged head `c4849dd01`; landed under the pre-cutover practice, because `check-landing` refuses the tip once recording a verdict moves it.
- Quiet-window inventory, decision basis, delete list, final abandon and survivor lists, port plan and its review: all in `~/.local/share/gf2/cutover-inventory/` (outside the tree). Owner rulings: every done-issue branch abandoned (49), open-issue and no-issue branches survive (47); the port leaves the cutover tool and is applied on main.
- Branch-only evidence sections of `4f395137`, `903dc74c`, `b5782224` carried onto main (`e0cd1b76f`); owner reword decisions recorded on `45ecf975` (`dc4e4e491`).
- `d1eba151`: Codex implementation; Claude rework 1; Codex rework 2 fixed a gate finding with a 130-line `env` parser; the steward approved a third, simple-rule attempt (Claude), which code-review failed on `env` forms the steward had scoped out; parked per his ruling.

## Next

1. On gf2-5c's request about 20 minutes before the window: recheck the lists (`final-lists-2026-11-15.md`) against the refs. During the window: no jit command, commit or worktree change.
2. After "window closed": jit runs through `scripts/tracker-env` from linked worktrees. Then `18276ff1` (settle the two review advisories, merge main, REQ-06 run with `run6.sh` only when gf2-5c confirms its cargo is idle, final gates, merge request to the steward with the gate set and the epics that receive `test-budgets`), `2a4c2684` (Codex by alternation), `84022ee3`.

## Traps

- Porting gate records needs their run files in the same step: `jit issue show` fails with "Failed to load gate runs" when a gate record names a run file the store lacks, and `jit validate` passes regardless. Check every issue loads.
- `check-landing` before the cutover refuses any tip whose verdicts were recorded on the branch; land on tracker-only drift with `check-references`, `check-tracked-budget` and `jit validate` at the tip.
- Quoting a reviewer finding that contains a dotted attribute (`result.stdout`) in issue prose trips the reference checker; paraphrase.
- A bare file name in issue prose (`code-task.json`) faults the reference checker; cite the store path.
- `jit dep add` refuses an edge that leaves the graph not transitively reduced; record an order in Notes instead of dropping the epic's direct edge.
- `cd` into a worktree inside a Bash call moves the session's directory; use subshells or `git -C`.
- Earlier traps in [handoff-2.md](handoff-2.md) still bind unless resolved above.

## Open questions for the invoker

None.

## References

- [plan.md](plan.md), [progress.json](progress.json); steward handoff `dev/active/2caf738d-1-0/handoff-5.md`; charter `dev/research/2caf738d-charter.md` (D-8 to D-12).
