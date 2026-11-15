#!/usr/bin/env python3
"""Moves the tracker store out of the code repository into its own repository.

The cutover turns `.jit/` of a code repository into an independent git
repository mounted at `.jit/` of the primary checkout and carries each linked
worktree's tracker state through that change. It is the forward path only:
no subcommand resumes an earlier run or rolls one back. Each subcommand
checks its fresh-run preconditions and, on any other state (a partial earlier
run, an existing staging or aside path, a pre-existing edit to a file it would
change, an unexpected HEAD), stops with a message and changes nothing.
Rollback is the runbook's git steps. Each subcommand is one step of
`cutover-sequence` in `plan.md`. Repositories are arguments; every other
location comes from git.

  cutover.py inventory --repo CODE [--main BRANCH]
      Lists each local branch and detached worktree HEAD whose `.jit` tree
      differs from the main branch's, each checkout whose unignored `.jit`
      files differ from it, each checkout that holds gate-run records, and
      each store with an unreleased lease (`<git common dir>/jit/leases/`) or
      a pending redo record (`<store>/tmp/redo.record`). A branch is judged by
      the `.jit` part of its merge result with the main branch, whatever its
      conflicts outside `.jit`. A line starting `action:` needs action before
      the cutover: a branch whose `.jit` conflicts with the main branch or
      whose merge would change the main branch's `.jit` tree, a checkout with
      uncommitted tracker changes, a lease or a redo record. Other lines
      start `info:`.
  cutover.py abandon --repo CODE (--branch B | --branches FILE)
      Moves each branch to `refs/archive/<B>` and deletes `refs/heads/<B>` in
      one ref transaction, both by compare-and-swap from the branch tip. FILE
      lists one branch per line with optional `#` comments. Prints one line
      per branch: `abandoned to refs/archive/<B>`, `already abandoned` when
      the archive ref exists and the branch ref does not, `refused: <reason>`
      or `failed: <reason>`. Refuses an existing archive ref beside the
      branch ref and a branch checked out in any worktree. A batch continues
      after failures and exits 1 when any branch is neither abandoned nor
      already abandoned.
  cutover.py archive --repo CODE --dest DIR [--verify]
      Writes the store of every checkout to `DIR/<name>.tar`, `<name>` being
      `primary` or `worktree-<name git registers>`, and the SHA-256 of every
      member to `DIR/manifest.json`; then re-reads each archive and compares
      it, and each store still on disk, with the manifest. `--verify` only
      re-reads; a store a clean deleted is compared with its archive alone.
      Refuses a DIR inside a store, inside the git directory or unignored
      inside a checkout, and, when writing, a DIR that exists and is not an
      empty directory.
  cutover.py extract --repo CODE --commit REV --dest NEW [--split REV]
      Splits the history of `.jit` up to REV with `git subtree split` (or
      takes the split commit `--split` names) and creates the repository NEW
      with it as `main`. Fails, creating nothing, unless the split tree and
      the head tree of NEW equal the tree of `REV:.jit`. Refuses an existing
      NEW.
  cutover.py code-side --repo PRIMARY --message MSG [--main BRANCH]
      Makes one commit on the main branch: the tracker paths leave the index
      (their files stay on disk), `.gitignore` ignores `/.jit/`,
      `.gitattributes` loses the union-merge attribute of the event log (and
      the file goes when nothing else remains), `scripts/tracked-budget.toml`
      loses the event log's exception, forbids `^\\.jit/` and lowers
      `max_tree_bytes` to the committed tree size, as
      `scripts/check-tracked-budget.py` measures it, rounded up to the next
      10 kB; `scripts/reference-exceptions.toml` loses the exceptions for the
      event log and `.jit/references.toml`. These four files are the only
      working-tree files outside `.jit` any subcommand writes. Refuses a
      linked worktree, another branch, any staged change, an unstaged change
      to one of those files and an index without tracker paths.
  cutover.py mount --repo PRIMARY --tracker NEW --aside PATH
      Clones NEW into the staging path `.<PATH name>.mount-stage` beside
      PATH, moves `.jit` to PATH, moves the clone into place and copies back
      the machine-local files: those the patterns of the store's
      `.gitignore` outside jit's marker region match. Fails unless every
      tracked file of the mount is byte-identical to the head tree of NEW.
      Refuses a linked worktree, a code repository that still tracks or does
      not ignore `.jit`, a `.jit` that is already a repository of its own,
      an existing PATH or staging path and a PATH on another filesystem than
      `.jit`.
  cutover.py harvest --store SHARED --from STORE
      Copies the run records of STORE, one file per run under
      `gate-runs/<issue>/`, into SHARED and copies each verdict pointer
      (`gate-runs/verdicts/<gate>/<inputs>.json`) SHARED lacks. Where both
      hold a pointer with different bytes it keeps SHARED's. Stops, copying
      nothing, on a run record SHARED holds with different bytes or on a
      pointer whose run SHARED would not hold; then verifies the harvest.
  cutover.py clean --repo CODE (--branch B | --branches FILE)
                --message MSG [--main BRANCH]
      Gives each parked branch one commit whose tree is the tip's tree
      without `.jit`, built from the tip's tree object alone (`git ls-tree`
      without the `.jit` entry, `git mktree`, `git commit-tree`), and moves
      the branch ref to it by compare-and-swap from the tip. It merges
      nothing and reads or writes no index or working tree. FILE lists one
      branch per line with optional `#` comments. Prints one line per branch:
      `cleaned; tree outside .jit equals the tip`, `already clean` when the
      tip tracks no `.jit` (nothing committed), `refused: <reason>` or
      `failed: <reason>`. Refuses a branch checked out in any worktree; use
      the worktree form for it. A batch continues after failures and exits 1
      when any branch is neither cleaned nor already clean.
  cutover.py clean --repo CODE --worktree WT --store SHARED
                --message MSG [--main BRANCH]
      Verifies the harvest of `WT/.jit` into SHARED, makes the same commit
      for the branch of the linked worktree WT, then removes the index
      entries under `.jit` of WT and deletes `WT/.jit`, also when the branch
      was already clean. Refuses an unverified harvest, uncommitted tracked
      changes outside `.jit` and a detached WT. Untracked and ignored files
      of WT outside `.jit` are left as they are.
      Both forms refuse an empty MSG, fail unless `git ls-files .jit` is
      empty on the main branch and on the cleaned commit, and check that
      the cleaned commit's tree outside `.jit` equals the tip's before the
      ref moves.
  cutover.py --self-test
      Runs every subcommand on scratch repositories under a new directory
      in TMPDIR and refuses a scratch path inside the invoking checkout. The
      fixture's ignore, attribute and budget files are those committed at
      HEAD of the invoking checkout; the lease and redo-record cases use the
      real `jit` and are skipped with a printed reason when it is absent.

Exit status: 0 on success or, for the inventory, nothing needing action; 1
when a step or its verification fails or the inventory lists an action; 2 on
a usage error or a refusal.
"""

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path

STORE = ".jit"
BUDGET = "scripts/tracked-budget.toml"
BUDGET_CHECKER = "scripts/check-tracked-budget.py"
REFERENCES = "scripts/reference-exceptions.toml"
EVENTS = ".jit/events.jsonl"
CITATIONS = ".jit/references.toml"
FORBID_STORE = r"^\.jit/"
TREE_STEP = 10_000
MARKER_BEGIN = "# jit:data-root-ignore:begin"
MARKER_END = "# jit:data-root-ignore:end"


class Refusal(Exception):
    """A precondition the step requires does not hold; nothing was changed."""


class Failure(Exception):
    """The step or its verification failed."""


def git(*args, cwd, check=True, input=None):
    result = subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, input=input
    )
    if check and result.returncode != 0:
        raise Failure(f"git {' '.join(args)} in {cwd}: {result.stderr.strip()}")
    return result.stdout


def git_rc(*args, cwd):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True).returncode


def main(argv, *, hooks=None):
    parser = argparse.ArgumentParser(prog="cutover.py", description=__doc__.split("\n")[0])
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="command")
    p = sub.add_parser("inventory")
    p.add_argument("--repo", required=True)
    p.add_argument("--main", default="main")
    p = sub.add_parser("abandon", description="Move branches to refs/archive/ and delete their refs.")
    p.add_argument("--repo", required=True)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument("--branch", help="local branch to abandon")
    target.add_argument("--branches", help="file listing local branches, one per line")
    p = sub.add_parser("archive")
    p.add_argument("--repo", required=True)
    p.add_argument("--dest", required=True)
    p.add_argument("--verify", action="store_true")
    p = sub.add_parser("extract")
    p.add_argument("--repo", required=True)
    p.add_argument("--commit", required=True)
    p.add_argument("--dest", required=True)
    p.add_argument("--split")
    p = sub.add_parser("code-side")
    p.add_argument("--repo", required=True)
    p.add_argument("--message", required=True)
    p.add_argument("--main", default="main")
    p = sub.add_parser("mount")
    p.add_argument("--repo", required=True)
    p.add_argument("--tracker", required=True)
    p.add_argument("--aside", required=True)
    p = sub.add_parser("harvest")
    p.add_argument("--store", required=True)
    p.add_argument("--from", dest="source", required=True)
    p = sub.add_parser("clean", description="Commit each branch tip's tree without .jit, merging nothing.",
                       epilog="--branches lists one branch per line with # comments. Each branch prints "
                       "its outcome; any branch neither cleaned nor already clean makes the exit status 1.")
    p.add_argument("--repo", required=True)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument("--worktree", help="linked worktree whose harvest must be verified")
    target.add_argument("--branch", help="parked local branch without a worktree")
    target.add_argument("--branches", help="file listing parked local branches")
    p.add_argument("--store", help="shared store; required with --worktree only")
    p.add_argument("--message", required=True)
    p.add_argument("--main", default="main")
    try:
        args = parser.parse_args(argv)
    except SystemExit as exit:
        return exit.code
    if args.self_test:
        return self_test()
    if not args.command:
        parser.print_usage(sys.stderr)
        return 2
    args.hooks = hooks or {}
    try:
        return COMMANDS[args.command](args) or 0
    except Refusal as refusal:
        print(f"cutover.py {args.command}: refused: {refusal}", file=sys.stderr)
        return 2
    except Failure as failure:
        print(f"cutover.py {args.command}: failed: {failure}", file=sys.stderr)
        return 1


