#!/usr/bin/env python3
"""Runtime location of the repository root and of the artifacts below it.

The root is the git checkout holding this file. An artifact is located by what
it is: a tracked file name, a line of its content, or the campaign a receipt
records. Receipt input snapshots hold byte copies of live files and are never
a location.

Usage: repo_artifacts.py tracked NAME | containing NAME TEXT
                         | receipt CAMPAIGN | addendum CAMPAIGN | ledger CAMPAIGN
"""

import json
import pathlib
import subprocess
import sys

SNAPSHOT_DIRECTORY = "inputs"


def _git(*arguments):
    return subprocess.run(
        ["git", *arguments], capture_output=True, check=True, text=True
    ).stdout


ROOT = pathlib.Path(
    _git("-C", str(pathlib.Path(__file__).resolve().parent), "rev-parse", "--show-toplevel").strip()
)


def _one(paths, what):
    live = [path for path in paths if SNAPSHOT_DIRECTORY not in pathlib.Path(path).parts]
    if len(live) != 1:
        raise SystemExit(f"{what} names {len(live)} tracked files rather than one: {live}")
    return pathlib.Path(live[0])


def tracked(name):
    """The root-relative path of the one tracked file called `name`."""
    listing = _git("-C", str(ROOT), "ls-files", "--", f":(glob)**/{name}")
    return _one(listing.splitlines(), name)


def containing(name, text):
    """The one tracked file called `name` that holds `text`."""
    found = subprocess.run(
        ["git", "-C", str(ROOT), "grep", "-l", "--full-name", "-F", "-e", text, "--",
         f":(glob)**/{name}"],
        capture_output=True, text=True,
    ).stdout
    return _one(found.splitlines(), f"{name} holding {text!r}")


def receipt(campaign):
    """The root-relative directory of the receipt `campaign` recorded."""
    found = subprocess.run(
        ["git", "-C", str(ROOT), "grep", "-l", "--full-name", "-F", "-e", campaign, "--",
         ":(glob)**/receipt.json"],
        capture_output=True, text=True,
    ).stdout
    own = [
        path for path in found.splitlines()
        if SNAPSHOT_DIRECTORY not in pathlib.Path(path).parts
        and json.loads((ROOT / path).read_bytes())["campaign_id"] == campaign
    ]
    return _one(own, f"campaign {campaign}").parent


def addendum(campaign):
    """The root-relative campaign addendum the receipt of `campaign` pins."""
    record = json.loads((ROOT / receipt(campaign) / "receipt.json").read_bytes())
    return pathlib.Path(record["addendum"]["path"])


def ledger(campaign):
    """The root-relative family ledger that campaign's addendum declares."""
    declared = json.loads((ROOT / addendum(campaign)).read_bytes())
    return pathlib.Path(declared["family_wise"]["ledger_path"])


if __name__ == "__main__":
    commands = {"tracked": tracked, "containing": containing, "receipt": receipt,
                "addendum": addendum, "ledger": ledger}
    if len(sys.argv) < 3 or sys.argv[1] not in commands:
        raise SystemExit(__doc__)
    print(commands[sys.argv[1]](*sys.argv[2:]))
