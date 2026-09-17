#!/usr/bin/env python3
"""Freeze source-derived route and memory-pass evidence for jit:9fb40c83."""

import hashlib
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[4]
OUTPUT = pathlib.Path(
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/dvb-source-evidence.json"
)

SOURCES = {
    "interleaver": pathlib.Path("crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs"),
    "bicm": pathlib.Path("crates/gf2-coding/src/dvb_t2_bicm_harness.rs"),
    "stage": pathlib.Path("crates/gf2-sim/src/stages/mod.rs"),
    "profile_arm": pathlib.Path("dev/active/eda07788/survey/gf2-side/src/bin/dvb-profile-arm.rs"),
    "xdsopl": pathlib.Path("dev/active/eda07788/survey/xdsopl-shim/xdsopl_shim.cpp"),
}

EXPECT = {
    "interleaver": [
        "pub fn interleave(&self, bits: &BitVec) -> BitVec",
        "let mut out = BitVec::zeros(n);",
        "for (i, &out_idx) in self.forward.iter().enumerate()",
        "out.set(out_idx, true);",
    ],
    "bicm": [
        "let interleaved = self.interleaver.interleave(bits);",
        "let interleaved_bits: Vec<bool>",
        "mapper.map_bits(&interleaved_bits, &mut tx_i, &mut tx_q);",
        "self.interleaver.deinterleave_llrs(&interleaved_llrs)",
    ],
    "stage": [
        ".map(|frame| self.interleaver.interleave(frame))",
        "erase(BitInterleave::new(interleaver.clone()))",
    ],
    "profile_arm": [
        "let unpacked = unpack_words(&words[bank], bits);",
        "let mut mutable_input = unpacked.clone();",
        "xdsopl_forward_mut(&modcod, &mut mutable_input, &mut output);",
        "let packed = pack_bits(&output);",
    ],
    "xdsopl": ["Interleaver::fwd(output, input);"],
}


def locate(path, needles):
    text = (ROOT / path).read_text()
    lines = text.splitlines()
    found = []
    for needle in needles:
        positions = [index + 1 for index, line in enumerate(lines) if needle in line]
        if not positions:
            raise SystemExit(f"{path}: required source fragment is absent: {needle}")
        found.append({"line": positions[0], "fragment": needle})
    return {
        "path": str(path),
        "sha256": hashlib.sha256(text.encode()).hexdigest(),
        "matches": found,
    }


def main():
    sources = {name: locate(path, EXPECT[name]) for name, path in SOURCES.items()}
    document = {
        "schema": "dvb-t2-interleave-source-evidence-v1",
        "issue": "9fb40c83",
        "selected_route": "DvbT2BitInterleaver::interleave scalar bit scatter over a precomputed forward Vec<usize>",
        "call_graph": [
            {"caller": "BicmAwgnChannel::transmit_and_demodulate_with_noise", "callee": "DvbT2BitInterleaver::interleave"},
            {"caller": "gf2_sim::stages::BitInterleave::process", "callee": "DvbT2BitInterleaver::interleave"},
            {"caller": "dvb_t2_bicm_stages", "callee": "BitInterleave::new"},
        ],
        "memory_passes": {
            "gf2_direct": [
                "zero-initialize the packed output allocation",
                "scan the forward permutation and input bits",
                "scatter true bits into packed output words",
            ],
            "gf2_sim_stage": [
                "iterate the input frame vector",
                "perform the gf2 direct passes once per frame",
                "collect output frames into BitPackedBatch",
            ],
            "xdsopl_stage_adapter": [
                "unpack packed words to one i32 per bit",
                "copy the destructively consumed i32 input",
                "allocate and write the i32 output",
                "apply PCTITL parity and column-twist passes",
                "pack i32 output bits into canonical little-endian words",
                "collect the packed output frame into BitPackedBatch",
            ],
            "bicm_channel": [
                "perform the gf2 direct passes",
                "expand the interleaved BitVec to Vec<bool>",
                "map bits into separate I and Q vectors",
                "visit I then Q for noise",
                "fill the noise-variance vector",
                "soft-demap into interleaved LLRs",
                "scatter LLRs back through the inverse permutation",
            ],
        },
        "sources": sources,
        "historical_context": {
            "receipt": "dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/receipt.json",
            "acceptance": "dev/bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/acceptance-summary.json",
            "role": "accepted exploratory context only; withdrawn warm receipts confirm no candidate",
        },
    }
    (ROOT / OUTPUT).write_text(json.dumps(document, indent=2) + "\n")
    print(f"{OUTPUT}: {len(document['call_graph'])} call edges, {len(sources)} source pins")


if __name__ == "__main__":
    main()
