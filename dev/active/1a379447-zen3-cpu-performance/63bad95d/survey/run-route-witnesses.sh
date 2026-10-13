#!/usr/bin/env bash
# Runs the shared dispatch fallback contract on each build configuration the
# route inventory covers and records what its reporters answer (jit:63bad95d),
# and runs the suites of the selectors jit:817fc47d brings under the core
# section with and without `simd`, on an unbaked and a baked build.
#
# Untimed: no timing window, no host lock, no ignored test. Each run writes
# `test-logs/<name>.txt` (toolchain, command, output, exit status) beside this
# script; a witness configuration also writes `route-witness/<name>.tsv`
# (entry, input, route, kind). `route-witness/dependency-features.tsv` holds
# the features cargo resolves for the kernel packages in each package's
# ordinary dependency build.
#
# Usage: run-route-witnesses.sh [witnesses|selectors], from any directory of
# the checkout; without an argument both sets run.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
# A build takes its flags, wrapper, profile and target from this script alone.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER \
    "${!CARGO_BUILD_@}" "${!CARGO_PROFILE_@}" "${!CARGO_TARGET_@}"
logs=$here/test-logs
records=$here/route-witness
mkdir -p "$logs" "$records"

set_selected=${1:-all}
case $set_selected in
  all | witnesses | selectors) ;;
  *)
    echo "usage: run-route-witnesses.sh [witnesses|selectors]" >&2
    exit 2
    ;;
esac

# logged NAME ENVIRONMENT CARGO_ARGUMENTS...
# ENVIRONMENT is `-` or one `NAME=value` assignment; a second target directory
# keeps a build with other compiler flags from evicting the ordinary cache.
logged() {
  local name=$1 assignment=$2
  shift 2
  local -a environment=()
  local shown=""
  if [[ $assignment != - ]]; then
    environment=("$assignment" "CARGO_TARGET_DIR=$root/target/63bad95d-${name}")
    shown="$assignment CARGO_TARGET_DIR=target/63bad95d-${name} "
  fi
  local -a command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run "$@"
    --cargo-profile ci-test --profile ci --no-capture)
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
}

witness() {
  logged "$@"
  grep -a $'^GF2_ROUTE_WITNESS\t' "$logs/$1.txt" | cut -f2- >"$records/$1.tsv"
  echo "$1: $(wc -l <"$records/$1.tsv") witness rows"
}

if [[ $set_selected != witnesses ]]; then
  suites=(--test dispatch_fallback --test tuning_profile_committed --test tuning_envelope_v2
    --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install)
  logged selectors-unbaked-portable - -p gf2-core --features test-support,tuning-profile \
    "${suites[@]}"
  logged selectors-unbaked-simd - -p gf2-core --features simd,test-support,tuning-profile \
    "${suites[@]}"
  logged selectors-baked-portable 'RUSTFLAGS=--cfg gf2_tuning_baked' -p gf2-core \
    --features test-support,tuning-profile "${suites[@]}"
  logged selectors-baked-simd 'RUSTFLAGS=--cfg gf2_tuning_baked' -p gf2-core \
    --features simd,test-support,tuning-profile "${suites[@]}"
  logged selectors-kernels - -p gf2-kernels-simd --all-features
  echo "selectors: five suite logs"
fi
if [[ $set_selected == selectors ]]; then
  exit 0
fi

witness core-default - -p gf2-core --test dispatch_fallback
witness core-simd - -p gf2-core --features simd --test dispatch_fallback
witness core-all-features - -p gf2-core --all-features --test dispatch_fallback
witness core-baked 'RUSTFLAGS=--cfg gf2_tuning_baked' -p gf2-core \
  --features simd,test-support,tuning-profile --test dispatch_fallback
witness coding-default - -p gf2-coding --test core_dispatch_fallback
witness coding-no-default-features - -p gf2-coding --no-default-features \
  --test core_dispatch_fallback
witness sim-default - -p gf2-sim --test core_dispatch_fallback

# Features of the kernel packages in each package's ordinary (non-dev)
# dependency build.
{
  for selection in "gf2-core" "gf2-core --all-features" "gf2-coding" \
    "gf2-coding --no-default-features" "gf2-algebra" "gf2-sim" "gf2-sim --all-features"; do
    # shellcheck disable=SC2086
    cargo tree --offline -p $selection -e normal -f '{p}|{f}' --prefix none |
      sed -E 's/ \(\*\)$//; s/ v[^ |]+( \([^)]*\))?\|/|/' |
      grep -E '^gf2-(core|kernels-simd)\|' | sort -u |
      while IFS='|' read -r package features; do
        printf '%s\t%s\t%s\n' "$selection" "$package" "$features"
      done
  done
} >"$records/dependency-features.tsv"
echo "dependency-features: $(wc -l <"$records/dependency-features.tsv") rows"
