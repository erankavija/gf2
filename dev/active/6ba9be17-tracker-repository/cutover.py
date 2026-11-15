#!/usr/bin/env python3
"""Moves the tracker store out of the code repository into its own repository.

The cutover turns `.jit/` of a code repository into an independent git
repository mounted at `.jit/` of the primary checkout, carries each linked
worktree's tracker state through that change and undoes it after a failed
verification. Each subcommand is one step of `cutover-sequence` in
`plan.md`. Repositories are arguments; every other location comes from git.

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
      Moves each branch to `refs/archive/<B>` and deletes `refs/heads/<B>`,
      both by compare-and-swap from the branch tip. FILE lists one branch per
      line with optional `#` comments. Prints one line per branch:
      `abandoned to refs/archive/<B>`, `already abandoned` when the archive
      ref exists and the branch ref does not, `refused: <reason>` or
      `failed: <reason>`. Refuses an existing archive ref beside the branch
      ref and a branch checked out in any worktree (detach that worktree
      first). A batch continues after failures and exits 1 when any branch is
      not abandoned.
  cutover.py archive --repo CODE --dest DIR [--verify]
      Writes the store of every checkout to `DIR/<name>.tar`, `<name>` being
      `primary` or `worktree-<name git registers>`, and the SHA-256 of every
      member to `DIR/manifest.json`; then re-reads each archive and compares
      it, and each store still on disk, with the manifest. `--verify` only
      re-reads; a store a clean deleted is compared with its archive alone.
      Each archive and the manifest are written under a `.partial` name and
      renamed. Refuses a DIR inside a store, inside the git directory or
      unignored inside a checkout, and, when writing, a DIR holding files.
  cutover.py extract --repo CODE --commit REV --dest NEW [--split REV]
      Splits the history of `.jit` up to REV with `git subtree split` in a
      temporary git directory (or takes the split commit `--split` names) and
      creates the repository NEW with it as `main`, staged at
      `.<NEW name>.extract-stage` and renamed into place. Fails, creating
      nothing, unless the split tree and the head tree of NEW equal the tree
      of `REV:.jit`. Refuses an existing NEW or stage.
  cutover.py code-side --repo PRIMARY --message MSG [--main BRANCH]
      Makes one commit on the main branch: the tracker paths leave the index
      (their files stay on disk), `.gitignore` ignores `/.jit/`,
      `.gitattributes` loses the union-merge attribute of the event log (and
      the file goes when nothing else remains), `scripts/tracked-budget.toml`
      loses the event log's exception, forbids `^\\.jit/` and lowers
      `max_tree_bytes` to the committed tree size, as
      `scripts/check-tracked-budget.py` measures it, rounded up to the next
      10 kB; `scripts/reference-exceptions.toml` loses the exceptions for the
      event log and `.jit/references.toml`. The commit is built with
      `commit-tree` and published by a compare-and-swap of the branch, so no
      commit hook runs. Refuses a linked worktree, another branch, any staged
      change, an unstaged change to those files, or an index without tracker
      paths.
  cutover.py mount --repo PRIMARY --tracker NEW --aside PATH
      Clones NEW into `.jit.mount-stage`, moves `.jit` to PATH, moves the
      clone into place and copies back the machine-local files: those the
      patterns of the store's `.gitignore` outside jit's marker region
      match. Fails unless every tracked file of the mount is byte-identical
      to the head tree of NEW. Refuses a linked worktree, a code repository
      that still tracks or does not ignore `.jit`, an existing PATH or stage
      and a PATH on another filesystem than `.jit`.
  cutover.py harvest --store SHARED --from STORE
      Copies the run records of STORE, one file per run under
      `gate-runs/<issue>/`, into SHARED and copies each verdict pointer
      (`gate-runs/verdicts/<gate>/<inputs>.json`) SHARED lacks. Where both
      hold a pointer with different bytes it keeps SHARED's. Stops, copying
      nothing, on a run record SHARED holds with different bytes or on a
      pointer whose run SHARED would not hold; then verifies the harvest.
  cutover.py clean --repo CODE --worktree WT --store SHARED --message MSG
                [--main BRANCH]
      Gives the branch of the linked worktree WT one commit whose tree is
      the branch tip's tree without `.jit`, made with `commit-tree` (no hook
      runs) and published by a compare-and-swap of the branch; removes the
      `.jit` entries from WT's index and leaves every file on disk; then
      deletes `WT/.jit` once its harvest into SHARED is verified and its
      bytes match the full-store tar retained before the commit as a blob
      under `refs/cutover-archives/`. A branch without tracked tracker paths
      is already clean: nothing is committed and the verified store is still
      deleted. Fails unless `git ls-files .jit` is empty on the main branch
      and on the branch and the commit's tree outside `.jit` equals the
      tip's. Refuses an unverified harvest, uncommitted tracked changes, an
      empty message and a detached WT. A failure after the branch moved puts
      it back at the tip and restores the index, and says so.
  cutover.py clean --repo CODE (--branch B | --branches FILE) --message MSG
                [--main BRANCH]
      Cleans parked branches the same way, without a checkout. FILE lists
      one branch per line with optional `#` comments. Prints one line per
      branch: `cleaned; tree outside .jit equals the tip's`, `already clean`
      when no tracker path is tracked (nothing committed), `refused:
      <reason>` or `failed: <reason>`. Refuses a branch checked out in any
      worktree. A batch continues after failures and exits 1 when any branch
      is neither cleaned nor already clean; 2 only for a usage error or an
      unreadable FILE.
  cutover.py rollback --repo PRIMARY --cutover REV --aside PATH --message MSG
                   [--archives DIR] [--main BRANCH]
      Moves the mounted tracker repository to PATH, reverts the code-side
      commit REV and copies the moved store's files over the restored
      `.jit`, deleting the tracked paths the store no longer has, in one
      commit built with `commit-tree`. With `--archives`, restores each
      linked worktree's store that a clean removed from its verified archive,
      extracting into `.jit.rollback-stage` checked against the manifest and
      renamed into place. A tracked file missing from the store counts as
      deleted. Refuses a HEAD that tracks `.jit`, an existing PATH or stage,
      PATH on another filesystem than `.jit`, uncommitted tracked changes, a
      REV that is not a code-side commit on the main branch, and a revert
      that conflicts (nothing is changed).
  cutover.py --self-test
      Runs every subcommand on scratch repositories under a new directory
      in TMPDIR and refuses a scratch path inside the invoking checkout. The
      fixture's ignore, attribute and budget files are those committed at
      HEAD of the invoking checkout; the lease, redo-record and port cases
      use the real `jit` and are skipped with a printed reason when it is
      absent.

Interruption. A subcommand that writes keeps an operation marker,
`cutover-state.json` in the git directory of the checkout it runs in
(`--worktree` for a linked clean, the shared store's checkout for harvest,
`--repo` otherwise). The marker is written to a temporary name, fsynced and
renamed, and the directory fsynced, before and after each step. It records
the operation and its canonical arguments, the starting HEAD, the content
identity (SHA-256 and mode, absence, directory manifests) of each location
the operation writes, each completed step and each commit or ref the
operation made. A rerun with a marker resumes after its last completed step
only when the arguments match, HEAD is the starting HEAD or a commit the
operation made, and every such location holds what the marker allows at that
step; otherwise it refuses, names the differences and changes nothing. A run
without a marker starts fresh and so refuses existing outputs. The tool
deletes or replaces only content it wrote under a marker or archived and
verified. A failed step keeps the marker unless nothing was published yet.
The success of a run removes its marker. Inventory and `archive --verify`
write nothing.

Exit status: 0 on success or, for the inventory, nothing needing action; 1
when a step or its verification fails or the inventory lists an action; 2 on
a usage error or a refusal.
"""

import argparse
import contextlib
import fcntl
import hashlib
import importlib.util
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import uuid
from pathlib import Path

STORE = ".jit"
BUDGET = "scripts/tracked-budget.toml"
BUDGET_CHECKER = "scripts/check-tracked-budget.py"
REFERENCES = "scripts/reference-exceptions.toml"
CONFIG_FILES = (".gitignore", ".gitattributes", BUDGET, REFERENCES)
EVENTS = ".jit/events.jsonl"
CITATIONS = ".jit/references.toml"
FORBID_STORE = r"^\.jit/"
TREE_STEP = 10_000
MARKER_BEGIN = "# jit:data-root-ignore:begin"
MARKER_END = "# jit:data-root-ignore:end"
STATE = "cutover-state.json"
STATE_VERSION = 2
ZERO = "0" * 40
CLEANED = "cleaned; tree outside .jit equals the tip's"


class Refusal(Exception):
    """A precondition the step requires does not hold; nothing was changed."""


class Failure(Exception):
    """The step or its verification failed."""


def git(*args, cwd, check=True, input=None, env=None):
    result = git_result(args, cwd, input, env)
    if check and result.returncode != 0:
        raise Failure(f"git {' '.join(args)} in {cwd}: {result.stderr.strip()}")
    return result.stdout


def git_rc(*args, cwd):
    return git_result(args, cwd).returncode


def git_result(args, cwd, input=None, env=None):
    command = ["git", "--no-optional-locks", *args]
    environment = dict(os.environ, **(env or {}))
    if args[0] == "diff" and "GIT_INDEX_FILE" not in (env or {}):
        # Diff may refresh the index even without optional locks; reading a
        # private copy keeps the caller's index byte for byte.
        index = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-path", "index"],
                               cwd=cwd, capture_output=True, text=True).stdout.strip()
        with tempfile.TemporaryDirectory(prefix="cutover-read-") as scratch:
            private = Path(scratch) / "index"
            if index and Path(index).exists():
                shutil.copy2(index, private)
            environment["GIT_INDEX_FILE"] = str(private)
            return subprocess.run(command, cwd=cwd, capture_output=True, text=True, input=input,
                                  env=environment)
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True, input=input, env=environment)


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
    p = sub.add_parser("clean", description="Remove every tracked tracker path in one commit.",
                       epilog="--branches lists one branch per line with # comments. Each branch prints "
                       "its outcome; any branch not cleaned makes the exit status 1.")
    p.add_argument("--repo", required=True)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument("--worktree", help="linked worktree whose harvest must be verified")
    target.add_argument("--branch", help="parked local branch without a worktree")
    target.add_argument("--branches", help="file listing parked local branches")
    p.add_argument("--store", help="shared store; required with --worktree only")
    p.add_argument("--message", required=True)
    p.add_argument("--main", default="main")
    p = sub.add_parser("rollback")
    p.add_argument("--repo", required=True)
    p.add_argument("--cutover", required=True)
    p.add_argument("--aside", required=True)
    p.add_argument("--message", required=True)
    p.add_argument("--archives")
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


def output_location(path):
    """Resolve parents without turning a dangling output symlink into absence."""
    value = Path(path)
    if not value.is_absolute():
        value = Path.cwd() / value
    if value.name in ("", ".", ".."):
        return physical(value)
    return physical(value.parent) / value.name


def present(path):
    return path.exists() or path.is_symlink()


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


def rev(name, repo):
    """The object `name` resolves to, or None."""
    return git("rev-parse", "-q", "--verify", f"{name}^{{object}}", cwd=repo, check=False).strip() or None


def require_branch(checkout, branch):
    current = git("symbolic-ref", "-q", "--short", "HEAD", cwd=checkout, check=False).strip()
    if current != branch:
        raise Refusal(f"{checkout} has {current or 'a detached HEAD'} checked out, not {branch}")


def worktree_listing(checkout, *args):
    return [p for p in git("ls-files", "-z", *args, cwd=checkout).split("\0") if p]


def changed_paths(repo, *args):
    return {p for p in git("diff", "-z", "--name-only", *args, cwd=repo).split("\0") if p}


def index_tree(checkout):
    """The tree the index of `checkout` holds, written from a private copy of the index."""
    index = Path(git("rev-parse", "--path-format=absolute", "--git-path", "index", cwd=checkout).strip())
    with tempfile.TemporaryDirectory(prefix="cutover-index-") as scratch:
        private = Path(scratch) / "index"
        if index.exists():
            shutil.copy2(index, private)
        return git("write-tree", cwd=checkout, env={"GIT_INDEX_FILE": str(private)}).strip()


def blob_of(rev_name, path, repo):
    """(blob, mode) of `rev:path`, or None when absent."""
    out = git("ls-tree", "-z", rev_name, "--", path, cwd=repo).split("\0")[0]
    if not out:
        return None
    meta, _ = out.split("\t", 1)
    mode, _, blob = meta.split()
    return [blob, mode]


def file_blob(path, repo):
    """(blob, mode) of the file at `path` as git would record it, or None when absent."""
    if not present(path):
        return None
    if path.is_symlink() or not path.is_file():
        return ["unsupported", "0"]
    blob = git("hash-object", "--no-filters", "--", str(path), cwd=repo).strip()
    return [blob, "100755" if path.stat().st_mode & stat.S_IXUSR else "100644"]


def same_filesystem(a, b):
    return os.stat(a).st_dev == os.stat(b).st_dev


def own_repository(path):
    """Whether `path` is the top level of a git repository of its own."""
    if not path.is_dir() or not (path / ".git").exists():
        return False
    top = git("rev-parse", "--show-toplevel", cwd=path, check=False).strip()
    return bool(top) and physical(top) == physical(path)


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


