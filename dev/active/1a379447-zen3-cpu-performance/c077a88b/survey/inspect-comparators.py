#!/usr/bin/env python3
"""Extracts the source evidence behind the survey's compatibility claims
(jit:c077a88b).

Every capability statement in `findings.md` about a comparator names a file and
a line in a pinned external tree. Those trees are staged outside the repository
and are not committed, so this script copies the exact lines into a committed
record: for each claim it stores the repository the line comes from, its pinned
commit, the path, the line number and the literal text. A reader can then check
a claim without re-fetching the upstream, and a re-run against a different pin
fails loudly instead of silently agreeing.

Usage:
  inspect-comparators.py --ext <staging-dir> --output <record.json>
"""

import argparse
import json
import os
import subprocess
import sys

# claim id -> (project, repository-relative path, needle, why it matters)
CLAIMS = [
    (
        "aff3ct-cxx-standard-11",
        "aff3ct",
        "CMakeLists.txt",
        "set(CMAKE_CXX_STANDARD 11)",
        "The pinned build compiles as C++11, so __cpp_aligned_new is undefined.",
    ),
    (
        "aff3ct-flooding-inter-gated",
        "aff3ct",
        "src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp",
        'else if (this->type == "BP_FLOODING" && this->simd_strategy == "INTER")',
        "The flooding INTER branch sits behind the aligned-new guard above it.",
    ),
    (
        "aff3ct-info-bits-iota",
        "aff3ct",
        "src/Tools/Codec/LDPC/Codec_LDPC.cpp",
        "std::iota(info_bits_pos->begin(), info_bits_pos->end(), 0);",
        "Without an encoder AFF3CT scores codeword positions 0..K-1, the window this survey scores.",
    ),
    (
        "aff3ct-layered-inter-no-int8",
        "aff3ct",
        "src/Module/Decoder/LDPC/BP/Horizontal_layered/ONMS/Decoder_LDPC_BP_horizontal_layered_ONMS_inter.cpp",
        "This decoder does not work in 8-bit fixed-point.",
        "The inter-frame layered decoder refuses 8-bit fixed point by construction.",
    ),
    (
        "aff3ct-flooding-single-ite-virtual",
        "aff3ct",
        "include/Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hpp",
        "virtual void _decode_single_ite",
        "The one virtual hook that lets the survey count flooding iterations exactly.",
    ),
    (
        "srsran-scaling-factor",
        "srsran",
        "lib/phy/upper/channel_coding/ldpc/ldpc_decoder_impl.h",
        "float scaling_factor = 0.8;",
        "srsRAN fixes its normalized min-sum factor privately; no public field reaches it.",
    ),
    (
        "srsran-shortened-input",
        "srsran",
        "lib/phy/upper/channel_coding/ldpc/ldpc_decoder_impl.cpp",
        "uint16_t max_input_length = bg_N_short * lifting_size;",
        "srsRAN takes the shortened codeblock, not the mother code.",
    ),
    (
        "srsran-crc-stopping",
        "srsran",
        "include/srsran/phy/upper/channel_coding/ldpc/ldpc_decoder.h",
        "Set to \\c nullptr for disabling early",
        "srsRAN stops on a CRC or not at all; it exposes no syndrome stopping.",
    ),
    (
        "srsran-avx2-source",
        "srsran",
        "lib/phy/upper/channel_coding/ldpc/CMakeLists.txt",
        'set_source_files_properties(ldpc_decoder_avx2.cpp PROPERTIES COMPILE_OPTIONS "-mavx2;")',
        "The accelerated backend and the exact flag srsRAN gives it.",
    ),
    (
        "xdsopl-dvb-t2-table",
        "xdsopl-ldpc",
        "dvb_t2_tables.hh",
        "struct DVB_T2_TABLE_A1",
        "The DVB-T2 rate-1/2 accumulator table the code-identity check rebuilds.",
    ),
    (
        "xdsopl-parity-orientation",
        "xdsopl-ldpc",
        "flooding_decoder.hh",
        "cnv[i] = alg.sign(alg.sign(alg.one(), bnv[i-1]), bnv[i]);",
        "Check i involves parity i-1 and i, fixing the staircase orientation.",
    ),
    (
        "xdsopl-no-normalized-min-sum",
        "xdsopl-ldpc",
        "generic.hh",
        "struct MinSumCAlgorithm",
        "The correction-factor rule xdsopl offers in place of a normalized min-sum.",
    ),
    (
        "oai-parity-check-stopping",
        "oai",
        "openair1/PHY/CODING/nrLDPC_decoder/nrLDPC_decoder.c",
        "while ((numIter < numMaxIter) && (pcRes != 0)) {",
        "OpenAirInterface stops on a parity check rather than a fixed count.",
    ),
    (
        "oai-punctured-prefix",
        "oai",
        "openair1/PHY/CODING/nrLDPC_decoder/nrLDPC_decoder.c",
        "because first 2 cols/BNs in BG are punctured and cannot be",
        "OpenAirInterface takes the punctured codeblock, not the mother code.",
    ),
    (
        "oai-avx512-kernel",
        "oai",
        "openair1/PHY/CODING/nrLDPC_decoder/nrLDPC_cnProc.h",
        "#if defined(__AVX512BW__)",
        "The accelerated check-node kernels the build selects at compile time.",
    ),
    (
        "oai-max-llr",
        "oai",
        "openair1/PHY/CODING/nrLDPC_decoder/nrLDPCdecoder_defs.h",
        "#define NR_LDPC_MAX_NUM_LLR",
        "The buffer bound that decides whether BG1 lifting 384 fits.",
    ),
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ext", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    commits = {}
    for project in ("aff3ct", "srsran", "xdsopl-ldpc", "oai"):
        root = os.path.join(args.ext, project)
        commits[project] = subprocess.run(
            ["git", "-C", root, "rev-parse", "HEAD"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()

    records = []
    missing = []
    for claim_id, project, path, needle, why in CLAIMS:
        full = os.path.join(args.ext, project, path)
        found = None
        with open(full, encoding="utf-8", errors="replace") as handle:
            for number, line in enumerate(handle, start=1):
                if needle in line:
                    found = (number, line.rstrip("\n"))
                    break
        if found is None:
            missing.append(claim_id)
            continue
        records.append(
            {
                "claim": claim_id,
                "project": project,
                "commit": commits[project],
                "path": path,
                "line": found[0],
                "text": found[1].strip(),
                "why": why,
            }
        )

    report = {
        "schema": "ldpc-survey-source-evidence-v1",
        "commits": commits,
        "claims": records,
        "claims_not_found": missing,
    }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(args.output)
    if missing:
        print("claims not found: %s" % ", ".join(missing), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
