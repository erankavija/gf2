# Plan: Move the tracker data into its own repository (6ba9be17)

> Planning node: aa67438d. Authoritative graph:
> [breakdown.json](breakdown.json).

## Outcome and criterion approach

The tracker becomes an independent git repository checked out at `.jit/` of
the primary checkout. The code repository ignores that path. Every checkout
on the host reads and writes the one store, so tracker state stops being
branch content.

| Criterion | Approach | Evidence / open gap |
|---|---|---|
| REQ-01 | The cutover commit removes the tracker paths from the index, ignores `/.jit/` and forbids tracked paths under it in the budget configuration | F-01, D-08 |
| REQ-02 | The tracker repository is extracted with the history of its paths; equality is the git tree identifier of `.jit` against the new head tree | D-10 |
| REQ-03 | One entry point resolves the store, the code root and the agent identity from git | `tracker-root-resolution`, F-02, F-04, F-07 |
| REQ-04 | A landing check compares each required gate's recorded commit with the branch tip | `landing-link`, F-03, F-08 |
| REQ-05 | Invariant identifiers resolve from the projection in `AGENTS.md`; tracker paths resolve against a mounted store | `tracker-reference-resolution`, F-11, F-12 |
| REQ-06 | Gate checkers read the data root and write logs under `target/` | F-14, F-15, F-09 |
| REQ-07 | A snapshot command commits under jit's write lock and pushes | `tracker-snapshot-discipline`, F-10 |
| REQ-08 | A mount command clones the tracker into place and validates it | D-09 |
| REQ-09 | A runbook and tool, rehearsed with rollback on a scratch clone | `cutover-sequence` |
| REQ-10 | The cutover commit carries the layout statements and the lower tree limit | D-08 |

## Shared architectural contracts

### `tracker-root-resolution` [plan-fixed] — store, code root and identity

The data root is `JIT_DATA_DIR` when set, otherwise `.jit` under the top
level of the current checkout. The entry point sets `JIT_DATA_DIR` to that
directory when it holds a store index, otherwise to `.jit` under the primary
checkout, the parent of the absolute git common directory. It sets
`JIT_WORKTREE_ROOT` to the top level of the invoking checkout. It keeps a
`JIT_AGENT_ID` the caller set, and otherwise derives `agent:<directory name>`
in a linked worktree. The store's configuration sets `[worktree] root = ".."`,
so plain jit is correct in the primary checkout. In a linked worktree plain
jit finds no store and fails (F-05); only the entry point reaches the store
from there, and it always sets the code root with it.

### `landing-link` [plan-fixed] — what ties a landed change to its issue

A code commit names its issue in its subject scope, the existing commit rule.
A gate-run record names the commit and branch its checker ran at (F-03). A
branch lands in this order: gates evaluated at the branch tip through the
entry point, the landing check, the merge, the merged-tree gate, the state
transition, a snapshot. Issue state and code are consistent at each landing
and are no longer one atomic merge; between landings the store is ahead of
`main` by the work in progress, which is what it describes.

### `tracker-snapshot-discipline` [plan-fixed] — commits of the tracker repository

jit commits nothing (F-10). The snapshot command validates the store, stages
and commits it under jit's write lock, and pushes. A lead runs it after each
landing and the steward at the end of a session; it is a no-op on a clean
store, so any agent may run it. Tracker commits are snapshots: the commit
subject rule and the tree budget of the code repository do not bind them. The
store has one writable checkout, on this host; the remote is its backup and
the source for a fresh clone.

### `tracker-reference-resolution` [plan-fixed] — references across the two repositories

`@/inv/<id>` resolves against the invariant projection in `AGENTS.md`, which
the code repository tracks (F-18). A path under `.jit/` named in tracked text
is a tracker reference: with the store mounted it must name a file of the
store, and without it the checker reports the count it left unchecked.
`@/issue/<id>` stays with the docs-mechanical gate, which needs jit (F-13).
Issue prose is checked whenever the store is mounted. GitHub runners have no
store and check everything else.

### `cutover-sequence` [plan-fixed] — order of the cutover

1. Quiesce: no tracker writer, every lease released, each live branch merged
   or its tracker changes merged into `main`, gate-run holders listed.
