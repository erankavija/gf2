#!/usr/bin/env python3
"""Write `source-evidence.json`: every code claim the findings rely on.

Each claim names a project, its revision, a path, a line and the verbatim text
of that line. The script reads the source, locates the line by an exact
substring and its occurrence index, and refuses to write a claim whose text has
moved, so the committed evidence is regenerated rather than transcribed and the
report cites a claim id instead of a line number.

Usage: dev/active/1d4fd63d/survey/make-source-evidence.py  (from anywhere)
"""

import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path(
    subprocess.check_output(
        ["git", "-C", str(ROOT), "rev-parse", "--show-toplevel"], text=True
    ).strip()
)

PROJECTS = {
    "gf2": {
        "root": REPO,
        "revision": subprocess.check_output(
            ["git", "-C", str(REPO), "rev-parse", "HEAD"], text=True
        ).strip(),
        "revision_kind": (
            "git commit (informational navigation metadata; the receipts pin "
            "these files by content digest)"
        ),
    }
}

KERNELS = "crates/gf2-kernels-simd/src"
CORE = "crates/gf2-core/src"
CODING = "crates/gf2-coding/src"
ARM = "dev/active/1d4fd63d/arms/src/main.rs"
ASM = f"{KERNELS}/x86/asm/transpose.asm.txt"

