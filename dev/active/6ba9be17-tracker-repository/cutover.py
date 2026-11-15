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
      a pending redo record (`<store>/tmp/redo.record`). A line starting
      `action:` needs action before the cutover: a branch whose merge into
      the main branch would change its `.jit` tree, a checkout with
      uncommitted tracker changes, a lease or a redo record. Other lines
      start `info:`.
  cutover.py archive --repo CODE --dest DIR [--verify]
      Writes the store of every checkout to `DIR/<name>.tar`, `<name>` being
      `primary` or `worktree-<name git registers>`, and the SHA-256 of every
      member to `DIR/manifest.json`; then re-reads each archive and compares
      it, and each store still on disk, with the manifest. `--verify` only
      re-reads; a store a clean deleted is compared with its archive alone.
      Each archive and the manifest are written under a `.partial` name and
      renamed; a matching durable marker resumes the recorded partial set.
      Refuses a DIR inside a store, inside the git directory or unignored
      inside a checkout, and, when writing, a DIR holding a manifest or
      files of its own.
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
      event log and `.jit/references.toml`. A rerun after an interruption
      validates the partial edit (index changes under `.jit/` and to those
      files) only when a durable marker records their exact contents. A
      marked rerun resumes its next step, including after its commit. Refuses a linked worktree, another branch, any other
      staged or unstaged change to those files, or an index without
      tracker paths.
  cutover.py mount --repo PRIMARY --tracker NEW --aside PATH
      Clones NEW into a sibling of `.jit`, moves `.jit` to PATH, moves the
      clone into place and copies back the machine-local files: those the
      patterns of the store's `.gitignore` outside jit's marker region
      match. Fails unless every tracked file of the mount is byte-identical
      to the head tree of NEW. A rerun after an interruption resumes: with
      a matching marker it validates the old store, clone and staging
      contents and resumes from the next recorded step.
      Refuses a linked worktree, a code repository that still tracks or does
      not ignore `.jit`, a PATH on another filesystem than `.jit` and a PATH
      that is not the old store of an interrupted mount.
  cutover.py harvest --store SHARED --from STORE
      Copies the run records of STORE, one file per run under
      `gate-runs/<issue>/`, into SHARED and copies each verdict pointer
      (`gate-runs/verdicts/<gate>/<inputs>.json`) SHARED lacks. Where both
      hold a pointer with different bytes it keeps SHARED's. Stops, copying
      nothing, on a run record SHARED holds with different bytes or on a
      pointer whose run SHARED would not hold; then verifies the harvest.
  cutover.py clean --repo CODE --worktree WT --store SHARED
                --merge-message MSG --message MSG [--main BRANCH]
      Verifies the harvest of `WT/.jit` into SHARED, merges the main branch
      into the branch of the linked worktree WT with tracker conflicts
      resolved by deletion, removes every path still tracked under `.jit/`
      and commits, then deletes `WT/.jit`, also when the branch was already
      clean and nothing is committed. Before merging, retains a verified
      full-store tar as a blob under refs/cutover-archives/. Fails unless `git ls-files .jit` is
      empty on the main branch and on the branch. Refuses an unverified
      harvest, uncommitted tracked changes, a detached WT and a conflict
      outside `.jit/` (the merge is aborted). Verifies that the cleaned
      commit's tree outside `.jit` equals the merge result. Any failure
      after the merge starts (a mismatch, a rejected or empty commit
      message, a failing `git rm`) restores the branch ref and checkout to
      the pre-clean head and says so; so does a failing deletion of the
      local store, whose message says part of it may be gone. When that
      restore fails, the message names the commit the ref is left at.
  cutover.py clean --repo CODE (--branch B | --branches FILE)
                --merge-message MSG --message MSG [--main BRANCH]
      Cleans a parked branch without a shared store or harvest, in a temporary
      linked worktree under TMPDIR outside every tracked tree, removed on
      success or failure. FILE lists one branch per line with optional `#`
      comments. Prints one line per branch: `cleaned; tree outside .jit equals
      the merge result`, `already clean` when
      main is already merged and no tracker paths remain (nothing committed),
      `conflict outside .jit: not forced: <paths>` (merge aborted, ref
      unchanged), `refused: <reason>` or `failed: <reason>`, the reason
      saying whether the ref was restored. Refuses a branch already checked
      out; use the worktree form for a linked checkout. A batch continues
      after failures. Both `--branch` and `--branches` exit 1 when any
      branch is neither cleaned nor already clean, and 2 only for a usage
      error or an unreadable FILE. Both forms require main and every
      cleaned branch to track no path under `.jit/`. A failing removal of
      the temporary worktree after a clean restores the branch ref and
      prunes the registration; an error removing it after another failure
      is reported together with that failure.
  cutover.py rollback --repo PRIMARY --cutover REV --aside PATH --message MSG
                   [--archives DIR] [--main BRANCH]
      Moves the mounted tracker repository to PATH, reverts the code-side
      commit REV and copies the moved store's files over the restored
      `.jit`, deleting the tracked paths the store no longer has, in one
      commit. With `--archives`, restores each linked worktree's store that
      a clean removed from its verified archive, extracting into a staging
      directory checked against the manifest and renamed into place. A
      tracked file missing from the store counts as deleted. A rerun after
      an interruption requires a matching durable marker, validates the
      recorded contents, index and tool-made commits, and resumes the next
      step. A completed rollback has no marker and refuses recovery. Refuses a state that is none of these, PATH
      on another filesystem than `.jit`, uncommitted changes outside the
      cutover commit's paths and a REV that is not a code-side commit on the
      main branch; a conflicting revert is aborted and the store moved back.
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
import copy
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
from pathlib import Path

STORE = ".jit"
BUDGET = "scripts/tracked-budget.toml"
BUDGET_CHECKER = "scripts/check-tracked-budget.py"
REFERENCES = "scripts/reference-exceptions.toml"
EVENTS = ".jit/events.jsonl"
CITATIONS = ".jit/references.toml"
FORBID_STORE = r"^\.jit/"
CONFLICT_OUTSIDE = f"conflict outside {STORE}: not forced"
TREE_STEP = 10_000
MARKER_BEGIN = "# jit:data-root-ignore:begin"
MARKER_END = "# jit:data-root-ignore:end"


class Refusal(Exception):
    """A precondition the step requires does not hold; nothing was changed."""


class Failure(Exception):
    """The step or its verification failed."""


def git(*args, cwd, check=True, input=None):
    mutating = args[0] in {"init", "config", "push", "clone", "reset", "checkout", "add", "rm",
                           "commit", "merge", "revert", "update-ref", "subtree"}
    mutating |= args[0] == "hash-object" and "-w" in args
    mutating |= args[0] == "worktree" and len(args) > 1 and args[1] != "list"
    if mutating:
        require_planning(cwd)
        if args[0] not in ("init", "clone"):
            require_repository_planning(cwd)
    result = git_result(args, cwd, input)
    if check and result.returncode != 0:
        raise Failure(f"git {' '.join(args)} in {cwd}: {result.stderr.strip()}")
    return result.stdout


def git_rc(*args, cwd):
    return git_result(args, cwd).returncode


def git_result(args, cwd, input=None):
    command = ["git", "--no-optional-locks", *args]
    # Git diff may refresh even with optional locks disabled. Read a private
    # index so a refusal preserves the caller's index byte for byte.
    if PLANNING is None and args[0] in ("diff", "status"):
        index = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-path", "index"],
                               cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()
        with tempfile.TemporaryDirectory(prefix="cutover-read-") as scratch:
            private = Path(scratch) / "index"
            if Path(index).exists():
                shutil.copy2(index, private)
            return subprocess.run(command, cwd=cwd, capture_output=True, text=True, input=input,
                                  env=dict(os.environ, GIT_INDEX_FILE=str(private)))
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True, input=input)


def main(argv, *, hooks=None):
    parser = argparse.ArgumentParser(prog="cutover.py", description=__doc__.split("\n")[0])
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="command")
    p = sub.add_parser("inventory")
    p.add_argument("--repo", required=True)
    p.add_argument("--main", default="main")
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
    p = sub.add_parser("clean", description="Merge main and remove every tracked tracker path.",
                       epilog="Parked branches use temporary worktrees under TMPDIR; --branches lists "
                       "one branch per line with # comments. Each branch prints its outcome; "
                       "any branch not cleaned makes the exit status 1.")
    p.add_argument("--repo", required=True)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument("--worktree", help="linked worktree whose harvest must be verified")
    target.add_argument("--branch", help="parked local branch without a worktree")
    target.add_argument("--branches", help="file listing parked local branches")
    p.add_argument("--store", help="shared store; required with --worktree only")
    p.add_argument("--merge-message", required=True)
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
        if args.command == "inventory" or (args.command == "archive" and args.verify):
            return COMMANDS[args.command](args) or 0
        if args.command == "clean" and not args.worktree:
            return clean(args) or 0
        return transactional(args) or 0
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
    require_planning(target)
    target.parent.mkdir(parents=True, exist_ok=True)
    stage = target.with_name(f".{target.name}.cutover-{os.getpid()}")
    if stage.exists() or stage.is_symlink():
        raise Refusal(f"unowned copy stage; changes: {stage}")
    with stage.open("xb") as handle, source.open("rb") as original:
        shutil.copyfileobj(original, handle)
    try:
        shutil.copystat(source, stage)
        os.replace(stage, target)
    finally:
        stage.unlink(missing_ok=True)


def step(args, name):
    """Marks the end of a step; the self-test interrupts or fails a subcommand here."""
    args.hooks.get("step", lambda _: None)(name)


def same_filesystem(a, b):
    return os.stat(a).st_dev == os.stat(b).st_dev


def worktree_listing(checkout, *args):
    return [p for p in git("ls-files", "-z", *args, cwd=checkout).split("\0") if p]


# inventory ------------------------------------------------------------------


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
        merged = subprocess.run(["git", "merge-tree", "--write-tree", "--no-messages", args.main, rev],
                                cwd=repo, capture_output=True, text=True)
        result = merged.stdout.split("\n", 1)[0].strip()
        if merged.returncode != 0 or tree_of(result, STORE, repo) != main_tree:
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


def relative_manifest(root):
    return {str(Path(path).relative_to(root)): value
            for path, value in path_manifest([str(root)], set()).items()}


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


def write_archives(args, dest, found):
    manifest = {}
    for checkout in found:
        store = checkout["path"] / STORE
        if not store.is_dir():
            continue
        partial = dest / f"{checkout['name']}.tar.partial"
        entries = full_store_archive(store, partial, include_root=False)
        files = {rel: value["sha256"] for rel, value in entries.items() if value["kind"] == "file"}
        os.rename(partial, dest / f"{checkout['name']}.tar")
        manifest[checkout["name"]] = {"checkout": str(checkout["path"]), "files": files}
        step(args, f"archived {checkout['name']}")
        print(f"archived {store}: {len(files)} files in {checkout['name']}.tar")
    (dest / "manifest.json.partial").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    os.rename(dest / "manifest.json.partial", dest / "manifest.json")


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