def node(path):
    """The content identity of one path: kind, mode and SHA-256 of its bytes."""
    if not present(path):
        return {"kind": "absent"}
    mode = stat.S_IMODE(path.lstat().st_mode)
    if path.is_symlink():
        return {"kind": "symlink", "mode": mode,
                "sha256": hashlib.sha256(os.fsencode(os.readlink(path))).hexdigest()}
    if path.is_dir():
        return {"kind": "directory", "mode": mode}
    if path.is_file():
        return {"kind": "file", "mode": mode, "sha256": sha256(path)}
    raise Refusal(f"unsupported file type: {path}")


def relative_manifest(root):
    """Relative path to identity of `root` and everything under it, ignored and untracked entries included."""
    result = {".": node(Path(root))}

    def visit(directory, prefix):
        with os.scandir(directory) as found:
            entries = sorted(found, key=lambda e: e.name)
        for entry in entries:
            rel, info = prefix + entry.name, entry.stat(follow_symlinks=False)
            mode = stat.S_IMODE(info.st_mode)
            if stat.S_ISLNK(info.st_mode):
                result[rel] = {"kind": "symlink", "mode": mode,
                               "sha256": hashlib.sha256(os.fsencode(os.readlink(entry.path))).hexdigest()}
            elif stat.S_ISDIR(info.st_mode):
                result[rel] = {"kind": "directory", "mode": mode}
                visit(entry.path, rel + "/")
            elif stat.S_ISREG(info.st_mode):
                result[rel] = {"kind": "file", "mode": mode, "sha256": sha256(entry.path)}
            else:
                raise Refusal(f"unsupported file type: {entry.path}")

    if result["."]["kind"] == "directory":
        visit(root, "")
    return result


def location(path):
    """The identity of a location: absent, or the manifest of what is there."""
    return {"kind": "absent"} if not present(path) else relative_manifest(path)


def differences(expected, actual):
    if expected.get("kind") == "absent" or actual.get("kind") == "absent":
        return [] if expected == actual else ["."]
    return [p for p in sorted(set(expected) | set(actual)) if expected.get(p) != actual.get(p)]


def expect_location(path, allowed, label):
    """Refuse unless `path` holds one of the `allowed` identities; returns the index it matches."""
    actual = location(path)
    for number, expected in enumerate(allowed):
        if actual == expected:
            return number
    diff = differences(allowed[-1], actual)
    raise Refusal(f"{label} {path} holds content this operation did not record; "
                  f"changes: {', '.join(diff[:20]) or path}")


def copy_file(source, target, identity=None):
    """Copies `source` over an absent or identical `target` through an exclusive stage."""
    target.parent.mkdir(parents=True, exist_ok=True)
    stage = target.with_name(f".{target.name}.cutover-stage")
    try:
        handle = stage.open("xb")
    except FileExistsError:
        raise Refusal(f"an unowned copy stage exists; changes: {stage}")
    try:
        with handle, source.open("rb") as original:
            shutil.copyfileobj(original, handle)
        shutil.copystat(source, stage)
        if identity is not None and node(stage) != identity:
            raise Refusal(f"{source} changed since the operation recorded it; changes: {source}")
        os.replace(stage, target)
    except BaseException:
        stage.unlink(missing_ok=True)
        raise


def write_bytes(target, data, mode):
    """Writes `data` over `target` through an exclusive stage."""
    stage = target.with_name(f".{target.name}.cutover-stage")
    try:
        handle = stage.open("xb")
    except FileExistsError:
        raise Refusal(f"an unowned write stage exists; changes: {stage}")
    try:
        with handle:
            handle.write(data)
        stage.chmod(mode)
        os.replace(stage, target)
    except BaseException:
        stage.unlink(missing_ok=True)
        raise


def step(args, name):
    """Marks a point in a subcommand; the self-test interrupts or fails a subcommand here."""
    args.hooks.get("step", lambda _: None)(name)


def move(args, source, target, name=None):
    """Renames `source` to the absent `target`; `name` marks the point before the marker records it."""
    if present(target):
        raise Refusal(f"{target} appeared; changes: {target}")
    os.rename(source, target)
    if name:
        step(args, f"{name}: unrecorded")


def compare_and_swap(args, repo, ref, new, old, name, message):
    """Moves `ref` from `old` to `new` (None deletes), refusing any other current value."""
    current = rev(ref, repo)
    if current == new:
        return
    if current != old:
        raise Refusal(f"{ref} is at {current}, neither {old} nor the operation's {new}; changes: {ref}")
    if new is None:
        git("update-ref", "-m", message, "-d", ref, old, cwd=repo)
    else:
        git("update-ref", "-m", message, ref, new, old or ZERO, cwd=repo)
    step(args, f"{name}: unrecorded")


def commit_tree(repo, tree, parent, message):
    return git("commit-tree", tree, "-p", parent, "-m", message, cwd=repo).strip()


# Operation markers ------------------------------------------------------------


