#!/usr/bin/env bash
# Builds the two release arms of the YMM raw-batch dispatch A/B (jit:1d0da41f).
#
# Both arms are the same adapter source (survey/ymm-clmul-arm). They differ
# only in the gf2 source tree they link:
#
#   candidate — the repaired crates of this checkout;
#   baseline  — a pinned snapshot of the pre-change crates, built with the
#               `frozen-baseline` feature so the arm reports the lane the
#               pre-change dispatch expression selects.
#
# Nothing is built inside the checkout: the staged baseline project and both
# target directories live under the staging root.
#
# Environment:
#   GF2_BASELINE_TREE  required; a checkout of the pre-change source tree,
#                      created with `git archive <pre-change-commit> | tar -x`.
#   GF2_ARM_STAGE      staging root (default: ${TMPDIR:-/tmp}/gf2-1d0da41f-arms).
#
# Every cargo invocation goes through scripts/cargo-budget.sh, which takes the
# shared side of the benchmark mutex, so a build can never overlap a timed run.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"

: "${GF2_BASELINE_TREE:?set GF2_BASELINE_TREE to the pre-change source tree}"
BASELINE_TREE=$(realpath "$GF2_BASELINE_TREE")
STAGE=${GF2_ARM_STAGE:-${TMPDIR:-/tmp}/gf2-1d0da41f-arms}
mkdir -p "$STAGE"
STAGE=$(realpath "$STAGE")

ADAPTER="$repo/dev/active/1d0da41f/survey/ymm-clmul-arm"
for crate in crates/gf2-core crates/gf2-kernels-simd; do
    [[ -f "$BASELINE_TREE/$crate/Cargo.toml" ]] ||
        { echo "baseline tree lacks $crate" >&2; exit 2; }
done
if grep -q 'is_x86_feature_detected!("avx2")' \
        "$BASELINE_TREE/crates/gf2-kernels-simd/src/x86/clmul.rs"; then
    echo "baseline tree already carries the repaired dispatch predicate" >&2
    exit 2
fi

# Stage the baseline arm: same adapter source, path dependencies repointed at
# the pre-change crates. The protocol tooling stays this checkout's copy so
# both arms speak the same child-v2 contract.
BASELINE_PROJECT="$STAGE/arm-baseline"
rm -rf "$BASELINE_PROJECT"
mkdir -p "$BASELINE_PROJECT"
cp -r "$ADAPTER/src" "$BASELINE_PROJECT/src"
cp "$ADAPTER/Cargo.lock" "$BASELINE_PROJECT/Cargo.lock"
sed -e "s#path = \"../../../../../crates/gf2-core\"#path = \"$BASELINE_TREE/crates/gf2-core\"#" \
    -e "s#path = \"../../../../../crates/gf2-kernels-simd\"#path = \"$BASELINE_TREE/crates/gf2-kernels-simd\"#" \
    -e "s#path = \"../../../../tools/tuning-campaign-support\"#path = \"$repo/dev/tools/tuning-campaign-support\"#" \
    "$ADAPTER/Cargo.toml" >"$BASELINE_PROJECT/Cargo.toml"
grep -q "$BASELINE_TREE/crates/gf2-core" "$BASELINE_PROJECT/Cargo.toml" ||
    { echo "baseline manifest rewrite failed" >&2; exit 2; }

"$repo/scripts/cargo-budget.sh" cargo build --locked --release \
    --manifest-path "$ADAPTER/Cargo.toml" \
    --target-dir "$STAGE/target-candidate"
"$repo/scripts/cargo-budget.sh" cargo build --locked --release \
    --manifest-path "$BASELINE_PROJECT/Cargo.toml" \
    --features frozen-baseline \
    --target-dir "$STAGE/target-baseline"

CANDIDATE="$STAGE/target-candidate/release/ymm-clmul-arm"
BASELINE="$STAGE/target-baseline/release/ymm-clmul-arm"
for binary in "$CANDIDATE" "$BASELINE"; do
    [[ -x "$binary" ]] || { echo "missing arm binary $binary" >&2; exit 1; }
done
echo "candidate arm: $CANDIDATE"
echo "baseline arm : $BASELINE"
sha256sum "$CANDIDATE" "$BASELINE"