def archive(args):
    if not args.verify:
        require_planning(args.repo)
    dest = output_location(args.dest)
    found = checkouts(args.repo)
    refuse_archive_dest(dest, found, args.repo)
    if not args.verify:
        if dest.exists() and (not dest.is_dir() or any(dest.iterdir())):
            raise Refusal(f"{dest} exists and holds a written archive set or other files")
        dest.mkdir(parents=True, exist_ok=True)
        write_archives(args, dest, found)
    manifest = read_manifest(dest)
    faults = []
    for name, entry in sorted(manifest.items()):
        faults += archive_faults(dest, name, entry, against_disk=True)
    for fault in faults:
        print(fault)
    if faults:
        raise Failure(f"{len(faults)} archive faults")
    print(f"verified {len(manifest)} archives in {dest}")


# extract --------------------------------------------------------------------


def subtree_split(repo, commit):
    """Split in the private repository; its cache is never published."""
    require_planning(repo)
    return git("subtree", "split", f"--prefix={STORE}", commit, cwd=repo).split()[-1]


def extract(args):
    require_planning(args.repo)
    repo, dest = args.repo, output_location(args.dest)
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
    stage.mkdir()
    try:
        git("init", "-q", "-b", "main", str(stage), cwd=repo)
        git("config", "receive.denyCurrentBranch", "updateInstead", cwd=stage)
        git("push", "-q", str(stage), f"{split}:refs/heads/main", cwd=repo)
        git("config", "--unset", "receive.denyCurrentBranch", cwd=stage)
        head_tree = git("rev-parse", "HEAD^{tree}", cwd=stage).strip()
        if head_tree != code_tree or git("status", "--porcelain", cwd=stage).strip():
            raise Failure(f"head tree {head_tree} of the new repository differs from {code_tree}")
        step(args, "extraction staged")
        os.rename(stage, dest)
        step(args, "extracted")
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
    """Plan the code-side commit in an exclusively owned replica."""
    require_planning(args.repo)
    repo = primary_checkout(args.repo)
    require_branch(repo, args.main)
    edited = (".gitignore", ".gitattributes", BUDGET, REFERENCES)
    staged, unstaged = changed_paths(repo, "--cached", "HEAD"), changed_paths(repo)
    if staged or unstaged & set(edited):
        raise Refusal(f"the checkout holds changes: {sorted(staged | unstaged)}")
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
    require_planning(args.repo)
    repo = primary_checkout(args.repo)
    tracker, aside, store = physical(args.tracker), output_location(args.aside), repo / STORE
    if worktree_listing(repo, "--", STORE):
        raise Refusal(f"the code repository still tracks {STORE}; run code-side first")
    if git_rc("check-ignore", "-q", f"{STORE}/", cwd=repo) != 0:
        raise Refusal(f"the code repository does not ignore {STORE}/")
    moved = aside.exists() or aside.is_symlink()
    if not moved:
        if not store.is_dir() or own_repository(store):
            raise Refusal(f"no old store at {store}")
        if not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent):
            raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}")
    elif not aside.is_dir() or own_repository(aside) or (store.exists() and not own_repository(store)):
        raise Refusal(f"{aside} exists and is not the old store of an interrupted mount")
    if not own_repository(store):
        stage = repo / f"{STORE}.mount-stage"
        if stage.exists() or stage.is_symlink():
            raise Refusal(f"unowned mount stage; changes: {stage}")
        git("clone", "-q", "--", str(tracker), str(stage), cwd=repo)
        step(args, "cloned")
        if not moved:
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
    require_planning(args.store)
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
        step(args, f"harvested {rel}")
    for rel in kept:
        print(f"kept the store's verdict pointer {rel}")
    left, unset, _, faults = harvest_plan(shared, source)
    if left or unset or faults:
        raise Failure(f"harvest of {source} does not verify: {faults or left + unset}")
    print(f"harvested {source}: {len(runs)} run records and {len(pointers)} pointers copied, verified")


# clean ----------------------------------------------------------------------


def clean(args):
    if args.worktree:
        if not args.store:
            raise Refusal("--worktree requires --store")
        return clean_linked(args)
    if args.store:
        raise Refusal("--store applies only to --worktree")
    if args.branches:
        try:
            branches = [line.partition("#")[0].strip()
                        for line in Path(args.branches).read_text().splitlines()]
        except (OSError, ValueError) as error:
            raise Refusal(f"cannot read branches: {error}")
        branches = [branch for branch in branches if branch]
    else:
        branches = [args.branch]
    failed = False
    for branch in branches:
        if PLANNING is None:
            single = argparse.Namespace(**vars(args))
            single.branch, single.branches = branch, None
            try:
                marker = state_path(args.repo)
                pending = json.loads(marker.read_text()) if marker.is_file() else None
                if (pending and pending.get("arguments", {}).get("command") == "clean"
                        and pending["arguments"].get("branch") != branch):
                    # Completed and failed earlier batch entries remain independent
                    # of the one interrupted entry. A preview never publishes.
                    verify_plan(pending)
                    with tempfile.TemporaryDirectory(prefix="cutover-preview-") as scratch:
                        records = participants(single)
                        roots = publication_roots(single, records)
                        planned, _ = make_plan(single, records, roots, Path(scratch))
                        validate_record(planned)
                        if planned["result"] or not any(a["kind"] != "checkpoint" for a in planned["actions"]):
                            print(planned["output"], end="")
                            failed |= planned["result"] != 0
                            continue
                result = transactional(single)
                failed |= result != 0
            except (ValueError, KeyError, Refusal, Failure, OSError) as error:
                print(f"{branch}: {'refused' if isinstance(error, Refusal) else 'failed'}: {error}")
                failed = True
            continue
        try:
            outcome = clean_parked(args, branch)
        except (Refusal, Failure, OSError) as error:
            outcome = " ".join(str(error).splitlines())
            if not outcome.startswith(CONFLICT_OUTSIDE):
                outcome = f"{'refused' if isinstance(error, Refusal) else 'failed'}: {outcome}"
            failed = True
        print(f"{branch}: {outcome}")
    return 1 if failed else 0


def retain_local_store(args, worktree, store):
    if not store.is_dir():
        return
    archive_path = PLANNING / "local-store.tar"
    full_store_archive(store, archive_path)
    blob = git("hash-object", "-w", "--", str(archive_path), cwd=worktree).strip()
    ref = f"refs/cutover-archives/{PLANNING.name}"
    git("update-ref", ref, blob, "0" * 40, cwd=worktree)
    archive_path.unlink()
    step(args, "archived local store")


def clean_linked(args):
    require_planning(args.repo)
    found = checkouts(args.repo)
    worktree = physical(args.worktree)
    checkout = next((c for c in found if c["path"] == worktree), None)
    if checkout is None or checkout["primary"]:
        raise Refusal(f"{worktree} is not a linked worktree of {args.repo}")
    branch = checkout["branch"]
    if branch is None:
        raise Refusal(f"{worktree} has a detached HEAD")
    if tracked_under(args.main, STORE, worktree):
        raise Failure(f"{args.main} still tracks paths under {STORE}")
    store, shared = worktree / STORE, physical(args.store)
    if store.is_dir():
        runs, pointers, _, faults = harvest_plan(shared, store)
        if runs or pointers or faults:
            raise Refusal(f"the harvest of {store} into {shared} is not verified")
    retain_local_store(args, worktree, store)
    def remove_store():
        try:
            step(args, "store-removal")
            if store.is_dir():
                shutil.rmtree(store)
        except OSError as error:
            raise Failure(f"deleting the local store {store} failed and may have removed part of it; "
                          f"its run records are harvested into {shared}: {error}")
        print(f"deleted the local store {store}")

    try:
        outcome = clean_checkout(args, worktree, branch, finish=remove_store)
    except Refusal as error:
        print(f"{branch}: refused: {error}")
        return 2
    print(f"{branch}: {outcome}")


def clean_parked(args, branch):
    require_planning(args.repo)
    repo = physical(args.repo)
    if (git_rc("check-ref-format", "--branch", branch, cwd=repo) != 0
            or git_rc("rev-parse", "--verify", f"refs/heads/{branch}^{{commit}}", cwd=repo) != 0):
        raise Refusal(f"not a local branch: {branch}")
    found = checkouts(repo)
    if any(c["branch"] == branch for c in found):
        raise Refusal(f"{branch} is already checked out; use --worktree and --store for a linked checkout")
    if tracked_under(args.main, STORE, repo):
        raise Failure(f"{args.main} still tracks paths under {STORE}")
    with tempfile.TemporaryDirectory(prefix="cutover-clean-") as directory:
        root = physical(directory)
        refuse_archive_dest(root, found, repo)
        if git("rev-parse", "--show-toplevel", cwd=root, check=False).strip():
            raise Refusal("TMPDIR places the temporary worktree inside a tracked tree")
        worktree = root / "worktree"
        error = None
        def remove_worktree():
            step(args, "worktree-removal")
            git("worktree", "remove", "--force", str(worktree), cwd=repo)

        try:
            git("worktree", "add", "-q", "--", str(worktree), branch, cwd=repo)
            return clean_checkout(args, worktree, branch, finish=remove_worktree, repo=repo)
        except (Refusal, Failure, OSError) as raised:
            error = raised
            raise
        finally:
            try:
                if any(c["path"] == worktree for c in checkouts(repo)):
                    git("worktree", "remove", "--force", str(worktree), cwd=repo)
            except (Failure, OSError) as cleanup:
                prior = f"{error}; and " if error else ""
                raise Failure(f"{prior}removing the temporary worktree {worktree} failed: {cleanup}") from error


def clean_checkout(args, worktree, branch, finish=None, repo=None):
    """Cleans the branch checked out at `worktree`, then runs `finish`.

    Any failure, `finish` included, restores the branch ref and checkout; `repo`
    restores the ref when `finish` removed the checkout.
    """
    if git("status", "--porcelain", "--untracked-files=no", cwd=worktree).strip():
        raise Refusal(f"{worktree} has uncommitted tracked changes")
    head = git("rev-parse", "HEAD", cwd=worktree).strip()
    try:
        outcome = merge_and_remove(args, worktree, branch, head)
        if finish:
            finish()
        return outcome
    except (Refusal, Failure, OSError) as error:
        raise restored(error, worktree, branch, head, repo) from error


