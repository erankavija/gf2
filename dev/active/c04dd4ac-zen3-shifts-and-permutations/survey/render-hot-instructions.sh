#!/usr/bin/env bash
# Instruction-level annotation of a committed DVB-T2 profile (jit:9fb40c83).
#
# This reads recorded perf samples and disassembles the pinned executable they
# name. It measures nothing, takes no host mutex and runs outside the scheduled
# benchmark window; the samples are whatever the profile session recorded.
#
# `--no-source` keeps the listing a function of the perf data and the executable
# alone, so re-running over the same committed directory reproduces it byte for
# byte however the Rust sources move afterwards. `run-profile.sh` renders the
# same listing inside a session with the same flags.
#
# Usage: render-hot-instructions.sh <profile-dir>
set -euo pipefail

PERCENT_LIMIT=1.0

OUT="${1:?usage: render-hot-instructions.sh <profile-dir>}"
OUT="$(cd "${OUT}" && pwd)"

rendered=0
for data in "${OUT}"/rep-*/hot/*.data; do
    [[ -e "${data}" ]] || {
        echo "no recorded perf data under ${OUT}" >&2
        exit 2
    }
    status="${data%.data}.status"
    [[ -e "${status}" && "$(cat "${status}")" == 0 ]] || continue
    perf annotate --stdio --no-source --percent-limit "${PERCENT_LIMIT}" \
        -i "${data}" >"${data%.data}.instructions.txt"
    rendered=$((rendered + 1))
done

echo "rendered ${rendered} instruction listings under ${OUT}" >&2
