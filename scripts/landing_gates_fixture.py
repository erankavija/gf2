"""Recorded jit output and a scratch harness for the landing scripts' self-tests.

A stand-in `jit` on PATH replays `--json` output shaped like jit 1.0.0's;
scratch git history lives under $TMPDIR. jit itself never runs.
"""

import json
import os
import shutil
import subprocess
import sys

ISSUE = "051c2ade-1d12-4fb7-bcd7-69a02900c6e4"
SOURCE_ISSUE = "a57cb8d0-29fc-4f05-a3a3-58142532b5bf"

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


def fixture(runs, history=None, ref=ISSUE[:8]):
    """Calls for an issue, shown as `ref`, requiring the gates in `runs`.

    `runs` maps a gate key to its latest run; the `landed` gate's definition
    and run are absent, so a script that queries them fails.
    """
    issue = {
        "assignee": None, "id": ISSUE, "short_id": ISSUE[:8], "state": "ready",
        "title": "t", "description": "d", "labels": [], "dependencies": [],
        "gates": [
            {"exit_code": r.get("exit_code"), "key": k,
             "last_run_at": r.get("started_at"), "status": r.get("status", "pending")}
            for k, r in runs.items()
        ],
    }
    calls = {f"issue show {ref} --json": reply(issue)}
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
    """Runs `script` in a scratch repository with the stand-in jit on PATH.

    The caller's git and JIT_* settings are dropped.
    """

    def __init__(self, tmp, script):
        self.script = script
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
                    if not k.startswith(("GIT_", "JIT_")) and k != "PATH"}
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

    def check(self, calls, *args, jit_on_path=True, env=None):
        with open(os.path.join(self.bin, "fixture.json"), "w") as f:
            json.dump(calls, f)
        env = {**self.env, **(env or {}),
               "PATH": self.bin if jit_on_path else self.bare_bin}
        proc = subprocess.run([sys.executable, "-B", self.script, *args],
                              cwd=self.repo, env=env, capture_output=True, text=True)
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


def report(name):
    """Prints the failures and returns the exit status."""
    if FAILURES:
        for failure in FAILURES:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    print(f"{name} self-test: ok")
    return 0
