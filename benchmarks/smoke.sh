#!/usr/bin/env bash
# benchmarks/smoke.sh — fast end-to-end build + run of the reference
# harnesses, intended as a manual substitute for the CI gate (which
# cannot run podman).
#
# Behaviour:
#   * Forces warmup=0, iters=1 so the harness produces one CSV row per
#     (field, op, size, regime) cell as cheaply as possible.
#   * Builds the image (or reuses if already built).
#   * Writes the CSV under benchmarks/results/smoke-<timestamp>.csv so
#     it does not get confused with a real timing run.
#   * Returns non-zero if any podman / build / run step fails — useful
#     for `./benchmarks/smoke.sh && echo OK` in a pre-PR script.
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

"${HERE}/run.sh" --image-tag "${IMAGE_TAG}" "$@"

# === linbox begin ===
# Per SOTA reference acceptance protocol § 6 *Correctness-oracle harness*,
# additionally invoke linbox_bench --smoke to assert the n=16 per-op
# equality contract (charpoly Cayley-Hamilton, minpoly annihilation,
# solve A·x ≡ b) for every (op, field) cell the LinBox harness covers.
# A non-zero exit here is a hard semantics-mismatch failure per § 9.
if [[ "${GF2_SKIP_LINBOX_SMOKE:-0}" -eq 0 ]]; then
    SEED="$(grep -v '^[[:space:]]*#' "${HERE}/seeds/seed.txt" \
              | grep -v '^[[:space:]]*$' \
              | head -n 1 \
              | tr -d '[:space:]')"
    echo "[smoke.sh] running linbox_bench --smoke inside ${IMAGE_TAG}" >&2
    MOUNT_OPTS=":Z,U"
    if [[ "${RUNTIME}" == "docker" ]]; then
        MOUNT_OPTS=""
    fi
    "${RUNTIME}" run --rm \
        --security-opt label=disable \
        -v "${HERE}:/work${MOUNT_OPTS}" \
        "${IMAGE_TAG}" \
        bash -c "set -e; cd /work/reference && make linbox_bench >/dev/null && /work/reference/linbox_bench --seed ${SEED} --smoke"
fi
# === linbox end ===
