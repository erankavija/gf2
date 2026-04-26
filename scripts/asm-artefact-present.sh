#!/usr/bin/env bash
# JIT gate runner: asm-artefact-present.
#
# Enforces the PPC-spiral I3 convention (see dev/plans/gf2_core_ppc_spiral.md):
# every change to a SIMD source file under
#   crates/gf2-kernels-simd/src/x86/<module>.rs
# must be accompanied by a corresponding regenerated artefact at
#   crates/gf2-kernels-simd/src/x86/asm/<module>.asm.txt
# (or a sibling matching <module>_<fn>.asm.txt for multi-function modules).
#
# Vacuously passes if no SIMD source file changed at HEAD.
#
# Exit codes:
#   0  gate satisfied (or vacuously passed)
#   1  SIMD source modified without matching asm.txt update

set -euo pipefail

repo_root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$repo_root"

# Defensive: handle a fresh repo with no HEAD~1.
if ! git rev-parse HEAD~1 >/dev/null 2>&1; then
    echo "asm-artefact-present: HEAD~1 not available; vacuously passing"
    exit 0
fi

src_prefix="crates/gf2-kernels-simd/src/x86/"
asm_prefix="crates/gf2-kernels-simd/src/x86/asm/"

# Files modified in the latest commit.
mapfile -t changed < <(git diff --name-only HEAD~1 HEAD)

# Filter to SIMD .rs files (excluding mod.rs).
simd_changed=()
for f in "${changed[@]}"; do
    case "$f" in
        "${src_prefix}mod.rs")
            continue
            ;;
        "${src_prefix}"*.rs)
            simd_changed+=("$f")
            ;;
    esac
done

if (( ${#simd_changed[@]} == 0 )); then
    echo "asm-artefact-present: no SIMD source files changed; vacuously passing"
    exit 0
fi

# Asm files modified in the same commit (we only care about modified ones).
asm_changed=()
for f in "${changed[@]}"; do
    case "$f" in
        "${asm_prefix}"*.asm.txt)
            asm_changed+=("$f")
            ;;
    esac
done

missing=()
for src in "${simd_changed[@]}"; do
    # Module name = basename without .rs
    base=$(basename -- "$src" .rs)
    expected_exact="${asm_prefix}${base}.asm.txt"
    matched=0
    for a in "${asm_changed[@]}"; do
        if [[ "$a" == "$expected_exact" ]]; then
            matched=1
            break
        fi
        # Allow per-fn artefacts: <module>_<fn>.asm.txt
        case "$a" in
            "${asm_prefix}${base}_"*.asm.txt)
                matched=1
                break
                ;;
        esac
    done
    if (( matched == 0 )); then
        missing+=("$src")
    fi
done

if (( ${#missing[@]} > 0 )); then
    {
        echo "asm-artefact-present: FAIL"
        echo
        echo "The following SIMD source files were modified at HEAD but no"
        echo "matching ${asm_prefix}<module>.asm.txt (or <module>_<fn>.asm.txt)"
        echo "was modified in the same commit:"
        echo
        for src in "${missing[@]}"; do
            base=$(basename -- "$src" .rs)
            echo "  ${src}"
            echo "    expected: ${asm_prefix}${base}.asm.txt"
        done
        echo
        echo "Regenerate the artefact via:"
        echo "  ./dev/scripts/regen-asm.sh gf2-kernels-simd <symbol> \\"
        echo "      ${asm_prefix}<module>.asm.txt"
        echo
        echo "See dev/plans/gf2_core_ppc_spiral.md (section I3) for rationale."
    } >&2
    exit 1
fi

echo "asm-artefact-present: PASS (${#simd_changed[@]} SIMD file(s) covered)"
exit 0
