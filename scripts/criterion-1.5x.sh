#!/usr/bin/env bash
# JIT gate runner: criterion-1.5x.
#
# Thin wrapper that bridges JIT's --pass-context envelope to the
# kernel-id-positional `dev/benchmarks/ppc-compare.sh` harness.
#
# The PPC-spiral (epic:gf2-core-ppc-spiral) per-kernel issues each carry a
# `ppc-kernel:<id>` label (e.g., `ppc-kernel:A1`) identifying which row of
# `dev/benchmarks/ppc-baselines.json` they belong to. JIT, when invoking a
# `--pass-context` gate, writes the issue's full JSON (labels included) to
# the path in $JIT_CONTEXT_FILE. This wrapper reads that file, extracts the
# `ppc-kernel:<id>` label, and forwards to the harness.
#
# Convention (mirrored in dev/benchmarks/ppc-baselines.json `comment` field
# and in crates/gf2-kernels-simd/README.md):
#
#   Every issue that should be gated on "geomean speedup >= 1.5x for kernel
#   X" must carry exactly one `ppc-kernel:X` label. Lead wires the label
#   when defining/dispatching the issue.
#
# Lead registers this gate post-merge with:
#
#   jit gate define criterion-1.5x \
#       --title "Criterion 1.5x speedup" \
#       --description "geomean speedup vs pinned baseline >= 1.5x for the kernel named by ppc-kernel:<id>" \
#       --mode auto --stage postcheck \
#       --pass-context \
#       --checker-command "./scripts/criterion-1.5x.sh"
#
# Exit codes (forwarded transparently from ppc-compare.sh):
#   0 — geomean speedup >= 1.5x (PASS)
#   1 — geomean speedup <  1.5x (FAIL — kernel below the bar)
#   2 — infrastructure error (missing label, missing context, missing
#       manifest entry, missing estimates.json, ...)
#   3 — baseline still "TBD-..." (b2ecd2ff pending — distinct from FAIL
#       because nothing is wrong with the kernel; the manifest just hasn't
#       been pinned yet)
#
# Discovered in jit:4f845881 R1 (R0 review found the gate was decorative
# without this wrapper).

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$repo_root"

HARNESS="${PPC_COMPARE_SCRIPT:-./dev/benchmarks/ppc-compare.sh}"

if [[ -z "${JIT_CONTEXT_FILE:-}" ]]; then
    echo "ERROR: JIT_CONTEXT_FILE not set. The criterion-1.5x gate requires --pass-context." >&2
    echo "       Lead: redefine this gate with --pass-context (see scripts/criterion-1.5x.sh header)." >&2
    exit 2
fi

if [[ ! -f "$JIT_CONTEXT_FILE" ]]; then
    echo "ERROR: JIT_CONTEXT_FILE points to a nonexistent path: $JIT_CONTEXT_FILE" >&2
    exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: jq not on PATH (required to parse JIT context labels)." >&2
    exit 2
fi

# Extract the first label of form `ppc-kernel:<id>`. Strip the prefix.
KERNEL_ID=$(jq -r '.labels // [] | map(select(startswith("ppc-kernel:"))) | .[0] // empty | sub("^ppc-kernel:"; "")' "$JIT_CONTEXT_FILE")

if [[ -z "$KERNEL_ID" ]]; then
    {
        echo "ERROR: criterion-1.5x: no \`ppc-kernel:<id>\` label on this issue."
        echo
        echo "Add a \`ppc-kernel:<id>\` label to this issue per the criterion-1.5x"
        echo "gate convention (see scripts/criterion-1.5x.sh and"
        echo "dev/benchmarks/ppc-baselines.json's comment field for the kernel-id"
        echo "values: A1, A2, A3, B1, B2, B3, C1, C2, C3, C4, C5, D1, D2)."
        echo
        echo "Example:"
        echo "  jit issue update <issue-id> --add-label ppc-kernel:A1"
    } >&2
    exit 2
fi

if [[ ! -x "$HARNESS" ]]; then
    echo "ERROR: criterion-1.5x: harness not executable: $HARNESS" >&2
    exit 2
fi

# Forward to the harness; pass-through exit code unchanged.
exec "$HARNESS" "$KERNEL_ID"
