#!/usr/bin/env python3
"""Records the instructions every survey arm emits (jit:26465e6c).

Disassembles the built `popcount-arm` binary with objdump and, per arm
function, counts the instruction classes that decide its algorithm, lists the
immediate comparisons that implement its dispatch thresholds, lists every
function of the binary that contains a `popcnt` instruction, and prints the
shared timed loops in full. It closes with the cache geometry sysfs reports
for the CPU that runs every cell. The thresholds and backends the findings
name are read from this output, not from any upstream document or other
processor.

Usage: observe-instructions.py <popcount-arm> > instruction-observation.txt
"""

import hashlib
import pathlib
import re
import subprocess
import sys
from collections import Counter

# (label, demangled symbol) in report order.
FUNCTIONS = [
    ("gf2 production dispatcher", "popcount_survey::arms::production_dispatch"),
    ("gf2 AVX2 nibble-LUT kernel", "gf2_kernels_simd::x86::avx2::avx2_popcnt"),
    ("scalar POPCNT control", "popcount_survey::arms::popcnt_instruction_loop"),
    ("compiler count_ones control", "popcount_survey::arms::compiler_count_ones"),
    ("libpopcnt entry", "survey_libpopcnt_words"),
    ("libpopcnt popcnt() dispatch", "popcnt"),
    ("libpopcnt popcnt_avx2_medium", "popcnt_avx2_medium"),
    ("libpopcnt popcnt_avx2 (Harley-Seal)", "popcnt_avx2"),
    ("Mula entry", "survey_mula_avx2_harley_seal_words"),
    ("Mula popcnt_AVX2_harley_seal", "popcnt_AVX2_harley_seal(unsigned char const*, unsigned long)"),
    ("Mula AVX2_harley_seal::popcnt", "AVX2_harley_seal::popcnt(long long __vector(4) const*, unsigned long)"),
    ("gf2 fused AND-popcount kernel", "gf2_kernels_simd::x86::avx2::avx2_and_popcnt"),
    ("AND scalar control", "popcount_survey::arms::and_scalar_control"),
    ("AND two-pass public route", "popcount_survey::arms::and_two_pass"),
]
TIMED = "popcount_arm::time"
# The bit-twiddle population count's masks and byte-sum multiplier.
SWAR_MASKS = {
    "$0x5555555555555555", "$0x3333333333333333", "$0xf0f0f0f0f0f0f0f", "$0x101010101010101",
}
KERNELS_NAMED_POPCNT = [
    "gf2_kernels_simd::x86::avx2::avx2_popcnt",
    "gf2_kernels_simd::x86::avx2::avx2_and_popcnt",
]
# Every single-core cell resolves to this logical CPU (the receipts' resolved_cpus).
MEASURED_CPU = 0
SYSFS_CACHE = f"/sys/devices/system/cpu/cpu{MEASURED_CPU}/cache"
CLASSES = [
    "popcnt", "vpshufb", "vpsadbw", "psadbw", "vpand", "vpor", "vpxor", "vpaddq",
    "vpaddb", "vpsrlw", "psrlw", "pand", "vmovdqa", "vmovdqu", "movdqu", "cpuid",
    "xgetbv", "div", "imul",
]
LINE = re.compile(r"^\s*([0-9a-f]+):\s+(\S+)\s*(.*)$")
SYMBOL = re.compile(r"^([0-9a-f]+) ([0-9a-f]+) [tTwW] (.*)$")


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def symbols(binary):
    table = {}
    for line in run("nm", "-C", "-S", binary).splitlines():
        match = SYMBOL.match(line)
        if match:
            address, size, name = match.groups()
            table.setdefault(name, []).append((int(address, 16), int(size, 16)))
    return table


def disassemble(binary, start, size):
    out = run(
        "objdump", "-d", "--no-show-raw-insn", "-C",
        f"--start-address={start:#x}", f"--stop-address={start + size:#x}", binary,
    )
    rows = []
    for line in out.splitlines():
        match = LINE.match(line)
        if match:
            rows.append((int(match.group(1), 16), match.group(2), match.group(3)))
    return rows


