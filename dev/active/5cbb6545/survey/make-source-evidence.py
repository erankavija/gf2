#!/usr/bin/env python3
"""Write the survey's code claims with verbatim source lines (jit:5cbb6545).

Each claim names an ID the findings cite, the file, the line, a fragment the line
must contain, why the line matters for the count routes, and its topic. The script reads
each line from the working tree, refuses a claim whose line lacks its fragment,
and records the verbatim text with the revision, so every mechanism the findings
cite can be checked at its location. The asm claims cite the committed artefacts
beside the kernel sources, which the regeneration script writes from the
compiler, so an instruction claim is checked the same way as a source claim.

Usage: make-source-evidence.py > dev/active/5cbb6545/survey/source-evidence.json
"""

import json
import pathlib
import subprocess
import sys

OPS = "crates/gf2-core/src/kernels/ops.rs"
BACKEND = "crates/gf2-core/src/kernels/backend.rs"
BAKED = "crates/gf2-core/src/tuning/baked.rs"
TUNING = "crates/gf2-core/src/tuning/mod.rs"
MATRIX = "crates/gf2-core/src/matrix.rs"
BITVEC = "crates/gf2-core/src/bitvec.rs"
SCALAR = "crates/gf2-core/src/kernels/scalar/logical.rs"
SIMD_BACKEND = "crates/gf2-core/src/kernels/simd/mod.rs"
CORE_MANIFEST = "crates/gf2-core/Cargo.toml"
KERNEL = "crates/gf2-kernels-simd/src/x86/popcount.rs"
BUNDLE = "crates/gf2-kernels-simd/src/lib.rs"
CSA_ASM = "crates/gf2-kernels-simd/src/x86/asm/popcount.asm.txt"
LUT_ASM = "crates/gf2-kernels-simd/src/x86/asm/avx2.asm.txt"

