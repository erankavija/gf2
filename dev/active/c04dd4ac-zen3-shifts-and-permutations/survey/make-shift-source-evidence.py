#!/usr/bin/env python3
"""Freeze the source-derived path and dispatch evidence for jit:85fc5ff4.

Every code claim the residual-shift profile makes is pinned here by project,
commit, path, line, verbatim line and reason, so the report states mechanisms
without carrying line numbers that drift. A required fragment that is absent
fails the run rather than producing a stale ledger.
"""

import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[4]
OUTPUT = pathlib.Path(
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/shift-source-evidence.json"
)
PROJECT = "gf2"

BITVEC = pathlib.Path("crates/gf2-core/src/bitvec.rs")
BENCH = pathlib.Path("crates/gf2-core/benches/shifts.rs")

CLAIMS = [
    (BITVEC, "pub fn shift_left(&mut self, k: usize) {",
     "the public in-place zero-fill left shift the family measures"),
    (BITVEC, "pub fn shift_right(&mut self, k: usize) {",
     "the public in-place zero-fill right shift the family measures"),
    (BITVEC, "let bit_shift = k % 64;",
     "the residual offset that selects between the word-aligned and residual paths"),
    (BITVEC, "(fns.shift_left_words_fn)(&mut self.data, word_shift);",
     "the word-aligned left control routes to the detected SIMD word-shift backend"),
    (BITVEC, "(fns.shift_right_words_fn)(&mut self.data, word_shift);",
     "the word-aligned right control routes to the detected SIMD word-shift backend"),
    (BITVEC, "if let Some(fns) = crate::simd::maybe_simd() {",
     "the word-aligned path takes the backend only when runtime detection supplies it"),
    (BITVEC, "#[cfg(feature = \"simd\")]",
     "the backend exists only under the non-default `simd` cargo feature of gf2-core"),
    (BITVEC, "self.data[i] = (self.data[i - word_shift] << bit_shift)",
     "the residual left path is a safe scalar double-word funnel over one word per step"),
    (BITVEC, "| (self.data[i - word_shift - 1] >> inv_shift);",
     "the residual left funnel's carry term reads the neighbouring word"),
    (BITVEC, "self.data[i] = (self.data[i + word_shift] >> bit_shift)",
     "the residual right path is the mirrored safe scalar funnel"),
    (BITVEC, "| (self.data[i + word_shift + 1] << inv_shift);",
     "the residual right funnel's carry term reads the neighbouring word"),
    (BITVEC, "let inv_shift = 64 - bit_shift;",
     "both residual paths derive the complementary shift count per call, not per word"),
    (BITVEC, "self.mask_tail();",
     "every path restores the zero tail padding the semantic oracle checks"),
    (BENCH, "Self::ResidualProduction => case.residual_offset,",
     "the baseline arm times the cell's declared residual offset"),
    (BENCH, "Self::WordAlignedControl => case.control_offset,",
     "the candidate arm times the cell's declared word-aligned control offset"),
    (BENCH, "(Self::ResidualProduction, Direction::Left) => \"bitvec-residual-scalar-left\",",
     "the arm labels the path each receipt execution records as `selected_path`"),
]


def file_record(path):
    text = (ROOT / path).read_text()
    commit = subprocess.run(
        ["git", "log", "-1", "--format=%H", "--", str(path)],
        cwd=ROOT,
        capture_output=True,
        check=True,
        text=True,
    ).stdout.strip()
    return text.splitlines(), {
        "project": PROJECT,
        "path": str(path),
        "commit": commit,
        "sha256": hashlib.sha256(text.encode()).hexdigest(),
    }


def main():
    files = {}
    lines = {}
    for path, _, _ in CLAIMS:
        if path not in files:
            lines[path], files[path] = file_record(path)
    claims = []
    for path, fragment, why in CLAIMS:
        positions = [
            index + 1
            for index, line in enumerate(lines[path])
            if fragment in line
        ]
        if not positions:
            raise SystemExit(f"{path}: required source fragment is absent: {fragment}")
        claims.append(
            {
                "project": PROJECT,
                "path": str(path),
                "commit": files[path]["commit"],
                "line": positions[0],
                "also_at_lines": positions[1:],
                "text": lines[path][positions[0] - 1].strip(),
                "why": why,
            }
        )
    document = {
        "schema": "bitvec-residual-shift-source-evidence-v1",
        "issue": "85fc5ff4",
        "measured_paths": {
            "residual-production": "safe scalar double-word funnel over the packed words, no SIMD and no runtime dispatch",
            "word-aligned-control": "whole-word move through the detected SIMD word-shift backend under the `simd` feature",
        },
        "files": [files[path] for path in files],
        "claims": claims,
    }
    (ROOT / OUTPUT).write_text(json.dumps(document, indent=2) + "\n")
    print(f"{OUTPUT}: {len(claims)} claims over {len(files)} source pins")


if __name__ == "__main__":
    main()
