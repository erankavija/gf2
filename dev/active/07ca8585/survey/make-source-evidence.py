#!/usr/bin/env python3
"""Write this issue's code claims with verbatim source lines (jit:07ca8585).

Each claim names the project, the file and line, a fragment the line must
contain, and why the line matters for the change. The script reads the line
from the working tree, refuses a claim whose line lacks its fragment, and
records the verbatim text with the revision the tree is at, so every mechanism
the findings cite can be checked at its location.

Two generations of claims live here. `before` claims pin the update path this
issue replaces and are read from the `--before-rev` revision; `after` claims
pin the replacement and are read from the working tree. A claim is written
only when its generation is selected, so the file regenerates byte for byte
from a tree that carries both revisions.

Usage: make-source-evidence.py --before-rev REV [--generation before|after|both]
"""

import argparse
import json
import subprocess

CORE = "crates/gf2-coding/src/ldpc/core.rs"
LLR = "crates/gf2-coding/src/llr.rs"
SIMD = "crates/gf2-kernels-simd/src/llr.rs"
SPARSE = "crates/gf2-core/src/sparse.rs"
LAYOUT = "crates/gf2-coding/src/ldpc/edge_layout.rs"
MINSUM = "crates/gf2-coding/src/ldpc/min_sum.rs"

# (generation, path, line, fragment, why, lever)
CLAIMS = [
    ("before", CORE, 1173, "for (pos, &_var) in neighbors.iter().enumerate()",
     "the min-sum check update computes one output position at a time", "reduction"),
    ("before", CORE, 1177, "for (other_pos, &other_var) in neighbors.iter().enumerate()",
     "each output re-gathers the other d_c - 1 inputs, so a check costs d_c (d_c - 1) gathers",
     "reduction"),
    ("before", CORE, 1179, "let var_check_pos = self.find_check_position(other_var, check);",
     "every gathered input is located by a search of the variable's check list", "edge-indexing"),
    ("before", CORE, 1189, "Llr::boxplus_minsum_n(&self.temp_inputs)",
     "one reduction per output edge instead of a shared minimum, second minimum and sign per check",
     "reduction"),
    ("before", CORE, 1293, ".position(|&check| check == target_check)",
     "find_check_position is a linear scan over the variable's checks", "edge-indexing"),
    ("before", CORE, 1302, ".position(|&v| v == var)",
     "check_to_var_message scans the check's variable list; the variable update calls it twice per edge",
     "edge-indexing"),
    ("before", CORE, 1275, "belief = Llr::new(belief.value() + self.check_to_var_message(var, pos).value());",
     "the variable-node sum reads check-to-variable messages through that search", "edge-indexing"),
    ("before", CORE, 913, "check_to_var: Vec<Vec<Llr>>,",
     "one heap vector per check: a jagged, pointer-chased message layout", "layout"),
    ("before", CORE, 915, "var_to_check: Vec<Vec<Llr>>,",
     "one heap vector per variable", "layout"),
    ("before", CORE, 917, "check_neighbors: Vec<Vec<usize>>,",
     "eight-byte neighbour indices in per-node vectors", "layout"),
    ("before", CORE, 1372, "fn hard_decode(&self) -> BitVec {",
     "every syndrome check builds a fresh hard-decision vector", "allocation"),
    ("before", CORE, 141, "let syndrome = self.syndrome(codeword);",
     "the syndrome is materialised in a new vector before its ones are counted", "allocation"),
    ("before", SPARSE, 1369, "let mut y = BitVec::with_capacity(self.rows);",
     "the syndrome matvec allocates its output", "allocation"),
    ("before", LLR, 392, "let f32_vals: Vec<f32> = llrs.iter().map(|l| l.0).collect();",
     "a heap allocation and copy per min-sum call, that is per edge per iteration", "allocation"),
    ("before", LLR, 393, "let result = (fns.minsum_fn)(&f32_vals);",
     "an indirect call per edge per iteration", "dispatch"),
    ("before", LLR, 406, "if llr.0 >= 0.0 { 1.0 } else { -1.0 }",
     "the supported scalar reference's sign rule: negative zero counts as positive", "numerical-contract"),
    ("before", LLR, 412, ".fold(f32::INFINITY, f32::min);",
     "the supported scalar reference's magnitude rule: f32::min skips a NaN input", "numerical-contract"),
    ("before", SIMD, 150, "vec_sign = _mm256_xor_ps(vec_sign, signs);",
     "the AVX2 kernel's vector lanes take the IEEE sign bit, so negative zero counts as negative there",
     "numerical-contract"),
    ("before", SIMD, 174, "if val < 0.0 {",
     "the AVX2 kernel's scalar tail counts negative zero as positive, so the two halves of one kernel disagree",
     "numerical-contract"),
    ("before", SIMD, 139, "let chunks = n / 8;",
     "only whole groups of eight inputs reach the vector lanes, so the disagreement needs nine or more inputs",
     "numerical-contract"),
    ("after", LAYOUT, None, "pub struct EdgeLayout {",
     "the one canonical precomputed edge indexing both node updates and the syndrome read", "edge-indexing"),
    ("after", LAYOUT, None, "var_edge_to_check_edge: Vec<u32>,",
     "a variable's edges resolve to canonical edge ids without a neighbour search", "edge-indexing"),
    ("after", MINSUM, None, "pub(crate) fn min_sum_check_row(",
     "one shared minimum, second-minimum and sign reduction writes every output of a check",
     "reduction"),
    ("after", MINSUM, None, "if magnitude < min1 {",
     "pass one tracks the two smallest magnitudes and the position of the smallest", "reduction"),
    ("after", MINSUM, None, "let magnitude = if index == arg1 { min2 } else { min1 };",
     "pass two takes each output's excluded minimum from the shared reduction", "reduction"),
    ("after", MINSUM, None, "if value >= 0.0 { 1.0 } else { -1.0 }",
     "the canonical reduction keeps the supported scalar reference's sign rule", "numerical-contract"),
    ("after", CORE, None, "check_to_var: Vec<Llr>,",
     "one flat check-major message array replaces the jagged per-check vectors", "layout"),
    ("after", CORE, None, "fn syndrome_passes(&self) -> bool {",
     "the syndrome is checked over the canonical layout without a hard-decision vector", "allocation"),
]


