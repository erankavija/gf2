#!/usr/bin/env bash
# Regression check that neither hostile ambient Autoconf inputs nor hostile GNU
# Make override channels can mislabel M4RI's pinned GCC provenance.
#
# Every case builds its own disposable cache under the repository's `target/`,
# so a retained comparator build is never rebuilt, overwritten, or removed.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(git -C "${here}" rev-parse --show-toplevel)"
# shellcheck source=m4ri-build-pins.sh
. "${here}/m4ri-build-pins.sh"

runner="${here}/run-m4ri-matvec-probe.sh"
compiler_producer="$(m4ri_compiler_producer)"
# Seeds each disposable cache with an already downloaded archive; the runner
# verifies its digest and downloads it when absent.
archive_seed="${M4RI_ARCHIVE_CACHE:-${repo}/.agents/ext/92385645}/${m4ri_archive}"

mkdir -p "${repo}/target"
scratch="$(mktemp -d "${repo}/target/m4ri-provenance-XXXXXX")"
keep=1
cleanup() {
    if [[ -n "${keep}" ]]; then
        printf 'M4RI provenance regression evidence kept under %s\n' "${scratch}" >&2
    else
        rm -rf "${scratch}"
    fi
}
trap cleanup EXIT

# A control that this GNU Make does promote MAKEFLAGS and GNUMAKEFLAGS
# definitions over a makefile's own assignment, so a passing regression below
# reports a cleared channel rather than an inert one.
control="${scratch}/make-channel-control"
mkdir -p "${control}"
# shellcheck disable=SC2016  # $(CC) is make syntax, not a shell expansion
printf 'CC = pinned\nall:\n\t@printf %%s "$(CC)"\n' >"${control}/Makefile"
[[ "$(MAKEFLAGS='CC=hostile' make -s -C "${control}")" == hostile ]]
[[ "$(GNUMAKEFLAGS='CC=hostile' make -s -C "${control}")" == hostile ]]
[[ "$(MAKEFLAGS='CC=hostile' GNUMAKEFLAGS='CC=hostile' \
    env -u MAKEFLAGS -u GNUMAKEFLAGS make -s -C "${control}")" == pinned ]]
printf 'control: MAKEFLAGS and GNUMAKEFLAGS override a makefile assignment; clearing them restores it\n'

# Fails unless the installed library, its build log, and its provenance record
# all name the pinned compiler and the pinned build flags.
assert_pinned() {
    local label="$1" ext="$2"
    local prefix="${ext}/prefix-${m4ri_generation}"
    local record="${prefix}/gf2-m4ri-build-record.txt"
    local log="${prefix}/gf2-m4ri-build-log.txt"
    local invocations library_producers

    grep -Fqx "CC = ${m4ri_compiler_command}" "${ext}/m4ri-src-${m4ri_generation}/Makefile"
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

    invocations="$(m4ri_verify_build_log "${log}")"
    grep -Fqx "compile_invocations=${invocations}" "${record}"
    # The hostile make-level cases also inject `CFLAGS=-O0`, which the pinned
    # flags would not otherwise exclude.
    if m4ri_compile_lines "${log}" | grep -q -- ' -O0 '; then
        printf '%s: a compile invocation carries -O0\n' "${label}" >&2
        return 1
    fi

    library_producers="$(m4ri_producers "${prefix}/lib/libm4ri.so")"
    [[ "${library_producers}" == "${compiler_producer}" ]]
    grep -Fqx "library_producers=${library_producers}" "${record}"

    printf '%s: %s compile invocations by %s, library producers %s\n' \
        "${label}" "${invocations}" "${m4ri_compiler_command}" "${library_producers}"
}

fresh_cache() {
    local label="$1"
    local ext="${scratch}/${label}"
    mkdir -p "${ext}"
    if [[ -f "${archive_seed}" ]]; then
        cp "${archive_seed}" "${ext}/${m4ri_archive}"
    fi
    printf '%s\n' "${ext}"
}

# Fresh cache under hostile ambient Autoconf inputs.
ext="$(fresh_cache ambient-autoconf)"
env CC=clang \
    CPPFLAGS=-DGF2_PROVENANCE_BYPASS \
    LDFLAGS=-Wl,--as-needed \
    LIBS=-ldl \
    CONFIG_SITE=/nonexistent \
    M4RI_EXT="${ext}" \
    bash "${runner}"
assert_pinned ambient-autoconf "${ext}"

# Fresh cache under a hostile GNU Make command-line override inherited through
# MAKEFLAGS.
makeflags_ext="$(fresh_cache makeflags)"
env MAKEFLAGS='CC=clang CFLAGS=-O0' \
    M4RI_EXT="${makeflags_ext}" \
    bash "${runner}"
assert_pinned makeflags "${makeflags_ext}"

# Fresh cache under the same override inherited through GNUMAKEFLAGS.
gnumakeflags_ext="$(fresh_cache gnumakeflags)"
env GNUMAKEFLAGS='CC=clang CFLAGS=-O0' \
    M4RI_EXT="${gnumakeflags_ext}" \
    bash "${runner}"
assert_pinned gnumakeflags "${gnumakeflags_ext}"

# Reuse of a retained cache with every hostile channel present at once.
env CC=clang \
    CPPFLAGS=-DGF2_PROVENANCE_BYPASS \
    LDFLAGS=-Wl,--as-needed \
    LIBS=-ldl \
    CONFIG_SITE=/nonexistent \
    MAKEFLAGS='CC=clang CFLAGS=-O0' \
    GNUMAKEFLAGS='CC=clang CFLAGS=-O0' \
    M4RI_EXT="${ext}" \
    bash "${runner}"
assert_pinned reuse "${ext}"

keep=""
printf 'M4RI compiler-provenance regression: PASS\n'
