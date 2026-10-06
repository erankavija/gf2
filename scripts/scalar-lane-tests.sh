#!/usr/bin/env bash
set -euo pipefail

# Runs the `gf2-algebra` and `gf2-coding` test suites on the scalar lane: each
# package's default features without `simd`, selected in one build so both
# packages link one `gf2-core`.
#
# Usage:
#   scripts/cargo-budget.sh --test scripts/scalar-lane-tests.sh [nextest args...]
#
# Before the run it prints, one `scalar lane:` line per workspace package of
# the build, the feature list Cargo resolves with dev-dependencies included,
# and exits 1 when a list names `simd`: a dev-dependency that enables the
# feature would otherwise turn the run into a second SIMD-lane run unnoticed.

packages=(gf2-algebra gf2-coding)

features=$(cargo metadata --no-deps --format-version 1 | python3 -B -c '
import json, sys

print(",".join(
    package["name"] + "/" + feature
    for package in json.load(sys.stdin)["packages"]
    if package["name"] in sys.argv[1:]
    for feature in package["features"]["default"]
    if feature != "simd"
))' "${packages[@]}")

selection=("${packages[@]/#/--package=}" --no-default-features --features "$features")

# A workspace package prints with its path in parentheses; a registry package
# does not, so the pattern keeps the former.
resolved=$(cargo tree "${selection[@]}" -e normal,dev --prefix none -f '{p} [{f}]' |
  sed -nE 's|^([^ ]+ v[^ ]+) \(/[^)]*\) (\[[^]]*\]).*|scalar lane: \1 \2|p' | sort -u)
echo "$resolved"

if grep -qE '[[,]simd[],]' <<<"$resolved"; then
  echo "ERROR: the scalar-lane build resolves the simd feature" >&2
  exit 1
fi

exec cargo nextest run "${selection[@]}" "$@"
