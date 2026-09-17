#!/usr/bin/env bash
# Annotated release disassembly of the measured logical-buffer routes
# (jit:18a87159).
#
# Disassembly reads a file and times nothing, so it runs in an ordinary
# session, outside the benchmark window. The bytes it reads are the bytes the
# window's own rebuild produces from this tree at this toolchain, and each
# output file records the executable digest it was produced from.
#
# Usage: dev/active/2037941f-.../survey/disassemble-logical.sh [<bin-dir>] [<asm-dir>]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
BIN="${1:-${REPO}/target/bb769456-arms/release}"
ASM="${2:-${HERE}/asm}"
SYMBOL_TABLE="${HERE}/logical-asm-symbols.txt"
mkdir -p "${ASM}"

# The toolchain recorded here is the one the measured executables are built
# with, so the launcher's pin is repeated rather than left to the ambient
# default.
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95

for BINARY_NAME in logical-arm logical-profile; do
    BINARY="${BIN}/${BINARY_NAME}"
    DIGEST="$(sha256sum "${BINARY}" | cut -d' ' -f1)"
    mkdir -p "${ASM}/${BINARY_NAME}"
    SYMBOLS="${ASM}/${BINARY_NAME}/symbols.txt"
    # `-S` adds each symbol's size, so a routine is disassembled to its own
    # last instruction rather than to a fixed window that runs into its
    # neighbour.
    nm -S -C --defined-only "${BINARY}" | sort -k4 >"${SYMBOLS}"

    {
        echo "# Annotated release disassembly of the measured logical routes (jit:18a87159)"
        echo "# binary: ${BINARY_NAME}"
        echo "# binary sha256: ${DIGEST}"
        echo "# rustc: $(rustc --version)"
        echo "# Fields: file  symbols  bytes  pattern"
    } >"${ASM}/${BINARY_NAME}/index.txt"

    while IFS='|' read -r label pattern; do
        [[ -z "${label}" || "${label}" == \#* ]] && continue
        target="${ASM}/${BINARY_NAME}/${label}.asm.txt"
        matches=$(grep -F "${pattern}" "${SYMBOLS}" | awk '$3 == "t" || $3 == "T" {print $1 ":" $2}' || true)
        copies=$(printf '%s\n' "${matches}" | grep -c . || true)
        bytes=0
        {
            echo "# label: ${label}"
            echo "# pattern: ${pattern}"
            echo "# binary: ${BINARY_NAME}"
            echo "# binary sha256: ${DIGEST}"
            echo "# text symbols matching this pattern: ${copies}"
            if [[ "${copies}" == 0 ]]; then
                echo "# no text symbol of this pattern is present in the measured binary, either"
                echo "# because the routine is inlined into its callers or because the build"
                echo "# contains no such path; this file records that rather than a silent gap."
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
    done <"${SYMBOL_TABLE}"

    # Instruction mix of the symbols the table names, plus the harness route
    # bodies that call them. Each count tests one field of an instruction line
    # exactly: the mnemonic after any prefix, or the presence of an indirect
    # call operand or a stack-slot operand. The classes are the ones the frozen
    # questions attribute to: a call is the public entry point's own overhead,
    # an indirect call is the per-call dispatch, a vector or scalar XOR is the
    # kernel body, and a stack-slot move is a spill.
    MIX_PATTERNS="${ASM}/${BINARY_NAME}/mix-patterns.txt"
    {
        awk -F'|' '/^[^#]/ && NF == 2 { print $2 }' "${SYMBOL_TABLE}"
        echo 'logical_buffer_harness::routes'
        echo "${BINARY_NAME//-/_}::"
    } >"${MIX_PATTERNS}"
    {
        echo "# Instruction mix of the measured logical routes (jit:18a87159)"
        echo "# binary: ${BINARY_NAME}"
        echo "# binary sha256: ${DIGEST}"
        echo "# Counts are of instruction lines within the named symbol; the symbols are"
        echo "# those the disassembly table names plus the harness route bodies calling them."
        echo
        echo "## call indirect-call vector-xor scalar-xor stack-slot-move total-instructions symbol"
        objdump -d --no-show-raw-insn -C "${BINARY}" | awk -F'\t' -v patterns="${MIX_PATTERNS}" '
            BEGIN {
                prefix = "^(lock|rep|repz|repnz|repe|repne|data16|data32|addr16|addr32|cs|ds|es|fs|gs|ss|notrack|bnd|xacquire|xrelease)$"
                while ((getline line < patterns) > 0) { if (line != "") wanted[++count] = line }
            }
            /^[0-9a-f]+ </ {
                fn = substr($0, index($0, "<"))
                keep = 0
                for (i = 1; i <= count; i++) { if (index(fn, wanted[i])) { keep = 1; break } }
                next
            }
            keep && NF == 2 && $1 ~ /^ +[0-9a-f]+:$/ {
                words = split($2, word, " ")
                i = 1
                while (i < words && word[i] ~ prefix) { i++ }
                mnemonic = word[i]
                seen[fn] = 1
                total[fn]++
                if (mnemonic ~ /^call/) {
                    calls[fn]++
                    if (word[i + 1] ~ /^\*/) indirect[fn]++
                }
                if (mnemonic ~ /^v?pxor/ || mnemonic ~ /^vxor[a-z]+$/) vectorxor[fn]++
                else if (mnemonic ~ /^xor[a-z]?$/) scalarxor[fn]++
                if (mnemonic ~ /^v?mov/ && $2 ~ /\(%r(sp|bp)\)/) spill[fn]++
            }
            END {
                for (f in seen) {
                    printf "%d %d %d %d %d %d %s\n", calls[f] + 0, indirect[f] + 0, \
                        vectorxor[f] + 0, scalarxor[f] + 0, spill[f] + 0, total[f] + 0, f \
                        | "LC_ALL=C sort -k7"
                }
                close("LC_ALL=C sort -k7")
            }'
    } >"${ASM}/${BINARY_NAME}/instruction-mix.txt"
done

echo "disassembly written to ${ASM}" >&2
