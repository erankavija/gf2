#!/usr/bin/env bash
# Regression check that hostile ambient Autoconf inputs cannot mislabel M4RI.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ext="${M4RI_EXT:?Set M4RI_EXT to a disposable or qualified M4RI cache root.}"
compiler_command="$(command -v gcc)"
prefix="${ext}/prefix-qualified-v2"
source_dir="${ext}/m4ri-src-qualified-v2"
record="${prefix}/gf2-m4ri-build-record.txt"

CC=clang \
CPPFLAGS=-DGF2_PROVENANCE_BYPASS \
LDFLAGS=-Wl,--as-needed \
LIBS=-ldl \
CONFIG_SITE=/nonexistent \
M4RI_EXT="${ext}" \
    bash "${here}/run-m4ri-matvec-probe.sh"

grep -Fqx "CC = ${compiler_command}" "${source_dir}/Makefile"
grep -Fqx "compiler_command=${compiler_command}" "${record}"
grep -Fqx "compiler_identity=$("${compiler_command}" --version | sed -n '1p')" "${record}"
grep -Fqx 'cppflags=<empty>' "${record}"
grep -Fqx 'ldflags=<empty>' "${record}"
grep -Fqx 'libs=<empty>' "${record}"
grep -Fqx 'config_site=/dev/null' "${record}"

printf 'M4RI compiler-provenance regression: PASS\n'