# Repository layout -----------------------------------------------------------


def physical(path):
    return Path(os.path.realpath(path))


def common_dir(repo):
    return physical(Path(repo) / git("rev-parse", "--git-common-dir", cwd=repo).strip())


def checkouts(repo):
    """Each checkout git registers for the repository: the primary one first."""
    entries, entry = [], {}
    for line in git("worktree", "list", "--porcelain", cwd=repo).splitlines() + [""]:
        if not line:
            if entry and "bare" not in entry and "prunable" not in entry:
                entries.append(entry)
            entry = {}
            continue
        key, _, value = line.partition(" ")
        entry[key] = value
    result = []
    for index, entry in enumerate(entries):
        path = physical(entry["worktree"])
        branch = entry.get("branch", "").removeprefix("refs/heads/") or None
        if index == 0:
            name = "primary"
        else:
            name = "worktree-" + physical(git("rev-parse", "--absolute-git-dir", cwd=path).strip()).name
        result.append({"path": path, "head": entry.get("HEAD"), "branch": branch, "name": name,
                       "primary": index == 0})
    return result


def primary_checkout(repo):
    """The primary checkout of `repo`, refusing a linked worktree."""
    top = physical(git("rev-parse", "--show-toplevel", cwd=repo).strip())
    if physical(git("rev-parse", "--absolute-git-dir", cwd=repo).strip()) != common_dir(repo):
        raise Refusal(f"{top} is a linked worktree; the step runs in the primary checkout")
    return top


def tree_of(rev, path, repo):
    """The tree id of `rev:path`, or None when it has none."""
    result = subprocess.run(["git", "rev-parse", "-q", "--verify", f"{rev}:{path}"],
                            cwd=repo, capture_output=True, text=True)
    if result.returncode != 0:
        return None
    tree = result.stdout.strip()
    return tree if git("cat-file", "-t", tree, cwd=repo).strip() == "tree" else None


def tracked_under(rev, path, repo):
    return [p for p in git("ls-tree", "-r", "-z", "--name-only", rev, "--", path, cwd=repo).split("\0") if p]


def require_branch(checkout, branch):
    current = git("symbolic-ref", "-q", "--short", "HEAD", cwd=checkout, check=False).strip()
    if current != branch:
        raise Refusal(f"{checkout} has {current or 'a detached HEAD'} checked out, not {branch}")


def store_files(store):
    """Relative path to SHA-256 of every regular file under `store`."""
    files = {}
    for root, dirs, names in os.walk(store):
        dirs.sort()
        for name in sorted(names):
            path = Path(root) / name
            if path.is_file() and not path.is_symlink():
                files[path.relative_to(store).as_posix()] = sha256(path)
    return files


def copy_file(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    stage = target.with_name(f".{target.name}.cutover-{os.getpid()}")
    try:
        shutil.copy2(source, stage)
        os.replace(stage, target)
    finally:
        stage.unlink(missing_ok=True)


def step(args, name):
    """Marks the end of a step; the self-test interrupts a subcommand here."""
    args.hooks.get("step", lambda _: None)(name)


def same_filesystem(a, b):
    return os.stat(a).st_dev == os.stat(b).st_dev


def worktree_listing(checkout, *args):
    return [p for p in git("ls-files", "-z", *args, cwd=checkout).split("\0") if p]


# inventory ------------------------------------------------------------------


def merge_result(repo, main_branch, rev):
    """The tree `git merge-tree` writes for merging `rev` into the main branch, and its conflicted paths."""
    merged = subprocess.run(["git", "merge-tree", "--write-tree", "--no-messages", "--name-only", "-z",
                             main_branch, rev], cwd=repo, capture_output=True, text=True)
    if merged.returncode not in (0, 1):
        raise Failure(f"git merge-tree {main_branch} {rev}: {merged.stderr.strip()}")
    tree, *conflicts = merged.stdout.split("\0")
    return tree.strip(), sorted({p for p in conflicts if p})


def inventory(args):
    repo = args.repo
    if git_rc("rev-parse", "-q", "--verify", f"refs/heads/{args.main}", cwd=repo) != 0:
        raise Refusal(f"no branch {args.main}")
    main_tree = tree_of(args.main, STORE, repo)
    lines = []
    found = checkouts(repo)
    heads = [(ref, ref) for ref in git("for-each-ref", "--format=%(refname:short)", "refs/heads/", cwd=repo).split()
             if ref != args.main]
    heads += [(c["head"], f"detached HEAD of {c['path']}") for c in found if c["branch"] is None]
    for rev, label in heads:
        if tree_of(rev, STORE, repo) == main_tree:
            continue
        tree, conflicts = merge_result(repo, args.main, rev)
        tracker = [p for p in conflicts if p == STORE or p.startswith(f"{STORE}/")]
        if tracker:
            lines.append(("action", f"branch {label}: its tracker files conflict with {args.main}: "
                                    f"{', '.join(tracker)}"))
        elif tree_of(tree, STORE, repo) != main_tree:
            lines.append(("action", f"branch {label}: holds tracker changes {args.main} lacks"))
        else:
            lines.append(("info", f"branch {label}: tracker files differ from {args.main}, "
                                  f"which holds all of its tracker changes"))
    main_blobs = {}
    for entry in filter(None, git("ls-tree", "-r", "-z", args.main, "--", STORE, cwd=repo).split("\0")):
        meta, path = entry.split("\t", 1)
        main_blobs[path] = meta.split()[2]
    lease_dirs = {common_dir(repo) / "jit" / "leases"}
    stores = []
    for checkout in found:
        path, store = checkout["path"], checkout["path"] / STORE
        if git_rc("check-ignore", "-q", f"{STORE}/", cwd=path) != 0:
            present = set(worktree_listing(path, "-co", "--exclude-standard", "--", STORE))
            present -= set(worktree_listing(path, "-d", "--", STORE))
            ordered = sorted(present)
            hashes = git("hash-object", "--stdin-paths", cwd=path, input="\n".join(ordered) + "\n").split() if ordered else []
            if dict(zip(ordered, hashes)) != main_blobs:
                lines.append(("info", f"checkout {path}: tracker files differ from {args.main}"))
            if git("status", "--porcelain", "--untracked-files=all", "--", STORE, cwd=path).strip():
                lines.append(("action", f"checkout {path}: uncommitted tracker changes"))
        if not store.is_dir():
            continue
        stores.append(store)
        runs = run_records(store)
        if runs:
            lines.append(("info", f"checkout {path}: {len(runs)} gate-run records in {store}"))
        if (store / "tmp" / "redo.record").exists():
            lines.append(("action", f"store {store}: pending redo record tmp/redo.record"))
        inner = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=store, capture_output=True, text=True)
        if inner.returncode == 0:
            lease_dirs.add(physical(store / inner.stdout.strip()) / "jit" / "leases")
    owners = {}
    for store in stores:
        try:
            owners[json.loads((store / "worktree.json").read_text())["worktree_id"]] = store
        except (OSError, ValueError, KeyError):
            pass
    for leases in sorted(lease_dirs):
        for lease in sorted(leases.glob("*.json")) if leases.is_dir() else []:
            try:
                record = json.loads(lease.read_text())
            except ValueError:
                record = {}
            holder = owners.get(record.get("worktree_id"), f"repository {leases.parent.parent}")
            lines.append(("action", f"store {holder}: unreleased lease on {record.get('issue_id', lease.stem)} "
                                    f"by {record.get('agent_id', 'unknown agent')} ({lease})"))
    for kind, text in lines:
        print(f"{kind}: {text}")
    return 1 if any(kind == "action" for kind, _ in lines) else 0


# abandon --------------------------------------------------------------------


def abandon(args):
    return batch(args, abandon_branch)


def abandon_branch(args, branch):
    repo = args.repo
    ref, archived = f"refs/heads/{branch}", f"refs/archive/{branch}"
    if git_rc("check-ref-format", "--branch", branch, cwd=repo) != 0:
        raise Refusal(f"not a branch name: {branch}")
    tip, kept = rev(ref, repo), rev(archived, repo)
    if tip is None:
        if kept:
            return "already abandoned"
        raise Refusal(f"not a local branch: {branch}")
    if kept:
        raise Refusal(f"{archived} exists at {kept}")
    refuse_checked_out(repo, branch)
    # One transaction: the archive ref must not exist and the branch ref must still be at the tip.
    git("update-ref", "-m", "cutover.py abandon", "--stdin", cwd=repo,
        input=f"create {archived} {tip}\ndelete {ref} {tip}\n")
    return f"abandoned to {archived}"


# archive --------------------------------------------------------------------


def refuse_archive_dest(dest, found, repo):
    if inside(dest, common_dir(repo)):
        raise Refusal(f"{dest} is inside the git directory")
    for checkout in found:
        path = checkout["path"]
        if inside(dest, path / STORE):
            raise Refusal(f"{dest} is inside the store {path / STORE}")
        if inside(dest, path):
            rel = Path(os.path.realpath(dest)).relative_to(path).as_posix()
            if rel == "." or git_rc("check-ignore", "-q", f"{rel}/", cwd=path) != 0:
                raise Refusal(f"{dest} lies in the tracked tree of {path}")