def restored(error, worktree, branch, head, repo=None):
    """`error` after putting `branch` back at `head`, or a Failure naming the unrestored ref."""
    if PLANNING is None or not inside(worktree, PLANNING):
        current = git("rev-parse", branch, cwd=worktree, check=False).strip()
        return Failure(f"{error}; {branch} is left at {current}, not restored to {head}: no owned planning checkout")
    kind = type(error) if isinstance(error, (Refusal, Failure)) else Failure
    if not (Path(worktree) / ".git").exists() and repo is not None:
        ref = f"refs/heads/{branch}"
        current = git("rev-parse", "-q", "--verify", ref, cwd=repo, check=False).strip()
        try:
            if current != head:
                git("update-ref", ref, head, cwd=repo)
            git("worktree", "prune", cwd=repo)
        except Failure as update:
            return Failure(f"{error}; {branch} is left at {current or 'an unknown commit'}, "
                           f"not restored to {head}: {update}")
        return kind(f"{error}; {branch} restored to {head}" if current != head else str(error))
    if git_rc("rev-parse", "-q", "--verify", "MERGE_HEAD", cwd=worktree) == 0:
        git("merge", "--abort", cwd=worktree, check=False)
    current = git("rev-parse", "HEAD", cwd=worktree, check=False).strip()
    dirty = git("status", "--porcelain", "--untracked-files=no", cwd=worktree, check=False).strip()
    if current == head and not dirty:
        return kind(str(error))
    try:
        git("reset", "-q", "--hard", head, cwd=worktree)
    except Failure as reset:
        return Failure(f"{error}; {branch} is left at {current or 'an unknown commit'}, "
                       f"not restored to {head}: {reset}")
    return kind(f"{error}; {branch} restored to {head}")


def merge_and_remove(args, worktree, branch, head):
    require_repository_planning(worktree)
    merged = subprocess.run(["git", "merge", "-q", "--no-edit", "-m", args.merge_message, args.main],
                            cwd=worktree, capture_output=True, text=True)
    step(args, "merge attempted")
    if merged.returncode != 0:
        conflicts = [p for p in git("diff", "-z", "--name-only", "--diff-filter=U", cwd=worktree).split("\0") if p]
        outside = [p for p in conflicts if not p.startswith(f"{STORE}/")]
        if outside or not conflicts:
            git("merge", "--abort", cwd=worktree, check=False)
            if outside:
                raise Refusal(f"{CONFLICT_OUTSIDE}: {', '.join(outside)}")
            raise Failure(f"merging {args.main} into {branch}: {merged.stderr.strip()}")
        git("rm", "-q", "--cached", "--", *conflicts, cwd=worktree)
        git("commit", "-q", "--no-edit", cwd=worktree)
    merge_result = git("rev-parse", "HEAD", cwd=worktree).strip()
    step(args, "merged")
    if worktree_listing(worktree, "--", STORE):
        git("rm", "-r", "-q", "--cached", "--", STORE, cwd=worktree)
        git("commit", "-q", "-m", args.message, cwd=worktree)
        step(args, "clean committed")
    for rev in (args.main, branch):
        if tracked_under(rev, STORE, worktree):
            raise Failure(f"{rev} still tracks paths under {STORE}")
    if git("rev-parse", "HEAD", cwd=worktree).strip() == head:
        return "already clean"
    if "after_clean" in args.hooks:
        args.hooks["after_clean"](worktree)
    if git_rc("diff", "--quiet", merge_result, "HEAD", "--", ".", ":(exclude).jit", cwd=worktree) != 0:
        raise Refusal("tree outside .jit mismatch with the merge result")
    return "cleaned; tree outside .jit equals the merge result"


# rollback -------------------------------------------------------------------


def rollback(args):
    """Plan a fresh rollback; only the publication journal handles restarts."""
    require_planning(args.repo)
    repo = primary_checkout(args.repo)
    require_branch(repo, args.main)
    store, aside = repo / STORE, output_location(args.aside)
    cutover = git("rev-parse", "--verify", f"{args.cutover}^{{commit}}", cwd=repo).strip()
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
    if aside.exists() or aside.is_symlink() or not own_repository(store):
        raise Refusal(f"no fresh mounted repository to roll back; changes: {store}, {aside}")
    if git("status", "--porcelain", "--untracked-files=no", cwd=repo).strip():
        raise Refusal(f"{repo} has uncommitted tracked changes")
    if not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent):
        raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}")
    os.rename(store, aside)
    step(args, "moved")
    restore_tracked_store(args, repo, cutover, store, aside)
    for name, entry in sorted((manifest or {}).items()):
        checkout = Path(entry["checkout"])
        if name == "primary" or not checkout.is_dir() or (checkout / STORE).exists():
            continue
        stage = checkout / f"{STORE}.rollback-stage"
        require_planning(stage)
        if stage.exists() or stage.is_symlink():
            raise Refusal(f"unowned rollback stage; changes: {stage}")
        stage.mkdir()
        with tarfile.open(physical(args.archives) / f"{name}.tar") as tar:
            tar.extractall(stage, filter="tar")
        expected = tar_manifest(output_location(args.archives) / f"{name}.tar")
        for rel, value in sorted(expected.items(), key=lambda item: -len(Path(item[0]).parts)):
            if value["kind"] != "symlink":
                (stage / rel).chmod(value["mode"])
        actual = relative_manifest(stage)
        if "." not in expected:
            actual.pop(".")
        if store_files(stage) != entry["files"] or actual != expected:
            raise Failure(f"{stage} differs from the manifest of {name}.tar")
        step(args, f"extracted {name}")
        os.rename(stage, checkout / STORE)
        step(args, f"restored {name}")
        print(f"restored {checkout / STORE} from {name}.tar")


def restore_tracked_store(args, repo, cutover, store, aside):
    """Reverts the code-side commit and carries the moved store's state into one commit."""
    require_repository_planning(repo)
    reverted = subprocess.run(["git", "revert", "--no-commit", cutover], cwd=repo, capture_output=True, text=True)
    if reverted.returncode != 0:
        git("revert", "--abort", cwd=repo, check=False)
        git("reset", "-q", "--hard", "HEAD", cwd=repo)
        if not own_repository(store):
            shutil.rmtree(store, ignore_errors=True)
            os.rename(aside, store)
        raise Failure(f"reverting {cutover} conflicts: {reverted.stderr.strip()}; store moved back")
    step(args, "reverted")
    deleted = set(worktree_listing(aside, "-d"))
    current = set(worktree_listing(aside, "-co", "--exclude-standard")) - deleted
    restored = set(p.removeprefix(f"{STORE}/") for p in worktree_listing(repo, "--", STORE))
    gone = sorted(restored - current)
    if gone:
        git("rm", "-q", "-f", "--ignore-unmatch", "--", *(f"{STORE}/{p}" for p in gone), cwd=repo)
    step(args, "deleted")
    local = set(worktree_listing(aside, "-o", "-i", "--exclude-standard"))
    for index, rel in enumerate(sorted(current | local)):
        if not rel.startswith(".git/") and (aside / rel).is_file():
            copy_file(aside / rel, store / rel)
            if index == 0:
                step(args, "copied a file")
    step(args, "copied")
    git("add", "-A", "--", STORE, cwd=repo)
    step(args, "staged")
    git("commit", "-q", "-m", args.message, cwd=repo)
    step(args, "committed")
    print(f"reverted {cutover}; {len(current)} tracker files restored, {len(gone)} deleted; "
          f"tracker repository at {aside}")


COMMANDS = {
    "inventory": inventory, "archive": archive, "extract": extract, "code-side": code_side,
    "mount": mount, "harvest": harvest, "clean": clean, "rollback": rollback,
}


# Durable publication --------------------------------------------------------

STATE = "cutover-state.json"
PLANNING = None


def require_planning(path):
    if PLANNING is None or not inside(path, PLANNING):
        raise Refusal(f"mutation requires an exclusively owned planning replica: {path}")


def require_repository_planning(repo):
    require_planning(repo)
    for argument in ("--absolute-git-dir", "--git-common-dir"):
        result = subprocess.run(["git", "rev-parse", argument], cwd=repo, capture_output=True, text=True)
        if result.returncode:
            raise Refusal(f"no isolated planning repository at {repo}")
        require_planning(physical(Path(repo) / result.stdout.strip()))


def state_path(repo):
    return Path(git("rev-parse", "--path-format=absolute", "--git-path", STATE, cwd=repo).strip())


def atomic_marker(path, record, *, create=False):
    """Publish a complete marker, including across loss of the writing process."""
    if create:
        if path.exists() or path.is_symlink():
            raise Refusal(f"another marker appeared; changes: {path}")
    else:
        try:
            previous = json.loads(path.read_text())
            if previous["id"] != record["id"] or previous["plan_sha256"] != record["plan_sha256"]:
                raise ValueError("another operation owns the marker")
            verify_plan(previous)
        except (OSError, IndexError, KeyError, TypeError, ValueError) as error:
            raise Refusal(f"marker changed: {error}; changes: {path}") from error
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


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def node(path, data=None):
    if not path.exists() and not path.is_symlink():
        return {"kind": "absent"}
    mode = stat.S_IMODE(path.lstat().st_mode)
    if path.is_symlink():
        data = os.fsencode(os.readlink(path)) if data is None else data
        kind = "symlink"
    elif path.is_dir():
        return {"kind": "directory", "mode": mode}
    elif path.is_file():
        data = path.read_bytes() if data is None else data
        kind = "file"
    else:
        raise Refusal(f"unsupported file type: {path}")
    return {"kind": kind, "mode": mode, "sha256": hashlib.sha256(data).hexdigest()}


def path_manifest(roots, excluded, *, locate=Path, save=None, translate=lambda p, b: b):
    """Full manifests include empty directories, modes, untracked and ignored entries."""
    result = {}

    def visit(original):
        if str(original) in excluded:
            return
        path = locate(original)
        value = node(path)
        if value["kind"] in ("file", "symlink"):
            data = (os.fsencode(os.readlink(path)) if path.is_symlink() else path.read_bytes())
            data = translate(original, data)
            value = node(path, data)
            if save:
                save(value["sha256"], data)
        result[str(original)] = value
        if value["kind"] == "directory":
            for child in sorted(path.iterdir()):
                visit(original / child.name)

    for root in roots:
        visit(Path(root))
    return result


def canonical_arguments(args):
    paths = {"repo", "dest", "worktree", "store", "tracker", "source", "aside", "archives", "branches"}
    return {key: str(output_location(value) if key in ("dest", "aside") else physical(value)) if key in paths and value else value
            for key, value in vars(args).items() if key not in ("hooks", "self_test")}


def owning_checkout(path):
    return physical(git("rev-parse", "--show-toplevel", cwd=path).strip())


def archive_destinations(args):
    if not getattr(args, "archives", None):
        return []
    return [physical(entry["checkout"]) for entry in read_manifest(physical(args.archives)).values()
            if Path(entry["checkout"]).is_dir()]


def participants(args):
    repositories = [args.repo] if hasattr(args, "repo") else [owning_checkout(args.store)]
    for checkout in archive_destinations(args):
        top = git("rev-parse", "--show-toplevel", cwd=checkout, check=False).strip()
        if top:
            repositories.append(physical(top))
    for key in ("tracker", "source", "store"):
        value = getattr(args, key, None)
        if value and Path(value).is_dir():
            top = git("rev-parse", "--show-toplevel", cwd=value, check=False).strip()
            if top:
                repositories.append(physical(top))
    found = {}
    for repository in repositories:
        for checkout in checkouts(repository):
            path = checkout["path"]
            found[str(path)] = {
                "path": str(path), "common": str(common_dir(path)),
                "gitdir": git("rev-parse", "--absolute-git-dir", cwd=path).strip(),
                "head": git("rev-parse", "HEAD", cwd=path).strip(),
                "branch": git("symbolic-ref", "-q", "HEAD", cwd=path, check=False).strip(),
                "index_entries": git("ls-files", "--stage", "-z", cwd=path),
            }
    return list(found.values())


