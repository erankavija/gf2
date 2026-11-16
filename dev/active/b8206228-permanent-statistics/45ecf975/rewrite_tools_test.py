#!/usr/bin/env python3
"""Self-test of the history rewrite tools. Run: python3 -B <this file>."""
import csv
import gzip
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

here = os.path.dirname(os.path.abspath(__file__))
new = {}
exec(open(os.path.join(here, "rewrite_paths.py")).read(), new)

# The filter before the .jit rule, embedded verbatim.
EX = (b'dev/active/55c3e433-aff3ct-ldpc-gaps/', b'dev/active/02b8137c/',
      b'dev/active/1a379447-zen3-cpu-performance/bench-window/')
def old_drop(f):
    if not f.startswith((b'dev/bench_results/',b'dev/active/')) or f.startswith(EX): return False
    p=f.split(b'/');b=p[-1].lower()
    if any(x in p[:-1] for x in (b'inputs',b'source-inputs',b'test-logs')): return True
    if b.endswith((b'.log',b'.err')) and not re.search(rb'execution|journal',b): return True
    return b.endswith((b'.zst',b'.data')) or b'perf.data' in b

paths = [p.encode() for p in """.jit .jit/x .jitx/y a/.jit/b dev/.jit/x
README.md dev/bench_results/x.txt dev/bench_results/a.zst dev/active/a/b.md
dev/activex/a.log dev/active/a/run.log dev/active/a/RUN.ERR
dev/active/a/execution.log dev/active/a/journal.err dev/active/a/x.zst
dev/active/a/x.data dev/active/a/perf.data.old dev/active/a/inputs/x.md
dev/active/a/source-inputs/x dev/active/a/test-logs/x dev/active/a/inputs
dev/active/55c3e433-aff3ct-ldpc-gaps/run.log dev/active/02b8137c/inputs/x
dev/active/1a379447-zen3-cpu-performance/bench-window/x.zst
dev/active/1a379447-zen3-cpu-performance/other/x.zst""".split()]
differ = [p for p in paths if old_drop(p) != new["drop"](p)]
assert differ == [b".jit", b".jit/x"], differ
assert all(new["drop"](p) for p in differ)


def run_map(env_root, cwd, pins, mapping, path="doc.md"):
    os.makedirs(os.path.join(env_root, "issues"), exist_ok=True)
    rec = os.path.join(env_root, "issues", "i.json")
    with open(rec, "w") as f:
        json.dump({"documents": [{"commit": c, "path": path} for c in pins] + [{}]}, f)
        f.write("\n")
    cmap = os.path.join(cwd, "commit-map")
    with open(cmap, "w") as f:
        f.write("old new\n" + "".join(f"{o} {n}\n" for o, n in mapping))
    env = dict(os.environ)
    env.pop("JIT_DATA_DIR", None)
    if env_root != os.path.join(cwd, ".jit"):
        env["JIT_DATA_DIR"] = env_root
    r = subprocess.run([sys.executable, "-B", os.path.join(here, "map_pins.py"),
                        cmap, os.path.join(cwd, ".git")], cwd=cwd, env=env, capture_output=True, text=True)
    return r, rec


a, b, z = "a" * 40, "b" * 40, "0" * 40
with tempfile.TemporaryDirectory() as t:
    subprocess.run(["git", "init", "-q", t], check=True)
    r, rec = run_map(os.path.join(t, ".jit"), t, ["aaaa"], [(a, b)])
    assert r.returncode == 0 and "mapped 1" in r.stdout, r
    assert json.load(open(rec))["documents"][0]["commit"] == b
    root = os.path.join(t, "store")
    r, rec = run_map(root, t, ["aaaa"], [(a, b)])
    assert r.returncode == 0 and json.load(open(rec))["documents"][0]["commit"] == b, r
    r, rec = run_map(root, t, ["cccc"], [(a, b)])
    assert r.returncode == 1 and rec in r.stderr and "absent" in r.stderr, r
    r, rec = run_map(root, t, ["aaaa"], [(a, z)])
    assert r.returncode == 1 and rec in r.stderr and "pruned" in r.stderr, r

def git(d, *a):
    return subprocess.run(["git", "-C", d, *a], check=True, capture_output=True,
                          text=True).stdout.strip()


