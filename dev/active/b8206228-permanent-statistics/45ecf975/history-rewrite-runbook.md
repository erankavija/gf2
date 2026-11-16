# History rewrite runbook

Removes copied inputs, raw output, large unreferenced blobs and the former
tracker paths under `.jit` from all refs of the code repository.
The force-push waits for the owner's confirmation in `@/issue/45ecf975`.

The tracker is its own repository, mounted at the ignored `.jit/` of the
primary checkout; its home is `git -C .jit remote get-url origin`. jit keeps
claims, leases, locks and the worktree identity under the code repository's
`.git/jit/`, outside the tracker.

## Preconditions

- The window has ended and no session or timer is active.
- The tracker cutover (`@/issue/6889e6e6`) has been executed and verified on
  the live repository: `git ls-files .jit` is empty on `main` and on every
  surviving branch, and its snapshot precedes the step-1 backup.
- Every live branch is merged into `main`, abandoned, or on the parked list,
  by the owner's decision of 2026-11-15, relayed by steward gf2-96.
- The Rehearsal below (REQ-06 of `@/issue/2a4c2684`) has passed from the
  current `main`.
- All worktrees are clean or their untracked files are recorded below. A
  parked worktree has its tracked changes committed.
- `pip install git-filter-repo` in a private virtual environment.

## Steps

1. Back up to `~/.local/share/gf2/pre-rewrite/`: `git clone --mirror` to
   `gf2.git`, `git bundle create gf2.bundle --all`, then `git bundle verify`.
   For the tracker, `git -C .jit bundle create <backup>/tracker.bundle --all`,
   `git clone --mirror .jit <backup>/tracker.git`, and
   `git -C <backup>/tracker.git bundle verify <backup>/tracker.bundle`.
   External archives stay in `~/.local/share/gf2/external-inputs/`.
2. Delete the stale branches, which the bundle still holds: every branch not
   merged into `main` whose issue is done or that has no issue, except those a
   live issue or queue line uses and the parked branches. Remove their worktrees
   with them. The list is prepared outside the tree before the rewrite, records
   each parked branch's head and worktree path, and branches of doubtful state
   go to the owner first.
3. Clone a fresh mirror and remove its `origin`. List blobs over 1 MiB absent from
   `main`'s tree into `strip-ids.txt`.
4. Rewrite:
   `git filter-repo --force --strip-blobs-with-ids strip-ids.txt --filename-callback`
   with `exec` of [rewrite_paths.py](rewrite_paths.py) and
   `return None if drop(filename) else filename`, and `--commit-callback` with
   the contents of [reword_subjects.py](reword_subjects.py), which rewords the
   subjects of the listed commits on `main` that break the commit convention.
   `drop` removes `.jit` and every path under it first.
5. `git gc --prune=now --aggressive`. Clone the mirror to a checkout, `<rw>`.
   A parked head whose entry in `filter-repo/commit-map` is all zeros stops the
   run.
6. Mount and remap in `<rw>`:
   - `scripts/tracker-bootstrap <tracker>`, where `<tracker>` is the primary
     checkout's `.jit`. A nonzero exit is accepted only when its findings are
     `gate-run-reference` alone: gate-run records are machine-local and absent
     from a clone.
   - Copy `gate-runs/` and `gate-artifacts/` of the primary `.jit` (read only)
     into `<rw>/.jit`; `scripts/tracker-env jit validate` then passes.
   - Record `git -C .jit rev-parse HEAD`, run
     `scripts/tracker-env python3 dev/active/b8206228-permanent-statistics/45ecf975/map_pins.py <mirror>/filter-repo/commit-map`,
     commit the remapped pins in the tracker with `scripts/tracker-snapshot`,
     and record the tracker head again.
   - In the code repository, add the gzipped map as
     `dev/history-rewrite-commit-map.gz`, state in `AGENTS.md` that older
     recorded commit IDs resolve through it, and commit those two paths only.
7. Check on a quiet host in `<rw>` with the tracker of step 6 mounted:
   `scripts/tracker-env jit validate`, `./scripts/cargo-ci.sh`,
   `git for-each-ref | wc -l`, pack and tree sizes, and
   `git -C <mirror> log --all --format= --name-only -- .jit | wc -l`, which is 0.
8. Owner confirms with these numbers.
9. Push with `--no-verify`, because the pre-push hook would inspect unrewritten
   ranges: `origin` holds `main` and `badges`, so the push is
   `git push --no-verify --force origin main badges` plus tags. The
   `Main protected` ruleset denies non-fast-forward pushes unless the account's
   repository role bypasses it.
10. Reseat the primary checkout:
    - Record `jit claim list` and the ID of `jit worktree info`.
    - Fast-forward the tracker:
      `git -C .jit fetch <rw>/.jit && git -C .jit merge --ff-only FETCH_HEAD`.
      The primary tracker is the one writable store; a non-fast-forward means
      a tracker write during the window and stops the step.
    - Move `.git` aside, clone the rewritten mirror, and copy `jit/` from the
      moved-aside `.git` into the new `.git` before any jit command. The
      ignored `.jit/` lies outside `.git` and stays in place;
      `git -C .jit rev-parse HEAD` equals the step-6 snapshot. The primary
      `.jit` keeps its own machine-local files.
    - Set `origin` and run `scripts/install-hooks.sh`.
    - Recreate needed worktrees at the mapped commits, each without a `.jit`.
      For pinned and parked worktrees, keep the untracked built arms in place
      with `git worktree repair <path>` and
      `git -C <path> reset --soft <mapped head>`. A parked worktree with
      uncommitted tracked changes stops the reseat.
    - `jit claim list` and the worktree ID equal the recorded ones, and
      `jit validate` passes.
11. Verify a fresh clone's size, then ask GitHub Support to purge unreachable
    objects if old commits stay reachable by ID.

## Rehearsal

A rehearsal runs steps 1 and 3 to 7 on a scratch mirror of the repository as
it stands after the cutover, with a scratch tracker clone in place of the
primary `.jit`, and step 10 on a scratch copy of a checkout holding a mounted
tracker and a `.git/jit/`. It reports to the owner through the issue:

- pack size, refs rewritten, commits on `main` and the compressed map size;
- `git -C mirror.git log --all --format= --name-only -- .jit | wc -l`, which is 0;
- the dropped counts of the old and the new `drop` over
  `git log --all --format= --name-only | sort -u` of the source, which differ
  only on `.jit` paths; the old `drop` is the copy in
  [rewrite_tools_test.py](rewrite_tools_test.py), which passes under
  `python3 -B`;
- the tracker head before and after the remap and the pinned commits mapped;
- the `jit validate` result in `<rw>` with the tracker mounted;
- the rehearsed reseat: tracker head kept, `jit claim list` and worktree ID
  unchanged, `jit validate` passing;
- one line per parked branch: old head, new head, and whether
  `git diff --stat main...<branch>` before and after agree outside `.jit` and
  the stripped paths.
