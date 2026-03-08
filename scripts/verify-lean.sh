#!/usr/bin/env bash
# Aeneas/Charon → Lean4 verification pipeline
#
# Extracts gf2-core field arithmetic (gfp/) to LLBC via Charon,
# translates to Lean4 via Aeneas, and verifies with lake build.
#
# Prerequisites:
#   - charon (v0.1.173, pinned to Aeneas compatibility)
#   - aeneas (built from c23de93)
#   - elan / lean / lake
#
# Usage: ./scripts/verify-lean.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROOFS_DIR="$REPO_ROOT/proofs"
LLBC_FILE="$REPO_ROOT/target/charon/gf2_core.llbc"

echo "=== Step 1: Charon extraction ==="
mkdir -p "$(dirname "$LLBC_FILE")"

# Extract gf2-core with gfp/ and gfpn/ transparent; everything else opaque or excluded.
# Using --preset aeneas for Aeneas-compatible output.
charon cargo \
  --preset aeneas \
  --opaque 'gf2_core::field' \
  --opaque 'gf2_core::gf2m' \
  --opaque 'gf2_core::bitvec' \
  --opaque 'gf2_core::bitslice' \
  --opaque 'gf2_core::matrix' \
  --opaque 'gf2_core::sparse' \
  --opaque 'gf2_core::alg' \
  --opaque 'gf2_core::compute' \
  --opaque 'gf2_core::kernels' \
  --opaque 'gf2_core::primitive_polys' \
  --opaque 'gf2_core::io' \
  --opaque 'gf2_core::macros' \
  --dest-file "$LLBC_FILE" \
  -- --manifest-path "$REPO_ROOT/crates/gf2-core/Cargo.toml" --no-default-features

if [ ! -f "$LLBC_FILE" ]; then
  echo "ERROR: Charon did not produce $LLBC_FILE"
  exit 1
fi
echo "Charon extraction succeeded: $LLBC_FILE"

echo ""
echo "=== Step 2: Aeneas translation ==="
LEAN_DIR="$PROOFS_DIR/Gf2Core"
mkdir -p "$LEAN_DIR"

aeneas \
  -backend lean \
  -dest "$LEAN_DIR" \
  -split-files \
  "$LLBC_FILE"

echo "Aeneas translation succeeded"

echo ""
echo "=== Step 3: Post-processing ==="
# Workaround: Aeneas generates duplicate field names in the FiniteField struct
# when a trait has bounds on multiple associated types (Self, Characteristic, Wide).
# See proofs/WORKAROUNDS.md for details.
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" "$LEAN_DIR/Types.lean" "$LEAN_DIR/Funs.lean"

# Always regenerate FunsExternal.lean from template (all entries are axioms)
cp "$LEAN_DIR/FunsExternal_Template.lean" "$LEAN_DIR/FunsExternal.lean"

echo "Post-processing done"

echo ""
echo "=== Step 4: Lake build ==="
cd "$PROOFS_DIR"
lake build

echo ""
echo "=== All steps passed ==="
