#!/usr/bin/env python3
"""Behavioral test for scripts/check-landing.

A stand-in `jit` on PATH replays recorded `--json` output shaped like jit
1.0.0's; scratch git history lives under $TMPDIR. jit itself never runs.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-landing")
ISSUE = "051c2ade-1d12-4fb7-bcd7-69a02900c6e4"
SOURCE_ISSUE = "a57cb8d0-29fc-4f05-a3a3-58142532b5bf"
SUCCESS = "every required verdict counts"

STAND_IN = """#!{python}
import json, os, sys
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixture.json")) as f:
    calls = json.load(f)
reply = calls.get(" ".join(sys.argv[1:]))
if reply is None:
    print("stand-in jit: no fixture for " + " ".join(sys.argv[1:]), file=sys.stderr)
    sys.exit(97)
sys.stdout.write(reply["out"])
sys.exit(reply["rc"])
"""

# Gate definitions as `jit gate show <key> --json` prints them.
DEFINITIONS = {
    "ci": {
        "checker": {"command": "./ci.sh", "env": {}, "pass_context": False,
                    "timeout_seconds": 30, "type": "exec", "working_dir": None},
        "description": "ci",
        "inputs": {"exclude": ["src/**/skip*.md", "**/target"], "roots": ["src"]},
        "key": "ci", "message": "Gate ci: ci", "mode": "auto", "priority": 100,
        "reserved": {}, "stage": "postcheck", "title": "ci", "version": 1,
    },
    "plain": {
        "checker": {"command": "./plain.sh", "env": {}, "pass_context": False,
                    "timeout_seconds": 30, "type": "exec", "working_dir": None},
        "description": "plain", "key": "plain", "message": "Gate plain: plain",
        "mode": "auto", "priority": 100, "reserved": {}, "stage": "postcheck",
        "title": "plain", "version": 1,
    },
    "att": {
        "description": "att", "key": "att", "message": "Gate att: att",
        "mode": "manual", "priority": 100, "reserved": {}, "stage": "postcheck",
        "title": "att", "version": 1,
    },
    "val": {
        "checker": {"type": "issue_validation"}, "description": "val", "key": "val",
        "message": "Gate val: val", "mode": "auto", "priority": 100, "reserved": {},
        "stage": "postcheck", "title": "val", "version": 1,
    },
}


def run_id(n):
    return f"00000000-0000-4000-8000-{n:012d}"


def executed(key, commit, n, status="passed"):
    record = {
        "by": "auto:executor", "command": DEFINITIONS[key]["checker"]["command"],
        "completed_at": "2026-11-15T16:56:20.547212530+00:00", "duration_ms": 100,
        "evaluation_stability": "stable", "exit_code": 0 if status == "passed" else 1,
        "key": key, "message": f"Gate '{key}': {status}",
        "origin": {"derivation": "executed"}, "run_id": run_id(n),
        "stage": "postcheck", "started_at": "2026-11-15T16:56:20.446969674+00:00",
        "status": status, "stderr": "", "stdout": "",
    }
    if commit is not None:
        record.update(branch="main", commit=commit)
    return record


def reused(key, source_run, n, status="passed"):
    return {
        "by": "auto:executor", "command": DEFINITIONS[key]["checker"]["command"],
        "completed_at": "2026-11-15T16:56:25.983604318+00:00", "duration_ms": 0,
        "evaluation_stability": "stable", "key": key,
        "message": f"Gate '{key}': {status}",
        "origin": {"derivation": "reused", "source_issue": SOURCE_ISSUE,
                   "source_run": source_run},
        "run_id": run_id(n), "stage": "postcheck",
        "started_at": "2026-11-15T16:56:25.983604318+00:00", "status": status,
        "stderr": "", "stdout": "",
    }


def attested(n, status="passed"):
    return {
        "by": "human:tester", "command": "",
        "completed_at": "2026-11-15T16:56:20.685430347+00:00",
        "evaluation_stability": "stable", "key": "att",
        "message": f"Gate 'att': {status}", "origin": {"derivation": "attested"},
        "run_id": run_id(n), "stage": "postcheck",
        "started_at": "2026-11-15T16:56:20.685430347+00:00", "status": status,
        "stderr": "",
    }


def builtin(n, status="passed"):
    return {
        "by": "auto:executor", "command": "builtin:issue_validation",
        "completed_at": "2026-11-15T16:56:20.675045339+00:00", "duration_ms": 1,
        "evaluation_stability": "stable", "exit_code": 0 if status == "passed" else 1,
        "findings": {"findings": [], "summary": "Built-in validation", "verdict": "pass"},
        "key": "val", "message": f"Gate 'val': {status}",
        "origin": {"derivation": "executed"}, "run_id": run_id(n),
        "stage": "postcheck", "started_at": "2026-11-15T16:56:20.673611481+00:00",
        "status": status, "stderr": "",
    }


def not_run(key):
    return {
        "issue_id": ISSUE, "key": key,
        "message": f"Gate '{key}' has not been run yet for issue {ISSUE}."
                   " Use 'jit gate evaluate' to run it.",
    }


def reply(data, rc=0):
    return {"out": json.dumps(data, indent=2) + "\n", "rc": rc}


def fixture(runs, history=None):
    """Calls for an issue requiring the gates in `runs` (key -> latest run)."""
    issue = {
        "assignee": None, "id": ISSUE, "short_id": ISSUE[:8], "state": "ready",
        "title": "t", "description": "d", "labels": [], "dependencies": [],
        "gates": [
            {"exit_code": r.get("exit_code"), "key": k,
             "last_run_at": r.get("started_at"), "status": r.get("status", "pending")}
            for k, r in runs.items()
        ],
    }
    calls = {f"issue show {ISSUE[:8]} --json": reply(issue)}
    for key, run in runs.items():
        if key == "landed":
            continue
        calls[f"gate show {key} --json"] = reply(DEFINITIONS[key])
        calls[f"gate status {ISSUE} {key} --json"] = reply(run)
    for key, results in (history or {}).items():
        calls[f"gate status {SOURCE_ISSUE} --all --gate {key} --json"] = reply(
            {"count": len(results), "results": results})
    return calls


class Harness:
    def __init__(self, tmp):
        self.tmp = tmp
        self.repo = os.path.join(tmp, "repo")
        self.bin = os.path.join(tmp, "bin")
        self.bare_bin = os.path.join(tmp, "bare-bin")
        os.makedirs(self.bin)
        os.makedirs(self.bare_bin)
        stand_in = os.path.join(self.bin, "jit")
        with open(stand_in, "w") as f:
            f.write(STAND_IN.format(python=sys.executable))
        os.chmod(stand_in, 0o755)
        for name in ("git",):
            os.symlink(shutil.which(name), os.path.join(self.bin, name))
            os.symlink(shutil.which(name), os.path.join(self.bare_bin, name))
        self.env = {k: v for k, v in os.environ.items()
                    if not k.startswith("GIT_") and k != "PATH"}
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)

    def git(self, *args):
        return subprocess.run(
            ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid",
             "-c", "commit.gpgsign=false", *args],
            cwd=self.repo, env={**self.env, "PATH": self.bin}, check=True,
            capture_output=True, text=True).stdout.strip()

    def commit(self, files, parent=None):
        if parent:
            self.git("checkout", "-q", "--detach", parent)
        for path, text in files.items():
            full = os.path.join(self.repo, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w") as f:
                f.write(text)
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "c")
        return self.git("rev-parse", "HEAD")

    def check(self, calls, *args, jit_on_path=True):
        with open(os.path.join(self.bin, "fixture.json"), "w") as f:
            json.dump(calls, f)
        env = {**self.env, "PATH": self.bin if jit_on_path else self.bare_bin}
        proc = subprocess.run([sys.executable, SCRIPT, *args], cwd=self.repo,
                              env=env, capture_output=True, text=True)
        return proc.returncode, proc.stdout, proc.stderr


FAILURES = []


def expect(label, condition, detail=""):
    if not condition:
        FAILURES.append(f"{label}: {detail}")


def expect_status(label, result, status, *needles, absent=()):
    rc, out, err = result
    text = out + err
    expect(label, rc == status, f"exit {rc}, expected {status}\n{text}")
    for needle in needles:
        expect(label, needle in text, f"missing {needle!r}\n{text}")
    for needle in absent:
        expect(label, needle not in text, f"unexpected {needle!r}\n{text}")


def main():
    tmp = tempfile.mkdtemp(prefix="check-landing-test.", dir=os.environ.get("TMPDIR"))
    try:
        run_cases(Harness(tmp))
    finally:
        shutil.rmtree(tmp)
    if FAILURES:
        for failure in FAILURES:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print("check-landing self-test: ok")
    return 0


def run_cases(h):
    os.makedirs(h.repo)
    h.git("init", "-q")
    base = h.commit({"src/a": "a\n", "src/d/skip1.md": "s\n", "other/x": "x\n"})
    # Changes inside declared exclusions only: a glob and a directory match.
    excluded = h.commit({"src/d/skip1.md": "t\n", "src/d/target/out": "o\n"})
    in_root = h.commit({"src/a": "b\n"})
    outside = h.commit({"other/x": "y\n"}, parent=base)
    h.git("branch", "tip", excluded)
    h.git("checkout", "-q", "--detach", base)
    h.git("commit", "-q", "--allow-empty", "-m", "same tree")
    same_tree = h.git("rev-parse", "HEAD")
    absent = "f" * 40
    short = ISSUE[:8]

    all_kinds = {"ci": executed("ci", base, 1), "plain": executed("plain", base, 2),
                 "att": attested(3), "val": builtin(4)}

    # REQ-01, REQ-04: exact commit; manual and built-in gates by status alone.
    result = h.check(fixture(all_kinds), short, base)
    expect_status("exact commit", result, 0, SUCCESS, "att: attested",
                  "val: store-judged", "ci: counts", "plain: counts")

    # The landed gate judges the merge after this check: skipped, never queried.
    result = h.check(fixture(dict(all_kinds, landed=not_run("landed"))), short, base)
    expect_status("landed skipped", result, 0, SUCCESS,
                  "  landed: judged after the merge\n")

    # REQ-01, REQ-06: a change inside a declared exclusion keeps the verdict.
    runs = dict(all_kinds, plain=executed("plain", excluded, 2))
    result = h.check(fixture(runs), short, "tip")
    expect_status("exclusion change", result, 0, SUCCESS,
                  f"ran at {base}", "declared inputs identical")

    # REQ-02, REQ-06: a change inside an input root refuses, naming the path.
    runs = dict(all_kinds, plain=executed("plain", in_root, 2))
    result = h.check(fixture(runs), short, in_root)
    expect_status("input root change", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {in_root}", "src/a",
                  absent=(SUCCESS, "skip1.md", "target/out"))

    # REQ-02, REQ-06: a gate without declared inputs counts for its commit only.
    runs = dict(all_kinds, ci=executed("ci", outside, 1))
    result = h.check(fixture(runs), short, outside)
    expect_status("no declared inputs", result, 1, "plain: refused",
                  f"recorded commit {base}", f"expected {outside}",
                  "no declared inputs", "other/x", absent=(SUCCESS,))

    # Without declared inputs an identical tree at another commit still refuses.
    runs = dict(all_kinds, ci=executed("ci", same_tree, 1))
    result = h.check(fixture(runs), short, same_tree)
    expect_status("no inputs, same tree", result, 1, "plain: refused",
                  f"recorded commit {base}", "no declared inputs", absent=(SUCCESS,))

    # REQ-02: an executed run that records no commit.
    runs = dict(all_kinds, ci=executed("ci", None, 1))
    result = h.check(fixture(runs), short, base)
    expect_status("no commit", result, 1, "ci: refused", "recorded commit none",
                  f"expected {base}", absent=(SUCCESS,))

    # REQ-02: a recorded commit unknown to this repository.
    runs = dict(all_kinds, ci=executed("ci", absent, 1))
    result = h.check(fixture(runs), short, base)
    expect_status("unknown commit", result, 1, "ci: refused",
                  f"recorded commit {absent}", "not in this repository",
                  absent=(SUCCESS,))

    # REQ-02: a run that did not pass names its commits and input paths.
    runs = dict(all_kinds, ci=executed("ci", base, 1, "failed"),
                plain=executed("plain", in_root, 2))
    result = h.check(fixture(runs), short, in_root)
    expect_status("failed at older commit", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {in_root}",
                  "differing input paths: src/a", f"latest run {run_id(1)} is failed",
                  absent=(SUCCESS, "skip1.md"))
    result = h.check(fixture(dict(all_kinds, ci=executed("ci", base, 1, "failed"))),
                     short, base)
    expect_status("failed at expected commit", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {base}",
                  "differing input paths: none", f"latest run {run_id(1)} is failed",
                  absent=(SUCCESS,))
    for key, run in (("att", attested(3, "failed")), ("val", builtin(4, "failed"))):
        result = h.check(fixture(dict(all_kinds, **{key: run})), short, base)
        expect_status(f"{key} failed", result, 1, f"{key}: refused", "is failed",
                      "no commit applies", absent=(SUCCESS,))
    result = h.check(fixture(dict(all_kinds, att=not_run("att"))), short, base)
    expect_status("manual never run", result, 1, "att: refused", "has no run",
                  "no commit applies", absent=(SUCCESS,))
    result = h.check(fixture(dict(all_kinds, ci=not_run("ci"))), short, base)
    expect_status("executed never run", result, 1, "ci: refused", "has no run",
                  "recorded commit none", f"expected {base}", "differing input paths",
                  absent=(SUCCESS,))

    # REQ-03: a reused verdict is judged by the run that executed the checker.
    source = executed("ci", base, 10)
    history = {"ci": [reused("ci", run_id(10), 11), source]}
    runs = dict(all_kinds, ci=reused("ci", run_id(10), 5),
                plain=executed("plain", excluded, 2))
    result = h.check(fixture(runs, history), short, excluded)
    expect_status("reused counts", result, 0, SUCCESS, f"ran at {base}",
                  f"via origin run {run_id(10)}")
    runs["plain"] = executed("plain", in_root, 2)
    result = h.check(fixture(runs, history), short, in_root)
    expect_status("reused refused", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {in_root}", "src/a",
                  "needs a forced evaluation at", "--force", absent=(SUCCESS,))
    # A failed reused run names its origin run and the input paths.
    failed_runs = dict(runs, ci=reused("ci", run_id(10), 5, "failed"))
    result = h.check(fixture(failed_runs, history), short, in_root)
    expect_status("reused failed", result, 1, "ci: refused",
                  f"recorded commit {base} via origin run {run_id(10)}",
                  f"expected {in_root}", "differing input paths: src/a",
                  f"latest run {run_id(5)} is failed", "needs a forced evaluation at",
                  absent=(SUCCESS,))
    missing = {"ci": [executed("ci", base, 12)]}
    result = h.check(fixture(runs, missing), short, in_root)
    expect_status("reused source missing", result, 1, "ci: refused",
                  "needs a forced evaluation at", absent=(SUCCESS,))

    # REQ-05: jit unavailable, the issue missing, jit failing.
    result = h.check(fixture(all_kinds), short, base, jit_on_path=False)
    expect_status("jit unavailable", result, 2, "cannot run jit", absent=(SUCCESS,))
    not_found = {f"issue show {short} --json": reply(
        {"error": {"code": "ISSUE_NOT_FOUND", "message": f"Issue not found: {short}",
                   "details": {"issue_id": short}}}, rc=3)}
    result = h.check(not_found, short, base)
    expect_status("issue not found", result, 2, "issue not found", absent=(SUCCESS,))
    broken = fixture(all_kinds)
    broken["gate show val --json"] = {"out": "not json\n", "rc": 0}
    result = h.check(broken, short, base)
    expect_status("jit garbage", result, 2, "no JSON object", absent=(SUCCESS,))
    result = h.check({}, short, base)
    expect_status("jit fails", result, 2, "exited 97", absent=(SUCCESS,))

    # Usage and revision errors.
    expect_status("no arguments", h.check({}), 64, "usage", absent=(SUCCESS,))
    expect_status("bad revision", h.check(fixture(all_kinds), short, "no-such-rev"),
                  64, "cannot resolve revision", absent=(SUCCESS,))


if __name__ == "__main__":
    sys.exit(main())
