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
- Every live branch is merged into `main`, deleted at step 2, or parked, by
  the owner's decisions of 2026-11-15 (steward gf2-96) and 2026-11-16
  (steward gf2-92). Parked is every unmerged branch of an open issue; it is
  carried through as a rewritten ref.
- Worktrees: the reseat recreates only `.agents/worktrees/agent-de311215` and
  `.agents/worktrees/agent-fd9d5416`, keeping their untracked built arms.
  Every other worktree is removed before the window, after a check of its
  ignored content (`git -C <path> status --porcelain --ignored`).
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
2. Delete the stale branches, which the bundle still holds: every branch
   merged into `main`, every branch without an issue, and every branch whose
   issue is done, rejected, or archived. The owner also named for deletion the
   superseded pre-replay branches `worktree-agent-243d1160` and
   `worktree-agent-7fbd1042`, whose `-a` successors stay parked, the backup
   branch `backup-506f76d4-e60912a40`, and the branches abandoned on
   2026-11-15, `worktree-agent-2bca2b28` and `wip-3461a52a-cb96b9810`. The list is
   derived from the refs and the tracker states before the rewrite, prepared
   outside the tree, and records each parked branch's head; a branch of
   doubtful state goes to the owner first.
3. Clone a fresh mirror and remove its `origin`. List blobs over 1 MiB absent from
   `main`'s tree into `strip-ids.txt`.
4. Rewrite:
   `git filter-repo --force --strip-blobs-with-ids strip-ids.txt --filename-callback`
   with `exec` of [rewrite_paths.py](rewrite_paths.py) and
   `return None if drop(filename) else filename`, and `--commit-callback` with
   the contents of [reword_subjects.py](reword_subjects.py), which rewords the
   subjects of the listed commits on `main` that break the commit convention.
   `drop` removes `.jit` and every path under it first.
5. `git gc --prune=now --aggressive`. Make every mirror branch a local branch
   in checkout `<rw>`:
   `git clone --bare <mirror> <rw>/.git`,
   `git -C <rw> config core.bare false`, then
   `git -C <rw> reset --hard`. This lets
   `scripts/check-references.py` resolve paths named in tracker prose against
   every local branch head. A parked branch whose ref is missing from the
   mirror, or whose `git diff main...<branch>` disagrees with the original
   outside `.jit` and the stripped blobs, stops the run. A head commit that
   only removed `.jit` paths, as the cutover's cleaning commit did, is pruned:
   its map entry is all zeros and the ref moves to its nearest kept ancestor.
6. Mount and remap in `<rw>`:
   - `scripts/tracker-bootstrap <tracker>`, where `<tracker>` is the primary
     checkout's `.jit`. The store declares gate-run records machine-local
     (`[storage] gate_run_records`), so a clone validates without them; the
     bootstrap's nonzero exit is accepted when its only findings are the
     `document-reference` errors of the unremapped pins.
   - Record `git -C .jit rev-parse HEAD`, run
     `scripts/tracker-env python3 dev/active/b8206228-permanent-statistics/45ecf975/map_pins.py <mirror>/filter-repo/commit-map <backup>/gf2.git`,
     commit the remapped pins in the tracker with `scripts/tracker-snapshot`,
     and record the tracker head again. A pin on a commit the rewrite pruned
     maps to its nearest kept first-parent ancestor, found in the step-1
     backup mirror; the tool prints both counts.
   - `scripts/tracker-env jit validate` passes.
   - Archive the map beside the step-1 bundle, outside both repositories:
     `gzip -n -c <mirror>/filter-repo/commit-map > <backup>/commit-map.gz`, then
     `digest=$(sha256sum <backup>/commit-map.gz | cut -d' ' -f1)` and
     `mv <backup>/commit-map.gz <backup>/commit-map-$digest.gz`.
7. Check on a quiet host in `<rw>` with the tracker of step 6 mounted:
   `scripts/tracker-env jit validate`, `./scripts/cargo-ci.sh`,
   `git for-each-ref | wc -l`, pack and tree sizes, and
   `git -C <mirror> log --all --format= --name-only -- .jit | wc -l`, which is 0.