2. Back up the code repository as a bundle and the primary store as an archive.
3. Extract the tracker paths with their history into a new repository; require
   equal tree identifiers.
4. Mount: commit the code-side change, replace `.jit/` of the primary
   checkout with a clone of the new repository, restore the machine-local
   files, write the store's ignore rules, merge attribute and worktree root,
   snapshot and push.
5. Verify: repository validation, issue count, a gate from a linked worktree,
   two agent identities, both checkers with and without the store.
6. Clean each surviving worktree: merge `main` with tracker conflicts resolved
   by deletion, harvest its gate-run records, delete its local store.
7. Roll back on a failed step 5: revert the cutover commit, copy the store's
   files over the restored paths, remove the nested repository.

## Generated decomposition overview

<!-- jit:breakdown-overview:begin -->
| Key | Title | Type | Outcome | Contracts | Sources | Footprint | Landing | Depends on |
|---|---|---|---|---|---|---|---|---|
| tracker-entry-point | Run jit from any checkout through one entry point | task | One script gives each checkout the shared store with its own code root | tracker-root-resolution | REQ-03, D-04, F-02, F-04, F-05, F-07, F-16, F-21 | creates 2, touches 3 | — | — |
| landing-check | Refuse a landing whose gate runs record another commit | task | A landing is refused unless each required gate passed at the branch tip | landing-link | REQ-04, D-05, F-03, F-04, F-08 | creates 1, touches 3 | — | tracker-entry-point |
| gate-checkers-data-root | Resolve tracker files through the data root in the gate checkers | task | Gate checkers read the store through the data root and write nothing under a code checkout's store path | tracker-root-resolution | REQ-06, D-04, D-07, F-09, F-14, F-15, F-16 | touches 9 | — | — |
| reference-checker-tracker-references | Check tracker references against a mounted store | task | The reference checker passes without the tracker and checks tracker references when it is mounted | tracker-reference-resolution | REQ-05, D-03, F-11, F-12, F-17, F-18, F-22 | touches 1 | — | — |
| tracker-lifecycle-commands | Mount and snapshot the tracker repository with one command each | task | A fresh clone mounts the tracker with one command and the store is snapshotted with another | tracker-root-resolution, tracker-snapshot-discipline | REQ-07, REQ-08, D-06, D-09, F-10 | creates 2, touches 1 | — | tracker-entry-point |
| cutover-rehearsal | Write the cutover runbook and rehearse it on a scratch copy | task | A rehearsed runbook with its tool carries the tracker through the cutover and back | cutover-sequence, tracker-root-resolution, tracker-reference-resolution | REQ-09, D-01, D-07, D-08, D-10, F-01, F-05, F-06, F-07, F-19, F-20 | creates 2 | — | landing-check, gate-checkers-data-root, reference-checker-tracker-references, tracker-lifecycle-commands |
| cutover-execution | Execute the tracker cutover | task | The live repository runs on the separate tracker with no tracked or stale store left | cutover-sequence, tracker-snapshot-discipline, tracker-reference-resolution | REQ-01, REQ-02, REQ-07, REQ-09, REQ-10, D-01, D-02, D-03, D-06, D-08, D-10, F-01, F-19, F-20 | touches 9 | — | cutover-rehearsal |

```mermaid
flowchart LR
    N0["tracker-entry-point: Run jit from any checkout through one entry point"]
    N1["landing-check: Refuse a landing whose gate runs record another commit"]
    N2["gate-checkers-data-root: Resolve tracker files through the data root in the gate checkers"]
    N3["reference-checker-tracker-references: Check tracker references against a mounted store"]
    N4["tracker-lifecycle-commands: Mount and snapshot the tracker repository with one command each"]
    N5["cutover-rehearsal: Write the cutover runbook and rehearse it on a scratch copy"]
    N6["cutover-execution: Execute the tracker cutover"]
    N0 --> N1
    N0 --> N4
    N1 --> N5
    N2 --> N5
    N3 --> N5
    N4 --> N5
    N5 --> N6
```
<!-- jit:breakdown-overview:end -->

## Material risks and owner decisions

