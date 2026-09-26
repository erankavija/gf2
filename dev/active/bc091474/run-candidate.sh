#!/usr/bin/env bash
# Untimed candidate build/smoke and queued pilot entry point (jit:bc091474).
# The timed path delegates run, resume, finalize, and acceptance to the
# canonical logical launcher.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the candidate worktree root' >&2
    exit 2
}
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

STORY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations
SURVEY="${STORY}/survey"
MANIFEST="${REPO}/${SURVEY}/harness/Cargo.toml"
PRODUCING="${SURVEY}/logical-producing-inputs.json"
BASE_TARGET="${REPO}/target/bb769456-arms"
BASE_ARM="${BASE_TARGET}/release/logical-arm"
CAMPAIGN="${BASE_TARGET}/release/logical-campaign"
RUNNER="${REPO}/target/release/benchmark-ab-runner"

usage() {
    echo 'usage: run-candidate.sh build|smoke|window 2|4 isolated|row' >&2
    exit 2
}
[[ $# == 3 ]] || usage
action="$1"
factor="$2"
family_kind="$3"
[[ "${factor}" == 2 || "${factor}" == 4 ]] || usage
case "${family_kind}" in
    isolated)
        family=2037941f-logical-isolated-xor
        addendum="${STORY}/campaigns/logical-isolated-xor.json" ;;
    row)
        family=2037941f-logical-public-row-xor
        addendum="${STORY}/campaigns/logical-public-row-xor.json" ;;
    *) usage ;;
esac
# The frozen candidate portfolio fixes 24 pairs per exploratory cell.
PILOT_PAIRS=24
flags="--cfg gf2_xor_unroll${factor}"
candidate_target="${REPO}/target/bc091474-unroll${factor}"
candidate_arm="${candidate_target}/release/logical-arm"

build_arms() {
    CARGO_TARGET_DIR="${BASE_TARGET}" ./scripts/cargo-budget.sh cargo build --release \
        --manifest-path "${MANIFEST}" --bin logical-arm --bin logical-campaign
    RUSTFLAGS="${flags}" CARGO_TARGET_DIR="${candidate_target}" \
        ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}" \
        --bin logical-arm --bin logical-oracle
}

case "${action}" in
    build)
        build_arms
        "${candidate_target}/release/logical-oracle"
        ;;
    smoke)
        build_arms
        ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
            --bin benchmark-ab-runner
        "${CAMPAIGN}" pins
        "${CAMPAIGN}" verify --family "${family}" --addendum "${addendum}"
        python3 -B "${SURVEY}/make-logical-producing-inputs.py" --check
        "${CAMPAIGN}" inputs --producing-manifest "${PRODUCING}" --also "${addendum}"
        smoke_root="${REPO}/target/bc091474-smoke"
        mkdir -p "${smoke_root}"
        lock="${smoke_root}/lock"
        touch "${lock}"
        plan="${smoke_root}/${factor}-${family_kind}.plan.json"
        stage="${smoke_root}/${factor}-${family_kind}.stage"
        record="dev/active/bc091474/smoke-${factor}-${family_kind}.json"
        "${CAMPAIGN}" plan --family "${family}" --addendum "${addendum}" \
            --campaign-id "bc091474-u${factor}-${family_kind}-smoke" \
            --campaign-seed 20260916 --label smoke --lock "${lock}" \
            --gf2-executable "${BASE_ARM}" \
            --candidate-gf2-executable "${candidate_arm}" \
            --candidate-gf2-rustflags "${flags}" \
            --producing-manifest "${PRODUCING}" \
            --max-cells-per-session 2 --pilot-pairs "${PILOT_PAIRS}" --output "${plan}"
        rm -rf "${stage}"
        "${RUNNER}" smoke "${plan}" --stage "${stage}" --record "${record}"
        echo "candidate smoke record: ${record}"
        ;;
    window)
        [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
            echo 'timed candidates run only in the scheduled benchmark window' >&2
            exit 2
        }
        "${SURVEY}/run-logical-harness.sh" window --family "${family}" \
            --addendum "${addendum}" --run-id "bc091474-u${factor}" \
            --candidate-gf2-executable "${candidate_arm}" \
            --candidate-gf2-rustflags "${flags}" --pilot-pairs "${PILOT_PAIRS}"
        ;;
    *) usage ;;
esac
