#!/usr/bin/env bash
# Builds and runs the untimed ISA-L xor_gen_base semantic qualification probe.
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "${here}/isal-build-pins.sh"
source_root=${ISAL_SOURCE:?Set ISAL_SOURCE to an ISA-L v2.32.1 checkout.}
want_revision=7c3479e0a9dac17f448603ec1ad64c7c625f530c
want_raid_base=4fb636b16cebebadfb52236871421f1a143d3d7e488e7bc9b23b2fc25bd04aeb
want_raid_h=5e51c4abcd86ade41426cef509c3eeb06b0c0b3f9ef28080ddedb6af02a5cd03
want_license=bc8fd4a3d031e65e05e9c9e2add2c3f336ce527fa85c1e31031c808b58216217

[[ $(git -C "$source_root" rev-parse HEAD) == "$want_revision" ]]
[[ $(sha256sum "$source_root/raid/raid_base.c" | awk '{print $1}') == "$want_raid_base" ]]
[[ $(sha256sum "$source_root/include/raid.h" | awk '{print $1}') == "$want_raid_h" ]]
[[ $(sha256sum "$source_root/LICENSE" | awk '{print $1}') == "$want_license" ]]

build_dir=$(mktemp -d "${TMPDIR:-/tmp}/gf2-isal-xor-probe.XXXXXX")
trap 'rm -rf "$build_dir"' EXIT
build_log="${ISAL_PROBE_BUILD_LOG:-${repo}/dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/isal-probe-build.log}"
flags=(-std=c11 -O3 -march=native -Wall -Wextra -Werror)
unset MAKEFLAGS GNUMAKEFLAGS MFLAGS MAKEFILES
{
    printf 'compiler_command=%s\ncompiler_identity=%s\n' "${isal_compiler_command}" "${isal_compiler_identity}"
    printf 'make_override_channels=%s\nmake_override_state=cleared\n' "${isal_make_channels}"
    printf '+ '; printf '%q ' "${isal_compiler_command}" -v "${flags[@]}" -I"${source_root}/include" -c "${here}/isal_xor_probe.c" -o "${build_dir}/probe.o"
    printf '\n'
    "${isal_compiler_command}" -v "${flags[@]}" -I"${source_root}/include" -c "${here}/isal_xor_probe.c" -o "${build_dir}/probe.o"
    printf '+ '; printf '%q ' "${isal_compiler_command}" -v "${flags[@]}" -I"${source_root}/include" -c "${source_root}/raid/raid_base.c" -o "${build_dir}/raid_base.o"
    printf '\n'
    "${isal_compiler_command}" -v "${flags[@]}" -I"${source_root}/include" -c "${source_root}/raid/raid_base.c" -o "${build_dir}/raid_base.o"
    printf '+ '; printf '%q ' "${isal_compiler_command}" -v "${flags[@]}" "${build_dir}/probe.o" "${build_dir}/raid_base.o" -o "${build_dir}/isal_xor_probe"
    printf '\n'
    "${isal_compiler_command}" -v "${flags[@]}" "${build_dir}/probe.o" "${build_dir}/raid_base.o" -o "${build_dir}/isal_xor_probe"
} >"${build_log}" 2>&1
sed -i 's/[[:blank:]]*$//' "${build_log}"
printf 'compiler_command=%s\ncompiler_identity=%s\n' "${isal_compiler_command}" "${isal_compiler_identity}"
printf 'make_override_channels=%s\nmake_override_state=cleared\n' "${isal_make_channels}"
printf 'verbose_build_log=%s sha256=%s\n' "${build_log}" "$(sha256sum "${build_log}" | cut -d' ' -f1)"
for object in probe.o raid_base.o; do
    printf 'object=%s sha256=%s .comment=%s\n' "$object" \
        "$(sha256sum "${build_dir}/${object}" | cut -d' ' -f1)" \
        "$(isal_object_producers "${build_dir}/${object}")"
done
nm -g --defined-only "$build_dir/isal_xor_probe" | awk '$3 == "xor_gen_base"'
objdump -d --no-show-raw-insn --disassemble=xor_gen_base "$build_dir/isal_xor_probe"
"$build_dir/isal_xor_probe"

if command -v nasm >/dev/null 2>&1; then
    printf 'nasm=%s\n' "$(nasm -v)"
else
    printf 'nasm=absent; public dispatched xor_gen is unavailable on this host\n'
fi
printf 'cost-audit=pointer-array,32B-alignment,fresh-output,GF2-destination-copy,conversion,output-observation\n'
printf 'source-root=%s\n' "$source_root"
printf 'repo=%s\n' "$repo"
