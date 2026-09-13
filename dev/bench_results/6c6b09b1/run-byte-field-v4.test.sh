#!/usr/bin/env bash
# Outcome guards of run-byte-field-v4.sh (jit:6c6b09b1).
#
# Usage: dev/bench_results/6c6b09b1/run-byte-field-v4.test.sh [repo-root]
#
# The launcher must tell three outcomes apart: a campaign that produced data,
# one that completed and produced none because every cell was inapplicable on
# this host, and one that did not complete at all. Two of those are cheap to
# synthesize and rare to observe, so this covers them with constructed journals
# and receipts rather than waiting for a host to produce one. Both guards are
# sourced verbatim out of the launcher, so the test runs the shipped code.
#
# The real artifacts pin the synthetic ones to reality: the failed version-3
# region-axpy stage of 2026-09-12 and the committed version-1 pilot receipt are
# checked alongside.
set -uo pipefail
cd "${1:-$(git rev-parse --show-toplevel)}"
T=$(mktemp -d)
sed -n '/^require_measured_journal() {$/,/^}$/p;/^require_measured_receipt() {$/,/^}$/p' \
  dev/bench_results/6c6b09b1/run-byte-field-v4.sh > "$T/guards.sh"
source "$T/guards.sh"

cat > "$T/mkjournal.py" <<'PY'
import json, sys
file, terminal, measured, *pairs = sys.argv[1:]
pairs = [int(x) for x in pairs]
rows, seq = [], 0
def add(event, details, case=None):
    global seq
    row = {"schema": "tuning-campaign-journal-v1", "campaign_id": "synthetic",
           "session_id": "session-1", "sequence": seq, "event": event,
           "details": details}
    if case is not None:
        row["case"] = case
    rows.append(row); seq += 1
add("campaign-start", {})
for i, n in enumerate(pairs):
    cid = f"cell-{i}"
    add("cell-complete", {"status": "measured" if n else "unavailable",
                          "pairs": n, "claimed": None},
        {"cell_id": cid, "key": cid})
add(terminal, {"measured_cells": int(measured)} if terminal == "complete"
    else {"error": "synthetic non-completion"})
with open(file, "w") as h:
    for row in rows:
        h.write(json.dumps(row) + "\n")
PY

cat > "$T/mkreceipt.py" <<'PY'
import json, sys
path, *pairs = sys.argv[1:]
cells = [{"cell_id": f"cell-{i}",
          "status": "measured" if int(n) else "unavailable",
          "unavailable_reason": None if int(n) else "core arm unresolvable on this host",
          "pairs": [{"baseline": {}, "candidate": {}} for _ in range(int(n))]}
         for i, n in enumerate(pairs)]
json.dump({"cells": cells}, open(path, "w"))
PY

journal() { mkdir -p "$1"; python3 "$T/mkjournal.py" "$1/execution.log" "${@:2}"; }
receipt() { mkdir -p "$1"; python3 "$T/mkreceipt.py" "$1/receipt.json" "${@:2}"; }

fails=0
run() {  # run NAME EXPECTED_RC COMMAND...
  local name=$1 want=$2; shift 2
  local out rc
  out=$("$@" 2>&1); rc=$?
  if [ "$rc" = "$want" ]; then printf 'PASS  rc=%s  %-42s %s\n' "$rc" "$name" "$out"
  else printf 'FAIL  rc=%s want=%s  %-34s %s\n' "$rc" "$want" "$name" "$out"; fails=1; fi
}

echo "--- journal guard"
journal "$T/mixed"     complete 3 12 0 12
STAGE=$T/mixed     run "measured + host-unavailable mix"    0 require_measured_journal
journal "$T/unavail"   complete 3 0 0 0
STAGE=$T/unavail   run "every cell host-unavailable"        4 require_measured_journal
journal "$T/resumed"   complete 0 12 12
STAGE=$T/resumed   run "resumed: final session attempted 0" 0 require_measured_journal
journal "$T/nocells"   complete 0
STAGE=$T/nocells   run "completed with no cell-complete"    4 require_measured_journal
journal "$T/failed"    failed   1 0
STAGE=$T/failed    run "campaign did not complete"          1 require_measured_journal
# The version-3 stage lives under target/ and is not committed, so this case
# runs only on the host that produced it.
STAGE=target/6c6b09b1-campaigns/6c6b09b1-v3-r1-region-axpy-pilot
if [ -f "$STAGE/execution.log" ]; then
  run "REAL failed v3 region-axpy stage"                    1 require_measured_journal
else
  printf 'SKIP        %-42s stage not present\n' "REAL failed v3 region-axpy stage"
fi

echo "--- receipt guard"
receipt "$T/r-mixed"   12 0 12; run "measured + host-unavailable mix" 0 require_measured_receipt "$T/r-mixed"
receipt "$T/r-unavail" 0 0 0;   run "every cell host-unavailable"     4 require_measured_receipt "$T/r-unavail"
receipt "$T/r-one"     12;      run "one measured cell"               0 require_measured_receipt "$T/r-one"
run "REAL committed v1 pilot receipt" 0 require_measured_receipt dev/bench_results/6c6b09b1/2026-09-08-6c6b09b1-byte-field-pilot

rm -rf "$T"
echo; [ "$fails" = 0 ] && echo "all guard cases behaved as specified" || echo "SOME CASES FAILED"
exit "$fails"