def state_path(checkout):
    return Path(git("rev-parse", "--path-format=absolute", "--git-path", STATE, cwd=checkout).strip())


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_marker(path, record):
    """Publish a complete marker, including across loss of the writing process."""
    fd, temporary = tempfile.mkstemp(prefix=f".{STATE}-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(record, handle, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        sync_directory(path.parent)
    finally:
        Path(temporary).unlink(missing_ok=True)


def read_marker(path):
    """The marker at `path`, None when there is none; refuses a malformed one."""
    if not present(path):
        return None
    try:
        if path.is_symlink():
            raise ValueError("the marker is a symlink")
        record = json.loads(path.read_text())
        if record.get("version") != STATE_VERSION:
            raise ValueError(f"version {record.get('version')!r}, not {STATE_VERSION}")
        for key, kind in (("identity", dict), ("start", dict), ("plan", dict), ("done", dict), ("id", str)):
            if not isinstance(record.get(key), kind):
                raise ValueError(f"no {key}")
    except (OSError, ValueError, AttributeError) as error:
        raise Refusal(f"malformed marker {path}: {error}; nothing changed")
    return record


def canonical(args, *keys):
    paths = {"repo", "dest", "worktree", "store", "tracker", "source", "aside", "archives", "plan"}
    result = {"command": args.command}
    for key in keys:
        value = getattr(args, key, None)
        if key in paths and value:
            value = str(output_location(value) if key in ("dest", "aside") else physical(value))
        result[key] = value
    return result


def head_state(checkout, ref=None):
    """The starting HEAD of `checkout`, or of `ref` when given."""
    if ref is None:
        ref = git("symbolic-ref", "-q", "HEAD", cwd=checkout, check=False).strip() or None
    return {"checkout": str(checkout), "ref": ref, "head": rev(ref or "HEAD", checkout)}


class Operation:
    """The marker of one run of a writing subcommand; see Interruption in the module docstring."""

    def __init__(self, args, checkout, identity):
        self.args = args
        self.path = state_path(checkout)
        self.identity = identity
        self.record = None

    def __enter__(self):
        self.fd = os.open(self.path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            os.close(self.fd)
            raise Refusal(f"another cutover run holds {self.path.parent}") from error
        return self

    def __exit__(self, *exc):
        os.close(self.fd)

    def load(self):
        """The matching marker, or None; refuses a marker of another operation."""
        record = read_marker(self.path)
        if record is None:
            return None
        if record["identity"] != self.identity:
            raise Refusal(f"the marker {self.path} records another operation {record['identity']}, "
                          f"not {self.identity}; nothing changed")
        self.record = record
        require_start(self)
        return record

    def begin(self, start, **plan):
        if present(self.path):
            raise Refusal(f"another marker appeared; changes: {self.path}")
        self.record = {"version": STATE_VERSION, "id": uuid.uuid4().hex, "identity": self.identity,
                       "start": start, "plan": plan, "done": {}, "intent": None}
        write_marker(self.path, self.record)

    @property
    def plan(self):
        return self.record["plan"]

    def done(self, name):
        return name in self.record["done"]

    def commits(self):
        made = [self.plan.get("commit")]
        made += [v.get("commit") for v in self.record["done"].values() if isinstance(v, dict)]
        return [c for c in made if c]

    def finish(self):
        self.path.unlink()
        sync_directory(self.path.parent)

    def run(self, steps):
        """Runs each step not yet completed; a `ref` step is rechecked on every run.

        A step of kind `pure` writes only objects; a failure before any other
        step completed removes the marker, since nothing was published.
        """
        for name, action, kind in steps:
            if self.done(name) and kind != "ref":
                continue
            self.record["intent"] = name
            write_marker(self.path, self.record)
            try:
                result = action()
            except (Refusal, Failure, OSError):
                if kind == "pure" and all(k == "pure" for n, _, k in steps if self.done(n)):
                    self.finish()
                raise
            self.record["done"][name] = True if result is None else result
            self.record["intent"] = None
            write_marker(self.path, self.record)
            step(self.args, name)
        self.finish()


def require_start(op):
    """Refuse a rerun whose HEAD is neither the starting HEAD nor a commit the operation made."""
    start = op.record["start"]
    if not start.get("checkout"):
        return
    checkout = Path(start["checkout"])
    current = rev(start["ref"] or "HEAD", checkout)
    allowed = [start["head"], *op.commits(), *start.get("also", [])]
    if current not in allowed:
        raise Refusal(f"{start['ref'] or 'HEAD'} of {checkout} is at {current}, neither the starting "
                      f"{start['head']} nor a commit this operation made; changes: {start['ref'] or 'HEAD'}")
    lock = Path(git("rev-parse", "--path-format=absolute", "--git-path", "index.lock", cwd=checkout).strip())
    if present(lock):
        raise Refusal(f"another git writer holds the index; changes: {lock}")


# inventory ------------------------------------------------------------------


def merge_result(repo, main_branch, rev_name):
    """The tree `git merge-tree` writes for merging `rev_name` into the main branch, and its conflicted paths."""
    merged = subprocess.run(["git", "merge-tree", "--write-tree", "--no-messages", "--name-only", "-z",
                             main_branch, rev_name], cwd=repo, capture_output=True, text=True)
    if merged.returncode not in (0, 1):
        raise Failure(f"git merge-tree {main_branch} {rev_name}: {merged.stderr.strip()}")
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
    for rev_name, label in heads:
        if tree_of(rev_name, STORE, repo) == main_tree:
            continue
        tree, conflicts = merge_result(repo, args.main, rev_name)
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
            listed = set(worktree_listing(path, "-co", "--exclude-standard", "--", STORE))
            listed -= set(worktree_listing(path, "-d", "--", STORE))
            ordered = sorted(listed)
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
    return batch(args, abandon_branch, ("repo", "branch"))


def abandon_branch(args, branch):
    repo = primary_checkout(args.repo)
    single = argparse.Namespace(**{**vars(args), "branch": branch, "branches": None})
    ref, archived = f"refs/heads/{branch}", f"refs/archive/{branch}"

    def refuse_checked_out():
        holder = next((c for c in checkouts(repo) if c["branch"] == branch), None)
        if holder:
            raise Refusal(f"{branch} is checked out in {holder['path']}; detach that worktree first")

    with Operation(single, repo, canonical(single, "repo", "branch")) as op:
        if op.load() is None:
            if git_rc("check-ref-format", "--branch", branch, cwd=repo) != 0:
                raise Refusal(f"not a branch name: {branch}")
            tip, kept = rev(ref, repo), rev(archived, repo)
            if tip is None:
                if kept:
                    return "already abandoned"
                raise Refusal(f"not a local branch: {branch}")
            if kept:
                raise Refusal(f"{archived} exists at {kept}")
            refuse_checked_out()
            # The branch ref is the subject; deleting it is this operation's last step.
            op.begin({**head_state(repo, ref), "also": [None]})
        tip = op.record["start"]["head"]

        def delete():
            refuse_checked_out()
            compare_and_swap(single, repo, ref, None, tip, "deleted", "cutover.py abandon")

        op.run([("archived", lambda: compare_and_swap(single, repo, archived, tip, None, "archived",
                                                      "cutover.py abandon"), "ref"),
                ("deleted", delete, "ref")])
    return f"abandoned to {archived}"


# archive --------------------------------------------------------------------


def inside(path, root):
    path, root = Path(os.path.realpath(path)), Path(os.path.realpath(root))
    return path == root or root in path.parents


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


def tar_manifest(path):
    entries = {}
    with tarfile.open(path) as tar:
        for member in tar:
            value = {"mode": member.mode}
            if member.isdir():
                value["kind"] = "directory"
            elif member.issym():
                value.update(kind="symlink", sha256=hashlib.sha256(os.fsencode(member.linkname)).hexdigest())
            elif member.isfile() or member.islnk():
                value.update(kind="file", sha256=hashlib.sha256(tar.extractfile(member).read()).hexdigest())
            else:
                raise Failure(f"unsupported archive member: {member.name}")
            entries[member.name] = value
    return entries


def full_store_archive(store, path, *, include_root=True):
    """Writes every entry under `store` to the new tar `path` and verifies it against the store."""
    expected = relative_manifest(store)
    if not include_root:
        expected.pop(".")
    with tarfile.open(path, "x") as tar:
        for rel in sorted(expected, key=lambda p: (len(Path(p).parts), p)):
            tar.add(store / rel, arcname=rel, recursive=False)
    current = relative_manifest(store)
    if not include_root:
        current.pop(".")
    if tar_manifest(path) != expected or current != expected:
        raise Failure(f"full-store archive does not verify: {path}")
    return expected


def archive_faults(dest, name, entry, against_disk):
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
    if against_disk and store.is_dir():
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


def manifest_bytes(stores, done):
    manifest = {name: {"checkout": checkout, "files": done[f"wrote {name}"]["files"]} for name, checkout in stores}
    return (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()


def archive(args):
    dest = output_location(args.dest)
    found = checkouts(args.repo)
    refuse_archive_dest(dest, found, args.repo)
    if not args.verify:
        repo = physical(git("rev-parse", "--show-toplevel", cwd=args.repo).strip())
        with Operation(args, repo, canonical(args, "repo", "dest")) as op:
            if op.load() is None:
                if dest.is_symlink() or (dest.exists() and (not dest.is_dir() or any(dest.iterdir()))):
                    raise Refusal(f"{dest} exists and holds a written archive set or other files; changes: {dest}")
                stores = [[c["name"], str(c["path"])] for c in found if (c["path"] / STORE).is_dir()]
                op.begin(head_state(repo), stores=stores, created=not dest.exists())
            write_archives(args, op, dest)
    manifest = read_manifest(dest)
    faults = []
    for name, entry in sorted(manifest.items()):
        faults += archive_faults(dest, name, entry, against_disk=True)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"{len(faults)} archive faults")
    print(f"verified {len(manifest)} archives in {dest}")


def write_archives(args, op, dest):
    stores = op.plan["stores"]

    def allowed_entries():
        """Each name in DIR with the identity the marker allows; None allows any bytes of an intended partial."""
        allowed = {}
        for name, _ in stores:
            wrote = op.record["done"].get(f"wrote {name}")
            if op.done(f"archived {name}") or (wrote and op.record["intent"] == f"archived {name}"):
                allowed[f"{name}.tar"] = wrote["node"]
            if op.done(f"archived {name}"):
                pass
            elif wrote:
                allowed[f"{name}.tar.partial"] = wrote["node"]
            elif op.record["intent"] == f"wrote {name}":
                allowed[f"{name}.tar.partial"] = None
        if op.record["intent"] == "manifest":
            allowed["manifest.json.partial"] = None
        if op.record["intent"] == "manifest" or op.done("manifest"):
            allowed["manifest.json"] = None
        return allowed

    if op.record["done"] and dest.is_dir():
        allowed = allowed_entries()
        for entry in sorted(dest.iterdir()):
            if entry.name not in allowed or (allowed[entry.name] is not None and node(entry) != allowed[entry.name]):
                raise Refusal(f"{dest} holds content this operation did not record; changes: {entry}")

    def make_destination():
        dest.mkdir(parents=True, exist_ok=True)

    def wrote(name, checkout):
        def action():
            partial = dest / f"{name}.tar.partial"
            if present(partial):
                # Only an interrupted write of this step leaves it: the marker
                # recorded the intent before the exclusive create.
                partial.unlink()
            entries = full_store_archive(Path(checkout) / STORE, partial, include_root=False)
            files = {rel: value["sha256"] for rel, value in entries.items() if value["kind"] == "file"}
            return {"node": node(partial), "files": files}
        return action

    def archived(name):
        def action():
            partial, final = dest / f"{name}.tar.partial", dest / f"{name}.tar"
            recorded = op.record["done"][f"wrote {name}"]["node"]
            if not present(partial) and node(final) == recorded:
                return
            if node(partial) != recorded:
                raise Refusal(f"{partial} differs from what this operation wrote; changes: {partial}")
            move(args, partial, final)
            print(f"archived {Path(dict(stores)[name]) / STORE}: "
                  f"{len(op.record['done'][f'wrote {name}']['files'])} files in {name}.tar")
        return action

    def write_manifest():
        data = manifest_bytes(stores, op.record["done"])
        final, partial = dest / "manifest.json", dest / "manifest.json.partial"
        if present(final) and not present(partial):
            if final.read_bytes() != data:
                raise Refusal(f"{final} differs from the manifest this operation writes; changes: {final}")
            return
        if present(partial):
            partial.unlink()
        with partial.open("xb") as handle:
            handle.write(data)
        move(args, partial, final)

    steps = [("destination", make_destination, "write")]
    for name, checkout in stores:
        steps += [(f"wrote {name}", wrote(name, checkout), "write"), (f"archived {name}", archived(name), "write")]
    steps.append(("manifest", write_manifest, "write"))
    op.run(steps)


# extract --------------------------------------------------------------------


def subtree_split(repo, commit):
    """`git subtree split` in a temporary git directory borrowing the repository's objects.

    The split's cache and commits stay in the temporary directory; the
    commits reach the new repository through the push from it.
    """
    scratch = Path(tempfile.mkdtemp(prefix="cutover-split-"))
    gitdir, work = scratch / "git", scratch / "work"
    git("init", "-q", "--bare", str(gitdir), cwd=scratch)
    (gitdir / "objects" / "info" / "alternates").write_text(str(common_dir(repo) / "objects") + "\n")
    work.mkdir()
    env = {"GIT_DIR": str(gitdir), "GIT_WORK_TREE": str(work)}
    split = git("subtree", "split", f"--prefix={STORE}", commit, cwd=work, env=env).split()[-1]
    return split, scratch


def extract(args):
    repo, dest = physical(args.repo), output_location(args.dest)
    stage = dest.with_name(f".{dest.name}.extract-stage")
    checkout = physical(git("rev-parse", "--show-toplevel", cwd=repo).strip())
    scratch = None
    try:
        with Operation(args, checkout, canonical(args, "repo", "commit", "dest", "split")) as op:
            if op.load() is None:
                if present(dest) or present(stage):
                    raise Refusal(f"the destination or its stage exists; changes: {dest if present(dest) else stage}")
                commit = git("rev-parse", "--verify", f"{args.commit}^{{commit}}", cwd=repo).strip()
                code_tree = tree_of(commit, STORE, repo)
                if code_tree is None:
                    raise Refusal(f"{commit} has no {STORE} tree")
                source = repo
                if args.split:
                    split = git("rev-parse", "--verify", f"{args.split}^{{commit}}", cwd=repo).strip()
                else:
                    split, scratch = subtree_split(repo, commit)
                    source = scratch / "git"
                split_tree = git("rev-parse", f"{split}^{{tree}}", cwd=source, env={"GIT_DIR": str(source)} if scratch else None).strip()
                if split_tree != code_tree:
                    raise Failure(f"split {split} has tree {split_tree}, {commit}:{STORE} is {code_tree}")
                op.begin(head_state(checkout), commit=commit, tree=code_tree, split=split)
            elif not op.done("extraction staged"):
                # The split commits live only in a temporary directory; split again.
                if not args.split:
                    split, scratch = subtree_split(repo, op.plan["commit"])
                    if split != op.plan["split"]:
                        raise Refusal(f"the split of {op.plan['commit']} is {split}, not the recorded "
                                      f"{op.plan['split']}; changes: {repo}")
            plan = op.plan
            if op.done("extraction staged") and not op.done("extracted"):
                moved = op.record["intent"] == "extracted" and not present(stage) and valid_extraction(dest, plan)
                if not moved and (not valid_extraction(stage, plan) or present(dest)):
                    raise Refusal(f"{stage} is not the staged extraction this operation recorded; changes: {stage}")
            elif not op.done("extraction staged") and present(stage) and op.record["intent"] != "extraction staged":
                raise Refusal(f"an unowned extraction stage exists; changes: {stage}")
            source = str(scratch / "git") if scratch else str(repo)

            def staged():
                if present(stage):
                    # The marker recorded this step before the stage was created.
                    if not (stage.is_dir() and (own_repository(stage) or not any(stage.iterdir()))):
                        raise Refusal(f"{stage} is not this operation's stage; changes: {stage}")
                else:
                    stage.mkdir()
                git("init", "-q", "-b", "main", str(stage), cwd=stage)
                git("config", "receive.denyCurrentBranch", "updateInstead", cwd=stage)
                env = {"GIT_DIR": source} if scratch else None
                git("push", "-q", str(stage), f"{plan['split']}:refs/heads/main", cwd=source, env=env)
                git("config", "--unset", "receive.denyCurrentBranch", cwd=stage)
                if not valid_extraction(stage, plan):
                    raise Failure(f"the new repository {stage} differs from {plan['tree']}")

            def extracted():
                if not present(stage) and valid_extraction(dest, plan):
                    return
                move(args, stage, dest, "extracted")

            op.run([("extraction staged", staged, "write"), ("extracted", extracted, "write")])
    finally:
        if scratch:
            shutil.rmtree(scratch)
    count = git("rev-list", "--count", "HEAD", cwd=dest).strip()
    print(f"extracted {dest}: {count} commits, head {plan['split']}, tree {plan['tree']}")


def valid_extraction(path, plan):
    """Whether `path` is the new repository: head `split`, head tree `tree`, nothing else in the worktree."""
    return (own_repository(path) and rev("HEAD", path) == plan["split"]
            and git("rev-parse", "HEAD^{tree}", cwd=path).strip() == plan["tree"]
            and not git("status", "--porcelain", "--ignored", "--untracked-files=all", cwd=path).strip())


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
    lines[end:end] = ["  # The tracker is its own repository, mounted here at .jit/.\n", f"  '{FORBID_STORE}',\n"]
    return "".join(lines)


def set_tree_limit(text, limit):
    result, count = re.subn(r"(?m)^max_tree_bytes = \d+$", f"max_tree_bytes = {limit}", text)
    if count != 1:
        raise Failure(f"{BUDGET} has {count} max_tree_bytes lines")
    return result


def hash_text(repo, text):
    return git("hash-object", "-w", "--stdin", cwd=repo, input=text).strip()


def code_side_plan(repo):
    """The files, tree and limit of the code-side commit, computed in a temporary index."""
    checker = load_budget_checker(repo)
    old_limit = tomllib.loads((repo / BUDGET).read_text())["max_tree_bytes"]
    original = {rel: blob_of("HEAD", rel, repo) for rel in CONFIG_FILES}
    planned = {rel: (repo / rel).read_text() if original[rel] else None for rel in CONFIG_FILES}
    ignore = planned[".gitignore"] or ""
    if "/.jit/" not in ignore.splitlines():
        sep = "" if not ignore or ignore.endswith("\n") else "\n"
        planned[".gitignore"] = f"{ignore}{sep}\n# The tracker repository, mounted here\n/.jit/\n"
    if planned[".gitattributes"] is not None:
        kept = [l for l in planned[".gitattributes"].splitlines(keepends=True)
                if not re.match(rf"\s*{re.escape(EVENTS)}\s.*\bmerge=union\b", l)]
        keep = any(l.strip() and not l.lstrip().startswith("#") for l in kept)
        planned[".gitattributes"] = "".join(kept) if keep else None
    planned[REFERENCES] = remove_exceptions(planned[REFERENCES], {EVENTS, CITATIONS})
    budget = forbid_store(remove_exceptions(planned[BUDGET], {EVENTS}))
    start_tree = git("rev-parse", "HEAD^{tree}", cwd=repo).strip()
    with tempfile.TemporaryDirectory(prefix="cutover-index-") as scratch:
        env = {"GIT_INDEX_FILE": f"{scratch}/index"}
        git("read-tree", start_tree, cwd=repo, env=env)
        git("rm", "-r", "-q", "--cached", "--", STORE, cwd=repo, env=env)
        limit = None
        for _ in range(4):
            planned[BUDGET] = budget if limit is None else set_tree_limit(budget, limit)
            files = {}
            for rel in CONFIG_FILES:
                if planned[rel] is None:
                    git("update-index", "--force-remove", "--", rel, cwd=repo, env=env)
                    files[rel] = None
                else:
                    mode = original[rel][1] if original[rel] else "100644"
                    files[rel] = [hash_text(repo, planned[rel]), mode]
                    git("update-index", "--add", "--cacheinfo", f"{mode},{files[rel][0]},{rel}", cwd=repo, env=env)
            tree = git("write-tree", cwd=repo, env=env).strip()
            size = sum(checker.tree_sizes(tree, str(repo)).values())
            rounded = -(-size // TREE_STEP) * TREE_STEP
            if rounded == limit:
                break
            limit = rounded
        else:
            raise Failure("the tree limit does not settle")
    if limit > old_limit:
        raise Failure(f"the measured tree {size} rounds to {limit}, above the current limit {old_limit}")
    return {"tree": tree, "start_tree": start_tree, "files": files, "original": original,
            "size": size, "limit": limit}


def publish_files(repo, files, original, label):
    """Brings each file to its planned blob; refuses one that is neither its starting nor its planned blob."""
    for rel, planned in files.items():
        path = repo / rel
        current = file_blob(path, repo)
        if current == planned:
            continue
        if current != original[rel]:
            raise Refusal(f"{path} changed since {label} recorded it; changes: {rel}")
        if planned is None:
            path.unlink()
        else:
            data = subprocess.run(["git", "cat-file", "blob", planned[0]], cwd=repo,
                                  capture_output=True, check=True).stdout
            path.parent.mkdir(parents=True, exist_ok=True)
            write_bytes(path, data, 0o755 if planned[1] == "100755" else 0o644)


def stage_index(repo, start_tree, tree, label):
    current = index_tree(repo)
    if current == tree:
        return
    if current != start_tree:
        raise Refusal(f"the index of {repo} changed since {label} recorded it; changes: index")
    git("read-tree", "-m", "-i", start_tree, tree, cwd=repo)


def code_side(args):
    repo = primary_checkout(args.repo)
    with Operation(args, repo, canonical(args, "repo", "message", "main")) as op:
        if op.load() is None:
            require_branch(repo, args.main)
            changes = changed_paths(repo, "--cached", "HEAD") | (changed_paths(repo) & set(CONFIG_FILES))
            if changes:
                raise Refusal(f"the checkout holds changes: {sorted(changes)}")
            if not worktree_listing(repo, "--", STORE):
                raise Refusal(f"the index holds no path under {STORE}")
            op.begin(head_state(repo), **code_side_plan(repo))
        plan, start = op.plan, op.record["start"]
        if op.done("staged") and index_tree(repo) != plan["tree"]:
            raise Refusal(f"the index of {repo} changed since code-side staged it; changes: index")
        for rel, planned in plan["files"].items():
            allowed = [planned] if op.done("edited") else [plan["original"][rel], planned]
            if file_blob(repo / rel, repo) not in allowed:
                raise Refusal(f"{repo / rel} changed since code-side recorded it; changes: {rel}")

        def commit():
            return {"commit": commit_tree(repo, plan["tree"], start["head"], args.message)}

        def publish():
            compare_and_swap(args, repo, start["ref"], op.record["done"]["committed"]["commit"], start["head"],
                             "published", "cutover.py code-side")

        op.run([("staged", lambda: stage_index(repo, plan["start_tree"], plan["tree"], "code-side"), "write"),
                ("edited", lambda: publish_files(repo, plan["files"], plan["original"], "code-side"), "write"),
                ("committed", commit, "write"),
                ("published", publish, "ref")])
    if worktree_listing(repo, "--", STORE):
        raise Failure(f"paths under {STORE} are still tracked")
    head = git("rev-parse", "HEAD", cwd=repo).strip()
    print(f"code-side commit {head}: tree {plan['size']} bytes, max_tree_bytes {plan['limit']}")


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


def valid_clone(path, head):
    return (own_repository(path) and rev("HEAD", path) == head
            and not git("status", "--porcelain", "--ignored", "--untracked-files=all", cwd=path).strip())


def mount(args):
    repo = primary_checkout(args.repo)
    tracker, aside, store = physical(args.tracker), output_location(args.aside), repo / STORE
    stage = repo / f"{STORE}.mount-stage"
    with Operation(args, repo, canonical(args, "repo", "tracker", "aside")) as op:
        if op.load() is None:
            if worktree_listing(repo, "--", STORE):
                raise Refusal(f"the code repository still tracks {STORE}; run code-side first")
            if git_rc("check-ignore", "-q", f"{STORE}/", cwd=repo) != 0:
                raise Refusal(f"the code repository does not ignore {STORE}/")
            if present(aside):
                raise Refusal(f"{aside} exists without a marker of an interrupted mount; changes: {aside}")
            if not store.is_dir() or own_repository(store):
                raise Refusal(f"no old store at {store}")
            if not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent):
                raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}")
            if present(stage):
                raise Refusal(f"an unowned mount stage exists; changes: {stage}")
            op.begin(head_state(repo), store=relative_manifest(store), tracker_head=rev("HEAD", tracker))
        plan = op.plan
        old, head = plan["store"], plan["tracker_head"]
        absent = {"kind": "absent"}
        # Each location holds what the steps completed so far allow.
        if op.done("moved"):
            expect_location(aside, [old], "the old store at")
        elif op.done("cloned"):
            expect_location(aside, [absent, old], "the aside location")
        else:
            expect_location(aside, [absent], "the aside location")
        if op.done("cloned") and not op.done("placed") and present(stage) and not valid_clone(stage, head):
            raise Refusal(f"{stage} is not the clone this operation made; changes: {stage}")
        if not op.done("cloned") and present(stage) and op.record["intent"] != "cloned":
            raise Refusal(f"an unowned mount stage exists; changes: {stage}")

        if op.done("placed") and not op.done("restored"):
            for rel in ignored_files(aside, machine_local_patterns(store)):
                target = store / rel
                if present(target.with_name(f".{target.name}.cutover-stage")):
                    raise Refusal(f"an unowned copy stage exists; changes: {target.with_name(f'.{target.name}.cutover-stage')}")

        def clone():
            if present(stage):
                if not valid_clone(stage, head):
                    raise Refusal(f"{stage} is not a complete clone of {tracker} at {head}; changes: {stage}")
                return
            git("clone", "-q", "--", str(tracker), str(stage), cwd=repo)
            if not valid_clone(stage, head):
                raise Failure(f"the clone {stage} is not {tracker} at {head}")

        def move_store():
            if not present(store) and location(aside) == old:
                return
            expect_location(store, [old], "the old store at")
            move(args, store, aside, "moved")

        def place():
            if not present(stage) and own_repository(store) and rev("HEAD", store) == head:
                return
            if present(store) or not valid_clone(stage, head):
                raise Refusal(f"{store} or {stage} is not where this operation left it; changes: {store}, {stage}")
            move(args, stage, store, "placed")

        def restore_local():
            restored = 0
            files = ignored_files(aside, machine_local_patterns(store))
            for rel in files:
                target = store / rel
                if present(target.with_name(f".{target.name}.cutover-stage")):
                    raise Refusal(f"an unowned copy stage exists; changes: {target.with_name(f'.{target.name}.cutover-stage')}")
            for rel in files:
                target = store / rel
                if present(target):
                    if node(target) != node(aside / rel):
                        raise Failure(f"machine-local {rel} would overwrite {target}")
                    continue
                copy_file(aside / rel, target)
                restored += 1
                if restored == 1:
                    step(args, "restored a machine-local file")
            return {"restored": restored}

        op.run([("cloned", clone, "write"), ("moved", move_store, "write"), ("placed", place, "write"),
                ("restored", restore_local, "write")])
    faults = mount_faults(store, tracker)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"{len(faults)} tracked files differ from the extracted tree")
    restored = len(ignored_files(aside, machine_local_patterns(store)))
    print(f"mounted {tracker} at {store}; {restored} machine-local files restored; old store at {aside}")


