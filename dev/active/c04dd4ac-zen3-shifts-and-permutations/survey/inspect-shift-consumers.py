#!/usr/bin/env python3
"""Record the complete production-source consumer sweep for BitVec shifts."""

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OWNER = Path("crates/gf2-core/src/bitvec.rs")
CALL = re.compile(r"\.\s*shift_(left|right)\s*\(")
OUTPUT = Path(
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/"
    "shift-profile-consumer-audit.json"
)


def report():
    sources = sorted(Path("crates").glob("*/src/**/*.rs"))
    digest = hashlib.sha256()
    downstream = []
    for path in sources:
        data = path.read_bytes()
        digest.update(str(path).encode())
        digest.update(b"\0")
        digest.update(data)
        if path == OWNER:
            continue
        for number, line in enumerate(data.decode().splitlines(), 1):
            if CALL.search(line):
                downstream.append({"path": str(path), "line": number, "text": line.strip()})
    return {
        "schema": "bitvec-shift-production-consumer-audit-v1",
        "issue": "85fc5ff4",
        "scope": "every Rust source below crates/*/src",
        "query": r"\.\s*shift_(left|right)\s*\(",
        "owner_excluded": str(OWNER),
        "files_scanned": len(sources),
        "source_closure_sha256": digest.hexdigest(),
        "downstream_production_calls": downstream,
        "downstream_production_call_count": len(downstream),
        "disposition": (
            "no downstream production caller; profile the public primitive as an "
            "isolated family and mark whole-consumer measurement inapplicable"
        ),
        "passed": not downstream,
    }


def encoded():
    return json.dumps(report(), indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = encoded()
    if args.check:
        observed = OUTPUT.read_text()
        if observed != expected:
            raise SystemExit(f"{OUTPUT} differs from the current production-source sweep")
        value = json.loads(observed)
        if not value["passed"]:
            raise SystemExit("a downstream production shift caller exists")
        print(
            f"{OUTPUT}: {value['files_scanned']} files, "
            f"{value['downstream_production_call_count']} downstream calls"
        )
    else:
        OUTPUT.write_text(expected)
        value = report()
        print(
            f"{OUTPUT}: {value['files_scanned']} files, "
            f"{value['downstream_production_call_count']} downstream calls"
        )


if __name__ == "__main__":
    main()
