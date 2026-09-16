#!/usr/bin/env python3
"""Selects committed benchmark receipts by their declared schema.

`reevaluate-receipts.sh` evaluates every `zen3-benchmark-receipt-v1` receipt
under `dev/bench_results/` with `benchmark-acceptance`. A receipt of another
schema, such as the 07ca8585 device-conformance evidence
(`ldpc-device-conformance-v1`), is not a benchmark receipt: handing it to a
benchmark-shaped evaluator aborts the run. This module partitions a tree of
committed receipts by each receipt's own declared `schema` field, never by
path, so a receipt of another schema is skipped and recorded instead of
evaluated.

Usage:
  dev/active/a203a23c/select-receipts.py <tree> --selected-out <path> --skipped-out <path>
  dev/active/a203a23c/select-receipts.py --self-test
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

BENCHMARK_SCHEMA = "zen3-benchmark-receipt-v1"


def receipt_paths(tree: Path) -> list[str]:
    """Lists every campaign receipt path under `tree`, snapshot copies excluded."""
    return sorted(
        str(path.relative_to(tree))
        for path in tree.glob("dev/bench_results/**/receipt.json")
        if "/inputs/" not in str(path.relative_to(tree))
    )


def select(tree: Path) -> tuple[list[str], list[tuple[str, str]]]:
    """Partitions committed receipts by each one's own declared schema.

    Returns the sorted benchmark-receipt paths and the sorted (path, schema)
    pairs of every receipt whose declared schema is not the benchmark schema.
    """
    selected: list[str] = []
    skipped: list[tuple[str, str]] = []
    for path in receipt_paths(tree):
        schema = json.loads((tree / path).read_text()).get("schema")
        if schema == BENCHMARK_SCHEMA:
            selected.append(path)
        else:
            skipped.append((path, schema))
    return selected, sorted(skipped)


def repo_root() -> Path:
    """Returns the repository root of the checkout containing this file."""
    return Path(
        subprocess.run(
            ["git", "-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    )


def self_test() -> int:
    """Asserts selection keeps a benchmark receipt and skips another schema.

    Copies one committed benchmark receipt and one committed receipt of
    another schema, found live among the repository's own committed
    receipts, into a temporary tree, then asserts selection evaluates the
    first and lists the second as skipped with its schema.
    """
    root = repo_root()
    live_selected, live_skipped = select(root)
    live_selected = [
        path
        for path in live_selected
        if json.loads((root / path).read_text()).get("schema") == BENCHMARK_SCHEMA
    ]
    if not live_selected:
        print("self-test: no committed benchmark receipt found", file=sys.stderr)
        return 1
    if not live_skipped:
        print(
            "self-test: no committed receipt of another schema found beside "
            "the benchmark receipts",
            file=sys.stderr,
        )
        return 1
    benchmark_receipt = live_selected[0]
    other_receipt, other_schema = live_skipped[0]
    with tempfile.TemporaryDirectory() as directory:
        tree = Path(directory)
        for receipt in (benchmark_receipt, other_receipt):
            destination = tree / receipt
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((root / receipt).read_bytes())
        selected, skipped = select(tree)
        expected_skipped = [(other_receipt, other_schema)]
        if selected != [benchmark_receipt] or skipped != expected_skipped:
            print(
                f"self-test: expected selected=[{benchmark_receipt}] "
                f"skipped={expected_skipped}, observed selected={selected} "
                f"skipped={skipped}",
                file=sys.stderr,
            )
            return 1
    print("select-receipts: self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tree", nargs="?", help="exported evidence tree root")
    parser.add_argument("--selected-out", help="write selected receipt paths, one per line")
    parser.add_argument("--skipped-out", help="write skipped rows as path<TAB>schema")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check selection against committed receipts of two schemas",
    )
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    if arguments.tree is None:
        parser.error("tree is required unless --self-test is given")
    selected, skipped = select(Path(arguments.tree))
    if arguments.selected_out:
        Path(arguments.selected_out).write_text("".join(f"{path}\n" for path in selected))
    if arguments.skipped_out:
        Path(arguments.skipped_out).write_text(
            "".join(f"{path}\t{schema}\n" for path, schema in skipped)
        )
    if not arguments.selected_out and not arguments.skipped_out:
        for path in selected:
            print(path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
