#!/usr/bin/env python3
"""Refuses copied inputs, raw single-host output and oversized files in git.

Enforces `@/inv/committed-evidence-budget`. Limits and exceptions live in
`tracked-budget.toml`; limits may only fall. A JSON document the path rules
admit under `record_documents` is also read, and refused when any member name
matches `record_members`, the vocabulary of an execution record.

  check-tracked-budget.py                 check the tree at HEAD
  check-tracked-budget.py --staged        check the index and staged additions
  check-tracked-budget.py --range REVS    check each commit in a `git rev-list` range
  check-tracked-budget.py --self-test
"""

import argparse
import fnmatch
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

CONFIG = "scripts/tracked-budget.toml"
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


def git(*args, cwd=None):
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout


def read_config(ref, cwd):
    """Reads the budget files from `ref`, or from the index when `ref` is empty."""
    prefix = f"{ref}:" if ref else ":"
    config = tomllib.loads(git("show", f"{prefix}{CONFIG}", cwd=cwd))
    return config


def tree_sizes(treeish, cwd):
    sizes = {}
    for line in git("ls-tree", "-r", "-l", "-z", treeish, cwd=cwd).split("\0"):
        if not line:
            continue
        meta, path = line.split("\t", 1)
        size = meta.split()[3]
        if size != "-":
            sizes[path] = int(size)
    return sizes


def excepted(path, config):
    return any(fnmatch.fnmatch(path, e["path"]) for e in config.get("exception", []))


def path_faults(paths, config):
    forbidden = [re.compile(p) for p in config["forbidden_paths"]]
    return [
        f"forbidden path: {path}"
        for path in paths
        if any(p.search(path) for p in forbidden)
        and not excepted(path, config)
    ]


def record_faults(paths, config, read):
    """Admitted workflow documents that carry an execution record.

    `read(path)` returns the document's text at the revision being checked.
    """
    documents = re.compile(config.get("record_documents", "(?!)"))
    members = [re.compile(m) for m in config.get("record_members", [])]
    faults = []
    for path in paths:
        if not documents.search(path) or path_faults([path], config) or excepted(path, config):
            continue
        try:
            names = set(_member_names(json.loads(read(path))))
        except json.JSONDecodeError:
            faults.append(f"{path}: not JSON, so its members cannot be checked")
            continue
        found = sorted(n for n in names if any(m.fullmatch(n) for m in members))
        if found:
            faults.append(f"{path}: execution record members {', '.join(found)}")
    return faults


def _member_names(value):
    if isinstance(value, dict):
        for name, member in value.items():
            yield name
            yield from _member_names(member)
    elif isinstance(value, list):
        for member in value:
            yield from _member_names(member)


def reader(revision, cwd):
    """Reads a path's blob at `revision`, or from the index when it is empty."""
    return lambda path: git("show", f"{revision}:{path}", cwd=cwd)


def size_faults(sizes, config):
    limit = config["max_file_bytes"]
    faults = []
    for path, size in sizes.items():
        if size > limit and not excepted(path, config):
            faults.append(f"{path}: {size} bytes exceeds {limit}")
    return faults


def tree_faults(sizes, config, read):
    total = sum(sizes.values())
    faults = path_faults(sizes, config) + size_faults(sizes, config)
    faults += record_faults(sizes, config, read)
    if total > config["max_tree_bytes"]:
        faults.append(f"tree: {total} bytes exceeds {config['max_tree_bytes']}")
    return faults


