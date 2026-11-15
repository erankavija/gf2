# Tracker cutover runbook

Moves `.jit/` of the code repository into an independent tracker repository
mounted at the same path of the primary checkout, carries every surviving
worktree's tracker state through the move, and undoes it by git steps after a
failed verification. The order is `cutover-sequence` in [plan.md](plan.md);
the execution issue is `@/issue/6889e6e6`. The history rewrite runs later, in a
window of its own, under the
[history rewrite runbook](../b8206228-permanent-statistics/45ecf975/history-rewrite-runbook.md),
and is outside this runbook.

## Conventions

Every command runs from the top level of the primary checkout unless a step
names another directory. The shell may be `sh`, `bash` or `zsh`; commands quote
every expansion. The cutover tool is
`dev/active/6ba9be17-tracker-repository/cutover.py` (task `@/issue/e610f5c4`);
its docstring is the reference for each subcommand's arguments, refusals and
exit statuses (0 done, 1 failed or action needed, 2 refused). It is the forward
path only; the rollback is git. A shell function shortens the calls:

```sh
epic=dev/active/6ba9be17-tracker-repository
cutover() { python3 -B "$epic/cutover.py" "$@"; }
```

The issue-load check runs in the checkout whose `.jit/` it reads. It runs
`jit issue show` for every id of `jit query all --json`, and scans every
`gates_status` entry that names a run for its file
`.jit/gate-runs/<issue>/<run>.json`. `jit validate` passes while an issue whose
entry names a missing run fails to load ("Failed to load gate runs"). With
`strict` the check passes on zero load failures and zero missing run files.
With `report` it prints both lists and passes when every load failure is an
issue that names a run, as `gate-runs/` is ignored by the store's
`.gitignore` and absent in a fresh clone.

```sh
load_check() { python3 -I - "$1" <<'EOF'
import json, subprocess, sys
from pathlib import Path
mode = sys.argv[1]
ids = [i["id"] for i in json.loads(subprocess.run(
    ["jit", "query", "all", "--json"], capture_output=True, text=True,
    check=True).stdout)["issues"]]
fail = [i for i in ids if subprocess.run(
    ["jit", "issue", "show", i], capture_output=True).returncode]
named, missing = set(), []
for i in ids:
    gates = json.loads(Path(".jit/issues/%s.json" % i).read_text()).get("gates_status", {})
    for g in gates.values():
        if g.get("run_id"):
            named.add(i)
            if not Path(".jit/gate-runs/%s/%s.json" % (i, g["run_id"])).is_file():
                missing.append(i)
print("load failures:", len(fail), "missing run files:", len(missing))
for i in fail:
    print("load failure", i)
bad = fail + missing if mode == "strict" else [i for i in fail if i not in named]
sys.exit(1 if bad else 0)
EOF
}
```

Arguments the executor fixes before the window and records in the window log:

| Argument | Meaning |
|---|---|
| `<backup>` | A new directory outside every checkout and outside the git directory, on the filesystem of the primary checkout. It holds the ref listing, the store archives, the moved-aside stores, the probe checkouts and the window log. |
| `<tracker-repo>` | The path the new tracker repository is created at, outside every checkout; it does not exist yet. |
| `<abandon-list>` | A file of branch names, one per line, `#` comments allowed: the "Abandon" section of the steward's final lists, rechecked right before the window. |
| `<survivor-list>` | A file of branch names in the same form: the branches the steward's final lists keep. |
| `<bundle>` | An existing all-refs bundle the steward names, used only when the archive fails. |
| `<executor>` | The executor's agent identity, `agent:<name>`. |

`TMPDIR` is exported as `<backup>/tmp`, so the temporary worktrees of the
branch clean lie outside every tracked tree. The window log is
`<backup>/window.md`: every command of a step, its output and its exit status
are appended to it. It stays outside the tracked tree
(`@/inv/committed-evidence-budget`); the execution issue cites its values.

