#!/usr/bin/env bash
# Compiler identity shared by the ISA-L probe and campaign arm.
command -v readelf >/dev/null || { echo 'readelf is required for ISA-L build provenance' >&2; exit 1; }
isal_compiler_command="$(command -v gcc)"
isal_compiler_command="$(realpath "${isal_compiler_command}")"
isal_compiler_identity="$("${isal_compiler_command}" --version | sed -n '1p')"
isal_make_channels='MAKEFLAGS GNUMAKEFLAGS MFLAGS MAKEFILES'
isal_object_producers() {
    readelf -p .comment "$1" | sed -n 's/^  \[[^]]*\]  //p' | sort -u | paste -sd'|' -
}