def write_archives(args, dest, found):
    manifest = {}
    for checkout in found:
        store = checkout["path"] / STORE
        if not store.is_dir():
            continue
        files = store_files(store)
        with tarfile.open(dest / f"{checkout['name']}.tar", "x") as tar:
            for root, dirs, names in os.walk(store):
                dirs.sort()
                for name in sorted(dirs) + sorted(names):
                    path = Path(root) / name
                    rel = path.relative_to(store).as_posix()
                    if path.is_dir() or rel in files:
                        tar.add(path, arcname=rel, recursive=False)
        manifest[checkout["name"]] = {"checkout": str(checkout["path"]), "files": files}
        step(args, f"archived {checkout['name']}")
        print(f"archived {store}: {len(files)} files in {checkout['name']}.tar")
    with open(dest / "manifest.json", "x") as handle:
        handle.write(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def archive_faults(dest, name, entry):
    faults, members = [], {}
    try:
        with tarfile.open(dest / f"{name}.tar") as tar:
            for member in tar:
                if member.isfile():
                    digest = hashlib.sha256()
                    handle = tar.extractfile(member)
                    for block in iter(lambda: handle.read(1 << 20), b""):
                        digest.update(block)
                    members[member.name] = digest.hexdigest()
    except (OSError, tarfile.TarError) as error:
        return [f"{name}.tar: unreadable: {error}"]
    for rel in sorted(set(members) | set(entry["files"])):
        if members.get(rel) != entry["files"].get(rel):
            faults.append(f"{name}.tar: {rel} differs from the manifest")
    store = Path(entry["checkout"]) / STORE
    if store.is_dir():
        disk = store_files(store)
        for rel in sorted(set(disk) | set(entry["files"])):
            if disk.get(rel) != entry["files"].get(rel):
                faults.append(f"{name}: {store / rel} differs from the manifest")
    return faults


def read_manifest(dest):
    try:
        return json.loads((dest / "manifest.json").read_text())
    except (OSError, ValueError) as error:
        raise Failure(f"no readable manifest in {dest}: {error}")


def archive(args):
    dest = physical(args.dest)
    found = checkouts(args.repo)
    refuse_archive_dest(dest, found, args.repo)
    if not args.verify:
        if dest.is_symlink() or (dest.exists() and (not dest.is_dir() or os.listdir(dest))):
            raise Refusal(f"{dest} exists and is not an empty directory")
        dest.mkdir(parents=True, exist_ok=True)
        write_archives(args, dest, found)
    manifest = read_manifest(dest)
    faults = []
    for name, entry in sorted(manifest.items()):
        faults += archive_faults(dest, name, entry)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"{len(faults)} archive faults")
    print(f"verified {len(manifest)} archives in {dest}")


# extract --------------------------------------------------------------------


def subtree_split(repo, commit):
    """`git subtree split`, removing the cache it leaves in the git directory."""
    cache = physical(git("rev-parse", "--absolute-git-dir", cwd=repo).strip()) / "subtree-cache"
    before = set(os.listdir(cache)) if cache.is_dir() else set()
    try:
        return git("subtree", "split", f"--prefix={STORE}", commit, cwd=repo).split()[-1]
    finally:
        for name in (set(os.listdir(cache)) if cache.is_dir() else set()) - before:
            shutil.rmtree(cache / name, ignore_errors=True)
        if cache.is_dir() and not before and not os.listdir(cache):
            cache.rmdir()


def extract(args):
    repo, dest = args.repo, physical(args.dest)
    if dest.exists() or dest.is_symlink():
        raise Refusal(f"{dest} exists")
    commit = git("rev-parse", "--verify", f"{args.commit}^{{commit}}", cwd=repo).strip()
    code_tree = tree_of(commit, STORE, repo)
    if code_tree is None:
        raise Refusal(f"{commit} has no {STORE} tree")
    split = git("rev-parse", "--verify", f"{args.split or subtree_split(repo, commit)}^{{commit}}", cwd=repo).strip()
    split_tree = git("rev-parse", f"{split}^{{tree}}", cwd=repo).strip()
    if split_tree != code_tree:
        raise Failure(f"split {split} has tree {split_tree}, {commit}:{STORE} is {code_tree}")
    stage = dest.with_name(f".{dest.name}.extract-{os.getpid()}")
    try:
        git("init", "-q", "-b", "main", str(stage), cwd=repo)
        git("config", "receive.denyCurrentBranch", "updateInstead", cwd=stage)
        git("push", "-q", str(stage), f"{split}:refs/heads/main", cwd=repo)
        git("config", "--unset", "receive.denyCurrentBranch", cwd=stage)
        head_tree = git("rev-parse", "HEAD^{tree}", cwd=stage).strip()
        if head_tree != code_tree or git("status", "--porcelain", cwd=stage).strip():
            raise Failure(f"head tree {head_tree} of the new repository differs from {code_tree}")
        os.rename(stage, dest)
    finally:
        shutil.rmtree(stage, ignore_errors=True)
    count = git("rev-list", "--count", "HEAD", cwd=dest).strip()
    print(f"extracted {dest}: {count} commits, head {split}, tree {code_tree}")


# code-side ------------------------------------------------------------------


def toml_blocks(lines, header):
    """(start, end) of each `header` table, ending before a blank, comment or table line."""
    blocks = []
    for start, line in enumerate(lines):
        if line.strip() == header:
            end = start + 1
            while end < len(lines) and lines[end].strip() and not lines[end].lstrip().startswith(("#", "[")):
                end += 1
            blocks.append((start, end))
    return blocks


def remove_exceptions(text, paths):
    lines = text.splitlines(keepends=True)
    for start, end in reversed(toml_blocks(lines, "[[exception]]")):
        entry = tomllib.loads("".join(lines[start + 1:end]))
        if isinstance(entry.get("path"), str) and entry["path"] in paths:
            if start > 0 and not lines[start - 1].strip():
                start -= 1
            del lines[start:end]
    result = "".join(lines)
    left = {e.get("path") for e in tomllib.loads(result).get("exception", []) if isinstance(e.get("path"), str)}
    if left & set(paths):
        raise Failure(f"exceptions for {sorted(left & set(paths))} remain")
    return result


def forbid_store(text):
    if FORBID_STORE in tomllib.loads(text).get("forbidden_paths", []):
        return text
    lines = text.splitlines(keepends=True)
    start = next((i for i, l in enumerate(lines) if l.startswith("forbidden_paths = [")), None)
    if start is None:
        raise Failure(f"{BUDGET} has no forbidden_paths list")
    end = next(i for i in range(start, len(lines)) if lines[i].rstrip() == "]")
    lines[end:end] = ["  # The tracker is its own repository, mounted at .jit/.\n", f"  '{FORBID_STORE}',\n"]
    return "".join(lines)


def set_tree_limit(text, limit):
    result, count = re.subn(r"(?m)^max_tree_bytes = \d+$", f"max_tree_bytes = {limit}", text)
    if count != 1:
        raise Failure(f"{BUDGET} has {count} max_tree_bytes lines")
    return result


def changed_paths(repo, *args):
    return {p for p in git("diff", "-z", "--name-only", *args, cwd=repo).split("\0") if p}


def code_side(args):
    repo = primary_checkout(args.repo)
    require_branch(repo, args.main)
    edited = (".gitignore", ".gitattributes", BUDGET, REFERENCES)
    staged, unstaged = changed_paths(repo, "--cached", "HEAD"), changed_paths(repo)
    if staged or unstaged & set(edited):
        raise Refusal(f"the index or a file this step edits differs from HEAD: {sorted(staged | unstaged & set(edited))}")
    if not worktree_listing(repo, "--", STORE):
        raise Refusal(f"the index holds no path under {STORE}")
    checker = load_budget_checker(repo)
    old_limit = tomllib.loads((repo / BUDGET).read_text())["max_tree_bytes"]
    git("rm", "-r", "-q", "--cached", "--", STORE, cwd=repo)
    step(args, "untracked")
    ignore = (repo / ".gitignore").read_text()
    if "/.jit/" not in ignore.splitlines():
        sep = "" if not ignore or ignore.endswith("\n") else "\n"
        (repo / ".gitignore").write_text(f"{ignore}{sep}\n# The tracker repository, mounted here\n/.jit/\n")
    attributes = repo / ".gitattributes"
    if attributes.exists():
        kept = [l for l in attributes.read_text().splitlines(keepends=True)
                if not re.match(rf"\s*{re.escape(EVENTS)}\s.*\bmerge=union\b", l)]
        if any(l.strip() and not l.lstrip().startswith("#") for l in kept):
            attributes.write_text("".join(kept))
        else:
            git("rm", "-q", "--", ".gitattributes", cwd=repo)
    (repo / REFERENCES).write_text(remove_exceptions((repo / REFERENCES).read_text(), {EVENTS, CITATIONS}))
    budget = forbid_store(remove_exceptions((repo / BUDGET).read_text(), {EVENTS}))
    step(args, "edited")
    limit = None
    for _ in range(4):
        (repo / BUDGET).write_text(budget if limit is None else set_tree_limit(budget, limit))
        git("add", "--", *(p for p in edited if (repo / p).exists()), cwd=repo)
        size = sum(checker.tree_sizes(git("write-tree", cwd=repo).strip(), str(repo)).values())
        rounded = -(-size // TREE_STEP) * TREE_STEP
        if rounded == limit:
            break
        limit = rounded
    else:
        raise Failure("the tree limit does not settle")
    if limit > old_limit:
        raise Failure(f"the measured tree {size} rounds to {limit}, above the current limit {old_limit}")
    step(args, "staged")
    git("commit", "-q", "-m", args.message, cwd=repo)
    step(args, "committed")
    if worktree_listing(repo, "--", STORE):
        raise Failure(f"paths under {STORE} are still tracked")
    head = git("rev-parse", "HEAD", cwd=repo).strip()
    print(f"code-side commit {head}: tree {size} bytes, max_tree_bytes {limit}")


# mount ----------------------------------------------------------------------


def machine_local_patterns(store):
    """The patterns of the store's ignore file outside jit's marker region."""
    patterns, marked = [], False
    for line in (store / ".gitignore").read_text().splitlines():
        if line.strip() == MARKER_BEGIN:
            marked = True
        elif line.strip() == MARKER_END:
            marked = False
        elif not marked and line.strip() and not line.startswith("#"):
            patterns.append(line)
    return patterns


def ignored_files(directory, patterns):
    """Files under `directory` the ignore patterns match, read by git alone."""
    with tempfile.TemporaryDirectory() as scratch:
        git("init", "-q", "--bare", f"{scratch}/git", cwd=scratch)
        (Path(scratch) / "patterns").write_text("\n".join(patterns) + "\n")
        out = git(f"--git-dir={scratch}/git", f"--work-tree={directory}", "ls-files", "-z", "-o", "-i",
                  f"--exclude-from={scratch}/patterns", cwd=directory)
    return [p for p in out.split("\0") if p and not p.startswith(".git/")]


def mount_faults(store, tracker):
    faults = []
    expected = {}
    for entry in filter(None, git("ls-tree", "-r", "-z", "HEAD", cwd=tracker).split("\0")):
        meta, rel = entry.split("\t", 1)
        expected[rel] = meta.split()[2]
    tracked = [rel for rel in worktree_listing(store) if (store / rel).is_file()]
    hashes = git("hash-object", "--no-filters", "--stdin-paths", cwd=store,
                 input="\n".join(tracked) + "\n").split() if tracked else []
    actual = dict(zip(tracked, hashes))
    for rel in sorted(set(expected) | set(actual)):
        if expected.get(rel) != actual.get(rel):
            faults.append(f"{store / rel} differs from the extracted tree")
    return faults


def own_repository(path):
    """Whether `path` is the top level of a git repository of its own."""
    if not path.is_dir() or not (path / ".git").exists():
        return False
    top = git("rev-parse", "--show-toplevel", cwd=path, check=False).strip()
    return bool(top) and physical(top) == physical(path)


def mount(args):
    repo = primary_checkout(args.repo)
    tracker, aside, store = physical(args.tracker), physical(args.aside), repo / STORE
    if worktree_listing(repo, "--", STORE):
        raise Refusal(f"the code repository still tracks {STORE}; run code-side first")
    if git_rc("check-ignore", "-q", f"{STORE}/", cwd=repo) != 0:
        raise Refusal(f"the code repository does not ignore {STORE}/")
    stage = aside.with_name(f".{aside.name}.mount-stage")
    for path in (aside, stage):
        if path.exists() or path.is_symlink():
            raise Refusal(f"{path} exists")
    if not store.is_dir() or own_repository(store):
        raise Refusal(f"no old store at {store}")
    if not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent):
        raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}")
    git("clone", "-q", "--", str(tracker), str(stage), cwd=repo)
    step(args, "cloned")
    os.rename(store, aside)
    step(args, "moved")
    os.rename(stage, store)
    step(args, "placed")
    restored = 0
    for rel in ignored_files(aside, machine_local_patterns(store)):
        target = store / rel
        if target.exists():
            if not same_bytes(aside / rel, target):
                raise Failure(f"machine-local {rel} would overwrite {target}")
            continue
        copy_file(aside / rel, target)
        restored += 1
        step(args, "restored a machine-local file")
    faults = mount_faults(store, tracker)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"{len(faults)} tracked files differ from the extracted tree")
    print(f"mounted {tracker} at {store}; {restored} machine-local files restored; old store at {aside}")