| Risk / decision | Resolution and rationale |
|---|---|
| D-01 Layout | Chosen by the owner on 2026-11-14: an independent repository mounted at `.jit/`. Rejected: a submodule, which needs a gitlink commit per tracker operation and conflicts on gitlinks across worktrees; an orphan branch with its own worktree and a subtree, which keep tracker objects and commits in the code repository; staying in-tree with `.jit/` excluded from the budget, kept only as the interim measure until the cutover. |
| D-02 Declarations | The whole store moves, including invariants, configuration, templates, rules and gate definitions: jit reads them from the data root only. The code repository keeps the invariant projection in `AGENTS.md`, so a change to an invariant still appears as a reviewed code commit. Rejected: links from the store into the code repository, which jit's root rules do not support. |
| D-03 References | See `tracker-reference-resolution`. The statement of `@/inv/resolvable-references` no longer holds literally for tracker paths. Proposed wording: "Tracked text names only what a fresh clone resolves, alone or with the tracker checkout mounted at `.jit/`: no host-local path and no repository path that does not exist." An invariant change is an owner decision; the cutover task applies the approved wording. Rejected: a tracked index of issue identifiers, which restores the churn this epic removes. |
| D-04 Environment | See `tracker-root-resolution`. Guards against the gate that records the wrong commit: plain jit fails in a linked worktree, the entry point never sets the store without the code root, and the landing check refuses a mismatched commit. Rejected: exporting the variables in each dispatch prompt, which every new dispatch path would have to repeat. |
| D-05 Landing | See `landing-link`. Rejected: recording code commits in tracker commits or in a ledger, a second copy of what the commit subject and the gate-run record state. |
| D-06 Snapshots | See `tracker-snapshot-discipline`. Rejected: a commit per jit mutation, which contends for the tracker's index across concurrent agents; a timer, which commits without a validated state. |
| D-07 Gate runs | One shared, uncommitted `gate-runs/` directory replaces the copying between worktrees. The records hold 46 MB of checker output on this host and are execution records under `@/inv/committed-evidence-budget`; the cutover harvests each worktree's records into the store. Kani logs move to `target/`. |
| D-08 Budgets | The code repository forbids tracked paths under `.jit/`, drops its tracker exceptions and lowers the tree limit to the measured size. The tracker repository has no size budget: it is the record. Its growth is bounded by request U-5 below. |
| D-09 Bootstrap | The mount command derives the tracker URL from the `origin` URL with `-tracker` appended and accepts another as argument. Without the tracker a clone builds, tests and passes CI; only jit is unavailable. |
| D-10 Timing | Chosen: cut over in the quiet window of the history rewrite and before its extraction step. Both need every branch settled and no session, so one window serves both, and the rewrite then drops the tracker paths from history. Before the window: possible, at the cost of settling all branches twice. After the rewrite: the rewritten history keeps the tracker paths until a second rewrite. The rewrite prunes the commits that touch only tracker paths, about a third since 2026-10-31; its commit map covers them. |
| RISK-01 Shared identity | Checkouts sharing the store share one worktree identity (F-07); a lease is attributed by agent identity alone. The entry point derives a distinct identity per checkout; request U-4 removes the dependence. |
| RISK-02 Unverified jit behaviour | F-04, F-05 and F-07 rest on documentation and one experiment. The rehearsal re-checks them and records any contradiction before the runbook changes. |
| RISK-03 Vendored skills | The dispatch scripts and skill texts under `.agents/skills/` come from the jit project and name jit without the entry point. `AGENTS.md`, which every agent loads, carries the rule; request U-7 moves it upstream. |
| RISK-04 Concurrent store | All sessions now write one store. jit serializes writers with its store lock; a tracker change is visible to every lead at once, without review by merge. |

### Requests to the jit project

Separate from the plan; nothing above depends on them.

- U-1: Discover the store of the primary checkout from a linked worktree and
  use the invoking checkout as code root. Retires the entry point.
- U-2: Refuse a gate evaluation whose invocation directory lies in a checkout
  other than the resolved code root.
- U-3: Report the code root in `jit worktree info`.
- U-4: Keep a worktree identity per checkout for a nested independent store.
- U-5: Rotate or compact the event log.
- U-6: Commit the data repository after a mutation, or run a configured hook.
- U-7: Have the dispatch scripts export the tracker environment.

## Investigation sources

- [Investigation](investigation.md): findings F-01 to F-22, the consumers of
  tracker paths and the facts not verified.