def registrations(records):
    return {common: [line for line in git("worktree", "list", "--porcelain", cwd=next(
                r["path"] for r in records if r["common"] == common)).splitlines()
                     if not line.startswith("HEAD ")]
            for common in sorted({r["common"] for r in records})}


def repository_state(records, locate=Path, save=None):
    refs, indices = {}, {}
    for record in records:
        original = Path(record["path"])
        repo = locate(original)
        refs[record["common"]] = dict(line.split(" ", 1) for line in
            git("for-each-ref", "--format=%(refname) %(objectname)", cwd=repo).splitlines())
        index = locate(Path(record["gitdir"]) / "index")
        indices[str(Path(record["gitdir"]) / "index")] = node(index)
        if save and index.is_file():
            save(sha256(index), index.read_bytes())
    return refs, indices


def fresh_preconditions(args):
    """No pathname, partial edit or completed commit constitutes interruption evidence."""
    command = args.command
    if command == "code-side":
        repo = primary_checkout(args.repo)
        require_branch(repo, args.main)
        edited = {".gitignore", ".gitattributes", BUDGET, REFERENCES}
        changes = changed_paths(repo, "--cached", "HEAD") | (changed_paths(repo) & edited)
        if changes:
            raise Refusal(f"the checkout holds changes: {sorted(changes)}")
        if not worktree_listing(repo, "--", STORE):
            raise Refusal(f"the index holds no path under {STORE}")
    if command in ("mount", "rollback"):
        repo = primary_checkout(args.repo)
        aside = output_location(args.aside)
        if aside.exists() or aside.is_symlink():
            raise Refusal(f"{aside} exists without a matching interruption marker; changes: {aside}")
        store = repo / STORE
        if store.is_dir() and (not aside.parent.is_dir() or not same_filesystem(store.parent, aside.parent)):
            raise Refusal(f"{aside} must lie in an existing directory on the filesystem of {store}; changes: {aside}")
        if command == "mount":
            stage = repo / f"{STORE}.mount-stage"
            if stage.exists() or stage.is_symlink():
                raise Refusal(f"unowned staging location; changes: {stage}")
        else:
            if tracked_under("HEAD", STORE, repo):
                raise Refusal(f"HEAD already tracks {STORE}; no marked rollback to resume")
            for checkout in [*(c["path"] for c in checkouts(repo)), *archive_destinations(args)]:
                stage = checkout / f"{STORE}.rollback-stage"
                if stage.exists() or stage.is_symlink():
                    raise Refusal(f"unowned staging location; changes: {stage}")
    if command == "archive":
        dest = output_location(args.dest)
        if dest.is_symlink() or (dest.exists() and (not dest.is_dir() or any(dest.iterdir()))):
            raise Refusal(f"{dest} exists and holds a written archive set or other files; changes: {dest}")
    if command == "extract":
        dest = output_location(args.dest)
        stages = list(dest.parent.glob(f".{dest.name}.extract-*"))
        if dest.exists() or dest.is_symlink() or stages:
            raise Refusal(f"destination or unowned staging location exists; changes: {[str(dest), *map(str, stages)]}")
    if hasattr(args, "repo"):
        for checkout in checkouts(args.repo):
            for name in ("MERGE_HEAD", "REVERT_HEAD", "CHERRY_PICK_HEAD", "sequencer"):
                path = Path(git("rev-parse", "--path-format=absolute", "--git-path", name,
                                cwd=checkout["path"]).strip())
                if path.exists():
                    raise Refusal(f"an operation is in progress; changes: {path}")


def publication_roots(args, records):
    roots = [Path(r["path"]) for r in records] + archive_destinations(args)
    for key in ("dest", "aside", "archives", "store", "source", "tracker"):
        value = getattr(args, key, None)
        if value:
            path = output_location(value) if key in ("dest", "aside") else physical(value)
            while not path.parent.exists():
                path = path.parent
            roots.append(path)
    if args.command == "extract":
        dest = output_location(args.dest)
        roots.append(dest.with_name(f".{dest.name}.extract-{os.getpid()}"))
    return sorted({str(p) for p in roots if not any(p != q and q in p.parents for q in roots)})


def translate_file(path, data, replace):
    if path.name in (".git", "gitdir", "commondir", "config", "manifest.json"):
        return replace(data)
    return data


def make_plan(args, records, roots, directory):
    """Run the existing algorithms only in an exclusive replica; archive every checkpoint.

    The replica includes complete stores, not just Git's inventory. Neither a failed
    merge nor cleanup of a replica can affect a caller's index, refs or local files.
    """
    global PLANNING
    replica = directory / "replica"
    replica.mkdir()
    locate = lambda p: replica / str(p).lstrip("/")
    excluded = {str(Path(r["path"]) / ".git") for r in records}
    registered = registrations(records)
    payload = directory / "payload.tar"
    saved = set()
    checkpoints = []
    new_repositories = {}
    pending_interrupts = set()
    original_hooks = args.hooks
    parameters = canonical_arguments(args)
    mapped = argparse.Namespace(**parameters)
    path_keys = ("repo", "dest", "worktree", "store", "tracker", "source", "aside", "archives", "branches")
    for key in path_keys:
        value = getattr(mapped, key, None)
        if value:
            setattr(mapped, key, str(locate(value)))
    replacements = sorted(set(roots) | {str(Path(r["path"])) for r in records} |
                          {str(Path(r["common"])) for r in records} |
                          {parameters[k] for k in path_keys if parameters.get(k)}, key=len, reverse=True)
    pattern = re.compile(b"|".join(re.escape(os.fsencode(p)) for p in replacements))
    incoming = lambda b: pattern.sub(lambda m: os.fsencode(locate(os.fsdecode(m[0]))), b)
    outgoing = lambda b: b.replace(os.fsencode(str(replica)), b"")
    with tarfile.open(payload, "x") as tar:
        def save(digest, data):
            if digest not in saved:
                info = tarfile.TarInfo(digest)
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
                saved.add(digest)

        initial_paths = path_manifest(roots, excluded, save=save)
        initial_refs, initial_indices = repository_state(records, save=save)
        for participant in records:
            if (participant["branch"] and initial_refs[participant["common"]].get(participant["branch"]) != participant["head"]
                    or git("ls-files", "--stage", "-z", cwd=participant["path"]) != participant["index_entries"]):
                raise Refusal(f"repository changed during planning; changes: {participant['path']} HEAD/index")
        for root in roots:
            source, target = Path(root), locate(root)
            target.parent.mkdir(parents=True, exist_ok=True)
            if source.exists() or source.is_symlink():
                copy_entry(source, target)
        # A branches file can lie outside every checkout. It is input, never output.
        if parameters.get("branches") and not locate(parameters["branches"]).exists():
            target = locate(parameters["branches"])
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(parameters["branches"], target)
        for common in sorted({r["common"] for r in records}):
            target = locate(common)
            if not target.exists():
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copytree(common, target, symlinks=True)
            for path in target.rglob("*"):
                if path.is_file() and not path.is_symlink() and path.name in ("gitdir", "commondir", "config"):
                    path.write_bytes(incoming(path.read_bytes()))
        for record in records:
            pointer = locate(record["path"]) / ".git"
            if pointer.is_symlink():
                pointer.unlink()
                pointer.write_text(f"gitdir: {locate(record['gitdir'])}\n")
            elif pointer.is_file():
                pointer.write_bytes(incoming(pointer.read_bytes()))
        # Archive manifests name their original checkouts, including on rollback.
        for root in roots:
            for path in locate(root).rglob("manifest.json") if locate(root).is_dir() else []:
                path.write_bytes(incoming(path.read_bytes()))

        def capture(name, *, hook=True):
            if hook:
                try:
                    original_hooks.get("step", lambda _: None)(name)
                except Interrupted:
                    pending_interrupts.add(len(checkpoints))
            paths = path_manifest(roots, excluded, locate=locate, save=save,
                                  translate=lambda p, b: translate_file(p, b, outgoing))
            refs, indices = repository_state(records, locate, save)
            for path, value in paths.items():
                if Path(path).name == ".git" and value["kind"] == "directory":
                    checkout = Path(path).parent
                    if str(checkout) not in new_repositories and own_repository(locate(checkout)):
                        new_repositories[str(checkout)] = {
                            "path": str(checkout), "starting_head": None,
                            "head": git("rev-parse", "HEAD", cwd=locate(checkout)).strip(),
                            "branch": git("symbolic-ref", "-q", "HEAD", cwd=locate(checkout), check=False).strip(),
                            "index_entries": git("ls-files", "--stage", "-z", cwd=locate(checkout)),
                            "first_step": name,
                        }
            checkpoints.append({"name": name, "paths": paths, "refs": refs, "indices": indices})

        mapped.hooks = {"step": capture}
        if "after_clean" in original_hooks:
            mapped.hooks["after_clean"] = original_hooks["after_clean"]
        output = io.StringIO()
        previous_temp = tempfile.tempdir
        work = directory / "work"
        work.mkdir()
        try:
            PLANNING = directory
            tempfile.tempdir = str(work)
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
                result = COMMANDS[args.command](mapped) or 0
            capture("verified", hook=False)
        except (Refusal, Failure, OSError) as error:
            print(outgoing(output.getvalue().encode()).decode(), end="")
            raise type(error)(outgoing(str(error).encode()).decode()) from error
        finally:
            PLANNING = None
            tempfile.tempdir = previous_temp
        # Preserve the objects of every tool-made commit before publishing any ref.
        objects = []
        for common in sorted({r["common"] for r in records}):
            original = Path(common) / "objects"
            planned = locate(original)
            for path in sorted(planned.rglob("*")):
                if path.is_file() and not path.is_symlink() and not (original / path.relative_to(planned)).exists():
                    data = path.read_bytes()
                    digest = hashlib.sha256(data).hexdigest()
                    save(digest, data)
                    objects.append({"path": str(original / path.relative_to(planned)),
                                    "after": node(path, data)})
    actions = []
    paths, refs, indices = initial_paths.copy(), initial_refs.copy(), initial_indices.copy()
    for item in objects:
        actions.append({"kind": "object", "before": {"kind": "absent"}, **item})
    for checkpoint in checkpoints:
        desired = checkpoint["paths"]
        # Files disappear before directories; new parents precede their children.
        removals = [p for p, old in paths.items() if old["kind"] != "absent" and
                    (p not in desired or desired[p]["kind"] != old["kind"])]
        for path in sorted(removals, key=lambda p: (-len(Path(p).parts), p)):
            actions.append({"kind": "path", "path": path, "before": paths[path], "after": {"kind": "absent"}})
            paths[path] = {"kind": "absent"}
        for path in sorted(desired, key=lambda p: (len(Path(p).parts), p)):
            after, before = desired[path], paths.get(path, {"kind": "absent"})
            if before != after:
                actions.append({"kind": "path", "path": path, "before": before, "after": after})
                paths[path] = after
        # Explicit absences matter only at roots; absent descendants are not entries.
        paths = desired.copy()
        for path, after in checkpoint["indices"].items():
            if indices[path] != after:
                actions.append({"kind": "index", "path": path, "before": indices[path], "after": after})
                indices[path] = after
        for common, desired_refs in checkpoint["refs"].items():
            for ref in sorted(set(refs[common]) | set(desired_refs)):
                before, after = refs[common].get(ref), desired_refs.get(ref)
                if before != after:
                    actions.append({"kind": "ref", "common": common, "ref": ref, "before": before, "after": after,
                                    "resulting_sha": after})
            refs[common] = desired_refs.copy()
        actions.append({"kind": "checkpoint", "name": checkpoint["name"], "number": len([
            a for a in actions if a["kind"] == "checkpoint"])})
    for action in actions:
        if action["kind"] == "path":
            for repository in new_repositories.values():
                if action["path"] == str(Path(repository["path"]) / ".git" / repository["branch"]) and action["after"]["kind"] == "file":
                    action["resulting_sha"] = repository["head"]
    for number, action in enumerate(actions):
        if action["kind"] == "index":
            token = f"{directory.name}:{number}\n"
            action["lock"] = {"path": action["path"] + ".lock", "before": {"kind": "absent"},
                              "token": token, "identity": {"kind": "file", "mode": 0o600,
                              "sha256": hashlib.sha256(token.encode()).hexdigest()}}
        if action["kind"] in ("path", "index", "object") and action["after"]["kind"] in ("file", "symlink"):
            action["stage"] = str(Path(action["path"]).with_name(f".cutover-{directory.name}-{number}"))
            if Path(action["stage"]).exists() or Path(action["stage"]).is_symlink():
                raise Refusal(f"unowned publication stage; changes: {action['stage']}")
    # Only changed bytes survive planning. Full manifests retain identity for
    # unchanged paths; the archive retains every byte deletion/restoration needs.
    required = {value["sha256"] for action in actions for value in (action.get("before"), action.get("after"))
                if isinstance(value, dict) and "sha256" in value}
    compact = directory / "payload-compact.tar"
    with tarfile.open(payload) as source, tarfile.open(compact, "x") as target:
        for digest in sorted(required):
            data = source.extractfile(digest).read()
            if hashlib.sha256(data).hexdigest() != digest:
                raise Failure("planning archive member does not verify")
            member = tarfile.TarInfo(digest)
            member.size = len(data)
            target.addfile(member, io.BytesIO(data))
    os.replace(compact, payload)
    shutil.rmtree(replica)
    shutil.rmtree(work)
    return {
        "version": 1, "id": directory.name, "arguments": parameters,
        "registrations": registered, "participants": records, "new_repositories": list(new_repositories.values()), "roots": roots,
        "starting_manifests": {root: {str(Path(p).relative_to(root)): value for p, value in initial_paths.items()
                                     if p == root or Path(root) in Path(p).parents} for root in roots},
        "scratch_manifest": path_manifest([str(directory)], set()), "excluded": sorted(excluded),
        "starting_paths": initial_paths, "starting_refs": initial_refs, "starting_indices": initial_indices,
        "paths": initial_paths.copy(), "refs": copy.deepcopy(initial_refs), "indices": initial_indices.copy(),
        "payload": str(payload), "payload_sha256": sha256(payload), "directory": str(directory),
        "actions": actions, "completed": [], "intent": None, "ref_replays": [], "ref_intent": None, "result": result,
        "output": outgoing(output.getvalue().encode()).decode(),
    }, pending_interrupts


