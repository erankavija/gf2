#!/usr/bin/env bash
# Runs the shared kernel suites in both builds of the entry points
# (jit:bfc2ceab): without a flag into `test-logs/current`, and with
# `RUSTFLAGS="--cfg gf2_entry_checked"` into `test-logs/checked`. The suites
# and their logs are the shared `run-kernel-suites.sh`; each directory also
# gets `rustflags.txt`, the flags its two logs were built with.
#
# Usage (from the worker worktree root): run-suites.sh
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
mapfile -t shared < <(git -C "$root" ls-files -- ':(glob)**/run-kernel-suites.sh' | grep -v '/inputs/')
[[ ${#shared[@]} -eq 1 ]] || {
    echo "${#shared[@]} live run-kernel-suites.sh files; exactly one must exist" >&2
    exit 2
}
cd "$root"
status=0
for build in current checked; do
    flags=''
    [[ $build == checked ]] && flags='--cfg gf2_entry_checked'
    logs="$here/test-logs/$build"
    mkdir -p "$logs"
    printf 'RUSTFLAGS=%s\n' "$flags" > "$logs/rustflags.txt"
    RUSTFLAGS="$flags" "$root/${shared[0]}" "$logs" || status=1
    grep -q '^# exit: 0$' "$logs/simd-build.txt" && grep -q '^# exit: 0$' "$logs/scalar-build.txt" ||
        status=1
done
exit $status
