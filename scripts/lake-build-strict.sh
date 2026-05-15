#!/usr/bin/env bash
# Strict `lake build` wrapper for the `lake-build` JIT quality gate.
#
# Issue 2e544a34: plain `lake build` exits 0 even when hand-written proofs
# contain `sorry`, because Lean emits `declaration uses 'sorry'` only as a
# warning. Lean's `warningAsError` option is global and would also fail on
# unrelated pre-existing linter noise (deprecated tactics, unused vars,
# `tactic does nothing`); that broader cleanup is out of scope here.
#
# This wrapper enforces the narrow contract:
#   * sorry in any hand-written file under proofs/Gf2Core/Proofs/  → FAIL
#   * sorry in any hand-written file under proofs/Gf2Algebra/Proofs/ → FAIL
#   * sorry anywhere else (Aeneas-generated `Funs.lean`, `FunsExternal.lean`,
#     `Types.lean`, `TypesExternal.lean`, etc.) → tolerated (extraction
#     artefacts, not proof debt)
#   * any other build error or non-sorry warning → propagated through lake's
#     own exit code
#
# Behaviour matches the gate's "lake build" semantics from a result-oriented
# point of view: hand-written-proof sorrys cause the build (as observed by
# the gate) to exit non-zero. Aeneas-generated files keep their pre-existing
# extraction sorrys without breaking the gate, matching the explicit carve-out
# in 2e544a34's success criteria.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROOFS_DIR="$REPO_ROOT/proofs"

cd "$PROOFS_DIR"

# Capture both stdout and stderr; preserve them on the wrapper's stderr/stdout
# so the gate's run history retains the full lake output.
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

if ! lake build 2>&1 | tee "$LOG"; then
  # Plain `lake build` failed. Forward its non-zero exit.
  exit "${PIPESTATUS[0]}"
fi

# `lake build` returned 0; check for sorry warnings in hand-written files.
# The Lean warning is exactly:
#   warning: <path>:<line>:<col>: declaration uses `sorry`
# We grep for that substring, then filter the path component to the two
# hand-written `Proofs/` directories.
HITS="$(grep -E "^warning: [^:]*:[0-9]+:[0-9]+: declaration uses .sorry." "$LOG" \
        | grep -E "^warning: (Gf2Core|Gf2Algebra)/Proofs/" || true)"

if [ -n "$HITS" ]; then
  echo ""
  echo "ERROR: hand-written proofs contain \`sorry\` (issue 2e544a34 gate):"
  echo "$HITS"
  echo ""
  echo "Aeneas-generated files (Funs.lean, FunsExternal.lean, Types.lean,"
  echo "TypesExternal.lean) are exempt from this gate; only Proofs/ paths fail."
  exit 1
fi

echo "lake build OK; no sorry warnings in hand-written Proofs/ files."
exit 0
