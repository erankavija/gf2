#!/usr/bin/env python3
"""Self-test of rewrite_paths.py and map_pins.py. Run: python3 -B <this file>."""
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
print("ok")
