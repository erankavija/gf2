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

export GF2_BENCH_WARMUP=0
export GF2_BENCH_ITERS=1

TS="$(date -u +%Y%m%dT%H%M%SZ)"
exec "${HERE}/run.sh" --image-tag "gf2-bench:smoke" "$@"
