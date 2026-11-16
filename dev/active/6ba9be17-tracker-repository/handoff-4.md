# Handoff: Move the tracker data into its own repository (6ba9be17), session 4, cutover line

**Date:** 2026-11-16
**Prior handoffs:** [handoff.md](handoff.md) (session 1), [handoff-2.md](handoff-2.md) (session 2), [handoff-3.md](handoff-3.md) (session 3, supporting line).

## Current state

- Lead `gf2-5c` under steward `gf2-a2`. The supporting line (`gf2-f6`) closed in session 3. The epic and `e610f5c4`, `6b9e6b14` and `6889e6e6` are assigned to `agent:gf2-5c`. Progress: [progress.json](progress.json).
- **The tracker cutover ran on 2026-11-16, 01:43 to 01:52 local**, and passed every verification check. `.jit/` of the primary checkout is the checkout of the tracker repository. Its home is `git -C .jit remote get-url origin`, a local bare repository (owner, through the steward). `main` tracks no path under `.jit/`.
- Gate state at this handoff: `cargo-ci` and `landed` pass on `e610f5c4`, `6b9e6b14` and `6889e6e6`, and `docs-mechanical` passes on `6889e6e6`. `code-review` (Codex) fails on all three; the findings are recorded on each issue (`jit gate status <id> code-review --findings`), and all three stay open. `6b9e6b14` and `6889e6e6` read `backlog`: their dependencies were open when the owner ordered the cutover run first (steward, 2026-11-16), so jit never promoted them. Their descriptions record the order of events.
- No worker runs. Nothing is pushed in the code repository.

## Cutover record

- Commits on `main`:
  - `54b4491a2`: merge of the tool;
  - `01a338f5a`: merge of the runbook;
  - `641fa5a46`: P, the owner's go;
  - `0ad613715`: A, the invariant amendment;
  - `c0ea7b369`: C, the cutover commit;
  - `28c472a14`: merge of the `6889e6e6` guidance.
- Tracker:
  - extracted with 3532 commits; its tree `5055cdd2` equals `A:.jit` and the mount;
  - 6667 machine-local files restored at the mount;
  - snapshots since the window, the last pushed by the steward to the home.
- Verification:
  - `jit validate` passes; every issue loads; 0 missing run files; 1411 issues before and after.
  - A `docs-mechanical` gate run through `scripts/tracker-env` from a linked worktree recorded that worktree's commit and branch.
  - Both checkers pass in the primary checkout and in a fresh clone without the store.
  - A fresh clone's mount fails validation only with `gate-run-reference` errors, the accepted result (`@/issue/6889e6e6` REQ-09).
- Branches:
  - 46 done-issue branches abandoned to `refs/archive/<name>`;
  - 2 merged branches deleted in the window, the tool's and the runbook's. The steward deleted the other merged branches before the window.
  - 22 parked survivors cleaned, 4 already clean; 19 survivors cleaned in their worktrees;
  - 46 local branches remain and none tracks `.jit`.
- Worktrees:
  - 15 stores harvested with no collision;
  - 3 worktrees dirty only under `.jit` discarded after `port_check`, because the port commits on `main` hold their content;
  - 5 clean detached worktrees removed;
  - 8 detached worktrees kept with stale `.jit` copies, the named exceptions to the epic's REQ-09, because they hold ignored content other than build output: `agent-de311215`, `agent-fd9d5416`, `jit-67194d70-run`, `jit-be6d6f45-run`, `req02-574-2`, `req02-574-3`, `req03-574`, and `agent-e610f5c4` (caches only);
  - 28 worktrees remain.
- Dispositions recorded in the window log, all steward rulings:
  - older branch-committed runs on `d73ac633` and `e29fd776`: superseded;
  - `18276ff1`'s `test-budgets` gate: it lands with that issue;
  - `receipt/86185f88`'s three gates: abandoned under the no-issue rule;
  - remainders on `22bb2dcd` and `506f76d4`: superseded under the port rule.
- The `landed` gate is on all 121 open code-producing issues. The `code-task` preset is rewritten as gate keys (`@/issue/84022ee3`).
- Records are kept outside the tree, in the cutover directory under the host's gf2 data directory, which the steward knows:
  - the window log;
  - the ref and worktree listings;
  - the store archives with their manifest;
  - the extraction;
  - the steward's all-refs bundle.

## Working rules for every session

- In a linked worktree, run jit only as `scripts/tracker-env jit ...`. Plain jit there finds the primary checkout's store, but takes the primary checkout as code root, so a gate would record the wrong commit (plan F-05 falsified, recorded in `@/issue/6889e6e6`).
- Landing flow:
  1. gates at the branch tip through `scripts/tracker-env`;
  2. `scripts/check-landing <issue> <tip>`;
  3. merge;
  4. the merged-tree gate;
  5. `scripts/tracker-snapshot`;
  6. the `landed` gate.