# harvest --------------------------------------------------------------------


def run_records(store):
    """Relative paths `gate-runs/<issue>/...` of the store's run records."""
    root = Path(store) / "gate-runs"
    if not root.is_dir():
        return []
    return sorted(p.relative_to(store).as_posix() for p in root.rglob("*")
                  if p.is_file() and p.relative_to(root).parts[0] != "verdicts")


def verdict_pointers(store):
    root = Path(store) / "gate-runs" / "verdicts"
    return sorted(p.relative_to(store).as_posix() for p in root.rglob("*.json")) if root.is_dir() else []


def pointed_run(store, rel):
    try:
        record = json.loads((Path(store) / rel).read_text())
        return f"gate-runs/{record['issue_id']}/{record['run_id']}.json"
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise Failure(f"{Path(store) / rel}: not a verdict pointer: {error}")


def same_bytes(a, b):
    return a.is_file() and b.is_file() and a.read_bytes() == b.read_bytes()


def harvest_plan(shared, source):
    """The runs and pointers to copy, the kept pointers and the faults that stop a harvest."""
    shared, source = Path(shared), Path(source)
    runs, pointers, kept, faults = [], [], [], []
    for rel in run_records(source):
        if not (shared / rel).exists():
            runs.append(rel)
        elif not same_bytes(shared / rel, source / rel):
            faults.append(f"run record {rel} exists in {shared} with different bytes")
    held = set(run_records(shared)) | set(runs)
    for rel in verdict_pointers(source):
        named = [pointed_run(source, rel)]
        if not (shared / rel).exists():
            pointers.append(rel)
        elif not same_bytes(shared / rel, source / rel):
            kept.append(rel)
            named.append(pointed_run(shared, rel))
        for run in named:
            if run not in held:
                faults.append(f"verdict pointer {rel} names {run}, absent from {shared}")
    return runs, pointers, kept, faults


def harvest(args):
    shared, source = physical(args.store), physical(args.source)
    for path in (shared, source):
        if not path.is_dir():
            raise Refusal(f"no store at {path}")
    if shared == source:
        raise Refusal("the source is the shared store")
    runs, pointers, kept, faults = harvest_plan(shared, source)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"harvest of {source} stopped; nothing copied")
    for rel in runs + pointers:
        copy_file(source / rel, shared / rel)
    for rel in kept:
        print(f"kept the store's verdict pointer {rel}")
    left, unset, _, faults = harvest_plan(shared, source)
    if left or unset or faults:
        raise Failure(f"harvest of {source} does not verify: {faults or left + unset}")
    print(f"harvested {source}: {len(runs)} run records and {len(pointers)} pointers copied, verified")


# clean ----------------------------------------------------------------------


def batch(args, one):
    """Runs `one(args, branch)` for each branch of `--branch` or `--branches`, one line each."""
    if args.branches:
        try:
            branches = [line.partition("#")[0].strip() for line in Path(args.branches).read_text().splitlines()]
        except (OSError, ValueError) as error:
            raise Refusal(f"cannot read branches: {error}")
        branches = [branch for branch in branches if branch]
    else:
        branches = [args.branch]
    failed = False
    for branch in branches:
        try:
            outcome = one(args, branch)
        except (Refusal, Failure, OSError) as error:
            reason = " ".join(str(error).splitlines())
            outcome = f"{'refused' if isinstance(error, Refusal) else 'failed'}: {reason}"
            failed = True
        print(f"{branch}: {outcome}")
    return 1 if failed else 0


def rev(ref, repo):
    return git("rev-parse", "-q", "--verify", f"{ref}^{{commit}}", cwd=repo, check=False).strip() or None


def refuse_checked_out(repo, branch):
    holder = next((c for c in checkouts(repo) if c["branch"] == branch), None)
    if holder:
        raise Refusal(f"{branch} is checked out in {holder['path']}")


def local_tip(repo, branch):
    if git_rc("check-ref-format", "--branch", branch, cwd=repo) != 0:
        raise Refusal(f"not a branch name: {branch}")
    tip = rev(f"refs/heads/{branch}", repo)
    if tip is None:
        raise Refusal(f"not a local branch: {branch}")
    return tip


def clean(args):
    if not args.message.strip():
        raise Refusal("the commit message is empty")
    if tracked_under(args.main, STORE, args.repo):
        raise Failure(f"{args.main} still tracks paths under {STORE}")
    if args.worktree:
        if not args.store:
            raise Refusal("--worktree requires --store")
        return clean_linked(args)
    if args.store:
        raise Refusal("--store applies only to --worktree")
    return batch(args, clean_parked)


def cleaned_commit(repo, branch, tip, message):
    """Moves `branch` from `tip` to a child whose tree is the tip's without `.jit`; None when already clean.

    The tree is built from the tip's tree object alone; no index or working
    tree is read or written. The ref moves by compare-and-swap from `tip`.
    """
    if not tracked_under(tip, STORE, repo):
        return None
    entries = [e for e in git("ls-tree", "-z", tip, cwd=repo).split("\0") if e and e.split("\t", 1)[1] != STORE]
    tree = git("mktree", "-z", cwd=repo, input="".join(f"{e}\0" for e in entries)).strip()
    new = git("commit-tree", tree, "-p", tip, "-m", message, cwd=repo).strip()
    if tracked_under(new, STORE, repo):
        raise Failure(f"the cleaned commit {new} still tracks paths under {STORE}")
    if git_rc("diff", "--quiet", tip, new, "--", ".", f":(exclude){STORE}", cwd=repo) != 0:
        raise Failure(f"the tree of {new} outside {STORE} differs from the tip {tip}")
    git("update-ref", "-m", "cutover.py clean", f"refs/heads/{branch}", new, tip, cwd=repo)
    return new


def clean_parked(args, branch):
    repo = args.repo
    tip = local_tip(repo, branch)
    refuse_checked_out(repo, branch)
    if cleaned_commit(repo, branch, tip, args.message) is None:
        return "already clean"
    return f"cleaned; tree outside {STORE} equals the tip"


def clean_linked(args):
    worktree = physical(args.worktree)
    checkout = next((c for c in checkouts(args.repo) if c["path"] == worktree), None)
    if checkout is None or checkout["primary"]:
        raise Refusal(f"{worktree} is not a linked worktree of {args.repo}")
    branch = checkout["branch"]
    if branch is None:
        raise Refusal(f"{worktree} has a detached HEAD")
    store, shared = worktree / STORE, physical(args.store)
    if store.is_symlink():
        raise Refusal(f"{store} is a symbolic link")
    if store.is_dir():
        runs, pointers, _, faults = harvest_plan(shared, store)
        if runs or pointers or faults:
            raise Refusal(f"the harvest of {store} into {shared} is not verified")
    if git("status", "--porcelain", "--untracked-files=no", "--", ".", f":(exclude){STORE}", cwd=worktree).strip():
        raise Refusal(f"{worktree} has uncommitted tracked changes outside {STORE}")
    tip = local_tip(args.repo, branch)
    if git("rev-parse", "HEAD", cwd=worktree).strip() != tip:
        raise Refusal(f"HEAD of {worktree} is not the tip of {branch}")
    new = cleaned_commit(args.repo, branch, tip, args.message)
    # HEAD names the cleaned commit; --cached changes index entries under .jit only, no working-tree file.
    try:
        git("rm", "-r", "-q", "-f", "--cached", "--ignore-unmatch", "--", STORE, cwd=worktree)
    except Failure as error:
        raise Failure(f"{branch} is at {new or tip}, but the index of {worktree} still lists {STORE}: {error}")
    if store.is_dir():
        try:
            shutil.rmtree(store)
        except OSError as error:
            raise Failure(f"deleting the local store {store} failed and may have removed part of it; "
                          f"its run records are harvested into {shared}: {error}")
        print(f"deleted the local store {store}")
    print(f"{branch}: {'already clean' if new is None else f'cleaned; tree outside {STORE} equals the tip'}")


COMMANDS = {
    "inventory": inventory, "abandon": abandon, "archive": archive, "extract": extract, "code-side": code_side,
    "mount": mount, "harvest": harvest, "clean": clean,
}


# Self-test ------------------------------------------------------------------

