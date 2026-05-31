#!/usr/bin/env bash
# Aeneas/Charon → Lean4 verification pipeline
#
# Extracts gf2-core field arithmetic (gfp/) to LLBC via Charon,
# translates to Lean4 via Aeneas, and verifies with lake build.
#
# Prerequisites:
#   - charon (upstream e069223a + 4 project-local patches; built with rustc
#     nightly-2026-02-22 per charon/rust-toolchain)
#   - aeneas (upstream 0f99a049, unmodified)
#   - elan / lean / lake
#
# Usage: ./scripts/verify-lean.sh
#
# Toolchain (as of issue 150d7d79, 2026-05-31):
#   * Charon: upstream HEAD `e069223a` (the commit pinned by Aeneas main via
#     `/data/aeneas-build/charon-pin`) + 4 project-local patches (HRTB-erase,
#     SelfClause/Local(0,0) fallback in `lookup_type_replacement`,
#     implied-clause constraint propagation — all in
#     `expand_associated_types.rs` — plus the obsolete-asserts removal in
#     `pretty/fmt_with_ctx.rs` `DynPredicate::fmt_with_ctx`). All four patches
#     still apply cleanly and are still required at e069223a. Patches preserved
#     at `dev/active/charon-patch-backup-2026-05-15/` (rebased copy:
#     `expand_and_fmt-rebased-150d7d79.patch`).
#   * Aeneas: upstream HEAD `0f99a049` (tag nightly-2026.05.30), unmodified.
#   * Lean: v4.30.0-rc2; Mathlib: v4.30.0-rc2 (see `proofs/lean-toolchain`,
#     `proofs/lakefile.lean`) — unchanged across the 150d7d79 upgrade.
#
# Workarounds in place (issue 9efd9c39):
#   * `crates/gf2-core/src/gfp/mod.rs` — 11 SIMD-fast-path overrides on the
#     `Fp::FiniteField` impl wrapped in `#[cfg(not(verify_lean))]` so the
#     trait defaults are used during extraction. Same pattern as the
#     existing `verify_lean` cfg for `ExtConfig::NON_RESIDUE`.
#   * `--opaque 'gf2_core::gfp::simd_ops'` below: keeps the SIMD-ops
#     module out of the LLBC.
#   * `--opaque` on the three `gf2_core::gfp::specialized::batch_*_mersenne31`
#     functions (added in 150d7d79): Aeneas 0f99a049 no longer ships the slice
#     iterator `.zip` instances those functions extract to. They are batch SIMD
#     helpers, not proof targets, so opaquing only those three (not the whole
#     `specialized` module — its scalar reductions are called by transparent
#     gfp code) keeps the extraction clean. Minor coverage delta vs the prior
#     trio, which extracted them transparently.
#   * `DEFAULT_CONST_BODIES['PLE_PANEL_COLS']` in fix-aeneas-dupes.py (150d7d79):
#     `const PLE_PANEL_COLS: usize = Self::PLE_BASE_COLS` (= 1 by default) is
#     referenced via the unemitted `.default` sibling for non-overriding
#     instances; inlined the same way as PLE_BASE_COLS.
#   * `proofs/Gf2Core/FunsExternal.lean` (150d7d79): the hand-written
#     `core.num.U64.overflowing_sub` override was REMOVED — Aeneas 0f99a049 now
#     provides it natively as a pure `U64 → U64 → (U64 × Bool)` (consumed via
#     `lift`); the old `Result`-returning override conflicted ("expected a
#     product type"). `wrapping_neg` is still hand-provided (Std has no native).
#   * `scripts/fix-aeneas-dupes.py` `inline_default_methods()` pass:
#     post-processes Aeneas-generated `Funs.lean` files to inline trait
#     defaults at instance-dictionary call sites (Aeneas at upstream HEAD
#     does not emit sibling defs for default trait methods; tracked by
#     9efd9c39's aspirational criterion for upstream resolution).
#   * `scripts/fix-aeneas-dupes.py` `silence_extraction_sorry()` pass:
#     injects `set_option warn.sorry false` into generated `Funs.lean`
#     so Aeneas extraction-artefact sorrys do not trip the strict
#     `lake-build` gate (added in issue 2e544a34).
#
# This script runs end-to-end clean on the toolchain above. If a step
# fails on `main`, file an issue rather than reverting the workarounds —
# they are load-bearing for the current Charon/Aeneas/Lean trio.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROOFS_DIR="$REPO_ROOT/proofs"
LLBC_FILE="$REPO_ROOT/target/charon/gf2_core.llbc"
LLBC_FILE_ALGEBRA="$REPO_ROOT/target/charon/gf2_algebra.llbc"

echo "=== Step 1: Charon extraction (gf2-core) ==="
mkdir -p "$(dirname "$LLBC_FILE")"

