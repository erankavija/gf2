#!/usr/bin/env bash
# Build and run the jit:92385645 semantic probe against M4RI's pinned build.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${here}" rev-parse --show-toplevel)"
# shellcheck source=m4ri-build-pins.sh
. "${here}/m4ri-build-pins.sh"

ext="${M4RI_EXT:-${repo}/.agents/ext/92385645}"
prefix="${ext}/prefix-${m4ri_generation}"
source_dir="${ext}/m4ri-src-${m4ri_generation}"
record="${prefix}/gf2-m4ri-build-record.txt"
build_log="${prefix}/gf2-m4ri-build-log.txt"
bin="${ext}/m4ri_matvec_probe-${m4ri_generation}"
compiler_producer="$(m4ri_compiler_producer)"

# This is deliberately M4RI-only: the prior external-comparator bundle also
# builds unrelated ISA-L, Bitshuffle, and Rust arms.  A semantic qualification
# should not make those unrelated build products a prerequisite.
mkdir -p "${ext}"
if [[ ! -f "${ext}/${m4ri_archive}" ]]; then
    curl -fsSL -o "${ext}/${m4ri_archive}" \
        "https://github.com/malb/m4ri/releases/download/${m4ri_version}/${m4ri_archive}"
fi
(cd "${ext}" && echo "${m4ri_archive_sha256}  ${m4ri_archive}" | sha256sum -c -)

if [[ -f "${prefix}/lib/libm4ri.so" ]]; then
    [[ -f "${record}" ]] || {
        printf 'unverified M4RI cache: missing %s\n' "${record}" >&2
        exit 1
    }
    grep -Fqx "archive_sha256=${m4ri_archive_sha256}" "${record}"
    grep -Fqx "compiler_command=${m4ri_compiler_command}" "${record}"
    grep -Fqx "compiler_identity=${m4ri_compiler_identity}" "${record}"
    grep -Fqx "compiler_producer=${compiler_producer}" "${record}"
    grep -Fqx "build_flags=${m4ri_build_flags}" "${record}"
    grep -Fqx "configure_args=${m4ri_configure_args}" "${record}"
    grep -Fqx "cppflags=${m4ri_cppflags}" "${record}"
    grep -Fqx "ldflags=${m4ri_ldflags}" "${record}"
    grep -Fqx "libs=${m4ri_libs}" "${record}"
    grep -Fqx "config_site=${m4ri_config_site}" "${record}"
    grep -Fqx "make_override_channels=${m4ri_make_channels}" "${record}"
    grep -Fqx "make_override_state=${m4ri_make_channel_state}" "${record}"
    grep -Fqx "compile_driver=${m4ri_compiler_command}" "${record}"
    # Reuse re-derives the compiler from the retained build log and from the
    # installed library's own producer strings, so a record alone cannot
    # attest to a compiler that never ran.
    grep -Fqx "compile_invocations=$(m4ri_verify_build_log "${build_log}")" "${record}"
    library_producers="$(m4ri_producers "${prefix}/lib/libm4ri.so")"
    [[ "${library_producers}" == "${compiler_producer}" ]]
    grep -Fqx "library_producers=${library_producers}" "${record}"
    recorded_library_sha256="$(sed -n 's/^library_sha256=//p' "${record}")"
    [[ -n "${recorded_library_sha256}" ]]
    [[ "$(sha256sum "${prefix}/lib/libm4ri.so" | awk '{print $1}')" == \
        "${recorded_library_sha256}" ]]
else
    [[ ! -e "${source_dir}" && ! -e "${prefix}" ]] || {
        printf 'incomplete M4RI qualification cache under %s; choose a clean M4RI_EXT\n' \
            "${ext}" >&2
        exit 1
    }
    mkdir -p "${source_dir}" "${prefix}"
    tar -C "${source_dir}" --strip-components=1 -xf "${ext}/${m4ri_archive}"
    (
        cd "${source_dir}"
        # The nested shell keeps configure, build, and install under one
        # cargo-budget invocation while expanding its own build-job setting.
        # It clears the ambient Autoconf inputs, the ambient tool overrides,
        # and every GNU Make override channel named in the pins, then logs each
        # verbose invocation for the provenance checks below.
        # shellcheck disable=SC2016
        CARGO_CI_NO_SCCACHE=1 "${repo}/scripts/cargo-budget.sh" \
            bash -c 'set -euo pipefail
                export CC="$1" CFLAGS="$2" CPPFLAGS="" LDFLAGS="" LIBS="" CONFIG_SITE=/dev/null
                unset AR AS CPP LD NM OBJDUMP RANLIB STRIP
                # Word-split on purpose: "$5" is the pinned channel list.
                unset $5
                if ! { ./configure --prefix="$3" --disable-static &&
                       make -j"${CARGO_BUILD_JOBS:-1}" V=1 &&
                       make install V=1; } >"$4" 2>&1; then
                    tail -n 40 "$4" >&2
                    exit 1
                fi' \
            _ "${m4ri_compiler_command}" "${m4ri_build_flags}" "${prefix}" \
            "${build_log}" "${m4ri_make_channels}"
    )
    # Configure's own selection, which a make-time command-line override would
    # leave untouched; the build log and producer checks cover that override.
    grep -Fqx "CC = ${m4ri_compiler_command}" "${source_dir}/Makefile"
    compile_invocations="$(m4ri_verify_build_log "${build_log}")"
    library_producers="$(m4ri_producers "${prefix}/lib/libm4ri.so")"
    [[ "${library_producers}" == "${compiler_producer}" ]]
    library_sha256="$(sha256sum "${prefix}/lib/libm4ri.so" | awk '{print $1}')"
    {
        printf 'archive_sha256=%s\n' "${m4ri_archive_sha256}"
        printf 'compiler_command=%s\n' "${m4ri_compiler_command}"
        printf 'compiler_identity=%s\n' "${m4ri_compiler_identity}"
        printf 'compiler_producer=%s\n' "${compiler_producer}"
        printf 'build_flags=%s\n' "${m4ri_build_flags}"
        printf 'configure_args=%s\n' "${m4ri_configure_args}"
        printf 'cppflags=%s\n' "${m4ri_cppflags}"
        printf 'ldflags=%s\n' "${m4ri_ldflags}"
        printf 'libs=%s\n' "${m4ri_libs}"
        printf 'config_site=%s\n' "${m4ri_config_site}"
        printf 'make_override_channels=%s\n' "${m4ri_make_channels}"
        printf 'make_override_state=%s\n' "${m4ri_make_channel_state}"
        printf 'compile_driver=%s\n' "${m4ri_compiler_command}"
        printf 'compile_invocations=%s\n' "${compile_invocations}"
        printf 'library_producers=%s\n' "${library_producers}"
        printf 'library_sha256=%s\n' "${library_sha256}"
    } >"${record}"
fi

"${m4ri_compiler_command}" -std=c11 -O3 -march=native -Wall -Wextra -Werror \
    -I"${prefix}/include" "${here}/m4ri_matvec_probe.c" \
    -L"${prefix}/lib" -Wl,-rpath,"${prefix}/lib" -lm4ri -lm \
    -o "${bin}"
"${bin}"
