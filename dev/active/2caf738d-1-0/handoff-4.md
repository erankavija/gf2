# Handoff: gf2 1.0 release (2caf738d), steward session 4

**Date:** 2026-11-15
**Lead handoffs:** one current handoff per epic directory, each written at this
wind-down: `dev/active/6ba9be17-tracker-repository/` (lead `gf2-e3`, the
carve-out), `dev/active/b4b4b9ee-tech-debt-2026-06-30/` (lead `gf2-7d`),
`dev/active/d77176e5-competitive-benchmarking/` (lead `gf2-f4`, also epic
`ae03bcd0`) and `dev/active/b8206228-permanent-statistics/` (lead `gf2-00`,
the `45ecf975` stream and the history rewrite). They hold the issue-level state.

## Current state

- Steward `gf2-96` ran four peer leads from 22:30 local on 2026-11-14. The owner narrowed the work in three steps (carve-out and rewrite first; then only those two; then only the carve-out) and ordered a full handoff at 01:35. This file was written before the last reports of `gf2-e3` and `gf2-7d` arrived; read their handoffs for what happened after.
- Nothing is pushed. All pushes are held until the history rewrite.
- The only work that resumes without a new owner word is the carve-out, epic `6ba9be17`. Every other stream is wound down with its branches parked.
- Closed on main this session: `6379e04e`, `2e6e3557`, `c34e8203`, `d1c0e062`; the planning and breakdown nodes of `6ba9be17` (`aa67438d`, `a49053be`) and of `3461a52a`. `fe2221d5` is rejected.
- The benchmark window of 2026-11-15 is cancelled and its timer stopped. The two `3461a52a` queue lines stay queued; no curve was measured.
- Charter decisions D-8 (tracker repository, cutover inside the rewrite window, `landed` gate, verdict reuse on identical declared inputs, no push in any criterion) and D-9 (time budgets as an epic gate) are logged.
- Free disk at the handoff: about 170 GB.

## Carve-out (6ba9be17)

- The plan passed plan-review after four failed reviews; the breakdown passed coverage-preview and breakdown-review. Ten issues are filed in six waves; `dev/active/6ba9be17-tracker-repository/plan.md` holds decisions D-01 to D-14.
- Wave 1 (`6d5ba9fb`, `28aa2a9d`) is implemented on branches and was in gating at the handoff.
- `2a4c2684` (carry the tracker through the rewrite) is implemented by the `45ecf975` stream lead on the carve-out lead's call, after `3fb4accb` and `6310679b` land. Its design is outside the tree; the stream's handoff names the location.
- `6889e6e6` (cutover execution) starts only on the owner's word. The plan places it in the rewrite's quiet window, and the rewrite is paused: its timing is an open owner question.
- The jit project plans native support for a separate store in jit v1.1 and declines a native landing condition; the plan's workarounds and the `landed` gate stay.

## Owner decisions of this session

- Parallelism is unrestricted by the steward. The leads hold themselves to two concurrent cargo-ci runs host-wide, the bound measured tonight.
- Tracker split: cutover inside the rewrite's quiet window; `@/inv/resolvable-references` takes the wording "alone or with the tracker checkout mounted at `.jit/`"; epic REQ-04 counts a verdict for identical declared inputs; REQ-06 binds repository-owned scripts; a branch-only document is linked pinned and re-linked when it lands; a required `landed` gate; the tracker repository is local and no criterion mentions pushing.
- `3461a52a`: a fourth planning attempt in place; it passed.
- `c34e8203`: contrived-byte findings are out of scope for the commit-msg hook under `@/inv/caller-trusted-fast-paths`.
- `18276ff1`: test processes hold pool tokens equal to their threads; task-level runs report a slow fast-tier test and do not kill it; the epic gate `test-budgets` enforces the budgets on a quiet host; the rayon gap is in scope. The issue is parked with its design recorded.
- Parked branches are carried through the rewrite as rewritten refs; the runbook's precondition becomes "merged, abandoned, or on the parked list".
- The window is cancelled; all streams but the carve-out are wound down; all sessions hand off.

## Steward rulings that bind the leads