# Extract gf2-core with gfp/ and gfpn/ transparent; everything else opaque or excluded.
# Using --preset aeneas for Aeneas-compatible output.
#
# Narrow Charon workaround: enable cfg(verify_lean) so ExtConfig exposes
# NON_RESIDUE() as a trait method instead of an associated const during extraction.
# Charon 0.1.173 rejects associated consts whose type is an associated type of
# Self (Self::BaseField) before Aeneas sees the arithmetic. This keeps gfp/gfpn
# production arithmetic transparent; only the config-level beta accessor shape is
# changed for extraction and can be removed when Charon handles that pattern.
charon cargo \
  --preset aeneas \
  --rustc-arg=--cfg=verify_lean \
  --start-from 'gf2_core::gfp' \
  --start-from 'gf2_core::gfpn' \
  --start-from 'gf2_core::gf2m::mul_raw' \
  --opaque 'gf2_core::field' \
  --opaque 'gf2_core::gf2m::field' \
  --opaque 'gf2_core::gf2m::generation' \
  --opaque 'gf2_core::gf2m::uint_ext' \
  --opaque 'gf2_core::gf2m::thread_safety_tests' \
  --opaque 'gf2_core::gf2m::barrett' \
  --opaque 'gf2_core::gfpn::batch' \
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
# proof (JIT issue f05ffbe1; sketch at dev/plans/d2_lean_bipedal3_sketch.md),
# gf2-algebra::packed::packed5 for the D5 F_5 correctness proof (JIT issue
# 30e98ef1; sketch at dev/plans/d5_lean_packed5_sketch.md), and
# gf2-algebra::packed::packed7 for the D6 F_7 correctness proof (JIT issue
# 30e98ef1; sketch at dev/plans/d6_lean_packed7_sketch.md). The `f5` and
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
# unresolvable recursive defaults.
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

# The narrowed extraction can leave an opaque gf2m external type in
# TypesExternal.lean while also generating the out-of-scope UintExt trait in
# Types.lean. Keep the opaque external axiom only; no verified gfp/gfpn code
# projects fields from UintExt.
python3 - "$LEAN_DIR/Types.lean" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
text = re.sub(
    r"/-- Trait declaration: \[gf2_core::gf2m::uint_ext::private::Sealed\][\s\S]*?"
    r"structure gf2m\.uint_ext\.private\.Sealed \(Self : Type\) where\n\n",
    "",
    text,
    count=1,
)
text = re.sub(
    r"/-- Trait declaration: \[gf2_core::gf2m::uint_ext::UintExt\][\s\S]*?"
    r"  from_u16 : Std\.U16 → Result Self\n\n",
    "",
    text,
    count=1,
)
path.write_text(text)
PY

# Workaround: Aeneas (0f99a049) cannot translate some gfpn function bodies from
# Charon (e069223a) LLBC. Restore known-good implementations from previous
# working extraction. See fix-aeneas-sorrys.py docstring for details.
python3 "$REPO_ROOT/scripts/fix-aeneas-sorrys.py" "$LEAN_DIR/Funs.lean"

# TypesExternal.lean contains auto-generated type axioms (no hand-editing needed).
# Always regenerate from template when present.
if [ -f "$LEAN_DIR/TypesExternal_Template.lean" ]; then
  cp "$LEAN_DIR/TypesExternal_Template.lean" "$LEAN_DIR/TypesExternal.lean"
  # The narrowed Charon start set can emit an opaque Gf2mElement_ external type
  # whose signature mentions the sealed UintExt trait outside the extraction
  # target. Add only that opaque external type dependency.
  python3 - "$LEAN_DIR/TypesExternal.lean" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
axiom = """\
/-- Opaque declaration for the sealed GF(2^m) integer-width trait.
    Added by scripts/verify-lean.sh for the narrowed Charon start-from set. -/
axiom gf2m.uint_ext.UintExt (Self : Type) : Type

"""
marker = "/-- [gf2_core::gf2m::field::Gf2mElement_]"
if "axiom gf2m.uint_ext.UintExt" not in text and marker in text:
    path.write_text(text.replace(marker, axiom + marker, 1))
PY
fi

# FunsExternal.lean contains hand-edited concrete definitions (wrapping_neg,
# overflowing_sub, U128 add/add_assign) that replace Aeneas axioms.
# Only seed from template on first run; never overwrite existing file.
if [ ! -f "$LEAN_DIR/FunsExternal.lean" ]; then
  cp "$LEAN_DIR/FunsExternal_Template.lean" "$LEAN_DIR/FunsExternal.lean"
  echo "NOTE: FunsExternal.lean seeded from template — fill in concrete defs"
fi

echo "Post-processing done"

echo ""
echo "=== Step 2b: Aeneas translation (gf2-algebra) ==="
LEAN_DIR_ALGEBRA="$PROOFS_DIR/Gf2Algebra"
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
# trait-impl wrappers with axioms. The bipedal3 V1 proofs never project
# these instances; axiomatising them eliminates `Unknown constant` /
# `could not resolve recursive fields` errors on the imports. See the
# script's docstring for the full reasoning.
python3 "$REPO_ROOT/scripts/fix-aeneas-gf2algebra.py" "$LEAN_DIR_ALGEBRA/Funs.lean"

# TypesExternal / FunsExternal seed (no hand-edits required for the
# bipedal3 V1 — the four bitwise ops do not exercise any U128 /
# wrapping_neg / overflowing_sub external).
if [ -f "$LEAN_DIR_ALGEBRA/TypesExternal_Template.lean" ]; then
  cp "$LEAN_DIR_ALGEBRA/TypesExternal_Template.lean" "$LEAN_DIR_ALGEBRA/TypesExternal.lean"
fi
# Unlike gf2-core, the gf2-algebra FunsExternal has no hand-edits (the
# bipedal3 ops use only bitwise primitives, no wrapping arithmetic), so we
# always regenerate from the template.
cp "$LEAN_DIR_ALGEBRA/FunsExternal_Template.lean" "$LEAN_DIR_ALGEBRA/FunsExternal.lean"
echo "Post-processing (gf2-algebra) done"

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
