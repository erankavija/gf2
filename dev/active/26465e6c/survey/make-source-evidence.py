#!/usr/bin/env python3
"""Writes the code-claim register of the population-count survey (jit:26465e6c).

Every claim the findings make about libpopcnt, Mula's sse-popcount, gf2 or the
superseded v1 harness is a row: project, commit, path, line, the verbatim line
text read at generation time, and why the line matters. External lines are
read from the vendored copy when the file is vendored and otherwise from the
pinned checkout, whose HEAD must equal the pin; every vendored file must be
byte-identical to the checkout. A row whose line does not contain its
identifying fragment fails the script, so a citation cannot drift from its
claim without regeneration failing.

Usage: make-source-evidence.py [external-checkout-root]  (from the repository root;
the default root is .agents/ext/26465e6c beside the main checkout)
"""

import hashlib
import json
import pathlib
import subprocess
import sys

SURVEY = pathlib.Path("dev/active/26465e6c/survey")
OUTPUT = SURVEY / "source-evidence.json"

PINS = {
    "libpopcnt": {
        "upstream": "https://github.com/kimwalisch/libpopcnt.git",
        "tag": "v4.2",
        "commit": "923c43377278edc97de155ea70ac3ab20f397f3e",
        "checkout": "libpopcnt",
        "vendored": {"libpopcnt.h": "vendor/libpopcnt/libpopcnt.h", "LICENSE": "vendor/libpopcnt/LICENSE"},
    },
    "mula-sse-popcount": {
        "upstream": "https://github.com/WojciechMula/sse-popcount.git",
        "tag": None,
        "commit": "138c91e21c3e6dab7875521b5d33b995e0e4c85e",
        "checkout": "mula-sse-popcount",
        "vendored": {
            "config.h": "vendor/mula/config.h",
            "popcnt-avx2-harley-seal.cpp": "vendor/mula/popcnt-avx2-harley-seal.cpp",
            "popcnt-lookup.cpp": "vendor/mula/popcnt-lookup.cpp",
            "LICENSE": "vendor/mula/LICENSE",
        },
    },
}

