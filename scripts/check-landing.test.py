#!/usr/bin/env python3
"""Behavioral test for scripts/check-landing.

A stand-in `jit` on PATH replays recorded `--json` output shaped like jit
1.0.0's; scratch git history lives under $TMPDIR. jit itself never runs.
"""

import os
import shutil
import sys
import tempfile

from landing_gates_fixture import (
    ISSUE, Harness, attested, builtin, executed, expect_status, fixture, not_run,
    report, reply, reused, run_id)
from landing_gates import RunStatus

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-landing")
SUCCESS = "every required verdict counts"


def main():
    tmp = tempfile.mkdtemp(prefix="check-landing-test.", dir=os.environ.get("TMPDIR"))
    try:
        run_cases(Harness(tmp, SCRIPT))
    finally:
        shutil.rmtree(tmp)
    return report("check-landing")


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
    runs = dict(all_kinds, ci=executed("ci", base, 1, RunStatus.FAILED),
                plain=executed("plain", in_root, 2))
    result = h.check(fixture(runs), short, in_root)
    expect_status("failed at older commit", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {in_root}",
                  "differing input paths: src/a", f"latest run {run_id(1)} is failed",
                  absent=(SUCCESS, "skip1.md"))
    result = h.check(fixture(dict(all_kinds, ci=executed("ci", base, 1, RunStatus.FAILED))),
                     short, base)
    expect_status("failed at expected commit", result, 1, "ci: refused",
                  f"recorded commit {base}", f"expected {base}",
                  "differing input paths: none", f"latest run {run_id(1)} is failed",
                  absent=(SUCCESS,))
    for key, run in (("att", attested(3, RunStatus.FAILED)), ("val", builtin(4, RunStatus.FAILED))):
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
    failed_runs = dict(runs, ci=reused("ci", run_id(10), 5, RunStatus.FAILED))
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

    unknown_status = fixture(all_kinds)
    unknown_status[f"gate status {ISSUE} ci --json"] = reply(
        {**executed("ci", base, 1), "status": "mystery-status"}
    )
    result = h.check(unknown_status, short, base)
    expect_status("unknown run status", result, 2,
                  "unknown gate run status 'mystery-status'", absent=(SUCCESS,))

    # Usage and revision errors.
    expect_status("no arguments", h.check({}), 64, "usage", absent=(SUCCESS,))
    expect_status("bad revision", h.check(fixture(all_kinds), short, "no-such-rev"),
                  64, "cannot resolve revision", absent=(SUCCESS,))


if __name__ == "__main__":
    sys.exit(main())
