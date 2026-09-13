#!/usr/bin/env bash
# Record the correctness of every measured path (jit:53c5a8c0).
#
# Usage: ./run-validation.sh
#
# Builds both targeting variants, runs the validator in each, and merges the
# two reports into survey/validation.json. The validator checks every arm path
# against a canonical shift-and-XOR product that shares no code with either
# library, and against long division by the field modulus where the result is
# reduced. It pins each executable and the gf2x library it loaded by content
# digest, so a receipt's arms join to a validated build. Correctness precedes
# timing: the launcher refuses to stage a campaign unless this report passes.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
SURVEY=dev/active/53c5a8c0/survey
OUT="$SURVEY/validation.json"

"$SURVEY/build-arms.sh" >/dev/null

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for variant in conservative native; do
  "$repo/target/53c5a8c0-arms-$variant/release/crossover-validate" "$tmp/$variant.json"
done

python3 - "$OUT" "$tmp/conservative.json" "$tmp/native.json" <<'PY'
import json
import sys

output, *reports = sys.argv[1:]
variants = {}
for path in reports:
    with open(path) as handle:
        report = json.load(handle)
    name = "conservative" if "conservative" in report["executables"]["crossover-arm"]["path"] else "native"
    variants[name] = report
merged = {
    "schema": "clmul-crossover-validation-merged-v1",
    "issue": "53c5a8c0",
    "passed": all(variant["passed"] for variant in variants.values()),
    "variants": variants,
}
with open(output, "w") as handle:
    json.dump(merged, handle, indent=2)
    handle.write("\n")
print(f"{output}: passed={merged['passed']}")
PY
