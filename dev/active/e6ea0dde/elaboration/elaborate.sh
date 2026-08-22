#!/usr/bin/env bash
# Elaboration harness for JIT issue e6ea0dde.
#
# Answers "does the Lean that Aeneas run AX3 generated elaborate as generated?"
# for dev/active/e6ea0dde/extraction/AX3_lean/. Regenerate the committed
# receipt with:
#
#   ./dev/active/e6ea0dde/elaboration/elaborate.sh \
#     > dev/active/e6ea0dde/elaboration/elaborate.log 2>&1
#
# The generated files are copied into a fresh mktemp tree and elaborated with
# `lake env`, which reads the committed proofs/ lake project's environment
# (Lean 4.30.0-rc2, Mathlib, Aeneas) without building or modifying it. Nothing
# here writes into proofs/, adds a lake target, or touches a closed issue's
# record.
#
# Three stages, each independently exit-coded:
#
#   1. THE DELIVERABLE. The generated files exactly as Aeneas wrote them. The
#      only filesystem operation is the rename `FunsExternal_Template.lean` ->
#      `FunsExternal.lean` that the template's own generated header instructs;
#      the stage asserts byte-identity across that rename with `cmp`.
#   2. DIAGNOSTIC ONLY, and outside this issue's contract. Stage 1 with
#      scripts/fix-aeneas-dupes.py applied, to localise what stage 1 hits and
#      to read off the axiom dependencies of the two target definitions. A tree
#      repaired by a script is not an answer to "elaborates as generated"; this
#      stage exists so the blocking defect is bounded, not to substitute for
#      stage 1.
#   3. DIAGNOSTIC ONLY. The proof sketch's lemma statements against the stage-2
#      tree.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
SRC="$REPO_ROOT/dev/active/e6ea0dde/extraction/AX3_lean"
HERE="$REPO_ROOT/dev/active/e6ea0dde/elaboration"
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
      "cd '$dir' && LEAN_PATH=\"\$LEAN_PATH:$dir\" lean -o X3Gf2Core/$mod.olean X3Gf2Core/$mod.lean" )
  rc=$?
  echo "exit=$rc  X3Gf2Core/$mod.lean"
  return $rc
}

# Stage a copy of the generated tree. The template rename is the only step.
stage_tree() {
  local dir="$1"
  mkdir -p "$dir/X3Gf2Core"
  cp "$SRC/Types.lean" "$SRC/Funs.lean" "$dir/X3Gf2Core/"
  cp "$SRC/FunsExternal_Template.lean" "$dir/X3Gf2Core/FunsExternal.lean"
}

echo "=== Stage 1: the generated files as generated ==="
S1="$WORK/s1"; stage_tree "$S1"
echo "--- the template rename changes no byte ---"
cmp "$SRC/FunsExternal_Template.lean" "$S1/X3Gf2Core/FunsExternal.lean" \
  && echo "cmp: FunsExternal_Template.lean == FunsExternal.lean"
echo "--- the staged tree is byte-identical to the generated one ---"
for f in Types.lean Funs.lean; do
  cmp "$SRC/$f" "$S1/X3Gf2Core/$f" && echo "cmp: $f unchanged"
done
echo "--- definitions present ---"
grep -n '^def ' "$S1/X3Gf2Core/Funs.lean"
echo "--- sorry occurrences in the generated tree ---"
grep -c 'sorry' "$S1/X3Gf2Core/Funs.lean" "$S1/X3Gf2Core/Types.lean" \
  "$S1/X3Gf2Core/FunsExternal.lean"
echo "--- elaboration ---"
elab "$S1" Types
elab "$S1" FunsExternal
elab "$S1" Funs
echo

echo "=== Stage 2 (DIAGNOSTIC): stage 1 after scripts/fix-aeneas-dupes.py ==="
S2="$WORK/s2"; stage_tree "$S2"
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" \
  "$S2/X3Gf2Core/Types.lean" "$S2/X3Gf2Core/Funs.lean"
echo "--- what the pass changed in Types.lean ---"
diff -u "$SRC/Types.lean" "$S2/X3Gf2Core/Types.lean"
echo "--- what the pass changed in Funs.lean ---"
diff -u "$SRC/Funs.lean" "$S2/X3Gf2Core/Funs.lean"
echo "--- elaboration ---"
elab "$S2" Types
elab "$S2" FunsExternal
elab "$S2" Funs
echo "--- axiom dependencies of the two targets ---"
cat > "$S2/Axioms.lean" <<'PROBE'
import X3Gf2Core.Funs
#print axioms gf2_core.field.traits.FiniteFieldExt.square.default
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop
#print axioms gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop.body
#print gf2_core.field.traits.FiniteFieldExt.frobenius.default_loop.body._native.decide.ax_3
PROBE
( cd "$REPO_ROOT/proofs" && lake env bash -c \
    "cd '$S2' && LEAN_PATH=\"\$LEAN_PATH:$S2\" lean Axioms.lean" )
echo "exit=$?  Axioms.lean"
echo

echo "=== Stage 3 (DIAGNOSTIC): the sketch's lemma statements against stage 2 ==="
cp "$HERE/Statements.lean" "$S2/Statements.lean"
( cd "$REPO_ROOT/proofs" && lake env bash -c \
    "cd '$S2' && LEAN_PATH=\"\$LEAN_PATH:$S2\" lean Statements.lean" )
echo "exit=$?  Statements.lean"
