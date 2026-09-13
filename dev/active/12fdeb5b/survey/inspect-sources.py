#!/usr/bin/env python3
"""Generate the source-evidence ledger for the NR encoder survey (jit:12fdeb5b).

Every claim names a project, a repository-relative path, a line number and the
verbatim text at that line. The generator reads the pinned trees, asserts that
the recorded anchor still occurs at the recorded line, and copies the line
exactly, so no claim's text is transcribed by hand. Claims that rest on the
absence of a construct are negative searches: the generator runs the search and
records the matching paths it found, which must be empty for the claim to hold.

Usage:
  inspect-sources.py <aff3ct-root> <srsran-root> <output.json>

The gf2 paths resolve against this checkout. External commits are the pins
`nr-encode-build.sh` verifies; file digests are the content identity that
decides whether a claim still describes the source.
"""

import hashlib
import json
import pathlib
import re
import subprocess
import sys

# (claim, project, path, line, anchor, why)
CLAIMS = [
    # ---- AFF3CT: licence, derivation, encoder wiring, bit selection --------
    ("aff3ct-license", "aff3ct", "LICENSE", 1, "MIT License",
     "AFF3CT v4.7.0 carries the MIT licence."),
    ("aff3ct-bg-selection", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 30,
     "if (K <= 292 || (K <= 3824 && R <= 0.67f) || R <= 0.25f)",
     "AFF3CT chooses the base graph itself from K and the rate K/N; a caller cannot impose one."),
    ("aff3ct-bg2-branch", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 32,
     "base_graph.Bg = 2;",
     "The rate and length test selects base graph 2 for the configurations it matches."),
    ("aff3ct-kb-560-640", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 77,
     "else if (K > 560 && K <= 640)",
     "AFF3CT's lifting Kb has its own branch for 560 < K <= 640."),
    ("aff3ct-kb-560-640-value", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 78,
     "Kb = 8;",
     "That branch uses Kb = 8, where TS 38.212 Section 5.2.2 and gf2 use 9."),
    ("aff3ct-mother-dimensions", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 115,
     "base_graph.K_LDPC = base_graph.K_LDPC * base_graph.Zc;",
     "K_LDPC is the base-graph systematic column count times the chosen lifting size."),
    ("aff3ct-mother-length", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", 116,
     "base_graph.N_LDPC = base_graph.N_LDPC * base_graph.Zc;",
     "N_LDPC is the base-graph column count times the chosen lifting size, which is gf2's rule."),
    ("aff3ct-encoder-5g-build", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", 154,
     "return new module::Encoder_LDPC_QC_fast<B>(",
     "The 5G encoder AFF3CT's factory builds is Encoder_LDPC_QC_fast, which the shim constructs identically."),
    ("aff3ct-encoder-5g-arguments", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", 155,
     "this->K, base_graph.N_LDPC, base_graph.Zc, G_path.c_str(), base_graph.K_LDPC);",
     "The factory passes K, the mother length, the lifting size, the generator-matrix path and K_LDPC."),
    ("aff3ct-encoder-5g-g-path", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", 150,
     'G_path = "conf/enc/LDPC/5G/NR_" + std::to_string(base_graph.Bg) + "_" +',
     "The generator matrix is a per (base graph, lifting set, lifting size) file in AFF3CT's conf tree."),
    ("aff3ct-encoder-public-encode", "aff3ct", "include/Module/Encoder/Encoder.hpp", 108,
     "void encode(const B* U_K, B* X_N, const int frame_id = -1, const bool managed_memory = true);",
     "Encoding is reachable through the public Encoder::encode entry point."),
    ("aff3ct-puncturer-public-puncture", "aff3ct", "include/Module/Puncturer/Puncturer.hpp", 102,
     "void puncture(const B* X_N1, B* X_N2, const int frame_id = -1, const bool managed_memory = true);",
     "Bit selection is reachable through the public Puncturer::puncture entry point."),
    ("aff3ct-puncture-selection", "aff3ct", "src/Module/Puncturer/LDPC/Puncturer_5G.cpp", 42,
     "X_N2[k] = X_N1[j % (this->N_cw - 2 * this->base_graph.Zc) + 2 * this->base_graph.Zc];",
     "Selection reads the mother codeword from position 2*Zc, wrapping over the shortened buffer."),
    ("aff3ct-puncture-filler-skip", "aff3ct", "src/Module/Puncturer/LDPC/Puncturer_5G.cpp", 39,
     "if (!(j % (this->N_cw - 2 * this->base_graph.Zc) + 2 * this->base_graph.Zc < this->base_graph.K_LDPC &&",
     "Selection skips the filler positions between K and K_LDPC."),
    ("aff3ct-qc-fast-encode", "aff3ct", "src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC_fast.cpp", 72,
     "Encoder_LDPC_QC_fast<B>::_encode(const B* U_K, B* X_N, const size_t frame_id)",
     "The encode hook the public entry runs is the fast quasi-cyclic encoder."),

    # ---- srsRAN: licence, interfaces, caller-supplied parameters ----------
    ("srsran-license", "srsran", "LICENSE", 1,
     "                    GNU AFFERO GENERAL PUBLIC LICENSE",
     "The srsRAN Project source carries the GNU Affero General Public Licence version 3."),
    ("srsran-version", "srsran", "cmake/modules/version.cmake", 21,
     "set(SRSRAN_VERSION_MAJOR 25)",
     "The pinned tree declares srsRAN Project version 25.10."),
    ("srsran-encoder-entry", "srsran", "include/srsran/phy/upper/channel_coding/ldpc/ldpc_encoder.h", 59,
     "virtual const ldpc_encoder_buffer& encode(const bit_buffer& input, const configuration& cfg) = 0;",
     "Encoding is a public interface method taking the packed message and returning an encoder buffer."),
    ("srsran-encoder-caller-base-graph", "srsran",
     "include/srsran/phy/upper/channel_coding/ldpc/ldpc_encoder.h", 45,
     "ldpc_base_graph_type base_graph = ldpc_base_graph_type::BG1;",
     "srsRAN takes the base graph from its caller, as gf2 does, so no derivation can diverge."),
    ("srsran-encoder-caller-lifting", "srsran",
     "include/srsran/phy/upper/channel_coding/ldpc/ldpc_encoder.h", 47,
     "ldpc::lifting_size_t lifting_size = ldpc::LS2;",
     "srsRAN takes the lifting size from its caller too."),
    ("srsran-codeblock-short", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_impl.cpp", 77,
     "return bg_N_short * lifting_size;",
     "The encoder's codeblock excludes the 2*Z punctured systematic prefix."),
    ("srsran-rate-matcher-entry", "srsran",
     "include/srsran/phy/upper/channel_coding/ldpc/ldpc_rate_matcher.h", 47,
     "virtual void rate_match(bit_buffer& output, const ldpc_encoder_buffer& input, const codeblock_metadata& cfg) = 0;",
     "Rate matching is a public interface method writing the packed rate-matched codeblock."),
    ("srsran-rate-matcher-rv-range", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 39,
     'srsran_assert((cfg.tb_common.rv >= 0) && (cfg.tb_common.rv <= 3), "RV should an integer between 0 and 3.");',
     "srsRAN implements all four redundancy versions."),
    ("srsran-rate-matcher-shift-bg1", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 34,
     "static const std::array<double, 4> shift_factor_bg1 = {0, 17, 33, 56};",
     "The base-graph-1 redundancy-version offsets are TS 38.212 Table 5.4.2.1-2's."),
    ("srsran-rate-matcher-shift-bg2", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 35,
     "static const std::array<double, 4> shift_factor_bg2 = {0, 13, 25, 43};",
     "The base-graph-2 redundancy-version offsets are that table's second row set."),
    ("srsran-rate-matcher-k0", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 90,
     "shift_k0   = static_cast<uint16_t>(std::floor(tmp)) * lifting_size;",
     "The starting offset k0 is zero only for redundancy version 0, whose shift factor is zero."),
    ("srsran-rate-matcher-start", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 108,
     "unsigned in_index      = shift_k0;",
     "Bit selection starts at that offset, which is the only difference from AFF3CT's rule."),
    ("srsran-rate-matcher-systematic", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 82,
     "nof_systematic_bits = (BG_K - 2) * lifting_size;",
     "The systematic count excludes the 2*Z prefix, so filler indices are relative to the shortened buffer."),
    ("srsran-rate-matcher-filler-range", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 121,
     "interval<unsigned> filler_bits_range(nof_systematic_bits - nof_filler_bits, nof_systematic_bits);",
     "Filler bits occupy the end of the systematic section and selection skips them."),
    ("srsran-interleave-identity", "srsran",
     "lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp", 176,
     "srsvec::bit_pack(out, in);",
     "At one bit per symbol TS 38.212 Section 5.4.2.2 interleaving is the identity, leaving only packing."),
    ("srsran-encoder-backend-auto", "srsran",
     "lib/phy/upper/channel_coding/channel_coding_factories.cpp", 154,
     'if (((enc_type == "avx2") || (enc_type == "auto")) && supports_avx2) {',
     "srsRAN's own factory selects the AVX2 backend on a CPU that reports AVX2; the shim reproduces that choice."),

    # ---- gf2: the consumer entry point and its fused selection ------------
    ("gf2-block-encoder-encode", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 1263,
     "fn encode(&self, message: &BitVec) -> BitVec {",
     "The public whole-consumer entry point is the BlockEncoder implementation on the rate-matched code."),
    ("gf2-encode-rate-matched-private", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 1087,
     "fn encode_rate_matched(&self, message: &BitVec) -> BitVec {",
     "The rate-matched encode itself is private, so the consumer is the only comparable operation."),
    ("gf2-filler-pad", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 1100,
     "let mut padded = BitVec::zeros(p.full_k);",
     "Filler positions are zero-padded into the systematic block before the parity solve."),
    ("gf2-transmitted-gather", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 1121,
     "for &col in &enc.transmitted_cols {",
     "Bit selection is a gather over a precomputed column list fused into the encode."),
    ("gf2-kb-560-640", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 228,
     "} else if target_k > 560 {",
     "gf2's lifting Kb has the same 560 < K <= 640 branch."),
    ("gf2-kb-560-640-value", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 229,
     "9",
     "That branch uses Kb = 9, the value TS 38.212 Section 5.2.2 specifies."),
    ("gf2-z-selection", "gf2", "crates/gf2-coding/src/ldpc/nr_5g/mod.rs", 499,
     "kb_for_z * z_us >= target_k && target_k + (nb - kb - 2) * z_us >= target_n",
     "gf2 additionally requires enough transmitted bits when choosing the lifting size."),
]

