#!/usr/bin/env bash
# Pinned build identity for the jit:92385645 M4RI qualification.
#
# The probe runner builds to these pins and the provenance regression verifies
# an installed library against them, so both read one definition.
# shellcheck shell=bash
# shellcheck disable=SC2034  # every pin below is read by the sourcing script

command -v readelf >/dev/null || {
    printf 'readelf is required to read M4RI build provenance\n' >&2
    exit 1
}

m4ri_generation=qualified-v3
m4ri_version=20260122
m4ri_archive="m4ri-${m4ri_version}.tar.gz"
m4ri_archive_sha256=7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404
m4ri_build_flags='-O3 -march=native -fPIC'
m4ri_configure_args='--disable-static'
m4ri_cppflags='<empty>'
m4ri_ldflags='<empty>'
m4ri_libs='<empty>'
m4ri_config_site=/dev/null

# GNU Make promotes a variable definition carried in MAKEFLAGS or GNUMAKEFLAGS
# to a command-line override, and a command-line override outranks the
# assignment in the makefile itself (GNU Make manual, "Communicating Options to
# a Sub-make" and "Overriding Variables"). MFLAGS and MAKEFILES are the
# remaining make-level inheritance channels. The build clears all four before
# configure, build, and install, so the compiler make runs is the one the
# provenance record names.
m4ri_make_channels='MAKEFLAGS GNUMAKEFLAGS MFLAGS MAKEFILES'
m4ri_make_channel_state=cleared

m4ri_compiler_command="$(command -v gcc)"
m4ri_compiler_identity="$("${m4ri_compiler_command}" --version | sed -n '1p')"

# The ELF `.comment` producer strings of one file, sorted and joined by `|`.
# Every driver that compiles or links part of a file leaves its own string
# there, so this set names each compiler that touched the bytes. A generated
# Makefile's `CC =` line names the compiler configure selected, which a
# make-time command-line override replaces without changing that line.
m4ri_producers() {
    readelf -p .comment "$1" | sed -n 's/^  \[[^]]*\]  //p' | sort -u | paste -sd'|' -
}

# The producer string of the pinned compiler, read from an object it emits
# during this run rather than typed.
m4ri_compiler_producer() {
    local object producer
    object="$(mktemp)"
    "${m4ri_compiler_command}" -c -x c /dev/null -o "${object}"
    producer="$(m4ri_producers "${object}")"
    rm -f "${object}"
    printf '%s\n' "${producer}"
}

# Automake's `V=1` verbose mode echoes every real compiler invocation through
# libtool; the first field of such a line is the driver make actually ran.
m4ri_compile_lines() {
    sed -n 's/^libtool: compile: *//p' "$1"
}

# Fails unless the build log records at least one compile invocation, every one
# of them runs the pinned compiler, and every one of them carries the pinned
# build flags. Prints the invocation count.
m4ri_verify_build_log() {
    local log="$1" drivers count flagged
    [[ -f "${log}" ]] || {
        printf 'missing M4RI build log %s\n' "${log}" >&2
        return 1
    }
    drivers="$(m4ri_compile_lines "${log}" | awk '{print $1}' | sort -u | paste -sd'|' -)"
    [[ "${drivers}" == "${m4ri_compiler_command}" ]] || {
        printf 'M4RI build log records compile drivers %s, expected %s\n' \
            "${drivers}" "${m4ri_compiler_command}" >&2
        return 1
    }
    count="$(m4ri_compile_lines "${log}" | grep -c '' || true)"
    (( count > 0 )) || {
        printf 'M4RI build log %s records no compile invocation\n' "${log}" >&2
        return 1
    }
    flagged="$(m4ri_compile_lines "${log}" | grep -cF -- "${m4ri_build_flags}" || true)"
    [[ "${flagged}" == "${count}" ]] || {
        printf '%s of %s M4RI compile invocations carry %s\n' \
            "${flagged}" "${count}" "${m4ri_build_flags}" >&2
        return 1
    }
    printf '%s\n' "${count}"
}
