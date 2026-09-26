#!/usr/bin/env bash
# Build the pinned ISA-L campaign arm and retain its compiler evidence.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${here}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${repo}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
source "${here}/isal-build-pins.sh"
source_root="${GF2_ISAL_SOURCE:?set GF2_ISAL_SOURCE to the pinned checkout}"
[[ "$(git -C "${source_root}" rev-parse HEAD)" == 7c3479e0a9dac17f448603ec1ad64c7c625f530c ]]
[[ "$(sha256sum "${source_root}/raid/raid_base.c" | cut -d' ' -f1)" == 4fb636b16cebebadfb52236871421f1a143d3d7e488e7bc9b23b2fc25bd04aeb ]]
[[ "$(sha256sum "${source_root}/include/raid.h" | cut -d' ' -f1)" == 5e51c4abcd86ade41426cef509c3eeb06b0c0b3f9ef28080ddedb6af02a5cd03 ]]
[[ "$(sha256sum "${source_root}/LICENSE" | cut -d' ' -f1)" == bc8fd4a3d031e65e05e9c9e2add2c3f336ce527fa85c1e31031c808b58216217 ]]

target="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR for the ISA-L arm}"
arm="${target}/release/logical-isal-arm"
record="${repo}/dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/isal-arm-build-record.txt"
log="${repo}/dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/isal-arm-build.log"
mkdir -p "${target}"
next_log="$(mktemp "${target}/isal-build.XXXXXX")"
trap 'rm -f "${next_log}"' EXIT
unset MAKEFLAGS GNUMAKEFLAGS MFLAGS MAKEFILES
export CC="${isal_compiler_command}" CC_ENABLE_DEBUG_OUTPUT=1 CARGO_CI_NO_SCCACHE=1
export RUSTUP_TOOLCHAIN=1.95
GF2_ISAL_SOURCE="${source_root}" CARGO_TARGET_DIR="${target}" \
    ./scripts/cargo-budget.sh cargo build -vv --release --locked --features isal \
    --manifest-path "${here}/harness/Cargo.toml" --bin logical-isal-arm \
    >"${next_log}" 2>&1 || {
        tail -n 60 "${next_log}" >&2
        exit 1
    }
sed -i 's/[[:blank:]]*$//' "${next_log}"
[[ -x "${arm}" ]]
mapfile -t objects < <(find "${target}/release/build" -path '*/out/*' \
    -name '*raid_base.o' -type f | sort)
(( "${#objects[@]}" > 0 )) || { echo 'ISA-L build produced no raid_base object' >&2; exit 1; }
arm_digest="$(sha256sum "${arm}" | cut -d' ' -f1)"
compile_line="$(grep -F 'running:' "${next_log}" | grep -F "${isal_compiler_command}" | grep -F 'raid_base.c' | head -n 1 || true)"
if [[ -n "${compile_line}" ]]; then
    [[ "${compile_line}" != *sccache* ]] || {
        echo 'ISA-L compile ran through sccache rather than the pinned compiler directly' >&2
        exit 1
    }
    mv "${next_log}" "${log}"
else
    [[ -f "${record}" && -f "${log}" ]] || {
        echo 'ISA-L compile transcript is missing and no retained build record exists' >&2
        exit 1
    }
    grep -Fqx "compiler_command=${isal_compiler_command}" "${record}"
    grep -Fqx "compiler_identity=${isal_compiler_identity}" "${record}"
    grep -Fqx "make_override_state=cleared" "${record}"
    grep -Fqx "compile_driver=${isal_compiler_command}" "${record}"
    grep -Fqx "verbose_build_log_sha256=$(sha256sum "${log}" | cut -d' ' -f1)" "${record}"
    grep -F 'running:' "${log}" | grep -F "${isal_compiler_command}" | grep -Fq 'raid_base.c'
fi
recorded_object_digest="$(sed -n 's/^object_sha256=//p' "${record}" 2>/dev/null | head -n 1 || true)"
{
    printf 'compiler_command=%s\ncompiler_identity=%s\n' "${isal_compiler_command}" "${isal_compiler_identity}"
    printf 'make_override_channels=%s\nmake_override_state=cleared\n' "${isal_make_channels}"
    printf 'compile_driver=%s\n' "${isal_compiler_command}"
    printf 'verbose_build_log=%s\nverbose_build_log_sha256=%s\n' \
        "${log#"${repo}/"}" "$(sha256sum "${log}" | cut -d' ' -f1)"
    for object in "${objects[@]}"; do
        producer="$(isal_object_producers "${object}")"
        digest="$(sha256sum "${object}" | cut -d' ' -f1)"
        [[ -n "${producer}" && "${producer}" == *"$("${isal_compiler_command}" -dumpfullversion)"* ]]
        [[ -n "${compile_line}" || "${digest}" == "${recorded_object_digest}" ]]
        printf 'object=%s\nobject_sha256=%s\nobject_comment=%s\n' \
            "${object#"${repo}/"}" "${digest}" "${producer}"
    done
    printf 'arm=%s\narm_sha256=%s\n' "${arm#"${repo}/"}" "${arm_digest}"
} >"${record}"
printf 'ISA-L arm build provenance: %s\n' "${record#"${repo}/"}" >&2
