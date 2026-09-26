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
    echo '       run-candidate.sh profile-window 2|4 profile' >&2
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
    profile)
        family=''
        addendum='' ;;
    *) usage ;;
esac
# The frozen candidate portfolio fixes 24 pairs per exploratory cell.
PILOT_PAIRS=24
flags="--cfg gf2_xor_unroll${factor}"
candidate_target="${REPO}/target/bc091474-unroll${factor}"
candidate_arm="${candidate_target}/release/logical-arm"

# Only a completed, independently checked receipt and its family ledger enter
# one evidence commit. Force-add the receipt's ignored Cargo.lock snapshots.
commit_evidence() {
    local evidence="$1" subject="$2" ledger="${3:-}" path
    [[ -n "${ledger}" ]] && git add -- "${ledger}"
    git add -f -A -- "${evidence}"
    while IFS= read -r path; do
        [[ "${path}" == "${ledger}" || "${path}" == "${evidence}/"* ]] || {
            echo "unrelated staged path refuses evidence commit: ${path}" >&2
            exit 2
        }
    done < <(git diff --cached --name-only)
    if ! git diff --cached --quiet; then
        git commit -m "${subject}"
    fi
}

build_arms() {
    CARGO_TARGET_DIR="${BASE_TARGET}" ./scripts/cargo-budget.sh cargo build --release \
        --manifest-path "${MANIFEST}" --bin logical-arm --bin logical-campaign
    RUSTFLAGS="${flags}" CARGO_TARGET_DIR="${candidate_target}" \
        ./scripts/cargo-budget.sh cargo build --release --manifest-path "${MANIFEST}" \
        --bin logical-arm --bin logical-oracle
}

case "${action}" in
    build)
        [[ "${family_kind}" != profile ]] || usage
        build_arms
        "${candidate_target}/release/logical-oracle"
        ;;
    smoke)
        [[ "${family_kind}" != profile ]] || usage
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
        while true; do
            set +e
            "${RUNNER}" smoke "${plan}" --stage "${stage}" --record "${record}"
            outcome=$?
            set -e
            case "${outcome}" in
                0) break ;;
                3) ;; # The frozen two-cell session bound pauses this untimed smoke.
                *) exit "${outcome}" ;;
            esac
        done
        echo "candidate smoke record: ${record}"
        ;;
    window)
        [[ "${family_kind}" != profile ]] || usage
        [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
            echo 'timed candidates run only in the scheduled benchmark window' >&2
            exit 2
        }
        receipt="dev/bench_results/2037941f/${family}/bc091474-u${factor}-pilot"
        if [[ ! -f "${receipt}/receipt.json" ]]; then
            "${SURVEY}/run-logical-harness.sh" window --family "${family}" \
                --addendum "${addendum}" --run-id "bc091474-u${factor}" \
                --candidate-gf2-executable "${candidate_arm}" \
                --candidate-gf2-rustflags "${flags}" --pilot-pairs "${PILOT_PAIRS}"
        fi
        [[ -f "${receipt}/receipt.json" ]] || {
            echo "pilot receipt is absent: ${receipt}" >&2
            exit 2
        }
        python3 -B dev/scripts/verify-campaign-log.py \
            --log "${receipt}/execution.log" --receipt "${receipt}/receipt.json" \
            --plan "${receipt}/plan.json"
        if [[ ! -x target/release/benchmark-acceptance ]]; then
            ./scripts/cargo-budget.sh cargo build --release --locked \
                -p tuning-campaign-support --bin benchmark-acceptance
        fi
        target/release/benchmark-acceptance "${receipt}"
        case "${family_kind}" in
            isolated) ledger=dev/bench_results/2037941f/logical-isolated-xor-ledger.jsonl ;;
            row) ledger=dev/bench_results/2037941f/logical-public-row-xor-ledger.jsonl ;;
        esac
        commit_evidence "${receipt}" \
            "perf(jit:bc091474): record factor ${factor} ${family_kind} pilot" "${ledger}"
        ;;
    profile-window)
        [[ "${family_kind}" == profile ]] || usage
        [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
            echo 'timed candidate profiles run only in the scheduled benchmark window' >&2
            exit 2
        }
        profile_out="dev/bench_results/bc091474/profile-unroll${factor}"
        if [[ ! -s "${profile_out}/profile-summary.md" ]] \
            || ! grep -q '^series done ' "${profile_out}/repetitions.log" 2>/dev/null; then
            GF2_LOGICAL_PROFILE_TARGET="${candidate_target}" \
                GF2_LOGICAL_PROFILE_RUSTFLAGS="${flags}" \
                GF2_LOGICAL_PROFILE_SCOPE=candidate \
                "${SURVEY}/run-logical-profile.sh" window "${profile_out}"
        fi
        [[ -s "${profile_out}/profile-summary.md" ]] || {
            echo "profile summary is absent: ${profile_out}" >&2
            exit 2
        }
        [[ "$(grep -Ec '^rep-[0-9][0-9] done ' "${profile_out}/repetitions.log")" == 9 ]] || {
            echo "profile does not contain nine completed repetitions" >&2
            exit 2
        }
        commit_evidence "${profile_out}" "perf(jit:bc091474): record factor ${factor} profile"
        ;;
    *) usage ;;
esac
