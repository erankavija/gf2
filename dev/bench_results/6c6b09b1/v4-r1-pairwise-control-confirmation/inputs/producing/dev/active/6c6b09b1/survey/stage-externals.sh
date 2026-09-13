#!/usr/bin/env bash
# Stage the pinned byte-field externals into this checkout's build area
# (jit:6c6b09b1).
#
# Usage: dev/active/6c6b09b1/survey/stage-externals.sh [source-staging] [destination]
#
# `fetch-build.sh` builds M4RI, M4RIE, GF-Complete and ISA-L from its pins
# into the primary checkout's `.agents/ext/6c6b09b1/prefix`. This script
# reads that staging and writes nothing there: it re-verifies every source
# pin through `fetch-build.sh --verify-sources`, checks every installed
# library and header against the digests committed in `ext-prefix.sha256`,
# and copies the prefix into `<destination>/prefix` (default
# `<repo>/target/6c6b09b1-ext`), from which the survey harness compiles and
# links. A digest mismatch stops here, before anything is built or timed.
#
# `--record` rewrites `ext-prefix.sha256` from the source staging instead of
# checking it; it is how the committed list was produced.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
PRIMARY="$(cd "$(dirname "$(git -C "${REPO}" rev-parse --path-format=absolute --git-common-dir)")" && pwd)"
RECORD=0
if [[ "${1:-}" == "--record" ]]; then
    RECORD=1
    shift
fi
SRC="${1:-${PRIMARY}/.agents/ext/6c6b09b1}"
DEST="${2:-${REPO}/target/6c6b09b1-ext}"
DIGESTS="${HERE}/ext-prefix.sha256"

"${HERE}/fetch-build.sh" --verify-sources "${SRC}"

# Every regular file the harness compiles against or loads: the installed
# libraries, their libtool and pkg-config records, and the headers.
list_prefix() {
    (cd "${SRC}/prefix" && find lib include -type f | LC_ALL=C sort)
}

if [[ "${RECORD}" == "1" ]]; then
    (cd "${SRC}/prefix" && list_prefix | xargs sha256sum) >"${DIGESTS}"
    echo "recorded $(wc -l <"${DIGESTS}") prefix digests in ${DIGESTS}"
    exit 0
fi

if [[ "$(list_prefix)" != "$(cut -c67- "${DIGESTS}")" ]]; then
    echo "the staged prefix file set differs from ${DIGESTS}" >&2
    exit 1
fi
(cd "${SRC}/prefix" && sha256sum --quiet -c "${DIGESTS}")
echo "verified $(wc -l <"${DIGESTS}") prefix files against ${DIGESTS}"

rm -rf "${DEST}/prefix"
mkdir -p "${DEST}/prefix"
cp -a "${SRC}/prefix/lib" "${SRC}/prefix/include" "${DEST}/prefix/"
(cd "${DEST}/prefix" && sha256sum --quiet -c "${DIGESTS}")
echo "staged the verified prefix at ${DEST}/prefix"
