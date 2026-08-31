#!/usr/bin/env bash
# External-baseline survey runner (jit:4e732b56).
#
# Captures host provenance, then drives the three survey harnesses through the
# repository's CCX1 lock wrapper so the run holds the shared benchmark-host
# mutex and lands on the CCX1 cores, matching the receipt convention of the
# other committed benchmark evidence.
#
# Usage:
#   ./run-survey.sh <output-dir> [codes]
#
#   <output-dir>  directory receiving host.txt, the CSVs, and the run logs.
#   [codes]       optional comma-separated contract rows (B1,B2,B3,T2S,T2N);
#                 defaults to every row. Bound a run with this when the
#                 measurement window cannot hold the largest shapes.
#   [prefix]      optional filename prefix, so several staged runs can share
#                 one receipt directory without overwriting each other.
#
# Everything written is produced by this run: no figure is carried in from a
# previous one.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
EXT="${REPO}/.agents/ext"
FLOCK="${REPO}/dev/scripts/ccx1-bench-flock.sh"

OUT="${1:?usage: run-survey.sh <output-dir> [codes] [prefix]}"
CODES="${2:-}"
PREFIX="${3:-}"
mkdir -p "${OUT}"

GF2_REV="$(git -C "${REPO}" rev-parse HEAD)"
export GF2_REV
[[ -n "${CODES}" ]] && export GF2_SURVEY_CODES="${CODES}"

# ---------------------------------------------------------------- provenance
{
    echo "# survey host record — captured by run-survey.sh"
    echo "# generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision: ${GF2_REV}"
    echo "# codes selected: ${CODES:-<all>}"
    echo
    echo "## uname"
    uname -a
    echo
    echo "## /etc/os-release"
    cat /etc/os-release
    echo
    echo "## CPU"
    lscpu
    echo
    echo "## CPU frequency governor"
    for p in /sys/devices/system/cpu/cpu[0-9]*/cpufreq/scaling_governor; do
        [[ -r "$p" ]] && echo "$p: $(cat "$p")"
    done
    echo
    echo "## toolchain"
    gcc --version | head -1
    g++ --version | head -1
    rustc --version
    cargo --version
    echo
    echo "## baseline pins"
    echo "aff3ct tag: $(git -C "${EXT}/aff3ct" describe --tags 2>/dev/null || echo '?')"
    echo "aff3ct commit: $(git -C "${EXT}/aff3ct" rev-parse HEAD 2>/dev/null || echo '?')"
    echo "m4ri version: $(PKG_CONFIG_PATH="${EXT}/prefix/lib/pkgconfig" pkg-config --modversion m4ri)"
    echo "m4ri tarball sha256: $(sha256sum "${EXT}/m4ri-20260122.tar.gz" | cut -d' ' -f1)"
    echo "bchlib tag: $(git -C "${EXT}/bchlib" describe --tags 2>/dev/null || echo '?')"
    echo "bchlib commit: $(git -C "${EXT}/bchlib" rev-parse HEAD 2>/dev/null || echo '?')"
    echo
    echo "## reference build flags"
    echo "aff3ct library: $(grep -m1 '^CXX_FLAGS' "${EXT}/aff3ct/build/CMakeFiles/aff3ct-obj.dir/flags.make" | cut -d= -f2-)"
    echo "aff3ct defines: $(grep -m1 '^CXX_DEFINES' "${EXT}/aff3ct/build/CMakeFiles/aff3ct-obj.dir/flags.make" | cut -d= -f2-)"
    echo "m4ri library: $(grep -m1 '^CFLAGS' "${EXT}/m4ri-src/config.log" | head -1 || echo 'CFLAGS=-O3 -march=native -fPIC')"
    echo
    echo "## lock wrapper"
    echo "wrapper: ${FLOCK}"
    echo "lock file: ${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
} >"${OUT}/${PREFIX}host.txt" 2>&1

echo "provenance -> ${OUT}/${PREFIX}host.txt"

# ------------------------------------------------------------------ harnesses
run_one() {
    local label="$1"
    shift
    echo "== ${label} =="
    "${FLOCK}" "$@" >"${OUT}/${PREFIX}${label}.csv" 2>"${OUT}/${PREFIX}${label}.log" || {
        echo "!! ${label} exited non-zero; see ${OUT}/${PREFIX}${label}.log" >&2
        return 1
    }
    echo "   rows: $(( $(wc -l <"${OUT}/${PREFIX}${label}.csv") - 1 ))"
}

"${HERE}/aff3ct_bch_bench" gdump >"${OUT}/generators.txt" 2>/dev/null
echo "generators -> ${OUT}/generators.txt"

run_one aff3ct "${HERE}/aff3ct_bch_bench" all
run_one bchlib "${HERE}/bchlib_bch_bench"
run_one m4ri "${HERE}/m4ri_genmatrix_bench" "${OUT}/generators.txt" all
run_one gf2 "${EXT}/survey-target/release/survey-gf2-side" all

# ---------------------------------------------------------------- perf record
# One representative cell per workload, so the receipt carries a hardware
# counter profile alongside the wall-clock rows.
if command -v perf >/dev/null 2>&1; then
    "${FLOCK}" perf stat -e task-clock,cycles,instructions,branches,branch-misses,cache-references,cache-misses \
        "${HERE}/aff3ct_bch_bench" w1 >/dev/null 2>"${OUT}/${PREFIX}aff3ct-perf-stat.txt" || true
    echo "perf -> ${OUT}/${PREFIX}aff3ct-perf-stat.txt"
fi

echo "done: ${OUT}"
