#!/usr/bin/env bash
# benchmarks/smoke.sh — fast end-to-end build + run of the reference
# harnesses, intended as a manual substitute for the CI gate (which
# cannot run podman).
#
# Behaviour:
#   * Engages each harness's `--smoke` mode first so the per-operation
#     algebraic-equality oracles (issue 5dea7457; protocol § 6) run at
#     n=16 before any timing pass. A failed oracle exits 1 immediately.
#   * Forces warmup=0, iters=1 so the harness produces one CSV row per
#     (field, op, size, regime) cell as cheaply as possible.
#   * Builds the image (or reuses if already built).
#   * Writes the CSV under benchmarks/results/smoke-<timestamp>.csv so
#     it does not get confused with a real timing run.
#   * Returns non-zero if any podman / build / run / smoke step fails —
#     useful for `./benchmarks/smoke.sh && echo OK` in a pre-PR script.
#
# This is the script the lead runs to substantiate the
# "container builds from clean state" + "harnesses run to completion"
# success criteria during code-review. It is not a substitute for the
# real timing run — those go through `run.sh` with the full warmup +
# iters and feed the per-cell numbers the bench report consumes.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME="${GF2_RUNTIME:-podman}"
IMAGE_TAG="${GF2_BENCH_TAG:-gf2-bench:smoke}"

export GF2_BENCH_WARMUP=0
export GF2_BENCH_ITERS=1
# run.sh reads GF2_CSV_PREFIX to prepend a tag to the output filename
# so smoke runs do not silently overwrite the latest.csv symlink that
# real timing runs emit.
export GF2_CSV_PREFIX=smoke

# Drive the canonical fflas + m4ri smoke through run.sh, then layer on
# the secondary references (linbox, m4rie, ntl, flint) and the
# cross-equality oracle. We do not modify run.sh further: each new lane
# either appends its CSV rows to the smoke CSV file run.sh wrote, or
# runs as a hard equality oracle whose exit code gates the whole smoke
# run. The fflas/m4ri per-op equality contracts are exercised by
# run.sh's own --smoke-equality flag (added by jit:5dea7457).
SEED_OVERRIDE=""
# Pre-parse so we can pass through to run.sh and reuse the same image
# for the secondary lanes.
ARGS=()
for ((i=1; i<=$#; i++)); do
    arg="${!i}"
    case "${arg}" in
        --image-tag)
            j=$((i+1)); IMAGE_TAG="${!j}"; ARGS+=("${arg}" "${IMAGE_TAG}"); ((i++)) ;;
        --seed)
            j=$((i+1)); SEED_OVERRIDE="${!j}"; ARGS+=("${arg}" "${SEED_OVERRIDE}"); ((i++)) ;;
        *)
            ARGS+=("${arg}") ;;
    esac
done

# Run the canonical smoke (fflas + m4ri) — non-exec so we keep going.
# `--smoke-equality` engages the per-op n=16 algebraic-equality oracle
# (jit:5dea7457) before the timing pass.
"${HERE}/run.sh" --image-tag "${IMAGE_TAG}" --smoke-equality "${ARGS[@]}"

# Locate the CSV run.sh just wrote (smoke-<TS>.csv plus a
# smoke-latest.csv symlink). The symlink path is the canonical handle.
LATEST_CSV="${HERE}/results/smoke-latest.csv"
if [[ ! -L "${LATEST_CSV}" && ! -f "${LATEST_CSV}" ]]; then
    echo "[smoke.sh] expected smoke-latest.csv missing; aborting" >&2
    exit 1
fi
TARGET_CSV="$(readlink -f "${LATEST_CSV}")"
echo "[smoke.sh] appending secondary-reference rows to ${TARGET_CSV}" >&2

# Resolve master seed the same way run.sh does (first non-comment line
# of seeds/seed.txt, unless --seed N was passed).
if [[ -n "${SEED_OVERRIDE}" ]]; then
    SEED="${SEED_OVERRIDE}"
else
    SEED="$(grep -v '^[[:space:]]*#' "${HERE}/seeds/seed.txt" \
              | grep -v '^[[:space:]]*$' \
              | head -n 1 \
              | tr -d '[:space:]')"
fi

MOUNT_OPTS=":Z,U"
if [[ "${RUNTIME}" == "docker" ]]; then
    MOUNT_OPTS=""
fi

# === linbox begin ===
# Per SOTA reference acceptance protocol § 6 *Correctness-oracle harness*,
# invoke linbox_bench --smoke to assert the n=16 per-op equality
# contract (charpoly Cayley-Hamilton, minpoly annihilation, solve
# A·x ≡ b) for every (op, field) cell the LinBox harness covers.
# A non-zero exit here is a hard semantics-mismatch failure per § 9.
if [[ "${GF2_SKIP_LINBOX_SMOKE:-0}" -eq 0 ]]; then
    echo "[smoke.sh] running linbox_bench --smoke inside ${IMAGE_TAG}" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "set -e; cd /work/reference && make linbox_bench >/dev/null && /work/reference/linbox_bench --seed ${SEED} --smoke"
fi
# === linbox end ===