def added_bytes(old, new, cwd):
    """Growth between two trees, and the new size of each added or modified path.

    `new` empty means the index. A modified file counts only its growth, so an
    append to a large log costs what it appends.
    """
    args = ["diff", "--raw", "--no-abbrev", "--no-renames", "-z"]
    args += ["--cached", old] if not new else [old, new]
    fields = git(*args, cwd=cwd).split("\0")
    changes = [
        (fields[i].split()[2], fields[i].split()[3], fields[i + 1])
        for i in range(0, len(fields) - 1, 2)
        if fields[i].split()[4] != "D"
    ]
    if not changes:
        return 0, {}
    objects = [blob for pair in changes for blob in pair[:2]]
    sizes = subprocess.run(
        ["git", "cat-file", "--batch-check=%(objectsize)"],
        cwd=cwd,
        input="\n".join(objects) + "\n",
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    # An all-zero object id prints "<id> missing": a new file has no old size.
    sizes = [0 if s.endswith("missing") else int(s) for s in sizes]
    total = sum(max(0, sizes[i + 1] - sizes[i]) for i in range(0, len(sizes), 2))
    return total, {change[2]: sizes[2 * i + 1] for i, change in enumerate(changes)}


def change_faults(old, new, config, label, cwd):
    total, _ = added_bytes(old, new, cwd)
    limit = config["max_commit_added_bytes"]
    return [f"{label}: adds {total} bytes, limit {limit}"] if total > limit else []


def ratchet_faults(config, cwd):
    """Limits may only fall against HEAD."""
    try:
        old_config = read_config("HEAD", cwd)
    except subprocess.CalledProcessError:
        return []
    faults = []
    for key in ("max_tree_bytes", "max_file_bytes", "max_commit_added_bytes"):
        if config[key] > old_config[key]:
            faults.append(f"{CONFIG}: {key} rises above HEAD's {old_config[key]}")
    return faults


def check_staged(cwd):
    config = read_config("", cwd)
    sizes = tree_sizes(git("write-tree", cwd=cwd).strip(), cwd)
    head = "HEAD" if has_head(cwd) else EMPTY_TREE
    return (
        tree_faults(sizes, config, reader("", cwd))
        + change_faults(head, "", config, "staged change", cwd)
        + ratchet_faults(config, cwd)
    )


def has_head(cwd):
    return (
        subprocess.run(
            ["git", "rev-parse", "-q", "--verify", "HEAD"],
            cwd=cwd,
            capture_output=True,
        ).returncode
        == 0
    )


def check_range(revisions, cwd):
    """Checks each commit against the budget of the range's newest commit.

    A merge counts only what its first parent lacks; a range whose newest
    commit predates the budget has nothing to check against.
    """
    faults = []
    commits = git("rev-list", "--reverse", "--no-merges", *revisions.split(), cwd=cwd).split()
    if not commits:
        return faults
    try:
        config = read_config(commits[-1], cwd)
    except subprocess.CalledProcessError:
        return faults
    for commit in commits:
        parents = git("rev-list", "--parents", "-n1", commit, cwd=cwd).split()[1:]
        parent = parents[0] if parents else EMPTY_TREE
        _, touched = added_bytes(parent, commit, cwd)
        label = commit[:9]
        faults += [f"{label}: {f}" for f in path_faults(touched, config)]
        faults += [f"{label}: {f}" for f in size_faults(touched, config)]
        faults += [f"{label}: {f}" for f in record_faults(touched, config, reader(commit, cwd))]
        faults += change_faults(parent, commit, config, label, cwd)
    return faults


def policy_faults(config):
    """The repository's own rules refuse each artifact kind and admit source."""
    refused = (
        "dev/active/x/oracle/gap.out", "experiments/a/run.stdout", "dev/x/receipt.yaml",
        "dev/active/x/receipt.toml", "dev/active/x/outcome.txt", "dev/active/x/config.json",
        "dev/active/x/execution.log", "experiments/a/results/table.md", "dev/research/x/table.csv",
        "dev/studies/x/findings.md", "dev/archive/x/plan.md", "dev/bench_results/x.md",
    )
    admitted = (
        "crates/a/src/lib.rs", "dev/active/x/handoff.md", "dev/active/x/progress.json",
        "dev/active/x/tool/Cargo.toml", "crates/a/tests/compile_fail/b.stderr",
    )
    faults = [f"admits {path}" for path in refused if not path_faults([path], config)]
    faults += [f"refuses {path}" for path in admitted if path_faults([path], config)]
    record = '{"waves": [{"ran_at": "t", "wall_seconds": 1, "binary_sha256": "a", "receipts": []}]}'
    workflow = '{"started_at": "t", "completed_at": "t", "status": "done", "waves": []}'
    if len(record_faults(["dev/active/x/progress.json"], config, lambda _: record)) != 1:
        faults.append("admits an execution record in dev/active/x/progress.json")
    if record_faults(["dev/research/x/progress.json"], config, lambda _: workflow):
        faults.append("refuses workflow state in dev/research/x/progress.json")
    return faults


def head_faults(cwd):
    config = read_config("HEAD", cwd)
    return tree_faults(tree_sizes("HEAD", cwd), config, reader("HEAD", cwd)) + policy_faults(config)


def self_test():
    config = """
max_file_bytes = 1000
max_commit_added_bytes = 2500
max_tree_bytes = 10000
forbidden_paths = ['(^|/)inputs/', '\\.log$', '(^|/)receipt\\.json$', '(^|/)(execution|journal)[^/]*\\.jsonl$', '^dev/(bench_results|benchmarks|simulation_results|archive)/']
record_documents = '^dev/(active|research)/.*\\.json$'
record_members = ['ran_at', '(\\w+_)?sha256', 'wall_seconds']
[[exception]]
path = "experiments/*/tests/fixtures/*"
reason = "test fixture"
[[exception]]
path = "big.lock"
reason = "fixture"
"""
    with tempfile.TemporaryDirectory() as raw:
        repo = raw
        run = lambda *a: git(*a, cwd=repo)
        run("init", "-q")
        run("config", "user.email", "t@example.invalid")
        run("config", "user.name", "t")

        def write(path, size, text=None):
            target = Path(repo, path)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text if text is not None else "x" * size)
            run("add", path)

        write(CONFIG, 0, config)
        write("old.bin", 900)
        write("big.lock", 4000)
        run("commit", "-qm", "base", "--no-verify")
        at_head = lambda: tree_faults(tree_sizes("HEAD", repo), read_config("HEAD", repo), reader("HEAD", repo))
        assert_faults(at_head(), [], "tree")

        cases = [
            ("large new file", lambda: write("new.bin", 1001), "new.bin"),
            ("file growth", lambda: write("old.bin", 1001), "old.bin"),
            ("copied input", lambda: write("a/inputs/b.rs", 1), "forbidden path"),
            ("raw log", lambda: write("a/run.log", 1), "forbidden path"),
            ("receipt", lambda: write("a/receipt.json", 1), "forbidden path"),
            ("execution log", lambda: write("a/execution.log", 1), "forbidden path"),
            ("execution stream", lambda: write("a/execution.jsonl", 1), "forbidden path"),
            ("journal", lambda: write("a/journal.jsonl", 1), "forbidden path"),
            ("measurement directory", lambda: write("/".join(["dev", "benchmarks", "a.csv"]), 1), "forbidden path"),
            ("run record", lambda: write("dev/active/x/progress.json", 0, '{"waves": [{"run": {"ran_at": "t"}}]}'), "execution record"),
            ("run digest", lambda: write("dev/research/x/progress.json", 0, '{"binary_sha256": "ab"}'), "execution record"),
            ("unreadable document", lambda: write("dev/active/x/progress.json", 0, "{"), "not JSON"),
            ("commit size", lambda: [write(f"f{i}", 900) for i in range(3)], "adds"),
            ("tree size", lambda: write("big.lock", 9000), "tree:"),
            ("raised limit", lambda: write(CONFIG, 0, config.replace("10000", "20000")), "rises"),
        ]
        for name, change, expected in cases:
            change()
            assert_faults(check_staged(repo), [expected], name)
            run("reset", "-q", "--hard", "HEAD")

        write("big.lock", 4100)
        assert_faults(check_staged(repo), [], "small append to an excepted large file")
        run("reset", "-q", "--hard", "HEAD")
        write("dev/active/x/progress.json", 0, '{"started_at": "t", "completed_at": "t", "status": "done", "notes": "ran_at"}')
        assert_faults(check_staged(repo), [], "workflow state")
        run("reset", "-q", "--hard", "HEAD")
        write("/".join(["experiments", "demo", "tests", "fixtures", "execution.jsonl"]), 1)
        write("/".join(["experiments", "demo", "tests", "fixtures", "receipt.json"]), 1)
        write("old.bin", 899)
        assert_faults(check_staged(repo), [], "test fixtures and shrink")
        run("commit", "-qm", "ok", "--no-verify")
        write("a/inputs/c.rs", 1)
        run("commit", "-qm", "bad", "--no-verify")
        assert_faults(check_range("HEAD~1..HEAD", repo), ["forbidden path"], "range")
        assert_faults(check_range("HEAD~2..HEAD~1", repo), [], "clean range")
        write("dev/active/x/progress.json", 0, '{"wall_seconds": 3}')
        run("commit", "-qm", "record", "--no-verify")
        assert_faults(check_range("HEAD~1..HEAD", repo), ["execution record"], "range record")
        run("rm", "-q", "dev/active/x/progress.json", "a/inputs/c.rs")
        run("commit", "-qm", "drop record", "--no-verify")
        assert_faults(at_head(), [], "tree after the record left")
        run("reset", "-q", "--hard", "HEAD~1")
        assert_faults(at_head(), ["execution record"], "tree record")
    print("check-tracked-budget: self-test passed")
    return 0


def assert_faults(faults, expected, name):
    if expected:
        ok = any(expected[0] in fault for fault in faults)
    else:
        ok = not faults
    if not ok:
        raise SystemExit(f"self-test {name}: expected {expected}, got {faults}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", action="store_true")
    parser.add_argument("--range", dest="revisions")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    cwd = git("rev-parse", "--show-toplevel").strip()
    if args.staged:
        faults = check_staged(cwd)
    elif args.revisions:
        faults = check_range(args.revisions, cwd)
    else:
        faults = head_faults(cwd)
    for fault in faults:
        print(f"check-tracked-budget: {fault}", file=sys.stderr)
    if faults:
        print(
            "check-tracked-budget: see @/inv/committed-evidence-budget; "
            f"limits are in {CONFIG}",
            file=sys.stderr,
        )
        return 1
    print("check-tracked-budget: ok")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        for name in ("GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"):
            os.environ.pop(name, None)
    sys.exit(main())
