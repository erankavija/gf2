# Quality record of the AFF3CT sum-product INTRA arm

> **Diátaxis Type:** Reference

The prepared per-frame quality of the AFF3CT [Cassagne2019] float sum-product INTRA arm over
the recorded DVB-T2 normal rate-1/2 and 5G NR base-graph-1 lifting-384 mother-code bundles of
`c077a88b`. Each `aff3ct-spa-intra-<code>.json` is the quality form the arm takes in
`GF2_LDPC_QUALITY`: the ordered per-frame bit-error vector, frame and bit error counts with
their intervals, the iteration distribution and the decoder settings. The error rates are read
from these files and are not restated here.

`record.json` pins the executable, the build identity record, the recorded-inputs archive, each
bundle's digests and seed, the frozen decoder case, and the decoder the replay observed (class,
SIMD backend, frames per AFF3CT call). The toolchain and build flags are those of the cited
build identity. Host facts are the CPU model and the AVX2 and FMA flags `/proc/cpuinfo` reports;
the replay has no random source beyond the bundles' recorded seed.

The record is one untimed replay of all frames of each bundle, random and all-zero codewords
included. `memory_bytes` and `latency_ns_p50` are zero because nothing is timed; the arm checks
only the frame-error vector and the settings. Intervals are the quality form's
`interval_method`: a Wilson score interval for the frame-error rate and a Hoeffding interval
over independent frames for the bit-error rate, both at 95% confidence with the replayed frame
count as the sample size.

`check.log` is the output of `record-intra-quality.py check`, a second replay that exits
non-zero on any difference from the committed files. Regenerate with
`python3 -B dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/record-intra-quality.py record
<this directory>`.
