#!/usr/bin/env bash
set -euo pipefail

jit issue create "Measure the table-based versus algebraic decoding crossover" --type task --priority low --orphan \
  --label component:gf2-coding \
  --description-file - <<'EOF'
Quantify at which block length and redundancy a table-driven syndrome decoder stops being cheaper than algebraic decoding (Berlekamp-Massey with Chien search) for binary BCH codes, so the choice of decoder per code size rests on measurements.

## Background

gf2-coding offers a syndrome-table decoder for small linear block codes and an algebraic BCH decoder. Table size grows exponentially in the number of parity bits while algebraic decoding cost grows polynomially, so a crossover exists for medium block lengths. The crossover is not measured for either implementation.

## Success Criteria

- [hard] REQ-01: A benchmark measures decode throughput of the syndrome-table decoder and the algebraic BCH decoder over at least four BCH parameter sets spanning small to medium block lengths, at error counts 0 through the designed correction capability t.
- [hard] REQ-02: The benchmark reports table construction time and table memory footprint next to decode throughput for each parameter set.
- [hard] REQ-03: Both decoders return identical corrected codewords for every sampled error pattern with weight at most t, checked within the benchmark or its companion test.
- [hard] REQ-04: A committed record states the measured crossover (block length or parity-bit count) with the benchmark protocol, host, toolchain and commit identity that produced it, and the benchmark runs in release mode on an uncontended host.
EOF

jit issue create "Study compression-transform ordering against error-correction coding" --type task --priority low --orphan \
  --label component:gf2-coding \
  --description-file - <<'EOF'
Measure whether applying a bit-level compression transform (run-length, delta, XOR chaining) before error-correction encoding, or after decoding, changes the redundancy needed or the residual error rate for structured bit streams.

## Background

Compression removes redundancy that error-correcting codes add back in structured form, and the order of the two stages changes both burst-error exposure and overhead. The crate has BCH, LDPC and convolutional encoders plus AWGN simulation, and `BitVec` supports the transforms, but no experiment compares the orderings.

## Success Criteria

- [hard] REQ-01: An experiment compares two pipelines on the same structured source streams: transform then encode, and encode of the raw stream, each over an AWGN channel through a gf2-coding code at a fixed code rate.
- [hard] REQ-02: Each result reports bit-error rate and frame-error rate per Eb/N0 point with sample count and a confidence interval, for at least two source streams of differing redundancy.
- [hard] REQ-03: Every pipeline decodes and inverse-transforms back to the exact source stream in a noiseless round trip, verified by a test.
- [hard] REQ-04: A committed record states the observed effect of ordering on error rates and overhead, with seeds, source identity, toolchain and hardware that produced the numbers.
EOF