def read_line(text, path, line, fragment):
    lines = text.splitlines()
    if line is None:
        matches = [i for i, l in enumerate(lines, 1) if fragment in l]
        if len(matches) != 1:
            raise SystemExit(f"{path}: {len(matches)} lines contain {fragment!r}, want exactly one")
        line = matches[0]
    if line > len(lines):
        raise SystemExit(f"{path}:{line} beyond end of file")
    text_line = lines[line - 1].strip()
    if fragment not in text_line:
        raise SystemExit(f"{path}:{line} is {text_line!r}, which lacks {fragment!r}")
    return line, text_line


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--before-rev", required=True)
    parser.add_argument("--generation", default="both", choices=["before", "after", "both"])
    args = parser.parse_args()

    wanted = {"before", "after"} if args.generation == "both" else {args.generation}
    head = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True,
                          check=True).stdout.strip()
    before = subprocess.run(["git", "rev-parse", args.before_rev], capture_output=True, text=True,
                            check=True).stdout.strip()

    cache = {}

    def source(generation, path):
        key = (generation, path)
        if key not in cache:
            if generation == "before":
                cache[key] = subprocess.run(["git", "show", f"{before}:{path}"],
                                            capture_output=True, text=True, check=True).stdout
            else:
                cache[key] = open(path, encoding="utf-8").read()
        return cache[key]

    claims = []
    for generation, path, line, fragment, why, lever in CLAIMS:
        if generation not in wanted:
            continue
        resolved, text = read_line(source(generation, path), path, line, fragment)
        claims.append({
            "generation": generation,
            "project": "gf2",
            "commit": before if generation == "before" else head,
            "path": path,
            "line": resolved,
            "text": text,
            "lever": lever,
            "why": why,
        })

    print(json.dumps({
        "schema": "ldpc-update-source-evidence-v1",
        "note": ("`before` claims are read from the revision named on the command line, the "
                 "update path this issue replaces; `after` claims are read from the working tree. "
                 "Revisions are navigation metadata: the receipts pin the measured bytes."),
        "claims": claims,
    }, indent=2))


if __name__ == "__main__":
    main()