# harvest --------------------------------------------------------------------


def run_records(store, issue=None):
    """Relative paths `gate-runs/<issue>/...` of the store's run records, of one issue when given."""
    root = Path(store) / "gate-runs"
    if not root.is_dir():
        return []
    return sorted(p.relative_to(store).as_posix() for p in root.rglob("*")
                  if p.is_file() and p.relative_to(root).parts[0] != "verdicts"
                  and (issue is None or p.relative_to(root).parts[0] == issue))


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


def harvest_plan(shared, source, issue=None):
    """The runs and pointers to copy, the kept pointers and the faults that stop a harvest.

    With `issue`, only that issue's run records and the pointers naming them.
    """
    shared, source = Path(shared), Path(source)
    runs, pointers, kept, faults = [], [], [], []
    for rel in run_records(source, issue):
        if not (shared / rel).exists():
            runs.append(rel)
        elif not same_bytes(shared / rel, source / rel):
            faults.append(f"run record {rel} exists in {shared} with different bytes")
    held = set(run_records(shared)) | set(runs)
    own = set(run_records(source, issue))
    for rel in verdict_pointers(source):
        named = [pointed_run(source, rel)]
        if issue is not None and named[0] not in own:
            continue
        if not (shared / rel).exists():
            pointers.append(rel)
        elif not same_bytes(shared / rel, source / rel):
            kept.append(rel)
            named.append(pointed_run(shared, rel))
        for run in named:
            if run not in held:
                faults.append(f"verdict pointer {rel} names {run}, absent from {shared}")
    return runs, pointers, kept, faults


def copy_steps(source, shared, copies, prefix):
    """One step per recorded copy: the target ends with the source's recorded identity."""
    def action(rel, identity):
        def run():
            target = shared / rel
            if node(target) == identity:
                return
            if node(source / rel) != identity:
                raise Refusal(f"{source / rel} changed since the operation recorded it; changes: {source / rel}")
            if present(target):
                raise Refusal(f"{target} appeared with other bytes; changes: {target}")
            copy_file(source / rel, target, identity)
        return run
    return [(f"{prefix} {rel}", action(rel, identity), "write") for rel, identity in copies.items()]


def harvest(args):
    shared, source = physical(args.store), physical(args.source)
    for path in (shared, source):
        if not path.is_dir():
            raise Refusal(f"no store at {path}")
    if shared == source:
        raise Refusal("the source is the shared store")
    owner = physical(git("rev-parse", "--show-toplevel", cwd=shared).strip())
    with Operation(args, owner, canonical(args, "store", "source")) as op:
        if op.load() is None:
            runs, pointers, kept, faults = harvest_plan(shared, source)
            for fault in faults:
                print(fault)
            if faults:
                raise Failure(f"harvest of {source} stopped; nothing copied")
            op.begin(head_state(owner), copies={rel: node(source / rel) for rel in runs + pointers}, kept=kept)
        copies = op.plan["copies"]
        for rel, identity in copies.items():
            if node(shared / rel) not in ({"kind": "absent"}, identity):
                raise Refusal(f"{shared / rel} holds bytes this operation did not copy; changes: {rel}")
        op.run(copy_steps(source, shared, copies, "harvested"))
    for rel in op.plan["kept"]:
        print(f"kept the store's verdict pointer {rel}")
    left, unset, _, faults = harvest_plan(shared, source)
    if left or unset or faults:
        raise Failure(f"harvest of {source} does not verify: {faults or left + unset}")
    runs = sum(1 for rel in copies if "/verdicts/" not in rel)
    print(f"harvested {source}: {runs} run records and {len(copies) - runs} pointers copied, verified")


# clean ----------------------------------------------------------------------


def tree_without_store(repo, tip):
    """The tree of `tip` without its top-level `.jit` entry."""
    entries = [e for e in git("ls-tree", "-z", tip, cwd=repo).split("\0") if e and e.split("\t", 1)[1] != STORE]
    return git("mktree", "-z", cwd=repo, input="".join(f"{e}\0" for e in entries)).strip()


def read_branches(args):
    if args.branches:
        try:
            listed = [line.partition("#")[0].strip() for line in Path(args.branches).read_text().splitlines()]
        except (OSError, ValueError) as error:
            raise Refusal(f"cannot read branches: {error}")
        return [branch for branch in listed if branch]
    return [args.branch]


def batch(args, single, keys):
    """Runs `single(args, branch)` for each listed branch, resuming a marked one first.

    Prints one line per branch in list order; exit status 1 when any fails.
    """
    branches = read_branches(args)
    repo = primary_checkout(args.repo)
    outcomes = {}
    pending = read_marker(state_path(repo))
    if pending:
        identity = pending["identity"]
        branch = identity.get("branch")
        expected = {**canonical(argparse.Namespace(**{**vars(args), "branch": branch}), *keys)}
        if identity == expected and branch in branches:
            outcomes[branch] = attempt(single, args, branch)
    failed = False
    for branch in branches:
        ok, text = outcomes.pop(branch) if branch in outcomes else attempt(single, args, branch)
        failed |= not ok
        print(f"{branch}: {text}")
    return 1 if failed else 0


def attempt(single, args, branch):
    try:
        return True, single(args, branch)
    except (Refusal, Failure, OSError) as error:
        kind = "refused" if isinstance(error, Refusal) else "failed"
        return False, f"{kind}: {' '.join(str(error).splitlines())}"


def clean(args):
    if args.worktree:
        if not args.store:
            raise Refusal("--worktree requires --store")
        return clean_linked(args)
    if args.store:
        raise Refusal("--store applies only to --worktree")
    return batch(args, clean_parked, ("repo", "branch", "message", "main"))


def local_branch(repo, branch):
    if (git_rc("check-ref-format", "--branch", branch, cwd=repo) != 0
            or rev(f"refs/heads/{branch}^{{commit}}", repo) is None):
        raise Refusal(f"not a local branch: {branch}")




def clean_steps(args, op, repo, ref, before=(), after=()):
    """Runs `before`, then commits, publishes and verifies the removal commit, then `after`.

    Each `after` step is (name, action, kind, undo). A failure once the branch
    moved runs every undo, puts the branch back at the tip, removes the marker
    and says so.
    """
    plan, tip = op.plan, op.record["start"]["head"]

    def commit():
        tree = args.hooks.get("clean_tree", lambda _repo, t: t)(repo, plan["tree"])
        made = commit_tree(repo, tree, tip, args.message)
        if tracked_under(made, STORE, repo):
            raise Failure(f"the clean commit {made} still tracks paths under {STORE}")
        if git_rc("diff", "--quiet", tip, made, "--", ".", f":(exclude){STORE}", cwd=repo) != 0:
            raise Refusal(f"tree outside {STORE} mismatch with the tip {tip}")
        return {"commit": made}

    def publish():
        compare_and_swap(args, repo, ref, op.record["done"]["clean committed"]["commit"], tip,
                         "clean published", "cutover.py clean")

    def verify():
        for name in (args.main, ref):
            if tracked_under(name, STORE, repo):
                raise Failure(f"{name} still tracks paths under {STORE}")
        if git_rc("diff", "--quiet", tip, ref, "--", ".", f":(exclude){STORE}", cwd=repo) != 0:
            raise Failure(f"tree outside {STORE} of {ref} differs from the tip {tip}")

    steps = [*before, ("clean committed", commit, "pure"), ("clean published", publish, "ref"),
             ("verified", verify, "write"), *[(name, action, kind) for name, action, kind, _ in after]]
    try:
        op.run(steps)
    except (Refusal, Failure, OSError) as error:
        made = op.record["done"].get("clean committed", {}).get("commit")
        if not present(op.path) or not made or rev(ref, repo) != made:
            raise
        kind = type(error) if isinstance(error, (Refusal, Failure)) else Failure
        try:
            for _, _, _, undo in reversed(after):
                if undo:
                    undo()
            git("update-ref", "-m", "cutover.py clean: restore", ref, tip, made, cwd=repo)
        except (Failure, OSError) as restore:
            raise Failure(f"{error}; {ref} is left at {rev(ref, repo)}, not restored to {tip}: {restore}")
        op.finish()
        raise kind(f"{error}; {ref.removeprefix('refs/heads/')} restored to {tip}") from error


