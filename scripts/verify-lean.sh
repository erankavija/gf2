#!/usr/bin/env bash
# Regenerate the selected Rust arithmetic and check its Lean proofs.
# Install the pinned upstream pair with scripts/install-lean-tools.sh.
# Extraction limits and post-processing are documented in proofs/WORKAROUNDS.md.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROOFS_DIR="$REPO_ROOT/proofs"
TOOLS_DIR="${LEAN_TOOLS_DIR:-$REPO_ROOT/target/lean-tools}"
for tool in charon charon-driver aeneas; do
  if [ ! -x "$TOOLS_DIR/bin/$tool" ]; then
    echo "ERROR: missing $TOOLS_DIR/bin/$tool; run scripts/install-lean-tools.sh"
    exit 1
  fi
done
export PATH="$TOOLS_DIR/bin:$PATH"
# Generate into empty directories: a failed translator must not reuse committed output.
GENERATED_DIR="$(mktemp -d)"
trap 'rm -rf "$GENERATED_DIR"' EXIT
LLBC_FILE="$REPO_ROOT/target/charon/gf2_core.llbc"
LLBC_FILE_ALGEBRA="$REPO_ROOT/target/charon/gf2_algebra.llbc"

echo "=== Step 1: Charon extraction (gf2-core) ==="
mkdir -p "$(dirname "$LLBC_FILE")"
rm -f "$LLBC_FILE" "$LLBC_FILE_ALGEBRA"

# Extract gf2-core with gfp/ and gfpn/ transparent; everything else opaque or excluded.
# Using --preset aeneas for Aeneas-compatible output.
#
# Preserve the proof-facing config accessor and scalar default-method path.
# Normal Rust builds retain their associated constant and SIMD hooks.
charon cargo \
  --preset aeneas \
  --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_core::gfp' \
  --start-from 'gf2_core::gfpn' \
  --start-from 'gf2_core::gf2m::mul_raw' \
  --opaque 'gf2_core::field' \
  --opaque 'gf2_core::tuning' \
  --opaque 'gf2_core::gf2m::field' \
  --opaque 'gf2_core::gf2m::generation' \
  --opaque 'gf2_core::gf2m::uint_ext' \
  --opaque 'gf2_core::gf2m::thread_safety_tests' \
  --opaque 'gf2_core::gf2m::barrett' \
  --opaque 'gf2_core::gfpn::batch' \
  --opaque 'gf2_core::gfpn::quotient' \
  --opaque 'gf2_core::gfp::simd_ops' \
  --opaque 'gf2_core::gfp::specialized::batch_mul_mersenne31' \
  --opaque 'gf2_core::gfp::specialized::batch_mul_add_mersenne31' \
  --opaque 'gf2_core::gfp::specialized::batch_dot_mersenne31' \
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
echo "=== Step 1b: Charon extraction (gf2-algebra) ==="
#
# Extract gf2-algebra::packed::bipedal3 for the D2 bipedal F_3 correctness
# proof (@/issue/f05ffbe1),
# gf2-algebra::packed::packed5 for the D5 F_5 correctness proof (JIT issue
# @/issue/30e98ef1), and
# gf2-algebra::packed::packed7 for the D6 F_7 correctness proof (JIT issue
# @/issue/30e98ef1). The `f5` and
# `f7` features are enabled so the `#[cfg(feature = "f5")]`-gated packed5 and
# `#[cfg(feature = "f7")]`-gated packed7 modules are compiled into the LLBC.
# Everything else in the crate is opaque — the proofs target only the
# inherent {Bipedal3,Packed5,Packed7}::{add,sub,mul,neg}_inherent wrappers.
#
# D6 Path B (user-chosen 2026-05-16, sketch §4.3 / §8): the three packed7
# 64 KiB compile-time LUTs are extracted as opaque external constants via
# `--opaque 'gf2_algebra::packed::packed7::{ADD,SUB,MUL}_LUT'`. The Lean
# proof axiomatises their contents with a source-faithful characterisation
# cross-validated by the exhaustive Rust `test_*_lut_contract_exhaustive`
# tests; `binary_op_word` + the four `*_correct` theorems are proved against
# the production code path. This is the same `--opaque`/axiom mechanism the
# pipeline already uses for the `gf2_core::gfp` instances. The `build_*_lut`
# `const fn` initialisers are *not* extracted (Path A is out of scope).
#
# gf2_core::* is opaque too: the bipedal3 / packed5 / packed7 arithmetic
# does not reach into Fp / FiniteField machinery at runtime, but Charon
# would otherwise transitively extract those trait impls and surface
# operator wrappers whose bodies are opaque to this narrow extraction.
charon cargo \
  --preset aeneas \
  --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_algebra::packed::bipedal3' \
  --start-from 'gf2_algebra::packed::packed5' \
  --start-from 'gf2_algebra::packed::packed7' \
  --start-from 'gf2_algebra::gray::gray_code_iter' \
  --start-from 'gf2_algebra::gray::gray_code_index_to_subset' \
  --opaque 'gf2_algebra::packed::packed7::ADD_LUT' \
  --opaque 'gf2_algebra::packed::packed7::SUB_LUT' \
  --opaque 'gf2_algebra::packed::packed7::MUL_LUT' \
  --opaque 'gf2_algebra::packed::packed7::build_add_lut' \
  --opaque 'gf2_algebra::packed::packed7::build_sub_lut' \
  --opaque 'gf2_algebra::packed::packed7::build_mul_lut' \
  --opaque 'gf2_algebra::permanent::bipedal3' \
  --opaque 'gf2_algebra::permanent::bipedal3_multiword' \
  --opaque 'gf2_algebra::permanent::ryser' \
  --opaque 'gf2_algebra::permanent::reference' \
  --opaque 'gf2_algebra::permanent::parallel_bipedal3' \
  --opaque 'gf2_algebra::packed::scalar' \
  --opaque 'gf2_algebra::testutil' \
  --opaque 'gf2_core::gfp' \
  --opaque 'gf2_core::gfpn' \
  --opaque 'gf2_core::field' \
  --opaque 'gf2_core::tuning' \
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
  --dest-file "$LLBC_FILE_ALGEBRA" \
  -- --manifest-path "$REPO_ROOT/crates/gf2-algebra/Cargo.toml" --no-default-features --features f5,f7

