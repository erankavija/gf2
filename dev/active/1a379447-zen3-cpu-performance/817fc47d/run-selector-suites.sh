#!/usr/bin/env bash
# Runs the dispatch fallback contract and the suites of the selectors
# jit:817fc47d brings under the core tuning section, with and without the
# `simd` feature, on an unbaked and a baked build.
#
# Untimed: no timing window, no host lock, no ignored test. Each build writes
# `test-logs/<name>.txt` (toolchain, command, output, exit status) beside this
# script.
#
# Usage: run-selector-suites.sh, from any directory of the checkout.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
# A build takes its flags, wrapper, profile and target from this script alone.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER \
    "${!CARGO_BUILD_@}" "${!CARGO_PROFILE_@}" "${!CARGO_TARGET_@}"
logs=$here/test-logs
mkdir -p "$logs"

suites=(--test dispatch_fallback --test tuning_profile_committed --test tuning_envelope_v2
  --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install)

# suite NAME ENVIRONMENT CARGO_ARGUMENTS...
# ENVIRONMENT is `-` or one `NAME=value` assignment; a second target directory
# keeps a build with other compiler flags from evicting the ordinary cache.
suite() {
  local name=$1 assignment=$2
  shift 2
  local -a environment=()
  local shown=""
  if [[ $assignment != - ]]; then
    environment=("$assignment" "CARGO_TARGET_DIR=$root/target/817fc47d-${name}")
    shown="$assignment CARGO_TARGET_DIR=target/817fc47d-${name} "
  fi
  local -a command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run "$@"
    --cargo-profile ci-test --profile ci)
  local log=$logs/$name.txt status=0
  {
    echo "# toolchain: $(rustc --version)"
    echo "# command: ${shown}${command[*]}"
    env "${environment[@]}" "${command[@]}" 2>&1 | sed "s#$root#.#g" || status=$?
    echo "# exit: $status"
  } >"$log"
  if [[ $status -ne 0 ]]; then
    echo "$name: exit $status, see ${log#"$root"/}" >&2
    return "$status"
  fi
  echo "$name: $(grep -ac '^ *PASS ' "$log") tests passed"
}

suite unbaked-portable - -p gf2-core --features test-support,tuning-profile "${suites[@]}"
suite unbaked-simd - -p gf2-core --features simd,test-support,tuning-profile "${suites[@]}"
suite baked-portable 'RUSTFLAGS=--cfg gf2_tuning_baked' -p gf2-core \
  --features test-support,tuning-profile "${suites[@]}"
suite baked-simd 'RUSTFLAGS=--cfg gf2_tuning_baked' -p gf2-core \
  --features simd,test-support,tuning-profile "${suites[@]}"
suite kernels - -p gf2-kernels-simd --all-features