def clean_parked(args, branch):
    repo = primary_checkout(args.repo)
    single = argparse.Namespace(**{**vars(args), "branch": branch, "branches": None})
    ref = f"refs/heads/{branch}"
    with Operation(single, repo, canonical(single, "repo", "branch", "message", "main")) as op:
        if op.load() is None:
            local_branch(repo, branch)
            if any(c["branch"] == branch for c in checkouts(repo)):
                raise Refusal(f"{branch} is checked out; use --worktree and --store for a linked checkout")
            if not args.message.strip():
                raise Refusal("the commit message is empty")
            if tracked_under(args.main, STORE, repo):
                raise Failure(f"{args.main} still tracks paths under {STORE}")
            tip = rev(ref, repo)
            if not tracked_under(tip, STORE, repo):
                return "already clean"
            op.begin(head_state(repo, ref), tree=tree_without_store(repo, tip))
        clean_steps(single, op, repo, ref)
    return CLEANED


def retain_local_store(args, op, worktree, store):
    """Retains a verified full-store tar of `store` as a blob under refs/cutover-archives/."""
    ref = f"refs/cutover-archives/{op.record['id']}"
    with tempfile.TemporaryDirectory(prefix="cutover-retain-") as scratch:
        path = Path(scratch) / "local-store.tar"
        manifest = full_store_archive(store, path)
        blob = git("hash-object", "-w", "--", str(path), cwd=worktree).strip()
        current = rev(ref, worktree)
        if current and current != blob:
            # The ref is this operation's, from an interrupted earlier attempt.
            older = Path(scratch) / "older.tar"
            older.write_bytes(subprocess.run(["git", "cat-file", "blob", current], cwd=worktree,
                                             capture_output=True, check=True).stdout)
            if tar_manifest(older) != manifest:
                raise Refusal(f"{ref} holds an archive of other content; changes: {store}")
            blob = current
    compare_and_swap(args, worktree, ref, blob, None, "archived local store", "cutover.py clean: retain")
    return {"blob": blob, "ref": ref, "manifest": manifest}


def clean_linked(args):
    found = checkouts(args.repo)
    worktree = physical(args.worktree)
    checkout = next((c for c in found if c["path"] == worktree), None)
    if checkout is None or checkout["primary"]:
        raise Refusal(f"{worktree} is not a linked worktree of {args.repo}")
    branch = checkout["branch"]
    if branch is None:
        raise Refusal(f"{worktree} has a detached HEAD")
    ref = f"refs/heads/{branch}"
    store, shared = worktree / STORE, physical(args.store)

    def unverified():
        return store.is_dir() and any(harvest_plan(shared, store)[i] for i in (0, 1, 3))

    with Operation(args, worktree, canonical(args, "repo", "worktree", "store", "message", "main")) as op:
        if op.load() is None:
            if tracked_under(args.main, STORE, worktree):
                raise Failure(f"{args.main} still tracks paths under {STORE}")
            if not args.message.strip():
                raise Refusal("the commit message is empty")
            if unverified():
                raise Refusal(f"the harvest of {store} into {shared} is not verified")
            if git("status", "--porcelain", "--untracked-files=no", cwd=worktree).strip():
                raise Refusal(f"{worktree} has uncommitted tracked changes")
            tip = rev("HEAD", worktree)
            tree = tree_without_store(worktree, tip) if tracked_under(tip, STORE, worktree) else None
            op.begin(head_state(worktree), tree=tree, tip_tree=rev(f"{tip}^{{tree}}", worktree),
                     store=store.is_dir())
        plan, tip = op.plan, op.record["start"]["head"]
        retained = op.record["done"].get("archived local store")
        if isinstance(retained, dict) and not op.done("store-removal"):
            expect_location(store, [retained["manifest"]], "the local store")
        allowed = [plan["tree"]] if op.done("index") else [plan["tip_tree"], plan["tree"]]
        if index_tree(worktree) not in allowed:
            raise Refusal(f"the index of {worktree} changed since the clean recorded it; changes: index")

        def retain():
            if plan["store"]:
                return retain_local_store(args, op, worktree, store)

        def index():
            if index_tree(worktree) != plan["tree"]:
                git("rm", "-r", "-q", "--cached", "--ignore-unmatch", "--", STORE, cwd=worktree)

        def unindex():
            if index_tree(worktree) != plan["tip_tree"]:
                git("reset", "-q", tip, "--", STORE, cwd=worktree)

        def remove_store():
            step(args, "store-removal")
            saved = op.record["done"].get("archived local store")
            if not isinstance(saved, dict) or not present(store):
                return
            if unverified():
                raise Refusal(f"the harvest of {store} into {shared} is not verified")
            expect_location(store, [saved["manifest"]], "the local store")
            try:
                shutil.rmtree(store)
            except OSError as error:
                raise Failure(f"deleting the local store {store} failed and may have removed part of it; "
                              f"its full archive is {saved['ref']}: {error}")
            print(f"deleted the local store {store}")

        retain_step = ("archived local store", retain, "pure")
        if plan["tree"]:
            clean_steps(args, op, worktree, ref, before=[retain_step],
                        after=[("index", index, "write", unindex), ("store-removal", remove_store, "write", None)])
            outcome = CLEANED
        else:
            op.run([retain_step, ("store-removal", remove_store, "write")])
            outcome = "already clean"
    print(f"{branch}: {outcome}")


# rollback -------------------------------------------------------------------


def rollback_plan(args, repo, store, cutover, manifest):
    """The reverted tree, the commit carrying the store's files and the restores, made from objects alone."""
    restores = []
    for name, entry in sorted((manifest or {}).items()):
        checkout = Path(entry["checkout"])
        if name == "primary" or not checkout.is_dir() or present(checkout / STORE):
            continue
        stage = checkout / f"{STORE}.rollback-stage"
        if present(stage):
            raise Refusal(f"an unowned rollback stage exists; changes: {stage}")
        archive_path = physical(args.archives) / f"{name}.tar"
        restores.append({"name": name, "checkout": str(checkout), "archive": str(archive_path),
                         "members": tar_manifest(archive_path), "files": entry["files"]})
    head = rev("HEAD", repo)
    merged = subprocess.run(["git", "merge-tree", "--write-tree", "--no-messages", f"--merge-base={cutover}",
                             head, f"{cutover}^"], cwd=repo, capture_output=True, text=True)
    if merged.returncode != 0:
        raise Failure(f"reverting {cutover} conflicts: {merged.stdout.strip()} {merged.stderr.strip()}; nothing changed")
    reverted = merged.stdout.split("\n", 1)[0].strip()
    deleted = set(worktree_listing(store, "-d"))
    current = sorted(p for p in set(worktree_listing(store, "-co", "--exclude-standard")) - deleted
                     if (store / p).is_file() and not (store / p).is_symlink())
    local = sorted(p for p in worktree_listing(store, "-o", "-i", "--exclude-standard")
                   if not p.startswith(".git/") and (store / p).is_file() and not (store / p).is_symlink())
    files = {rel: node(store / rel) for rel in sorted(set(current) | set(local))}
    with tempfile.TemporaryDirectory(prefix="cutover-index-") as scratch:
        env = {"GIT_INDEX_FILE": f"{scratch}/index"}
        blobs = git("hash-object", "-w", "--no-filters", "--stdin-paths", cwd=repo,
                    input="".join(f"{store / rel}\n" for rel in current)).split() if current else []
        entries = "".join(f"{'100755' if files[rel]['mode'] & stat.S_IXUSR else '100644'} {blob}\t{rel}\n"
                          for rel, blob in zip(current, blobs))
        git("update-index", "--index-info", cwd=repo, env=env, input=entries)
        store_tree = git("write-tree", cwd=repo, env=env).strip()
    top = [e for e in git("ls-tree", "-z", reverted, cwd=repo).split("\0") if e and e.split("\t", 1)[1] != STORE]
    if current:
        top.append(f"040000 tree {store_tree}\t{STORE}")
    tree = git("mktree", "-z", cwd=repo, input="".join(f"{e}\0" for e in top)).strip()
    restored = tracked_under(reverted, STORE, repo)
    gone = sorted(set(p.removeprefix(f"{STORE}/") for p in restored) - set(current))
    outside = {}
    for rel in filter(None, git("diff", "-z", "--name-only", head, reverted, "--", ".",
                                f":(exclude){STORE}", cwd=repo).split("\0")):
        outside[rel] = {"original": blob_of(head, rel, repo), "planned": blob_of(reverted, rel, repo)}
    return {"cutover": cutover, "start_tree": rev("HEAD^{tree}", repo), "tree": tree,
            "commit": commit_tree(repo, tree, head, args.message), "files": files, "current": len(current),
            "gone": len(gone), "outside": outside, "store_head": rev("HEAD", store), "restores": restores}


def rollback(args):
    repo = primary_checkout(args.repo)
    store, aside = repo / STORE, output_location(args.aside)
    with Operation(args, repo, canonical(args, "repo", "cutover", "aside", "message", "archives", "main")) as op:
        if op.load() is None:
            require_branch(repo, args.main)
            cutover = git("rev-parse", "--verify", f"{args.cutover}^{{commit}}", cwd=repo).strip()
            if tracked_under("HEAD", STORE, repo):
                raise Refusal(f"HEAD already tracks {STORE}; no marked rollback to resume")
            if (git_rc("merge-base", "--is-ancestor", cutover, "HEAD", cwd=repo) != 0
                    or tracked_under(cutover, STORE, repo) or not tracked_under(f"{cutover}^", STORE, repo)):
                raise Refusal(f"{cutover} is not a code-side commit on {args.main}")
            manifest = None
            if args.archives:
                archives = physical(args.archives)
                manifest = read_manifest(archives)
                faults = [f for name, entry in sorted(manifest.items())
                          for f in archive_faults(archives, name, entry, against_disk=False)]
                if faults:
                    raise Refusal(f"archives do not verify: {faults}")
            if present(aside):
                raise Refusal(f"{aside} exists without a marker of an interrupted rollback; changes: {aside}")
            if not own_repository(store):
                raise Refusal(f"no mounted repository at {store} to roll back")
            if git("status", "--porcelain", "--untracked-files=no", cwd=repo).strip():
                raise Refusal(f"{repo} has uncommitted tracked changes")
            if not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent):
                raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}")
            op.begin(head_state(repo), **rollback_plan(args, repo, store, cutover, manifest))
        plan = op.plan
        files = plan["files"]
        if not op.done("moved"):
            if present(aside) and not (op.record["intent"] == "moved" and not present(store)):
                raise Refusal(f"{aside} exists; changes: {aside}")
        else:
            if not own_repository(aside) or rev("HEAD", aside) != plan["store_head"]:
                raise Refusal(f"{aside} is not the moved tracker repository; changes: {aside}")
            if present(store):
                listed = {p for p, v in relative_manifest(store).items() if v["kind"] != "directory"}
                if listed - set(files):
                    raise Refusal(f"{store} holds files this rollback does not restore; "
                                  f"changes: {sorted(listed - set(files))[:20]}")
        if op.done("copied"):
            changed = [rel for rel, identity in files.items() if node(store / rel) != identity]
            if changed:
                raise Refusal(f"{store} changed since the rollback copied it; changes: {changed[:20]}")
        if op.done("staged") and index_tree(repo) != plan["tree"]:
            raise Refusal(f"the index of {repo} changed since the rollback staged it; changes: index")

        def move_store():
            if not present(store) and own_repository(aside) and rev("HEAD", aside) == plan["store_head"]:
                return
            if not own_repository(store) or rev("HEAD", store) != plan["store_head"]:
                raise Refusal(f"{store} is not the recorded tracker repository; changes: {store}")
            move(args, store, aside, "moved")

        def revert_outside():
            publish_files(repo, {rel: v["planned"] for rel, v in plan["outside"].items()},
                          {rel: v["original"] for rel, v in plan["outside"].items()}, "the rollback")

        def copy_store():
            for number, (rel, identity) in enumerate(files.items()):
                target = store / rel
                if node(target) == identity:
                    continue
                if present(target):
                    raise Refusal(f"{target} holds bytes this rollback did not copy; changes: {target}")
                copy_file(aside / rel, target, identity)
                if number == 0:
                    step(args, "copied a file")

        def publish():
            compare_and_swap(args, repo, op.record["start"]["ref"], plan["commit"], op.record["start"]["head"],
                             "committed", "cutover.py rollback")

        steps = [("moved", move_store, "write"), ("reverted", revert_outside, "write"),
                 ("copied", copy_store, "write"),
                 ("staged", lambda: stage_index(repo, plan["start_tree"], plan["tree"], "the rollback"), "write"),
                 ("committed", publish, "ref")]
        for restore in plan["restores"]:
            steps += restore_steps(args, op, restore)
        op.run(steps)
    print(f"reverted {plan['cutover']}; {plan['current']} tracker files restored, {plan['gone']} deleted; "
          f"tracker repository at {aside}")
    for restore in plan["restores"]:
        print(f"restored {Path(restore['checkout']) / STORE} from {restore['name']}.tar")


