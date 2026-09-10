#!/usr/bin/env bash
# Generated-code evidence for the bit-storage consumer profile (jit:04b85d10).
#
# Disassembles the kernels and drivers the profile attributes cost to, so a
# claim about instruction mix, a dependency chain, or a register spill rests on
# the code the measured executable actually contains. Disassembly reads a file
# and times nothing, so it runs outside the benchmark mutex.
#
# Usage: dev/active/04b85d10/survey/disassemble.sh <output-dir> <bin-dir>
#
# `<bin-dir>` is the release directory the profile was measured from, so the
# disassembled bytes are the measured executable's bytes; its digest is
# recorded in every output file.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

OUT="${1:?usage: disassemble.sh <output-dir> <bin-dir>}"
BIN="${2:?usage: disassemble.sh <output-dir> <bin-dir>}"
mkdir -p "${OUT}/asm"

BINARY="${BIN}/consumer-profile"
SYMBOLS="${OUT}/asm/symbols.txt"
# `-S` adds each symbol's size, so a routine is disassembled to its own last
# instruction rather than to a fixed window that would run into its neighbour.
nm -S -C --defined-only "${BINARY}" | sort -k4 >"${SYMBOLS}"

# Each row is a label and the demangled substring identifying the routine.
# A row that matches no symbol writes an empty file naming the miss, which is
# the honest record that the routine was inlined away rather than a silent gap.
while IFS='|' read -r label pattern; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    target="${OUT}/asm/${label}.txt"
    matches=$(grep -F "${pattern}" "${SYMBOLS}" || true)
    copies=$(printf "%s\n" "${matches}" | grep -c . || true)
    # Keep only rows carrying an address, a size and a text-section type.
    # One instance per routine: link-time optimization emits several
    # identical copies of a kernel reached through a function pointer, and
    # summing them would multiply every count by the copy number.
    # The largest text symbol of the pattern, and only one: link-time
    # optimization emits several identical copies of a kernel reached through
    # a function pointer, and a thin wrapper often shares the routine's name.
    matches=$(printf "%s\n" "${matches}" \
        | awk '$3 == "t" || $3 == "T" {print $2 ":" $1}' | sort -r | head -1 \
        | awk -F: '{print $2 ":" $1}')
    {
        echo "# label: ${label}"
        echo "# pattern: ${pattern}"
        echo "# binary: ${BINARY}"
        echo "# binary sha256: $(sha256sum "${BINARY}" | cut -d' ' -f1)"
        echo "# symbols matching this pattern in the binary: ${copies}"
        if [[ -z "${matches}" ]]; then
            echo "# no symbol of this pattern survives in the measured binary"
        fi
    } >"${target}"
    for entry in ${matches}; do
        address=${entry%%:*}
        size=${entry##*:}
        {
            echo
            echo "## symbol at ${address}, 0x${size} bytes"
            objdump -d --no-show-raw-insn -C \
                --start-address="0x${address}" \
                --stop-address="$((0x${address} + 0x${size}))" \
                "${BINARY}" | sed -n '/>:/,$p'
        } >>"${target}"
    done
done <"${HERE}/asm-symbols.txt"

# Where the POPCNT instruction and the portable bit-twiddle population count
# actually live in the measured executable. The conservative-portable build
# enables no `popcnt` target feature, so a scalar `u64::count_ones` compiles
# to the mask-and-shift sequence identified by its 0x5555.../0x3333...
# constants; POPCNT can appear only inside kernels compiled under an enabled
# target feature. The attribution is per symbol, from the whole binary.
{
    echo "# POPCNT instructions per symbol (jit:04b85d10)"
    echo "# binary: ${BINARY}"
    echo "# binary sha256: $(sha256sum "${BINARY}" | cut -d' ' -f1)"
    echo
    echo "## symbols containing a popcnt instruction (count, symbol)"
    objdump -d --no-show-raw-insn -C "${BINARY}" \
        | awk '/^[0-9a-f]+ </ {fn=substr($0, index($0, "<"))} /popcnt/ {c[fn]++} END {for (f in c) print c[f], f}' \
        | sort -rn
    echo
    echo "## symbols containing the bit-twiddle popcount constants 0x5555... or 0x3333... (count, symbol)"
    objdump -d --no-show-raw-insn -C "${BINARY}" \
        | awk '/^[0-9a-f]+ </ {fn=substr($0, index($0, "<"))} /0x3333333333333333|0x5555555555555555/ {c[fn]++} END {for (f in c) print c[f], f}' \
        | sort -rn
} >"${OUT}/asm/popcnt-attribution.txt"

# Frame traffic per routine, joined with the source-derived reason the
# routine has it. The counter cannot separate a compiler spill from a buffer
# the algorithm is defined over; the classification table cites the source
# declaration that does.
python3 "${HERE}/frame-traffic.py" "${OUT}/asm" \
    "${HERE}/asm-frame-classification.txt" >"${OUT}/asm/frame-traffic.txt"

echo "disassembly written to ${OUT}/asm" >&2
