#!/usr/bin/env python3
"""Evidence for issue f6148d1e: the two validator facts that hold the dbd8787d entry in place.

    python3 <this file> STAGE REPOSITORY

STAGE is the published dbd8787d stage directory and REPOSITORY the checkout that
holds its campaign evidence archive. Prints, for the committed tree:
  verdicts   --self-test, --receipt-check and --publication of the validator.
  distinct   `locate_declaration` on the tree, then on a scratch repository that
             holds the declaration and a copy whose path strings gain one
             directory level (the edit a move makes).
  digest     `recorded_declaration` against the lifecycle digests of the stage
             revision, for the declaration and for the edited copy.
Nothing in the checkout changes.
"""
import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ISSUE = "dbd8787d"


def run(*args, cwd):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


ROOT = Path(run("git", "rev-parse", "--show-toplevel", cwd=Path(__file__).resolve().parent))
SCRIPT = run("git", "ls-files", "--", ":(glob)*/scripts/validate-tuning-extent-campaign.py", cwd=ROOT)
spec = importlib.util.spec_from_file_location("validator", ROOT / SCRIPT)
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)


def verdict(label, *args):
    done = subprocess.run([sys.executable, str(ROOT / SCRIPT), *args], capture_output=True, text=True)
    print(f"{label}: exit={done.returncode} {(done.stdout + done.stderr).strip()[:200]}")


def attempt(label, call):
    try:
        print(f"{label}: {call()}")
    except V.ValidationError as error:
        print(f"{label}: ValidationError: {error}")


def scratch_repository(base, files):
    for name, data in files.items():
        (base / name).parent.mkdir(parents=True, exist_ok=True)
        (base / name).write_bytes(data)
    run("git", "init", "-q", cwd=base)
    run("git", "add", ".", cwd=base)


stage = sys.argv[1]
print("== verdicts")
verdict("self-test", "--self-test")
verdict("receipt-check", "--stage", stage, "--receipt-check")
verdict("publication", "--stage", stage, "--publication", sys.argv[2])

declared = V.locate_declaration(ISSUE, ROOT)
original = (ROOT / declared).read_bytes()
edited = original.replace(f"/{ISSUE}/".encode(), f"/moved/{ISSUE}/".encode())
assert edited != original
moved = f"moved/{declared}"
print("== distinct")
print(f"tree: {declared}")
with tempfile.TemporaryDirectory() as tmp:
    scratch = Path(tmp)
    scratch_repository(scratch, {declared: original, moved: edited})
    attempt("declaration and edited copy", lambda: V.locate_declaration(ISSUE, scratch))

print("== digest")
revision = next(
    json.loads((ROOT / name).read_text())["assembly"]["source_revision"]
    for name in run("git", "ls-files", "--", f":(glob)**/gf2-{ISSUE}-*.json", cwd=ROOT).split("\n")
    if '"assembly"' in (ROOT / name).read_text())
declaration = json.loads(original)
lifecycle = V.source_hashes(
    ROOT, V.producing_inputs(ROOT, declaration["producing_manifest"], revision)["lifecycle_sources"], revision)
for label, data in (("declaration", original), ("edited copy", edited)):
    with tempfile.TemporaryDirectory() as tmp:
        scratch = Path(tmp)
        scratch_repository(scratch, {declared: data})
        V.repository_root = lambda scratch=scratch: scratch
        attempt(f"{label} against the lifecycle digests of the stage revision",
                lambda: V.recorded_declaration(declaration, lifecycle, "lifecycle manifest")[0])