def restore_steps(args, op, restore):
    """Extract one worktree store's archive into its stage, check it, rename it into place."""
    name, checkout = restore["name"], Path(restore["checkout"])
    stage, target, archive_path = checkout / f"{STORE}.rollback-stage", checkout / STORE, Path(restore["archive"])

    def complete(path):
        actual = relative_manifest(path)
        actual.pop(".")
        return actual == restore["members"] and store_files(path) == restore["files"]

    def extracted():
        if present(stage):
            if complete(stage):
                return
            raise Refusal(f"{stage} is not a complete extraction of {name}.tar; changes: {stage}")
        if present(target):
            if op.record["intent"] == f"extracted {name}" or not complete(target):
                raise Refusal(f"{target} appeared; changes: {target}")
            return
        if tar_manifest(archive_path) != restore["members"]:
            raise Refusal(f"{archive_path} changed since the rollback recorded it; changes: {archive_path}")
        stage.mkdir()
        with tarfile.open(archive_path) as tar:
            tar.extractall(stage, filter="tar")
        for rel, value in sorted(restore["members"].items(), key=lambda item: -len(Path(item[0]).parts)):
            if value["kind"] != "symlink":
                (stage / rel).chmod(value["mode"])
        if not complete(stage):
            raise Failure(f"{stage} differs from the manifest of {name}.tar")

    def restored():
        if not present(stage) and present(target) and complete(target):
            return
        if not complete(stage):
            raise Refusal(f"{stage} is not the extraction this rollback checked; changes: {stage}")
        move(args, stage, target)

    return [(f"extracted {name}", extracted, "write"), (f"restored {name}", restored, "write")]


