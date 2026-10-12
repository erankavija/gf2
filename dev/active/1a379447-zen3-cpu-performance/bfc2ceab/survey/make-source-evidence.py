#!/usr/bin/env python3
"""Freeze the source claims of the caller-trusted entry point review (jit:bfc2ceab).

Usage: make-source-evidence.py, with no arguments

Writes `source-evidence.json` beside itself: per claim of `review.py`, the
first line holding its fragment, that line verbatim, the file digest and the
commit that last changed the file. Fails when a fragment is absent or held by
another number of lines than the claim states.
"""

import hashlib
import json
import subprocess

import review
from locate import HERE, ISSUE, ROOT, family_stages, package, repository_files

ROOT_FILE = "Cargo.toml"


def cleared_claim_id(path, function):
    """The identifier of the claim that clears `function` of the package path `path`."""
    module = path.removeprefix("src/").removesuffix(".rs").replace("/", "-").replace("_", "-")
    return f"cleared-{module}-{function.replace('_', '-')}"


def claims():
    """Every claim of the review, as (id, owner, path, fragment, occurrences, why)."""
    rows = list(review.CLAIMS)
    for path, function, basis, evidence, fragment, occurrences in review.CLEARED:
        rows.append(
            (cleared_claim_id(path, function), review.SIMD, evidence, fragment, occurrences,
             f"`{function}` of `{path}`: {review.BASES[basis]}")
        )
    return rows


def resolve(owner, relative):
    """Root-relative path of a claim's file."""
    if owner == review.MEASURED:
        stage = family_stages(HERE.parent / review.CONFIRMATION_ADDENDUM)["confirmation"]
        return str((stage / "inputs" / "producing" / relative).relative_to(ROOT))
    if owner is not None:
        return f"{package(owner)}/{relative}"
    if relative == ROOT_FILE:
        return relative
    return repository_files.live_file(ROOT, relative)


def last_change(path):
    """The commit that last changed `path`; content digests decide validity."""
    return subprocess.run(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", path],
        capture_output=True, check=True, text=True,
    ).stdout.strip()


def main():
    rows = claims()
    if len({row[0] for row in rows}) != len(rows):
        raise SystemExit("claim identifiers repeat")
    records, failures = [], []
    for identifier, owner, relative, fragment, occurrences, why in rows:
        path = resolve(owner, relative)
        text = (ROOT / path).read_text()
        lines = text.splitlines()
        positions = [index + 1 for index, line in enumerate(lines) if fragment in line]
        if len(positions) != occurrences:
            failures.append(
                f"{identifier}: {path}: expected {occurrences} lines holding {fragment!r}, "
                f"found {len(positions)}"
            )
            continue
        verbatim = lines[positions[0] - 1]
        if sum(verbatim in line for line in lines) != occurrences:
            failures.append(f"{identifier}: {path}: lines holding the fragment differ")
            continue
        records.append(
            {
                "id": identifier,
                "project": "gf2",
                "commit": last_change(path),
                "path": path,
                "line": positions[0],
                "occurrences": occurrences,
                "verbatim": verbatim,
                "why": why,
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
            }
        )
    if failures:
        raise SystemExit("\n".join(failures))
    output = HERE / "source-evidence.json"
    output.write_text(
        json.dumps({"schema": "source-evidence-v1", "issue": ISSUE, "claims": records}, indent=2)
        + "\n"
    )
    print(f"{output.relative_to(ROOT)}: {len(records)} source claims")


if __name__ == "__main__":
    main()
