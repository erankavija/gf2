#!/usr/bin/env bash
# Candidate pilot and profile entry point (jit:bc091474). The unroll variants
# exist in the receipts' input snapshots only, so every action that would build
# or measure a candidate refuses; `window` and `profile-window` check the
# committed receipt or profile.
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
    isolated) family=2037941f-logical-isolated-xor ;;
    row) family=2037941f-logical-public-row-xor ;;
    profile) family='' ;;
    *) usage ;;
esac

refuse() {
    echo "refusing $1: the unroll variants exist in the receipts' input snapshots only" >&2
    exit 2
}

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

case "${action}" in
    build | smoke)
        [[ "${family_kind}" != profile ]] || usage
        refuse "${action}"
        ;;
    window)
        [[ "${family_kind}" != profile ]] || usage
        [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
            echo 'timed candidates run only in the scheduled benchmark window' >&2
            exit 2
        }
        receipt="dev/bench_results/2037941f/${family}/bc091474-u${factor}-pilot"
        [[ -f "${receipt}/receipt.json" ]] || refuse 'a timed pilot'
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
        [[ -s "${profile_out}/profile-summary.md" ]] || refuse 'a timed profile'
        [[ "$(grep -Ec '^rep-[0-9][0-9] done ' "${profile_out}/repetitions.log")" == 9 ]] || {
            echo "profile does not contain nine completed repetitions" >&2
            exit 2
        }
        commit_evidence "${profile_out}" "perf(jit:bc091474): record factor ${factor} profile"
        ;;
    *) usage ;;
esac
