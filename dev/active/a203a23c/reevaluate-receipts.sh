#!/usr/bin/env bash
# Re-evaluates every committed benchmark receipt from committed content alone.
#
# Usage: dev/active/a203a23c/reevaluate-receipts.sh [<evidence-revision> [<output>]]
#
# Run from any directory of the repository; the tooling crate must have no
# uncommitted change, so HEAD identifies the evaluating source. The evidence
# revision, HEAD by default, is exported and every receipt is evaluated inside
# that export, so an uncommitted or ignored file of the working tree cannot
# supply an input the commit omits — which is what a fresh checkout sees. Naming
# an earlier revision evaluates that evidence with the same evaluator, which is
# how the record of the state before a restoration is produced. The committed
# acceptance summaries are removed from the export, so no committed file is
# written. Receipts are selected by their own declared `schema` field
# (dev/active/a203a23c/select-receipts.py); a receipt of another schema is not
# evaluated and is listed under the record's `skipped` section instead.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
head=$(git rev-parse --verify HEAD)
evidence=$(git rev-parse --verify "${1:-HEAD}^{commit}")
output=${2:-dev/active/a203a23c/receipt-reevaluation.json}
crate=dev/tools/tuning-campaign-support
git diff --quiet HEAD -- "$crate" Cargo.toml Cargo.lock .cargo || {
  echo "$crate or the workspace manifests differ from HEAD" >&2
  exit 1
}

work=target/a203a23c-reevaluation
rm -rf "$work"
mkdir -p "$work/tree"
git archive "$evidence" | tar -x -C "$work/tree"
./scripts/cargo-budget.sh cargo build --locked --release \
  -p tuning-campaign-support --bin benchmark-acceptance
cp target/release/benchmark-acceptance "$work/benchmark-acceptance"

# Selects receipts of the exported evidence by their own declared schema: a
# zen3-benchmark-receipt-v1 receipt is evaluated, any other schema is recorded
# as skipped and never handed to the acceptance evaluator.
python3 dev/active/a203a23c/select-receipts.py "$work/tree" \
  --selected-out "$work/selected.txt" --skipped-out "$work/skipped.tsv"

# One tab-separated row per selected receipt of the evidence revision: receipt,
# exit code. The committed summary is set aside before the evaluator writes
# its own.
: > "$work/exits.tsv"
while IFS= read -r receipt; do
  [ -n "$receipt" ] || continue
  dir="$work/tree/$(dirname "$receipt")"
  [ -e "$dir/acceptance-summary.json" ] &&
    mv "$dir/acceptance-summary.json" "$dir/committed-summary.json"
  rm -f "$dir/acceptance-summary.md"
  status=0
  "$work/benchmark-acceptance" "$dir" > /dev/null 2> "$dir.stderr" || status=$?
  printf '%s\t%s\n' "$receipt" "$status" >> "$work/exits.tsv"
done < "$work/selected.txt"

python3 - "$root" "$work" "$head" "$evidence" "$output" <<'EOF'
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

root, work = Path(sys.argv[1]), Path(sys.argv[2])
head, evidence, output = sys.argv[3], sys.argv[4], sys.argv[5]
tree = root / work / "tree"

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "receipt_inputs", root / "dev/scripts/check-receipt-input-snapshots.py"
)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
absent = {}
for finding in checker.check(root, evidence):
    absent.setdefault(finding.receipt, []).append(finding.path)

# The manifest path `RunnerPlan::producing_manifest_path` substitutes for a plan
# that names none (dev/tools/tuning-campaign-support/src/protocol.rs).
PLAN_MANIFEST_DEFAULT = "dev/active/f547c394/producing-inputs.json"


def verdict_of(path):
    """Reads one acceptance summary of the export, absent summary included."""
    if not path.exists():
        return None, []
    document = json.loads(path.read_text())
    return document["verdict"], document["findings"]


rows = []
for line in (root / work / "exits.tsv").read_text().splitlines():
    receipt, status = line.split("\t")
    receipt_dir = tree / Path(receipt).parent
    document = json.loads((tree / receipt).read_text())
    addendum = json.loads((receipt_dir / document["addendum"]["snapshot"]).read_text())
    committed_verdict, _ = verdict_of(receipt_dir / "committed-summary.json")
    verdict, findings = verdict_of(receipt_dir / "acceptance-summary.json")
    plan = json.loads((receipt_dir / "plan.json").read_text())
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

skipped = []
skipped_path = root / work / "skipped.tsv"
if skipped_path.exists():
    for line in skipped_path.read_text().splitlines():
        if not line:
            continue
        skipped_receipt, schema = line.split("\t", 1)
        skipped.append({"receipt": skipped_receipt, "schema": schema})
skipped.sort(key=lambda row: row["receipt"])

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
    "evidence_revision": evidence,
    "evaluator_revision": head,
    "benchmark_acceptance_sha256": hashlib.sha256(
        (root / work / "benchmark-acceptance").read_bytes()
    ).hexdigest(),
    "receipts": rows,
    "skipped": skipped,
    "result": {
        "receipts": len(rows),
        "reproduces": sum(row["reproduces"] for row in rows),
        "skipped": len(skipped),
        "differs": [row["receipt"] for row in rows if not row["reproduces"]],
    },
}
(root / output).write_text(json.dumps(record, indent=2) + "\n")
print(json.dumps({key: value for key, value in record["result"].items() if not isinstance(value, list)}))
EOF