- `scripts/tracker-snapshot` commits the store locally; `git -C .jit push origin main` keeps the home current.
- A fresh clone mounts the tracker with `scripts/tracker-bootstrap "$(git -C <primary-checkout>/.jit remote get-url origin)"`.
- `AGENTS.md` and `dev/index.md` state these rules.

## What happened

- Took over from `gf2-bf`. The owner's order on branch cleaning became charter D-12: removal-only cleaning, abandon to `refs/archive/`, and merged branches deleted.
- Criteria were reworded several times on the owner's orders, each in its own tracker commit:
  - D-12 (`c15d7e5dd`);
  - REQ-11 `port`, added then withdrawn (`b2a8192a8`, `9d2fed2b5`);
  - the forward path only (`9d2fed2b5`);
  - the rehearsal falsifications (`85bc7e448`);
  - REQ-08 of `e610f5c4`, which excepts the code-side edits.
- The marker-based final round of `e610f5c4` was stopped, and a second worker kept the marker machinery after being redirected. Both were replaced. The forward-only tool `adf7406f3` came from a fresh worker in about 30 minutes; its self-test ran 130 checks in 3 s.
- Rehearsals on scratch clones of the real repository:
  - an early timing pass;
  - forward run 1, which found F2 to F10;
  - forward run 2, clean, including a literal rollback.

  Results are in the rehearsal reports in the session's data directory.

## Next

1. Fix forward the code-review findings, each a recorded defect, none affecting the live store or refs (the lead verified that every run record of a deleted worktree store is in the shared store):
   - `e610f5c4`, six findings on the tool's code, moot since the tool is retired; the issue closes as delivered and retired or is rejected, with the owner.
   - `6b9e6b14`, five findings in runbook text: the discard checks; a bundle fallback that lacks the ignored records; worktree removal after a failed harvest; the final branch checks; the rehearsal report, now appended to the issue.
   - `6889e6e6`, two findings: the eight detached worktrees with stale stores (REQ-03, which needs the owner's acceptance or the worktrees' cleanup) and the self-test above.
2. A finding of tonight goes forward as a recorded finding: the issue stays open, with no rework dispatched in the window session.
3. The epic `6ba9be17` closes when its children are done. Reconcile `surfaced_pitfalls` in [progress.json](progress.json) against the epic's criteria first. REQ-09 carries the eight named worktree exceptions; they need the owner's acceptance or their owners' cleanup.

## Traps

- The cutover tool is retired (steward ruling): commit `c0ea7b369` holds it, and the runbook runs it from there. Its self-test fails on any commit after the cutover, because its fixture copies `.gitattributes` from `HEAD`. The successor decides with the owner whether `@/issue/e610f5c4` closes as delivered and retired or is rejected.
- The tree-size limit set at the cutover leaves no room under "limits only fall"; `@/issue/8660c096` asks the owner for a headroom rule. Retiring the tool freed the room for this handoff.
- The host's jit binary changed at 01:10 on 2026-11-16 and still reports `1.0.0`. It validates gate-run records and reads only key-only presets. `jit profile diff` now fails on `packages/jit-default` (manifest version 1, expected 2); a projected profile region in `AGENTS.md` was therefore checked by diff against its source, not by jit. The jit session holds further installs until the steward sends "gf2 cutover done". A future jit install on this host is announced to gf2 first.
- The runbook's rollback items 5 and 8 name no command for the manifest check. `cutover archive --repo . --dest <backup>/stores --verify` is that check.
- `scripts/tracker-env jit gate evaluate` leaves `.jit-bootstrap.lock` at the worktree's top level; `worktree_remove` in the runbook allows it.
- A cutover-era rule: a gate run with the executor's lease needs `JIT_AGENT_ID` set to the lease holder, or jit refuses the change on that issue.
- The Bash tool's working directory follows a `cd` into a worktree. Use `git -C` and absolute paths.

## Open questions for the invoker

- Whether `scripts/tracker-snapshot` pushes to the tracker's home by itself. It commits only locally today, and the steward pushed by hand. Recommendation: a follow-up task that adds an opt-in push after the commit.
- The eight detached worktrees kept with stale `.jit` copies: accept them as REQ-09 exceptions, or ask their owners to clean them.

## References

- [plan.md](plan.md), [progress.json](progress.json), [cutover-runbook.md](cutover-runbook.md), [handoff-3.md](handoff-3.md).
- Charter `dev/research/2caf738d-charter.md`, D-10 to D-12.