def describe(label, name, rows, size):
    counts = Counter(mnemonic for _, mnemonic, _ in rows)
    classes = ", ".join(f"{c}={counts[c]}" for c in CLASSES if counts[c])
    compares = sorted({
        operands.split(",")[0]
        for _, mnemonic, operands in rows
        if mnemonic.startswith("cmp") and operands.startswith("$")
    })
    masks = sorted({
        operands.split(",")[0]
        for _, mnemonic, operands in rows
        if mnemonic == "and" and operands.startswith("$0xffffffffffff")
    })
    swar = sorted({
        operands.split(",")[0]
        for _, mnemonic, operands in rows
        if mnemonic == "movabs" and operands.split(",")[0] in SWAR_MASKS
    })
    indirect = sum(
        1 for _, mnemonic, operands in rows
        if mnemonic in ("call", "jmp") and computed(operands)
    )
    direct = sorted({
        operands.split("<", 1)[1].rstrip(">")
        for _, mnemonic, operands in rows
        if mnemonic in ("call", "jmp") and "<" in operands and not operands.startswith("*")
        and not operands.split("<", 1)[1].startswith(name)
    })
    print(f"## {label}: `{name}` ({size} bytes, {len(rows)} instructions)")
    print(f"- instruction classes: {classes or 'none of the tracked classes'}")
    print(f"- immediate comparisons: {' '.join(compares) or 'none'}")
    if masks:
        print(f"- alignment/trip-count masks: {' '.join(masks)}")
    if swar:
        print(f"- 64-bit SWAR popcount masks: {' '.join(swar)}")
    print(f"- computed (non-GOT) indirect calls/jumps: {indirect}")
    print(f"- direct calls/jumps out: {'; '.join(direct) or 'none'}")
    print()


def computed(operands):
    """An indirect target read from a register or data, not a GOT slot."""
    return operands.startswith("*") and "(%rip)" not in operands


def popcnt_sites(binary):
    """Per function: `popcnt` mnemonics, and disassembly lines (header included)
    that merely contain the substring, such as branch targets named after the
    function."""
    mnemonics, substrings = Counter(), Counter()
    function = None
    header = re.compile(r"^[0-9a-f]+ <(.*)>:$")
    for line in run("objdump", "-d", "--no-show-raw-insn", "-C", binary).splitlines():
        match = header.match(line)
        if match:
            function = match.group(1)
        if function is None:
            continue
        if "popcnt" in line:
            substrings[function] += 1
        row = LINE.match(line)
        if row and row.group(2) == "popcnt":
            mnemonics[function] += 1
    return mnemonics, substrings


def cache_geometry():
    rows = []
    for index in sorted(pathlib.Path(SYSFS_CACHE).glob("index*")):
        fields = {name: (index / name).read_text().strip()
                  for name in ("level", "type", "size", "shared_cpu_list")}
        rows.append(fields)
    return rows


def timed_loops(rows):
    """Loops containing a computed indirect call: the per-call harness path."""
    loops = []
    for index, (address, mnemonic, operands) in enumerate(rows):
        if mnemonic == "call" and computed(operands):
            for later in rows[index + 1:]:
                if later[1] == "jne":
                    target = int(later[2].split()[0], 16)
                    if target <= address:
                        loops.append((target, later[0]))
                    break
    return loops


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    binary = sys.argv[1]
    table = symbols(binary)
    digest = hashlib.sha256(open(binary, "rb").read()).hexdigest()
    print("# Instruction observation of the survey arm binary")
    print()
    print(f"- binary sha256: `{digest}`")
    print(f"- disassembler: {run('objdump', '--version').splitlines()[0]}")
    print(f"- external build record: `{run(binary, '--build-identity').strip()}`")
    print()
    for label, name in FUNCTIONS:
        entries = table.get(name)
        if not entries or len(entries) != 1:
            print(f"## {label}: `{name}` not uniquely present ({entries})")
            print()
            continue
        start, size = entries[0]
        describe(label, name, disassemble(binary, start, size), size)
    print("## Every function containing `popcnt`")
    print()
    sites, substrings = popcnt_sites(binary)
    for function, count in sorted(sites.items()):
        print(f"- `{function}`: {count}")
    print(f"- total: {sum(sites.values())} `popcnt` instructions in {len(sites)} functions")
    print()
    print("Lines that only contain the substring `popcnt`, header included, are not "
          "instructions; for gf2's AVX2 kernels they are branch targets named after the "
          "function:")
    print()
    for name in KERNELS_NAMED_POPCNT:
        print(f"- `{name}`: {substrings[name]} lines containing the substring, "
              f"{sites[name]} `popcnt` instructions")
    print()
    print("## Shared timed loops")
    print()
    print(
        "Every arm of a family runs the same `time` monomorph; the arm is the "
        "target of its computed indirect call. Each loop below is printed from "
        "its backward-branch target to the branch."
    )
    print()
    for monomorph, (start, size) in enumerate(table.get(TIMED, [])):
        rows = disassemble(binary, start, size)
        for loop_start, loop_end in timed_loops(rows):
            print(f"### `{TIMED}` monomorph {monomorph}, loop {loop_start:#x}..{loop_end:#x}")
            print("```")
            for address, mnemonic, operands in rows:
                if loop_start <= address <= loop_end:
                    print(f"{address:x}: {mnemonic} {operands}".rstrip())
            print("```")
            print()
    print(f"## Cache geometry of CPU {MEASURED_CPU} ({SYSFS_CACHE})")
    print()
    for fields in cache_geometry():
        print(f"- L{fields['level']} {fields['type']}: {fields['size']}, shared with CPUs "
              f"{fields['shared_cpu_list']}")


if __name__ == "__main__":
    main()
