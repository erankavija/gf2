#!/usr/bin/env bash
# Runs the `gf2-core` fast tier under Rust 1.95 in each feature configuration
# and writes one log per run into `test-logs/` beside this script
# (jit:54e70918). Each log opens with the toolchain and command and closes with
# the exit status.
#
#   <config>-whole            every test target of the package
#   <config>-targets          the library tests and the `simd_equiv_*` targets
#   <config>-implementations  the tests that run through
#                             `kernels::backend::contract::assert_each`, with
#                             the output of each passing test
#   all-features-precondition  the calibration-harness tests that need a SIMD
#                             arm, with the output of each passing test
#
# Usage: run-suites.sh
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
export RUSTUP_TOOLCHAIN=1.95.0
mkdir -p "$here/test-logs"

run() {
    local log="$here/test-logs/$1.txt"
    shift
    local command=(nice -n 19 ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core "$@"
        --cargo-profile ci-test --profile ci)
    {
        echo "# toolchain: $(rustc --version)"
        echo "# command: ${command[*]}"
        "${command[@]}" 2>&1 | sed "s|$root/||g; s|$HOME/|~/|g"
        echo "# exit: ${PIPESTATUS[0]}"
    } > "$log"
    echo "$(basename "$log"): $(tail -n 1 "$log")"
}

quiet=(--status-level fail --final-status-level fail)
targets=(--lib --test simd_equiv_demo --test simd_equiv_dispatch_hoist --test simd_equiv_gf2m_batch)
listed='binary(simd_equiv_demo) | binary(simd_equiv_dispatch_hoist) | binary(simd_equiv_gf2m_batch)'
listed+=' | test(/^kernels::backend::tests::every_backend_/) | test(/^gfp::simd_ops::tests::/)'
each=("${targets[@]}" -E "$listed" --status-level none --final-status-level none --success-output final)

configuration() {
    local name=$1
    shift
    run "$name-whole" "$@" "${quiet[@]}"
    run "$name-targets" "$@" "${targets[@]}" "${quiet[@]}"
    run "$name-implementations" "$@" "${each[@]}"
}

configuration default
configuration simd --features simd
configuration all-features --all-features
# Every `gf2-core` feature except `simd`.
configuration no-simd --no-default-features \
    --features rand,io,parallel,visualization,tuning-profile,test-support
run all-features-precondition --all-features --test tuning_calibration_harness \
    -E 'test(/simd_arm|::accepted_owner_|::owner_emission_publishes|::accepted_complete_experiment/)' \
    --status-level none --final-status-level none --success-output final
