#!/usr/bin/env python3
"""Write the survey's code claims with verbatim source lines (jit:5cbb6545).

Each claim names an ID the findings cite, the file, the line, a fragment the line
must contain, why the line matters for the count routes, and its topic. The script reads
each line from the working tree, refuses a claim whose line lacks its fragment,
and records the verbatim text with the revision, so every mechanism the findings
cite can be checked at its location. The asm claims cite the committed artefacts
beside the kernel sources, which the regeneration script writes from the
compiler, so an instruction claim is checked the same way as a source claim.

Usage:
  make-source-evidence.py > dev/active/5cbb6545/survey/source-evidence.json
  make-source-evidence.py --check dev/active/5cbb6545/survey/source-evidence.json
"""

import argparse
import json
import pathlib
import subprocess
import sys

# The claims describe the production source after the contract-literal route
# decision. A documentation-only commit followed that source commit, so select
# the source revision explicitly and reject generation if any claimed source
# path differs from it.
SOURCE_REVISION = "af0dacfa5aa0c2986f6dda6883f99109c4b6a4c4"

OPS = "crates/gf2-core/src/kernels/ops.rs"
BACKEND = "crates/gf2-core/src/kernels/backend.rs"
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
     "BitVec::count_ones is the unfused public entry point and uses the established dispatcher",
     "entry-point"),
    ("matvec-entry", MATRIX, 1538, "pub fn matvec(&self, x: &crate::BitVec)",
     "BitMatrix::matvec is the fused public entry point", "entry-point"),
    ("matvec-established-fused", MATRIX, 1607, "(fns.and_popcnt_fn)(row, x_words)",
     "the matrix consumer retains the bundle's established fused nibble-lookup function",
     "retained-route"),
    ("ops-scalar-retained", OPS, 387, "SCALAR_BACKEND.popcount(buf)",
     "below the bit-backend SIMD threshold the public dispatcher retains the scalar backend's "
     "portable count", "retained-route"),
    ("ops-nibble-resolver", OPS, 328, "backend.popcnt_fn",
     "the fixed-width resolver retains the bundle's established nibble-lookup function at every "
     "SIMD width", "retained-route"),
    ("ops-fused-nibble-resolver", OPS, 366, "backend.and_popcnt_fn",
     "the fused resolver retains the bundle's established nibble-lookup function at every SIMD "
     "width", "retained-route"),
    ("ops-route-report", OPS, 245, "PopcountRoute::SimdNibbleLut",
     "the observable route reports the retained SIMD implementation without a CSA boundary",
     "retained-route"),
    ("backend-simd-threshold", BACKEND, 115, "if _size >= SIMD_MIN_WORDS",
     "the bit-backend threshold that separates the scalar count from the vector kernels",
     "boundary"),
    ("tuning-retained-selector", TUNING, 557, "simd_min_words: usize",
     "the canonical bit-backend tuning family retains its established SIMD threshold and carries "
     "no unconfirmed CSA cutoff", "tuning"),
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
     "the scalar POPCNT kernel remains in the bundle as a measured comparator no resolver selects",
     "no-win-region"),
    ("bundle-csa-comparator", BUNDLE, 122, "pub popcnt_csa_fn: fn(&[u64]) -> u64",
     "the carry-save kernel remains in the bundle as preserved, tested negative evidence rather "
     "than an automatic route", "no-win-region"),
    ("bundle-fused-csa-comparator", BUNDLE, 129,
     "pub and_popcnt_csa_fn: fn(&[u64], &[u64]) -> u64",
     "the fused carry-save kernel remains a tested comparator rather than an automatic route",
     "no-win-region"),
    ("csa-block-vectors", KERNEL, 35, "pub const CSA_BLOCK_VECTORS: usize = 16",
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


def validate_source_revision():
    subprocess.run(
        ["git", "rev-parse", "--verify", f"{SOURCE_REVISION}^{{commit}}"],
        check=True,
        capture_output=True,
        text=True,
    )
    paths = sorted({path for _, path, *_ in CLAIMS})
    result = subprocess.run(
        ["git", "diff", "--quiet", SOURCE_REVISION, "--", *paths], check=False
    )
    if result.returncode == 0:
        return
    if result.returncode == 1:
        changed = subprocess.run(
            ["git", "diff", "--name-only", SOURCE_REVISION, "--", *paths],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        raise SystemExit(
            "claimed source paths differ from selected revision "
            f"{SOURCE_REVISION}:\n{changed}"
        )
    raise SystemExit(f"git diff failed with exit status {result.returncode}")


def render():
    validate_source_revision()
    claims = []
    for claim_id, path, line, fragment, why, topic in CLAIMS:
        text = pathlib.Path(path).read_text(encoding="utf-8").splitlines()[line - 1]
        if fragment not in text:
            raise SystemExit(f"{path}:{line} does not contain {fragment!r}: {text!r}")
        if any(claim["id"] == claim_id for claim in claims):
            raise SystemExit(f"claim id {claim_id!r} is not unique")
        claims.append({"id": claim_id, "path": path, "line": line, "text": text.strip(),
                       "topic": topic, "why": why})
    document = {
        "schema": "count-optimization-source-evidence-v1",
        "commit": SOURCE_REVISION,
        "note": (
            "The generator selects this source revision explicitly and refuses to emit when any "
            "claimed source path has a byte delta from it. The producing manifests of the "
            "campaigns separately pin the source bytes the receipts measured. Lines under "
            "crates/gf2-kernels-simd/src/x86/asm/ are the committed artefacts "
            "dev/scripts/regen-asm.sh writes from the compiler, and each artefact's header names "
            "the revision and toolchain it was recorded from."
        ),
        "claims": claims,
    }
    return json.dumps(document, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        type=pathlib.Path,
        metavar="ARTIFACT",
        help="verify that ARTIFACT is byte-identical to generated output",
    )
    args = parser.parse_args()
    rendered = render()
    if args.check is None:
        sys.stdout.write(rendered)
        return
    if args.check.read_bytes() != rendered.encode("utf-8"):
        raise SystemExit(f"{args.check} does not match generated source evidence")
    print(f"source evidence reproduces byte-for-byte: {args.check}")


if __name__ == "__main__":
    main()