# (claim, project, path, line, fragment, why)
CLAIMS = [
    ("libpopcnt-license", "libpopcnt", "LICENSE", 1, "BSD 2-Clause License",
     "libpopcnt is BSD-2-Clause, so vendoring its header with the licence is permitted."),
    ("libpopcnt-single-buffer-entry", "libpopcnt", "libpopcnt.h", 777,
     "static uint64_t popcnt(const void* data, uint64_t size)",
     "libpopcnt's x86 entry counts one buffer; it exposes no fused AND or XOR reduction."),
    ("libpopcnt-cpuid-cached", "libpopcnt", "libpopcnt.h", 793,
     "int cpuid = atomic_load_explicit(&libpopcnt_cpuid, memory_order_relaxed);",
     "Compiled as C without ISA flags, popcnt() dispatches on a cached CPUID word at run time."),
    ("libpopcnt-avx512-branch", "libpopcnt", "libpopcnt.h", 827, "size >= 40)",
     "The AVX-512 VPOPCNTDQ path needs its CPUID bit, which this Zen 3 host lacks; it is emitted but never taken."),
    ("libpopcnt-avx2-threshold", "libpopcnt", "libpopcnt.h", 838, "size >= 96)",
     "Below 96 bytes libpopcnt counts with scalar POPCNT; the compiled comparison is cmp $0x5f."),
    ("libpopcnt-harley-seal-threshold", "libpopcnt", "libpopcnt.h", 843, "if (size >= 1024)",
     "From 1 KiB libpopcnt switches to the AVX2 Harley-Seal loop; the compiled comparison is cmp $0x3ff."),
    ("libpopcnt-medium-loop", "libpopcnt", "libpopcnt.h", 846,
     "cnt += popcnt_avx2_medium(ptr256, size / 32);",
     "Between 96 bytes and 1 KiB libpopcnt runs a per-vector nibble-lookup loop, gf2's algorithm."),
    ("libpopcnt-scalar-loop", "libpopcnt", "libpopcnt.h", 867, "cnt += popcnt64(bits);",
     "libpopcnt's scalar path and every tail use the POPCNT instruction."),
    ("libpopcnt-harley-seal-trip", "libpopcnt", "libpopcnt.h", 567,
     "uint64_t limit = size - size % 16;",
     "libpopcnt's carry-save loop consumes 16 vectors per iteration."),
    ("libpopcnt-readme-no-isa-flags", "libpopcnt", "README.md", 36,
     "does not require any special compiler flags like ```-mavx2```!",
     "The upstream build is plain -O3 with runtime dispatch, the deployment model gf2 also uses."),
    ("libpopcnt-readme-o3", "libpopcnt", "README.md", 41, "cc  -O3 program.c",
     "The survey compiles libpopcnt with exactly -O3."),
    ("libpopcnt-changelog-medium", "libpopcnt", "ChangeLog", 6,
     "Up to 3x faster popcount for medium arrays (96 bytes to 1 KB)",
     "v4.2 introduced the medium-array loop; its thresholds were tuned upstream, not on this host."),
    ("mula-license-holder", "mula-sse-popcount", "LICENSE", 1, "Copyright (c) 2008-2016, Wojciech Mu",
     "Copyright line of the vendored reference."),
    ("mula-license-binary-clause", "mula-sse-popcount", "LICENSE", 13,
     "2. Redistributions in binary form must reproduce the above copyright",
     "The two-condition BSD text: BSD-2-Clause, so vendoring with the licence is permitted."),
    ("mula-aligned-dereference", "mula-sse-popcount", "popcnt-avx2-harley-seal.cpp", 77,
     "uint64_t total = AVX2_harley_seal::popcnt((const __m256i*) data, size / 32);",
     "The reference reads through const __m256i*, so its input must be 32-byte aligned (vmovdqa)."),
    ("mula-csa-trip", "mula-sse-popcount", "popcnt-avx2-harley-seal.cpp", 33,
     "const uint64_t limit = size - size % 16;",
     "Mula's carry-save loop engages only from 16 vectors, 512 bytes."),
    ("mula-vector-tail", "mula-sse-popcount", "popcnt-avx2-harley-seal.cpp", 64,
     "total = _mm256_add_epi64(total, popcount(data[i]));",
     "Vectors beyond the last 16-vector block are counted one at a time with the SWAR popcount."),
    ("mula-byte-tail", "mula-sse-popcount", "popcnt-avx2-harley-seal.cpp", 80,
     "total += lookup8bit[data[i]];",
     "Bytes beyond the last whole vector are counted by byte table lookup."),
    ("mula-single-buffer-registry", "mula-sse-popcount", "function_registry.cpp", 7,
     "using function_ptr      = std::uint64_t (*)(const uint8_t* data,  const size_t size);",
     "Every registered sse-popcount implementation counts one buffer; none is a fused AND or XOR reduction."),
    ("mula-registry-avx2-harley-seal", "mula-sse-popcount", "function_registry.cpp", 162,
     'add("avx2-harley-seal",', "The surveyed backend is the registry's avx2-harley-seal entry."),
    ("mula-makefile-flags", "mula-sse-popcount", "Makefile", 17,
     "FLAGS=-std=c++17 -O2 -Wall -pedantic -Wextra -Wfatal-errors",
     "Upstream compiles with -O2; the survey reuses these flags."),
    ("mula-makefile-intel", "mula-sse-popcount", "Makefile", 18,
     "FLAGS_INTEL=$(FLAGS) -mpopcnt -fabi-version=6", "Upstream's x86 flags."),
    ("mula-makefile-avx2", "mula-sse-popcount", "Makefile", 28,
     "FLAGS_AVX2=$(FLAGS_INTEL) -mavx2 -DHAVE_AVX2_INSTRUCTIONS",
     "Upstream's AVX2 build, which the survey compiles the vendored reference with."),
    ("mula-single-translation-unit", "mula-sse-popcount", "speed.cpp", 11, '#include "popcnt-all.cpp"',
     "Upstream builds its implementations as one translation unit, the model the survey keeps."),
    ("gf2-simd-threshold-default", "gf2", "crates/gf2-core/src/kernels/backend.rs", 83,
     "pub(crate) const SIMD_MIN_WORDS_DEFAULT: usize = 8;",
     "The shipped conservative build selects SIMD from 8 words; the compiled comparison is cmp $0x7."),
    ("gf2-simd-threshold-compare", "gf2", "crates/gf2-core/src/kernels/backend.rs", 115,
     "if _size >= SIMD_MIN_WORDS {", "select_backend_for_size compares the word count with the threshold."),
    ("gf2-baked-threshold", "gf2", "crates/gf2-core/src/tuning/baked.rs", 17,
     "pub(crate) const SIMD_MIN_WORDS: usize = 4;",
     "A four-word threshold exists only under --cfg gf2_tuning_baked, which the measured build does not set."),
    ("gf2-popcount-dispatcher", "gf2", "crates/gf2-core/src/kernels/ops.rs", 204,
     "pub fn popcount(buf: &[u64]) -> u64 {", "The production dispatcher, the baseline of every popcount cell."),
    ("gf2-scalar-count-ones", "gf2", "crates/gf2-core/src/kernels/scalar/logical.rs", 105,
     "buf.iter().map(|w| w.count_ones() as u64).sum()",
     "Below the threshold the dispatcher runs this portable loop, which a baseline x86-64 build lowers without POPCNT."),
    ("gf2-simd-backend-lazylock", "gf2", "crates/gf2-core/src/kernels/simd/mod.rs", 64,
     "pub static SIMD_BACKEND: LazyLock<Option<SimdBackend>> = LazyLock::new(SimdBackend::detect);",
     "Each SIMD-path call reads a lazily initialized backend before its indirect call."),
    ("gf2-simd-popcount-indirect", "gf2", "crates/gf2-core/src/kernels/simd/mod.rs", 56,
     "(self.fns.popcnt_fn)(buf)", "The SIMD path reaches the AVX2 kernel through a function pointer."),
    ("gf2-simd-fns-private", "gf2", "crates/gf2-core/src/kernels/simd/mod.rs", 18,
     "fns: gf2_kernels_simd::LogicalFns,",
     "gf2-core keeps the kernel bundle private, so it offers no public fused AND-popcount route."),
    ("gf2-avx2-popcnt", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", 264,
     "unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {", "gf2's AVX2 nibble-lookup population count."),
    ("gf2-avx2-nibble-lookup", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", 286,
     "let pc_lo = _mm256_shuffle_epi8(lut, lo);",
     "gf2's AVX2 kernel counts every vector by nibble lookup; it has no carry-save stage."),
    ("gf2-avx2-byte-tail", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", 308,
     "total += (*tail_ptr.add(k)).count_ones() as u64;",
     "gf2's AVX2 kernel counts the bytes beyond the last whole vector one byte at a time."),
    ("gf2-avx2-and-popcnt", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", 316,
     "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {", "gf2's fused AVX2 AND-popcount kernel."),
    ("gf2-kernels-detect-public", "gf2", "crates/gf2-kernels-simd/src/lib.rs", 103,
     "pub fn detect() -> Option<LogicalFns> {",
     "The fused kernel is public only through gf2-kernels-simd's detect() bundle."),
    ("gf2-matvec-uses-fused", "gf2", "crates/gf2-core/src/matrix.rs", 1560,
     "y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);",
     "The fused kernel is load-bearing in BitMatrix::matvec_simd, which keeps only each row's parity."),
    ("gf2-bitvec-count-ones", "gf2", "crates/gf2-core/src/bitvec.rs", 602,
     "crate::kernels::ops::popcount(&self.data) as usize",
     "BitVec::count_ones counts every storage word through the dispatcher, relying on zero padding."),
    ("gf2-bitvec-and", "gf2", "crates/gf2-core/src/bitvec.rs", 403,
     "crate::kernels::ops::and_inplace(&mut self.data, &other.data);",
     "BitVec::bit_and_into is the public AND the two-pass route models."),
    ("gf2-from-words-precondition", "gf2", "crates/gf2-core/src/bitvec.rs", 186,
     "/// The caller must ensure tail masking invariant: padding bits beyond",
     "Tail semantics are a caller precondition of from_words; count_ones does not mask."),
    ("v1-harness-bank-modulo", "gf2", "dev/active/26465e6c/superseded/v1/survey/gf2-side/src/main.rs", 448,
     "let value = popcount_operation(arm, black_box(fixtures[bank % banks].words()))",
     "The v1 gf2 arm reduced the bank index by a runtime modulus in every timed call (a div in the receipted binary)."),
    ("v1-harness-string-dispatch", "gf2", "dev/active/26465e6c/superseded/v1/survey/gf2-side/src/main.rs", 279,
     "fn popcount_operation(arm: &str, buffer: &[u64]) -> Result<u64, String> {",
     "The v1 gf2 arm matched the arm name as a string inside every timed call."),
    ("v1-c-harness-bank", "gf2", "dev/active/26465e6c/superseded/v1/survey/wire_common.h", 253,
     "size_t bank = streaming ? (start + (size_t)i) & (SURVEY_BANKS - 1U) : 0U;",
     "The v1 C arms selected the bank without a division, so the two sides of a cell ran different harness code."),
    ("v1-external-march-native", "gf2", "dev/active/26465e6c/superseded/v1/survey/Makefile", 16,
     "OPT ?= -O3 -march=native", "The v1 external arms were built -march=native, not with their upstream flags."),
]


