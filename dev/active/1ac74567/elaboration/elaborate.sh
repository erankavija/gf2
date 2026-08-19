#!/usr/bin/env bash
# Elaboration harness for issue 1ac74567 (REQ-04).
#
# Answers "does the Lean that Aeneas run A8b generated elaborate?" for
# dev/active/34d85cb9/extraction/A8b_lean/, and localises what stands in the
# way. Regenerate the committed receipt with:
#
#   ./dev/active/1ac74567/elaboration/elaborate.sh \
#     > dev/active/1ac74567/elaboration/elaborate.log 2>&1
#
# Nothing here writes into proofs/, adds a lake target, or touches the closed
# issue's record under dev/active/34d85cb9/. The generated files are copied
# into a fresh mktemp tree and elaborated with `lake env`, which reads the
# committed proofs/ lake project's environment (Lean 4.30.0-rc2, Mathlib,
# Aeneas) without building or modifying it.
#
# Four stages, each independently exit-coded:
#
#   1. The generated files verbatim.
#   2. After scripts/fix-aeneas-dupes.py, the post-processing pass that
#      scripts/verify-lean.sh already runs on the committed extraction.
#   3. Stage 2 with the pow chain deleted, isolating the two target
#      definitions, plus their axiom dependencies.
#   4. The sketch's lemma statements against the stage-3 tree.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
SRC="$REPO_ROOT/dev/active/34d85cb9/extraction/A8b_lean"
HERE="$REPO_ROOT/dev/active/1ac74567/elaboration"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "repo-root  = $REPO_ROOT"
echo "generated  = $SRC"
echo "scratch    = $WORK"
echo "toolchain  = $(cat "$REPO_ROOT/proofs/lean-toolchain")"
echo "lean       = $(cd "$REPO_ROOT/proofs" && lean --version 2>/dev/null | tail -1)"
echo "lake       = $(cd "$REPO_ROOT/proofs" && lake --version 2>/dev/null | tail -1)"
echo

# Elaborate one module of a staged tree. $1 = stage dir, $2 = module basename.
elab() {
  local dir="$1" mod="$2" rc
  ( cd "$REPO_ROOT/proofs" && lake env bash -c \
      "cd '$dir' && LEAN_PATH=\"\$LEAN_PATH:$dir\" lean -o R8bGf2Core/$mod.olean R8bGf2Core/$mod.lean" )
  rc=$?
  echo "exit=$rc  R8bGf2Core/$mod.lean"
  return $rc
}

stage_tree() {
  local dir="$1"
  mkdir -p "$dir/R8bGf2Core"
  cp "$SRC/Types.lean" "$SRC/Funs.lean" "$dir/R8bGf2Core/"
  cp "$HERE/FunsExternal.lean" "$dir/R8bGf2Core/FunsExternal.lean"
}

echo "=== Stage 1: generated files verbatim ==="
S1="$WORK/s1"; stage_tree "$S1"
elab "$S1" Types
elab "$S1" FunsExternal
elab "$S1" Funs
echo

echo "=== Stage 2: after scripts/fix-aeneas-dupes.py ==="
S2="$WORK/s2"; stage_tree "$S2"
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" \
  "$S2/R8bGf2Core/Types.lean" "$S2/R8bGf2Core/Funs.lean"
echo "--- what the pass changed in Types.lean ---"
diff -u "$SRC/Types.lean" "$S2/R8bGf2Core/Types.lean"
echo "--- what the pass changed in Funs.lean ---"
diff -u "$SRC/Funs.lean" "$S2/R8bGf2Core/Funs.lean"
echo "--- elaboration ---"
elab "$S2" Types
elab "$S2" FunsExternal
elab "$S2" Funs
echo

echo "=== Stage 3: stage 2 with the pow chain deleted ==="
S3="$WORK/s3"; stage_tree "$S3"
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" \
  "$S3/R8bGf2Core/Types.lean" "$S3/R8bGf2Core/Funs.lean"
awk '
  /^\/-- \[gf2_core::field::traits::FiniteFieldExt::pow\]/ { skip = 1 }
  /^\/-- \[gf2_core::field::traits::FiniteFieldExt::frobenius\]: loop body 0:/ { skip = 0 }
  !skip { print }
' "$S3/R8bGf2Core/Funs.lean" > "$S3/Funs.trimmed" && mv "$S3/Funs.trimmed" "$S3/R8bGf2Core/Funs.lean"
echo "--- definitions kept ---"
grep -n '^def ' "$S3/R8bGf2Core/Funs.lean"
echo "--- elaboration ---"
elab "$S3" Types
elab "$S3" FunsExternal
elab "$S3" Funs
echo "--- axiom dependencies of the two targets ---"
cat > "$S3/Axioms.lean" <<'PROBE'
import R8bGf2Core.Funs
#print axioms gf2_core.field.traits.FiniteFieldExt.square.default
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop.body
#print gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop.body._native.decide.ax_3
PROBE
( cd "$REPO_ROOT/proofs" && lake env bash -c \
    "cd '$S3' && LEAN_PATH=\"\$LEAN_PATH:$S3\" lean Axioms.lean" )
echo "exit=$?  Axioms.lean"
echo

echo "=== Stage 4: the sketch's lemma statements against the stage-3 tree ==="
cp "$HERE/statements.lean" "$S3/Statements.lean"
( cd "$REPO_ROOT/proofs" && lake env bash -c \
    "cd '$S3' && LEAN_PATH=\"\$LEAN_PATH:$S3\" lean Statements.lean" )
echo "exit=$?  Statements.lean"
