#!/usr/bin/env python3
"""Write the survey's code claims with verbatim source lines (jit:3be770d5).

Each claim names a project, the file and line, a fragment the line must
contain, and why the line matters for the lever ranking. The script reads the
line from the pinned source, refuses a claim whose line lacks its fragment,
and records the verbatim text with the source revision, so every mechanism the
findings cite can be checked at its location.

Usage: make-source-evidence.py --aff3ct DIR > source-evidence.json
"""

import argparse
import json
import pathlib
import subprocess

CORE = "crates/gf2-coding/src/ldpc/core.rs"
LLR = "crates/gf2-coding/src/llr.rs"
SIMD = "crates/gf2-kernels-simd/src/llr.rs"
SPARSE = "crates/gf2-core/src/sparse.rs"
SWEEP = "crates/gf2-sim/src/bin/ldpc_bler_sweep.rs"
FLOOD = "include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx"
MS = "include/Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx"
SYNDROME = "include/Tools/Code/LDPC/Syndrome/LDPC_syndrome.hxx"
INTER = "src/Module/Decoder/LDPC/BP/Horizontal_layered/ONMS/Decoder_LDPC_BP_horizontal_layered_ONMS_inter.cpp"

CLAIMS = [
    ("gf2", CORE, 1202, "for (pos, &_var) in neighbors.iter().enumerate()",
     "check-node update: one output position at a time", "reduction"),
    ("gf2", CORE, 1205, "for (other_pos, &other_var) in neighbors.iter().enumerate()",
     "each output re-gathers the other d_c - 1 inputs, so a check costs d_c (d_c - 1) gathers", "reduction"),
    ("gf2", CORE, 1207, "let var_check_pos = self.find_check_position(other_var, check);",
     "every gathered input is located by a search of the variable's check list", "edge-indexing"),
    ("gf2", CORE, 1216, "Llr::boxplus_normalized_minsum_n(&self.temp_inputs, alpha)",
     "one reduction per output edge instead of a shared minimum, second minimum and sign per check", "reduction"),
    ("gf2", CORE, 1293, ".position(|&check| check == target_check)",
     "find_check_position is a linear scan", "edge-indexing"),
    ("gf2", CORE, 1302, ".position(|&v| v == var)",
     "check_to_var_message scans the check's variable list; the variable update calls it twice per edge",
     "edge-indexing"),
    ("gf2", CORE, 1275, "belief = Llr::new(belief.value() + self.check_to_var_message(var, pos).value());",
     "variable-node sum reads check-to-variable messages through the search", "edge-indexing"),
    ("gf2", LLR, 392, "let f32_vals: Vec<f32> = llrs.iter().map(|l| l.0).collect();",
     "a heap allocation and copy per min-sum call, that is per edge per iteration", "allocation"),
    ("gf2", LLR, 393, "let result = (fns.minsum_fn)(&f32_vals);",
     "an indirect call per edge per iteration", "dispatch"),
    ("gf2", LLR, 387, "static SIMD_FNS: Lazy<Option<gf2_kernels_simd::llr::LlrFns>> =",
     "the kernel table is read through a lazy static on every call", "dispatch"),
    ("gf2", SIMD, 139, "let chunks = n / 8;",
     "the AVX2 kernel vectorizes only whole groups of eight inputs", "dispatch"),
    ("gf2", SIMD, 172, "for &val in &inputs[chunks * 8..] {",
     "degree-6 and degree-7 DVB checks (5 or 6 inputs) and most NR checks run entirely in the scalar tail",
     "degree-structure"),
    ("gf2", CORE, 913, "check_to_var: Vec<Vec<Llr>>,",
     "one heap vector per check: jagged, pointer-chased message layout", "layout"),
    ("gf2", CORE, 915, "var_to_check: Vec<Vec<Llr>>,",
     "one heap vector per variable", "layout"),
    ("gf2", CORE, 917, "check_neighbors: Vec<Vec<usize>>,",
     "eight-byte neighbor indices in per-node vectors", "layout"),
    ("gf2", CORE, 1352, "let decoded = self.hard_decode();",
     "every iteration builds a new hard-decision vector before the syndrome check", "syndrome"),
    ("gf2", CORE, 1353, "if self.code.is_valid_codeword(&decoded) {",
     "every iteration computes the full syndrome", "syndrome"),
    ("gf2", CORE, 1375, "decoded.push_bit(belief.hard_decision());",
     "hard decisions are appended one bit at a time", "syndrome"),
    ("gf2", CORE, 141, "let syndrome = self.syndrome(codeword);",
     "the syndrome is materialized in a new vector before counting its ones", "syndrome"),
    ("gf2", SPARSE, 1369, "let mut y = BitVec::with_capacity(self.rows);",
     "the syndrome matvec allocates its output", "allocation"),
    ("gf2", CORE, 1337, "for (var, &llr) in llrs.iter().enumerate().take(self.code.n()) {",
     "per-frame message initialization walks the jagged variable vectors", "layout"),
    ("gf2", SWEEP, 230, "let mut dec = LdpcDecoder::with_config(code.clone(), cfg);",
     "gf2-sim's multi-worker consumer builds one decoder per worker chunk, the model the gf2 arm pins", "dispatch"),
    ("aff3ct", FLOOD, 247, "this->up_rule.compute_chk_node_in(v, msg_var_to_chk[transpose_ptr[v]]);",
     "first pass over a check's inputs through a precomputed edge index", "edge-indexing"),
    ("aff3ct", FLOOD, 252, "msg_chk_to_var[transpose_ptr[v]] = this->up_rule.compute_chk_node_out(v, msg_var_to_chk[transpose_ptr[v]]);",
     "second pass writes every output from the shared reduction: 2 d_c reads per check", "reduction"),
    ("aff3ct", FLOOD, 31, "msg_chk_to_var(this->n_frames, std::vector<R>(this->H.get_n_connections()))",
     "one flat message array per frame", "layout"),
    ("aff3ct", FLOOD, 225, "msg_var_to_chk_ptr[c] = tmp - msg_chk_to_var_ptr[c];",
     "variable-major sequential variable update", "layout"),
    ("aff3ct", FLOOD, 192, "valid_synd = this->template check_syndrome_soft<R, Syndrome_checker>(this->post.data());",
     "the syndrome is checked on the soft posteriors without a hard-decision vector", "syndrome"),
    ("aff3ct", SYNDROME, 51, "while (c < n_chk_nodes && !syndrome)",
     "the soft syndrome stops at the first unsatisfied check", "syndrome"),
    ("aff3ct", MS, 65, "this->min2 = std::min(this->min2, std::max(var_abs, this->min1));",
     "shared minimum and second-minimum tracking", "reduction"),
    ("aff3ct", MS, 62, "const auto var_sign = std::signbit((float)var_val) ? -1 : 0;",
     "AFF3CT's sign treats negative zero as negative; gf2's scalar min-sum treats it as positive, a contract detail a shared reduction must preserve",
     "numerical-contract"),
    ("gf2", LLR, 406, ".map(|llr| if llr.0 >= 0.0 { 1.0 } else { -1.0 })",
     "gf2's scalar min-sum sign: negative zero counts as positive", "numerical-contract"),
    ("gf2", SIMD, 149, "let signs = _mm256_and_ps(vals, sign_mask);",
     "the AVX2 kernel's vector lanes take the sign bit, so negative zero counts as negative there",
     "numerical-contract"),
    ("gf2", SIMD, 174, "if val < 0.0 {",
     "the AVX2 kernel's scalar tail counts negative zero as positive: the two parts of one kernel "
     "disagree on signed zero, a contract detail a shared reduction must settle", "numerical-contract"),
    ("aff3ct", MS, 88, "const auto res_abs = ((var_abs == this->min1) ? this->cst1 : this->cst2);",
     "outputs select the second minimum by magnitude equality with the first", "numerical-contract"),
    ("aff3ct", FLOOD, 80, "auto m = new Decoder_LDPC_BP_flooding(*this);",
     "AFF3CT replicates a decoder by copy, the route the AFF3CT arm uses per worker", "dispatch"),
    ("aff3ct", FLOOD, 54, "for (auto ii = 0; ii < (int)var_id; ii++)",
     "the constructor computes each edge's index with a scan over earlier variables, which makes construction (setup) expensive on DVB",
     "setup"),
    ("aff3ct", INTER, 40, "this->set_n_frames_per_wave(mipp::N<R>());",
     "the INTER decoder decodes one frame per SIMD lane: eight f32 or sixteen i16 frames per wave on AVX2",
     "inter-frame"),
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--aff3ct", required=True, type=pathlib.Path)
    args = parser.parse_args()
    roots = {"gf2": pathlib.Path("."), "aff3ct": args.aff3ct}
    revisions = {
        "gf2": subprocess.run(["git", "rev-parse", "HEAD"], check=True, capture_output=True,
                              text=True).stdout.strip(),
        "aff3ct": subprocess.run(["git", "-C", str(args.aff3ct), "rev-parse", "HEAD"], check=True,
                                 capture_output=True, text=True).stdout.strip(),
    }
    claims = []
    for project, path, line, fragment, why, lever in CLAIMS:
        text = (roots[project] / path).read_text(encoding="utf-8").splitlines()[line - 1]
        if fragment not in text:
            raise SystemExit(f"{project} {path}:{line} does not contain {fragment!r}: {text!r}")
        claims.append({"project": project, "commit": revisions[project], "path": path, "line": line,
                       "text": text.strip(), "lever": lever, "why": why})
    json.dump({"schema": "ldpc-throughput-source-evidence-v1",
               "note": ("gf2 revisions are navigation metadata; the producing manifests of the "
                        "campaigns pin the gf2 source bytes. AFF3CT is the pinned v4.7.0 checkout "
                        "whose source archive c077a88b committed."),
               "claims": claims}, fp=__import__("sys").stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
