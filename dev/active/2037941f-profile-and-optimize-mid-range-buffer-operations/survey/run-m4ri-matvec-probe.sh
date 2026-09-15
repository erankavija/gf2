#!/usr/bin/env bash
# Build and run the jit:92385645 semantic probe against M4RI's pinned build.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${here}" rev-parse --show-toplevel)"
ext="${M4RI_EXT:-${repo}/.agents/ext/92385645}"
prefix="${ext}/prefix-qualified-v2"
source_dir="${ext}/m4ri-src-qualified-v2"
record="${prefix}/gf2-m4ri-build-record.txt"
bin="${ext}/m4ri_matvec_probe-qualified-v2"
version=20260122
archive="m4ri-${version}.tar.gz"
archive_sha256=7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404
build_flags='-O3 -march=native -fPIC'
configure_args='--disable-static'
compiler_command="$(command -v gcc)"
compiler_identity="$("${compiler_command}" --version | sed -n '1p')"
cppflags='<empty>'
ldflags='<empty>'
libs='<empty>'
config_site=/dev/null

# This is deliberately M4RI-only: the prior external-comparator bundle also
# builds unrelated ISA-L, Bitshuffle, and Rust arms.  A semantic qualification
# should not make those unrelated build products a prerequisite.
mkdir -p "${ext}"
if [[ ! -f "${ext}/${archive}" ]]; then
    curl -fsSL -o "${ext}/${archive}" \
        "https://github.com/malb/m4ri/releases/download/${version}/${archive}"
fi
(cd "${ext}" && echo "${archive_sha256}  ${archive}" | sha256sum -c -)

if [[ -f "${prefix}/lib/libm4ri.so" ]]; then
    [[ -f "${record}" ]] || {
        printf 'unverified M4RI cache: missing %s\n' "${record}" >&2
        exit 1
    }
    grep -Fqx "archive_sha256=${archive_sha256}" "${record}"
    grep -Fqx "compiler_command=${compiler_command}" "${record}"
    grep -Fqx "compiler_identity=${compiler_identity}" "${record}"
    grep -Fqx "build_flags=${build_flags}" "${record}"
    grep -Fqx "configure_args=${configure_args}" "${record}"
    grep -Fqx "cppflags=${cppflags}" "${record}"
    grep -Fqx "ldflags=${ldflags}" "${record}"
    grep -Fqx "libs=${libs}" "${record}"
    grep -Fqx "config_site=${config_site}" "${record}"
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
    tar -C "${source_dir}" --strip-components=1 -xf "${ext}/${archive}"
    (
        cd "${source_dir}"
        # The nested shell keeps configure, build, and install under one
        # cargo-budget invocation while expanding its own build-job setting.
        # shellcheck disable=SC2016
        CARGO_CI_NO_SCCACHE=1 "${repo}/scripts/cargo-budget.sh" \
            bash -c 'export CC="$1" CFLAGS="$2" CPPFLAGS="" LDFLAGS="" LIBS="" CONFIG_SITE=/dev/null; unset AR AS CPP LD NM OBJDUMP RANLIB STRIP; ./configure --prefix="$3" --disable-static && make -j"${CARGO_BUILD_JOBS:-1}" && make install' \
            _ "${compiler_command}" "${build_flags}" "${prefix}"
    )
    grep -Fqx "CC = ${compiler_command}" "${source_dir}/Makefile"
    library_sha256="$(sha256sum "${prefix}/lib/libm4ri.so" | awk '{print $1}')"
    {
        printf 'archive_sha256=%s\n' "${archive_sha256}"
        printf 'compiler_command=%s\n' "${compiler_command}"
        printf 'compiler_identity=%s\n' "${compiler_identity}"
        printf 'build_flags=%s\n' "${build_flags}"
        printf 'configure_args=%s\n' "${configure_args}"
        printf 'cppflags=%s\n' "${cppflags}"
        printf 'ldflags=%s\n' "${ldflags}"
        printf 'libs=%s\n' "${libs}"
        printf 'config_site=%s\n' "${config_site}"
        printf 'library_sha256=%s\n' "${library_sha256}"
    } >"${record}"
fi

"${compiler_command}" -std=c11 -O3 -march=native -Wall -Wextra -Werror \
    -I"${prefix}/include" "${here}/m4ri_matvec_probe.c" \
    -L"${prefix}/lib" -Wl,-rpath,"${prefix}/lib" -lm4ri -lm \
    -o "${bin}"
"${bin}"
