#!/usr/bin/env bash
# Checks the `CARGO_CI_STEPS` filter of `scripts/cargo-ci.sh` (jit:316150fd):
# a filtered run lists every skipped step, states the filter and the skipped
# count, and still runs the matching step; a run without the variable prints
# neither. The unfiltered case runs a copy of the script reduced to one
# python step, so no cargo step builds.
#
# Usage: check-ci-step-filter.sh
set -euo pipefail

root=$(git -C "$(dirname -- "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
cd "$root"
script=scripts/cargo-ci.sh
steps=$(grep -c '^run_step ' "$script")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }

out=$(CARGO_CI_STEPS='^$' "$script")
[ "$(grep -c 'skipped by CARGO_CI_STEPS' <<<"$out")" -eq "$steps" ] || fail "not every step is listed as skipped"
grep -q "skipped $steps steps; this is not the CI contract's verdict" <<<"$out" || fail "no filtered-run line"

out=$(CARGO_CI_STEPS='^repository-files-self-test$' "$script")
grep -q '✓ repository-files-self-test' <<<"$out" || fail "the matching step did not run"
grep -q "skipped $((steps - 1)) steps" <<<"$out" || fail "wrong skipped count"

# Keep the one python step, placed before the closing fixture check.
python3 - "$script" "$work/cargo-ci.sh" <<'PY'
import sys
src, dst = sys.argv[1:]
text = open(src).read()
steps = [l for l in text.splitlines(keepends=True) if l.startswith("run_step repository-files-self-test ")]
head = "".join(l for l in text.splitlines(keepends=True) if not l.startswith("run_step "))
marker = "fixtures_after=$(fixture_inventory)"
open(dst, "w").write(head.replace(marker, "".join(steps) + marker, 1))
PY
out=$(bash "$work/cargo-ci.sh")
grep -q '✓ repository-files-self-test' <<<"$out" || fail "unfiltered run did not run the step"
if grep -qE 'skipped|FILTERED' <<<"$out"; then fail "unfiltered run mentions a filter"; fi
echo "ok: $steps steps"
