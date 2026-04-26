#!/usr/bin/env bash
# benchmarks/run.sh — drive the reference reproducibility run.
#
# Builds the pinned container (rootless podman by default; Docker
# tolerated), captures host hardware metadata into benchmarks/host.txt,
# launches the reference harnesses, and writes a single CSV file at
# benchmarks/results/<timestamp>.csv with the schema documented in
# benchmarks/README.md.
#
# Usage:
#   ./benchmarks/run.sh                  # full run, default seed
#   ./benchmarks/run.sh --seed 0xCAFE     # override the master seed
#   ./benchmarks/run.sh --skip-build      # reuse the existing image
#   ./benchmarks/run.sh --skip-fflas      # M4RI only (CI smoke run)
#   ./benchmarks/run.sh --skip-m4ri       # fflas-ffpack only
#   ./benchmarks/run.sh --image-tag T     # use a non-default tag
#
# Environment overrides:
#   GF2_RUNTIME=podman|docker             # default: podman
#   GF2_BENCH_TAG=gf2-bench:ref           # built image tag
#   GF2_BENCH_WARMUP=3
#   GF2_BENCH_ITERS=5
#
# This script writes only into benchmarks/{host.txt,results/}; nothing
# leaves the working tree.

set -euo pipefail

# ---- locate ourselves ---------------------------------------------------
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="${HERE}/results"
mkdir -p "${RESULTS_DIR}"

# ---- argument parsing ---------------------------------------------------
RUNTIME="${GF2_RUNTIME:-podman}"
IMAGE_TAG="${GF2_BENCH_TAG:-gf2-bench:ref}"
WARMUP="${GF2_BENCH_WARMUP:-3}"
ITERS="${GF2_BENCH_ITERS:-5}"
SKIP_BUILD=0
RUN_FFLAS=1
RUN_M4RI=1
SEED_OVERRIDE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --seed)         SEED_OVERRIDE="$2"; shift 2 ;;
        --skip-build)   SKIP_BUILD=1; shift ;;
        --skip-fflas)   RUN_FFLAS=0; shift ;;
        --skip-m4ri)    RUN_M4RI=0; shift ;;
        --image-tag)    IMAGE_TAG="$2"; shift 2 ;;
        --warmup)       WARMUP="$2"; shift 2 ;;
        --iters)        ITERS="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,32p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            echo "run.sh: unknown argument $1" >&2
            exit 2
            ;;
    esac
done

# ---- read master seed ---------------------------------------------------
if [[ -n "${SEED_OVERRIDE}" ]]; then
    SEED="${SEED_OVERRIDE}"
else
    # First non-comment, non-empty line of seeds/seed.txt.
    SEED="$(grep -v '^[[:space:]]*#' "${HERE}/seeds/seed.txt" \
              | grep -v '^[[:space:]]*$' \
              | head -n 1 \
              | tr -d '[:space:]')"
fi
if [[ -z "${SEED}" ]]; then
    echo "run.sh: failed to determine master seed" >&2
    exit 1
fi
echo "[run.sh] runtime=${RUNTIME} image=${IMAGE_TAG} seed=${SEED}" >&2
echo "[run.sh] warmup=${WARMUP} iters=${ITERS}" >&2

# ---- runtime check ------------------------------------------------------
if ! command -v "${RUNTIME}" >/dev/null 2>&1; then
    echo "run.sh: ${RUNTIME} not found on PATH" >&2
    echo "        install rootless podman or pass GF2_RUNTIME=docker" >&2
    exit 127
fi

# ---- capture host info --------------------------------------------------
HOST_TXT="${HERE}/host.txt"
{
    echo "# benchmarks/host.txt — captured by run.sh"
    echo "# generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# runtime: ${RUNTIME}"
    echo "# image:   ${IMAGE_TAG}"
    echo
    echo "## uname"
    uname -a || true
    echo
    echo "## /etc/os-release"
    [[ -r /etc/os-release ]] && cat /etc/os-release || true
    echo
    echo "## CPU model"
    if command -v lscpu >/dev/null 2>&1; then
        lscpu
    else
        grep -m1 -E '^model name|^cpu MHz|^cache size|^flags' /proc/cpuinfo || true
    fi
    echo
    echo "## /proc/cpuinfo (first core only)"
    awk 'BEGIN{p=1} /^processor[[:space:]]*:[[:space:]]*1$/{p=0} p==1{print}' \
        /proc/cpuinfo 2>/dev/null || true
    echo
    echo "## meminfo (head)"
    head -n 5 /proc/meminfo 2>/dev/null || true
    echo
    echo "## ${RUNTIME} version"
    "${RUNTIME}" --version || true
} > "${HOST_TXT}"
echo "[run.sh] wrote ${HOST_TXT}" >&2

# ---- build the image ----------------------------------------------------
if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    echo "[run.sh] ${RUNTIME} build -t ${IMAGE_TAG} -f benchmarks/Containerfile benchmarks/" >&2
    "${RUNTIME}" build \
        -t "${IMAGE_TAG}" \
        -f "${HERE}/Containerfile" \
        "${HERE}"
else
    echo "[run.sh] --skip-build set; reusing ${IMAGE_TAG}" >&2
fi

# Capture the local image id and stamp it into image.lock so the next
# review cycle has a concrete digest to compare against.
LOCAL_ID="$("${RUNTIME}" image inspect "${IMAGE_TAG}" \
                --format '{{.Id}}' 2>/dev/null || true)"
if [[ -n "${LOCAL_ID}" ]]; then
    echo "[run.sh] local image id: ${LOCAL_ID}" >&2
fi

# ---- run the harnesses --------------------------------------------------
TS="$(date -u +%Y%m%dT%H%M%SZ)"
CSV_OUT="${RESULTS_DIR}/${TS}.csv"

# Mount options:
#   :Z  — request a private SELinux relabel (rootless podman on
#         SELinux-enabled hosts, e.g. Fedora). Harmless on Debian/Ubuntu.
#   :U  — chown the mount to the container's mapped uid. Required on
#         rootless podman when the container writes back into the mount.
MOUNT_OPTS=":Z,U"
if [[ "${RUNTIME}" == "docker" ]]; then
    MOUNT_OPTS=""   # Docker doesn't accept :Z/:U.
fi

# Header row.
echo "lib,operation,field,m,k,n,rank_regime,seed,wall_ns,throughput_ops" \
    > "${CSV_OUT}"

# We always (re)compile the harnesses inside the container so the host's
# /work bind mount stays clean between runs.
COMPILE_CMD='set -e; cd /work/reference && make -B'

if [[ "${RUN_FFLAS}" -eq 1 ]]; then
    echo "[run.sh] running fflas_bench inside ${IMAGE_TAG}" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "${COMPILE_CMD} && /work/reference/fflas_bench --seed ${SEED} --warmup ${WARMUP} --iters ${ITERS}" \
        >> "${CSV_OUT}"
fi

if [[ "${RUN_M4RI}" -eq 1 ]]; then
    echo "[run.sh] running m4ri_bench inside ${IMAGE_TAG}" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "${COMPILE_CMD} && /work/reference/m4ri_bench --seed ${SEED} --warmup ${WARMUP} --iters ${ITERS}" \
        >> "${CSV_OUT}"
fi

# Convenience symlink to the most recent run.
ln -sf "${TS}.csv" "${RESULTS_DIR}/latest.csv"

echo "[run.sh] CSV written to ${CSV_OUT}" >&2
echo "[run.sh] symlink: ${RESULTS_DIR}/latest.csv -> ${TS}.csv" >&2
