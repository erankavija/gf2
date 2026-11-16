# Handoff: Move the tracker data into its own repository (6ba9be17), session 5, defect line

**Date:** 2026-11-17
**Prior handoffs:** [handoff.md](handoff.md), [handoff-2.md](handoff-2.md), [handoff-3.md](handoff-3.md), [handoff-4.md](handoff-4.md) (cutover line).

## Current state

- Lead `gf2-54` under steward `gf2-92`, assigned to fix the defects the post-cutover flow exposed. The epic is assigned to `agent:gf2-54`. Nothing is pushed in the code repository; the tracker home is current.
- Landed on `main` and done:
  - merge `5b4539c7b` carries `c4d14911`, `a5773e5b`, `c568cb92`, `63813d52`, `a6d0c14a`, `673cd2bc`, `0cb65df5` and `3f10673e`;
  - merge `ecf17069f` carries `8660c096`;
  - `8d6a135f` (worktree retirement) has no code and is done.
- Parked by the owner's order until the history rewrite, all `ready`:
  - `a9ddca84`: amend plan decision D-12 to a tracker home that leads keep current.
  - `83ca8ca5`: reference checker in a linked worktree. It goes after gf2-8a's `da6200b8`.
  - `d1eba151`: interpreter-line classes in the reference checker. The steward records the owner's ruling on it, which allows two forms. It goes after `da6200b8` and `83ca8ca5`.
  - `1be3446b`: act on a leased issue from the primary checkout as its lease holder.
  - `581481c0`: reconcile the profile package sources with the live state. The owner schedules it, and nobody applies the profile until it is done.
  - `121f55fb`: state the landing order for a store-sourced projection change.
- Not this line's: `15f56b64` (waits for a jit install), `2a4c2684`, `84022ee3` and `23c47bea` (gf2-8a), `d1eba151`'s parked branch. `e610f5c4`, `6b9e6b14` and `6889e6e6` are rejected by the owner.
- Leases: none held by `agent:gf2-54`.
- Worktrees: `agent-6ba9be17-fix` and `agent-8660c096-b` hold merged branches and warm build caches. Reclaim them when no build runs.

## What happened

- Filed and fixed the reported defects:
  - `tracker-env` adopts the issue's lease holder for gate commands;
  - `tracker-snapshot --push`, a `Jit-Agent` trailer, and a line naming the commit or the clean head;
  - the landing flow with the merged-tree step is defined in `dev/index.md`;
  - the indefinite-lease limit is stated;
  - the profile manifests are at version 2, so a profile diff of `packages/sim-research` runs again;
  - the stale commit rule is dropped.
- Falsified, recorded on `a6d0c14a`: the brief's "one indefinite lease per agent". jit's default is two, under `[coordination] max_indefinite_leases_per_agent`.
- Review rounds:
  - `a5773e5b` code review: global options and `evaluate-many` bypassed the lease lookup; fixed.
  - `673cd2bc` doc review, twice: step 1 gated `landed`, and the push step conflicted with plan D-12. The steward then refined the merged-tree ruling to cargo-ci's input counting.
  - `8660c096` code review: a mutable headroom allowed a second reset; fixed. The stale projection was resolved by editing the store before the rerun (steward option (a)).
- `8660c096`: the tree limit is reset once to the measured tree, 20611491 bytes, plus 262144 (owner decision, 2026-11-16). From then on every change must equal the tree plus that headroom.
- Owner decision 3: six detached worktrees were removed and two kept with their stale `.jit` deleted under skip-worktree. The listing is in the cutover records directory.

## Next

1. On the steward's word after the jit install and the rewrite, take the parked items in the order above. `83ca8ca5` and `d1eba151` go after `da6200b8`, and each extends its checker's self-test.
2. `121f55fb` states the store-sourced projection order in the landing flow. The order: the tracked copy on the branch, the store entry when the branch enters review, then the merge promptly.
3. Close the epic once its children are done. Reconcile `progress.json`'s `surfaced_pitfalls` against the epic criteria first.

## Traps

- **A new worktree's build cache must be seeded before a gated cargo-ci.**
  - Evidence: two gate runs in cold worktrees hit the 900 s checker timeout and recorded `error`.
  - Instead: seed the cache, or run the build once before the gate. Warm runs took 6 to 8 minutes.
  - The owner raised the `cargo-ci` gate timeout to 1800 s on 2026-11-17; the steward commits the package source.
- **Killing a cargo or test run drains the host jobserver pool.**
  - Evidence: a killed client keeps its tokens. The pool daemon refills only when no client holds the lease, and with several sessions building that never happens. On 2026-11-16 the pool fell to 0, and every test then waited for tokens until the gate timed out.
  - Instead:
    - Kill a run only when it is hung, and then also kill its orphaned `cargo-pool-run` processes, matched by working directory.
    - The steward pauses the other sessions so the pool refills.
    - Chain no second cargo-ci after a run that may time out.
- **Gate runs count only where they ran.** `code-review`, `doc-review` and `docs-mechanical` declare no inputs, so any commit after them makes every review rerun. Freeze the tip before the reviews, and settle doc rulings first.
- **A store-sourced projection cannot land atomically** (`121f55fb`).
  - Evidence: between the store edit and the merge, `main`'s `AGENTS.md` reads as stale, and `jit validate` fails in the primary checkout with derived-state drift.
  - Instead: nobody runs `jit validate --fix` there in that window.
- **The zsh Bash tool does not word-split a command held in a variable.** Use a function.
- **The removal safety check refuses `rm -rf` on a path built from variables.** Use literal absolute paths.

## Open questions for the invoker

- Whether the skill's `land.sh` defers to the repository flow. The steward prepares that edit for the owner.
- The jit documentation's `[coordination]` example (3 and 12) disagrees with its code defaults (2 and 10). The steward passed this to the jit session.

## References

- [plan.md](plan.md), [progress.json](progress.json), the landing flow in `dev/index.md`.
- Steward rulings and owner decisions of 2026-11-16 are recorded on each issue named above.
