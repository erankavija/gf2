#!/usr/bin/env bash
# Generated-code evidence for the byte-field consumer assessment (jit:19513245).
#
# Disassembles the routes the profile and the campaigns measure, so a claim
# about instruction mix, an indirect call, a reference-count update or a
# dependency chain rests on the code the measured executable actually contains.
# Disassembly reads a file and times nothing, so it runs outside the benchmark
# mutex.
#
# Usage: dev/active/19513245/survey/disassemble.sh <bin-dir> [<asm-dir>]
#
# `<bin-dir>` is the release directory the profile was measured from, so the
# disassembled bytes are the measured executable's bytes; its digest is
# recorded in every output file. `<asm-dir>` defaults to this survey's
# committed `asm/` directory.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${1:?usage: disassemble.sh <bin-dir> [<asm-dir>]}"
ASM="${2:-${HERE}/asm}"
mkdir -p "${ASM}"

# Both measured executables: the arm the campaigns run, and the profile binary,
# which additionally holds the matrix-vector routes no campaign cell measures.
for BINARY_NAME in consumer-arm consumer-profile; do
mkdir -p "${ASM}/${BINARY_NAME}"
BINARY="${BIN}/${BINARY_NAME}"
DIGEST="$(sha256sum "${BINARY}" | cut -d' ' -f1)"
SYMBOLS="${ASM}/${BINARY_NAME}/symbols.txt"
# `-S` adds each symbol's size, so a routine is disassembled to its own last
# instruction rather than to a fixed window that would run into its neighbour.
nm -S -C --defined-only "${BINARY}" | sort -k4 >"${SYMBOLS}"

: >"${ASM}/${BINARY_NAME}/index.txt"
{
    echo "# Annotated release disassembly of the measured consumer routes (jit:19513245)"
    echo "# binary: ${BINARY}"
    echo "# binary sha256: ${DIGEST}"
    echo "# Fields: file  symbols  bytes  pattern"
} >>"${ASM}/${BINARY_NAME}/index.txt"

while IFS='|' read -r label pattern; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    target="${ASM}/${BINARY_NAME}/${label}.asm.txt"
    matches=$(grep -F "${pattern}" "${SYMBOLS}" | awk '$3 == "t" || $3 == "T" {print $1 ":" $2}' || true)
    copies=$(printf "%s\n" "${matches}" | grep -c . || true)
    bytes=0
    {
        echo "# label: ${label}"
        echo "# pattern: ${pattern}"
        echo "# binary: ${BINARY}"
        echo "# binary sha256: ${DIGEST}"
        echo "# text symbols matching this pattern: ${copies}"
        if [[ "${copies}" == 0 ]]; then
            echo "# no symbol of this pattern survives in the measured binary: the routine was"
            echo "# inlined into its caller, and this file records that rather than a silent gap."
        fi
    } >"${target}"
    for entry in ${matches}; do
        address=${entry%%:*}
        size=${entry##*:}
        bytes=$((bytes + 0x${size}))
        {
            echo
            echo "## symbol at ${address}, 0x${size} bytes"
            objdump -d --no-show-raw-insn -C \
                --start-address="0x${address}" \
                --stop-address="$((0x${address} + 0x${size}))" \
                "${BINARY}" | sed -n '/>:/,$p'
        } >>"${target}"
    done
    printf '%s.asm.txt  %s  %s  %s\n' "${label}" "${copies}" "${bytes}" "${pattern}" \
        >>"${ASM}/${BINARY_NAME}/index.txt"
done <"${HERE}/asm-symbols.txt"

# Instruction mix per symbol, over the whole binary. Each count tests one
# field of an instruction line exactly: its mnemonic after any prefix, or the
# presence of an indirect call operand. A pattern over the whole line would
# also count symbol headers, branch-target labels and operands that merely
# name a routine.
#
# The four mnemonic classes are the ones that separate the two routes. A
# carry-less multiply is the current GF(2^8) multiply's arithmetic; a
# lock-prefixed instruction is a reference-count update on the element
# representation's field handle; an indirect call is the function-pointer
# dispatch the element multiply reaches its kernel through; a byte shuffle is
# what a vectorised successor would use and what neither route contains.
{
    echo "# Instruction mix of the measured routes (jit:19513245)"
    echo "# binary: ${BINARY}"
    echo "# binary sha256: ${DIGEST}"
    echo "# Each count is of instruction lines in the named symbol whose mnemonic field"
    echo "# (after any prefix) matches the class, or whose call operand is indirect."
    objdump -d --no-show-raw-insn -C "${BINARY}" | awk -F'\t' '
        BEGIN {
            prefix = "^(lock|rep|repz|repnz|repe|repne|data16|data32|addr16|addr32|cs|ds|es|fs|gs|ss|notrack|bnd|xacquire|xrelease)$"
        }
        /^[0-9a-f]+ </ { fn = substr($0, index($0, "<")); next }
        NF == 2 && $1 ~ /^ +[0-9a-f]+:$/ {
            words = split($2, word, " ")
            i = 1
            locked = 0
            while (i < words && word[i] ~ prefix) { if (word[i] == "lock") locked = 1; i++ }
            mnemonic = word[i]
            seen[fn] = 1
            if (locked) atomics[fn]++
            if (mnemonic ~ /^v?pclmul/) clmul[fn]++
            if (mnemonic ~ /^v?pshufb$/) shuffle[fn]++
            if (mnemonic ~ /^call/) {
                calls[fn]++
                if (word[i + 1] ~ /^\*/) indirect[fn]++
            }
            if (mnemonic ~ /^i?div/) divide[fn]++
        }
        END {
            print ""
            print "## lock-prefixed, carry-less-multiply, byte-shuffle, call, indirect-call and divide counts, symbol"
            fflush()
            for (f in seen) {
                if (atomics[f] + clmul[f] + shuffle[f] + calls[f] + divide[f] == 0) continue
                printf "%d %d %d %d %d %d %s\n", atomics[f] + 0, clmul[f] + 0, shuffle[f] + 0, \
                    calls[f] + 0, indirect[f] + 0, divide[f] + 0, f | "LC_ALL=C sort -k1,1nr -k2,2nr -k7"
            }
            close("LC_ALL=C sort -k1,1nr -k2,2nr -k7")
        }'
} >"${ASM}/${BINARY_NAME}/instruction-mix.txt"

done

echo "disassembly written to ${ASM}" >&2