CLAIMS = [
    # Public entry points and the routes they reach.
    ("bitvec-count-ones", BITVEC, 602, "crate::kernels::ops::popcount(&self.data)",
     "BitVec::count_ones is the unfused public entry point: it resolves one route per call",
     "entry-point"),
    ("matvec-entry", MATRIX, 1538, "pub fn matvec(&self, x: &crate::BitVec)",
     "BitMatrix::matvec is the fused public entry point", "entry-point"),
    ("matvec-resolve-once", MATRIX, 1604, "let and_popcount = crate::kernels::ops::resolve_and_popcount(self.stride_words)",
     "the product resolves the fused route once for the stride it repeats, so a row pays no "
     "resolution", "entry-point"),
    ("ops-scalar-retained", OPS, 341, "SelectedBackend::Scalar => scalar_popcount",
     "below the bit-backend SIMD threshold the resolver returns the scalar backend's portable "
     "count: the established implementation the measured no-win region retains", "no-win-region"),
    ("ops-csa-boundary", OPS, 335, "backend.popcnt_csa_fn",
     "at or above the carry-save boundary the unfused resolver returns the Harley-Seal kernel",
     "boundary"),
    ("ops-fused-csa-boundary", OPS, 378, "backend.and_popcnt_csa_fn",
     "the fused resolver splits at the same boundary", "boundary"),
    ("ops-route-report", OPS, 245, "word_len >= crate::kernels::backend::POPCOUNT_CSA_MIN_WORDS",
     "popcount_route reports the boundary at run time, so a test or an arm observes the route "
     "instead of inferring it", "boundary"),
    ("backend-simd-threshold", BACKEND, 133, "if _size >= SIMD_MIN_WORDS",
     "the bit-backend threshold that separates the scalar count from the vector kernels",
     "boundary"),
    ("backend-csa-default", BACKEND, 95, "POPCOUNT_CSA_MIN_WORDS_DEFAULT: usize = 256",
     "the adopted carry-save boundary in the conservative table's compile-time counterpart",
     "tuning"),
    ("baked-csa-mirror", BAKED, 27, "POPCOUNT_CSA_MIN_WORDS: usize = 256",
     "the baked table mirrors the same boundary, so the cfg-selected build takes the same route",
     "tuning"),
    ("tuning-csa-selector", TUNING, 561, "popcount_csa_min_words: usize",
     "the canonical tuning mechanism carries the boundary as a bit-backend selector, which is "
     "how the sweeps moved it", "tuning"),
    ("core-simd-optional", CORE_MANIFEST, 56, "simd = []",
     "the kernel routes are reachable only through a non-default feature of gf2-core",
     "feature"),
    ("scalar-count-ones", SCALAR, 105, "buf.iter().map(|w| w.count_ones() as u64).sum()",
     "the established sub-threshold count: one portable count_ones per word, which this "
     "toolchain lowers without POPCNT", "no-win-region"),
    ("legacy-bundle-popcount", SIMD_BACKEND, 56, "(self.fns.popcnt_fn)(buf)",
     "the route the pre-change dispatcher took at and above the threshold, which the baseline arm "
     "reproduces", "baseline"),
    ("bundle-scalar-popcnt-comparator", BUNDLE, 115, "pub popcnt_scalar_fn: fn(&[u64]) -> u64",
     "the scalar POPCNT kernel stays in the bundle as the measured comparator no resolver selects",
     "no-win-region"),
    ("csa-block-vectors", KERNEL, 36, "pub const CSA_BLOCK_VECTORS: usize = 16",
     "one carry-save block folds sixteen vectors, so the block is 512 bytes and a shorter buffer "
     "reaches only the remainder", "kernel"),
    # Emitted instructions: the carry-save kernel.
    ("csa-block-loop", CSA_ASM, 44, ".LBB120_4:",
     "the carry-save block loop of avx2_popcnt_csa", "dependency-chain"),
    ("csa-unaligned-loads", CSA_ASM, 45, "vmovdqu ymm9, ymmword ptr [rdi + r8]",
     "the block loop loads its sixteen vectors with unaligned moves, so no alignment "
     "precondition reaches a caller", "alignment"),
    ("csa-block-stride", CSA_ASM, 144, "add r8, 512",
     "the block loop advances 512 bytes per iteration", "kernel"),
    ("csa-callee-saved-spill", CSA_ASM, 148, "push rbp",
     "the carry-save kernel spills six callee-saved registers: holding five carry-save "
     "accumulators and the total across the loop costs the register file", "register-pressure"),
    ("csa-vector-remainder", CSA_ASM, 217, ".LBB120_7:",
     "the per-vector remainder loop between the block loop and the word tail: a buffer shorter "
     "than one block is counted here through the same nibble lookup the established route uses",
     "tail"),
    ("csa-word-tail", CSA_ASM, 263, "imul r11, rcx",
     "the word tail of the carry-save kernel counts with the SWAR multiply, not with POPCNT",
     "tail"),
    ("fused-and-on-load", CSA_ASM, 322, "vpand ymm13, ymm9, ymmword ptr [rdi + r9]",
     "the fused kernel ANDs the second operand as it loads the first, so no temporary buffer "
     "exists", "fused"),
    ("popcnt-words-symbol", CSA_ASM, 577, "gf2_kernels_simd::x86::popcount::popcnt_words:",
     "the scalar POPCNT kernel", "no-win-region"),
    ("popcnt-words-loop", CSA_ASM, 584, "popcnt rdx, qword ptr [rdi + rcx]",
     "one POPCNT per word into one accumulator: five instructions per word with a single "
     "dependency chain and no unrolling, which is what loses at four words", "dependency-chain"),
    # Emitted instructions: the established per-vector lookup.
    ("lut-vector-loop", LUT_ASM, 177, ".LBB66_9:",
     "the per-vector loop of avx2_popcnt", "dependency-chain"),
    ("lut-accumulator", LUT_ASM, 186, "vpaddq ymm0, ymm4, ymm0",
     "the per-vector loop accumulates into one register per vector, so its lookup and its "
     "accumulator chain both run once per 32 bytes", "dependency-chain"),
    ("lut-word-tail", LUT_ASM, 207, ".LBB66_6:",
     "the established kernel's word tail, the SWAR count it shares with the carry-save kernel",
     "tail"),
]


def main():
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()
    claims = []
    for claim_id, path, line, fragment, why, topic in CLAIMS:
        text = pathlib.Path(path).read_text(encoding="utf-8").splitlines()[line - 1]
        if fragment not in text:
            raise SystemExit(f"{path}:{line} does not contain {fragment!r}: {text!r}")
        if any(claim["id"] == claim_id for claim in claims):
            raise SystemExit(f"claim id {claim_id!r} is not unique")
        claims.append({"id": claim_id, "path": path, "line": line, "text": text.strip(),
                       "topic": topic, "why": why})
    json.dump(
        {
            "schema": "count-optimization-source-evidence-v1",
            "commit": revision,
            "note": (
                "The commit is navigation metadata; the producing manifests of the campaigns pin "
                "the source bytes the receipts measured. Lines under "
                "crates/gf2-kernels-simd/src/x86/asm/ are the committed artefacts "
                "dev/scripts/regen-asm.sh writes from the compiler, and each artefact's header "
                "names the revision and toolchain it was recorded from."
            ),
            "claims": claims,
        },
        fp=sys.stdout,
        indent=2,
    )
    print()


if __name__ == "__main__":
    main()
