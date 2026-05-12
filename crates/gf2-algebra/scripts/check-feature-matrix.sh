#!/usr/bin/env bash
# check-feature-matrix.sh — W1-T1 acceptance gate for gf2-algebra.
#
# Iterates the 64 cells of the (simd, parallel, hip, f5, f7, serde)
# Cargo feature matrix from D1c §4 and runs `cargo check -p gf2-algebra`
# against each. Fails fast on the first non-zero exit.
#
# Cell encoding: a 6-bit bitmap (b5..b0) where the bits map to features
# in the order (simd, parallel, hip, f5, f7, serde) — same order as the
# table at D1c §4. Cell `110110` (`b5=simd`, `b4=parallel`, `b2=f5`,
# `b1=f7`) is the crate's
# `default = ["simd", "parallel", "f5", "f7"]` set as of the W4 closing
# edit; `f5` and `f7` were flipped default-on once `packed5` / `packed7`
# landed (jit:6917eb85 / jit:56c5dabc).
#
# `hip` substitution rule (D1c §6.1, "Hosts without hipcc / ROCm"): on a
# host that lacks hipcc, the 32 cells carrying `hip` substitute the same
# combination with `hip` removed; the 32 non-hip cells are exercised
# unchanged. The substitution is recorded in the per-cell log line.
#
# Usage:
#   bash crates/gf2-algebra/scripts/check-feature-matrix.sh
#   bash crates/gf2-algebra/scripts/check-feature-matrix.sh --log out.log
#
# Exit codes:
#   0 — all 64 cells passed (any hip substitutions noted in the log).
#   1 — a cell failed; the offending bitmap and `cargo check` output
#       are printed before exit.

set -euo pipefail

# Resolve the repo root so the script works no matter where it's invoked.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

LOG_FILE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --log)
            LOG_FILE="$2"
            shift 2
            ;;
        *)
            printf 'unknown arg: %s\n' "$1" >&2
            exit 2
            ;;
    esac
done

# Detect hipcc availability. When absent, every `hip`-bearing cell maps
# to its hip-removed equivalent. When present, the cell runs as-is.
HIP_AVAILABLE=0
if command -v hipcc >/dev/null 2>&1; then
    HIP_AVAILABLE=1
fi

# Feature names in the same bit order as the D1c §4 bitmap (b5..b0).
FEATURES=("simd" "parallel" "hip" "f5" "f7" "serde")

emit() {
    local msg="$1"
    printf '%s\n' "${msg}"
    if [[ -n "${LOG_FILE}" ]]; then
        printf '%s\n' "${msg}" >>"${LOG_FILE}"
    fi
}

# Reset the log file if requested.
if [[ -n "${LOG_FILE}" ]]; then
    : >"${LOG_FILE}"
fi

emit "# gf2-algebra feature-matrix sweep (D1c §6.1)"
emit "# host hipcc available: ${HIP_AVAILABLE}"
emit "# repo root: ${REPO_ROOT}"
emit "# date: $(date --iso-8601=seconds)"
emit ""

PASS_COUNT=0
FAIL_COUNT=0

for ((cell=0; cell<64; cell++)); do
    bitmap=""
    enabled=()
    for ((bit=5; bit>=0; bit--)); do
        if (( (cell >> bit) & 1 )); then
            bitmap+="1"
            enabled+=("${FEATURES[5 - bit]}")
        else
            bitmap+="0"
        fi
    done

    # Substitute `hip` away if hipcc is missing.
    note=""
    if [[ ${HIP_AVAILABLE} -eq 0 ]]; then
        filtered=()
        substituted=0
        for f in "${enabled[@]}"; do
            if [[ "${f}" == "hip" ]]; then
                substituted=1
                continue
            fi
            filtered+=("${f}")
        done
        if (( substituted )); then
            enabled=("${filtered[@]}")
            note=" (hip substituted away — host has no hipcc)"
        fi
    fi

    # Build the cargo argv. `--no-default-features` is always passed so
    # the bitmap fully determines the feature set; the default cell
    # `110110` matches `cargo check` with no flags only because we then
    # append `--features simd,parallel,f5,f7`.
    args=(check -p gf2-algebra --no-default-features)
    if (( ${#enabled[@]} > 0 )); then
        joined="$(IFS=,; printf '%s' "${enabled[*]}")"
        args+=(--features "${joined}")
    fi

    label="cell=${cell} bitmap=${bitmap} args=$(printf '%q ' "${args[@]}")${note}"

    if cargo "${args[@]}" >/dev/null 2>&1; then
        emit "PASS ${label}"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        emit "FAIL ${label}"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        # Re-run with output streamed so the user sees the failure.
        cargo "${args[@]}" || true
        emit ""
        emit "# ABORT after first failure (cell=${cell} bitmap=${bitmap})"
        exit 1
    fi
done

emit ""
emit "# summary: ${PASS_COUNT} pass, ${FAIL_COUNT} fail (out of 64)"
