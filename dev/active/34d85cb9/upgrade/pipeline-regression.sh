#!/usr/bin/env bash
# Pipeline regression harness for JIT issue 34d85cb9 (upgrade leg).
#
# Runs the *committed* scripts/verify-lean.sh extraction stages (Steps 1-3b)
# against a chosen Charon/Aeneas pair, writing every artefact into a scratch
# directory so neither proofs/ nor target/charon/ is touched, then diffs the
# regenerated Lean against the committed proofs/.
#
# Step 4 (`lake build`) is deliberately excluded: this worktree has no built
# proofs/.lake and seeding one needs a Mathlib v4.30.0-rc2 fetch. What this
# harness answers is whether extraction still reproduces the committed
# generated Lean, not whether the proofs still elaborate.
#
# Usage:
#   pipeline-regression.sh baseline          # charon/aeneas from PATH
#   pipeline-regression.sh new               # the newly built pair
set -uo pipefail

WHICH="${1:?usage: pipeline-regression.sh <baseline|new>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
OUT="$HERE/pipeline-$WHICH"
GEN="$HERE/verify-lean-$WHICH.sh"

NEW_PREFIX=/data/aeneas-upgrade-34d85cb9
case "$WHICH" in
  baseline) CHARON=charon;                              AENEAS=aeneas ;;
  new)      CHARON="$NEW_PREFIX/aeneas/charon/bin/charon"
            AENEAS="$NEW_PREFIX/aeneas/src/_build/default/main.exe" ;;
  *) echo "unknown leg: $WHICH" >&2; exit 2 ;;
esac

rm -rf "$OUT"
mkdir -p "$OUT/proofs"

# Derive the harness from the committed script by four substitutions:
#   * REPO_ROOT -> the worktree root (the copy sits deeper than scripts/)
#   * PROOFS_DIR / LLBC paths -> the scratch directory
#   * the `charon` and `aeneas` command words -> the chosen binaries
#   * truncate before Step 4 (lake build)
sed -n '1,374p' "$ROOT/scripts/verify-lean.sh" \
  | sed -e "s|^REPO_ROOT=.*|REPO_ROOT=\"$ROOT\"|" \
        -e "s|^PROOFS_DIR=.*|PROOFS_DIR=\"$OUT/proofs\"|" \
        -e "s|^LLBC_FILE=.*|LLBC_FILE=\"$OUT/gf2_core.llbc\"|" \
        -e "s|^LLBC_FILE_ALGEBRA=.*|LLBC_FILE_ALGEBRA=\"$OUT/gf2_algebra.llbc\"|" \
        -e "s|^charon cargo|\"$CHARON\" cargo|" \
        -e "s|^aeneas |\"$AENEAS\" |" \
  >"$GEN"
chmod +x "$GEN"

exec > >(tee "$HERE/logs/pipeline-$WHICH-summary.txt") 2>&1

# Pin the workspace toolchain to the MSRV, matching the .cmd contract of both
# legs. Charon still drives its own `charon toolchain-version` nightly for the
# rustc that produces MIR; this only fixes the cargo/rustc that builds the crate.
export RUSTUP_TOOLCHAIN=1.95.0

echo "=== running $GEN ==="
echo "RUSTUP_TOOLCHAIN=$RUSTUP_TOOLCHAIN  cargo: $(cargo --version)"
"$GEN" >"$HERE/logs/pipeline-$WHICH.log" 2>&1
RC=$?
echo "verify-lean extraction stages exit=$RC"

echo "=== diff regenerated Lean vs committed proofs/ ==="
STATUS=0
for f in Gf2Core/Types.lean Gf2Core/Funs.lean Gf2Algebra/Types.lean Gf2Algebra/Funs.lean; do
  if [ ! -f "$OUT/proofs/$f" ]; then
    echo "MISSING  $f (not generated)"
    STATUS=1
    continue
  fi
  if diff -q "$ROOT/proofs/$f" "$OUT/proofs/$f" >/dev/null 2>&1; then
    echo "IDENTICAL $f"
  else
    N=$(diff "$ROOT/proofs/$f" "$OUT/proofs/$f" | grep -c '^[<>]')
    echo "DIFFERS   $f ($N changed lines)"
    STATUS=1
  fi
done
echo "=== regeneration status: $STATUS (0 = byte-identical to committed) ==="
exit $RC