MACHINE_LOCAL = (
    "\n# Machine-local tracker state\n**/*.lock\n/worktree.json\n/server.log\n"
    "/server.pid.json\n/gate-runs/\n/gate-artifacts/\n"
)
FIXTURE_FILES = (".gitignore", ".gitattributes", BUDGET, REFERENCES, BUDGET_CHECKER)
CLEARED_ENV = (
    "JIT_DATA_DIR", "JIT_WORKTREE_ROOT", "JIT_AGENT_ID", "JIT_CONTEXT_FILE",
    "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_CEILING_DIRECTORIES",
)
RUN_A = b'{"run": "r1", "status": "passed"}\n'
RUN_B = b'{"run": "r1", "status": "failed"}\n'
I1, I2, I3, I9 = (f"{n}{n}{n}{n}0000-0000-4000-8000-00000000000{n}" for n in (1, 2, 3, 9))


class SelfTest:
    def __init__(self, base, invoking):
        self.base = base
        self.invoking = invoking
        self.failures = []
        self.cases = 0
        self.jit = shutil.which("jit")

    def check(self, condition, label):
        self.cases += 1
        if not condition:
            self.failures.append(label)
            print(f"  FAIL {label}")
        return condition

    def run(self, *argv, hooks=None):
        for arg in argv:
            if arg.startswith("/"):
                if inside(Path(arg), self.invoking):
                    raise Refusal(f"self-test argument inside the invoking checkout: {arg}")
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            try:
                code = main(list(argv), hooks=hooks)
            except Interrupted:
                code = "interrupted"
        return code, out.getvalue()

    def snapshot(self):
        keep = self.base / ".snapshot"
        shutil.rmtree(keep, ignore_errors=True)
        keep.mkdir()
        for entry in self.base.iterdir():
            if entry.name != keep.name:
                copy_entry(entry, keep / entry.name)

    def restore(self):
        keep = self.base / ".snapshot"
        for entry in list(self.base.iterdir()):
            if entry.name != keep.name:
                remove_entry(entry)
        for entry in keep.iterdir():
            copy_entry(entry, self.base / entry.name)

    def observe(self):
        """Every path and file digest under the scratch base, every ref and every index listing.

        Git object and log directories are skipped, and an index file is compared
        by its listing, since git may rewrite its stat cache.
        """
        state, indexes = {}, {}
        for root, dirs, names in os.walk(self.base):
            here = Path(root)
            if here == self.base:
                dirs[:] = [d for d in dirs if d != ".snapshot"]
            if ".git" in here.parts or here.name == ".git":
                dirs[:] = [d for d in dirs if d not in ("objects", "logs")]
            for name in dirs:
                state[(here / name).relative_to(self.base).as_posix()] = "dir"
            for name in names:
                path = here / name
                rel = path.relative_to(self.base).as_posix()
                if name == "index" and (".git" in here.parts or here.name == ".git"):
                    indexes[rel] = git(f"--git-dir={here}", "ls-files", "-s", cwd=here, check=False)
                    continue
                state[rel] = "link:" + os.readlink(path) if path.is_symlink() else sha256(path)
        refs = {}
        for repo in ("code", "tracker"):
            if (self.base / repo / ".git").exists():
                refs[repo] = git("for-each-ref", "--format=%(refname) %(objectname)", cwd=self.base / repo)
        return state, indexes, refs

    def refused_unchanged(self, argv, label, hooks=None):
        """Runs `argv` and requires a refusal (exit 2) that changes nothing."""
        before = self.observe()
        rc, out = self.run(*argv, hooks=hooks)
        after = self.observe()
        if not self.check(rc == 2 and after == before, label):
            print(f"    rc {rc}: {out.strip()}")
            for part, (x, y) in enumerate(zip(before, after)):
                for key in sorted(set(x) | set(y)):
                    if x.get(key) != y.get(key):
                        print(f"    changed: {key}")
        return out

    def refuse_after_each(self, argv, steps, label):
        """Interrupts `argv` after each step; a rerun on that partial state is refused and changes nothing."""
        self.check(steps != [], f"{label}: the command reports its steps")
        for step in dict.fromkeys(steps):
            self.restore()
            rc, _ = self.run(*argv, hooks={"step": interrupt_at(step)})
            self.check(rc == "interrupted", f"{label}: interrupted after {step}")
            self.refused_unchanged(argv, f"REQ-08 {label}: a rerun after an interruption at {step} "
                                         f"is refused and changes nothing")
        self.restore()
        rc, out = self.run(*argv)
        self.check(rc == 0, f"{label}: an uninterrupted run passes again")
        self.refused_unchanged(argv, f"REQ-08 {label}: a rerun of the finished command is refused and changes nothing")

    def sh(self, *cmd, cwd, check=True, env=None):
        result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, env=env)
        if check and result.returncode != 0:
            raise Failure(f"{' '.join(cmd)} in {cwd}: {result.stderr.strip()}")
        return result

    def jit_env(self, agent=None):
        env = dict(os.environ, JIT_WORKTREE_WRITE_POLICY="allow")
        if agent:
            env["JIT_AGENT_ID"] = agent
        return env

    def path(self, name):
        return str(self.base / name)

    def tracker_write(self, checkout, n):
        """Adds an issue to the checkout's store, through jit when it is present."""
        store = Path(checkout) / STORE
        if self.jit:
            out = self.sh("jit", "issue", "create", "--title", f"Fixture issue {n}", "--json",
                          cwd=checkout, env=self.jit_env()).stdout
            return json.loads(out)["id"]
        issue = f"{n:08d}-0000-4000-8000-000000000000"
        (store / "issues" / f"{issue}.json").write_text(json.dumps({"id": issue, "title": f"Fixture issue {n}"}) + "\n")
        with open(store / "events.jsonl", "a") as log:
            log.write(json.dumps({"event": "issue_created", "issue": issue}) + "\n")
        return issue

    def commit_all(self, checkout, message):
        self.sh("git", "add", "-A", cwd=checkout)
        self.sh("git", "commit", "-q", "-m", message, cwd=checkout)

    def write_runs(self, store, files):
        for rel, data in files.items():
            target = Path(store) / "gate-runs" / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)

    def build(self):
        code = self.path("code")
        os.makedirs(code)
        self.sh("git", "init", "-q", "-b", "main", cwd=code)
        for rel in FIXTURE_FILES:
            data = subprocess.run(["git", "show", f"HEAD:{rel}"], cwd=self.invoking,
                                  capture_output=True, check=True).stdout
            target = Path(code) / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        (Path(code) / "src").mkdir()
        (Path(code) / "src" / "lib.rs").write_text("pub fn f() {}\n")
        store = Path(code) / STORE
        if self.jit:
            self.sh("jit", "init", cwd=code, env=self.jit_env())
        else:
            (store / "issues").mkdir(parents=True)
            (store / ".gitignore").write_text(f"{MARKER_BEGIN}\n/tmp/\n{MARKER_END}\n")
            (store / "config.toml").write_text('[worktree]\nroot = ".."\n')
            (store / "index.json").write_text('{"deleted_ids": [], "schema_version": 4}\n')
            (store / "events.jsonl").write_text("")
            (store / "worktree.json").write_text('{"worktree_id": "wt:fixture"}\n')
            (store / "tmp").mkdir()
        with open(store / ".gitignore", "a") as ignore:
            ignore.write(MACHINE_LOCAL)
        (store / "server.log").write_text("server log\n")
        self.commit_all(code, "chore: initial tree with tracker")
        for n in (1, 2, 3):
            self.tracker_write(code, n)
            self.commit_all(code, f"chore: tracker history {n}")
        for name in ("wa", "wb"):
            self.sh("git", "worktree", "add", "-q", "-b", name, self.path(name), "main", cwd=code)
        self.write_runs(store, {f"{I1}/r1.json": RUN_A,
                                "verdicts/cargo-ci/d1.json": pointer(I1, "r1")})
        self.write_runs(self.path("wa") + "/" + STORE, {
            f"{I1}/r1.json": RUN_A, f"{I2}/r2.json": b'{"run": "r2"}\n',
            "verdicts/cargo-ci/d1.json": pointer(I2, "r2"),
            "verdicts/cargo-ci/d2.json": pointer(I2, "r2")})
        self.write_runs(self.path("wb") + "/" + STORE, {
            f"{I1}/r1.json": RUN_B, f"{I3}/r3.json": b'{"run": "r3"}\n',
            "verdicts/cargo-ci/d3.json": pointer(I9, "r9")})
        return code

    def inventory(self, code):
        wa, wb = self.path("wa"), self.path("wb")
        self.tracker_write(wa, 4)
        self.commit_all(wa, "chore: branch-only tracker change on wa")
        self.tracker_write(wb, 5)
        self.commit_all(wb, "chore: branch-only tracker change on wb")
        (Path(wa) / STORE / "config.toml").open("a").write("# uncommitted\n")
        redo = Path(code) / STORE / "tmp" / "redo.record"
        redo.parent.mkdir(exist_ok=True)
        redo.write_text("interrupted publication\n")
        rc, out = self.run("inventory", "--repo", code)
        print(out, end="")
        self.check(rc == 1, "REQ-01 inventory exits 1 while branches and stores need action")
        lines = out.splitlines()
        for branch in ("wa", "wb"):
            self.check(any(l.startswith("action:") and f"branch {branch}" in l for l in lines),
                       f"REQ-01 inventory names branch {branch} with branch-only tracker changes")
        self.check(any(l.startswith("action:") and wa in l and "uncommitted" in l for l in lines),
                   "REQ-01 inventory names the checkout with uncommitted tracker changes")
        for holder in (code, wa, wb):
            self.check(any(f"{holder}:" in l and "gate-run records" in l for l in lines),
                       f"REQ-01 inventory names gate-run holder {holder}")
        self.check(any(l.startswith("action:") and "redo record" in l for l in lines),
                   "REQ-01 inventory names the pending redo record")
        if self.jit:
            listed = self.sh("jit", "issue", "list", cwd=code, check=False, env=self.jit_env())
            self.check(listed.returncode != 0, "REQ-01 jit itself refuses the store with the planted redo record")
        redo.unlink()
        self.sh("git", "checkout", "--", f"{STORE}/config.toml", cwd=wa)
        self.sh("git", "merge", "-q", "--no-edit", "wa", cwd=code)
        self.sh("git", "merge", "-q", "--no-edit", "wb", cwd=code)
        rc, out = self.run("inventory", "--repo", code)
        lines = out.splitlines()
        self.check(rc == 0, "REQ-01 inventory exits 0 once tracker changes are merged")
        self.check(not any(l.startswith("action:") for l in lines), "REQ-01 no action listed after the merges")
        for branch in ("wa", "wb"):
            self.check(any(l.startswith("info:") and f"branch {branch}" in l for l in lines),
                       f"REQ-01 inventory still lists branch {branch}, whose tracker files differ")
        self.check(any("gate-run records" in l for l in lines), "REQ-01 gate-run holders listed after the merges")
        redo.write_text("interrupted publication\n")
        rc, out = self.run("inventory", "--repo", code)
        self.check(rc == 1 and "redo record" in out, "REQ-01 a pending redo record alone needs action")
        redo.unlink()
        self.inventory_conflicts(code)
        if not self.jit:
            print("  skip REQ-01 lease case: jit is not on PATH")
            return
        issue = self.tracker_write(code, 6)
        self.commit_all(code, "chore: tracker issue to lease")
        self.sh("jit", "claim", "acquire", issue, cwd=code, env=self.jit_env("agent:selftest"))
        rc, out = self.run("inventory", "--repo", code)
        self.check(rc == 1 and any(l.startswith("action:") and "lease" in l and issue in l
                                   for l in out.splitlines()),
                   "REQ-01 an unreleased jit lease needs action")
        self.sh("jit", "claim", "release", issue, cwd=code, env=self.jit_env("agent:selftest"))
        if git("status", "--porcelain", cwd=code).strip():
            self.commit_all(code, "chore: tracker state of the claim")
        rc, out = self.run("inventory", "--repo", code)
        self.check(rc == 0, "REQ-01 inventory exits 0 once the lease is released")

    def inventory_conflicts(self, code):
        """A conflict outside .jit leaves the judgement to the .jit part of the merge result."""
        for name in ("outside-conflict", "tracker-conflict"):
            self.sh("git", "branch", name, "main", cwd=code)
        side = self.path("conflict-fixture")
        self.sh("git", "worktree", "add", "-q", side, "outside-conflict", cwd=code)
        (Path(side) / "src" / "lib.rs").write_text("pub fn outside() {}\n")
        self.commit_all(side, "test: a code change that conflicts with main")
        self.sh("git", "switch", "-q", "tracker-conflict", cwd=side)
        (Path(side) / "src" / "lib.rs").write_text("pub fn outside() {}\n")
        (Path(side) / STORE / "config.toml").open("a").write("# branch side of a tracker conflict\n")
        self.commit_all(side, "test: code and tracker changes that conflict with main")
        self.sh("git", "worktree", "remove", side, cwd=code)
        (Path(code) / "src" / "lib.rs").write_text("pub fn inside() {}\n")
        (Path(code) / STORE / "config.toml").open("a").write("# main side of a tracker conflict\n")
        self.commit_all(code, "test: main changes code and tracker")
        rc, out = self.run("inventory", "--repo", code)
        lines = out.splitlines()
        self.check(merge_result(code, "main", "outside-conflict")[1] == ["src/lib.rs"],
                   "REQ-09 the fixture branch outside-conflict conflicts with main outside .jit only")
        self.check(any(l.startswith("info:") and "branch outside-conflict:" in l for l in lines)
                   and not any(l.startswith("action:") and "branch outside-conflict:" in l for l in lines),
                   "REQ-01 a branch that conflicts outside .jit and whose .jit merges cleanly is judged info")
        self.check(any(l.startswith("action:") and "branch tracker-conflict:" in l and f"{STORE}/config.toml" in l
                       for l in lines) and rc == 1,
                   "REQ-01 a branch whose .jit conflicts with main needs action and names the conflict")
        self.sh("git", "branch", "-D", "tracker-conflict", cwd=code)

    def archive(self, code):
        inside_dest = str(Path(code) / "docs" / "archives")
        rc, out = self.run("archive", "--repo", code, "--dest", inside_dest)
        self.check(rc == 2 and not os.path.exists(inside_dest),
                   "REQ-02 archive refuses a destination in the tracked tree")
        dest = self.path("archives")
        rc, out = self.run("archive", "--repo", code, "--dest", dest)
        print(out, end="")
        self.check(rc == 0, "REQ-02 archive writes and verifies each store")
        names = sorted(os.listdir(dest)) if os.path.isdir(dest) else []
        self.check(names == ["manifest.json", "primary.tar", "worktree-wa.tar", "worktree-wb.tar"],
                   f"REQ-02 one archive per store and a manifest: {names}")
        if not os.path.isfile(os.path.join(dest, "manifest.json")):
            return dest
        manifest = json.loads(Path(dest, "manifest.json").read_text())
        wb_files = manifest["worktree-wb"]["files"]
        self.check(wb_files.get(f"gate-runs/{I1}/r1.json") == hashlib.sha256(RUN_B).hexdigest(),
                   "REQ-02 the manifest holds the SHA-256 of each member, ignored run records included")
        with tarfile.open(os.path.join(dest, "worktree-wb.tar")) as tar:
            self.check(f"gate-runs/{I3}/r3.json" in tar.getnames(), "REQ-02 the archive holds the run records")
        rc, _ = self.run("archive", "--repo", code, "--dest", dest, "--verify")
        self.check(rc == 0, "REQ-02 archive --verify passes on unchanged stores")
        record = Path(self.path("wb")) / STORE / "gate-runs" / I3 / "r3.json"
        record.write_bytes(b"changed\n")
        rc, out = self.run("archive", "--repo", code, "--dest", dest, "--verify")
        self.check(rc == 1 and "r3.json" in out, "REQ-02 archive --verify reports a store that differs")
        record.write_bytes(b'{"run": "r3"}\n')
        rc, _ = self.run("archive", "--repo", code, "--dest", dest)
        self.check(rc == 2, "REQ-02 archive refuses to overwrite a written archive directory")
        partial = self.path("archives-partial")
        rc, _ = self.run("archive", "--repo", code, "--dest", partial, hooks={"step": interrupt_at("archived primary")})
        self.check(rc == "interrupted" and "manifest.json" not in os.listdir(partial),
                   "REQ-02 an interrupted archive writes no manifest")
        self.refused_unchanged(("archive", "--repo", code, "--dest", partial),
                               "REQ-08 archive refuses a rerun on a partial archive set and changes nothing")
        shutil.rmtree(partial)
        return dest

    def extract(self, code):
        older = git("rev-parse", "main~1", cwd=code).strip()
        stale = git("subtree", "split", f"--prefix={STORE}", older, cwd=code).strip().splitlines()[-1]
        new = self.path("tracker")
        cache = Path(git("rev-parse", "--absolute-git-dir", cwd=code).strip()) / "subtree-cache"
        before = set(os.listdir(cache)) if cache.is_dir() else set()
        rc, out = self.run("extract", "--repo", code, "--commit", "main", "--dest", new, "--split", stale)
        self.check(rc == 1 and not os.path.exists(new),
                   "REQ-03 extract fails on a planted tree mismatch and creates nothing")
        rc, out = self.run("extract", "--repo", code, "--commit", "main", "--dest", new)
        print(out, end="")
        self.check(rc == 0, "REQ-03 extract creates the tracker repository")
        if rc != 0:
            raise Failure("extract failed; later steps depend on it")
        head_tree = git("rev-parse", "HEAD^{tree}", cwd=new).strip()
        self.check(head_tree == git("rev-parse", "main:.jit", cwd=code).strip(),
                   "REQ-03 the head tree equals the .jit tree of the code commit")
        tracker_commits = int(git("rev-list", "--count", "main", "--", STORE, cwd=code))
        self.check(int(git("rev-list", "--count", "HEAD", cwd=new)) == tracker_commits,
                   "REQ-03 the new repository holds one commit per code commit that changed .jit")
        after = set(os.listdir(cache)) if cache.is_dir() else set()
        self.check(after <= before, "REQ-03 extract leaves no subtree cache in the code repository")
        return new

    def code_side(self, code):
        before = git("rev-parse", "HEAD", cwd=code).strip()
        rc, _ = self.run("code-side", "--repo", self.path("wa"), "--message", "chore: cut over")
        self.check(rc == 2, "REQ-04 code-side refuses a linked worktree")
        argv = ("code-side", "--repo", code, "--message", "chore: move the tracker out")
        for rel in (".gitignore", ".gitattributes", BUDGET, REFERENCES):
            target = Path(code) / rel
            original = target.read_bytes()
            target.write_bytes(original + b"# an ordinary edit\n")
            self.refused_unchanged(argv, f"REQ-08 code-side refuses an ordinary edit to {rel} and changes nothing")
            target.write_bytes(original)
        self.sh("git", "rm", "-q", "--cached", f"{STORE}/config.toml", cwd=code)
        self.refused_unchanged(argv, "REQ-08 code-side refuses an index under .jit that differs from HEAD")
        self.sh("git", "reset", "-q", "--", f"{STORE}/config.toml", cwd=code)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0, "REQ-04 code-side commits")
        if rc != 0:
            raise Failure("code-side failed; later steps depend on it")
        head = git("rev-parse", "HEAD", cwd=code).strip()
        self.check(git("rev-parse", "HEAD^", cwd=code).strip() == before, "REQ-04 exactly one commit")
        self.check(git("ls-files", STORE, cwd=code) == "", "REQ-04 no tracker path in the index")
        self.check((Path(code) / STORE / "config.toml").is_file(), "REQ-04 the store stays on disk")
        changed = set(git("diff", "--name-only", "HEAD^", "HEAD", cwd=code).split())
        allowed = {".gitignore", ".gitattributes", BUDGET, REFERENCES}
        self.check(all(p in allowed or p.startswith(".jit/") for p in changed),
                   f"REQ-04 the commit touches only the tracker and its four configuration files: {changed}")
        self.check("/.jit/" in git("show", "HEAD:.gitignore", cwd=code).splitlines(), "REQ-04 /.jit/ ignored")
        self.check(git_rc("check-ignore", "-q", f"{STORE}/issues/probe.json", cwd=code) == 0,
                   "REQ-04 git ignores a path under .jit/")
        attributes = git("show", "HEAD:.gitattributes", cwd=code, check=False)
        self.check("merge=union" not in attributes, "REQ-04 the union-merge attribute is gone")
        budget = tomllib.loads(git("show", f"HEAD:{BUDGET}", cwd=code))
        self.check(all(e["path"] != EVENTS for e in budget.get("exception", [])),
                   "REQ-04 the event log's budget exception is gone")
        checker = load_budget_checker(code)
        self.check(FORBID_STORE in budget["forbidden_paths"]
                   and checker.path_faults([f"{STORE}/issues/probe.json"], budget) != [],
                   "REQ-04 tracked paths under .jit/ are forbidden")
        size = sum(checker.tree_sizes(head, code).values())
        self.check(budget["max_tree_bytes"] == -(-size // TREE_STEP) * TREE_STEP,
                   f"REQ-04 max_tree_bytes {budget['max_tree_bytes']} is the tree size {size} rounded up to 10 kB")
        self.check(checker.head_faults(code) == [], "REQ-04 the budget checker passes at the cutover commit")
        references = tomllib.loads(git("show", f"HEAD:{REFERENCES}", cwd=code))
        paths = [e["path"] for e in references.get("exception", [])]
        self.check(EVENTS not in paths and CITATIONS not in paths,
                   "REQ-04 the event log and citation registry reference exceptions are gone")
        self.check(".jit/issues/*" in paths and ".jit/archive-evidence/*" in paths,
                   "REQ-04 the issue-prose and archive-record reference exceptions stay")
        self.refuse_after_each(argv, steps, "code-side")
        return git("rev-parse", "HEAD", cwd=code).strip()

    def mount(self, code, new):
        aside = self.path("primary-store-aside")
        rc, out = self.run("mount", "--repo", self.path("wa"), "--tracker", new, "--aside", aside)
        self.check(rc == 2 and not os.path.exists(aside), "REQ-05 mount refuses a linked worktree")
        argv = ("mount", "--repo", code, "--tracker", new, "--aside", aside)
        os.mkdir(aside)
        self.refused_unchanged(argv, "REQ-08 mount refuses an existing aside path and changes nothing")
        os.rmdir(aside)
        stage = Path(aside).with_name(f".{Path(aside).name}.mount-stage")
        stage.mkdir()
        self.refused_unchanged(argv, "REQ-08 mount refuses an existing staging path and changes nothing")
        stage.rmdir()
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0, "REQ-05 mount replaces .jit with a clone")
        if rc != 0:
            raise Failure("mount failed; later steps depend on it")
        store = Path(code) / STORE
        self.check(git("rev-parse", "--show-toplevel", cwd=store).strip() == str(store.resolve()),
                   "REQ-05 .jit is the top level of its own repository")
        tree = git("ls-tree", "-r", "-z", "HEAD", cwd=new).split("\0")
        same = True
        for entry in filter(None, tree):
            meta, rel = entry.split("\t", 1)
            blob = subprocess.run(["git", "cat-file", "blob", meta.split()[2]], cwd=new,
                                  capture_output=True, check=True).stdout
            same = same and (store / rel).read_bytes() == blob
        tracked = set(filter(None, git("ls-files", "-z", cwd=store).split("\0")))
        self.check(same and tracked == {e.split("\t", 1)[1] for e in filter(None, tree)},
                   "REQ-05 the tracked files are byte-identical to the extracted tree")
        for rel in (f"gate-runs/{I1}/r1.json", "gate-runs/verdicts/cargo-ci/d1.json", "server.log", "worktree.json"):
            self.check((store / rel).is_file() and (store / rel).read_bytes() == (Path(aside) / rel).read_bytes(),
                       f"REQ-05 machine-local {rel} is restored")
        self.check(not (store / "tmp" / "redo.record").exists(), "REQ-05 jit's scratch directory is not restored")
        self.check(git("status", "--porcelain", cwd=code) == "", "REQ-05 the code checkout stays clean")
        self.check(not stage.exists(), "REQ-05 mount leaves no staging path")
        self.refuse_after_each(argv, steps, "mount")

    def harvest(self, code):
        shared = str(Path(code) / STORE)
        wa, wb = Path(self.path("wa")) / STORE, Path(self.path("wb")) / STORE
        runs = Path(shared) / "gate-runs"
        rc, out = self.run("harvest", "--store", shared, "--from", str(wb))
        self.check(rc == 1 and f"{I1}/r1.json" in out, "REQ-06 harvest stops on a run-record collision")
        self.check(not (runs / I3 / "r3.json").exists(), "REQ-06 a stopped harvest copies nothing")
        self.check((runs / I1 / "r1.json").read_bytes() == RUN_A, "REQ-06 the store's run record is untouched")
        (wb / "gate-runs" / I1 / "r1.json").unlink()
        rc, out = self.run("harvest", "--store", shared, "--from", str(wb))
        self.check(rc == 1 and "r9" in out and not (runs / I3 / "r3.json").exists(),
                   "REQ-06 harvest stops on a pointer whose run the store would not hold")
        (wb / "gate-runs" / "verdicts" / "cargo-ci" / "d3.json").unlink()
        rc, out = self.run("harvest", "--store", shared, "--from", str(wb))
        self.check(rc == 0 and (runs / I3 / "r3.json").is_file(), "REQ-06 harvest copies a new run record")
        rc, out = self.run("harvest", "--store", shared, "--from", str(wa))
        print(out, end="")
        self.check(rc == 0, "REQ-06 harvest passes with identical records and a pointer collision")
        self.check((runs / I2 / "r2.json").is_file(), "REQ-06 one file per run copied")
        self.check((runs / "verdicts" / "cargo-ci" / "d1.json").read_bytes() == pointer(I1, "r1"),
                   "REQ-06 a differing verdict pointer keeps the store's")
        self.check((runs / "verdicts" / "cargo-ci" / "d2.json").read_bytes() == pointer(I2, "r2"),
                   "REQ-06 a pointer the store lacks is copied")
        rc, _ = self.run("harvest", "--store", shared, "--from", str(wa))
        self.check(rc == 0, "REQ-06 a repeated harvest passes")

    def abandon(self, code):
        for name in ("landed-a", "landed-b", "landed-c", "landed-d"):
            self.sh("git", "branch", name, "main", cwd=code)
        self.sh("git", "update-ref", "refs/archive/landed-b", "main~1", cwd=code)
        tip = git("rev-parse", "landed-a", cwd=code).strip()
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-a")
        self.check(rc == 0 and out.splitlines() == ["landed-a: abandoned to refs/archive/landed-a"]
                   and rev("refs/archive/landed-a", code) == tip and rev("refs/heads/landed-a", code) is None,
                   "REQ-10 abandon moves the branch to refs/archive/<name> and deletes the branch ref")
        before = self.observe()
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-a")
        self.check(rc == 0 and out.splitlines() == ["landed-a: already abandoned"] and self.observe() == before,
                   "REQ-10 a rerun reports already abandoned and changes nothing")
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-b")
        self.check(rc == 1 and out.startswith("landed-b: refused: refs/archive/landed-b exists")
                   and self.observe() == before,
                   "REQ-10 abandon refuses an existing archive ref and changes nothing")
        rc, out = self.run("abandon", "--repo", code, "--branch", "wa")
        self.check(rc == 1 and out.startswith("wa: refused: wa is checked out in") and self.observe() == before,
                   "REQ-10 abandon refuses a branch checked out in a worktree and changes nothing")
        listed = Path(self.path("abandon.txt"))
        listed.write_text("# landed branches\nlanded-c\nwa # checked out\nlanded-d\n")
        rc, out = self.run("abandon", "--repo", code, "--branches", str(listed))
        lines = out.splitlines()
        self.check(rc == 1 and len(lines) == 3 and lines[0] == "landed-c: abandoned to refs/archive/landed-c"
                   and lines[1].startswith("wa: refused:") and lines[2] == "landed-d: abandoned to refs/archive/landed-d",
                   "REQ-10 a batch prints one line per branch, continues after a refusal and exits 1")
        self.sh("git", "update-ref", "-d", "refs/archive/landed-b", cwd=code)
        self.sh("git", "branch", "-D", "landed-b", cwd=code)

    def outside(self, checkout):
        """Digest of every file of `checkout` outside `.jit` and its git link, untracked and ignored files included."""
        files = {}
        for root, dirs, names in os.walk(checkout):
            here = Path(root)
            if here == Path(checkout):
                dirs[:] = [d for d in dirs if d not in (STORE, ".git")]
            for name in names:
                path = here / name
                rel = path.relative_to(checkout).as_posix()
                if rel != ".git":
                    files[rel] = sha256(path)
        return files

    def clean(self, code, cutover_state):
        shared = str(Path(code) / STORE)
        wa, wb = self.path("wa"), self.path("wb")
        args = ("--store", shared, "--message", "chore: drop the tracker")
        if cutover_state == "before":
            rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
            self.check(rc == 1, "REQ-07 clean fails while main tracks .jit")
            return
        (Path(wb) / EVENTS).open("a").write('{"event": "late"}\n')
        (Path(wb) / STORE / "issues" / "late.json").write_text('{"late": true}\n')
        self.commit_all(wb, "chore: tracker changes on wb after the cutover")
        start = git("rev-parse", "wb", cwd=code).strip()
        (Path(wb) / "notes.txt").write_text("untracked\n")
        (Path(wb) / "target" / "debug").mkdir(parents=True)
        (Path(wb) / "target" / "debug" / "build.log").write_text("ignored\n")
        (Path(wb) / STORE / "config.toml").open("a").write("# uncommitted tracker edit\n")
        self.check(git_rc("check-ignore", "-q", "target/debug/build.log", cwd=wb) == 0
                   and "?? notes.txt" in git("status", "--porcelain", cwd=wb),
                   "REQ-09 the fixture worktree wb is dirty with an untracked and an ignored file")
        files = self.outside(wb)
        rc, out = self.run("clean", "--repo", code, "--worktree", wb, *args)
        print(out, end="")
        self.check(rc == 0 and "wb: cleaned; tree outside .jit equals the tip" in out.splitlines(),
                   "REQ-07 clean passes on wb and reports tree equality outside .jit")
        self.check(self.outside(wb) == files,
                   "REQ-08 the files of the dirty worktree wb outside .jit are byte-identical after its clean")
        self.check(git("rev-parse", "wb^", cwd=code).strip() == start
                   and len(git("rev-list", "--parents", "-n1", "wb", cwd=code).split()) == 2
                   and git_rc("diff", "--quiet", start, "wb", "--", ".", ":(exclude).jit", cwd=code) == 0,
                   "REQ-07 wb gets one commit on its tip, merging nothing, with the tip's tree outside .jit")
        self.check(git("ls-tree", "-r", "--name-only", "wb", "--", STORE, cwd=code) == ""
                   and git("ls-tree", "-r", "--name-only", "main", "--", STORE, cwd=code) == ""
                   and git("ls-files", STORE, cwd=wb) == "",
                   "REQ-07 git ls-files .jit is empty on main, on wb and in the index of wb")
        self.check(git("status", "--porcelain", "--untracked-files=no", cwd=wb) == "",
                   "REQ-07 the index of wb matches its new HEAD")
        self.check(not (Path(wb) / STORE).exists(), "REQ-07 wb's local store is deleted")
        lib = Path(wa) / "src" / "lib.rs"
        original = lib.read_bytes()
        lib.write_bytes(original + b"// uncommitted\n")
        self.refused_unchanged(("clean", "--repo", code, "--worktree", wa, *args),
                               "REQ-07 clean refuses uncommitted tracked changes outside .jit and changes nothing")
        lib.write_bytes(original)
        extra = Path(wa) / STORE / "gate-runs" / I2 / "unharvested.json"
        extra.write_bytes(b'{"run": "late"}\n')
        self.refused_unchanged(("clean", "--repo", code, "--worktree", wa, *args),
                               "REQ-07 clean refuses an unverified harvest and keeps the store")
        extra.unlink()
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 0 and not (Path(wa) / STORE).exists()
                   and "wa: cleaned; tree outside .jit equals the tip" in out.splitlines()
                   and git("ls-tree", "-r", "--name-only", "wa", "--", STORE, cwd=code) == "",
                   "REQ-07 clean passes on wa")
        head = git("rev-parse", "wa", cwd=code).strip()
        local = Path(wa) / STORE
        local.mkdir()
        (local / "server.log").write_text("machine-local\n")
        self.write_runs(local, {f"{I2}/r2.json": b'{"run": "r2"}\n'})
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 0 and "wa: already clean" in out.splitlines()
                   and git("rev-parse", "wa", cwd=code).strip() == head and not local.exists(),
                   "REQ-07 an already clean linked branch commits nothing and deletes its verified store")

    def parked_setup(self, code):
        fixture = self.path("parked-fixture")
        self.sh("git", "worktree", "add", "-q", "-b", "parked", fixture, "main", cwd=code)
        (Path(fixture) / STORE / "config.toml").open("a").write("# parked tracker change\n")
        (Path(fixture) / STORE / "issues" / "parked.json").write_text('{"parked": true}\n')
        self.commit_all(fixture, "chore: parked tracker changes")
        for name in ("parked-single", "parked-empty"):
            self.sh("git", "branch", name, "parked", cwd=code)
        self.sh("git", "switch", "-q", "-c", "parked-conflict", "main", cwd=fixture)
        (Path(fixture) / "src" / "lib.rs").write_text("pub fn parked() {}\n")
        (Path(fixture) / STORE / "config.toml").open("a").write("# conflicting branch tracker\n")
        self.commit_all(fixture, "chore: parked code conflict")
        self.sh("git", "worktree", "remove", fixture, cwd=code)
        (Path(code) / "src" / "lib.rs").write_text("pub fn main_branch() {}\n")
        self.commit_all(code, "chore: main code change")

    def parked_clean(self, code):
        args = ("--message", "chore: drop the tracker")
        self.sh("git", "branch", "parked-already", "main", cwd=code)
        store = Path(code) / STORE
        (store / "issues" / "primary-only.json").write_text('{"primary": true}\n')
        primary = {k: v for k, v in store_files(store).items() if not k.startswith(".git/")}
        self.check(git("show", f"parked-single:{STORE}/config.toml", cwd=code).encode()
                   != (store / "config.toml").read_bytes(),
                   "REQ-09 the primary checkout's .jit differs from the parked branch's tracked .jit")
        worktrees = git("worktree", "list", "--porcelain", cwd=code)
        single = git("rev-parse", "parked-single", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-single", *args)
        print(out, end="")
        self.check(rc == 0 and out.strip() == "parked-single: cleaned; tree outside .jit equals the tip",
                   "REQ-07 a single parked branch cleans without a shared store or harvest")
        self.check(not tracked_under("parked-single", STORE, code)
                   and git("rev-parse", "parked-single^", cwd=code).strip() == single
                   and len(git("rev-list", "--parents", "-n1", "parked-single", cwd=code).split()) == 2
                   and git_rc("diff", "--quiet", single, "parked-single", "--", ".", ":(exclude).jit", cwd=code) == 0,
                   "REQ-07 a parked branch gets one commit on its tip with the tip's tree outside .jit")
        self.check({k: v for k, v in store_files(store).items() if not k.startswith(".git/")} == primary
                   and git("worktree", "list", "--porcelain", cwd=code) == worktrees,
                   "REQ-07 a parked clean leaves the primary checkout's .jit and the worktree list as they were")
        for branch in ("parked-conflict", "outside-conflict"):
            tip = git("rev-parse", branch, cwd=code).strip()
            conflicts = merge_result(code, "main", branch)[1]
            self.check([p for p in conflicts if not p.startswith(f"{STORE}/")] == ["src/lib.rs"],
                       f"REQ-09 the fixture branch {branch} conflicts with main outside .jit")
            rc, out = self.run("clean", "--repo", code, "--branch", branch, *args)
            self.check(rc == 0 and out.strip() == f"{branch}: cleaned; tree outside .jit equals the tip"
                       and git("rev-parse", f"{branch}^", cwd=code).strip() == tip
                       and git_rc("diff", "--quiet", tip, branch, "--", ".", ":(exclude).jit", cwd=code) == 0
                       and not tracked_under(branch, STORE, code),
                       f"REQ-07 {branch}, which conflicts with main outside .jit, is cleaned with its code tree unchanged")
        already = git("rev-parse", "parked-already", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-already", *args)
        self.check(rc == 0 and out.strip() == "parked-already: already clean"
                   and git("rev-parse", "parked-already", cwd=code).strip() == already,
                   "REQ-07 an already clean parked branch commits nothing")
        branches = Path(self.path("branches.txt"))
        branches.write_text("# parked branches\nparked\nmissing-branch\nwa # checked out\n\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        print(out, end="")
        lines = out.splitlines()
        self.check(rc == 1 and len(lines) == 4
                   and lines[0] == "parked: cleaned; tree outside .jit equals the tip"
                   and lines[1] == "missing-branch: refused: not a local branch: missing-branch"
                   and lines[2].startswith("wa: refused: wa is checked out in")
                   and lines[3] == "parked-already: already clean",
                   "REQ-07 a batch lists each outcome, continues after refusals and exits 1")
        self.check(all(not tracked_under(r, STORE, code) for r in ("main", "parked", "parked-already")),
                   "REQ-07 main and every cleaned branch track no tracker path")
        branches.write_text("parked\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        self.check(rc == 0 and out.splitlines() == ["parked: already clean", "parked-already: already clean"],
                   "REQ-07 a batch of clean branches succeeds without commits")
        self.refused_unchanged(("clean", "--repo", code, "--branch", "parked-empty", "--message", ""),
                               "REQ-07 clean refuses an empty commit message and changes nothing")


class Interrupted(Exception):
    """Raised by a self-test hook to stop a subcommand between two of its steps."""


def interrupt_at(target):
    def hook(step):
        if step == target:
            raise Interrupted(step)
    return hook


def copy_entry(source, target):
    if source.is_dir() and not source.is_symlink():
        shutil.copytree(source, target, symlinks=True)
    else:
        shutil.copy2(source, target, follow_symlinks=False)


def remove_entry(path):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink()


def pointer(issue, run):
    return (json.dumps({"issue_id": issue, "run_id": run}, indent=2) + "\n").encode()


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def inside(path, root):
    path, root = Path(os.path.realpath(path)), Path(os.path.realpath(root))
    return path == root or root in path.parents


def load_budget_checker(repo):
    spec = importlib.util.spec_from_file_location("check_tracked_budget", Path(repo) / BUDGET_CHECKER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def self_test():
    try:
        invoking = git("rev-parse", "--show-toplevel", cwd=os.getcwd()).strip()
    except Failure as failure:
        print(f"cutover.py --self-test: the fixture needs the invoking checkout: {failure}", file=sys.stderr)
        return 2
    for name in CLEARED_ENV:
        os.environ.pop(name, None)
    base = Path(tempfile.mkdtemp(prefix="cutover-self-test-"))
    if inside(base, invoking):
        shutil.rmtree(base)
        print(f"cutover.py --self-test: refused: scratch directory {base} is inside {invoking}", file=sys.stderr)
        return 2
    config = base / "gitconfig"
    config.write_text("[user]\n\tname = cutover self-test\n\temail = self-test@invalid\n")
    os.environ.update(GIT_CONFIG_GLOBAL=str(config), GIT_CONFIG_NOSYSTEM="1",
                      HOME=str(base), XDG_CONFIG_HOME=str(base / ".config"))
    test = SelfTest(base, invoking)
    if not test.jit:
        print("  skip jit-backed cases: jit is not on PATH; the store is fabricated")
    try:
        code = test.build()
        print("inventory")
        test.inventory(code)
        print("abandon")
        test.abandon(code)
        print("archive")
        archives = test.archive(code)
        print("extract")
        new = test.extract(code)
        test.clean(code, "before")
        test.parked_setup(code)
        print("code-side")
        test.code_side(code)
        print("mount")
        test.mount(code, new)
        print("harvest")
        test.harvest(code)
        print("clean")
        test.clean(code, "after")
        test.parked_clean(code)
    except (Failure, Refusal, subprocess.CalledProcessError, OSError, KeyError) as error:
        test.failures.append(f"aborted: {error}")
        print(f"  FAIL aborted: {error}")
    finally:
        shutil.rmtree(base, ignore_errors=True)
    if test.failures:
        print(f"cutover.py --self-test: {len(test.failures)} of {test.cases} checks failed", file=sys.stderr)
        return 1
    print(f"cutover.py --self-test: ok, {test.cases} checks")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
