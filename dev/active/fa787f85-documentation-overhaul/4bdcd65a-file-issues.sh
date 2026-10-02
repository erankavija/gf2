#!/usr/bin/env bash
# Files the backlog tasks that 4bdcd65a-roadmap-map.md marks "newly filed".
set -euo pipefail

jit issue create "Compare ORBGRAND with OSD and algebraic decoding on short codes" \
  --type task --label component:gf2-coding --priority low --description "$(cat <<'EOF'
Measures when GRAND-family decoding outperforms ordered-statistics and algebraic decoding for short block codes, answering the roadmap question of where GRAND is faster.

## Background

The workspace ships ORBGRAND and SOGRAND decoders (`gf2-coding` `grand` module), a generator-matrix OSD decoder (`osd` module) with a published eBCH OSD curve dataset, and algebraic BCH decoding. No benchmark compares the three on the same code, channel and operating point.

## Success Criteria

- [hard] REQ-01: A seeded AWGN campaign reports FER with sample counts and confidence intervals for ORBGRAND, OSD and the algebraic decoder on at least one extended BCH code of length 128 or shorter, at identical Eb/N0 points.
- [hard] REQ-02: A throughput measurement per decoder at each operating point is taken on a prepared benchmark host through the repository benchmark protocol, with the committed receipt naming hardware, toolchain and seeds.
- [hard] REQ-03: The committed result states, per operating point, which decoder has the lowest decoding time at a matched FER and records any operating region where the ranking changes.
- [hard] REQ-04: The campaign is resumable and writes a durable execution log, and a fixed seed reproduces identical FER counts across worker counts.
EOF
)"

jit issue create "Measure the DVB-T2 receive-chain stage latency budget" \
  --type task --label component:gf2-coding --priority low --description "$(cat <<'EOF'
Quantifies per-stage and end-to-end latency of the DVB-T2 receive chain (deinterleave, QAM demapping, LDPC, BCH) for one FEC frame, answering the roadmap latency-budget question.

## Background

The unified DVB-T2 FEC chain and its BICM harness exist in `gf2-coding`, and the bit interleaver and CPU LDPC decoder have throughput profiles. Those profiles report rates, not the per-frame latency each stage contributes to a single frame.

## Success Criteria

- [hard] REQ-01: A benchmark reports per-frame wall-clock latency for each receive stage (deinterleave, demap, LDPC decode, BCH decode) and for the full chain, for a short and a normal DVB-T2 frame at one code rate.
- [hard] REQ-02: The benchmark runs on a prepared benchmark host through the repository benchmark protocol, and the committed receipt records hardware, toolchain, iteration count and confidence intervals.
- [hard] REQ-03: The committed report states each stage's share of total latency and identifies the dominant stage, with LDPC iteration count reported as a parameter.
- [hard] REQ-04: Measured chain output equals the reference chain output for the benchmark inputs, so the timed path is the production path.
EOF
)"