if [ ! -f "$LLBC_FILE_ALGEBRA" ]; then
  echo "ERROR: Charon did not produce $LLBC_FILE_ALGEBRA"
  exit 1
fi
echo "Charon extraction succeeded: $LLBC_FILE_ALGEBRA"

echo ""
echo "=== Step 2: Aeneas translation (gf2-core) ==="
LEAN_DIR="$GENERATED_DIR/Gf2Core"
mkdir -p "$LEAN_DIR"

# Aeneas may exit 1 when it generates partial files for functions it cannot
# fully translate (currently Debug::fmt implementations).
# We capture the exit code and verify output files were generated.
AENEAS_EXIT=0
aeneas \
  -backend lean \
  -dest "$LEAN_DIR" \
  -split-files \
  "$LLBC_FILE" || AENEAS_EXIT=$?

# Verify that the key output files were actually generated.
MISSING=0
for f in Types.lean Funs.lean FunsExternal_Template.lean TypesExternal_Template.lean; do
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

cp "$LEAN_DIR/TypesExternal_Template.lean" "$LEAN_DIR/TypesExternal.lean"

# Preserve the concrete external models maintained with the proofs.
cp "$PROOFS_DIR/Gf2Core/FunsExternal.lean" "$LEAN_DIR/FunsExternal.lean"

echo "Post-processing done"

echo ""
echo "=== Step 2b: Aeneas translation (gf2-algebra) ==="
LEAN_DIR_ALGEBRA="$GENERATED_DIR/Gf2Algebra"
mkdir -p "$LEAN_DIR_ALGEBRA"

AENEAS_EXIT_ALGEBRA=0
aeneas \
  -backend lean \
  -dest "$LEAN_DIR_ALGEBRA" \
  -split-files \
  "$LLBC_FILE_ALGEBRA" || AENEAS_EXIT_ALGEBRA=$?

MISSING_ALGEBRA=0
for f in Types.lean Funs.lean FunsExternal_Template.lean TypesExternal_Template.lean; do
  if [ ! -f "$LEAN_DIR_ALGEBRA/$f" ]; then
    echo "ERROR: Aeneas did not produce $LEAN_DIR_ALGEBRA/$f"
    MISSING_ALGEBRA=1
  fi
done
if [ "$MISSING_ALGEBRA" -eq 1 ]; then
  echo "ERROR: Aeneas failed to generate gf2-algebra files (exit code $AENEAS_EXIT_ALGEBRA)"
  exit 1
fi
if [ "$AENEAS_EXIT_ALGEBRA" -ne 0 ]; then
  echo "WARNING: Aeneas exited with code $AENEAS_EXIT_ALGEBRA on gf2-algebra (partial files generated — Debug::fmt impls are opaque)"
fi
echo "Aeneas translation (gf2-algebra) completed"

echo ""
echo "=== Step 3b: Post-processing (gf2-algebra) ==="
# Same duplicate-field workaround as gf2-core. The Aeneas extraction of
# gf2-algebra transitively pulls in the FiniteField trait declaration,
# which has the multi-associated-type field-name collisions
# fix-aeneas-dupes.py resolves.
python3 "$REPO_ROOT/scripts/fix-aeneas-dupes.py" \
  "$LEAN_DIR_ALGEBRA/Types.lean" "$LEAN_DIR_ALGEBRA/Funs.lean"

# Replace the transitively-extracted but unresolvable gf2_core::gfp::Fp
# operator wrappers with axioms. The bipedal3 V1 proofs never project
# these instances; axiomatising them eliminates `Unknown constant` errors
# on the imports. The ordinary FiniteField dictionary remains generated.
# See the script's docstring for the full reasoning.
python3 "$REPO_ROOT/scripts/fix-aeneas-gf2algebra.py" "$LEAN_DIR_ALGEBRA/Funs.lean"

# Algebra external declarations have no hand-maintained definitions.
if [ -f "$LEAN_DIR_ALGEBRA/TypesExternal_Template.lean" ]; then
  cp "$LEAN_DIR_ALGEBRA/TypesExternal_Template.lean" "$LEAN_DIR_ALGEBRA/TypesExternal.lean"
fi
cp "$LEAN_DIR_ALGEBRA/FunsExternal_Template.lean" "$LEAN_DIR_ALGEBRA/FunsExternal.lean"
echo "Post-processing (gf2-algebra) done"

echo ""
echo "=== Step 4: Lake build ==="
cd "$PROOFS_DIR"

# The pinned Lake dependency is the same revision used by install-lean-tools.sh.
# Publish only after both translations and post-processing have succeeded.
cp "$LEAN_DIR/"*.lean "$PROOFS_DIR/Gf2Core/"
cp "$LEAN_DIR_ALGEBRA/"*.lean "$PROOFS_DIR/Gf2Algebra/"
"$REPO_ROOT/scripts/lake-build-strict.sh"

echo ""
echo "=== All steps passed ==="