PLAN_FIELDS = ("version", "id", "arguments", "participants", "roots", "excluded",
               "starting_paths", "starting_refs", "starting_indices", "payload", "payload_sha256",
               "directory", "registrations", "scratch_manifest", "new_repositories", "starting_manifests", "actions", "output", "result")


def plan_digest(record):
    return hashlib.sha256(json.dumps({key: record[key] for key in PLAN_FIELDS},
                                     sort_keys=True).encode()).hexdigest()


def verify_plan(record):
    if record["plan_sha256"] != plan_digest(record):
        raise ValueError("immutable plan digest differs")
    paths = record["starting_paths"].copy()
    refs = copy.deepcopy(record["starting_refs"])
    indices = record["starting_indices"].copy()
    for number, completed in enumerate(record["completed"]):
        action = record["actions"][number]
        expected = {"step": number}
        if "resulting_sha" in action:
            expected["resulting_sha"] = action["resulting_sha"]
        if action["kind"] == "ref":
            if action["after"] is None:
                refs[action["common"]].pop(action["ref"], None)
            else:
                refs[action["common"]][action["ref"]] = action["after"]
        elif action["kind"] in ("path", "index"):
            target = paths if action["kind"] == "path" else indices
            target[action["path"]] = action["after"]
        if completed != expected:
            raise ValueError("completed steps are not a contiguous prefix of the plan")
    for replay in [*record["ref_replays"], *([record["ref_intent"]] if record["ref_intent"] else [])]:
        common, ref = replay["common"], replay["ref"]
        count = replay["completed_count"]
        if not 0 <= count <= len(record["completed"]):
            raise ValueError("invalid ref replay position")
        previous = record["starting_refs"][common].get(ref)
        last = previous
        for action in record["actions"][:count]:
            if action["kind"] == "ref" and action["common"] == common and action["ref"] == ref:
                last = action["after"]
        if replay != {"common": common, "ref": ref, "before": previous, "after": last,
                      "resulting_sha": last, "completed_count": count} or last == previous:
            raise ValueError("ref replay does not name a recorded tool commit")
    if record["intent"] not in (None, len(record["completed"])):
        raise ValueError("intent is not the next planned step")
    if (paths, refs, indices) != (record["paths"], record["refs"], record["indices"]):
        raise ValueError("completed state differs from the recorded steps")


def changed_description(record, differences):
    lines = list(differences)
    for participant in record.get("participants", []):
        repo = participant["path"]
        status = git("status", "--porcelain", "--ignored", "--untracked-files=all",
                     cwd=repo, check=False).strip()
        if status:
            lines.append(f"{repo}: {status}")
    return "changes: " + "; ".join(lines)


def validate_record(record):
    """Allow only the last completed state or either side of the recorded intent."""
    differences = []
    scratch = path_manifest([record["directory"]], set())
    if scratch != record["scratch_manifest"]:
        differences += [p for p in sorted(set(scratch) | set(record["scratch_manifest"]))
                        if scratch.get(p) != record["scratch_manifest"].get(p)]
    intent = record["intent"]
    action = record["actions"][intent] if intent is not None else None
    actual = path_manifest(record["roots"], set(record["excluded"]))
    expected = {p: v for p, v in record["paths"].items() if v["kind"] != "absent" or p in record["roots"]}
    ignored = set()
    if action and action["kind"] == "path":
        path = action["path"]
        if actual.get(path, {"kind": "absent"}) not in (action["before"], action["after"]):
            differences.append(path)
        ignored.add(path)
    for planned in record["actions"]:
        if planned.get("stage") and (not action or planned.get("stage") != action.get("stage")):
            if node(Path(planned["stage"])) != {"kind": "absent"}:
                differences.append(planned["stage"])
    if action and action.get("stage"):
        stage = action["stage"]
        value = node(Path(stage))
        if value not in ({"kind": "absent"}, action["after"]):
            differences.append(stage)
        ignored.add(stage)
    differences += [p for p in sorted(set(actual) | set(expected)) if p not in ignored and
                    actual.get(p, {"kind": "absent"}) != expected.get(p, {"kind": "absent"})]
    for participant in record["participants"]:
        repo = participant["path"]
        if (str(common_dir(repo)) != participant["common"] or
                git("rev-parse", "--absolute-git-dir", cwd=repo).strip() != participant["gitdir"] or
                git("symbolic-ref", "-q", "HEAD", cwd=repo, check=False).strip() != participant["branch"]):
            differences.append(f"repository identity/branch {repo}")
        lock = Path(participant["gitdir"]) / "index.lock"
        lock_state = node(lock)
        owned_lock = action.get("lock") if action and action["kind"] == "index" else None
        if lock_state != {"kind": "absent"} and not (
                owned_lock and owned_lock["path"] == str(lock) and lock_state == owned_lock["identity"]):
            differences.append(str(lock))
        for name in ("MERGE_HEAD", "REVERT_HEAD", "CHERRY_PICK_HEAD", "sequencer"):
            if Path(git("rev-parse", "--path-format=absolute", "--git-path", name, cwd=repo).strip()).exists():
                differences.append(f"{repo}: unexpected {name}")
    if registrations(record["participants"]) != record["registrations"]:
        differences.append("worktree registrations")
    actual_refs, actual_indices = repository_state(record["participants"])
    for common, expected_refs in record["refs"].items():
        for ref in set(expected_refs) | set(actual_refs[common]):
            # A reset to the immutable starting tip is also resumable. Reapply
            # the recorded tool tip with a separate durable compare-and-swap.
            allowed = [expected_refs.get(ref), record["starting_refs"][common].get(ref)]
            if action and action["kind"] == "ref" and action["common"] == common and action["ref"] == ref:
                allowed += [action["before"], action["after"]]
            if actual_refs[common].get(ref) not in allowed:
                differences.append(f"ref {ref} in {common}")
    for path, expected_index in record["indices"].items():
        allowed = [expected_index]
        if action and action["kind"] == "index" and action["path"] == path:
            allowed += [action["before"], action["after"]]
        if actual_indices[path] not in allowed:
            differences.append(f"index {path}")
    # HEAD is immutable for detached participants, and follows the validated ref otherwise.
    for participant in record["participants"]:
        if not participant["branch"] and git("rev-parse", "HEAD", cwd=participant["path"]).strip() != participant["head"]:
            differences.append(f"HEAD {participant['path']}")
    if differences:
        raise Refusal(changed_description(record, differences))


def validate_action(record, action):
    kind = action["kind"]
    if kind in ("path", "index", "object"):
        path = Path(action["path"])
        if node(path) not in (action["before"], action["after"]):
            raise Refusal(changed_description(record, [str(path)]))
        # Refuse a symlink inserted into an ancestor of a publication target.
        for parent in path.parents:
            if parent.is_symlink():
                raise Refusal(changed_description(record, [str(parent)]))
        if action.get("stage") and node(Path(action["stage"])) not in ({"kind": "absent"}, action["after"]):
            raise Refusal(changed_description(record, [action["stage"]]))
    elif kind == "checkpoint":
        validate_record(record)


def payload_bytes(tar, value):
    data = tar.extractfile(value["sha256"]).read()
    if hashlib.sha256(data).hexdigest() != value["sha256"]:
        raise Refusal("the recorded payload member does not verify")
    return data