A subcommand that fails or refuses changes nothing it does not name; the
executor reads its message and either corrects the cause and reruns the
subcommand or starts the [rollback](#rollback).

## Preconditions

- `jit --version` prints `jit 1.0.0 (profile release)`.
- The tool, `scripts/tracker-env`, `scripts/tracker-bootstrap`,
  `scripts/tracker-snapshot`, `scripts/check-landing`, the reference checker
  that resolves tracker references through the data root, the `landed` gate
  and the `code-task` preset are on `main` or on the branches step 0 merges.
- The port of branch-only tracker edits is on `main` (gf2-f6, `97b8e84a9`).
- `<abandon-list>` (rechecked by gf2-f6 right before the window) and
  `<survivor-list>` exist as files.
- `.jit/references.toml` names the upstream Sionna files as full URLs (a
  tracker commit on `main`), so the reference checker passes once step 6's
  code-side commit drops its exception.
- The steward's "go" message has arrived and the owner has confirmed.
- The quiet window is announced to every active session through the steward,
  with its start and expected length (TBD-rehearsal: wall time of the steps on
  the scratch clone, scaled to the live number of worktrees). gf2-f6
  and the jit session have confirmed quiet; every session stops
  writing to the tracker and no timer or server runs jit.

## Steps

### 0. Merge the tool and the runbook, record the go

Merge `worktree-agent-e610f5c4` (the tool) and `worktree-agent-6b9e6b14` (this
runbook) into `main` by hand, taking `main`'s copy on tracker conflicts. Record
the go in the execution issue and commit it, so the extracted tracker carries
it:

```sh
jit issue update 6889e6e6 --append-description "Owner's go for the live cutover: <UTC time>, <the owner's words>."
git add .jit
git commit -m "chore(jit:6889e6e6): record the owner's go for the cutover"
P=$(git rev-parse HEAD)
```

`P` is the pre-cutover `main`. The window log opens with the go, the
arguments and `P`.

### 1. Quiesce

1. Release the executor's own lease: `jit claim release 6889e6e6`, run in the
   checkout that took it (see [Executor's lease](#executors-lease)). The issue
   stays assigned to `<executor>`.
2. Run `git worktree prune` and record the registrations it removed.

### 2. Back up

```sh
git for-each-ref > "<backup>/refs-before.txt"
cutover archive --repo . --dest "<backup>/stores"
```

`archive` re-reads every archive against its manifest and the stores on disk;
`cutover archive --repo . --dest "<backup>/stores" --verify` repeats that read.
The step passes when the listing is complete and the archive exits 0.
When the archive fails, `git bundle verify "<bundle>"` passing is the
fallback. TBD-rehearsal: archive size and duration per store.

### 3. Abandon

Each linked worktree whose branch is on `<abandon-list>` is first detached:

```sh
git -C "<worktree>" switch --detach
cutover abandon --repo . --branches "<abandon-list>"
```

`abandon` moves each listed ref to `refs/archive/<name>`. A detached worktree
whose tracker store and working tree are clean is harvested in step 7 and
removed with `git worktree remove` in step 9. A dirty one stays detached with
its store, is never removed, and is listed for the steward.

Then the branches already merged into `main` are abandoned as a batch,
generated at this moment and never built by hand. An entry that is not an
ancestor of `main` is a survivor and is appended to `<survivor-list>` for
step 9; so is any entry already on `<survivor-list>`, which is not abandoned.

```sh
: > "<backup>/merged-abandon.txt"
sed 's/#.*//' "<survivor-list>" | sed '/^[[:space:]]*$/d' > "<backup>/survivors-before-merged.txt"
git branch --merged main --format='%(refname:short)' | while IFS= read -r b; do
  [ "$b" = main ] && continue
  if grep -qxF -- "$b" "<backup>/survivors-before-merged.txt"; then continue
  elif git merge-base --is-ancestor "$b" main; then printf '%s\n' "$b" >> "<backup>/merged-abandon.txt"
  else printf '%s\n' "$b" >> "<survivor-list>"; fi
done
git worktree list --porcelain | awk '/^worktree /{w=substr($0,10)} /^branch refs\/heads\//{print substr($0,19) "\t" w}' |
  while IFS="$(printf '\t')" read -r b w; do
    grep -qxF -- "$b" "<backup>/merged-abandon.txt" && git -C "$w" switch --detach
  done
cutover abandon --repo . --branches "<backup>/merged-abandon.txt"
```

The detached worktrees follow the same rule as above: clean ones are
harvested and removed, dirty ones stay and are listed for the steward.

### 4. Inventory

Run `cutover inventory --repo .` and record its output. Resolve or record each
`action:` line as [Reconciling the inventory](#reconciling-the-inventory)
states, then rerun it. The step passes when no `action:` line is unresolved
and every one left is recorded in the window log. The `info:` lines that name
gate-run holders are the harvest list of step 7. Record the issue count,
`jit query all --json`, field `count`, as `count_before`.

### 5. Amend the invariant and extract

1. Set the statement of `resolvable-references` in `.jit/invariants.toml` to
   the approved wording in the execution issue, run `jit project render`, and
   check that the only changes are that statement and its projection in
   `AGENTS.md`. Commit both:

   ```sh
   git add .jit/invariants.toml AGENTS.md
   git commit -m "docs(jit:6889e6e6): amend the resolvable-references invariant"
   A=$(git rev-parse HEAD)
   ```

2. Extract the tracker history up to `A`:

   ```sh
   cutover extract --repo . --commit "$A" --dest "<tracker-repo>"
   ```

   The subcommand fails, creating nothing, unless the split tree and the head
   tree of `<tracker-repo>` equal `git rev-parse "$A:.jit"`. Record both tree
   identifiers. TBD-rehearsal: duration of the subtree split over the
   tracker's history.

### 6. Commit the code side and mount

```sh
cutover code-side --repo . --message "chore(jit:6889e6e6): move the tracker out of the code repository"
C=$(git rev-parse HEAD)
cutover mount --repo . --tracker "<tracker-repo>" --aside "<backup>/aside-mount"
```

`code-side` makes the cutover commit `C`, whose parent is `A`: the tracker
paths leave the index, `/.jit/` is ignored, the tracker attribute and
exceptions go, the budget forbids `^\.jit/` and the tree limit falls to the
measured size. `mount` clones `<tracker-repo>` into `.jit/`, moves the old
store to `<backup>/aside-mount` and copies back the machine-local files. It
fails unless every tracked file of the mount equals the head tree of
`<tracker-repo>`. From here the primary checkout runs plain jit and every
linked worktree runs jit through `scripts/tracker-env`.

The tracker's one writable checkout is `.jit/` of the primary checkout.
`<tracker-repo>` keeps the extraction as it was made; nothing pushes to it.

### 7. Harvest

For each checkout the inventory listed as a gate-run holder, other than the
primary checkout, whose records the mount restored:

```sh
cutover harvest --store .jit --from "<worktree>/.jit"
```

A run record the shared store holds with different bytes, or a verdict
pointer whose run the store would not hold, stops that harvest with nothing
copied. The executor records the two records' paths and leaves that
worktree's store in place; step 9 cannot clean the worktree, which is then
listed for the steward. A differing verdict pointer keeps the store's copy.
After the harvests, `load_check strict` runs in the primary checkout. The
step passes when every harvest exits 0 or is listed and the check passes; a
failure stops the cutover.

### 8. Verify

Create two probe worktrees at the cutover commit and a fresh clone:

```sh
git worktree add -b cutover-probe-1 "<backup>/probe-1" "$C"
git worktree add -b cutover-probe-2 "<backup>/probe-2" "$C"
git clone . "<backup>/fresh"
```

Each check below passes on the stated condition only. A failed check starts
the [rollback](#rollback) before any further work. `load_check strict` also
runs before step 5, in the primary checkout, and its result is recorded as the
baseline.

| Check | Command | Pass condition |
|---|---|---|
| Repository validation | `jit validate` | exits 0 (TBD-rehearsal: duration with the harvested gate-run records) |
| Issue load | `load_check strict` in the primary checkout | exits 0: every issue loads and every named run file exists |
| Issue count | `jit query all --json`, field `count` | equals `count_before` |
| Tree identity | `git rev-parse "$C^:.jit"` and `git -C .jit rev-parse 'HEAD^{tree}'` | equal, and equal to the identifiers step 5 recorded |
| Code side | `git ls-files .jit` on `main`; `git check-ignore -q .jit/`; `git status --porcelain` | prints nothing; exits 0; lists no path under `.jit/` |
| Gate from a linked worktree | `jit gate add 6889e6e6 docs-mechanical` in the primary checkout, then `scripts/tracker-env jit gate evaluate 6889e6e6 docs-mechanical` in `<backup>/probe-1` | passes, and its run record under `.jit/gate-runs/` names commit `$C` and branch `cutover-probe-1` (TBD-rehearsal: duration of the checker from a probe worktree) |
| Plain jit in a linked worktree | in `<backup>/probe-1`: `jit status` | fails, finding no store |
| Checkers with the store | `python3 -B scripts/check-references.py` and `python3 scripts/check-tracked-budget.py` in the primary checkout | both exit 0 |
| Checkers without the store | the same two commands in `<backup>/fresh` | both exit 0; the reference checker reports the count of tracker references it left unchecked |
| Issue load without the store's runs | `load_check report` in `<backup>/fresh` after its mount | exits 0: the only load failures are issues whose gate records name runs; the listing is recorded |
| Fresh-clone mount | in `<backup>/fresh`: `scripts/tracker-bootstrap "<the primary checkout>/.jit"`, then the two checkers | the mount exits 0, which includes `jit validate`; both checkers exit 0 |

When every check passes, the executor re-acquires its own lease from the
primary checkout, `JIT_AGENT_ID=<executor> jit claim acquire 6889e6e6`,
removes the probes with `git worktree remove` and `git branch -D`, deletes
`<backup>/fresh`, and snapshots: `scripts/tracker-snapshot`. The snapshot
precedes the history rewrite's first bundle.

### 9. Clean

Each surviving branch gets one commit that removes its tracked `.jit/` paths
and merges nothing; its code tree is unchanged. A branch without a worktree:

```sh
cutover clean --repo . --branches "<survivor-list>" \
  --message "chore(jit:6889e6e6): remove the tracked tracker paths"
```

A branch checked out in a linked worktree, after its harvest:

```sh
cutover clean --repo . --worktree "<worktree>" --store .jit \
  --message "chore(jit:6889e6e6): remove the tracked tracker paths"
```

The batch prints one line per branch and continues after failures; the
worktree form verifies the harvest, commits, and deletes `<worktree>/.jit`.
Each refusal or failure is listed for the steward with the subcommand's
message. A worktree left with its store is refused by `scripts/tracker-env`,
so no session uses its stale copy. A clean worktree of an abandoned branch,
detached in step 3 and harvested in step 7, is removed with
`git worktree remove`; a dirty one is listed.

A removal-only branch checked out in the primary checkout shows the mounted
tracker as untracked until it merges `main`; the branch merges `main` when it
resumes.

The step passes when every local branch is `main`, archived under
`refs/archive/` or cleaned, and both commands below print no branch other
than those listed for the steward. The first lists a branch that is neither
`main` nor on `<survivor-list>`, the second a branch that tracks `.jit`:

```sh
{ echo main; sed 's/#.*//' "<survivor-list>" | sed '/^[[:space:]]*$/d'; } > "<backup>/live.txt"
git for-each-ref --format='%(refname:short)' refs/heads/ | grep -vxF -f "<backup>/live.txt"
git for-each-ref --format='%(refname:short)' refs/heads/ | while IFS= read -r b; do
  [ -z "$(git ls-tree -r --name-only "$b" -- .jit)" ] || printf '%s\n' "$b"
done
```

### 10. Finish and release the window

1. Land the execution issue's guidance change (`AGENTS.md`, `dev/index.md`
   and the projection source) through the landing flow below.
2. Add the `landed` gate to every open issue whose required gates include
   `cargo-ci` and not `landed`: `jit gate add <issue> landed`.
3. Run `scripts/tracker-snapshot`, record the tracker head and `main` in the
   window log, and tell every session through the steward that the window is
   released and that a linked worktree runs jit through `scripts/tracker-env`.

## Rollback

The rollback applies when a check of step 8 fails, and after step 9 when a
later check of this runbook fails before the release. It runs before any
further work, from the primary checkout:

1. Move the mounted clone aside: `mv .jit "<backup>/aside-rollback"`.
2. `git checkout -f "$P"`. This drops the cutover commit `C` and the
   invariant amendment `A`, so `resolvable-references` and its projection in
   `AGENTS.md` read as before.
3. Put every ref back to the listing. Refs that differ are set, which
   restores the branches step 3 abandoned, and refs created
   since (`refs/archive/*`, the probe branches) are deleted:

   ```sh
   while read -r sha _ ref; do
     [ "$(git rev-parse --verify -q "$ref")" = "$sha" ] || git update-ref "$ref" "$sha"
   done < "<backup>/refs-before.txt"
   cut -f2 "<backup>/refs-before.txt" | sort > "<backup>/names-before.txt"
   git for-each-ref --format='%(refname)' | sort | comm -13 "<backup>/names-before.txt" - |
     while IFS= read -r ref; do git update-ref -d "$ref"; done
   git switch main
   ```

4. `git checkout -- .jit`.
5. In each linked worktree whose branch step 9 cleaned, put the tracked store
   back into its index and files with `git -C "<worktree>" checkout HEAD -- .jit`;
   nothing outside `.jit` changes.
6. Re-attach each worktree step 3 detached, to the same commit so no file
   changes: `git -C "<worktree>" switch <branch>`.
7. Restore each worktree store a clean deleted from its archive, adding the
   ignored files:
   `tar -C "<worktree>/.jit" -xf "<backup>/stores/worktree-<name>.tar"`, the
   members checked against `<backup>/stores/manifest.json`.
8. Run `jit validate`, which exits 0, and `load_check strict`, which exits 0,
   and compare `git for-each-ref` with
   the saved listing, which are equal.

Tracker writes made after the listing live in the mounted clone and are lost
with it unless the executor copies them from `<backup>/aside-rollback`. Leases
taken after the listing live in the control plane of the tracker repository
and stop existing for jit: the restored store reads the code repository's
control plane, whose leases step 1 released. Every holder, the executor
included, re-acquires its lease with `jit claim acquire`.

The executor records the failed check, its output and the rollback in the
execution issue. A failed cutover is retried on another day.

## Reconciling the inventory

Each line of `cutover inventory` is resolved before extraction as follows.
The resolution of each line is recorded in the window log. Branch edits that
exist only on a branch are ported to `main` by tracker commits before the
window; the inventory lists what remains.

| Inventory line | Resolution |
|---|---|
| `action: branch <b>: holds tracker changes main lacks` | Ported: the steward's port commits the branch's tracker changes to `main`, and the line disappears on rerun. Abandoned: the branch is on `<abandon-list>` and step 3 moves its ref to `refs/archive/<name>`; the window log records it with `git diff --stat "main...<b>" -- .jit`. Any other branch is listed for the steward and the window waits. |
| `info: branch <b>: tracker files differ from main, which holds all of its tracker changes` | Nothing before extraction; step 9 cleans the branch. |
| `action: checkout <path>: uncommitted tracker changes` | The checkout's session commits them on its branch, which turns them into a branch line above, or discards them with `git -C <path> restore --staged --worktree -- .jit` and `git -C <path> clean -fd -- .jit`; for the primary checkout they are committed on `main`. |
| `info: checkout <path>: tracker files differ from main` | Nothing; the checkout's branch is reconciled by its branch line. |
| `info: checkout <path>: <n> gate-run records in <store>` | Harvested in step 7. |
| `action: store <store>: pending redo record tmp/redo.record` | `jit recover` in that checkout applies the record; the window log records its output. |
| `action: store <holder>: unreleased lease on <issue> by <agent>` | The holder releases it with `jit claim release <issue>`. A lease whose session has ended is evicted with `jit claim force-evict <issue> --reason "tracker cutover quiesce"`; `jit recover` evicts expired ones. |

A detached worktree appears as a branch line labelled with its HEAD. The
worktree of an abandoned branch is detached in step 3, harvested in step 7 and
handled as step 3 and step 9 state.

### Executor's lease

The executor releases its lease on `@/issue/6889e6e6` in step 1, because the
inventory reports every lease, its own included. The issue stays assigned and
`in_progress` throughout. The executor runs the window from the primary
checkout: its own linked worktree, if it has one, holds a store until step 9
cleans it, and `scripts/tracker-env` refuses there. It re-acquires the lease
in the mounted store at the end of step 8, or in the restored store after a
rollback.

### Control-plane files in the code repository's git directory

jit keeps leases, the claim log and claim locks under `jit/` of the git common
directory of the repository that holds the data root. Before the cutover that
is the code repository; after the mount it is the tracker repository, and the
code repository's copy is read by nothing. Step 1 leaves it with no lease and
it stays in place, untouched, so the rollback's restored store reads it
again. Its claim log stays there.

## Landing after the cutover

A branch lands in the order `landing-link` in [plan.md](plan.md) fixes:

1. Evaluate the issue's gates at the branch tip through
   `scripts/tracker-env jit gate evaluate <issue> <gate>` from its worktree.
2. Run the landing check, `scripts/tracker-env scripts/check-landing <issue> <tip>`.
3. Merge into `main`.
4. Evaluate the merged-tree gate.
5. Run `scripts/tracker-snapshot`.
6. Evaluate the `landed` gate, which a code-producing issue carries through
   the `code-task` preset.

A document that exists only on a branch is linked pinned to a branch commit
and re-linked unpinned when it lands.
