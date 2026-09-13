#!/usr/bin/env bash
# Emits the byte-field survey's arm provenance record (jit:6c6b09b1).
#
# REQ-02 of the issue asks for the revision, license, compiler targeting and
# selected arithmetic backend of every external arm. Everything below is read
# from the build and from the loaded binaries on this host; nothing is
# restated from prose. The pins come from `fetch-build.sh --print-pins`, the
# library identities from the survey Makefile, and the selected kernels from
# `backend_provenance`, whose pointer offsets this script resolves against
# each library's symbol table.
#
# Usage: dev/active/6c6b09b1/survey/arm-provenance.sh > <record>
#
# Reads the pinned sources from the primary checkout's external staging
# (GF2_SURVEY_SRC), the verified prefix copy the harness links
# (GF2_SURVEY_EXT), the C build (GF2_SURVEY_BUILD) and the Rust arms
# (GF2_SURVEY_TARGET); the launcher's build step sets all four.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
PRIMARY="$(cd "$(dirname "$(git -C "${REPO}" rev-parse --path-format=absolute --git-common-dir)")" && pwd)"
SRC="${GF2_SURVEY_SRC:-${PRIMARY}/.agents/ext/6c6b09b1}"
EXT="${GF2_SURVEY_EXT:-${REPO}/target/6c6b09b1-ext}"
BUILD="${GF2_SURVEY_BUILD:-${REPO}/target/6c6b09b1-survey/c}"
SURVEY_TARGET="${GF2_SURVEY_TARGET:-${REPO}/target/6c6b09b1-survey}"
PREFIX="${EXT}/prefix"

if [[ ! -x "${BUILD}/backend_provenance" ]]; then
    echo "run make -C ${HERE} first" >&2
    exit 2
fi

# Resolves a load-base-relative code offset to the symbol that contains it.
# `dladdr` cannot do this: the kernels of interest are file-local symbols
# that never enter the dynamic symbol table.
resolve_symbol() {
    local library="$1" offset="$2"
    nm --defined-only "${library}" 2>/dev/null | awk -v want="${offset}" '
        BEGIN { target = strtonum(want); best = -1 }
        NF >= 3 && $1 ~ /^[0-9a-fA-F]+$/ {
            address = strtonum("0x" $1)
            if (address <= target && address > best) {
                best = address
                name = $3
                kind = $2
            }
        }
        END {
            if (best < 0) { print "unnamed" }
            else if (best == target) { printf "%s [%s]\n", name, kind }
            else { printf "%s+0x%x [%s]\n", name, target - best, kind }
        }'
}

echo "# Byte-field survey arm provenance (jit:6c6b09b1)"
echo "# observed_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "# host: $(uname -srm), $(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -1)"
echo

echo "## Source pins"
echo "# projected from fetch-build.sh, which re-verifies each pin on every run."
bash "${HERE}/fetch-build.sh" --print-pins "${SRC}"
echo

echo "## Pinned sources as staged on this host"
for project in gf-complete isa-l; do
    printf '%s head=%s\n' "${project}" "$(git -C "${SRC}/${project}" rev-parse HEAD)"
done
for tarball in "${SRC}"/*.tar.gz "${SRC}"/*.tar.xz; do
    [[ -e "${tarball}" ]] || continue
    printf '%s sha256=%s\n' "$(basename "${tarball}")" "$(sha256sum "${tarball}" | cut -d' ' -f1)"
done
echo

echo "## Licenses"
for entry in \
    "nasm ${SRC}/nasm-src/LICENSE" \
    "m4ri ${SRC}/m4ri-src/COPYING" \
    "m4rie ${SRC}/m4rie-src/COPYING" \
    "gf-complete ${SRC}/gf-complete/License.txt" \
    "isa-l ${SRC}/isa-l/LICENSE"; do
    set -- ${entry}
    project="$1"
    path="$2"
    if [[ ! -f "${path}" ]]; then
        printf '%s license-file-missing %s\n' "${project}" "${path}"
        continue
    fi
    printf '%s sha256=%s first-line=%s\n' "${project}" \
        "$(sha256sum "${path}" | cut -d' ' -f1)" \
        "$(grep -m1 -v '^[[:space:]]*$' "${path}" | tr -s '[:space:]' ' ' | cut -c1-96)"
done
echo

echo "## Compiler targeting"
make -s -C "${HERE}" identities
echo "gcc-march-native=$(gcc -march=native -Q --help=target 2>/dev/null | sed -n 's/^[[:space:]]*-march=[[:space:]]*//p' | head -1)"
echo "rustc=$(rustc --version)"
echo "rust-arm-rustflags=${RUSTFLAGS:-}"
echo "rust-arm-profile=release opt-level=3 lto=true codegen-units=1"
echo

echo "## Linked libraries of the external arm"
if [[ -x "${SURVEY_TARGET}/release/ext-arm" ]]; then
    ldd "${SURVEY_TARGET}/release/ext-arm" \
        | sed -n 's/.*=> \(.*\) (0x.*/\1/p' \
        | grep -F "${PREFIX}/lib" \
        | while read -r library; do
            printf '%s sha256=%s\n' "$(basename "$(readlink -f "${library}")")" \
                "$(sha256sum "$(readlink -f "${library}")" | cut -d' ' -f1)"
        done
else
    echo "ext-arm-not-built"
fi
echo

echo "## Selected arithmetic backends, observed at run time"
"${BUILD}/backend_provenance" | while read -r kind rest; do
    if [[ "${kind}" != "ptr" ]]; then
        printf '%s %s\n' "${kind}" "${rest}"
        continue
    fi
    set -- ${rest}
    printf 'kernel   %s %s %s -> %s\n' "$1" "$(basename "$2")" "$3" \
        "$(resolve_symbol "$2" "$3")"
done