# (claim, project, roots, terms, meaning)
NEGATIVE_SEARCHES = [
    ("gf2-no-redundancy-version", "gf2", ["crates/gf2-coding/src/ldpc"],
     ["redundancy_version", "redundancy version", "rv0", "k_0", "shift_k0"],
     "gf2's LDPC sources carry no redundancy-version parameter, so its rate-matched "
     "encoder implements only the version-0 starting offset."),
    ("aff3ct-no-redundancy-version", "aff3ct",
     ["src/Module/Puncturer/LDPC", "include/Module/Puncturer/LDPC",
      "src/Tools/Code/LDPC/Standard/5G", "include/Tools/Code/LDPC/Standard/5G"],
     ["redundancy", "rv", "k0"],
     "AFF3CT's 5G puncturer and base-graph derivation carry no redundancy-version "
     "offset, so they implement only the version-0 starting offset."),
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def commit(root):
    return subprocess.run(["git", "-C", str(root), "rev-parse", "HEAD"],
                          capture_output=True, text=True, check=True).stdout.strip()


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    aff3ct = pathlib.Path(sys.argv[1]).resolve()
    srsran = pathlib.Path(sys.argv[2]).resolve()
    output = pathlib.Path(sys.argv[3])
    gf2 = pathlib.Path(subprocess.run(["git", "rev-parse", "--show-toplevel"],
                                      capture_output=True, text=True,
                                      check=True).stdout.strip())
    roots = {"aff3ct": aff3ct, "srsran": srsran, "gf2": gf2}

    errors = []
    files = {}
    claims = []
    for name, project, relative, line, anchor, why in CLAIMS:
        path = roots[project] / relative
        text = path.read_text(errors="replace").splitlines()
        if not 1 <= line <= len(text):
            errors.append(f"{name}: {project}/{relative} has no line {line}")
            continue
        got = text[line - 1].strip()
        if anchor.strip() not in got:
            errors.append(
                f"{name}: {project}/{relative}:{line} is {got!r}, expected to contain {anchor!r}")
            continue
        files[f"{project}/{relative}"] = digest(path)
        claims.append({"claim": name, "project": project, "path": relative,
                       "line": line, "text": got, "why": why})

    searches = []
    for name, project, search_roots, terms, meaning in NEGATIVE_SEARCHES:
        pattern = re.compile("|".join(re.escape(term) for term in terms), re.IGNORECASE)
        matching = []
        for search_root in search_roots:
            base = roots[project] / search_root
            for candidate in sorted(base.rglob("*")):
                if not candidate.is_file():
                    continue
                if pattern.search(candidate.read_text(errors="replace")):
                    matching.append(str(candidate.relative_to(roots[project])))
        searches.append({"claim": name, "project": project, "roots": search_roots,
                         "case_insensitive_terms": terms,
                         "matching_paths": matching, "meaning": meaning})
        if matching:
            errors.append(f"{name}: negative search matched {matching}")

    document = {
        "schema": "nr-encoder-source-evidence-v1",
        "commits": {"aff3ct": commit(aff3ct),
                    "aff3ct-conf": commit(aff3ct / "conf"),
                    "srsran": commit(srsran),
                    "gf2": commit(gf2)},
        "file_sha256": files,
        "claims": claims,
        "negative_searches": searches,
        "errors": errors,
    }
    output.write_text(json.dumps(document, indent=1) + "\n")
    print(f"{len(claims)} claims, {len(searches)} negative searches, {len(errors)} errors"
          f" -> {output}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