COMMANDS = {
    "inventory": inventory, "abandon": abandon, "archive": archive, "extract": extract, "code-side": code_side,
    "mount": mount, "harvest": harvest, "clean": clean, "rollback": rollback,
}


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def load_budget_checker(repo):
    spec = importlib.util.spec_from_file_location("check_tracked_budget", Path(repo) / BUDGET_CHECKER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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
WRITING = ("abandon", "port", "archive", "extract", "code-side", "mount", "harvest", "clean", "rollback")
RUN_A = b'{"run": "r1", "status": "passed"}\n'
RUN_B = b'{"run": "r1", "status": "failed"}\n'
I1, I2, I3, I9 = (f"{n}{n}{n}{n}0000-0000-4000-8000-00000000000{n}" for n in (1, 2, 3, 9))


class Interrupted(Exception):
    """Raised by a self-test hook to stop a subcommand at one of its steps."""


def interrupt_at(target):
    def hook(name):
        if name == target:
            raise Interrupted(name)
    return hook


def fail_at(target):
    def hook(name):
        if name == target:
            raise OSError(f"planted failure at {name}")
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


class SelfTest:
    def __init__(self, base, invoking):
        self.base = base
        self.invoking = invoking
        self.failures = []
        self.passed = 0
        self.jit = shutil.which("jit")

    def check(self, condition, label):
        if condition:
            self.passed += 1
        else:
            self.failures.append(label)
            print(f"  FAIL {label}")
        return condition

    def run(self, *argv, hooks=None):
        for arg in argv:
            if arg.startswith("/") and inside(Path(arg), self.invoking):
                raise Refusal(f"self-test argument inside the invoking checkout: {arg}")
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            try:
                code = main(list(argv), hooks=dict(hooks or {}))
            except Interrupted:
                code = "interrupted"
        if code == 0 and argv[0] in WRITING and "--verify" not in argv and "--dry-run" not in argv:
            self.check(not self.marker_for(argv).exists(), f"SAFE marker-removed {argv[0]}")
        return code, out.getvalue()

    def preservation_state(self, code):
        return (store_files(Path(code)),
                (Path(git("rev-parse", "--absolute-git-dir", cwd=code).strip()) / "index").read_bytes(),
                git("show-ref", cwd=code))

    def observe(self):
        """The identity of every path of the fixture and every ref, the snapshot excluded."""
        seen = {}
        for entry in sorted(self.base.iterdir()):
            if entry.name != ".snapshot":
                seen.update({f"{entry.name}/{k}": v for k, v in relative_manifest(entry).items()})
        return seen

    def snapshot(self):
        keep = self.base / ".snapshot"
        if keep.exists():
            shutil.rmtree(keep)
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

    def marker_for(self, argv):
        if argv[0] == "harvest":
            repo = git("rev-parse", "--show-toplevel", cwd=argv[argv.index("--store") + 1]).strip()
        elif argv[0] == "clean" and "--worktree" in argv:
            repo = argv[argv.index("--worktree") + 1]
        else:
            repo = argv[argv.index("--repo") + 1]
        return state_path(repo)

    def record(self, argv):
        return json.loads(self.marker_for(argv).read_text())

    def interrupt_each(self, argv, steps, state, label):
        """Interrupts the command at each step and requires a rerun to reach `state`."""
        self.check(steps != [], f"{label}: the command reports its steps")
        for number, name in enumerate(steps):
            self.restore()
            seen = -1

            def stop_occurrence(reached):
                nonlocal seen
                seen += 1
                if seen == number:
                    raise Interrupted(reached)

            rc, out = self.run(*argv, hooks={"step": stop_occurrence})
            self.check(rc == "interrupted", f"{label}: interrupted at {name}")
            marker = self.marker_for(argv)
            self.check(marker.is_file(), f"SAFE marked-interruption {label} {name}")
            rc, out = self.run(*argv)
            self.check(rc == 0 and not marker.exists() and self.state_of(argv) == state,
                       f"{label}: a rerun after an interruption at {name} reaches the uninterrupted end state")
        rc, out = self.run(*argv)
        self.check(rc in (0, 2) and self.state_of(argv) == state,
                   f"{label}: a rerun of the finished command changes nothing")

    def guard_probes(self, argv, at, label, owned):
        """Probes a marked interruption at `at` with a foreign HEAD, planted content and a bad marker.

        `owned(record)` names a (directory, file) the operation validates at
        that step, or None when it validates none; planted content there must
        refuse with nothing changed, elsewhere it must survive a completed rerun.
        """
        for probe in ("stale-head", "untracked-drift", "ignored-drift", "mode-drift", "malformed-marker",
                      "arguments", "missing-marker"):
            self.restore()
            rc, _ = self.run(*argv, hooks={"step": interrupt_at(at)})
            marker = self.marker_for(argv)
            self.check(rc == "interrupted" and marker.is_file(), f"SAFE {probe} setup {label}")
            if not marker.is_file():
                continue
            record = json.loads(marker.read_text())
            target = owned(record)
            planted = None
            if probe == "stale-head":
                start = record["start"]
                repo = start["checkout"]
                ref = start["ref"] or "HEAD"
                parent = rev(ref, repo) or start["head"]
                tree = git("rev-parse", f"{parent}^{{tree}}", cwd=repo).strip()
                foreign = self.sh("git", "commit-tree", tree, "-p", parent, "-m", "foreign head", cwd=repo).stdout.strip()
                self.sh("git", "update-ref", ref, foreign, cwd=repo)
            elif probe == "mode-drift":
                if target:
                    planted = target[1]
                    planted.chmod(stat.S_IMODE(planted.stat().st_mode) ^ stat.S_IXUSR)
                else:
                    planted = Path(record["start"]["checkout"]) / "mode-drift"
                    planted.write_text("unowned mode\n")
                    planted.chmod(0o755)
            elif probe in ("untracked-drift", "ignored-drift"):
                root = target[0] if target and target[0] else Path(record["start"]["checkout"])
                planted = root / ("gate-runs/safety-extra.json" if probe == "ignored-drift" else "safety-extra")
                planted.parent.mkdir(parents=True, exist_ok=True)
                planted.write_text("unowned content\n")
            elif probe == "malformed-marker":
                damaged = dict(record, done={**record["done"], "unknown step": True}, version=1)
                marker.write_text(json.dumps(damaged))
            elif probe == "missing-marker":
                marker.unlink()
            before = self.observe()
            invocation = list(argv)
            if probe == "arguments":
                key = next(k for k in ("--message", "--dest", "--aside", "--from", "--plan", "--branch") if k in invocation)
                invocation[invocation.index(key) + 1] += "-other"
            rc, out = self.run(*invocation)
            unowned = not target or (probe != "mode-drift" and not target[0])
            if probe in ("untracked-drift", "ignored-drift", "mode-drift") and unowned:
                self.check(rc == 0 and planted.read_text() == ("unowned mode\n" if probe == "mode-drift" else "unowned content\n"),
                           f"SAFE {probe} {label}: content the operation does not own survives the resumed run")
            elif probe == "missing-marker" and argv[0] in ("harvest", "clean"):
                # Without a marker these run fresh: harvest copies only absent
                # targets and clean starts from the branch as it stands.
                self.check(rc in (0, 1, 2), f"SAFE {probe} {label}: a fresh run")
            else:
                self.check(rc in (1, 2) and self.observe() == before,
                           f"SAFE {probe} {label}: refuses without changing bytes, modes, index or refs")
        self.restore()

    def unowned_location(self, argv, path, label):
        self.restore()
        path = Path(path)
        path.mkdir(parents=True)
        (path / "gate-runs").mkdir()
        (path / "gate-runs" / "user.json").write_text("ignored user bytes\n")
        (path / "user").write_text("untracked user bytes\n")
        before = self.observe()
        rc, out = self.run(*argv)
        self.check(rc == 2 and self.observe() == before,
                   f"SAFE unowned-location {label}: refuses and preserves ignored/untracked bytes")
        self.restore()
        absent = path.with_name(path.name + "-absent-target")
        path.symlink_to(absent)
        before = self.observe()
        rc, out = self.run(*argv)
        self.check(rc == 2 and self.observe() == before,
                   f"SAFE dangling-output {label}: link is present, not inferred absent")
        self.restore()

    def state_of(self, argv):
        code = Path(argv[argv.index("--repo") + 1]) if "--repo" in argv else Path(self.path("code"))
        aside = Path(argv[argv.index("--aside") + 1]) if "--aside" in argv else None

        def files(root):
            if root is None or not root.is_dir():
                return None
            return {k: v for k, v in store_files(root).items() if not k.startswith(".git/")}

        def head(root):
            return git("rev-parse", "HEAD", cwd=root, check=False).strip() if root and (root / ".git").exists() else None

        state = {"code": git("rev-parse", "HEAD^{tree}", "HEAD^", cwd=code),
                 "status": git("status", "--porcelain", "--ignored", cwd=code),
                 "worktrees": [l for l in git("worktree", "list", "--porcelain", cwd=code).splitlines()
                               if not l.startswith("HEAD ")],
                 "store": files(code / STORE), "store head": head(code / STORE),
                 "aside": files(aside), "aside head": head(aside),
                 "listing": sorted(os.listdir(code)), "revert": (code / ".git" / "REVERT_HEAD").exists(),
                 "refs": sorted(l.split()[1] for l in git("show-ref", cwd=code).splitlines()
                                if "refs/cutover-archives/" not in l)}
        for name in ("wa", "wb"):
            if os.path.isdir(self.path(name)):
                state[name] = files(Path(self.path(name)) / STORE)
                state[f"{name} listing"] = sorted(os.listdir(self.path(name)))
                state[f"{name} status"] = git("status", "--porcelain", cwd=self.path(name))
        if "--dest" in argv:
            dest = Path(argv[argv.index("--dest") + 1])
            state["destination"] = files(dest)
            state["destination head"] = head(dest)
        if "--archives" in argv:
            manifest = read_manifest(Path(argv[argv.index("--archives") + 1]))
            state["archive stores"] = {name: files(Path(entry["checkout"]) / STORE)
                                       for name, entry in manifest.items()}
        state["branch trees"] = {ref: git("rev-parse", f"{ref}^{{tree}}", cwd=code).strip()
                                 for ref in git("for-each-ref", "--format=%(refname)", "refs/heads", cwd=code).splitlines()}
        return state

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
        self.sh("git", "branch", "-D", "outside-conflict", "tracker-conflict", cwd=code)

    def abandon(self, code):
        for name in ("landed-a", "landed-b", "landed-c", "landed-d", "landed-e"):
            self.sh("git", "branch", name, "main", cwd=code)
        self.sh("git", "update-ref", "refs/archive/landed-b", "main~1", cwd=code)
        tip = git("rev-parse", "landed-a", cwd=code).strip()
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-a")
        self.check(rc == 0 and out.splitlines() == ["landed-a: abandoned to refs/archive/landed-a"]
                   and rev("refs/archive/landed-a", code) == tip and rev("refs/heads/landed-a", code) is None,
                   "REQ-10 abandon moves the branch to refs/archive/<name> and deletes the branch ref")
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-a")
        self.check(rc == 0 and out.splitlines() == ["landed-a: already abandoned"]
                   and rev("refs/archive/landed-a", code) == tip,
                   "REQ-10 a rerun after success reports already abandoned and changes nothing")
        before = git("show-ref", cwd=code)
        rc, out = self.run("abandon", "--repo", code, "--branch", "landed-b")
        self.check(rc == 1 and out.startswith("landed-b: refused: refs/archive/landed-b exists")
                   and git("show-ref", cwd=code) == before,
                   "REQ-10 abandon refuses an existing archive ref and changes no ref")
        rc, out = self.run("abandon", "--repo", code, "--branch", "wa")
        self.check(rc == 1 and out.startswith("wa: refused: wa is checked out in")
                   and git("show-ref", cwd=code) == before,
                   "REQ-10 abandon refuses a branch checked out in a worktree and changes no ref")
        listed = Path(self.path("abandon.txt"))
        listed.write_text("# landed branches\nlanded-c\nwa # checked out\nlanded-d\n")
        rc, out = self.run("abandon", "--repo", code, "--branches", str(listed))
        lines = out.splitlines()
        self.check(rc == 1 and len(lines) == 3 and lines[0] == "landed-c: abandoned to refs/archive/landed-c"
                   and lines[1].startswith("wa: refused:") and lines[2] == "landed-d: abandoned to refs/archive/landed-d",
                   "REQ-10 a batch prints one line per branch, continues after a refusal and exits 1")
        argv = ("abandon", "--repo", code, "--branch", "landed-e")
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        self.check(rc == 0, "REQ-10 abandon of landed-e")
        self.interrupt_each(argv, steps, self.state_of(argv), "REQ-10 abandon")
        self.guard_probes(argv, "archived", "abandon", lambda r: None)
        rc, _ = self.run(*argv)
        self.check(rc == 0 and rev("refs/heads/landed-e", code) is None, "SAFE restart abandon final fixture")
        self.sh("git", "update-ref", "-d", "refs/archive/landed-b", cwd=code)
        self.sh("git", "branch", "-D", "landed-b", cwd=code)

    def archive(self, code):
        inside_dest = str(Path(code) / "docs" / "archives")
        rc, out = self.run("archive", "--repo", code, "--dest", inside_dest)
        self.check(rc == 2 and not os.path.exists(inside_dest),
                   "REQ-02 archive refuses a destination in the tracked tree")
        extra = Path(self.path("wa")) / STORE
        (extra / "empty-local-directory").mkdir()
        (extra / "local-link").symlink_to("server.log")
        mode_file = extra / "local-mode"
        mode_file.write_text("executable local file\n")
        mode_file.chmod(0o771)
        dest = self.path("archives")
        argv = ("archive", "--repo", code, "--dest", dest)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
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
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart archive")
        self.guard_probes(argv, "wrote primary", "archive",
                          lambda r: (Path(dest), Path(dest) / "primary.tar.partial"))
        self.unowned_location(argv, dest, "archive")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart archive final fixture")
        partial = self.path("archives-partial")
        rc, _ = self.run("archive", "--repo", code, "--dest", partial, hooks={"step": interrupt_at("archived primary")})
        self.check(rc == "interrupted" and "manifest.json" not in os.listdir(partial),
                   "REQ-02 an interrupted archive writes no manifest")
        rc, _ = self.run("archive", "--repo", code, "--dest", partial)
        self.check(rc == 0 and sorted(os.listdir(partial)) == names, "REQ-02 a rerun completes an interrupted archive")
        return dest

    def extract(self, code):
        older = git("rev-parse", "main~1", cwd=code).strip()
        stale = self.sh("git", "subtree", "split", f"--prefix={STORE}", older, cwd=code).stdout.strip().splitlines()[-1]
        new = self.path("tracker")
        cache = Path(git("rev-parse", "--absolute-git-dir", cwd=code).strip()) / "subtree-cache"
        before = set(os.listdir(cache)) if cache.is_dir() else set()
        rc, out = self.run("extract", "--repo", code, "--commit", "main", "--dest", new, "--split", stale)
        self.check(rc == 1 and not os.path.exists(new),
                   "REQ-03 extract fails on a planted tree mismatch and creates nothing")
        argv = ("extract", "--repo", code, "--commit", "main", "--dest", new)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
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
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart extract")
        stage = Path(new).with_name(f".{Path(new).name}.extract-stage")
        self.guard_probes(argv, "extraction staged", "extract", lambda r: (stage, stage / "config.toml"))
        self.unowned_location(argv, stage, "extract stage")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart extract final fixture")
        return new

    def code_side(self, code):
        argv = ("code-side", "--repo", code, "--message", "chore: move the tracker out")
        self.snapshot()
        for rel in CONFIG_FILES:
            self.restore()
            target = Path(code) / rel
            original = target.read_bytes()
            target.write_bytes(original + b"\n# ordinary user edit\n")
            before_edit = self.preservation_state(code)
            rc, out = self.run(*argv)
            self.check(rc == 2 and self.preservation_state(code) == before_edit,
                       f"SAFE config-edit {rel}: refusal preserves bytes, index and refs")
        self.restore()
        before = git("rev-parse", "HEAD", cwd=code).strip()
        rc, _ = self.run("code-side", "--repo", self.path("wa"), "--message", "chore: cut over")
        self.check(rc == 2, "REQ-04 code-side refuses a linked worktree")
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
        self.check(all(p in CONFIG_FILES or p.startswith(".jit/") for p in changed),
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
        self.check(git("status", "--porcelain", cwd=code) == "", "REQ-04 the checkout is clean after the commit")
        state = self.state_of(argv)
        self.interrupt_each(argv, steps, state, "REQ-04 code-side")
        self.restore()
        rc, _ = self.run(*argv, hooks={"step": interrupt_at("published")})
        record = self.record(argv)
        made = record["done"]["committed"]["commit"]
        self.check(rc == "interrupted" and made == git("rev-parse", "HEAD", cwd=code).strip()
                   and record["start"]["head"] == before,
                   "SAFE completed-ref: the made commit is recorded, the starting HEAD immutable")
        self.sh("git", "update-ref", "refs/heads/main", before, made, cwd=code)
        rc, _ = self.run(*argv)
        self.check(rc == 0 and self.state_of(argv) == state,
                   "SAFE starting-head-resume: the recorded commit is republished by compare-and-swap")
        self.restore()
        rc, _ = self.run(*argv, hooks={"step": interrupt_at("staged")})
        lock = Path(git("rev-parse", "--absolute-git-dir", cwd=code).strip()) / "index.lock"
        lock.write_text("foreign index bytes\n")
        before_lock = self.observe()
        rc, _ = self.run(*argv)
        self.check(rc == 2 and self.observe() == before_lock,
                   "SAFE foreign-index-lock: a resume preserves another writer's lock and index")
        self.guard_probes(argv, "edited", "code-side", lambda r: (None, Path(code) / ".gitignore"))
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart code-side final fixture")
        return git("rev-parse", "HEAD", cwd=code).strip()

    def mount(self, code, new):
        global same_filesystem
        aside = self.path("primary-store-aside")
        rc, out = self.run("mount", "--repo", self.path("wa"), "--tracker", new, "--aside", aside)
        self.check(rc == 2 and not os.path.exists(aside), "REQ-05 mount refuses a linked worktree")
        self.snapshot()
        original_filesystem = same_filesystem
        try:
            same_filesystem = lambda a, b: False
            before = self.preservation_state(code)
            rc, out = self.run("mount", "--repo", code, "--tracker", new, "--aside", aside)
            self.check(rc == 2 and self.preservation_state(code) == before and not Path(aside).exists(),
                       "SAFE original-filesystem: an aside on another filesystem is refused with nothing changed")
        finally:
            same_filesystem = original_filesystem
        steps = []
        argv = ("mount", "--repo", code, "--tracker", new, "--aside", aside)
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
        self.interrupt_each(argv, steps, self.state_of(argv), "REQ-05 mount")
        stage = Path(code) / f"{STORE}.mount-stage"
        self.guard_probes(argv, "cloned", "mount", lambda r: (stage, stage / "config.toml"))
        self.restore()
        rc, _ = self.run(*argv, hooks={"step": interrupt_at("placed")})
        planted = store / ".server.log.cutover-stage"
        planted.write_text("foreign stage bytes\n")
        before = self.observe()
        rc, _ = self.run(*argv)
        self.check(rc == 2 and self.observe() == before,
                   "SAFE publication-stage-drift: a refusal preserves foreign stage bytes")
        self.unowned_location(argv, stage, "mount stage")
        self.unowned_location(argv, aside, "mount aside")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart mount final fixture")

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
        argv = ("harvest", "--store", shared, "--from", str(wa))
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0, "REQ-06 harvest passes with identical records and a pointer collision")
        self.check((runs / I2 / "r2.json").is_file(), "REQ-06 one file per run copied")
        self.check((runs / "verdicts" / "cargo-ci" / "d1.json").read_bytes() == pointer(I1, "r1"),
                   "REQ-06 a differing verdict pointer keeps the store's")
        self.check((runs / "verdicts" / "cargo-ci" / "d2.json").read_bytes() == pointer(I2, "r2"),
                   "REQ-06 a pointer the store lacks is copied")
        rc, _ = self.run("harvest", "--store", shared, "--from", str(wa))
        self.check(rc == 0, "REQ-06 a repeated harvest passes")
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart harvest")
        first = Path(shared) / steps[0].removeprefix("harvested ")
        self.guard_probes(argv, steps[0], "harvest", lambda r: None)
        self.restore()
        rc, _ = self.run(*argv, hooks={"step": interrupt_at(steps[0])})
        first.chmod(stat.S_IMODE(first.stat().st_mode) ^ stat.S_IXUSR)
        before = self.observe()
        rc, _ = self.run(*argv)
        self.check(rc == 2 and self.observe() == before,
                   "SAFE mode-drift harvest target: a copied file whose mode changed refuses")
        self.restore()
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart harvest final fixture")
        raw = Path(self.path("plain-harvest-source"))
        self.write_runs(raw, {f"{I9}/plain.json": b'{"run": "plain"}\n'})
        rc, out = self.run("harvest", "--store", shared, "--from", str(raw))
        self.check(rc == 0 and (runs / I9 / "plain.json").is_file(),
                   "SAFE harvest-source: an external plain store needs no repository")

    def clean(self, code, cutover_state):
        shared = str(Path(code) / STORE)
        wa, wb = self.path("wa"), self.path("wb")
        args = ("--store", shared, "--message", "chore: drop the tracker")
        if cutover_state == "before":
            head = git("rev-parse", "HEAD", cwd=wa).strip()
            rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
            self.check(rc == 1 and git("rev-parse", "HEAD", cwd=wa).strip() == head,
                       "REQ-07 clean fails while main tracks .jit and changes nothing")
            return
        events = Path(wb) / EVENTS
        events.open("a").write('{"event": "late"}\n')
        (Path(wb) / STORE / "issues" / "late.json").write_text('{"late": true}\n')
        self.commit_all(wb, "chore: tracker changes on wb after the cutover")
        before_store = relative_manifest(Path(wb) / STORE)
        start = git("rev-parse", "wb", cwd=code).strip()
        start_tree = git("rev-parse", "wb^{tree}", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--worktree", wb, *args,
                           hooks={"step": fail_at("store-removal")})
        self.check(rc == 1 and "restored" in out and git("rev-parse", "wb", cwd=code).strip() == start
                   and tracked_under("wb", STORE, code) != [] and (Path(wb) / STORE).is_dir()
                   and git("status", "--porcelain", "--untracked-files=no", cwd=wb) == ""
                   and not self.marker_for(("clean", "--worktree", wb)).exists(),
                   "REQ-07 a failing store deletion restores the branch ref and index and keeps the store")
        argv = ("clean", "--repo", code, "--worktree", wb, *args)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0 and f"wb: {CLEANED}" in out.splitlines(),
                   "REQ-07 clean passes on wb and reports tree equality outside .jit")
        self.check(git("ls-tree", "-r", "--name-only", "wb", "--", STORE, cwd=code) == ""
                   and git("ls-tree", "-r", "--name-only", "main", "--", STORE, cwd=code) == "",
                   "REQ-07 git ls-files .jit is empty on main and on wb")
        parents = git("rev-list", "--parents", "-n1", "wb", cwd=code).split()[1:]
        self.check(parents == [start], "REQ-07 the clean of wb is one commit on the tip, merging nothing")
        self.check(git_rc("diff", "--quiet", start, "wb", "--", ".", ":(exclude).jit", cwd=code) == 0,
                   "REQ-07 the tree of wb outside .jit equals the tip's")
        removed = git("diff", "--name-only", "--diff-filter=D", "wb^", "wb", cwd=code).split()
        self.check(f"{STORE}/issues/late.json" in removed,
                   "REQ-07 the branch-only tracker file is removed by the clean commit")
        self.check(git("ls-files", STORE, cwd=wb) == "" and git("status", "--porcelain", "--untracked-files=no", cwd=wb) == "",
                   "REQ-07 the index of wb follows the clean commit")
        self.check(not (Path(wb) / STORE).exists(), "REQ-07 wb's local store is deleted")
        backups = git("for-each-ref", "--format=%(refname)", "refs/cutover-archives", cwd=code).splitlines()
        saved = Path(self.path("retained-local-store.tar"))
        if backups:
            data = subprocess.run(["git", "cat-file", "blob", backups[-1]], cwd=code, capture_output=True, check=True).stdout
            saved.write_bytes(data)
        self.check(bool(backups) and tar_manifest(saved) == before_store,
                   "SAFE retained-clean-archive: the complete store before the clean survives marker removal")
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart linked-clean")
        local = Path(wb) / STORE
        self.guard_probes(argv, "archived local store", "linked-clean",
                          lambda r: (local, local / "config.toml"))
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart linked-clean final fixture")
        extra = Path(wa) / STORE / "gate-runs" / I2 / "unharvested.json"
        extra.write_bytes(b'{"run": "late"}\n')
        head = git("rev-parse", "HEAD", cwd=wa).strip()
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 2 and (Path(wa) / STORE).is_dir() and git("rev-parse", "HEAD", cwd=wa).strip() == head,
                   "REQ-07 clean refuses an unverified harvest and keeps the store")
        extra.unlink()

        def plant_mismatch(repo, tree):
            entries = [e for e in git("ls-tree", "-z", "main", cwd=repo).split("\0") if e]
            return git("mktree", "-z", cwd=repo, input="".join(f"{e}\0" for e in entries)).strip()

        before = self.observe()
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args, hooks={"clean_tree": plant_mismatch})
        self.check(rc == 2 and "tree outside .jit mismatch" in out
                   and git("rev-parse", "wa", cwd=code).strip() == head
                   and not git("status", "--porcelain", "--untracked-files=no", cwd=wa).strip()
                   and (Path(wa) / STORE).is_dir() and not self.marker_for(("clean", "--worktree", wa)).exists(),
                   "REQ-07 a planted tree mismatch is refused before the branch moves and keeps the store")
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 0 and not (Path(wa) / STORE).exists()
                   and f"wa: {CLEANED}" in out.splitlines()
                   and git("ls-tree", "-r", "--name-only", "wa", "--", STORE, cwd=code) == "",
                   "REQ-07 clean passes on wa")
        head = git("rev-parse", "wa", cwd=code).strip()
        local = Path(wa) / STORE
        local.mkdir()
        (local / "server.log").write_text("machine-local\n")
        self.write_runs(local, {f"{I2}/r2.json": b'{"run": "r2"}\n'})
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 0 and "wa: already clean" in out.splitlines()
                   and git("rev-parse", "wa", cwd=code).strip() == head
                   and not local.exists(),
                   "REQ-07 an already clean linked branch commits nothing and deletes its verified store")

    def parked_setup(self, code):
        fixture = self.path("parked-fixture")
        self.sh("git", "worktree", "add", "-q", "-b", "parked", fixture, "main", cwd=code)
        (Path(fixture) / STORE / "config.toml").open("a").write("# parked tracker change\n")
        (Path(fixture) / STORE / "issues" / "parked.json").write_text('{"parked": true}\n')
        self.commit_all(fixture, "chore: parked tracker changes")
        for name in ("parked-single", "parked-hook", "parked-empty"):
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
        before = git("worktree", "list", "--porcelain", cwd=code)
        tmp_before = set(Path(tempfile.gettempdir()).glob("cutover-*"))
        head = git("rev-parse", "parked-conflict", cwd=code).strip()
        single_head = git("rev-parse", "parked-single", cwd=code).strip()
        argv = ("clean", "--repo", code, "--branch", "parked-single", *args)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0 and out.strip() == f"parked-single: {CLEANED}",
                   "REQ-07 a single parked branch cleans without a shared store or harvest")
        self.check(not tracked_under("parked-single", STORE, code)
                   and git("rev-list", "--parents", "-n1", "parked-single", cwd=code).split()[1:] == [single_head],
                   "REQ-07 a parked branch gets one commit on its tip, merging nothing, without tracked tracker paths")
        removed = git("diff", "--name-only", "--diff-filter=D", "parked-single^", "parked-single", cwd=code)
        self.check(f"{STORE}/issues/parked.json" in removed,
                   "REQ-07 the parked branch-only tracker file is removed and committed")
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart parked-clean")
        self.guard_probes(argv, "clean committed", "parked-clean", lambda r: None)
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart parked-clean final fixture")
        clean_head = git("rev-parse", "parked-already", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-already", *args)
        self.check(rc == 0 and out.strip() == "parked-already: already clean"
                   and git("rev-parse", "parked-already", cwd=code).strip() == clean_head,
                   "REQ-07 an already clean parked branch commits nothing")
        self.check("src/lib.rs" in merge_result(code, "main", "parked-conflict")[1],
                   "REQ-09 the fixture branch parked-conflict conflicts with main outside .jit")
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-conflict", *args)
        self.check(rc == 0 and out.strip() == f"parked-conflict: {CLEANED}"
                   and git("rev-list", "--parents", "-n1", "parked-conflict", cwd=code).split()[1:] == [head]
                   and git_rc("diff", "--quiet", head, "parked-conflict", "--", ".", ":(exclude).jit", cwd=code) == 0
                   and not tracked_under("parked-conflict", STORE, code),
                   "REQ-07 a branch that conflicts with main outside .jit is cleaned: one commit, code tree unchanged")
        self.sh("git", "update-ref", "refs/heads/parked-conflict", head, cwd=code)
        branches = Path(self.path("branches.txt"))
        branches.write_text("# parked branches\nparked\nparked-conflict # outside conflict\n\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        print(out, end="")
        self.check(rc == 0 and out.splitlines() == [
            f"parked: {CLEANED}", f"parked-conflict: {CLEANED}", "parked-already: already clean"],
            "REQ-07 batch lists each outcome")
        self.check(git("rev-parse", "parked-already", cwd=code).strip() == clean_head,
                   "REQ-07 an already clean batch branch keeps its ref")
        self.check(all(not tracked_under(rev_name, STORE, code)
                       for rev_name in ("main", "parked", "parked-conflict", "parked-already")),
                   "REQ-07 main and every cleaned branch track no tracker path")
        self.check(git("worktree", "list", "--porcelain", cwd=code) == before,
                   "REQ-07 git worktree list is unchanged after the batch")
        self.check(set(Path(tempfile.gettempdir()).glob("cutover-*")) == tmp_before,
                   "REQ-07 success and refusal leave no temporary directory behind")
        branches.write_text("parked\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        self.check(rc == 0 and out.splitlines() == ["parked: already clean", "parked-already: already clean"],
                   "REQ-07 a batch of clean branches succeeds without commits")
        branches.write_text("missing-branch\nwa\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        lines = out.splitlines()
        self.check(rc == 1 and len(lines) == 3
                   and lines[0] == "missing-branch: refused: not a local branch: missing-branch"
                   and lines[1].startswith("wa: refused: wa is checked out;")
                   and lines[2] == "parked-already: already clean",
                   "REQ-07 other refusals include their reasons and do not stop the batch")
        self.check(git("worktree", "list", "--porcelain", cwd=code) == before
                   and set(Path(tempfile.gettempdir()).glob("cutover-*")) == tmp_before,
                   "REQ-07 other refusals leave no temporary directory or registration")
        for name in ("parked-batch-first", "parked-batch-second", "parked-batch-middle"):
            self.sh("git", "branch", name, "parked-hook", cwd=code)
        batch_file = Path(self.path("restart-branches.txt"))
        batch_file.write_text("parked-batch-first\nparked-batch-middle\nparked-batch-second\n")
        self.snapshot()
        count = 0

        def stop_second(name):
            nonlocal count
            if name == "clean published":
                count += 1
                if count == 2:
                    raise Interrupted(name)

        batch_argv = ("clean", "--repo", code, "--branches", str(batch_file), *args)
        rc, out = self.run(*batch_argv, hooks={"step": stop_second})
        self.check(rc == "interrupted" and self.record(batch_argv)["identity"]["branch"] == "parked-batch-middle",
                   "SAFE restart parked-batch: the interrupted branch owns the marker")
        rc, out = self.run(*batch_argv)
        self.check(rc == 0 and out.splitlines() == [
            "parked-batch-first: already clean", f"parked-batch-middle: {CLEANED}",
            f"parked-batch-second: {CLEANED}"]
            and not self.marker_for(batch_argv).exists(),
            "SAFE restart parked-batch: prior outcomes independent, the marked branch resumes")
        self.restore()
        start = git("rev-parse", "parked-hook", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-hook", *args,
                           hooks={"step": fail_at("clean published")})
        self.check(rc == 1 and out.startswith("parked-hook: failed:") and "restored" in out
                   and git("rev-parse", "parked-hook", cwd=code).strip() == start
                   and tracked_under("parked-hook", STORE, code) != []
                   and git("worktree", "list", "--porcelain", cwd=code) == before
                   and not self.marker_for(argv).exists(),
                   "REQ-07 a failure after the removal commit is published restores the branch ref")
        start = git("rev-parse", "parked-empty", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-empty", "--message", "")
        self.check(rc == 1 and out.startswith("parked-empty: refused:")
                   and git("rev-parse", "parked-empty", cwd=code).strip() == start
                   and tracked_under("parked-empty", STORE, code) != [],
                   "REQ-07 an empty commit message is refused and leaves the branch ref")

    def rollback(self, code, cutover, archives):
        store = Path(code) / STORE
        issue_files = sorted(p.name for p in (store / "issues").iterdir())
        (store / "issues" / issue_files[0]).unlink()
        with open(store / "events.jsonl", "a") as log:
            log.write('{"event": "after the cutover"}\n')
        self.commit_all(str(store), "snapshot after the cutover")
        (store / "issues" / "after.json").write_text('{"after": true}\n')
        unsnapshotted = f"issues/{issue_files[1]}"
        (store / unsnapshotted).unlink()
        plain = Path(self.path("unregistered-checkout"))
        plain.mkdir()
        (plain / "sentinel").write_text("user bytes\n")
        manifest = read_manifest(Path(archives))
        manifest["unregistered"] = {**manifest["worktree-wa"], "checkout": str(plain)}
        shutil.copy2(Path(archives) / "worktree-wa.tar", Path(archives) / "unregistered.tar")
        (Path(archives) / "manifest.json").write_text(json.dumps(manifest))
        aside = self.path("nested-aside")
        argv = ("rollback", "--repo", code, "--cutover", cutover, "--aside", aside,
                "--message", "chore: roll back the cutover", "--archives", archives)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0, "REQ-08 rollback passes")
        if rc != 0:
            return
        self.check(git("rev-parse", "--show-toplevel", cwd=aside).strip() == str(Path(aside).resolve()),
                   "REQ-08 the nested repository is moved aside")
        self.check(git_rc("diff", "--quiet", f"{cutover}^", "HEAD", "--", ".", ":!.jit", cwd=code) == 0,
                   "REQ-08 the code-side commit is reverted")
        expected = {}
        deleted = set(worktree_listing(aside, "-d"))
        for rel in worktree_listing(aside, "-co", "--exclude-standard"):
            if rel not in deleted:
                expected[f"{STORE}/{rel}"] = git("hash-object", "--no-filters", rel, cwd=aside).strip()
        restored = {}
        for entry in filter(None, git("ls-tree", "-r", "-z", "HEAD", "--", STORE, cwd=code).split("\0")):
            meta, rel = entry.split("\t", 1)
            restored[rel] = meta.split()[2]
        self.check(restored == expected, "REQ-08 the tracked store holds each tracker change since the cutover")
        self.check(f"{STORE}/issues/{issue_files[0]}" not in restored, "REQ-08 a snapshotted deletion is carried")
        self.check(f"{STORE}/{unsnapshotted}" not in restored and not (store / unsnapshotted).exists()
                   and unsnapshotted in deleted,
                   "REQ-08 a deletion the tracker never snapshotted is carried")
        self.check(f"{STORE}/issues/after.json" in restored, "REQ-08 an unsnapshotted addition is carried")
        self.check((store / "gate-runs" / I3 / "r3.json").is_file(), "REQ-08 the shared run records stay")
        self.check(git("status", "--porcelain", cwd=code) == "", "REQ-08 the code checkout is clean")
        manifest = json.loads(Path(archives, "manifest.json").read_text())
        for name in ("wa", "wb"):
            files = manifest[f"worktree-{name}"]["files"]
            root = Path(self.path(name)) / STORE
            self.check(store_files(root) == files, f"REQ-08 the store of {name} is restored from its archive")
            complete = relative_manifest(root)
            complete.pop(".")
            self.check(complete == tar_manifest(Path(archives) / f"worktree-{name}.tar"),
                       f"SAFE restored-archive {name}: modes, symlinks and empty directories preserved")
        end = self.state_of(argv)
        self.restore()
        rc, out = self.run(*argv, hooks={"step": interrupt_at("moved")})
        record = self.record(argv)
        self.check(rc == "interrupted" and not (plain / STORE).exists()
                   and str(plain) in [r["checkout"] for r in record["plan"]["restores"]],
                   "SAFE external-restore-plan: an unregistered checkout is recorded and untouched before its step")
        rc, out = self.run(*argv)
        self.check(rc == 0 and self.state_of(argv) == end and (plain / "sentinel").read_text() == "user bytes\n",
                   "SAFE external-restore-resume: manifest checkout restored with caller bytes preserved")
        self.interrupt_each(argv, steps, end, "REQ-08 rollback")
        self.guard_probes(argv, "copied", "rollback", lambda r: (store, store / "config.toml"))
        self.unowned_location(argv, aside, "rollback aside")
        self.unowned_location(argv, Path(self.path("wa")) / f"{STORE}.rollback-stage", "rollback stage")
        self.unowned_location(argv, plain / f"{STORE}.rollback-stage", "unregistered rollback stage")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart rollback final fixture")


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
    config.write_text("[user]\n\tname = cutover self-test\n\temail = self-test@invalid\n[init]\n\ttemplateDir =\n")
    os.environ.update(GIT_CONFIG_GLOBAL=str(config), GIT_CONFIG_NOSYSTEM="1",
                      HOME=str(base), XDG_CONFIG_HOME=str(base / ".config"))
    previous_temp = tempfile.tempdir
    scratch = base.parent / f"{base.name}-tmp"
    scratch.mkdir()
    tempfile.tempdir = str(scratch)
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
        cutover = test.code_side(code)
        print("mount")
        test.mount(code, new)
        print("harvest")
        test.harvest(code)
        print("clean")
        test.clean(code, "after")
        test.parked_clean(code)
        print("rollback")
        test.rollback(code, cutover, archives)
    except (Failure, Refusal, subprocess.CalledProcessError, OSError, KeyError) as error:
        test.failures.append(f"aborted: {error}")
        print(f"  FAIL aborted: {error}")
    finally:
        tempfile.tempdir = previous_temp
        shutil.rmtree(base, ignore_errors=True)
        shutil.rmtree(scratch, ignore_errors=True)
    if test.failures:
        print(f"cutover.py --self-test: {len(test.failures)} failure(s), {test.passed} passed", file=sys.stderr)
        return 1
    print(f"cutover.py --self-test: ok, {test.passed} checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