def publish_file(action, tar):
    """The caller durably records intent and validates both names before replacement."""
    path, after = Path(action["path"]), action["after"]
    if node(path) == after:
        if action.get("stage") and Path(action["stage"]).is_symlink():
            Path(action["stage"]).unlink()
        elif action.get("stage") and Path(action["stage"]).exists():
            Path(action["stage"]).unlink()
        return
    kind = after["kind"]
    if kind == "absent":
        if path.is_dir() and not path.is_symlink():
            path.rmdir()
        else:
            path.unlink()
    elif kind == "directory":
        if not path.exists():
            path.mkdir()
        path.chmod(after["mode"])
    else:
        stage = Path(action["stage"])
        if not stage.exists() and not stage.is_symlink():
            data = payload_bytes(tar, after)
            if kind == "symlink":
                stage.symlink_to(os.fsdecode(data))
            else:
                with stage.open("xb") as handle:
                    handle.write(data)
                stage.chmod(after["mode"])
        if node(stage) != after:
            raise Refusal(f"publication stage differs; changes: {stage}")
        os.replace(stage, path)


def compare_and_swap(common, ref, before, after):
    command = ["git", "--git-dir=" + common, "update-ref"]
    command += (["-d", ref, before] if after is None else [ref, after, before or "0" * 40])
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise Failure(result.stderr.strip())


def reapply_recorded_refs(marker, record):
    for common, refs in record["refs"].items():
        actual = dict(line.split(" ", 1) for line in git("--git-dir=" + common, "for-each-ref",
                      "--format=%(refname) %(objectname)", cwd=Path(common).parent).splitlines())
        for ref in sorted(set(refs) | set(actual)):
            expected, current = refs.get(ref), actual.get(ref)
            starting = record["starting_refs"][common].get(ref)
            pending = record["ref_intent"]
            if current == expected:
                if pending and (pending["common"], pending["ref"]) == (common, ref):
                    record["ref_replays"].append(pending)
                    record["ref_intent"] = None
                    atomic_marker(marker, record)
                continue
            if current != starting:
                # The next planned ref action may already have executed.
                continue
            replay = {"common": common, "ref": ref, "before": starting, "after": expected,
                      "resulting_sha": expected, "completed_count": len(record["completed"])}
            record["ref_intent"] = replay
            atomic_marker(marker, record)
            compare_and_swap(common, ref, starting, expected)
            record["ref_replays"].append(replay)
            record["ref_intent"] = None
            atomic_marker(marker, record)


def acquire_index_lock(action):
    owned = action["lock"]
    path = Path(owned["path"])
    if not path.exists() and not path.is_symlink():
        try:
            with path.open("xb") as handle:
                handle.write(owned["token"].encode())
            path.chmod(0o600)
        except FileExistsError as error:
            raise Refusal(f"another Git writer holds the index; changes: {path}") from error
    if node(path) != owned["identity"]:
        raise Refusal(f"another Git writer holds the index; changes: {path}")


def publish(args, marker, record, pending_interrupts=None):
    verify_plan(record)
    validate_record(record)
    payload = Path(record["payload"])
    if not payload.is_file() or sha256(payload) != record["payload_sha256"]:
        raise Refusal(f"recorded archive does not verify; changes: {payload}")
    reapply_recorded_refs(marker, record)
    with tarfile.open(payload) as tar:
        while len(record["completed"]) < len(record["actions"]):
            number = len(record["completed"])
            action = record["actions"][number]
            record["intent"] = number
            atomic_marker(marker, record)
            validate_action(record, action)
            args.hooks.get("publish", lambda _: None)(f"before {number} {action['kind']}")
            kind = action["kind"]
            if kind == "object":
                path = Path(action["path"])
                # Object addition never replaces an existing object or pack.
                if not path.exists():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    publish_file(action, tar)
            elif kind in ("path", "index"):
                if kind == "index":
                    acquire_index_lock(action)
                    args.hooks.get("publish", lambda _: None)(f"index-locked {number} {kind}")
                publish_file(action, tar)
                target = record["paths"] if kind == "path" else record["indices"]
                target[action["path"]] = action["after"]
            elif kind == "ref":
                current = git("--git-dir=" + action["common"], "rev-parse", "-q", "--verify", action["ref"],
                              cwd=Path(action["common"]).parent, check=False).strip() or None
                if current != action["after"]:
                    compare_and_swap(action["common"], action["ref"], action["before"], action["after"])
                if action["after"] is None:
                    record["refs"][action["common"]].pop(action["ref"], None)
                else:
                    record["refs"][action["common"]][action["ref"]] = action["after"]
            if kind in ("path", "index", "object") and node(Path(action["path"])) != action["after"]:
                raise Failure(f"publication does not verify: {action['path']}")
            if kind == "ref":
                tip = git("--git-dir=" + action["common"], "rev-parse", "-q", "--verify", action["ref"],
                          cwd=Path(action["common"]).parent, check=False).strip() or None
                if tip != action["after"]:
                    raise Failure(f"published ref does not verify: {action['ref']}")
            if kind == "index":
                args.hooks.get("publish", lambda _: None)(f"index-written {number} {kind}")
                lock = action["lock"]
                if node(Path(lock["path"])) != lock["identity"]:
                    raise Refusal(f"index lock changed; changes: {lock['path']}")
                Path(lock["path"]).unlink()
            args.hooks.get("publish", lambda _: None)(f"executed {number} {kind}")
            record["completed"].append({"step": number, **({"resulting_sha": action["resulting_sha"]} if "resulting_sha" in action else {})})
            record["intent"] = None
            atomic_marker(marker, record)
            args.hooks.get("publish", lambda _: None)(f"after {number} {kind}")
            if kind == "checkpoint":
                validate_record(record)
                if pending_interrupts is None:
                    args.hooks.get("step", lambda _: None)(action["name"])
                elif action["number"] in pending_interrupts:
                    raise Interrupted(action["name"])
        validate_record(record)
    print(record["output"], end="")
    marker.unlink()
    sync_directory(marker.parent)
    # Payloads and replicas are exclusively allocated scratch; never caller paths.
    shutil.rmtree(record["directory"])
    return record["result"]


