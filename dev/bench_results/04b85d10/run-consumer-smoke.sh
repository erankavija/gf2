#!/usr/bin/env bash
# Untimed wire smoke of one bit-storage consumer campaign (jit:04b85d10).
#
# Usage: dev/bench_results/04b85d10/run-consumer-smoke.sh <family> pilot|confirmation [version]
#
# Carries the frozen addendum through the same plan derivation, the same
# `benchmark-ab-runner` and the same acceptance tool a campaign uses, at one
# pair and one one-millisecond window per execution, so every arm the plan
# names reaches a result line before a measurement window is spent. The run
# proves the wire contract and makes no performance claim: the plan carries a
# timing override, so the receipt records `settings_deviation` and the
# acceptance tool reports every cell `not-confirmatory`.
#
# Everything the smoke writes stays under `target/`: a copy of the addendum
# whose ledger path points at a scratch ledger, that ledger, the stage and the
# receipt. The committed family ledger and the committed receipts are
# untouched, so the smoke spends no attempt of the real family.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=04b85d10
FAMILY=${1:-}
MODE=${2:-}
VERSION=${3:-4}
usage="usage: $0 logical|count|layout pilot|confirmation [protocol-version]"
case "$FAMILY" in logical|count|layout) ;; *) echo "$usage" >&2; exit 2 ;; esac
case "$MODE" in pilot|confirmation) ;; *) echo "$usage" >&2; exit 2 ;; esac
case "$VERSION" in 3|4) ;; *) echo "$usage" >&2; exit 2 ;; esac

FROZEN=dev/active/$ISSUE/addendum-bit-storage-$FAMILY-v$VERSION-$MODE.json
[[ -f "$FROZEN" ]] || { echo "no frozen addendum $FROZEN" >&2; exit 2; }

SMOKE=target/consumer-smoke-$ISSUE/$FAMILY-v$VERSION-$MODE
rm -rf "$SMOKE"
mkdir -p "$SMOKE"
ADDENDUM=$SMOKE/family-addendum.json
LEDGER=$SMOKE/family-ledger.jsonl

export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
HARNESS_MANIFEST=dev/active/$ISSUE/survey/gf2-side/Cargo.toml
HARNESS_TARGET="$repo/target/consumer-profile-$ISSUE"
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
CARGO_TARGET_DIR="$HARNESS_TARGET" ./scripts/cargo-budget.sh cargo build --release \
  --manifest-path "$HARNESS_MANIFEST"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
ARM=$(realpath "$HARNESS_TARGET/release/consumer-arm")
VERIFY=$(realpath "$HARNESS_TARGET/release/consumer-verify")

# The scratch addendum differs from the frozen one only in its ledger path, so
# the smoke exercises the frozen cells without reserving an attempt of the
# family the frozen ledger tracks.
python3 - "$FROZEN" "$ADDENDUM" "$LEDGER" <<'PY'
import json, sys
frozen, out, ledger = sys.argv[1:]
with open(frozen, encoding="utf-8") as handle:
    document = json.load(handle)
document["family_wise"]["ledger_path"] = ledger
with open(out, "w", encoding="utf-8") as handle:
    json.dump(document, handle, indent=2)
    handle.write("\n")
PY
: >"$LEDGER"

CAMPAIGN="smoke-$MODE-v$VERSION-$FAMILY-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE="$repo/$SMOKE/stage"
PLAN="$SMOKE/plan.json"
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

python3 dev/active/$ISSUE/survey/build-plan.py \
  "$PLAN" "$CAMPAIGN" "$ARM" "$LOCK" "$MODE" "$ADDENDUM" 99

# One pair and one short window per execution: enough for every arm to emit a
# result line, far too little to measure anything.
python3 - "$PLAN" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    plan = json.load(handle)
plan["timing_override"] = {"windows_per_execution": 1, "window_target_ms": 1}
for cell in plan["cells"]:
    cell["pilot_pairs"] = 1
with open(path, "w", encoding="utf-8") as handle:
    json.dump(plan, handle, indent=2)
    handle.write("\n")
PY

echo "# consumer-verify:"
"$VERIFY"

OUT="$SMOKE/receipt"
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
  "$RUNNER" run "$STAGE" "$PLAN"
"$RUNNER" finalize "$STAGE" "$OUT"
set +e
"$ACCEPTANCE" "$OUT"
verdict=$?
set -e
echo "# acceptance exit: $verdict"

# Every arm the plan names must have produced a result line.
python3 - "$PLAN" "$OUT/receipt.json" <<'PY'
import json, sys
plan_path, receipt_path = sys.argv[1:]
with open(plan_path, encoding="utf-8") as handle:
    plan = json.load(handle)
with open(receipt_path, encoding="utf-8") as handle:
    receipt = json.load(handle)
named = set(plan["arms"])
seen = {}
for cell in receipt["cells"]:
    for pair in cell.get("pairs") or []:
        for side in ("baseline", "candidate"):
            seen[pair[side]["arm"]] = pair[side]["selected_path"]
missing = sorted(named - set(seen))
if missing:
    sys.exit(f"no result line from {missing}")
for arm in sorted(seen):
    print(f"# arm {arm}: {seen[arm]}")
print(f"# arms with a result line: {len(seen)}/{len(named)}")
PY
echo "$FAMILY $MODE v$VERSION smoke receipt: $OUT" >&2
