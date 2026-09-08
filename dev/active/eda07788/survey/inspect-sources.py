#!/usr/bin/env python3
"""Emit source-backed evidence for the eda07788 comparator survey."""

import argparse
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

# claim, project, path, needle, interpretation
CLAIMS = [
    ("xdsopl-dvb-t2-standard", "xdsopl-ldpc", "README.md", "en_302755v010401p.pdf", "The project names ETSI EN 302 755 v1.4.1."),
    ("xdsopl-dvb-t2-table-a1", "xdsopl-ldpc", "tables_handler.cc", "return new LDPC<DVB_T2_TABLE_A1>();", "The program selects the rate-1/2 DVB-T2 table."),
    ("xdsopl-pitl-definition", "xdsopl-ldpc", "interleaver.hh", "struct PITL\n", "PITL is the parity-interleaving stage."),
    ("xdsopl-pctitl-definition", "xdsopl-ldpc", "interleaver.hh", "struct PCTITL", "PCTITL composes parity interleaving with column twisting."),
    ("xdsopl-ct8-definition", "xdsopl-ldpc", "interleaver.hh", "struct CT8", "CT8 supplies the eight-column twist."),
    ("xdsopl-ct12-definition", "xdsopl-ldpc", "interleaver.hh", "struct CT12", "CT12 supplies the twelve-column twist."),
    ("xdsopl-normal-r12-instantiation", "xdsopl-ldpc", "itls_handler.cc", "PCTITL<code_type, 64800, 90, CT>", "The program instantiates normal rate-1/2 PCTITL."),
    ("xdsopl-short-r12-instantiation", "xdsopl-ldpc", "itls_handler.cc", "PCTITL<code_type, 16200, 25, CT>", "The program instantiates short rate-1/2 PCTITL."),
    ("aff3ct-bg-selection", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", "if (K <= 292 ||", "AFF3CT selects the 5G base graph from K and rate."),
    ("aff3ct-kb-560-640", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", "else if (K > 560 && K <= 640)", "The pinned implementation uses Kb=8 in this band."),
    ("aff3ct-mother-dimensions", "aff3ct", "src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp", "base_graph.K_LDPC = base_graph.K_LDPC * base_graph.Zc;", "AFF3CT scales mother-code dimensions by Zc."),
    ("aff3ct-puncture-entry", "aff3ct", "src/Module/Puncturer/LDPC/Puncturer_5G.cpp", "Puncturer_5G<B, Q>::_puncture", "The 5G puncturer owns the selection operation."),
    ("aff3ct-puncture-selection", "aff3ct", "src/Module/Puncturer/LDPC/Puncturer_5G.cpp", "X_N2[k] = X_N1[j %", "The operation reads the circular buffer while skipping fillers."),
    ("aff3ct-puncturer-factory", "aff3ct", "src/Factory/Module/Puncturer/LDPC/Puncturer_LDPC.cpp", "if (this->type == \"LDPC_5G\") return new module::Puncturer_5G", "The puncturer factory constructs Puncturer_5G."),
    ("aff3ct-codec-5g-wiring", "aff3ct", "src/Factory/Tools/Codec/LDPC/Codec_LDPC.cpp", "if (enc->type == \"LDPC_5G\")", "The codec wires encoder and decoder to the 5G standard and dimensions."),
    ("aff3ct-encoder-5g-header", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", "if (this->type == \"LDPC_5G\")", "The encoder factory exposes 5G source and build paths."),
    ("aff3ct-encoder-5g-build", "aff3ct", "src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp", "return new module::Encoder_LDPC_QC_fast", "The encoder factory selects the QC implementation."),
    ("aff3ct-decoder-5g-build", "aff3ct", "src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp", "if (this->H_path.empty() && this->standard == \"5G\")", "The decoder factory derives its 5G base graph and matrix path."),
    ("aff3ct-qc-rotation", "aff3ct", "src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC.cpp", "std::rotate(Gen.begin() + j * Zc", "The QC encoder rotates each Zc-sized generator block."),
    ("aff3ct-qc-rotation-call", "aff3ct", "src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC.cpp", "res = this->_CSRAA", "The encoder invokes the rotation from its per-block loop."),
    ("srsran-circular-shift-backward", "srsran", "include/srsran/srsvec/circ_shift.h", "void circ_shift_backward", "srsRAN exposes a circular backward shift."),
    ("srsran-ldpc-encoder-rotation", "srsran", "lib/phy/upper/channel_coding/ldpc/ldpc_encoder_avx2.cpp", "srsvec::circ_shift_backward(", "The AVX2 LDPC encoder consumes the circular shift."),
]


def revision(path):
    return subprocess.run(
        ["git", "-C", path, "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ext", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    roots = {
        "aff3ct": os.path.join(args.ext, "aff3ct"),
        "aff3ct-conf": os.path.join(args.ext, "aff3ct", "conf"),
        "srsran": os.path.join(args.ext, "srsran"),
        "xdsopl-ldpc": os.path.join(args.ext, "xdsopl-ldpc"),
    }
    commits = {name: revision(path) for name, path in roots.items()}
    errors = [f"{name}: expected {PINS[name]}, got {got}" for name, got in commits.items() if got != PINS[name]]
    records = []
    for claim, project, path, needle, why in CLAIMS:
        full = os.path.join(roots[project], path)
        found = None
        with open(full, encoding="utf-8", errors="replace") as handle:
            for number, line in enumerate(handle, 1):
                if needle in line:
                    found = (number, line.strip())
                    break
        if found is None:
            errors.append(f"{claim}: source needle not found")
        else:
            records.append({"claim": claim, "project": project, "commit": commits[project], "path": path, "line": found[0], "text": found[1], "why": why})

    matrix_root = os.path.join(roots["aff3ct-conf"], "enc", "LDPC", "5G")
    matrices = sorted(name for name in os.listdir(matrix_root) if name.startswith("NR_") and name.endswith(".txt"))
    if len(matrices) != 97:
        errors.append(f"AFF3CT 5G matrix count: expected 97, got {len(matrices)}")

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

    report = {
        "schema": "shift-permutation-source-evidence-v1",
        "commits": commits,
        "claims": records,
        "inventories": {"aff3ct-5g-matrix-count": len(matrices)},
        "negative_searches": [{"project": "aff3ct", "roots": ["src", "include"], "case_insensitive_terms": absent_terms, "matching_paths": sorted(set(absent_hits))}],
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
