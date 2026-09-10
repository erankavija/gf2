#!/usr/bin/env bash
# Recomputes each cell's flagged-window fraction per arm (jit:26465e6c).
#
#   dev/active/26465e6c/survey/recount-flagged-windows.sh [receipt-dir ...]
#
# The acceptance tool pools both arms' windows under one median before applying
# the protocol's flagged_window_factor (receipt.rs:1315 collects every window
# into `all_windows`, then `abtest::flagged_windows` takes a single median over
# that pool). When the two arms differ by a large factor the pooled median sits
# between them, so the threshold is loose for the slower arm and tight for the
# faster one, and the reported instability is an artifact of the effect size
# rather than a property of the host.
#
# This script applies the same factor to each arm separately, against that arm's
# own median, and prints both numbers so a stability claim rests on the per-arm
# figure. It reads only committed receipts and changes nothing.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKTREE="$(git -C "${HERE}" rev-parse --show-toplevel)"
cd "${WORKTREE}"
if [[ $# -eq 0 ]]; then
  set -- dev/bench_results/26465e6c/*/
fi

python3 - "$@" <<'PY_RECOUNT'
import json, statistics, sys

FACTOR = 2.0  # protocol flagged_window_factor
MAX_FRACTION = 0.1  # protocol max_flagged_fraction

def flagged(values, center):
    return sum(1 for value in values if value > FACTOR * center)

print(f"per-arm flagged windows at factor {FACTOR} (protocol limit "
      f"{MAX_FRACTION:.0%} of a confirmatory cell's windows)")
for directory in sys.argv[1:]:
    directory = directory.rstrip("/")
    try:
        receipt = json.load(open(f"{directory}/receipt.json"))
    except FileNotFoundError:
        continue
    print(f"\n{directory}  ({receipt['label']})")
    for cell in receipt["cells"]:
        pooled, per_arm = [], {"baseline": [], "candidate": []}
        for pair in cell.get("pairs", []):
            for role in ("baseline", "candidate"):
                values = [window["elapsed_ns"] / window["calls"] for window in pair[role]["windows"]]
                per_arm[role].extend(values)
                pooled.extend(values)
        if not pooled:
            print(f"  {cell['cell_id']}: no samples ({cell.get('unavailable_reason') or cell['status']})")
            continue
        pooled_flagged = flagged(pooled, statistics.median(pooled))
        parts, worst = [], 0.0
        for role in ("baseline", "candidate"):
            values = per_arm[role]
            count = flagged(values, statistics.median(values))
            fraction = count / len(values)
            worst = max(worst, fraction)
            parts.append(f"{role} {count}/{len(values)} ({fraction:.1%})")
        ratio = statistics.median(per_arm["baseline"]) / statistics.median(per_arm["candidate"])
        verdict = "over the limit" if worst > MAX_FRACTION else "within the limit"
        print(f"  {cell['cell_id']}: pooled {pooled_flagged}/{len(pooled)} "
              f"({pooled_flagged / len(pooled):.1%}); per arm {', '.join(parts)}; "
              f"arm ratio {ratio:.4f}x; worst per-arm fraction {verdict}")
PY_RECOUNT
