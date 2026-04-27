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
# If cargo-show-asm reports multiple matches for a symbol, suffix the path
# with #<index>, e.g. gf2_kernels_simd::x86::clmul::clmul_batch#0.
#
# Env knobs:
#   TARGET_CPU       passed to RUSTFLAGS as -C target-cpu=$TARGET_CPU.
#                    Default: <empty> (rely on #[target_feature] attributes
#                    in the kernel sources; set explicitly e.g.
#                    TARGET_CPU=x86-64-v3 for portable baselines, or
#                    TARGET_CPU=native for host-tuned dumps).
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
                      Append #<index> to disambiguate cargo-show-asm matches.
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
    local asm_sym="$sym"
    local display_sym="$sym"
    local selector_args=()
    if [[ "$asm_sym" =~ ^(.+)#([0-9]+)$ ]]; then
        asm_sym="${BASH_REMATCH[1]}"
        selector_args=("${BASH_REMATCH[2]}")
        display_sym="${asm_sym} (index ${BASH_REMATCH[2]})"
    fi
    {
        echo
        echo ";=========================================================="
        echo "; symbol: ${display_sym}"
        echo ";=========================================================="
        echo
        if ! RUSTFLAGS="$rustflags_value" cargo asm \
            -p "$crate" --lib "$asm_sym" "${selector_args[@]}" --simplify 2>&1; then
            echo
            echo "; (cargo asm failed for symbol ${display_sym}; output above)" >&2
            return 1
        fi
    } >> "$out_path"
}

emit_with_fallback() {
    local sym="$1"
    local asm_sym="$sym"
    if [[ "$asm_sym" =~ ^(.+)#([0-9]+)$ ]]; then
        asm_sym="${BASH_REMATCH[1]}"
    fi
    local tmp
    tmp=$(mktemp -d)
    # Clean up the temp dir on function return. Using an explicit
    # `rm -rf -- "$tmp"` substituted into the trap body (rather than
    # quoting `$tmp` for later expansion) avoids unbound-variable errors
    # under `set -u` once the function returns and the local `$tmp` goes
    # out of scope before the RETURN trap fires.
    # shellcheck disable=SC2064  # intentional eager expansion
    trap "rm -rf -- '$tmp'" RETURN
    {
        echo
        echo ";=========================================================="
        echo "; symbol: ${sym} (cargo-rustc whole-crate fallback)"
        echo ";=========================================================="
        echo
        # cargo rustc dumps the whole crate's asm under <target-dir>/.../*.s.
        #
        # IMPORTANT: do NOT pass `--out-dir` in the trailing rustc args (i.e.
        # after `--`). Cargo already injects its own `--out-dir` when invoking
        # rustc; passing a second one triggers
        #   error: Option 'out-dir' given more than once
        # Instead, redirect cargo's own output via `--target-dir "$tmp"`,
        # which isolates this asm dump from the caller's main `target/`
        # without colliding with cargo's internal flag plumbing. Then locate
        # the emitted `.s` file under "$tmp/release/deps/".
        #
        # Only pass -C target-cpu when explicitly set; otherwise omit the
        # flag entirely so an empty value never reaches rustc.
        local rustc_args=(--emit=asm)
        if [[ -n "$target_cpu" ]]; then
            rustc_args+=(-C "target-cpu=$target_cpu")
        fi
        # Buffer cargo output to a file so cargo's exit status and stdout
        # are decoupled from `head -5`'s pipe-close behaviour. Under
        # `set -euo pipefail`, the previous `cargo … 2>&1 | head -5`
        # pipeline aborted with status 141 (SIGPIPE) whenever cargo
        # emitted more than 5 lines — which is the normal case for a
        # fresh `--target-dir`. With a file buffer, real cargo failures
        # are surfaced as fallback failures and head only truncates the
        # log dump.
        local cargo_log="$tmp/cargo.out"
        if ! RUSTFLAGS="$rustflags_value" \
                cargo rustc --release --target-dir "$tmp" -p "$crate" --lib -- \
                    "${rustc_args[@]}" >"$cargo_log" 2>&1; then
            cat "$cargo_log"
            echo
            echo "; (cargo rustc failed for ${sym}; output above)"
            return 1
        fi
        head -5 "$cargo_log"
        local asm_files
        asm_files=$(find "$tmp" -name '*.s' 2>/dev/null || true)
        if [[ -z "$asm_files" ]]; then
            echo "; (no .s files produced by fallback)"
            return 1
        fi
        # Locate the symbol in the (Itanium-mangled) raw asm. The .s file
        # contains names like `_ZN16gf2_kernels_simd3x864avx213avx2_xor_into...`,
        # so we cannot grep for the unmangled `crate::path::name` form. Match
        # on the function's basename (last `::` segment), which appears
        # verbatim inside the mangled string.
        local needle
        needle="${asm_sym##*::}"
        local found=0
        for f in $asm_files; do
            if grep -q "$needle" "$f" 2>/dev/null; then
                grep -n -A 80 "$needle" "$f" | head -200
                found=1
                break
            fi
        done
        if (( ! found )); then
            echo "; (fallback: symbol ${needle} not found in any .s file under ${tmp})"
            return 1
        fi
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
