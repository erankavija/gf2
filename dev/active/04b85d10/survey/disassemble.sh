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

# Where population counts live in the measured executable, per symbol, from
# the whole binary. Every count tests one field of an instruction line
# exactly: its mnemonic, after any prefix, or one of its operands. A pattern
# over the whole line would also count symbol headers, branch-target labels
# and operands that merely name a `popcnt` routine. The conservative-portable
# build enables no `popcnt` target feature, so a scalar `u64::count_ones`
# compiles to the bit-twiddle sequence whose masks and byte-sum multiplier
# are 64-bit immediates; POPCNT can appear only inside code compiled under an
# enabled target feature.
{
    echo "# Population-count instructions and constants per symbol (jit:04b85d10)"
    echo "# binary: ${BINARY}"
    echo "# binary sha256: $(sha256sum "${BINARY}" | cut -d' ' -f1)"
    echo "# Each count is of instruction lines whose mnemonic field (after any"
    echo "# prefix) or one operand equals the named value; no other text is tested."
    objdump -d --no-show-raw-insn -C "${BINARY}" | awk -F'\t' '
        BEGIN {
            prefix = "^(lock|rep|repz|repnz|repe|repne|data16|data32|addr16|addr32|cs|ds|es|fs|gs|ss|notrack|bnd|xacquire|xrelease)$"
            split("5555555555555555 3333333333333333 f0f0f0f0f0f0f0f 101010101010101", constant, " ")
        }
        /^[0-9a-f]+ </ { fn = substr($0, index($0, "<")); next }
        NF == 2 && $1 ~ /^ +[0-9a-f]+:$/ {
            words = split($2, word, " ")
            i = 1
            while (i < words && word[i] ~ prefix) i++
            mnemonic = word[i]
            if (mnemonic ~ /^popcnt[wlq]?$/) popcnt[fn]++
            if (mnemonic == "vpshufb") shuffle[fn]++
            if (mnemonic == "vpsadbw") sad[fn]++
            operands = split(word[i + 1], operand, ",")
            for (o = 1; o <= operands; o++) {
                if (operand[o] !~ /^\$0x[0-9a-f]+$/) continue
                value = substr(operand[o], 4)
                sub(/^0+/, "", value)
                for (k = 1; k <= 4; k++) if (value == constant[k]) { hits[fn, k]++; any[fn] = 1 }
            }
        }
        END {
            print ""
            print "## POPCNT instructions (mnemonic popcnt, popcntw, popcntl or popcntq): count, symbol"
            fflush()
            found = 0
            for (f in popcnt) { print popcnt[f], f | "LC_ALL=C sort -k1,1nr -k2"; found = 1 }
            close("LC_ALL=C sort -k1,1nr -k2")
            if (!found) print "none: no instruction in the binary has a popcnt mnemonic"
            print ""
            print "## nibble-lookup byte counts: VPSHUFB and VPSADBW instructions: vpshufb, vpsadbw, symbol"
            for (f in shuffle) seen[f] = 1
            for (f in sad) seen[f] = 1
            fflush()
            for (f in seen) print shuffle[f] + 0, sad[f] + 0, f | "LC_ALL=C sort -k1,1nr -k2,2nr -k3"
            close("LC_ALL=C sort -k1,1nr -k2,2nr -k3")
            print ""
            print "## bit-twiddle population-count immediates: 0x5555555555555555, 0x3333333333333333, 0x0f0f0f0f0f0f0f0f (masks), 0x0101010101010101 (byte-sum multiplier), symbol"
            fflush()
            for (f in any) print hits[f, 1] + 0, hits[f, 2] + 0, hits[f, 3] + 0, hits[f, 4] + 0, f | "LC_ALL=C sort -k4,4nr -k1,1nr -k5"
            close("LC_ALL=C sort -k4,4nr -k1,1nr -k5")
        }'
} >"${OUT}/asm/popcnt-attribution.txt"

# Frame traffic per routine, joined with the source-derived reason the
# routine has it. The counter cannot separate a compiler spill from a buffer
# the algorithm is defined over; the classification table cites the source
# declaration that does.
python3 "${HERE}/frame-traffic.py" "${OUT}/asm" \
    "${HERE}/asm-frame-classification.txt" >"${OUT}/asm/frame-traffic.txt"

echo "disassembly written to ${OUT}/asm" >&2