def git(*args, cwd="."):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def sha256(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def external_root():
    """The pinned checkouts live beside the main checkout, not in each worktree."""
    if len(sys.argv) > 1:
        return pathlib.Path(sys.argv[1])
    common = pathlib.Path(git("rev-parse", "--path-format=absolute", "--git-common-dir"))
    return common.parent / ".agents/ext/26465e6c"


EXT = None


def main():
    global EXT
    EXT = external_root()
    commits = {"gf2": git("rev-parse", "HEAD")}
    files = {}
    for project, pin in PINS.items():
        checkout = EXT / pin["checkout"]
        if git("rev-parse", "HEAD", cwd=checkout) != pin["commit"]:
            sys.exit(f"{checkout} is not at the pinned commit {pin['commit']}")
        if pin["tag"] and git("rev-parse", f"{pin['tag']}^{{commit}}", cwd=checkout) != pin["commit"]:
            sys.exit(f"{project} tag {pin['tag']} does not name the pinned commit")
        if git("status", "--porcelain", cwd=checkout):
            sys.exit(f"{checkout} has local modifications")
        commits[project] = pin["commit"]
        for upstream, vendored in pin["vendored"].items():
            if sha256(checkout / upstream) != sha256(SURVEY / vendored):
                sys.exit(f"{SURVEY / vendored} differs from {project} {upstream}")
            files[str(SURVEY / vendored)] = {
                "project": project, "upstream_path": upstream, "sha256": sha256(SURVEY / vendored)}
    claims = []
    for claim, project, path, hint, fragment, why in CLAIMS:
        if project == "gf2":
            source = pathlib.Path(path)
        else:
            vendored = PINS[project]["vendored"].get(path)
            source = SURVEY / vendored if vendored else EXT / PINS[project]["checkout"] / path
        lines = source.read_text(encoding="utf-8").splitlines()
        if not (0 < hint <= len(lines)) or fragment not in lines[hint - 1]:
            sys.exit(f"{claim}: {source}:{hint} does not contain {fragment!r}")
        claims.append({"claim": claim, "project": project, "commit": commits[project], "path": path,
                       "line": hint, "text": lines[hint - 1].strip(), "why": why})
    register = {
        "schema": "popcount-source-evidence-v1",
        "pins": {project: {key: pin[key] for key in ("upstream", "tag", "commit")}
                 for project, pin in PINS.items()},
        "commits": commits,
        "vendored_files": files,
        "claims": claims,
    }
    OUTPUT.write_text(json.dumps(register, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{OUTPUT}: {len(claims)} claims, {len(files)} vendored files verified")


if __name__ == "__main__":
    main()