def transactional(args):
    coordinator = (args.worktree if args.command == "clean" and args.worktree else
                   args.repo if hasattr(args, "repo") else owning_checkout(args.store))
    marker = state_path(coordinator)
    fd = os.open(marker.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        # Lock the directory inode: no stale lock file and no marker overwrite race.
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Refusal(f"another cutover owns {marker}") from error
        if marker.exists() or marker.is_symlink():
            try:
                if marker.is_symlink():
                    raise ValueError("marker is a symlink")
                record = json.loads(marker.read_text())
                if record["version"] != 1 or record["arguments"] != canonical_arguments(args):
                    raise ValueError("operation or canonical arguments differ")
                return publish(args, marker, record)
            except (IndexError, KeyError, TypeError, ValueError, OSError, tarfile.TarError) as error:
                raise Refusal(f"invalid interruption marker {marker}: {error}; changes: marker retained") from error
        fresh_preconditions(args)
        records = participants(args)
        for participant in records:
            lock = Path(participant["gitdir"]) / "index.lock"
            if lock.exists() or lock.is_symlink():
                raise Refusal(f"another Git writer holds the index; changes: {lock}")
        roots = publication_roots(args, records)
        directory = Path(tempfile.mkdtemp(prefix="cutover-operation-"))
        retained = False
        try:
            # Scratch never lives inside a participating checkout or archive input.
            if any(inside(directory, root) for root in roots):
                raise Refusal("TMPDIR places cutover planning inside an input or output tree")
            record, interruptions = make_plan(args, records, roots, directory)
            record["plan_sha256"] = plan_digest(record)
            validate_record(record)
            if record["result"]:
                print(record["output"], end="")
                return record["result"]
            atomic_marker(marker, record, create=True)
            retained = True
            return publish(args, marker, record, interruptions)
        finally:
            if not retained:
                shutil.rmtree(directory)
    finally:
        os.close(fd)


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
        self.jit = shutil.which("jit")

    def check(self, condition, label):
        if not condition:
            self.failures.append(label)
            print(f"  FAIL {label}")
        return condition

    def run(self, *argv, after_clean=None, hooks=None):
        for arg in argv:
            if arg.startswith("/"):
                if inside(Path(arg), self.invoking):
                    raise Refusal(f"self-test argument inside the invoking checkout: {arg}")
        hooks = dict(hooks or {})
        if after_clean:
            hooks["after_clean"] = after_clean
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            try:
                code = main(list(argv), hooks=hooks)
            except Interrupted:
                code = "interrupted"
        if code == 0 and argv[0] in ("archive", "extract", "code-side", "mount", "harvest", "clean", "rollback") and "--verify" not in argv:
            self.check(not self.marker_for(argv).exists(), f"SAFE marker-removed {argv[0]}")
        return code, out.getvalue()

    def preservation_state(self, code):
        return (store_files(Path(code)),
                (Path(git("rev-parse", "--absolute-git-dir", cwd=code).strip()) / "index").read_bytes(),
                git("show-ref", cwd=code))

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

    def interrupt_each(self, argv, steps, state, label):
        """Interrupts the command after each step and requires a rerun to reach `state`."""
        self.check(steps != [], f"{label}: the command reports its steps")
        for number, step in enumerate(steps):
            self.restore()
            seen = -1
            def stop_occurrence(name):
                nonlocal seen
                seen += 1
                if seen == number:
                    raise Interrupted(name)
            rc, out = self.run(*argv, hooks={"step": stop_occurrence})
            self.check(rc == "interrupted", f"{label}: interrupted after {step}")
            marker = self.marker_for(argv)
            self.check(marker.is_file(), f"SAFE marked-interruption {label} {step}")
            rc, out = self.run(*argv)
            self.check(rc == 0 and not marker.exists() and self.state_of(argv) == state,
                       f"{label}: a rerun after an interruption at {step} reaches the uninterrupted end state")
        rc, out = self.run(*argv)
        self.check(rc in (0, 2) and self.state_of(argv) == state, f"{label}: a rerun of the finished command changes nothing")

    def marker_for(self, argv):
        if argv[0] == "harvest":
            repo = owning_checkout(argv[argv.index("--store") + 1])
        elif argv[0] == "clean" and "--worktree" in argv:
            repo = argv[argv.index("--worktree") + 1]
        else:
            repo = argv[argv.index("--repo") + 1]
        return state_path(repo)

    def guard_probes(self, argv, steps, label):
        """Probe an owned marker with foreign HEADs, extra files and bad stage bytes."""
        for probe in ("stale-head", "untracked-drift", "ignored-drift", "mode-drift", "scratch-drift", "malformed-marker", "arguments", "missing-marker"):
            self.restore()
            rc, out = self.run(*argv, hooks={"step": interrupt_at(steps[0])})
            marker = self.marker_for(argv)
            self.check(rc == "interrupted" and marker.is_file(), f"SAFE {probe} setup {label}")
            if not marker.is_file():
                continue
            record = json.loads(marker.read_text())
            if probe == "stale-head":
                participant = next(r for r in record["participants"] if r["branch"])
                repo = participant["path"]
                tree = git("rev-parse", "HEAD^{tree}", cwd=repo).strip()
                parent = git("rev-parse", "HEAD", cwd=repo).strip()
                commit = self.sh("git", "commit-tree", tree, "-p", parent, "-m", "foreign head", cwd=repo).stdout.strip()
                self.sh("git", "update-ref", participant["branch"], commit, parent, cwd=repo)
            elif probe == "mode-drift":
                target = next(Path(p) for p, value in record["paths"].items()
                              if value["kind"] == "file" and Path(p).is_file())
                target.chmod(stat.S_IMODE(target.stat().st_mode) ^ stat.S_IXUSR)
            elif probe == "scratch-drift":
                (Path(record["directory"]) / "unowned-staging-file").write_text("user data\n")
            elif probe == "malformed-marker":
                damaged = copy.deepcopy(record)
                damaged["actions"][0]["kind"] = "invalid"
                marker.write_text(json.dumps(damaged))
            elif probe == "arguments":
                pass
            elif probe == "missing-marker":
                marker.unlink()
            else:
                # Prefer a real staging/aside location at this checkpoint.
                locations = [Path(p) for p, value in record["paths"].items()
                             if value["kind"] == "directory" and
                             (p.endswith(".mount-stage") or p == record["arguments"].get("aside") or
                              ".extract-" in Path(p).name or p == record["arguments"].get("dest"))]
                root = locations[0] if locations else Path(record["participants"][0]["path"]) / STORE
                root.mkdir(parents=True, exist_ok=True)
                target = root / ("gate-runs/safety-extra.json" if probe == "ignored-drift" else "safety-extra")
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("unowned content\n")
            observed = [*record["roots"], record["directory"]]
            before = path_manifest(observed, set())
            invocation = list(argv)
            if probe == "arguments":
                key = "--message" if "--message" in invocation else "--dest" if "--dest" in invocation else "--aside" if "--aside" in invocation else "--from"
                invocation[invocation.index(key) + 1] += "-other"
            rc, out = self.run(*invocation)
            # Harvest remains an idempotent operation without a marker; it performs
            # its ordinary absent-or-identical target checks on a fresh invocation.
            if probe != "missing-marker" or argv[0] not in ("harvest", "clean"):
                self.check(rc in (1, 2) and path_manifest(observed, set()) == before,
                           f"SAFE {probe} {label}: refuses without changing bytes, modes, index or refs")
            # A deliberately abandoned marker's payload belongs only to this fixture.
            if Path(record["directory"]).exists():
                shutil.rmtree(record["directory"])
        self.restore()

    def publication_probes(self, argv, state):
        for kind in ("object", "path", "index", "ref"):
            boundaries = ("before", "index-locked", "index-written", "executed", "after") if kind == "index" else ("before", "executed", "after")
            for boundary in boundaries:
                self.restore()
                def stop(label):
                    if label.startswith(boundary + " ") and label.endswith(" " + kind):
                        raise Interrupted(label)
                rc, out = self.run(*argv, hooks={"publish": stop})
                marker = self.marker_for(argv)
                self.check(rc == "interrupted" and marker.is_file(), f"SAFE publish-{boundary}-{kind}")
                if not marker.is_file():
                    continue
                record = json.loads(marker.read_text())
                if kind == "ref" and boundary == "after":
                    self.check(record["completed"][-1].get("resulting_sha") == git("rev-parse", "HEAD", cwd=record["participants"][0]["path"]).strip()
                               and record["starting_refs"] != record["refs"],
                               "SAFE completed-ref: resulting SHA recorded, starting refs immutable")
                rc, out = self.run(*argv)
                self.check(rc == 0 and self.state_of(argv) == state and not marker.exists(),
                           f"SAFE publish-{boundary}-{kind}: restart reaches end state")
        self.restore()
        def stop_ref(label):
            if label.startswith("after ") and label.endswith(" ref"):
                raise Interrupted(label)
        rc, out = self.run(*argv, hooks={"publish": stop_ref})
        marker = self.marker_for(argv)
        record = json.loads(marker.read_text())
        participant = record["participants"][0]
        current = git("rev-parse", "HEAD", cwd=participant["path"]).strip()
        self.sh("git", "update-ref", participant["branch"], participant["head"], current, cwd=participant["path"])
        rc, out = self.run(*argv)
        self.check(rc == 0 and self.state_of(argv) == state and not marker.exists(),
                   "SAFE starting-head-resume: recorded tool commit reapplied with compare-and-swap")
        self.restore()
        def stop_index(label):
            if label.startswith("before ") and label.endswith(" index"):
                raise Interrupted(label)
        rc, out = self.run(*argv, hooks={"publish": stop_index})
        marker = self.marker_for(argv)
        record = json.loads(marker.read_text())
        lock = Path(record["participants"][0]["gitdir"]) / "index.lock"
        lock.write_text("foreign index bytes\n")
        observed = [*record["roots"], record["directory"]]
        before = path_manifest(observed, set())
        rc, out = self.run(*argv)
        self.check(rc == 2 and path_manifest(observed, set()) == before,
                   "SAFE foreign-index-lock: resume preserves another writer's lock and index")
        shutil.rmtree(record["directory"])
        self.restore()
        def stop_file(label):
            if label.startswith("before "):
                record = json.loads(self.marker_for(argv).read_text())
                action = record["actions"][record["intent"]]
                if action["kind"] == "path" and action.get("stage"):
                    raise Interrupted(label)
        rc, out = self.run(*argv, hooks={"publish": stop_file})
        marker = self.marker_for(argv)
        record = json.loads(marker.read_text())
        action = record["actions"][record["intent"]]
        stage = Path(action["stage"])
        stage.write_text("foreign stage bytes\n")
        before = path_manifest([*record["roots"], record["directory"]], set())
        rc, out = self.run(*argv)
        self.check(rc == 2 and path_manifest([*record["roots"], record["directory"]], set()) == before,
                   "SAFE publication-stage-drift: refusal preserves foreign stage bytes")
        shutil.rmtree(record["directory"])
        self.restore()

    def unowned_location(self, argv, path, label):
        self.restore()
        path = Path(path)
        path.mkdir(parents=True)
        (path / "gate-runs").mkdir()
        (path / "gate-runs" / "user.json").write_text("ignored user bytes\n")
        (path / "user").write_text("untracked user bytes\n")
        code = argv[argv.index("--repo") + 1]
        before = self.preservation_state(code), store_files(path)
        rc, out = self.run(*argv)
        self.check(rc == 2 and (self.preservation_state(code), store_files(path)) == before,
                   f"SAFE unowned-location {label}: refuses and preserves ignored/untracked bytes")
        self.restore()
        absent = path.with_name(path.name + "-absent-target")
        path.symlink_to(absent)
        before = path_manifest([str(Path(code)), str(path), str(absent)], set())
        rc, out = self.run(*argv)
        self.check(rc == 2 and path_manifest([str(Path(code)), str(path), str(absent)], set()) == before,
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
                 "listing": sorted(os.listdir(code)), "revert": (code / ".git" / "REVERT_HEAD").exists()}
        for name in ("wa", "wb"):
            state[name] = files(Path(self.path(name)) / STORE)
            state[f"{name} listing"] = sorted(os.listdir(self.path(name)))
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
        self.guard_probes(argv, steps, "archive")
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
        self.guard_probes(argv, steps, "extract")
        self.unowned_location(argv, Path(new).with_name(f".{Path(new).name}.extract-{os.getpid()}"), "extract stage")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart extract final fixture")
        return new

    def code_side(self, code):
        argv = ("code-side", "--repo", code, "--message", "chore: move the tracker out")
        self.snapshot()
        for rel in (".gitignore", ".gitattributes", BUDGET, REFERENCES):
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
        argv = ("code-side", "--repo", code, "--message", "chore: move the tracker out")
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
        self.interrupt_each(argv, steps, self.state_of(argv), "REQ-04 code-side")
        state = self.state_of(argv)
        self.publication_probes(argv, state)
        self.guard_probes(argv, steps, "code-side")
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
            same_filesystem = lambda a, b: False if PLANNING is None else original_filesystem(a, b)
            before = self.preservation_state(code)
            rc, out = self.run("mount", "--repo", code, "--tracker", new, "--aside", aside)
            self.check(rc == 2 and self.preservation_state(code) == before and not Path(aside).exists(),
                       "SAFE original-filesystem: a replica cannot satisfy the caller's device precondition")
        finally:
            same_filesystem = original_filesystem
        self.restore()
        self.mount_steps = []
        rc, out = self.run("mount", "--repo", code, "--tracker", new, "--aside", aside,
                           hooks={"step": self.mount_steps.append})
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
        argv = ("mount", "--repo", code, "--tracker", new, "--aside", aside)
        self.interrupt_each(argv, self.mount_steps, self.state_of(argv), "REQ-05 mount")
        self.guard_probes(argv, self.mount_steps, "mount")
        self.unowned_location(argv, Path(code) / f"{STORE}.mount-stage", "mount stage")
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
        self.guard_probes(argv, steps, "harvest")
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
        args = ("--store", shared, "--merge-message", "chore: merge main", "--message", "chore: drop the tracker")
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
        rc, out = self.run("clean", "--repo", code, "--worktree", wb, *args,
                           hooks={"step": fail_at("store-removal")})
        self.check(rc == 1 and "restored" in out and git("rev-parse", "wb", cwd=code).strip() == start
                   and tracked_under("wb", STORE, code) != [] and (Path(wb) / STORE).is_dir()
                   and git("status", "--porcelain", "--untracked-files=no", cwd=wb) == "",
                   "REQ-07 a failing store deletion restores the branch ref and keeps the store")
        argv = ("clean", "--repo", code, "--worktree", wb, *args)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0 and "wb: cleaned; tree outside .jit equals the merge result" in out.splitlines(),
                   "REQ-07 clean passes on wb and reports tree equality outside .jit")
        self.check(git("ls-tree", "-r", "--name-only", "wb", "--", STORE, cwd=code) == ""
                   and git("ls-tree", "-r", "--name-only", "main", "--", STORE, cwd=code) == "",
                   "REQ-07 git ls-files .jit is empty on main and on wb")
        merge = git("rev-parse", "wb^", cwd=code).strip()
        parents = git("rev-list", "--parents", "-n1", merge, cwd=code).split()[1:]
        self.check(len(parents) == 2 and git_rc("merge-base", "--is-ancestor", "main", merge, cwd=code) == 0,
                   "REQ-07 wb merges main")
        removed = git("diff", "--name-only", "--diff-filter=D", "wb^", "wb", cwd=code).split()
        self.check(f"{STORE}/issues/late.json" in removed,
                   "REQ-07 the branch-only file that merged without conflict is removed in the next commit")
        self.check(not (Path(wb) / STORE).exists(), "REQ-07 wb's local store is deleted")
        backups = git("for-each-ref", "--format=%(refname)", "refs/cutover-archives", cwd=code).splitlines()
        saved = Path(self.path("retained-local-store.tar"))
        if backups:
            data = subprocess.run(["git", "cat-file", "blob", backups[-1]], cwd=code, capture_output=True, check=True).stdout
            saved.write_bytes(data)
        self.check(bool(backups) and tar_manifest(saved) == before_store,
                   "SAFE retained-clean-archive: complete pre-merge store survives marker removal")
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart linked-clean")
        self.guard_probes(argv, steps, "linked-clean")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart linked-clean final fixture")
        extra = Path(wa) / STORE / "gate-runs" / I2 / "unharvested.json"
        extra.write_bytes(b'{"run": "late"}\n')
        head = git("rev-parse", "HEAD", cwd=wa).strip()
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 2 and (Path(wa) / STORE).is_dir() and git("rev-parse", "HEAD", cwd=wa).strip() == head,
                   "REQ-07 clean refuses an unverified harvest and keeps the store")
        extra.unlink()

        def plant_mismatch(worktree):
            (Path(worktree) / "src" / "lib.rs").write_text("pub fn planted_mismatch() {}\n")
            self.commit_all(worktree, "test: plant a non-tracker tree mismatch")

        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args,
                           after_clean=plant_mismatch)
        self.check(rc == 2 and "tree outside .jit mismatch" in out
                   and f"wa restored to {head}" in out
                   and git("rev-parse", "wa", cwd=code).strip() == head
                   and not git("status", "--porcelain", "--untracked-files=no", cwd=wa).strip()
                   and (Path(wa) / STORE).is_dir(),
                   "REQ-07 a planted tree mismatch is refused, restores the ref and keeps the store")
        unrestored = restored(Failure("planted"), Path(wa), "wa", "0" * 40)
        self.check(isinstance(unrestored, Failure) and "wa is left at" in str(unrestored)
                   and "not restored to" in str(unrestored) and git("rev-parse", "wa", cwd=code).strip() == head,
                   "REQ-07 a failed restore names the ref it leaves")
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args)
        self.check(rc == 0 and not (Path(wa) / STORE).exists()
                   and "wa: cleaned; tree outside .jit equals the merge result" in out.splitlines()
                   and git("ls-tree", "-r", "--name-only", "wa", "--", STORE, cwd=code) == "",
                   "REQ-07 clean passes on wa")
        head = git("rev-parse", "wa", cwd=code).strip()
        local = Path(wa) / STORE
        local.mkdir()
        (local / "server.log").write_text("machine-local\n")
        self.write_runs(local, {f"{I2}/r2.json": b'{"run": "r2"}\n'})
        rc, out = self.run("clean", "--repo", code, "--worktree", wa, *args,
                           after_clean=plant_mismatch)
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
        args = ("--merge-message", "chore: merge main", "--message", "chore: drop the tracker")
        self.sh("git", "branch", "parked-already", "main", cwd=code)
        before = git("worktree", "list", "--porcelain", cwd=code)
        tmp_before = set(Path(tempfile.gettempdir()).glob("cutover-clean-*"))
        head = git("rev-parse", "parked-conflict", cwd=code).strip()
        single_head = git("rev-parse", "parked-single", cwd=code).strip()
        argv = ("clean", "--repo", code, "--branch", "parked-single", *args)
        self.snapshot()
        steps = []
        rc, out = self.run(*argv, hooks={"step": steps.append})
        print(out, end="")
        self.check(rc == 0 and out.strip() == "parked-single: cleaned; tree outside .jit equals the merge result",
                   "REQ-07 a single parked branch cleans without a shared store or harvest")
        self.check(not tracked_under("parked-single", STORE, code)
                   and git_rc("merge-base", "--is-ancestor", "main", "parked-single", cwd=code) == 0
                   and git("rev-parse", "parked-single", cwd=code).strip() != single_head,
                   "REQ-07 a parked branch merges main and removes all tracked tracker paths")
        removed = git("diff", "--name-only", "--diff-filter=D", "parked-single^", "parked-single", cwd=code)
        self.check(f"{STORE}/issues/parked.json" in removed,
                   "REQ-07 the parked branch-only tracker file is removed and committed")
        self.interrupt_each(argv, steps, self.state_of(argv), "SAFE restart parked-clean")
        self.guard_probes(argv, steps, "parked-clean")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart parked-clean final fixture")
        clean_head = git("rev-parse", "parked-already", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-already", *args)
        self.check(rc == 0 and out.strip() == "parked-already: already clean"
                   and git("rev-parse", "parked-already", cwd=code).strip() == clean_head,
                   "REQ-07 an already clean parked branch commits nothing")
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-conflict", *args)
        self.check(rc == 1 and out.strip() == "parked-conflict: conflict outside .jit: not forced: src/lib.rs"
                   and git("rev-parse", "parked-conflict", cwd=code).strip() == head,
                   "REQ-07 a parked code conflict is listed, aborted and leaves its ref unchanged")
        branches = Path(self.path("branches.txt"))
        branches.write_text("# parked branches\nparked\nparked-conflict # requires the steward\n\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        print(out, end="")
        self.check(rc == 1 and out.splitlines() == [
            "parked: cleaned; tree outside .jit equals the merge result",
            "parked-conflict: conflict outside .jit: not forced: src/lib.rs",
            "parked-already: already clean"],
            "REQ-07 batch lists each outcome and exits nonzero while a code conflict remains")
        self.check(git("rev-parse", "parked-conflict", cwd=code).strip() == head
                   and git("rev-parse", "parked-already", cwd=code).strip() == clean_head,
                   "REQ-07 a failed batch branch changes no other branch ref")
        self.check(all(not tracked_under(rev, STORE, code)
                       for rev in ("main", "parked", "parked-already"))
                   and git_rc("merge-base", "--is-ancestor", "main", "parked", cwd=code) == 0,
                   "REQ-07 main and every cleaned branch track no tracker path")
        self.check(git("worktree", "list", "--porcelain", cwd=code) == before,
                   "REQ-07 git worktree list is unchanged after the batch")
        self.check(set(Path(tempfile.gettempdir()).glob("cutover-clean-*")) == tmp_before,
                   "REQ-07 success and refusal leave no temporary worktree behind")
        branches.write_text("parked\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        self.check(rc == 0 and out.splitlines() == ["parked: already clean", "parked-already: already clean"],
                   "REQ-07 a batch of clean branches succeeds without commits")
        branches.write_text("missing-branch\nwa\nparked-already\n")
        rc, out = self.run("clean", "--repo", code, "--branches", str(branches), *args)
        lines = out.splitlines()
        self.check(rc == 1 and len(lines) == 3
                   and lines[0] == "missing-branch: refused: not a local branch: missing-branch"
                   and lines[1].startswith("wa: refused: wa is already checked out;")
                   and lines[2] == "parked-already: already clean",
                   "REQ-07 other refusals include their reasons and do not stop the batch")
        self.check(git("worktree", "list", "--porcelain", cwd=code) == before
                   and set(Path(tempfile.gettempdir()).glob("cutover-clean-*")) == tmp_before,
                   "REQ-07 other refusals leave no temporary worktree or registration")
        for name in ("parked-batch-first", "parked-batch-second"):
            self.sh("git", "branch", name, "parked-hook", cwd=code)
        batch = Path(self.path("restart-branches.txt"))
        batch.write_text("parked-batch-first\nparked-conflict\nparked-batch-second\n")
        self.snapshot()
        count = 0
        def stop_second(name):
            nonlocal count
            if name == "worktree-removal":
                count += 1
                if count == 2:
                    raise Interrupted(name)
        batch_argv = ("clean", "--repo", code, "--branches", str(batch), *args)
        rc, out = self.run(*batch_argv, hooks={"step": stop_second})
        self.check(rc == "interrupted" and self.marker_for(batch_argv).is_file(),
                   "SAFE restart parked-batch: later branch owns a marker")
        rc, out = self.run(*batch_argv)
        self.check(rc == 1 and out.splitlines() == [
            "parked-batch-first: already clean",
            "parked-conflict: conflict outside .jit: not forced: src/lib.rs",
            "parked-batch-second: cleaned; tree outside .jit equals the merge result"]
            and not self.marker_for(batch_argv).exists(),
            "SAFE restart parked-batch: prior outcomes independent, later entry resumes")
        self.restore()
        hook = Path(git("rev-parse", "--git-common-dir", cwd=code).strip())
        hook = (hook if hook.is_absolute() else Path(code) / hook) / "hooks" / "commit-msg"
        hook.parent.mkdir(exist_ok=True)
        hook.write_text('#!/bin/sh\nif grep -q REJECT "$1"; then echo rejected >&2; exit 1; fi\n')
        hook.chmod(0o755)
        failing = {"parked-hook": ("--merge-message", "chore: merge main", "--message", "chore: REJECT"),
                   "parked-empty": ("--merge-message", "chore: merge main", "--message", "")}
        for name, messages in failing.items():
            start = git("rev-parse", name, cwd=code).strip()
            rc, out = self.run("clean", "--repo", code, "--branch", name, *messages)
            self.check(rc == 1 and out.startswith(f"{name}: failed:") and "restored" in out
                       and git("rev-parse", name, cwd=code).strip() == start
                       and tracked_under(name, STORE, code) != []
                       and git("worktree", "list", "--porcelain", cwd=code) == before,
                       f"REQ-07 a removal commit that fails ({name}) restores the branch ref")
        hook.unlink()
        start = git("rev-parse", "parked-hook", cwd=code).strip()
        rc, out = self.run("clean", "--repo", code, "--branch", "parked-hook", *args,
                           hooks={"step": fail_at("worktree-removal")})
        self.check(rc == 1 and "restored" in out and git("rev-parse", "parked-hook", cwd=code).strip() == start
                   and git("worktree", "list", "--porcelain", cwd=code) == before
                   and set(Path(tempfile.gettempdir()).glob("cutover-clean-*")) == tmp_before,
                   "REQ-07 a failing temporary-worktree removal restores the ref and leaves no registration")

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
        record = json.loads(self.marker_for(argv).read_text())
        self.check(rc == "interrupted" and not (plain / STORE).exists() and str(plain) in record["roots"],
                   "SAFE external-restore-plan: an unregistered checkout is untouched during planning")
        rc, out = self.run(*argv)
        self.check(rc == 0 and self.state_of(argv) == end and (plain / "sentinel").read_text() == "user bytes\n",
                   "SAFE external-restore-resume: manifest checkout restored with caller bytes preserved")
        self.interrupt_each(argv, steps, end, "REQ-08 rollback")
        self.guard_probes(argv, steps, "rollback")
        self.unowned_location(argv, aside, "rollback aside")
        self.unowned_location(argv, Path(self.path("wa")) / f"{STORE}.rollback-stage", "rollback stage")
        self.unowned_location(argv, plain / f"{STORE}.rollback-stage", "unregistered rollback stage")
        rc, _ = self.run(*argv)
        self.check(rc == 0, "SAFE restart rollback final fixture")


class Interrupted(Exception):
    """Raised by a self-test hook to stop a subcommand between two of its steps."""


def interrupt_at(target):
    def hook(step):
        if step == target:
            raise Interrupted(step)
    return hook


def fail_at(target):
    def hook(step):
        if step == target:
            raise OSError(f"planted failure at {step}")
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
        shutil.rmtree(base, ignore_errors=True)
    if test.failures:
        print(f"cutover.py --self-test: {len(test.failures)} failure(s)", file=sys.stderr)
        return 1
    print("cutover.py --self-test: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
