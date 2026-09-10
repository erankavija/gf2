#!/usr/bin/env python3
"""Emit source-backed evidence for the eda07788 comparator survey.

Usage: inspect-sources.py --ext <staging-dir> --output <source-evidence.json>

Every claim names a project, a path and a needle. The recorded line is the
first line containing the needle after the optional `after` line, and the
record carries that line's verbatim text. External projects must be clean
checkouts at their pinned commits. gf2 claims are read from this repository's
committed HEAD; the file must equal its HEAD blob, and the record adds the
file's SHA-256.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys


PINS = {
    "aff3ct": "e8a65c5047262d97a15563b9edc961f69b2792cc",
    "aff3ct-conf": "ecae10cd0375f12febe2f5513f8ca39f2540c640",
    "srsran": "d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc",
    "xdsopl-ldpc": "32357d8ad55a6a302c34e093759f0454e45cca56",
}

BG = "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp"
PUNCT = "src/Module/Puncturer/LDPC/Puncturer_5G.cpp"
QC = "src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC.cpp"
QC_FAST = "src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.cpp"
NR = "crates/gf2-coding/src/ldpc/nr_5g/mod.rs"
DVB = "crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs"
LDPC_CORE = "crates/gf2-coding/src/ldpc/core.rs"
RM = "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp"
SURVEY = "dev/active/eda07788/survey/"

# claim, project, path, needle, interpretation[, after-line]
CLAIMS = [
    ("xdsopl-license", "xdsopl-ldpc", "LICENSE", "Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted.", "xdsopl/LDPC carries the zero-clause BSD grant."),
    ("aff3ct-license", "aff3ct", "LICENSE", "MIT License", "AFF3CT is MIT-licensed."),
    ("srsran-license-grant", "srsran", RM, "the License, or (at your option) any later version.", "The surveyed srsRAN files grant AGPL version 3 or any later version."),
    ("xdsopl-dvb-t2-standard", "xdsopl-ldpc", "README.md", "en_302755v010401p.pdf", "The project names ETSI EN 302 755 v1.4.1."),
    ("xdsopl-dvb-t2-table-a1", "xdsopl-ldpc", "tables_handler.cc", "return new LDPC<DVB_T2_TABLE_A1>();", "The program selects the rate-1/2 DVB-T2 table."),
    ("xdsopl-pitl-definition", "xdsopl-ldpc", "interleaver.hh", "struct PITL\n", "PITL is the parity-interleaving stage."),
    ("xdsopl-pitl-fwd", "xdsopl-ldpc", "interleaver.hh", "out[K+M*q+m] = in[K+Q*m+q];", "Parity interleaving writes out[K+360q+m] from in[K+Qm+q], gf2's parity permutation."),
    ("xdsopl-pctitl-definition", "xdsopl-ldpc", "interleaver.hh", "struct PCTITL", "PCTITL composes parity interleaving with column twisting."),
    ("xdsopl-pctitl-overwrites-input", "xdsopl-ldpc", "interleaver.hh", "in[n] = out[n];", "PCTITL::fwd copies the parity stage back into its input buffer, so the adapter passes a scratch copy."),
    ("xdsopl-pctitl-column-twist", "xdsopl-ldpc", "interleaver.hh", "CT::fwd(out+COLS*row, in, ROWS, row);", "The second stage fills each output row through the column-twist functor."),
    ("xdsopl-ct8-definition", "xdsopl-ldpc", "interleaver.hh", "struct CT8", "CT8 supplies the eight-column twist."),
    ("xdsopl-ct8-fwd", "xdsopl-ldpc", "interleaver.hh", "out[0] = in[0*S+(R+S-T0)%S];", "Column c of row R reads in[c*Nr + (R+Nr-tc) mod Nr], gf2's inverse column twist."),
    ("xdsopl-ct12-definition", "xdsopl-ldpc", "interleaver.hh", "struct CT12", "CT12 supplies the twelve-column twist."),
    ("xdsopl-normal-r12-instantiation", "xdsopl-ldpc", "itls_handler.cc", "PCTITL<code_type, 64800, 90, CT>", "The program instantiates normal rate-1/2 PCTITL inside its BITL demux wrapper."),
    ("xdsopl-short-r12-instantiation", "xdsopl-ldpc", "itls_handler.cc", "PCTITL<code_type, 16200, 25, CT>", "The program instantiates short rate-1/2 PCTITL inside its BITL demux wrapper."),
    ("aff3ct-bg-selection", "aff3ct", BG, "if (K <= 292 ||", "AFF3CT selects the 5G base graph from K and the rate K/N."),
    ("aff3ct-bg2-columns", "aff3ct", BG, "base_graph.K_LDPC = 10;", "BG2 starts from its 10 systematic base columns."),
    ("aff3ct-bg1-columns", "aff3ct", BG, "base_graph.K_LDPC = 22;", "BG1 starts from its 22 systematic base columns."),
    ("aff3ct-kb-560-640", "aff3ct", BG, "else if (K > 560 && K <= 640)", "The lifting table has a separate branch for 560 < K <= 640."),
    ("aff3ct-kb-560-640-value", "aff3ct", BG, "Kb = 8;", "That branch assigns Kb = 8, where gf2's table uses 9."),
    ("aff3ct-lifting-uses-kb", "aff3ct", BG, "int curr_k = Kb * val_z;", "Kb enters only the lifting-size search."),
    ("aff3ct-mother-dimensions", "aff3ct", BG, "base_graph.K_LDPC = base_graph.K_LDPC * base_graph.Zc;", "K_LDPC is the base-graph systematic column count times Zc, not Kb times Zc."),
    ("aff3ct-mother-length", "aff3ct", BG, "base_graph.N_LDPC = base_graph.N_LDPC * base_graph.Zc;", "N_LDPC is the base-graph column count times Zc."),
    ("aff3ct-puncture-entry", "aff3ct", PUNCT, "Puncturer_5G<B, Q>::_puncture", "The 5G puncturer implements bit selection in _puncture."),
    ("aff3ct-puncture-filler-skip", "aff3ct", PUNCT, "+ 2 * this->base_graph.Zc < this->base_graph.K_LDPC &&", "Selection skips the filler range [K, K_LDPC) of the circular buffer after 2Zc."),
    ("aff3ct-puncture-selection", "aff3ct", PUNCT, "X_N2[k] = X_N1[j %", "The operation copies circular-buffer positions in order until N bits are emitted."),
    ("aff3ct-depuncture-entry", "aff3ct", PUNCT, "Puncturer_5G<B, Q>::_depuncture", "The same module implements the inverse LLR mapping."),
    ("aff3ct-depuncture-zero-prefix", "aff3ct", PUNCT, "std::fill(Y_N2, Y_N2 + 2 * this->base_graph.Zc, (Q)0);", "De-puncturing writes zero into the 2Zc punctured systematic positions."),
    ("aff3ct-depuncture-filler-inf", "aff3ct", PUNCT, "std::numeric_limits<Q>::infinity());", "Floating-point filler positions receive +infinity."),
    ("aff3ct-puncturer-public-puncture", "aff3ct", "include/Module/Puncturer/Puncturer.hpp", "void puncture(const B* X_N1, B* X_N2,", "The public puncture entry point runs the module's puncture task."),
    ("aff3ct-puncturer-public-depuncture", "aff3ct", "include/Module/Puncturer/Puncturer.hpp", "void depuncture(const Q* Y_N1, Q* Y_N2,", "The public depuncture entry point runs the module's depuncture task."),
    ("aff3ct-puncture-task-dispatch", "aff3ct", "include/Module/Puncturer/Puncturer.hxx", "pct._puncture(", "The puncture task calls the protected _puncture hook."),
    ("aff3ct-puncturer-5g-hook-protected", "aff3ct", "include/Module/Puncturer/LDPC/Puncturer_5G.hpp", "void _puncture(const B* X_N1, B* X_N2, const size_t frame_id) const;", "Puncturer_5G overrides only the protected hook; callers use the public base-class entry."),
    ("aff3ct-puncturer-factory", "aff3ct", "src/Factory/Module/Puncturer/LDPC/Puncturer_LDPC.cpp", "if (this->type == \"LDPC_5G\") return new module::Puncturer_5G", "The puncturer factory constructs Puncturer_5G."),
    ("aff3ct-codec-5g-wiring", "aff3ct", "src/Factory/Tools/Codec/LDPC/Codec_LDPC.cpp", "if (enc->type == \"LDPC_5G\")", "The codec's 5G branch sets the 5G standard on encoder and decoder."),
    ("aff3ct-codec-5g-base-graph", "aff3ct", "src/Factory/Tools/Codec/LDPC/Codec_LDPC.cpp", "auto base_graph = tools::build_5G_base_graph(enc_ldpc->K, enc_ldpc->N);", "The codec derives the 5G base graph from K and N."),
    ("aff3ct-codec-5g-ncw", "aff3ct", "src/Factory/Tools/Codec/LDPC/Codec_LDPC.cpp", "enc_ldpc->N_cw = base_graph.N_LDPC;", "The codec sets the mother-codeword length the puncturer reads to N_LDPC."),
    ("aff3ct-encoder-5g-build-branch", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", "if (this->type == \"LDPC_5G\")", "The encoder factory's build method has a 5G branch.", 135),
    ("aff3ct-encoder-5g-build", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", "return new module::Encoder_LDPC_QC_fast", "The 5G build constructs Encoder_LDPC_QC_fast."),
    ("aff3ct-decoder-5g-build", "aff3ct", "src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp", "if (this->H_path.empty() && this->standard == \"5G\")", "The decoder factory derives its 5G base graph and matrix path."),
    ("aff3ct-qc-fast-subclass", "aff3ct", "include/Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.hpp", "class Encoder_LDPC_QC_fast : public Encoder_LDPC_QC<B>", "The fast QC encoder derives from the QC encoder."),
    ("aff3ct-qc-fast-encode", "aff3ct", QC_FAST, "Encoder_LDPC_QC_fast<B>::_encode", "The fast QC encoder overrides _encode."),
    ("aff3ct-qc-fast-shift-wrap", "aff3ct", QC_FAST, "parity_block[l] ^= input_ptr[start + l];", "Each nonzero circulant XORs the wrapped tail of the Zc-bit input block into the parity block."),
    ("aff3ct-qc-fast-shift-body", "aff3ct", QC_FAST, "parity_block[l] ^= input_ptr[l - shift];", "The rest of the block is XORed at offset -shift: a per-frame cyclic-shift XOR."),
    ("aff3ct-qc-rotation", "aff3ct", QC, "std::rotate(Gen.begin() + j * Zc", "The base QC encoder rotates each Zc-sized generator block; the 5G factory path does not use this _encode."),
    ("aff3ct-qc-rotation-call", "aff3ct", QC, "res = this->_CSRAA", "The base QC encoder invokes the rotation from its per-block loop."),
    ("aff3ct-user-interleaver-file", "aff3ct", "src/Tools/Interleaver/User/Interleaver_core_user.cpp", "Interleaver_core_user<T>::Interleaver_core_user(const int size, const std::string& filename)", "The user interleaver core loads an arbitrary permutation table from a file."),
    ("srsran-circular-shift-forward", "srsran", "include/srsran/srsvec/circ_shift.h", "void circ_shift_forward", "srsRAN exposes a circular forward shift."),
    ("srsran-circular-shift-backward", "srsran", "include/srsran/srsvec/circ_shift.h", "void circ_shift_backward", "srsRAN exposes a circular backward shift."),
    ("srsran-ldpc-encoder-generic-rotation", "srsran", "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_generic.cpp", "srsvec::circ_shift_backward(out_node, codeblock_node, node_shift);", "The generic LDPC encoder rotates a codeblock node by its lifted shift."),
    ("srsran-ldpc-encoder-rotation", "srsran", "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.cpp", "srsvec::circ_shift_backward(", "The AVX2 LDPC encoder consumes the circular shift."),
    ("srsran-rate-matcher-k0", "srsran", RM, "shift_k0   = static_cast<uint16_t>(std::floor(tmp)) * lifting_size;", "The rate matcher computes the redundancy-version start offset k0."),
    ("srsran-rate-matcher-select-bits", "srsran", RM, "void ldpc_rate_matcher_impl::select_bits(", "Bit selection is a separate member function."),
    ("srsran-rate-matcher-start", "srsran", RM, "unsigned in_index      = shift_k0;", "Selection starts reading the circular buffer at k0."),
    ("gf2-dvb-t2-interleaver", "gf2", DVB, "pub struct DvbT2BitInterleaver", "gf2's DVB-T2 bit interleaver type."),
    ("gf2-dvb-t2-parity-perm", "gf2", DVB, "parity_perm[k + 360 * t + s] = k + q * s + t;", "gf2's parity permutation, matched by xdsopl's PITL."),
    ("gf2-dvb-t2-twist-row", "gf2", DVB, "let src_row = (r + nr - config.twist[c] % nr) % nr;", "gf2's inverse column twist, matched by xdsopl's CT functors."),
    ("gf2-dvb-t2-forward-table", "gf2", DVB, "forward: Vec<usize>,", "The permutation is a table of one usize per bit, built once by new()."),
    ("gf2-dvb-t2-interleave", "gf2", DVB, "pub fn interleave(&self, bits: &BitVec) -> BitVec {", "The timed gf2 call permutes a packed BitVec into a new BitVec."),
    ("gf2-dvb-t2-scatter-loop", "gf2", DVB, "for (i, &out_idx) in self.forward.iter().enumerate() {", "interleave walks the table once per input bit."),
    ("gf2-dvb-t2-scatter-branch", "gf2", DVB, "if bits.get(i) {", "Each input bit is tested and, when set, scattered to its output position.", 473),
    ("gf2-dvb-t2-twist-spec-test", "gf2", DVB, "fn test_twist_offsets_match_spec()", "gf2 checks its twist offsets against the standard's table."),
    ("gf2-dvb-t2-twist-example-test", "gf2", DVB, "fn test_64qam_normal_col_twist_spec_example()", "gf2 checks a 64-QAM normal-frame column-twist example."),
    ("gf2-kb-for-z-selection", "gf2", NR, "pub fn kb_for_z_selection(base_graph: u8, target_k: usize) -> usize {", "gf2's block-size-dependent Kb, used only for lifting selection."),
    ("gf2-kb-560-640", "gf2", NR, "} else if target_k > 560 {", "gf2's table has a separate branch above 560 that returns 9."),
    ("gf2-z-selection", "gf2", NR, "kb_for_z * z_us >= target_k && target_k + (nb - kb - 2) * z_us >= target_n", "gf2 also requires enough transmitted bits when choosing Z."),
    ("gf2-full-k", "gf2", NR, "let full_k = kb * z;", "gf2's mother-code message length uses the base graph's full Kb."),
    ("gf2-rate-matched-doc-filler", "gf2", NR, "/// punctured positions get LLR=0, filler positions get LLR=+inf.", "The constructor's rustdoc states +inf for fillers."),
    ("gf2-filler-llr", "gf2", NR, "const FILLER_LLR: f32 = 15.0;", "The filler LLR the code writes is 15.0."),
    ("gf2-encode-rate-matched-private", "gf2", NR, "fn encode_rate_matched(&self, message: &BitVec) -> BitVec {", "Rate-matched encoding is a private method."),
    ("gf2-parity-matvec", "gf2", NR, "let parity = enc.parity_matrix.matvec_transpose(&padded);", "gf2 computes parity with one dense matvec rather than per-block circulant shifts."),
    ("gf2-rref-column-mapping", "gf2", NR, "// This correctly handles the case where RREF assigns some natural", "gf2 places message and parity bits through an RREF-derived column mapping."),
    ("gf2-nr-block-encoder", "gf2", NR, "impl crate::traits::BlockEncoder for Nr5gRateMatchedCode {", "The rate-matched code's public encoder is the BlockEncoder implementation."),
    ("gf2-nr-block-encoder-encode", "gf2", NR, "self.encode_rate_matched(message)", "BlockEncoder::encode runs the private rate-matched encoding."),
    ("aff3ct-encoder-public-encode", "aff3ct", "include/Module/Encoder/Encoder.hpp", "void encode(const B* U_K, B* X_N,", "AFF3CT's encoder modules expose a public encode entry point."),
    ("gf2-transmitted-gather", "gf2", NR, "for &col in &enc.transmitted_cols {", "Bit selection is a gather fused into encoding."),
    ("gf2-prepare-llrs-public", "gf2", NR, "pub fn prepare_llrs(&self, channel_llrs: &[Llr]) -> Vec<Llr> {", "The inverse LLR mapping is public."),
    ("gf2-prepare-llrs-zero-init", "gf2", NR, "let mut full_llrs = vec![Llr::zero(); p.full_n];", "Untransmitted and punctured positions start at zero."),
    ("gf2-prepare-llrs-filler", "gf2", NR, "full_llrs[col] = Llr::new(FILLER_LLR);", "Filler positions receive FILLER_LLR."),
    ("gf2-circulant-to-edges", "gf2", LDPC_CORE, "pub fn to_edges(&self, base_row: usize, base_col: usize) -> Vec<(usize, usize)> {", "gf2's only circulant primitive emits sparse coordinates for one block."),
    ("gf2-qc-to-edges-caller", "gf2", LDPC_CORE, "let block_edges = circ.to_edges(base_row, base_col);", "The QC code expands each block into coordinates when it builds its matrix."),
    ("gf2-ldpc-from-qc", "gf2", LDPC_CORE, "let edges = qc.to_edges();", "LDPC code construction consumes those coordinates once per code."),
    ("survey-gf2-arm-setup", "gf2", SURVEY + "gf2-side/src/bin/gf2-dvb-t2-candidate.rs", "let interleaver = DvbT2BitInterleaver::new(modcod_for_name(&case.modcod));", "The gf2 arm builds its table once, outside the timed body, and reports that time as setup."),
    ("survey-gf2-arm-body", "gf2", SURVEY + "gf2-side/src/bin/gf2-dvb-t2-candidate.rs", "let output = interleaver.interleave(&buffers[bank % banks]);", "The gf2 arm's timed body is one interleave of a packed BitVec."),
    ("survey-external-unpack", "gf2", SURVEY + "gf2-side/src/bin/xdsopl-dvb-t2-baseline.rs", "let input = unpack_words(packed, bits);", "The external arm's timed body unpacks the packed frame to one int32 per bit."),
    ("survey-external-output", "gf2", SURVEY + "gf2-side/src/bin/xdsopl-dvb-t2-baseline.rs", "let mut output = vec![0_i32; bits];", "It allocates an int32 output frame inside the timed body."),
    ("survey-external-pack", "gf2", SURVEY + "gf2-side/src/bin/xdsopl-dvb-t2-baseline.rs", "let packed_output = pack_bits(&output);", "It packs the permuted frame back inside the timed body."),
    ("survey-shim-input-copy", "gf2", SURVEY + "xdsopl-shim/xdsopl_shim.cpp", "std::vector<int32_t> mutable_input(input, input + Interleaver::N);", "The shim copies the input because PCTITL::fwd overwrites it."),
    ("survey-shift-offsets", "gf2", SURVEY + "analysis/src/bin/validate-shift-semantics.rs", "offsets.push(length + 1);", "The shift validation includes offsets past the vector length."),
    ("gf2-bitvec-shift-left", "gf2", "crates/gf2-core/src/bitvec.rs", "pub fn shift_left(&mut self, k: usize) {", "Arbitrary-offset zero-fill left shift."),
    ("gf2-bitvec-shift-right", "gf2", "crates/gf2-core/src/bitvec.rs", "pub fn shift_right(&mut self, k: usize) {", "Arbitrary-offset zero-fill right shift."),
    ("gf2-avx2-shift-left-words", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", "unsafe fn avx2_shift_left_words(buf: &mut [u64], word_shift: usize) {", "AVX2 word-shift kernel behind the left shift."),
    ("gf2-avx2-shift-right-words", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", "unsafe fn avx2_shift_right_words(buf: &mut [u64], word_shift: usize) {", "AVX2 word-shift kernel behind the right shift."),
]

# project, files, case-insensitive terms, meaning of an empty result
FILE_SEARCHES = [
    ("aff3ct", [PUNCT, QC_FAST, "include/Module/Puncturer/LDPC/Puncturer_5G.hpp",
                "include/Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.hpp"],
     ["mipp", "__m256", "_mm"],
     "The 5G puncturer and fast QC encoder are scalar C++ with no MIPP or intrinsic backend."),
]


def git(path, *args):
    return subprocess.run(
        ["git", "-C", path, *args], check=True, capture_output=True, text=True,
    ).stdout


def find(full, needle, after):
    with open(full, encoding="utf-8", errors="replace") as handle:
        for number, line in enumerate(handle, 1):
            if number > after and needle in line:
                return number, line.strip()
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ext", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    repo = git(os.path.dirname(os.path.abspath(__file__)), "rev-parse", "--show-toplevel").strip()
    roots = {
        "aff3ct": os.path.join(args.ext, "aff3ct"),
        "aff3ct-conf": os.path.join(args.ext, "aff3ct", "conf"),
        "srsran": os.path.join(args.ext, "srsran"),
        "xdsopl-ldpc": os.path.join(args.ext, "xdsopl-ldpc"),
        "gf2": repo,
    }
    commits = {name: git(path, "rev-parse", "HEAD").strip() for name, path in roots.items()}
    errors = [f"{name}: expected {PINS[name]}, got {commits[name]}" for name in PINS if commits[name] != PINS[name]]
    errors += [f"{name}: checkout is not clean" for name in PINS if git(roots[name], "status", "--porcelain")]
    records = []
    for claim, project, path, needle, why, *rest in CLAIMS:
        full = os.path.join(roots[project], path)
        found = find(full, needle, rest[0] if rest else 0)
        if found is None:
            errors.append(f"{claim}: source needle not found")
            continue
        record = {"claim": claim, "project": project, "commit": commits[project], "path": path,
                  "line": found[0], "text": found[1], "why": why}
        if project == "gf2":
            content = open(full, "rb").read()
            if content != subprocess.run(["git", "-C", repo, "show", f"HEAD:{path}"],
                                         check=True, capture_output=True).stdout:
                errors.append(f"{claim}: {path} differs from its HEAD blob")
            record["sha256"] = hashlib.sha256(content).hexdigest()
        records.append(record)

    matrix_root = os.path.join(roots["aff3ct-conf"], "enc", "LDPC", "5G")
    matrices = sorted(name for name in os.listdir(matrix_root) if name.startswith("NR_") and name.endswith(".txt"))
    if len(matrices) != 97:
        errors.append(f"AFF3CT 5G matrix count: expected 97, got {len(matrices)}")
    interleavers = sorted(os.listdir(os.path.join(roots["aff3ct"], "src", "Tools", "Interleaver")))
    encoders = sorted(os.listdir(os.path.join(roots["aff3ct"], "src", "Module", "Encoder", "LDPC")))

    absent_terms = ["dvb-t2", "dvb_t2", "302 755", "302755"]
    absent_hits = []
    for tree in ("src", "include"):
        for root, _, files in os.walk(os.path.join(roots["aff3ct"], tree)):
            for name in files:
                path = os.path.join(root, name)
                try:
                    text = open(path, encoding="utf-8", errors="replace").read().lower()
                except OSError:
                    continue
                if any(term in text for term in absent_terms):
                    absent_hits.append(os.path.relpath(path, roots["aff3ct"]))
    if absent_hits:
        errors.append("AFF3CT DVB-T2 absence search found: " + ", ".join(sorted(set(absent_hits))))
    negative = [{"project": "aff3ct", "roots": ["src", "include"], "case_insensitive_terms": absent_terms,
                 "matching_paths": sorted(set(absent_hits)),
                 "meaning": "AFF3CT has no DVB-T2 or EN 302 755 code."}]
    for project, files, terms, meaning in FILE_SEARCHES:
        hits = [path for path in files
                if any(term in open(os.path.join(roots[project], path), encoding="utf-8",
                                    errors="replace").read().lower() for term in terms)]
        if hits:
            errors.append(f"{project} search {terms} found: " + ", ".join(hits))
        negative.append({"project": project, "files": files, "case_insensitive_terms": terms,
                         "matching_paths": hits, "meaning": meaning})

    report = {
        "schema": "shift-permutation-source-evidence-v1",
        "commits": commits,
        "claims": records,
        "inventories": {
            "aff3ct-5g-matrix-count": len(matrices),
            "aff3ct-interleaver-cores": interleavers,
            "aff3ct-ldpc-encoder-entries": encoders,
        },
        "negative_searches": negative,
        "errors": errors,
    }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(args.output)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
