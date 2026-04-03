#!/usr/bin/env bash
# Aeneas/Charon → Lean4 verification pipeline
#
# Extracts gf2-core field arithmetic (gfp/) to LLBC via Charon,
# translates to Lean4 via Aeneas, and verifies with lake build.
#
# Prerequisites:
#   - charon (v0.1.173, pinned to Aeneas compatibility)
#   - aeneas (built from 1180be60)
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
  --opaque 'gf2_core::gf2m::field' \
  --opaque 'gf2_core::gf2m::generation' \
  --opaque 'gf2_core::gf2m::uint_ext' \
  --opaque 'gf2_core::gf2m::thread_safety_tests' \
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

# Aeneas may exit 1 when it generates partial files for functions it cannot
# fully translate (e.g. gfpn arithmetic with complex trait hierarchies).
# We capture the exit code and verify output files were generated.
AENEAS_EXIT=0
aeneas \
  -backend lean \
  -dest "$LEAN_DIR" \
  -split-files \
  "$LLBC_FILE" || AENEAS_EXIT=$?

# Verify that the key output files were actually generated.
MISSING=0
for f in Types.lean Funs.lean FunsExternal_Template.lean; do
  if [ ! -f "$LEAN_DIR/$f" ]; then
    echo "ERROR: Aeneas did not produce $LEAN_DIR/$f"
    MISSING=1
  fi
done
if [ "$MISSING" -eq 1 ]; then
  echo "ERROR: Aeneas failed to generate required files (exit code $AENEAS_EXIT)"
  exit 1
fi
if [ "$AENEAS_EXIT" -ne 0 ]; then
  echo "WARNING: Aeneas exited with code $AENEAS_EXIT (partial files generated — some function bodies are opaque)"
fi
echo "Aeneas translation completed"

echo ""
echo "=== Step 3: Post-processing ==="
# Workaround: Aeneas generates duplicate field names in the FiniteField struct
# when a trait has bounds on multiple associated types (Self, Characteristic, Wide).
# See proofs/WORKAROUNDS.md for details.
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" "$LEAN_DIR/Types.lean" "$LEAN_DIR/Funs.lean"

# Workaround: Aeneas (1180be60) cannot translate 16 gfpn function bodies from
# Charon (419f53b6+) LLBC. Restore known-good implementations from previous
# working extraction. See fix-aeneas-sorrys.py docstring for details.
python3 "$REPO_ROOT/scripts/fix-aeneas-sorrys.py" "$LEAN_DIR/Funs.lean"

# FunsExternal.lean contains hand-edited concrete definitions (wrapping_neg,
# overflowing_sub, U128 add/add_assign) that replace Aeneas axioms.
# Only seed from template on first run; never overwrite existing file.
if [ ! -f "$LEAN_DIR/FunsExternal.lean" ]; then
  cp "$LEAN_DIR/FunsExternal_Template.lean" "$LEAN_DIR/FunsExternal.lean"
  echo "NOTE: FunsExternal.lean seeded from template — fill in concrete defs"
fi

echo "Post-processing done"

echo ""
echo "=== Step 4: Lake build ==="
cd "$PROOFS_DIR"

# If AENEAS_LEAN_DIR is set (e.g. in CI), patch lakefile.lean and
# lake-manifest.json to point there instead of the local dev path.
if [ -n "${AENEAS_LEAN_DIR:-}" ]; then
  echo "Patching aeneas path → $AENEAS_LEAN_DIR"
  sed -i "s|require aeneas from .*|require aeneas from \"$AENEAS_LEAN_DIR\"|" lakefile.lean
  sed -i "s|\"dir\": \".*backends/lean\"|\"dir\": \"$AENEAS_LEAN_DIR\"|" lake-manifest.json
  rm -rf .lake/packages/aeneas
fi

lake build

echo ""
echo "=== All steps passed ==="
