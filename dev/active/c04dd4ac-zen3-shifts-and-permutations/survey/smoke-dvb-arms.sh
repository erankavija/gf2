#!/usr/bin/env bash
# Non-timed smoke of the DVB profile arms (jit:9fb40c83).
#
# Usage (from the worker worktree root): smoke-dvb-arms.sh [--check]
# Contract: `benchmark-ab-runner smoke`, stated at
# `tuning_campaign_support::arm::smoke`.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

MODE="${1:-}"
case "${MODE}" in
    ""|--check) ;;
    *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

ACTIVE=dev/active/c04dd4ac-zen3-shifts-and-permutations
SURVEY="${ACTIVE}/survey"
ADDENDUM="${ACTIVE}/dvb-profile-addendum.json"
PRODUCING="${SURVEY}/dvb-producing-inputs.json"
PLAN_TOOL="${SURVEY}/make-dvb-plan.py"
RECORD="${SURVEY}/runner-smoke.txt"
ARM="${REPO}/target/9fb40c83-arms-native/release/dvb-profile-arm"
SMOKE=target/9fb40c83-arm-smoke

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

[[ -x "${ARM}" ]] || {
    echo "${ARM} is absent; run ${SURVEY}/build-dvb-harness.sh first" >&2
    exit 2
}
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
    --bin benchmark-ab-runner
RUNNER="$(realpath target/release/benchmark-ab-runner)"

rm -rf "${SMOKE}"
mkdir -p "${SMOKE}"

# The plan is the campaign's own projection of the frozen addendum over every
# frozen cell; the lock it names is never opened.
python3 -B "${PLAN_TOOL}" \
    --addendum "${ADDENDUM}" \
    --campaign-id 9fb40c83-dvb-interleave-arms-smoke \
    --campaign-seed 20260916 \
    --lock "${SMOKE}/unused.lock" \
    --executable "${ARM}" \
    --producing-manifest "${PRODUCING}" \
    --output "${SMOKE}/plan.json" \
    --max-cells-per-session 2 \
    --pilot-pairs 6

"${RUNNER}" smoke "${SMOKE}/plan.json" --record "${SMOKE}/smoke.json"
python3 -B "${SURVEY}/summarize-arm-smoke.py" \
    --record "${SMOKE}/smoke.json" --output "${RECORD}" \
    --title 'DVB-T2 interleaver profile non-timed arm smoke' \
    --command "${SURVEY}/smoke-dvb-arms.sh" ${MODE:+--check}
cat "${RECORD}"
