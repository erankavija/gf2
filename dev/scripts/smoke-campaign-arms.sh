#!/usr/bin/env bash
# Shared non-timed harness smoke of every arm and cell of a frozen family.
#
# Usage (from the worker worktree root), with every flag required except the
# bracketed one:
#   smoke-campaign-arms.sh --issue ID --addendum JSON
#       --arm-manifest Cargo.toml --arm-bin NAME --plan-tool PY
#       --producing JSON --record PATH --campaign-id ID --seed N
#       --max-cells N [--pilot-pairs N]
#
# A campaign that reaches the benchmark window and dies on its first arm spends
# the window and measures nothing, and reading the runner and the arm side by
# side does not establish the wire between two processes. Three untimed steps
# establish it. The arm workspace's tests pin the request mirror against
# `benchmark-ab-runner`'s own `ArmRequest` declaration, so the record can only be
# regenerated while the two agree; `benchmark-ab-runner check` applies the decode
# and validation the runner applies before its first measurement;
# `benchmark-ab-runner smoke`, whose contract `tuning_campaign_support::arm::smoke`
# states, drives every arm of every declared cell once in the `validation`
# position and writes the record.
#
# The projected plan carries the label of the stage the addendum freezes:
# `confirmation` when a cell is confirmatory, `pilot` when every cell is
# exploratory.
#
# Nothing here is timed: the throwaway plan and its unused lock live under
# `target/`, the family ledger is never opened, no receipt is finalized and no
# record line carries a clock reading.
set -euo pipefail

REPO=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

PILOT_PAIRS=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --issue) ISSUE=$2 ;;
        --addendum) ADDENDUM=$2 ;;
        --arm-manifest) MANIFEST=$2 ;;
        --arm-bin) ARM_BIN=$2 ;;
        --plan-tool) PLAN_TOOL=$2 ;;
        --producing) PRODUCING=$2 ;;
        --record) RECORD=$2 ;;
        --campaign-id) CAMPAIGN_ID=$2 ;;
        --seed) SEED=$2 ;;
        --max-cells) MAX_CELLS=$2 ;;
        --pilot-pairs) PILOT_PAIRS=$2 ;;
        *) echo "unknown flag $1" >&2; exit 2 ;;
    esac
    shift 2
done
: "${ISSUE:?--issue}" "${ADDENDUM:?--addendum}" "${MANIFEST:?--arm-manifest}"
: "${ARM_BIN:?--arm-bin}" "${PLAN_TOOL:?--plan-tool}"
: "${PRODUCING:?--producing}" "${RECORD:?--record}" "${CAMPAIGN_ID:?--campaign-id}"
: "${SEED:?--seed}" "${MAX_CELLS:?--max-cells}"

ARM_TARGET="${REPO}/target/${ISSUE}-arm"
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

LABEL=$(python3 -B - "${ADDENDUM}" <<'PY'
import json, sys
roles = {cell["role"] for cell in json.load(open(sys.argv[1]))["cells"]}
if "confirmatory" in roles:
    print("confirmation")
elif roles == {"exploratory"}:
    print("pilot")
else:
    raise SystemExit(f"{sys.argv[1]}: cell roles {sorted(roles)} name no smoked stage")
PY
)

# The plan the campaign measures, differing only in the campaign identity, the
# seed and the unused lock, so every arm, cell and case reaching the smoke is the
# one the window will measure.
python3 -B "${PLAN_TOOL}" \
    --addendum "${ADDENDUM}" \
    --label "${LABEL}" \
    --campaign-id "${CAMPAIGN_ID}" \
    --campaign-seed "${SEED}" \
    --lock "$(realpath "${SCRATCH}/unused.lock")" \
    --executable "${ARM_TARGET}/release/${ARM_BIN}" \
    --producing-manifest "${PRODUCING}" \
    --max-cells-per-session "${MAX_CELLS}" \
    ${PILOT_PAIRS:+--pilot-pairs "${PILOT_PAIRS}"} \
    --output "${SCRATCH}/plan.json"
"${RUNNER}" check "${SCRATCH}/plan.json"

"${RUNNER}" smoke "${SCRATCH}/plan.json" --record "${RECORD}"
echo "smoke record: ${RECORD}" >&2
