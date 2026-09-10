#!/usr/bin/env python3
"""Record source, license, build, binary and selected-backend evidence.

Selected-backend evidence is observed, not asserted: every arm binary and
library is disassembled with `objdump` and the instruction mix of the
functions on each measured route is counted by register class (zmm / ymm /
xmm / general-purpose), together with the presence of `cpuid`, which is the
only way any of these routes could dispatch at run time. The arms' own
`--backend` output contributes compile-time facts of the linked libraries.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = Path(subprocess.check_output(
    ["git", "-C", ROOT, "rev-parse", "--show-toplevel"], text=True
).strip())
COMMON_ROOT = Path(subprocess.check_output(
    ["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"],
    text=True,
).strip()).parent
EXT = Path(os.environ.get("GF2_SURVEY_EXT", COMMON_ROOT / ".agents/ext/6fb89a3c"))
GF2_TARGET = Path(os.environ.get("GF2_SURVEY_GF2_TARGET", ROOT / "gf2-side/target/release"))
RUST_RUSTFLAGS = os.environ.get("GF2_SURVEY_RUSTFLAGS", "-C target-cpu=x86-64")

FUNCTION_HEADER = re.compile(r"^[0-9a-f]+ <(.+)>:$")
INSTRUCTION = re.compile(r"^\s*[0-9a-f]+:\s+([a-z][a-z0-9.]*)\s*(.*)$")


def output(*args: str, cwd: Path | None = None) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True, stderr=subprocess.STDOUT).strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def backend(binary: str) -> dict:
    return json.loads(output(str(ROOT / binary), "--backend"))


def symbols(path: Path, wanted: tuple[str, ...]) -> list[str]:
    listing = output("nm", "-A", str(path))
    return sorted({name for name in wanted if any(line.rstrip().endswith(f" {name}") for line in listing.splitlines())})


def git_pin(path: Path) -> dict:
    return {
        "commit": output("git", "rev-parse", "HEAD", cwd=path),
        "describe": output("git", "describe", "--tags", "--always", cwd=path),
        "tree_status": output("git", "status", "--short", cwd=path),
    }


def disassemble(path: Path) -> dict[str, list[tuple[str, str]]]:
    """Maps every function in `path` to its (mnemonic, operands) list."""
    text = subprocess.run(
        ["objdump", "-d", "--no-show-raw-insn", "--demangle", str(path)],
        text=True, capture_output=True, check=True,
    ).stdout
    functions: dict[str, list[tuple[str, str]]] = {}
    current: list[tuple[str, str]] | None = None
    for line in text.splitlines():
        header = FUNCTION_HEADER.match(line)
        if header:
            current = functions.setdefault(header.group(1), [])
            continue
        instruction = INSTRUCTION.match(line)
        if instruction and current is not None:
            current.append((instruction.group(1), instruction.group(2)))
    return functions


def classify(instructions: list[tuple[str, str]]) -> dict:
    classes = Counter()
    mnemonics = Counter()
    for mnemonic, operands in instructions:
        mnemonics[mnemonic] += 1
        if "zmm" in operands:
            classes["zmm"] += 1
        elif "ymm" in operands:
            classes["ymm"] += 1
        elif "xmm" in operands:
            classes["xmm"] += 1
        else:
            classes["general"] += 1
    return {
        "instructions": len(instructions),
        "by_register_class": dict(sorted(classes.items())),
        "cpuid": mnemonics.get("cpuid", 0),
        "top_mnemonics": dict(mnemonics.most_common(12)),
    }


def probe(path: Path, routes: dict[str, str]) -> dict:
    """Instruction-mix observation of the named routes and of the whole file.

    `routes` maps a route name to a regular expression over (demangled)
    function names; every matching function is reported separately so a
    missing symbol is visible as an empty match rather than a silent zero.
    """
    functions = disassemble(path)
    everything = [instruction for body in functions.values() for instruction in body]
    report = {"file": str(path.name), "sha256": sha256(path), "whole_file": classify(everything), "routes": {}}
    for route, pattern in routes.items():
        matcher = re.compile(pattern)
        matched = {name: classify(body) for name, body in functions.items() if matcher.search(name)}
        report["routes"][route] = {"pattern": pattern, "functions": matched}
    return report


def main() -> None:
    m4ri_library = (EXT / "prefix/lib/libm4ri.so.2").resolve()
    bitshuffle_archive = EXT / "prefix/lib/libbitshuffle_min.a"
    isal_archive = EXT / "prefix/lib/libisal_base.a"
    evidence = {
        "schema": "gf2-external-comparator-build-evidence-v2",
        "issue": "6fb89a3c",
        "host": {
            "kernel": output("uname", "-srmo"),
            "compiler": output("gcc", "--version").splitlines()[0],
            "rust_compiler": output("rustup", "run", "1.95.0", "rustc", "--version"),
            "rust_rustflags": RUST_RUSTFLAGS,
            "nasm": output("nasm", "-v") if shutil.which("nasm") else None,
            "objdump": output("objdump", "--version").splitlines()[0],
            "avx2_observed": "avx2" in Path("/proc/cpuinfo").read_text().split("flags", 1)[-1].split(),
        },
        "m4ri": {
            "version": "20260122",
            "source": {
                "kind": "release-tarball",
                "archive": "m4ri-20260122.tar.gz",
                "sha256": sha256(EXT / "m4ri-20260122.tar.gz"),
            },
            "license": {
                "spdx": "GPL-2.0-or-later",
                "evidence": "COPYING is the GNU GPL version 2 text; m4ri/mzd.h header: 'Distributed under the terms of the GNU General Public License (GPL) version 2 or higher'; README.md: 'available under the General Public License Version 2 or later (GPLv2+)'",
                "file_sha256": sha256(EXT / "m4ri-src/COPYING"),
            },
            "build": {
                "configure": "CFLAGS='-O3 -march=native -fPIC' ./configure --prefix=<ext>/prefix --disable-static",
                "library_path": "<ext>/prefix/lib/libm4ri.so.2",
                "library_sha256": sha256(m4ri_library),
                "compiled_sse2": "#define __M4RI_HAVE_SSE2\t\t1" in (EXT / "prefix/include/m4ri/m4ri_config.h").read_text(),
                "enable_mmc": "#define __M4RI_ENABLE_MMC               1" in (EXT / "prefix/include/m4ri/m4ri_config.h").read_text(),
            },
            "binary_observation": backend("m4ri_transpose_arm"),
            "consumer_binary_observation": backend("m4ri_genmatrix_arm"),
            "linked_symbols": symbols(ROOT / "m4ri_transpose_arm", ("mzd_transpose",)) + symbols(ROOT / "m4ri_genmatrix_arm", ("mzd_echelonize_m4ri", "mzd_copy")),
            "disassembly": probe(m4ri_library, {
                "mzd_transpose": r"^(mzd_transpose|_mzd_transpose.*|_mzd_copy_transpose.*)$",
                "mzd_echelonize_m4ri": r"^(mzd_echelonize_m4ri|_mzd_echelonize_m4ri|_mzd_gauss_submatrix.*|mzd_make_table.*|mzd_process_rows.*)$",
                "memory_cache": r"^(m4ri_mmc_malloc|m4ri_mmc_free|mzd_init|mzd_free)$",
            }),
        },
        "bitshuffle": {
            "version": "0.5.2",
            "source": git_pin(EXT / "bitshuffle"),
            "license": {"spdx": "MIT", "file_sha256": sha256(EXT / "bitshuffle/LICENSE")},
            "build": {
                "commands": [
                    "gcc -O3 -march=native -fPIC -Wall -Wextra -c -I bitshuffle/src bitshuffle/src/bitshuffle_core.c",
                    "gcc -O3 -march=native -fPIC -Wall -Wextra -c -I bitshuffle/src bitshuffle/src/iochain.c",
                    "ar rcs <ext>/prefix/lib/libbitshuffle_min.a bitshuffle_core.o iochain.o",
                ],
                "archive_sha256": sha256(bitshuffle_archive),
            },
            "binary_observation": backend("bitshuffle_transpose_arm"),
            "linked_symbols": symbols(ROOT / "bitshuffle_transpose_arm", ("bshuf_bitshuffle", "bshuf_trans_bit_elem", "bshuf_trans_bit_elem_AVX", "bshuf_trans_bit_elem_AVX512", "bshuf_trans_bit_elem_SSE", "bshuf_trans_bit_elem_scal")),
            "disassembly": probe(ROOT / "bitshuffle_transpose_arm", {
                "bshuf_trans_bit_elem_dispatcher": r"^bshuf_trans_bit_elem$",
                "avx2_route": r"^(bshuf_trans_bit_elem_AVX|bshuf_trans_bit_byte_AVX|bshuf_trans_byte_elem_SSE|bshuf_trans_bitrow_eight|bshuf_trans_byte_elem_SSE_(16|32|64)|bshuf_trans_byte_elem_remainder|bshuf_trans_bit_byte_remainder)$",
                "sse2_route": r"^(bshuf_trans_bit_elem_SSE|bshuf_trans_bit_byte_SSE)$",
                "scalar_route": r"^(bshuf_trans_bit_elem_scal|bshuf_trans_bit_byte_scal|bshuf_trans_byte_elem_scal)$",
            }),
        },
        "isa_l": {
            "version": "v2.32.1",
            "source": git_pin(EXT / "isa-l"),
            "license": {"spdx": "BSD-3-Clause", "file_sha256": sha256(EXT / "isa-l/LICENSE")},
            "build": {
                "command": "gcc -O3 -march=native -fPIC -Wall -Wextra -c isa-l/raid/raid_base.c; ar rcs <ext>/prefix/lib/libisal_base.a raid_base.o",
                "archive_sha256": sha256(isal_archive),
            },
            "binary_observation": backend("isal_xor_arm"),
            "linked_symbols": symbols(ROOT / "isal_xor_arm", ("xor_gen", "xor_gen_base", "xor_gen_sse", "xor_gen_avx", "xor_gen_avx512")),
            "disassembly": probe(ROOT / "isal_xor_arm", {"xor_gen_base": r"^xor_gen_base$"}),
        },
        "harness_binaries": {
            name: {"sha256": sha256(ROOT / name), "file": output("file", "-b", str(ROOT / name))}
            for name in ("m4ri_transpose_arm", "m4ri_genmatrix_arm", "bitshuffle_transpose_arm", "isal_xor_arm")
        },
        "rust_harness_binaries": {
            name: {
                "sha256": sha256(GF2_TARGET / name),
                "file": output("file", "-b", str(GF2_TARGET / name)),
            }
            for name in ("gf2_transpose_arm", "gf2_logical_xor_arm", "gf2_bch_genmatrix_arm")
        },
        "gf2_disassembly": {
            "gf2_transpose_arm": probe(GF2_TARGET / "gf2_transpose_arm", {
                "avx2_transpose": r"transpose_64x64_avx2",
                "scalar_transpose": r"transpose_64x64_scalar",
                "bitmatrix_transpose": r"BitMatrix.*transpose",
            }),
            "gf2_logical_xor_arm": probe(GF2_TARGET / "gf2_logical_xor_arm", {
                "avx2_xor": r"avx2_xor_into",
                "scalar_xor": r"scalar_xor_inplace",
                "xor_fn_wrapper": r"fns::xor_fn",
            }),
            "gf2_bch_genmatrix_arm": probe(GF2_TARGET / "gf2_bch_genmatrix_arm", {
                "generator_by_encoding": r"write_generator_by_encoding|for_each_generator_row",
                "systematic_encode": r"bch::encode",
            }),
        },
    }
    (ROOT / "build-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
