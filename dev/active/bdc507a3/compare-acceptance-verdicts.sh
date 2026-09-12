#!/usr/bin/env bash
# Compares the acceptance verdict of every committed benchmark receipt under
# the benchmark-acceptance binary built from a base revision and under the one
# built from HEAD, and writes dev/active/bdc507a3/verdict-preservation.json.
#
# Usage: dev/active/bdc507a3/compare-acceptance-verdicts.sh <base-revision>
#
# Run from any directory of the repository; the tooling crate must have no
# uncommitted change, so HEAD identifies the evaluated source. Receipts are
# evaluated in hard-linked copies under target/bdc507a3-verdicts/ with their
# committed acceptance summaries unlinked, so no committed file is written.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <base-revision>" >&2
  exit 2
fi

root=$(git rev-parse --show-toplevel)
cd "$root"
base=$(git rev-parse --verify "$1^{commit}")
head=$(git rev-parse --verify HEAD)
crate=dev/tools/tuning-campaign-support
git diff --quiet HEAD -- "$crate" Cargo.toml Cargo.lock .cargo || {
  echo "$crate or the workspace manifests differ from HEAD" >&2
  exit 1
}

work=target/bdc507a3-verdicts
rm -rf "$work"
mkdir -p "$work/base-src" "$work/runs"

# The base binary is built from the exact base tree of the workspace crates
# and the research prototype one of them depends on.
git archive "$base" -- Cargo.toml Cargo.lock .cargo crates dev/tools dev/research/f3_bipedal |
  tar -x -C "$work/base-src"
(cd "$work/base-src" && CARGO_TARGET_DIR="$root/$work/base-target" \
  "$root/scripts/cargo-budget.sh" cargo build --locked --release \
  -p tuning-campaign-support --bin benchmark-acceptance)
./scripts/cargo-budget.sh cargo build --locked --release \
  -p tuning-campaign-support --bin benchmark-acceptance
cp "$work/base-target/release/benchmark-acceptance" "$work/base-benchmark-acceptance"
cp target/release/benchmark-acceptance "$work/head-benchmark-acceptance"

# One tab-separated row per receipt and side: receipt, side, exit code.
: > "$work/exits.tsv"
git ls-files -z 'dev/bench_results/**/receipt.json' | while IFS= read -r -d '' receipt; do
  case "$receipt" in */inputs/*) continue ;; esac
  dir=$(dirname "$receipt")
  for side in base head; do
    copy="$work/runs/$side/$dir"
    mkdir -p "$(dirname "$copy")"
    cp -al "$dir" "$copy"
    rm -f "$copy/acceptance-summary.json" "$copy/acceptance-summary.md"
    status=0
    "$work/$side-benchmark-acceptance" "$copy" > /dev/null 2> "$copy.stderr" || status=$?
    printf '%s\t%s\t%s\n' "$receipt" "$side" "$status" >> "$work/exits.tsv"
  done
done

python3 - "$root" "$work" "$base" "$head" <<'EOF'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

root, work, base, head = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], sys.argv[4]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def summary(side, receipt_dir):
    path = root / work / "runs" / side / receipt_dir / "acceptance-summary.json"
    if not path.exists():
        return None, None
    return json.loads(path.read_text())["verdict"], digest(path)


exits = {}
for line in (root / work / "exits.tsv").read_text().splitlines():
    receipt, side, status = line.split("\t")
    exits.setdefault(receipt, {})[side] = int(status)

rows = []
for receipt, statuses in sorted(exits.items()):
    receipt_dir = str(Path(receipt).parent)
    document = json.loads((root / receipt).read_text())
    addendum = json.loads((root / receipt_dir / document["addendum"]["snapshot"]).read_text())
    committed = root / receipt_dir / "acceptance-summary.json"
    row = {
        "receipt": receipt,
        "protocol_version": addendum["protocol"]["version"],
        "committed_verdict": json.loads(committed.read_text())["verdict"] if committed.exists() else None,
    }
    for side in ("base", "head"):
        verdict, summary_sha256 = summary(side, receipt_dir)
        row[side] = {"exit": statuses[side], "verdict": verdict, "summary_sha256": summary_sha256}
    row["identical_verdict"] = (row["base"]["exit"], row["base"]["verdict"]) == (
        row["head"]["exit"],
        row["head"]["verdict"],
    )
    row["identical_summary"] = row["base"]["summary_sha256"] == row["head"]["summary_sha256"]
    rows.append(row)

record = {
    "schema": "bdc507a3-verdict-preservation-v1",
    "purpose": "Acceptance verdicts of every committed benchmark receipt under the evaluator before and after protocol version 4 changed the P-11 rule for interrupted cells.",
    "committed_verdict": "Verdict of the acceptance summary committed beside the receipt, which an earlier evaluator wrote from the files present where it ran.",
    "generator": "dev/active/bdc507a3/compare-acceptance-verdicts.sh",
    "toolchain": subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
    "base": {
        "revision": base,
        "benchmark_acceptance_sha256": digest(root / work / "base-benchmark-acceptance"),
    },
    "head": {
        "revision": head,
        "benchmark_acceptance_sha256": digest(root / work / "head-benchmark-acceptance"),
    },
    "receipts": rows,
    "result": {
        "receipts": len(rows),
        "identical_verdicts": sum(row["identical_verdict"] for row in rows),
        "identical_summaries": sum(row["identical_summary"] for row in rows),
        "committed_verdict_differs_from_both": [
            row["receipt"]
            for row in rows
            if row["committed_verdict"] not in (None, row["base"]["verdict"], row["head"]["verdict"])
        ],
    },
}
output = root / "dev/active/bdc507a3/verdict-preservation.json"
output.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({key: value for key, value in record["result"].items() if not isinstance(value, list)}))
EOF
