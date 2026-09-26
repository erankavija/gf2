#!/usr/bin/env bash
# Release assembly of the frozen dense-parity baseline arms.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
OUT="${HERE}/asm/dense-baseline"
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
mkdir -p "${OUT}"

SYMBOLS=(
    'public-matvec|gf2_core::matrix::BitMatrix::matvec'
    'simd-matvec|gf2_core::matrix::BitMatrix::matvec_simd'
    'scalar-matvec|gf2_core::matrix::BitMatrix::matvec_scalar'
    'avx2-fused|gf2_kernels_simd::x86::avx2::avx2_and_popcnt'
    'bundle-entry|gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn'
    'retained-output|dense_parity_harness::routes::OutputSink'
    'arm-body|dense_arm::run::{{closure}}'
)
for build in simd scalar; do
    binary="target/e1f9a78f-$([[ "${build}" == simd ]] && echo arms || echo scalar-arm)/release/dense-arm"
    [[ -x "${binary}" ]] || { echo "missing ${binary}" >&2; exit 2; }
    dir="${OUT}/${build}"
    mkdir -p "${dir}"
    digest="$(sha256sum "${binary}" | cut -d' ' -f1)"
    {
        echo "# Dense baseline release assembly"
        echo "# binary: ${binary}"
        echo "# sha256: ${digest}"
        echo "# rustc: $(rustc --version)"
        echo "# objdump: $(objdump --version | head -n 1)"
        echo "# label symbols bytes source-pattern"
    } >"${dir}/index.txt"
    nm -S -C --defined-only "${binary}" >"${dir}/symbols.txt"
    for spec in "${SYMBOLS[@]}"; do
        label="${spec%%|*}"
        pattern="${spec#*|}"
        file="${dir}/${label}.asm.txt"
        {
            echo "# ${label}: ${pattern}"
            echo "# binary sha256: ${digest}"
        } >"${file}"
        count=0 bytes=0
        while read -r address size type name; do
            [[ "${type}" == t || "${type}" == T ]] || continue
            if [[ "${label}" == retained-output || "${label}" == arm-body ]]; then
                [[ "${name}" == *"${pattern}"* ]] || continue
            else
                [[ "${name}" == "${pattern}" ]] || continue
            fi
            count=$((count + 1))
            bytes=$((bytes + 0x${size}))
            {
                echo
                echo "## ${name} at ${address}, 0x${size} bytes"
                objdump -d --no-show-raw-insn -C \
                    --start-address="0x${address}" \
                    --stop-address="$((0x${address} + 0x${size}))" \
                    "${binary}" | sed -n '/>:/,$p'
            } >>"${file}"
        done <"${dir}/symbols.txt"
        printf '%s %s %s %s\n' "${label}" "${count}" "${bytes}" "${pattern}" >>"${dir}/index.txt"
    done
done
echo "assembly written to ${OUT#"${REPO}/"}"
