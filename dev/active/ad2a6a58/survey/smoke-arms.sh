#!/usr/bin/env bash
# Non-timed harness smoke of every arm and cell of the frozen axpy family
# (jit:ad2a6a58), writing dev/active/ad2a6a58/survey/runner-smoke.txt.
#
# Usage (from the worker worktree root):
#   dev/active/ad2a6a58/survey/smoke-arms.sh
#
# A campaign that reaches the benchmark window and dies on its first arm spends
# the window and measures nothing, and reading the runner and the arm side by
# side does not establish the wire between two processes. Three untimed steps
# establish it instead. The arm workspace's tests pin the request mirror against
# `benchmark-ab-runner`'s own `ArmRequest` declaration, so the record below can
# only be regenerated while the two still agree. `benchmark-ab-runner check`
# then applies the decode and validation the runner applies before its first
# measurement, over a throwaway plan projected from the frozen addendum.
# `gf256-axpy-smoke` finally drives every arm of every declared cell with the
# runner's own case encoder, sentinel, child environment and result parser in
# the `validation` position, so each arm performs one untimed dispatch and
# returns no timing window.
#
# Nothing here is timed and nothing it writes is evidence of performance: the
# plan and the throwaway lock live under `target/`, the family ledger is never
# opened, no receipt is finalized and no record line carries a clock reading.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

ISSUE=ad2a6a58
SURVEY=dev/active/${ISSUE}/survey
ADDENDUM=dev/active/${ISSUE}/addendum-v4-axpy-pilot.json
PRODUCING=${SURVEY}/producing-inputs.json
PLAN_TOOL=${SURVEY}/make-plan.py
MANIFEST=${SURVEY}/axpy-arm/Cargo.toml
RECORD=${SURVEY}/runner-smoke.txt
ARM_TARGET="${REPO}/target/${ISSUE}-arm"
SMOKE_BIN="${ARM_TARGET}/release/gf256-axpy-smoke"
RUNNER="${REPO}/target/release/benchmark-ab-runner"
# Repository-relative: the plan names the addendum by a literal relative path,
# so both the projection and the smoke resolve it from the worktree root.
SCRATCH=target/${ISSUE}-campaigns/arms-smoke

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95

CARGO_TARGET_DIR="${ARM_TARGET}" ./scripts/cargo-budget.sh --test cargo test --locked \
    --manifest-path "${MANIFEST}"
CARGO_TARGET_DIR="${ARM_TARGET}" ./scripts/cargo-budget.sh cargo build --release --locked \
    --manifest-path "${MANIFEST}"
./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner >/dev/null

rm -rf "${SCRATCH}"
mkdir -p "${SCRATCH}"
# The plan declares an absolute lock path. This one is a throwaway file nothing
# ever locks: the smoke spawns no timed execution, so it needs no host mutex.
touch "${SCRATCH}/unused.lock"

# The plan the campaigns measure, differing only in the campaign identity, the
# seed and the unused lock, so every arm, cell and case reaching the smoke is
# the one the window will measure.
python3 -B "${PLAN_TOOL}" \
    --addendum "${ADDENDUM}" \
    --label pilot \
    --campaign-id "${ISSUE}-axpy-arms-smoke" \
    --campaign-seed 20260918 \
    --lock "$(realpath "${SCRATCH}/unused.lock")" \
    --executable "${ARM_TARGET}/release/gf256-axpy-arm" \
    --producing-manifest "${PRODUCING}" \
    --max-cells-per-session 10 \
    --pilot-pairs 12 \
    --output "${SCRATCH}/plan.json"
"${RUNNER}" check "${SCRATCH}/plan.json"

"${SMOKE_BIN}" "${SCRATCH}/plan.json" >"${RECORD}"
cat "${RECORD}"
echo "smoke record: ${RECORD}" >&2
