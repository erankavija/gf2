#!/usr/bin/env bash
# Build and run the jit:92385645 semantic probe against M4RI's pinned build.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${here}" rev-parse --show-toplevel)"
ext="${repo}/.agents/ext/92385645"
prefix="${ext}/prefix"
bin="${ext}/m4ri_matvec_probe"
version=20260122
archive="m4ri-${version}.tar.gz"
archive_sha256=7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404

# This is deliberately M4RI-only: the prior external-comparator bundle also
# builds unrelated ISA-L, Bitshuffle, and Rust arms.  A semantic qualification
# should not make those unrelated build products a prerequisite.
mkdir -p "${ext}" "${prefix}"
if [[ ! -f "${prefix}/lib/libm4ri.so" ]]; then
    if [[ ! -f "${ext}/${archive}" ]]; then
        curl -fsSL -o "${ext}/${archive}" \
            "https://github.com/malb/m4ri/releases/download/${version}/${archive}"
    fi
    (cd "${ext}" && echo "${archive_sha256}  ${archive}" | sha256sum -c -)
    if [[ ! -d "${ext}/m4ri-src" ]]; then
        mkdir -p "${ext}/m4ri-src"
        tar -C "${ext}/m4ri-src" --strip-components=1 -xf "${ext}/${archive}"
    fi
    (
        cd "${ext}/m4ri-src"
        # The nested shell keeps configure, build, and install under one
        # cargo-budget invocation while expanding its own build-job setting.
        # shellcheck disable=SC2016
        CARGO_CI_NO_SCCACHE=1 "${repo}/scripts/cargo-budget.sh" \
            bash -c 'CFLAGS="-O3 -march=native -fPIC" ./configure --prefix="$1" --disable-static && make -j"${CARGO_BUILD_JOBS:-1}" && make install' _ "${prefix}"
    )
fi

gcc -std=c11 -O3 -march=native -Wall -Wextra -Werror \
    -I"${prefix}/include" "${here}/m4ri_matvec_probe.c" \
    -L"${prefix}/lib" -Wl,-rpath,"${prefix}/lib" -lm4ri -lm \
    -o "${bin}"
"${bin}"
