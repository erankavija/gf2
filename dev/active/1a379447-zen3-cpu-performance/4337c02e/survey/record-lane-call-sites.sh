#!/usr/bin/env bash
# Record which lane symbols the measured entries call (jit:4337c02e).
#
# Usage: record-lane-call-sites.sh <dense-arm executable>, from the worktree root.
#
# The record lists every `BitMatrix` matvec symbol of the executable and every
# call from one to another, so it shows whether the public `matvec` and the
# pinned `matvec_with_route` reach one scalar-lane body. Addresses are omitted:
# the executable digest identifies the bytes.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worktree root' >&2
    exit 2
}
BINARY="${1:?usage: record-lane-call-sites.sh <dense-arm executable>}"
RECORD="${HERE}/lane-call-sites.txt"
PATTERN='gf2_core::matrix::BitMatrix::matvec'

{
    echo '# Lane symbols and call sites of the dense arm (jit:4337c02e)'
    echo "# executable sha256: $(sha256sum "${BINARY}" | cut -d' ' -f1)"
    echo "# objdump: $(objdump --version | head -n 1)"
    echo '## symbols (name, size in bytes)'
    nm -S -C --defined-only "${BINARY}" |
        awk -v pattern="${PATTERN}" 'index($4, pattern) == 1 { printf "%s %d\n", $4, strtonum("0x" $2) }' |
        LC_ALL=C sort
    echo '## calls (caller => callee, count)'
    objdump -d --no-show-raw-insn -C "${BINARY}" |
        awk -v pattern="${PATTERN}" '
            /^[0-9a-f]+ <.*>:$/ { caller = substr($2, 2, length($2) - 3) }
            /\t(call|jmp) / && index($NF, "<" pattern) == 1 && index(caller, pattern) == 1 {
                callee = substr($NF, 2, length($NF) - 2)
                sub(/\+0x[0-9a-f]+$/, "", callee)
                if (callee != caller) print caller " => " callee
            }' | LC_ALL=C sort | uniq -c | awk '{ count = $1; $1 = ""; print substr($0, 2) ", " count }'
} >"${RECORD}.tmp"
mv "${RECORD}.tmp" "${RECORD}"
cat "${RECORD}"
