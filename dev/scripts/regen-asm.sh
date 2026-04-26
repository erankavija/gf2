#!/usr/bin/env bash
# Regenerates per-symbol asm artefacts for SIMD review.
#
# Convention (PPC-spiral I3): each
#   crates/gf2-kernels-simd/src/x86/<module>.rs
# carries a sibling
#   crates/gf2-kernels-simd/src/x86/asm/<module>.asm.txt
# regenerated after every spiral-step edit so reviewers can confirm the
# expected mnemonics (vpxor, vpclmulqdq, vpternlogq, ...) actually landed.
#
# Usage:
#   ./dev/scripts/regen-asm.sh <crate> <symbol> <output-path> [extra-symbol ...]
#
# The <symbol> is a fully-qualified Rust path, e.g.
#   gf2_kernels_simd::x86::avx2::avx2_xor_into
# Pass additional symbols as further positional args; each is appended
# to the same artefact under its own divider.
#
# Env knobs:
#   TARGET_CPU       passed to RUSTFLAGS as -C target-cpu=$TARGET_CPU.
#                    Default: native.
#   EXTRA_RUSTFLAGS  appended verbatim to RUSTFLAGS.
#
# Tooling: requires cargo-show-asm (https://github.com/pacak/cargo-show-asm).
# Install with: cargo install cargo-show-asm --locked
#
# If cargo-show-asm is unavailable AND the env var REGEN_ASM_FALLBACK=1
# is set, the script falls back to `cargo rustc --emit=asm`. That fallback
# dumps the entire crate's asm into the output, which is noisier and not
# per-symbol; prefer the proper tool wherever possible.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
usage: regen-asm.sh <crate> <symbol> <output-path> [extra-symbol ...]

  <crate>            cargo package name (e.g. gf2-kernels-simd)
  <symbol>           fully-qualified Rust path to the function
                     (e.g. gf2_kernels_simd::x86::avx2::avx2_xor_into)
  <output-path>      destination file; parent directory will be created
  [extra-symbol ...] further symbols appended to the same file
USAGE
}

if [[ $# -lt 3 ]]; then
    usage
    exit 2
fi

crate="$1"
primary_symbol="$2"
out_path="$3"
shift 3
extra_symbols=("$@")

# Default target-cpu is empty so that #[target_feature] gating in the
# kernel source remains the only source of feature-set information. Set
# TARGET_CPU=native (or x86-64-v3 etc.) to globally enable a baseline.
# Note: target-cpu=native often causes #[target_feature] wrappers to be
# inlined away, which can make per-symbol asm extraction fail.
target_cpu="${TARGET_CPU:-}"
extra_rustflags="${EXTRA_RUSTFLAGS:-}"
if [[ -n "$target_cpu" ]]; then
    rustflags_value="-C target-cpu=${target_cpu} ${extra_rustflags}"
else
    rustflags_value="${extra_rustflags}"
fi

out_dir=$(dirname -- "$out_path")
mkdir -p -- "$out_dir"

rustc_version=$(rustc --version 2>/dev/null || echo "rustc: unknown")
host_triple=$(rustc -vV 2>/dev/null | grep '^host:' | cut -d' ' -f2 || echo "unknown")
git_sha=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
date_now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Resolve the asm-emission tool.
asm_tool=""
if command -v cargo-asm >/dev/null 2>&1; then
    # cargo-show-asm installs the binary as `cargo-asm`, invoked via
    # `cargo asm`. Probe both forms to be safe.
    asm_tool="show-asm"
elif command -v cargo-show-asm >/dev/null 2>&1; then
    asm_tool="show-asm"
fi

if [[ -z "$asm_tool" && "${REGEN_ASM_FALLBACK:-0}" != "1" ]]; then
    cat >&2 <<'EOF'
error: cargo-show-asm not found.
       install with:  cargo install cargo-show-asm --locked
       or set REGEN_ASM_FALLBACK=1 to use the cargo-rustc fallback
       (whole-crate asm dump; much noisier, not per-symbol).
EOF
    exit 1
fi

# Write banner.
{
    echo "; gf2-core PPC-spiral asm artefact"
    echo "; ---------------------------------------------------------"
    echo "; crate         : ${crate}"
    echo "; primary       : ${primary_symbol}"
    if (( ${#extra_symbols[@]} > 0 )); then
        echo "; extra symbols : ${extra_symbols[*]}"
    fi
    echo "; output        : ${out_path}"
    echo "; rustc         : ${rustc_version}"
    echo "; host triple   : ${host_triple}"
    echo "; target-cpu    : ${target_cpu:-<unset; rely on #[target_feature]>}"
    echo "; RUSTFLAGS     : ${rustflags_value:-<empty>}"
    echo "; commit        : ${git_sha}"
    echo "; regenerated   : ${date_now}"
    if [[ -n "$asm_tool" ]]; then
        echo "; tool          : cargo-show-asm"
    else
        echo "; tool          : cargo rustc --emit=asm (fallback)"
    fi
    echo "; ---------------------------------------------------------"
    echo
} > "$out_path"

emit_with_show_asm() {
    local sym="$1"
    {
        echo
        echo ";=========================================================="
        echo "; symbol: ${sym}"
        echo ";=========================================================="
        echo
        if ! RUSTFLAGS="$rustflags_value" cargo asm \
            -p "$crate" --lib "$sym" --simplify 2>&1; then
            echo
            echo "; (cargo asm failed for symbol ${sym}; output above)" >&2
            return 1
        fi
    } >> "$out_path"
}

emit_with_fallback() {
    local sym="$1"
    local tmp
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' RETURN
    {
        echo
        echo ";=========================================================="
        echo "; symbol: ${sym} (cargo-rustc whole-crate fallback)"
        echo ";=========================================================="
        echo
        # cargo rustc dumps the whole crate's asm under target/.../*.s
        RUSTFLAGS="$rustflags_value" \
            cargo rustc --release -p "$crate" --lib -- \
                --emit=asm -C target-cpu="$target_cpu" \
                --out-dir "$tmp" 2>&1 | head -5
        local asm_files
        asm_files=$(find "$tmp" -maxdepth 2 -name '*.s' 2>/dev/null || true)
        if [[ -z "$asm_files" ]]; then
            echo "; (no .s files produced by fallback)"
            return 1
        fi
        # Demangle and grep neighbourhood of the requested symbol.
        local needle
        needle=$(echo "$sym" | tr -d ' ')
        for f in $asm_files; do
            if grep -q "$needle" "$f" 2>/dev/null; then
                grep -n -A 80 "$needle" "$f" | head -200
                break
            fi
        done
    } >> "$out_path"
}

emit() {
    local sym="$1"
    if [[ "$asm_tool" == "show-asm" ]]; then
        emit_with_show_asm "$sym"
    else
        emit_with_fallback "$sym"
    fi
}

emit "$primary_symbol"
for sym in "${extra_symbols[@]}"; do
    emit "$sym"
done

echo "regen-asm: wrote ${out_path}" >&2