def commit(d, name):
    p = os.path.join(d, name)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "a").write("x\n")
    git(d, "add", "-f", name)
    git(d, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", name)
    return git(d, "rev-parse", "HEAD")


with tempfile.TemporaryDirectory() as t:
    git(t, "init", "-q")
    A = commit(t, "doc.md")
    B = commit(t, ".jit/x")
    C = commit(t, "other.md")
    A2, C2, s = "1" * 40, "2" * 40, os.path.join(t, "s")
    m = [(A, A2), (B, z), (C, C2)]
    r, rec = run_map(s, t, [B[:12]], m)
    assert r.returncode == 0 and json.load(open(rec))["documents"][0]["commit"] == A2, r
    assert "1 through a pruned" in r.stdout, r
    r, rec = run_map(s, t, [B[:12]], m, path=".jit/x")
    assert r.returncode == 1 and rec in r.stderr and "dropped" in r.stderr, r
    r, rec = run_map(s, t, [B[:12]], [(A, A2), (C, C2)])
    assert r.returncode == 1 and rec in r.stderr and "absent" in r.stderr, r
    r, rec = run_map(s, t, [A[:12]], [(A, z)])
    assert r.returncode == 1 and rec in r.stderr and "no kept ancestor" in r.stderr, r
with tempfile.TemporaryDirectory() as t:
    git(t, "init", "-q")
    A = commit(t, "doc.md")
    B = commit(t, ".jit/x")
    C = commit(t, "other.md")
    A2, C2 = "1" * 40, "2" * 40
    cmap = os.path.join(t, "commit-map")
    open(cmap, "w").write(f"old new\n{A} {A2}\n{B} {z}\n{C} {C2}\n")
    original = os.path.join(t, ".git")
    checkout = os.path.join(t, "checkout")
    git(t, "clone", "-q", t, checkout)
    text = (f"{A}\n{A[:12]}\n{A[:7]}\n{B}\n"
            f"jit:{A[:8]} @/issue/{A[:8]} issue/{A[:8]}\n"
            f"{C[:8]} {C[:7]}\n" + "f" * 64 + "\n" + "e" * 40 + "\n")
    target = os.path.join(checkout, "doc.md")
    open(target, "w").write(text)
    store = os.path.join(t, "store")
    os.makedirs(os.path.join(store, "issues"))
    record = os.path.join(store, "issues", C[:8] + "-issue.json")
    issue = {"description": "référence\n" + A,
             "documents": [{"commit": A, "path": "doc.md"}]}
    open(record, "w").write(json.dumps(issue) + "\n")
    events = os.path.join(store, "events.jsonl")
    open(events, "w").write(A + "\n")
    binary = os.path.join(checkout, "binary")
    large = os.path.join(checkout, "large")
    open(binary, "wb").write(b"\0" + A.encode())
    open(large, "w").write(A + " " * (2 * 1024 * 1024))
    git(checkout, "add", "binary", "large")
    report = os.path.join(t, "inventory.tsv")

    def old_ids(mode, *extra):
        return subprocess.run(
            [sys.executable, "-B", os.path.join(here, "old_ids.py"), mode,
             cmap, original, checkout, store, "--out", report, *extra],
            capture_output=True, text=True,
        )

    r = old_ids("inventory")
    assert r.returncode == 0, r
    rows = list(csv.DictReader(open(report), delimiter="\t"))
    assert len(rows) == 5, rows
    assert {kind: sum(row["class"] == kind for row in rows)
            for kind in ("mapped", "pruned", "short")} == {
                "mapped": 4, "pruned": 1, "short": 0}, rows
    assert all(row["suggestion"] == A2[:len(row["token"])] for row in rows), rows
    assert [(row["line"], row["token"]) for row in rows if row["file"] == target] == [
        ("1", A), ("2", A[:12]), ("3", A[:7]), ("4", B)], rows
    r = old_ids("check")
    assert r.returncode == 1 and "mapped=4" in r.stdout, r
    r = old_ids("apply", "--exclude", "doc*", "--exclude", "*/issues/*.json")
    assert r.returncode == 0 and "total=0" in r.stdout, r
    assert open(target).read() == text and json.load(open(record)) == issue
    r = old_ids("apply")
    assert r.returncode == 0 and "rewrote 4" in r.stdout, r
    assert open(target).read() == text.replace(A + "\n", A2 + "\n").replace(
        A[:12] + "\n", A2[:12] + "\n").replace(A[:7] + "\n", A2[:7] + "\n")
    issue["description"] = "référence\n" + A2
    assert open(record).read() == json.dumps(issue, indent=2, ensure_ascii=False) + "\n"
    assert open(events).read() == A + "\n"
    assert open(binary, "rb").read() == b"\0" + A.encode()
    assert open(large).read().startswith(A)
    r = old_ids("check")
    assert (r.returncode == 1 and "mapped=0" in r.stdout
            and "pruned=1" in r.stdout and "short=0" in r.stdout), r

    # Exercise ambiguity with two real commit objects sharing seven hex digits.
    tree = git(t, "rev-parse", "HEAD^{tree}")
    header = f"tree {tree}\nauthor t <t@t> 0 +0000\ncommitter t <t@t> 0 +0000\n\n"
    seen = {}
    nonce = 0
    while True:
        payload = (header + str(nonce)).encode()
        digest = hashlib.sha1(b"commit " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        prefix = digest[:7]
        if prefix in seen and prefix != C[:7]:
            for number in (seen[prefix], nonce):
                subprocess.run(["git", "-C", t, "hash-object", "-w", "-t", "commit", "--stdin"],
                               input=(header + str(number)).encode(), check=True, capture_output=True)
            break
        seen[prefix] = nonce
        nonce += 1
    open(target, "w").write(prefix + "\n" + C + "\n")
    with gzip.open(cmap + ".gz", "wt") as f:
        f.write(f"old new\n{A} {A2}\n{B} {z}\n")
    cmap += ".gz"
    issue = {"description": "escaped: " + A, "nested": [A[:13]],
             "documents": [{"commit": A, "caption": A[:12]}], A: "a key"}
    open(record, "w").write(json.dumps(issue))
    r = old_ids("inventory")
    assert r.returncode == 0, r
    rows = list(csv.DictReader(open(report), delimiter="\t"))
    assert {kind: sum(row["class"] == kind for row in rows)
            for kind in ("mapped", "ambiguous", "unmapped")} == {
                "mapped": 3, "ambiguous": 1, "unmapped": 1}, rows
    assert all(row["line"] == "1" for row in rows if row["file"] == record), rows
    r = old_ids("apply")
    assert r.returncode == 0 and "rewrote 3" in r.stdout, r
    issue["description"] = "escaped: " + A2
    issue["nested"] = [A2[:13]]
    issue["documents"][0]["caption"] = A2[:12]
    assert open(record).read() == json.dumps(issue, indent=2, ensure_ascii=False)
    assert open(target).read() == prefix + "\n" + C + "\n"
    r = old_ids("check")
    assert (r.returncode == 0 and "mapped=0" in r.stdout
            and "ambiguous=1" in r.stdout and "unmapped=1" in r.stdout), r
    open(target, "w").write("clean prose\n")
    issue["description"] = "clean prose"
    open(record, "w").write(json.dumps(issue))
    r = old_ids("check")
    assert r.returncode == 0 and "total=0" in r.stdout, r
# Short tokens take the same-length new prefix, lengthened when two kept new
# IDs share it; a pruned commit's short token stays remainder.
with tempfile.TemporaryDirectory() as t:
    git(t, "init", "-q")
    D, E, F = (commit(t, n) for n in ("d.md", "e.md", "f.md"))
    D2, E2 = "3" * 7 + "a" * 33, "3" * 7 + "b" * 33
    cmap = os.path.join(t, "commit-map")
    open(cmap, "w").write(f"old new\n{D} {D2}\n{E} {E2}\n{F} {z}\n")
    checkout = os.path.join(t, "checkout")
    git(t, "clone", "-q", t, checkout)
    target = os.path.join(checkout, "d.md")
    open(target, "w").write(f"{D[:7]} {E[:9]} {F[:8]}\n")
    r = subprocess.run([sys.executable, "-B", os.path.join(here, "old_ids.py"), "apply",
                        cmap, os.path.join(t, ".git"), checkout], capture_output=True, text=True)
    assert r.returncode == 0 and "rewrote 2" in r.stdout and "pruned=1" in r.stdout, r
    assert open(target).read() == f"{D2[:8]} {E2[:9]} {F[:8]}\n", open(target).read()
print("ok")
