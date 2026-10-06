#!/usr/bin/env bash
# usage: f79bcba3-builds.sh <outfile>
# Builds, on Rust 1.95, every tracked harness manifest of the eight entries of
# issue f79bcba3 outside inputs/ snapshot directories; prints one line per
# manifest: its path from the entry directory on, the exit status of dependency
# resolution (cargo metadata), the exit status of the build and the first error
# line of the build.
set -u
cd "$(git rev-parse --show-toplevel)"
for e in 04b85d10 07ca8585 12fdeb5b 1c602857 1d0da41f 1d4fd63d 2037941f-profile-and-optimize-mid-range-buffer-operations 3be770d5; do
  git ls-files -- ":(glob)**/$e/**/Cargo.toml" | grep -v '/inputs/' | sort |
  while read -r m; do
    ./scripts/cargo-budget.sh cargo +1.95.0 metadata --format-version 1 --manifest-path "$m" >/dev/null 2>&1
    resolve=$?
    log=$(CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --manifest-path "$m" 2>&1)
    echo "$e/${m#*"$e"/} resolve=$resolve build=$? $(grep -m1 '^error' <<<"$log")"
  done
done > "$1"
