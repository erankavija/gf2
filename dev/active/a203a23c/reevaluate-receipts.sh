#!/usr/bin/env bash
# Re-evaluates every committed benchmark receipt from committed content alone
# and writes dev/active/a203a23c/receipt-reevaluation.json.
#
# Usage: dev/active/a203a23c/reevaluate-receipts.sh
#
# Run from any directory of the repository; the tooling crate must have no
# uncommitted change, so HEAD identifies the evaluating source. Each receipt is
# evaluated in a copy exported from HEAD, so an uncommitted or ignored file of
# the working tree cannot supply an input the commit omits — which is what a
# fresh checkout sees. The committed acceptance summaries are removed from the
# copy, so no committed file is written.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
head=$(git rev-parse --verify HEAD)
crate=dev/tools/tuning-campaign-support
git diff --quiet HEAD -- "$crate" Cargo.toml Cargo.lock .cargo || {
  echo "$crate or the workspace manifests differ from HEAD" >&2
  exit 1
}

work=target/a203a23c-reevaluation
rm -rf "$work"
mkdir -p "$work/tree"
git archive "$head" | tar -x -C "$work/tree"
./scripts/cargo-budget.sh cargo build --locked --release \
  -p tuning-campaign-support --bin benchmark-acceptance
cp target/release/benchmark-acceptance "$work/benchmark-acceptance"

# One tab-separated row per receipt: receipt, exit code.
: > "$work/exits.tsv"
git ls-files -z 'dev/bench_results/**/receipt.json' | while IFS= read -r -d '' receipt; do
  case "$receipt" in */inputs/*) continue ;; esac
  dir="$work/tree/$(dirname "$receipt")"
  rm -f "$dir/acceptance-summary.json" "$dir/acceptance-summary.md"
  status=0
  "$work/benchmark-acceptance" "$dir" > /dev/null 2> "$dir.stderr" || status=$?
  printf '%s\t%s\n' "$receipt" "$status" >> "$work/exits.tsv"
done

python3 - "$root" "$work" "$head" <<'EOF'
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

root, work, head = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "receipt_inputs", root / "dev/scripts/check-receipt-input-snapshots.py"
)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
absent = {}
for finding in checker.check(root, head):
    absent.setdefault(finding.receipt, []).append(finding.path)

# The manifest path `RunnerPlan::producing_manifest_path` substitutes for a plan
# that names none (dev/tools/tuning-campaign-support/src/protocol.rs).
PLAN_MANIFEST_DEFAULT = "dev/active/f547c394/producing-inputs.json"


def summary(receipt_dir):
    path = root / work / "tree" / receipt_dir / "acceptance-summary.json"
    if not path.exists():
        return None, []
    document = json.loads(path.read_text())
    return document["verdict"], document["findings"]


rows = []
for line in (root / work / "exits.tsv").read_text().splitlines():
    receipt, status = line.split("\t")
    receipt_dir = str(Path(receipt).parent)
    document = json.loads((root / receipt).read_text())
    addendum = json.loads((root / receipt_dir / document["addendum"]["snapshot"]).read_text())
    committed = root / receipt_dir / "acceptance-summary.json"
    committed_verdict = (
        json.loads(committed.read_text())["verdict"] if committed.exists() else None
    )
    verdict, findings = summary(receipt_dir)
    plan = json.loads((root / receipt_dir / "plan.json").read_text())
    row = {
        "receipt": receipt,
        "protocol_version": addendum["protocol"]["version"],
        "committed_verdict": committed_verdict,
        "fresh_checkout": {
            "exit": int(status),
            "verdict": verdict,
            "errors": sorted(
                {
                    f"{finding['rule']}: {finding['message']}"
                    for finding in findings
                    if finding["severity"] == "error"
                }
            ),
        },
        "reproduces": committed_verdict == verdict,
    }
    if not row["reproduces"]:
        row["cause"] = {
            "pinned_inputs_absent": sorted(absent.get(receipt, [])),
            "producing_manifest": {
                "plan": plan.get("producing_manifest"),
                "receipt": document["source"]["producing"]["manifest_path"],
                "evaluated_default": PLAN_MANIFEST_DEFAULT,
            },
        }
    rows.append(row)
rows.sort(key=lambda row: row["receipt"])

record = {
    "schema": "a203a23c-receipt-reevaluation-v1",
    "purpose": (
        "Acceptance verdict of every committed benchmark receipt when the "
        "evaluator reads committed content alone, beside the verdict of the "
        "acceptance summary committed with it."
    ),
    "generator": "dev/active/a203a23c/reevaluate-receipts.sh",
    "toolchain": subprocess.run(
        ["rustc", "--version"], capture_output=True, text=True, check=True
    ).stdout.strip(),
    "revision": head,
    "benchmark_acceptance_sha256": hashlib.sha256(
        (root / work / "benchmark-acceptance").read_bytes()
    ).hexdigest(),
    "receipts": rows,
    "result": {
        "receipts": len(rows),
        "reproduces": sum(row["reproduces"] for row in rows),
        "differs": [row["receipt"] for row in rows if not row["reproduces"]],
    },
}
output = root / "dev/active/a203a23c/receipt-reevaluation.json"
output.write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({key: value for key, value in record["result"].items() if not isinstance(value, list)}))
EOF