# (claim id, project, path, exact substring, occurrence index, why it matters)
CLAIMS = [
    # The lane family and its one dispatch.
    (
        "lane-enumeration",
        "gf2",
        f"{KERNELS}/transpose.rs",
        "pub enum TransposeLane {",
        0,
        "Every implementation of the 64x64 block contract is a variant here, so "
        "the family the campaign measures is the family the library publishes.",
    ),
    (
        "lane-entry-point",
        "gf2",
        f"{KERNELS}/transpose.rs",
        "pub fn lane(lane: TransposeLane) -> Option<Transpose64x64Fn> {",
        0,
        "The one way to reach a specific lane. A candidate arm names its lane "
        "here rather than through a private entry point of its own.",
    ),
    (
        "lane-feature-gate",
        "gf2",
        f"{KERNELS}/transpose.rs",
        'if !is_x86_feature_detected!("avx2") {',
        0,
        "An AVX2 lane is published only after the runtime feature test, which is "
        "the whole safety contract of each unsafe kernel.",
    ),
    (
        "production-preference",
        "gf2",
        f"{KERNELS}/transpose.rs",
        "pub const PRODUCTION_PREFERENCE: [TransposeLane; 2] =",
        0,
        "The order `detect` resolves the production lane in. Adopting a lane is "
        "an edit to this constant and to nothing else.",
    ),
    (
        "detect-resolves-preference",
        "gf2",
        f"{KERNELS}/transpose.rs",
        "pub fn detect() -> Option<TransposeFns> {",
        0,
        "The dispatch every production consumer reaches; the campaign's baseline "
        "arm calls it.",
    ),
    # The candidate kernels.
    (
        "ymm6-kernel",
        "gf2",
        f"{KERNELS}/x86/transpose.rs",
        "pub(crate) unsafe fn transpose_64x64_avx2_ymm6(",
        0,
        "The candidate that runs all six stages in YMM registers and declares no "
        "stack scratch.",
    ),
    (
        "movemask-kernel",
        "gf2",
        f"{KERNELS}/x86/transpose.rs",
        "pub(crate) unsafe fn transpose_64x64_avx2_movemask(",
        0,
        "The movemask candidate. Its byte transpose is the tile assembly and its "
        "bit-plane extraction the packing that the 32x8 geometry needs to answer "
        "the same 64x64 contract, so both are inside every cell that times it.",
    ),
    (
        "pshufb-kernel",
        "gf2",
        f"{KERNELS}/x86/transpose.rs",
        "pub(crate) unsafe fn transpose_64x64_avx2_pshufb(",
        0,
        "The PSHUFB candidate the issue names.",
    ),
    (
        "incumbent-declares-scratch",
        "gf2",
        f"{KERNELS}/x86/transpose.rs",
        "let mut buf: [u64; 64] = *input;",
        0,
        "The incumbent lane's declared 512-byte local. Its frame traffic is an "
        "intentional scratch, not a compiler spill.",
    ),
    (
        "movemask-declares-planes",
        "gf2",
        f"{KERNELS}/x86/transpose.rs",
        "let mut planes = [[0u8; 64]; 8];",
        0,
        "The movemask lane's declared byte-plane array, the second intentional "
        "scratch of the family.",
    ),
    # Frame traffic, from the committed artefact.
    (
        "asm-incumbent-block-copy",
        "gf2",
        ASM,
        "; block copies (memcpy)                   2",
        0,
        "The incumbent lane copies the block into its declared local and back "
        "out; the annotation counts both calls from the disassembly itself.",
    ),
    (
        "asm-ymm6-no-block-copy",
        "gf2",
        ASM,
        "; block copies (memcpy)                   0",
        0,
        "The candidate copies no block. Its remaining frame traffic belongs to no "
        "declared buffer, so it is register pressure rather than scratch.",
    ),
    # The consumers the lane reaches.
    (
        "matrix-transpose-driver",
        "gf2",
        f"{CORE}/matrix.rs",
        "pub fn transpose_with_block_kernel(",
        0,
        "The tiling driver of `BitMatrix::transpose` with its block primitive as "
        "an argument. The whole-matrix cells drive the production tiling, output "
        "allocation, zero padding and tail mask through it.",
    ),
    (
        "matrix-transpose-delegates",
        "gf2",
        f"{CORE}/matrix.rs",
        "self.transpose_with_block_kernel(Self::resolved_block_kernel())",
        0,
        "`BitMatrix::transpose` is that driver at the resolved kernel, so a cell "
        "and the production entry point run the same code.",
    ),
    (
        "bch-bundle-transpose-field",
        "gf2",
        f"{KERNELS}/bch_encode.rs",
        "pub transpose_lane_block: crate::transpose::Transpose64x64Fn,",
        0,
        "The bit-sliced BCH bundle reaches the transpose through this field, "
        "which is what a conversion cell substitutes a lane into.",
    ),
    (
        "bch-bundle-uses-dispatch",
        "gf2",
        f"{KERNELS}/bch_encode.rs",
        "let transpose = crate::transpose::detect()?;",
        0,
        "The accelerated bundle takes the production dispatch's lane rather than "
        "a second wrapper around the same kernel.",
    ),
    (
        "absorb-block-transposes",
        "gf2",
        f"{KERNELS}/bch_encode.rs",
        "(self.transpose_lane_block)(windows, &mut slices);",
        0,
        "The packing half of the conversion: one block transpose bit-slices a "
        "lane group's message window before the recurrence consumes it.",
    ),
    (
        "unpack-parity-transposes",
        "gf2",
        f"{KERNELS}/bch_encode.rs",
        "(self.transpose_lane_block)(&block, &mut lanes);",
        0,
        "The unpacking half: one block transpose per parity word reads the "
        "reduced register back as packed per-frame parity.",
    ),
    (
        "bitslice-family-admission",
        "gf2",
        f"{CODING}/bch/encode.rs",
        "batch_len >= selectors.bitslice_interleaved_min_batch()",
        0,
        "The bit-sliced family is admitted by the active profile's minimum batch. "
        "The whole-consumer control calls the family entry point directly, so it "
        "measures that family whatever the profile admits.",
    ),
    (
        "bitslice-min-batch-conservative",
        "gf2",
        f"{CODING}/bch/encode.rs",
        "pub const BITSLICE_INTERLEAVED_MIN_BATCH: usize = usize::MAX;",
        0,
        "Under the conservative profile no batch length selects the bit-sliced "
        "family, so the consumer the conversion sits inside is reached by "
        "installing a profile or by naming the family, not by the default walk.",
    ),
    # The arm.
    (
        "arm-selects-lane-by-environment",
        "gf2",
        ARM,
        'let requested = std::env::var("GF2_TRANSPOSE_LANE")',
        0,
        "One executable serves every arm; the lane is the only difference between "
        "a baseline and a candidate child.",
    ),
    (
        "arm-baseline-is-production-dispatch",
        "gf2",
        ARM,
        'if requested == "production" {',
        0,
        "The baseline arm runs whatever `detect` publishes, so it is the pinned "
        "pre-change implementation rather than a lane named to imitate it.",
    ),
    (
        "arm-warm-pass",
        "gf2",
        ARM,
        "for bank in 0..banks {",
        0,
        "The declared warm policy: one untimed pass of the timed body over every "
        "bank, before calibration, in both arms.",
    ),
    (
        "arm-control-reports-dispatched-lane",
        "gf2",
        ARM,
        "Case::BchEncodeBitslice { .. } => format!(",
        0,
        "The whole-consumer control reports the dispatched lane whatever the arm "
        "names, which is what makes it an identity control.",
    ),
]


def main():
    claims = {}
    for claim_id, project, path, needle, index, why in CLAIMS:
        source = PROJECTS[project]["root"] / path
        text = source.read_text().splitlines()
        hits = [number for number, line in enumerate(text, 1) if needle in line]
        if len(hits) <= index:
            raise SystemExit(
                f"{claim_id}: {needle!r} occurs {len(hits)} times in {path}, "
                f"needed occurrence {index}"
            )
        line_number = hits[index]
        claims[claim_id] = {
            "project": project,
            "path": path,
            "line": line_number,
            "text": text[line_number - 1].rstrip(),
            "occurrence": index,
            "why": why,
        }
    document = {
        "schema": "transpose-lane-source-evidence-v1",
        "issue": "1d4fd63d",
        "projects": {
            name: {k: (str(v) if k == "root" else v) for k, v in fields.items() if k != "root"}
            for name, fields in PROJECTS.items()
        },
        "claims": claims,
    }
    output = REPO / "dev/active/1d4fd63d/survey/source-evidence.json"
    with output.open("w") as handle:
        json.dump(document, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    print(f"{len(claims)} claims -> {output}")


if __name__ == "__main__":
    main()
