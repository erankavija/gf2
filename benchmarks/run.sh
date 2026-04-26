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
# This script writes into benchmarks/host.txt, benchmarks/results/, and
# (after a successful build) stamps the image.lock [image].local_id slot
# with the new build's content-addressable id; nothing leaves the
# working tree.

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

# ---- enforce image.lock as the single source of truth -------------------
# Containerfile carries `ARG <NAME>_SHA256=<hex>` lines for each upstream
# tarball; image.lock carries the same hashes under [libs.<name>]. Drift
# between the two would silently invalidate the lock file, so verify
# them before any build runs.
LOCK_FILE="${HERE}/image.lock"
CONTAINERFILE="${HERE}/Containerfile"
verify_sha() {
    local arg_name="$1"   # e.g. GIVARO_SHA256
    local lock_section="$2"  # e.g. libs.givaro
    local cf_value lock_value
    cf_value="$(grep -E "^ARG ${arg_name}=" "${CONTAINERFILE}" \
                  | head -1 | sed -E 's/^ARG [^=]+=([0-9a-f]+).*$/\1/')"
    lock_value="$(awk -v sec="[${lock_section}]" '
        $0==sec {in_sec=1; next}
        /^\[/ {in_sec=0}
        in_sec && /^sha256/ {gsub(/[\"[:space:]]/,""); sub(/^sha256=/,""); print; exit}
    ' "${LOCK_FILE}")"
    if [[ -z "${cf_value}" || -z "${lock_value}" ]]; then
        echo "[run.sh] error: missing sha for ${arg_name}/${lock_section}" >&2
        exit 1
    fi
    if [[ "${cf_value}" != "${lock_value}" ]]; then
        echo "[run.sh] error: sha drift between Containerfile and image.lock" >&2
        echo "  ARG ${arg_name}      = ${cf_value}" >&2
        echo "  [${lock_section}]    = ${lock_value}" >&2
        echo "Update one or the other so the lock file is authoritative." >&2
        exit 1
    fi
}
verify_sha GIVARO_SHA256 libs.givaro
verify_sha FFLAS_SHA256  libs.fflas-ffpack
verify_sha M4RI_SHA256   libs.m4ri

# Verify the base-image digest in [base].digest matches the Containerfile
# `FROM ...@sha256:...` line.
verify_base_digest() {
    local cf_digest lock_digest
    cf_digest="$(grep -E '^FROM .+@sha256:[0-9a-f]+' "${CONTAINERFILE}" \
                  | head -1 | sed -E 's/^.*@(sha256:[0-9a-f]+).*$/\1/')"
    lock_digest="$(awk '
        $0=="[base]" {in_sec=1; next}
        /^\[/ {in_sec=0}
        in_sec && /^digest/ {gsub(/[\"[:space:]]/,""); sub(/^digest=/,""); print; exit}
    ' "${LOCK_FILE}")"
    if [[ -z "${cf_digest}" || -z "${lock_digest}" ]]; then
        echo "[run.sh] error: missing base digest in Containerfile or image.lock" >&2
        exit 1
    fi
    if [[ "${cf_digest}" != "${lock_digest}" ]]; then
        echo "[run.sh] error: base-image digest drift" >&2
        echo "  Containerfile FROM = ${cf_digest}" >&2
        echo "  image.lock [base]  = ${lock_digest}" >&2
        exit 1
    fi
}
verify_base_digest

# Verify apt-pinned package versions in [libs.<name>] match the
# `<package>=<version>` pins in the Containerfile's apt-get install
# block. Three pins are tracked in image.lock today: gcc-12, openblas,
# gmp.
verify_apt_pin() {
    local pkg_pattern="$1"   # apt-get install pattern, e.g. gcc-12
    local lock_section="$2"  # e.g. libs.gcc
    local cf_version lock_version
    cf_version="$(grep -E "^[[:space:]]+${pkg_pattern}=" "${CONTAINERFILE}" \
                    | head -1 | sed -E "s|^[[:space:]]+${pkg_pattern}=([^ \\\\]+).*\$|\1|")"
    # Strip apt epoch prefix (e.g. "2:6.2.1..." -> "6.2.1...") so the
    # lock file can store the upstream version unambiguously.
    cf_version_no_epoch="${cf_version#*:}"
    lock_version="$(awk -v sec="[${lock_section}]" '
        $0==sec {in_sec=1; next}
        /^\[/ {in_sec=0}
        in_sec && /^version/ {gsub(/[\"[:space:]]/,""); sub(/^version=/,""); print; exit}
    ' "${LOCK_FILE}")"
    if [[ -z "${cf_version}" || -z "${lock_version}" ]]; then
        echo "[run.sh] error: missing version for ${pkg_pattern}/${lock_section}" >&2
        exit 1
    fi
    if [[ "${cf_version_no_epoch}" != "${lock_version}" ]]; then
        echo "[run.sh] error: apt version drift for ${pkg_pattern}" >&2
        echo "  Containerfile = ${cf_version} (after stripping apt epoch: ${cf_version_no_epoch})" >&2
        echo "  [${lock_section}] = ${lock_version}" >&2
        exit 1
    fi
}
verify_apt_pin gcc-12         libs.gcc
verify_apt_pin g\+\+-12        libs.gpp
verify_apt_pin libopenblas-dev libs.openblas
verify_apt_pin libgmp-dev      libs.gmp
verify_apt_pin liblapack-dev   libs.lapack
verify_apt_pin cmake           libs.cmake

echo "[run.sh] verified Containerfile pins (sha256 ARGs + base digest + apt versions) match image.lock" >&2

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
    # Stamp image.lock's [image].local_id so subsequent invocations on
    # the same host can detect environment drift. The file is committed
    # with a "TODO_FILL_AFTER_FIRST_BUILD" placeholder; this in-place
    # rewrite replaces it with the concrete sha256.
    if [[ -f "${HERE}/image.lock" ]]; then
        # Ensure local_id starts with sha256: prefix (podman returns it
        # bare on some versions).
        case "${LOCAL_ID}" in
            sha256:*) LOCAL_ID_TAGGED="${LOCAL_ID}" ;;
            *)        LOCAL_ID_TAGGED="sha256:${LOCAL_ID}" ;;
        esac
        # Use a tmpfile so a partial write cannot corrupt image.lock.
        TMP_LOCK="$(mktemp)"
        sed -E "s|^(local_id\\s*=\\s*\")[^\"]*\"|\\1${LOCAL_ID_TAGGED}\"|" \
            "${HERE}/image.lock" > "${TMP_LOCK}"
        mv "${TMP_LOCK}" "${HERE}/image.lock"
        echo "[run.sh] stamped image.lock local_id = ${LOCAL_ID_TAGGED}" >&2
    fi
fi

# ---- run the harnesses --------------------------------------------------
TS="$(date -u +%Y%m%dT%H%M%SZ)"
# GF2_CSV_PREFIX (env-var, optional) namespaces the CSV file so that
# `smoke.sh` runs cannot be confused with real timing runs. Empty / unset
# means "real timing run" → results/<TS>.csv. Set means
# results/<prefix>-<TS>.csv plus a separate <prefix>-latest.csv symlink.
CSV_PREFIX="${GF2_CSV_PREFIX:-}"
if [[ -n "${CSV_PREFIX}" ]]; then
    CSV_OUT="${RESULTS_DIR}/${CSV_PREFIX}-${TS}.csv"
    LATEST_LINK="${RESULTS_DIR}/${CSV_PREFIX}-latest.csv"
else
    CSV_OUT="${RESULTS_DIR}/${TS}.csv"
    LATEST_LINK="${RESULTS_DIR}/latest.csv"
fi

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
ln -sf "$(basename "${CSV_OUT}")" "${LATEST_LINK}"

echo "[run.sh] CSV written to ${CSV_OUT}" >&2
echo "[run.sh] symlink: ${LATEST_LINK} -> $(basename "${CSV_OUT}")" >&2