# === m4rie begin ===
# M4RIE smoke gate (jit:507b0036).
#
# `run.sh` doesn't yet wire m4rie_bench into the timing flow (the
# dispatch contract for 507b0036 is read-only on run.sh), so we run
# m4rie's correctness-oracle path directly here. This invokes the
# container's `m4rie_bench --smoke` to satisfy the per-cell n=16
# bitwise-equality check required by `dev/plans/sota_reference_acceptance_protocol.md`
# § 6 for every claimed (op, field) cell — currently matmul over GF(2^4),
# GF(2^8), GF(2^16). Failure exits non-zero and fails the smoke gate.
#
# Skipped silently when the image hasn't been rebuilt against the new
# Containerfile (e.g. --skip-build with a pre-507b0036 image).
if "${RUNTIME}" image inspect "${IMAGE_TAG}" >/dev/null 2>&1; then
    if "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c 'set -e; cd /work/reference && make -B m4rie_bench >/dev/null && /work/reference/m4rie_bench --smoke' \
        ; then
        echo "[smoke.sh] m4rie smoke OK" >&2
    else
        echo "[smoke.sh] m4rie smoke FAILED" >&2
        exit 1
    fi
else
    echo "[smoke.sh] note: ${IMAGE_TAG} not present; skipping m4rie smoke" >&2
fi
# === m4rie end ===

# === ntl begin ===
# Append NTL smoke rows.
echo "[smoke.sh] running ntl_bench --smoke inside ${IMAGE_TAG}" >&2
"${RUNTIME}" run --rm \
    --security-opt label=disable \
    -v "${HERE}:/work${MOUNT_OPTS}" \
    "${IMAGE_TAG}" \
    bash -c "set -e; cd /work/reference && make ntl_bench >/dev/null && ./ntl_bench --seed ${SEED} --smoke" \
    >> "${TARGET_CSV}"
# === ntl end ===

# === flint begin ===
# Append FLINT smoke rows.
echo "[smoke.sh] running flint_bench --smoke inside ${IMAGE_TAG}" >&2
"${RUNTIME}" run --rm \
    --security-opt label=disable \
    -v "${HERE}:/work${MOUNT_OPTS}" \
    "${IMAGE_TAG}" \
    bash -c "set -e; cd /work/reference && make flint_bench >/dev/null && ./flint_bench --seed ${SEED} --smoke" \
    >> "${TARGET_CSV}"

# Cross-equality oracle (per protocol §6). Emits no CSV rows; exits
# non-zero on any (op, field) mismatch at n=16.
echo "[smoke.sh] running ntl_flint_smoke equality oracle" >&2
"${RUNTIME}" run --rm \
    --security-opt label=disable \
    -v "${HERE}:/work${MOUNT_OPTS}" \
    "${IMAGE_TAG}" \
    bash -c "set -e; cd /work/reference && make ntl_flint_smoke >/dev/null && ./ntl_flint_smoke" \
    >&2
# === flint end ===

# === b13799ac GF(2^32) NTL bitwise-equality smoke begin ===
# NTL `mat_GF2E` matmul over GF(2^32) ↔ self-contained scalar reference
# bitwise-equality oracle at n=16. Implements the protocol § 6
# correctness contract for the Wave-3 GF(2^32) matmul promotion (issue
# b13799ac). The scalar reference is purely defined from the
# Conway-polynomial bits in
# `crates/gf2-core/src/primitive_polys.rs::standard(32)`, so a
# polynomial drift on the gf2-core side fails this oracle before any
# timing run. Skipped silently when the image was built without NTL.
if [[ "${GF2_SKIP_NTL_GF2POW32_SMOKE:-0}" -eq 0 ]]; then
    echo "[smoke.sh] running ntl_gf2pow32_smoke (GF(2^32) NTL ↔ scalar ref)" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "set -e; cd /work/reference && make ntl_gf2pow32_smoke >/dev/null && ./ntl_gf2pow32_smoke" \
        >&2
fi
# === b13799ac GF(2^32) NTL bitwise-equality smoke end ===

# === c3e79272 charpoly/minpoly cross-library smoke begin ===
# LinBox ↔ FLINT bitwise polynomial-coefficient equality oracle for
# charpoly + minpoly at n=16 across the four reference primes (issue
# c3e79272). Complements ntl_flint_smoke by adding the minpoly cross-
# check (NTL has no user-facing matrix-minpoly API). Skipped silently
# if the image was built without LinBox + FLINT both installed.
if [[ "${GF2_SKIP_CHARPOLY_MINPOLY_SMOKE:-0}" -eq 0 ]]; then
    echo "[smoke.sh] running charpoly_minpoly_smoke (LinBox ↔ FLINT)" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "set -e; cd /work/reference && make charpoly_minpoly_smoke >/dev/null && ./charpoly_minpoly_smoke" \
        >&2
fi
# === c3e79272 charpoly/minpoly cross-library smoke end ===

# === 47698404 sparse cross-equality oracle begin ===
# Cross-equality oracle for sparse cells at n=16 (issue 47698404,
# protocol § 6). Compares fflas-ffpack `fspmv` to an in-harness scalar
# SpMV reference for every (op, field) cell the sparse harnesses claim
# to cover. Exits non-zero on bitwise mismatch.
if [[ "${GF2_SKIP_SPARSE_SMOKE:-0}" -eq 0 ]]; then
    echo "[smoke.sh] running sparse_smoke (sparse cross-equality oracle)" >&2
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "set -e; cd /work/reference && make sparse_smoke >/dev/null && ./sparse_smoke" \
        >&2
fi
# === 47698404 sparse cross-equality oracle end ===

echo "[smoke.sh] OK — smoke CSV at ${TARGET_CSV}" >&2
