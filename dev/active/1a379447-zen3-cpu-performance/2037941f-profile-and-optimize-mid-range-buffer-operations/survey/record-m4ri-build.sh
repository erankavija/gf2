#!/usr/bin/env bash
# Print the build record of the M4RI install at the level `M4RI_BUILD_LEVEL`
# names (see m4ri-build-pins.sh), building it first when it is absent.
#
# Every line is read from the install, its retained source directory and build
# log, or the pins; the semantic cases are the probe's own output. The install
# is shared under the primary checkout's .agents/ext unless `M4RI_EXT` names
# another cache.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=m4ri-build-pins.sh
. "${here}/m4ri-build-pins.sh"

invocation="M4RI_BUILD_LEVEL=${m4ri_build_level} bash $(realpath --relative-to="$(git -C "${here}" rev-parse --show-toplevel)" "${BASH_SOURCE[0]}")"
if [[ -z "${M4RI_EXT:-}" ]]; then
    common="$(git -C "${here}" rev-parse --path-format=absolute --git-common-dir)"
    M4RI_EXT="$(dirname "${common}")/.agents/ext/92385645"
fi
export M4RI_EXT M4RI_BUILD_LEVEL="${m4ri_build_level}"
probe="$(bash "${here}/run-m4ri-matvec-probe.sh")"

prefix="${M4RI_EXT}/prefix-${m4ri_generation}"
source_dir="${M4RI_EXT}/m4ri-src-${m4ri_generation}"
log="${prefix}/gf2-m4ri-build-log.txt"
record="${prefix}/gf2-m4ri-build-record.txt"
library="${prefix}/lib/libm4ri.so"
header="${prefix}/include/m4ri/m4ri_config.h"
digest() { sha256sum "$1" | cut -d' ' -f1; }
configured() { sed -n "s/^$1=//p" "${source_dir}/config.log" | tr -d "'"; }

echo "M4RI comparator build record, level ${m4ri_build_level}"
echo "command: ${invocation}"
echo "host: $(uname -srm)"
echo "generation: ${m4ri_generation}"
echo "compiler: ${m4ri_compiler_identity}"
echo "compiler producer string: $(m4ri_compiler_producer)"
echo "M4RI release: ${m4ri_version}"
echo "source archive sha256: $(digest "${M4RI_EXT}/${m4ri_archive}")"
echo "installed libm4ri.so sha256: $(digest "${library}")"
echo "install build record sha256: $(digest "${record}")"
echo "library build flags: CFLAGS=$(sed -n 's/^build_flags=//p' "${record}"); $(sed -n 's/^configure_args=//p' "${record}")"
echo "shim and probe targeting flags: ${m4ri_targeting_flags:-<none>}"
echo "compile invocations in the retained build log: $(m4ri_verify_build_log "${log}"), every one by ${m4ri_compiler_command}"
echo "compile invocations carrying a targeting option: $(m4ri_compile_lines "${log}" | grep -Ec -- "${m4ri_targeting_pattern}" || true)"
echo "first compile invocation: $(m4ri_compile_lines "${log}" | sed -n '1p')"
echo "configure CFLAGS: $(configured CFLAGS)"
echo "configure M4RI_SSE2_ENABLED: $(configured M4RI_SSE2_ENABLED)"
echo "configure M4RI_HAVE_OPENMP: $(configured M4RI_HAVE_OPENMP)"
echo "configure cache sizes read from the build host: L1=$(configured M4RI_CPU_L1_CACHE) L2=$(configured M4RI_CPU_L2_CACHE) L3=$(configured M4RI_CPU_L3_CACHE)"
echo "installed m4ri_config.h sha256: $(digest "${header}")"
echo "installed m4ri_config.h definitions:"
grep -E '^#(define|if|undef|endif)' "${header}" | sed 's/^/  /'
echo "library instructions naming a ymm or zmm register: $(objdump -d --no-show-raw-insn "${library}" | grep -Ec '%[yz]mm' || true)"
echo "library instructions naming an xmm register: $(objdump -d --no-show-raw-insn "${library}" | grep -Ec '%xmm' || true)"
echo "semantic probe executable sha256: $(digest "${M4RI_EXT}/m4ri_matvec_probe-${m4ri_generation}")"
echo "semantic probe cases:"
printf '%s\n' "${probe}" | sed -n 's/^ok /  ok /p'
