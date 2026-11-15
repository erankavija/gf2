#!/usr/bin/env python3
"""Behavioral test for scripts/check-landed.

A stand-in `jit` on PATH replays recorded `--json` output shaped like jit
1.0.0's; scratch git history lives under $TMPDIR. jit itself never runs.
"""

import os
import shutil
import tempfile

from landing_gates_fixture import (
    ISSUE, Harness, attested, builtin, executed, expect_status, fixture, not_run,
    report, reply, reused, run_id)
from landing_gates import RunStatus

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-landed")
LANDED = "check-landed: every executed run's commit is on main"
NONE_JUDGED = "no executed-command gate, so no commit was judged"
ISSUE_ENV = {"JIT_ISSUE_ID": ISSUE}


def main():
    tmp = tempfile.mkdtemp(prefix="check-landed-test.", dir=os.environ.get("TMPDIR"))
    try:
        run_cases(Harness(tmp, SCRIPT))
    finally:
        shutil.rmtree(tmp)
    return report("check-landed")


def run_cases(h):
    os.makedirs(h.repo)
    h.git("init", "-q")
    merged = h.commit({"src/a": "a\n"})
    on_main = h.commit({"src/a": "b\n"})
    unmerged = h.commit({"src/a": "c\n"})
    h.git("checkout", "-q", "--detach")
    h.git("branch", "-f", "main", on_main)
    absent = "f" * 40

    def check(runs, *args, history=None, **kwargs):
        kwargs.setdefault("env", ISSUE_ENV)
        return h.check(fixture(runs, history, ref=ISSUE), *args, **kwargs)

    all_kinds = {"ci": executed("ci", merged, 1), "plain": executed("plain", on_main, 2),
                 "att": attested(3), "val": builtin(4)}

    # REQ-01, REQ-02: executed runs on main pass; manual and built-in are listed.
    result = check(all_kinds)
    expect_status("all on main", result, 0, LANDED, f"ci: on main: ran at {merged}",
                  f"plain: on main: ran at {on_main}",
                  "att: attested; imposes no condition",
                  "val: store-judged; imposes no condition", absent=("not landed",))

    # The gate does not judge itself: its definition and run are never queried.
    result = check(dict(all_kinds, landed=not_run("landed")))
    expect_status("landed not judged", result, 0, LANDED, absent=("  landed:",))

    # REQ-01: a commit that is not an ancestor of main fails, naming the gate.
    result = check(dict(all_kinds, ci=executed("ci", unmerged, 1)))
    expect_status("unmerged commit", result, 1, "check-landed: not landed: ci\n",
                  f"ci: not landed: commit {unmerged} of run {run_id(1)} is not on main",
                  f"plain: on main: ran at {on_main}", absent=(LANDED,))

    # REQ-01: a commit unknown to this repository is not on main.
    result = check(dict(all_kinds, plain=executed("plain", absent, 2)))
    expect_status("unknown commit", result, 1, "check-landed: not landed: plain\n",
                  f"plain: not landed: commit {absent}", absent=(LANDED,))

    # REQ-01, REQ-06: an executed run that records no commit fails, naming it.
    result = check(dict(all_kinds, ci=executed("ci", None, 1)))
    expect_status("no commit", result, 1, "check-landed: not landed: ci\n",
                  f"ci: not landed: run {run_id(1)} records no commit",
                  absent=(LANDED,))

    # Every failing gate is named.
    result = check(dict(all_kinds, ci=executed("ci", unmerged, 1),
                        plain=executed("plain", None, 2)))
    expect_status("two gates", result, 1, "check-landed: not landed: ci, plain\n",
                  absent=(LANDED,))

    # An executed-command gate that never ran records no commit.
    result = check(dict(all_kinds, ci=not_run("ci")))
    expect_status("never run", result, 1, "check-landed: not landed: ci\n",
                  "ci: not landed: has no run, so no commit is recorded",
                  absent=(LANDED,))

    # REQ-01, REQ-06: a reused verdict is judged by the run that executed it.
    history = {"ci": [reused("ci", run_id(10), 11), executed("ci", merged, 10)]}
    result = check(dict(all_kinds, ci=reused("ci", run_id(10), 5)), history=history)
    expect_status("reused on main", result, 0, LANDED,
                  f"ci: on main: ran at {merged} via origin run {run_id(10)}")
    history = {"ci": [executed("ci", unmerged, 10)]}
    result = check(dict(all_kinds, ci=reused("ci", run_id(10), 5)), history=history)
    expect_status("reused unmerged", result, 1, "check-landed: not landed: ci\n",
                  f"commit {unmerged} of run {run_id(10)} is not on main",
                  absent=(LANDED,))
    history = {"ci": [executed("ci", merged, 12)]}
    result = check(dict(all_kinds, ci=reused("ci", run_id(10), 5)), history=history)
    expect_status("reused origin missing", result, 1, "check-landed: not landed: ci\n",
                  f"ci: not landed: the run that executed the checker for run"
                  f" {run_id(5)} is not found", absent=(LANDED,))

    # REQ-02, REQ-06: no executed-command gate passes with nothing judged,
    # whatever the manual and built-in runs say.
    result = check({"att": attested(3, RunStatus.FAILED), "val": builtin(4)})
    expect_status("no executed gate", result, 0, NONE_JUDGED,
                  "att: attested; imposes no condition",
                  "val: store-judged; imposes no condition", absent=(LANDED,))
    result = check({"landed": not_run("landed")})
    expect_status("only landed", result, 0, NONE_JUDGED)

    # Errors: no issue in the environment, jit unavailable, issue missing.
    expect_status("no issue", check(all_kinds, env={}), 64, "JIT_ISSUE_ID",
                  absent=(LANDED,))
    expect_status("arguments", check(all_kinds, "x"), 64, "usage", absent=(LANDED,))
    expect_status("jit unavailable", check(all_kinds, jit_on_path=False), 2,
                  "cannot run jit", absent=(LANDED,))
    not_found = {f"issue show {ISSUE} --json": reply(
        {"error": {"code": "ISSUE_NOT_FOUND", "message": f"Issue not found: {ISSUE}",
                   "details": {"issue_id": ISSUE}}}, rc=3)}
    result = h.check(not_found, env=ISSUE_ENV)
    expect_status("issue not found", result, 2, "issue not found", absent=(LANDED,))


if __name__ == "__main__":
    raise SystemExit(main())
