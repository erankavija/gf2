# History rewrite runbook

Removes copied inputs, raw output and large unreferenced blobs from all refs.
The force-push waits for the owner's confirmation in `@/issue/45ecf975`.

## Preconditions

- The window has ended and no session or timer is active.
- Epic `@/issue/55c3e433` has committed what it keeps from its pilots, and its
  branches `worktree-agent-1d787ed3`, `worktree-agent-a72f2b3f` are final.
- All worktrees are clean or their untracked files are recorded below.
- `pip install git-filter-repo` in a private virtual environment.

## Steps

1. Back up: `git clone --mirror` to `~/.local/share/gf2/pre-rewrite/gf2.git`,
   and `git bundle create ~/.local/share/gf2/pre-rewrite/gf2.bundle --all`, then
   `git bundle verify`. External archives stay in
   `~/.local/share/gf2/external-inputs/`.
2. Clone a fresh mirror and remove its `origin`. List blobs over 1 MiB absent from
   `main`'s tree into `strip-ids.txt`.
3. Rewrite:
   `git filter-repo --force --strip-blobs-with-ids strip-ids.txt --filename-callback`
   with `exec` of [rewrite_paths.py](rewrite_paths.py) and
   `return None if drop(filename) else filename`.
4. `git gc --prune=now --aggressive`. Clone the mirror to a checkout.
5. In that checkout run [map_pins.py](map_pins.py) on `filter-repo/commit-map`, add
   the gzipped map as `dev/history-rewrite-commit-map.gz`, state in `AGENTS.md`
   that older recorded commit IDs resolve through it, and commit.
6. Check on a quiet host: `jit validate`, `./scripts/cargo-ci.sh`,
   `git for-each-ref | wc -l`, pack and tree sizes.
7. Owner confirms with these numbers.
8. Push with `--no-verify`, because the pre-push hook would inspect unrewritten
   ranges: `git push --force origin main` plus the existing remote branches and
   tags. The `Main protected` ruleset denies non-fast-forward pushes unless the
   account's repository role bypasses it.
9. Reseat the local repository: move `.git` aside, clone the rewritten mirror, set
   `origin`, run `scripts/install-hooks.sh`, and recreate needed worktrees at the
   mapped commits, keeping the untracked built arms of pinned worktrees in place
   with `git worktree repair` and `git reset --soft <mapped>`.
10. Verify a fresh clone's size, then ask GitHub Support to purge unreachable
    objects if old commits stay reachable by ID.

## Rehearsal

A rehearsal runs steps 1 to 6 on a scratch mirror and reports pack size, refs
rewritten, commits on `main`, pinned document commits mapped, the
`jit validate` result and the compressed map size to the owner through the
issue.
