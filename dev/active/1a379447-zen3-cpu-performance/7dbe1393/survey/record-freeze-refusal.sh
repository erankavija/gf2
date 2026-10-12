#!/usr/bin/env bash
# Record the canonical freezer's answer to the ragged-tail pilot (jit:7dbe1393).
#
# Usage: record-freeze-refusal.sh <freeze-confirmation.py> <survey-analysis>
# <dense-campaign> <frozen-utc>, from the worktree root. The resolution is the one `make-resolution.py` derives
# under the frozen rule; the freezer receives it with every margin of the pilot
# addendum unchanged. The record holds the command, the freezer's output and
# exit status, and whether a confirmation addendum exists afterwards. Nothing
# here is timed.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worktree root' >&2
    exit 2
}
USAGE='usage: record-freeze-refusal.sh <freeze-confirmation.py> <survey-analysis> <dense-campaign> <frozen-utc>'
FREEZER="${1:?${USAGE}}"
ANALYSIS="${2:?${USAGE}}"
CAMPAIGN_TOOL="${3:?${USAGE}}"
FROZEN_UTC="${4:?${USAGE}}"

live() {
    git ls-files -- ":(glob)**/$1" | grep -v '/inputs/'
}
SURVEY="${HERE#"${REPO}/"}"
STORY="$(dirname "${SURVEY}")"
PILOT_ADDENDUM="$(live matvec-ragged-tail.json)"
PILOT="$(dirname "$(git grep -l --full-name -F '"campaign_id": "v4-r1-7dbe1393-matvec-ragged-tail"' \
    -- ':(glob)**/receipt.json' | grep -v '/inputs/')")"
OUTPUT="$(dirname "${PILOT_ADDENDUM}")/matvec-ragged-tail-confirmation.json"
DERIVATION="${STORY}/pilot-resolution.md"
RECORD="${HERE}/freeze-refusal.txt"
DESCRIPTION="Allocated BitMatrix::matvec_parity_kernel, which reduces each row through the bundle's fused AND-parity kernel, as candidate against the public BitMatrix::matvec on its selected SIMD lane in the same build, with the output allocation and appends inside both calls, at the frozen ragged strides and the aligned control in the warm cache state. This addendum is the confirmation: every cell is confirmatory and is measured on fresh pairs."

rule="$(python3 -B "${HERE}/make-resolution.py" "${ANALYSIS}" "${CAMPAIGN_TOOL}")"
resolution="$(sed -n 's/^resolution=\([0-9.]*\) .*/\1/p' <<<"${rule}")"
command=(python3 -B "${FREEZER}" --pilot-addendum "${PILOT_ADDENDUM}" --pilot "${PILOT}"
    --frozen-utc "${FROZEN_UTC}" --output "${OUTPUT}" --record "${OUTPUT%.json}-derivation.txt"
    --resolution "${resolution}" --resolution-derivation "${DERIVATION}"
    --resolution-decimals 3 --family-description "${DESCRIPTION}")
set +e
answer="$("${command[@]}" 2>&1)"
status=$?
set -e
{
    echo '# Canonical freezer on the ragged-tail pilot (jit:7dbe1393)'
    echo "# freezer sha256: $(sha256sum "${FREEZER}" | cut -d' ' -f1)"
    echo "# pilot receipt sha256: $(sha256sum "${PILOT}/receipt.json" | cut -d' ' -f1)"
    echo "# derivation sha256: $(sha256sum "${DERIVATION}" | cut -d' ' -f1)"
    echo "# frozen rule: ${rule}"
    printf '# command:'
    printf ' %q' "${command[@]}"
    echo
    echo "${answer}"
    echo "# exit status: ${status}"
    if [[ -e "${OUTPUT}" ]]; then
        echo "# confirmation addendum: ${OUTPUT}"
    else
        echo '# confirmation addendum: none written'
    fi
} >"${RECORD}.tmp"
mv "${RECORD}.tmp" "${RECORD}"
cat "${RECORD}"
