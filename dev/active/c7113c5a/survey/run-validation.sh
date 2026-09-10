#!/usr/bin/env bash
# Run the correctness validation of every build variant (jit:c7113c5a).
#
# Usage:
#   ./run-validation.sh [output-file] [staging-directory]
#
# Correctness evidence precedes timing: this script must exit 0 before any
# receipt run. It executes `poly-validate` once per build variant, so each
# variant's own binaries are the ones checked, and concatenates the machine
# readable reports into one committed artifact. A nonzero exit means at least
# one arm disagrees with canonical arithmetic and no timing may proceed.
#
# The validator is compute-only and short, so it takes the shared side of the
# CCX1 mutex like a build rather than the exclusive side. It acquires that side
# through `scripts/cargo-budget.sh`, which is the only supported shared
# acquirer: a bare `flock -s` on the mutex skips the turnstile in
# `dev/scripts/ccx1-bench-flock.sh` and can enter ahead of a queued measurement
# run, which is the starvation the turnstile exists to remove.
#
# The report also records the digest of every executable and gf2x library of
# every variant, so a receipt's arm digests and the library digests its gf2x
# arms report identify which validated binaries it measured.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
ISSUE=c7113c5a
OUT="${1:-${HERE}/validation-v3.json}"
EXT="${2:-${GF2_SURVEY_EXT:-${REPO}/target/${ISSUE}-ext}}"
BUDGET="${REPO}/scripts/cargo-budget.sh"
LIB=lib/libgf2x.so.3.0.0

status=0
{
    echo '{'
    echo '  "schema": "poly-baseline-validation-set-v2",'
    echo "  \"issue\": \"${ISSUE}\","
    echo '  "variants": {'
    separator=""
    for name in conservative tuned native; do
        printf '%s    "%s": ' "${separator}" "${name}"
        if "${BUDGET}" "${EXT}/arms-${name}/release/poly-validate"; then
            :
        else
            status=1
        fi
        separator=$',\n'
    done
    echo
    echo '  },'
    echo '  "executables": {'
    separator=""
    for name in conservative tuned native; do
        printf '%s    "%s": {\n' "${separator}" "${name}"
        inner=""
        for binary in poly-validate gf2-poly-arm gf2x-poly-arm; do
            path="${EXT}/arms-${name}/release/${binary}"
            printf '%s      "%s": "%s"' \
                "${inner}" "${binary}" "$(sha256sum "${path}" | cut -d' ' -f1)"
            inner=$',\n'
        done
        printf '%s      "libgf2x": "%s"' \
            "${inner}" "$(sha256sum "${EXT}/prefix-${name}/${LIB}" | cut -d' ' -f1)"
        printf '\n    }'
        separator=$',\n'
    done
    echo
    echo '  }'
    echo '}'
} >"${OUT}"

if [[ "${status}" -ne 0 ]]; then
    echo "validation failed; see ${OUT}" >&2
    exit 1
fi
echo "validation passed: ${OUT}" >&2
