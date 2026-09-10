#!/usr/bin/env python3
"""Record source, build, binary, and selected-backend evidence for the survey."""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
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


def main() -> None:
    m4ri_library = (EXT / "prefix/lib/libm4ri.so.2").resolve()
    bitshuffle_archive = EXT / "prefix/lib/libbitshuffle_min.a"
    isal_archive = EXT / "prefix/lib/libisal_base.a"
    evidence = {
        "schema": "gf2-external-comparator-build-evidence-v1",
        "issue": "6fb89a3c",
        "host": {
            "kernel": output("uname", "-srmo"),
            "compiler": output("gcc", "--version").splitlines()[0],
            "rust_compiler": output("rustup", "run", "1.95.0", "rustc", "--version"),
            "nasm": output("nasm", "-v") if shutil.which("nasm") else None,
            "avx2_observed": "avx2" in Path("/proc/cpuinfo").read_text().split("flags", 1)[-1].split(),
        },
        "m4ri": {
            "version": "20260122",
            "source": {
                "kind": "release-tarball",
                "archive": "m4ri-20260122.tar.gz",
                "sha256": sha256(EXT / "m4ri-20260122.tar.gz"),
            },
            "license": {"spdx": "GPL-2.0-or-later", "file_sha256": sha256(EXT / "m4ri-src/COPYING")},
            "build": {
                "configure": "CFLAGS='-O3 -march=native -fPIC' ./configure --prefix=<ext>/prefix --disable-static",
                "library_path": "<ext>/prefix/lib/libm4ri.so.2",
                "library_sha256": sha256(m4ri_library),
                "compiled_sse2": "#define __M4RI_HAVE_SSE2\t\t1" in (EXT / "prefix/include/m4ri/m4ri_config.h").read_text(),
            },
            "binary_observation": backend("m4ri_transpose_arm"),
            "consumer_binary_observation": backend("m4ri_genmatrix_arm"),
            "linked_symbols": symbols(ROOT / "m4ri_transpose_arm", ("mzd_transpose",)),
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
            "linked_symbols": symbols(ROOT / "bitshuffle_transpose_arm", ("bshuf_bitshuffle", "bshuf_trans_bit_elem_AVX", "bshuf_trans_bit_elem_AVX512", "bshuf_trans_bit_elem_SSE")),
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
        },
        "harness_binaries": {
            name: {"sha256": sha256(ROOT / name), "file": output("file", "-b", str(ROOT / name))}
            for name in ("m4ri_transpose_arm", "m4ri_genmatrix_arm", "bitshuffle_transpose_arm", "isal_xor_arm")
        },
        "rust_harness_binaries": {
            name: {
                "sha256": sha256(ROOT / "gf2-side/target/release" / name),
                "file": output("file", "-b", str(ROOT / "gf2-side/target/release" / name)),
            }
            for name in ("gf2_transpose_arm", "gf2_logical_xor_arm", "gf2_bch_genmatrix_arm")
        },
    }
    (ROOT / "build-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
