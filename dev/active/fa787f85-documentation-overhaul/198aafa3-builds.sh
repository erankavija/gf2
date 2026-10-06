#!/usr/bin/env bash
# usage: 198aafa3-builds.sh <outfile>
# Builds, on Rust 1.95, every tracked harness manifest outside inputs/ snapshot
# directories that lies in one of the eight entries of issue 198aafa3 or names
# one of them in a relative path dependency; prints one line per manifest: its
# path below dev/active/ without the epic directory, the exit status of
# dependency resolution (cargo metadata), the exit status of the build and the
# first error line of the build without its absolute path. The build scripts
# of three crates read the staged external libraries: the gf2x prefix beside
# the primary checkout, and the byte-field prefix and C shim under target/.
set -u
cd "$(git rev-parse --show-toplevel)"
primary=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
export GF2X_PREFIX="$primary/.agents/ext/c7113c5a/prefix-native"
export GF2_SURVEY_EXT="$PWD/target/6c6b09b1-ext" GF2_SURVEY_BUILD="$PWD/target/6c6b09b1-survey/c"
entries='4c1e441f|53c5a8c0|6c6b09b1|6fb89a3c|ad2a6a58|c077a88b|c7113c5a|f63a2464'
{
  git ls-files -- ':(glob)dev/active/**/Cargo.toml' | grep -E "/($entries)/"
  git grep -l -E "path *= *\"\.\./.*($entries)" -- ':(glob)dev/active/**/Cargo.toml'
} | grep -v '/inputs/' | sort -u |
while read -r m; do
  ./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --manifest-path "$m" >/dev/null 2>&1
  resolve=$?
  log=$(CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path "$m" 2>&1)
  build=$?
  m=${m#dev/active/}
  echo "${m#1a379447-zen3-cpu-performance/} resolve=$resolve build=$build $(grep -m1 '^error' <<<"$log" | sed -E 's# \(/[^)]*\)##')"
done | sort > "$1"