- At most two cargo-ci runs host-wide until `18276ff1` is on main; a third run pushes gates past their 900 s timeout. The jit project runs one beside them.
- After any killed run or gate timeout, check for `cargo-pool-run` waiters with idle CPU and for the daemon's refill line in the pool log. Nobody but the owner touches the pool, its FIFO or its daemon.
- A gate record marked `inputs_moved` means the declared inputs changed during the run: do not merge main into a branch or edit its `.jit` while its gate runs.
- Merge main into a branch before a review or plan gate: a base older than `49c78483d` still tracks stale skill copies, and gates run them.
- A branch whose gate or review fails during a wind-down is parked and reported, with no rework.
- `574ed420` REQ-04 evidence is one pair (cold and warm, before and after), every run passing, the two sides back to back on a quiet host. The loaded first pair is an indication.
- `9a1b24fc`: the default tile counts the profile threshold only while the family is open and at or below a structural cap; the cap is a pure function of cache sizes and staging footprint, tested with injected values.
- `3461a52a`: DVB-T2 density evolution models the staircase parity structure; the plain-ensemble cap-200 threshold differs by 0.09 dB and is recorded as a caveat.
- Leads set each subagent's model by task difficulty, never Fable, alternate Claude and Codex per dispatch, and review with the other vendor.

## What to do next

1. Read the four lead handoffs. Confirm no cargo process of a closed session remains and the pool is full.
2. Resume the carve-out with one lead: wave 1 gates and landing, then waves 2 to 5. Bring the timing of `6889e6e6` to the owner when wave 5 is in sight.
3. Ask the owner which parked stream resumes first. `18276ff1` and `574ed420` remove the gate timeouts and the pool leak, so they are the steward's recommendation.
4. Put the open questions below to the owner.

## Traps

- The cargo pool bounds compile only in part: under sccache each `rustc` gets a private jobserver (`574ed420` fixes it), and a test process takes one token whatever its thread count (`18276ff1`). Sixteen concurrent cargo runs drove the load to 87 and every gate past its timeout.
- A killed test runner keeps its pool token. The daemon refills only when no client holds the lease, so queued runs keep an empty pool empty: the host sat at load 0.3 with 75 waiters until every session stopped its runs.
- A token wait counts against nextest's per-test timer, so a loaded host produces test timeouts that say nothing about the test.
- A direct `./scripts/cargo-ci.sh` has no 900 s limit: one ran for an hour under load and failed only on test timeouts.
- Landing a branch while main's working tree holds other sessions' uncommitted tracker files: build the merge with `git commit-tree` and `git update-ref`, check out only the changed paths, and merge an overlapping issue file field-wise. `git merge-file` inside an `&&` chain under `set -e` does not stop on a conflict; it left conflict markers in an issue file for a minute.
- A `cd` into a worktree inside a Bash call moves the session's working directory.
- A watcher started after the event it waits for reports a false alarm; check the log first.
- The planning and breakdown nodes do not move to done on a passing gate while they sit in `ready`; set the state by hand.
- A relayed owner decision that changes a criterion can be refused by a lead session's permission classifier; the lead then asks the owner in its own session.
- `systemd-run --user` and `systemctl --user` need `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS` set.

## Open questions for the owner

- Which parked stream resumes first, and when the history rewrite resumes.
- Timing of the cutover execution `6889e6e6` now that the rewrite is paused.
- Whether commit `49c78483d` (removal of the repository skill copies) is the owner's, and its reword to `chore(jit:2caf738d): ...` in the rewrite.
- The 17 no-issue branches (the stream lead recommends deleting all; the bundle keeps them) and five ambiguous subject rewords.
- Whether the plan-review verdict of `6ba9be17` stands with the two wording-only edits made after it; the steward accepted it because the later breakdown-review passed on the final text.
- The force-push of the rewritten history, and every ordinary push.
- `41517289`, filing the amdgpu report upstream, stays a deferred owner action.

## References

- Charter: `dev/research/2caf738d-charter.md`.
- Carve-out plan: `dev/active/6ba9be17-tracker-repository/plan.md`.
- Rewrite runbook: `dev/active/b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md`.
- Window runner and queue: `dev/scripts/bench-window/run-window.sh`, `dev/scripts/bench-window/queue.tsv`.