8. Owner confirms with these numbers.
9. Push with `--no-verify`, because the pre-push hook would inspect unrewritten
   ranges: `git push --no-verify --force origin main`. `main` is pushed
   alone: `badges` on `origin` is a one-commit orphan the rewrite leaves
   unchanged, `origin` holds no tags, and the local tags are backups `origin`
   never had. The `Main protected` ruleset denies non-fast-forward pushes
   unless the account's repository role bypasses it. The push may follow
   step 10; no `git fetch` from `origin` runs between the reseat and the
   push, because it would bring the old history back.
10. Reseat the primary checkout:
    - Record `jit claim list`, the ID of `jit worktree info`, and for each kept
      worktree its `worktree` entry from `git worktree list --porcelain` plus
      the admin name from `basename $(git -C <path> rev-parse --git-dir)`.
    - Fast-forward the tracker:
      `git -C .jit fetch <rw>/.jit && git -C .jit merge --ff-only FETCH_HEAD`.
      The primary tracker is the one writable store; a non-fast-forward means
      a tracker write during the window and stops the step. Then
      `git -C .jit push origin main`.
    - Move `.git` aside, `git clone --mirror <mirror> .git`, set
      `core.bare false`, unset `remote.origin.mirror` and
      `remote.origin.tagopt`, set `remote.origin.fetch` to
      `+refs/heads/*:refs/remotes/origin/*`, delete every `refs/remotes/`
      ref (the rewritten copies of the old remote state), run `git reset` to
      fill the index, and copy `jit/` from the moved-aside `.git` into the new
      `.git` before any jit command. A mirror clone keeps every ref; a bare
      clone keeps only branches and tags, so it would drop `refs/archive/`,
      `refs/salvage/`, `refs/backup/` and `refs/claude/`. A clone carries
      neither `.git/info/exclude` nor the local `branch.*` and
      `core.logallrefupdates` settings: copy `info/exclude` from the
      moved-aside `.git` and set those keys again, then compare
      `git config --list --local` with the moved-aside `.git`'s. The
      ignored `.jit/` lies outside `.git` and stays in place;
      `git -C .jit rev-parse HEAD` equals the step-6 snapshot. The primary
      `.jit` keeps its own machine-local files.
    - Set `origin` and run `scripts/install-hooks.sh`.
    - Recreate the kept worktrees of the Preconditions, each without a `.jit`.
      The new `.git` has no `worktrees/` entries, so `git worktree repair`
      alone fails. Each keeps its untracked built arms in place: create a fresh
      empty parent `<tmpdir>`, set `<tmp>` to `<tmpdir>/<admin name>` (the
      recorded admin name, which can differ from the checkout's basename),
      and run `git worktree add --no-checkout <tmp> <branch>`, or
      `git worktree add --no-checkout --detach <tmp> <new>` for a detached
      worktree, where `<new>` is its recorded HEAD mapped as step 6 maps a pin.
      Move `<tmp>/.git` over `<path>/.git`, remove `<tmp>` and its parent,
      then `git worktree repair <path>` and `git -C <path> reset`; its status
      is then clean. A kept worktree with uncommitted tracked changes stops
      the reseat.
    - The recorded worktree paths and admin names are unchanged; the leases of
      `jit claim list` and the worktree ID equal the recorded ones, and
      `jit validate` passes.
11. Fix old commit IDs in the primary checkout and its tracker issue records.
    Run [old_ids.py](old_ids.py) from the primary checkout, using the archived
    map and original history. The commands below abbreviate its repository path
    as `<tool>` and `<backup>/commit-map-<digest>.gz <backup>/gf2.git . .jit <ex>` as `<args>`, where
    `<ex>` is
    `--exclude '*tuning-profiles/*.json' --exclude 'crates/gf2-coding/tests/data/tuning_profile_encode_family.json' --exclude 'crates/gf2-core/tests/tuning_profile_committed.rs' --exclude 'dev/tools/tuning-profile-compose/tests/golden/*' --exclude 'experiments/permanent-statistics/config/campaign.json' --exclude 'dev/active/b8206228-permanent-statistics/45ecf975/*'`.
    Excluded files keep their old IDs, because a digest or a rewrite tool's
    input covers them: the profiles' `assembly.content_sha256` and the
    `sha256` of records naming them, the test asserting a committed profile's
    `source_revision`, the preregistration's digest of `campaign.json`, and
    the original-ID keys of [reword_subjects.py](reword_subjects.py). The
    archived map resolves them.
    - Inventory: `python3 -B <tool> inventory <args> --out <backup>/old-ids.tsv`.
    - Apply the mechanical pass: `python3 -B <tool> apply <args>`.
      It replaces mapped IDs of at least 12 hex characters at the same width,
      and 7-11 hex commit references by the prefix of the new ID, lengthened
      to the shortest length unique among the kept new IDs; document pins
      belong to step 6.
    - Write the remainder: `python3 -B <tool> inventory <args> --out <backup>/old-ids-remainder.tsv`.
      It holds the pruned, unmapped, ambiguous and leftover short references.
      Hand that file to low-tier workers (Codex luna, Claude Haiku). Each worker
      rewrites a reference to its new ID, using the suggested kept ancestor for
      a pruned commit, or rewords it when the pruned commit was tracker-only.
    - Rerun the check after the workers: it must pass with zero mapped, short
      and pruned references outside the exclusions. Review the workers' edits
      and the remainder; commit the code changes with a subject of the
      form `chore(jit:45ecf975): ...`, and snapshot the tracker changes with
      `scripts/tracker-snapshot`, then `git -C .jit push origin main`. Code
      and tracker commit subjects, and the tracker's `events.jsonl`, are
      history and stay as written.
    - `scripts/tracker-env jit validate` and `./scripts/cargo-ci.sh` pass.
12. Verify a fresh clone's size, then ask GitHub Support to purge unreachable
    objects if old commits stay reachable by ID.

## Rehearsal

A rehearsal runs steps 1 to 7 on a scratch mirror of the repository as it
stands after the cutover, with a scratch tracker clone in place of the primary
`.jit`. Step 2 derives deletions from that mirror's refs and the scratch
tracker's issue states using the step-2 rules, then applies them to the scratch
mirror before step 3 builds the strip list. The rehearsal also runs step 10 on
a scratch copy of a checkout holding a mounted tracker and a `.git/jit/`. It
runs step 11's inventory, mechanical pass and check on those scratch copies,
dispatching no workers and pushing nothing.
It reports to the owner
through the issue:

- pack size, refs rewritten, commits on `main` and the compressed map size;
- the deleted and kept branch counts of step 2;
- `git -C mirror.git log --all --format= --name-only -- .jit | wc -l`, which is 0;
- the dropped counts of the old and the new `drop` over
  `git log --all --format= --name-only | sort -u` of the source, which differ
  only on `.jit` paths; the old `drop` is the copy in
  [rewrite_tools_test.py](rewrite_tools_test.py), which passes under
  `python3 -B`;
- the tracker head before and after the remap and the pinned commits mapped
  directly and through a pruned commit (the rehearsal's original is its
  backup mirror);
- the `jit validate` result in `<rw>` with the tracker mounted;
- the rehearsed reseat: tracker head kept, `jit claim list` and worktree ID
  unchanged, `jit validate` passing;
- old-ID inventory counts: total occurrences, mechanically mapped (full, 12+ hex and short), pruned
  targets, and remainder (pruned, ambiguous and unmapped); the check
  after the mechanical pass is expected to fail with zero mapped and exactly
  the remaining short and pruned references (ambiguous and unmapped references
  are reported but do not fail the check);
- `./scripts/cargo-ci.sh` in `<rw>` after step 11's mechanical pass passes;
- one line per parked branch: old head, new head, and whether
  `git diff --stat main...<branch>` before and after agree outside `.jit` and
  the stripped paths.
