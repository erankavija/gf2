# Quality record of the AFF3CT sum-product INTRA arm

> **Diátaxis Type:** Reference

The prepared per-frame quality of the AFF3CT [Cassagne2019] float sum-product INTRA arm over
the recorded DVB-T2 normal rate-1/2 and 5G NR base-graph-1 lifting-384 mother-code bundles of
`c077a88b`. Each `aff3ct-spa-intra-<code>.json` is the quality form the arm takes in
`GF2_LDPC_QUALITY`: the ordered per-frame bit-error vector, frame and bit error counts with
their intervals, the iteration distribution and the decoder settings. The error rates are read
from these files and are not restated here.

`record.json` states the replay executable's build identity (digest, compiler options,
toolchain, release profile and build flags) with the environment the catalogue declares for the
arm, cites the build identity record and the recorded-inputs archive, and gives per code the
bundle's digests and seed, the `request_declaration` the replay receives and what is
`effective`: the decoder the replay reports (class, SIMD backend, frames per AFF3CT call), the
LLR factor it reports and the settings of the quality file. Host facts are the CPU model and the
AVX2 and FMA flags `/proc/cpuinfo` reports; the replay has no random source beyond the bundles'
recorded seed.

The record is one untimed replay of all frames of each bundle, random and all-zero codewords
included. `memory_bytes` and `latency_ns_p50` are zero because nothing is timed; the arm checks
only the frame-error vector and the settings. Intervals are the quality form's
`interval_method`: a Wilson score interval for the frame-error rate and a Hoeffding interval
over independent frames for the bit-error rate, both at 95% confidence with the replayed frame
count as the sample size.

`check.log` is the output of `record-intra-quality.py check`, a second replay that exits
non-zero when a field its `COMPARED` table names differs from the committed files: the closed
identity set of `@/inv/behavioral-evidence-validity` and the replay results. Every other field
of the record, the linked AFF3CT revision and library among them, is descriptive. Regenerate the files with
`python3 -B dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/record-intra-quality.py record
<this directory>`; the check takes `check` in place of `record`.

## Arm acceptance

`arm-acceptance/` holds the untimed smoke of the group `aff3ct` of `smoke-gf2-arms.py`, which
dispatches the `aff3ct-intra-arm` executable (named there by SHA-256) through the shared runner
with these quality files. The arm decodes its first wave and checks its per-frame bit errors
against the file; the diagnostics record the refusal of a file that differs in one frame and of
a missing file. Regenerate with
`python3 -B dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/smoke-gf2-arms.py <this
directory>/arm-acceptance aff3ct`. The smoke record under `bad1aea8/` names a quality file
replayed into `target/`; the record in `arm-acceptance/` names the committed files and governs
the current plan.

## LLR factor

The recorded LLRs are `r/sigma^2`, half the BPSK-AWGN closed form `2r/sigma^2` (code claims in
`survey/source-evidence.json`, topic `recorded-llr-scale`; `@/issue/11bfbfd9` owns the frame
source). Normalized min-sum is invariant to LLR scale and sum-product is not, so the arm and
this replay multiply each recorded LLR by the factor the catalogue entry `aff3ct-spa-intra` of
`survey/arms.json` declares in `GF2_AFF3CT_LLR_FACTOR` (decision DEC-13 of the story plan). The
factor is a power of two, so the product is exact in f32; the bundles and the matched normalized
min-sum arms' records keep their bytes.

`llr-scale-probe.json` is the evidence for the declared factor: per bundle the corpus scale
measured against the closed form, and the frame and bit errors of a full replay, with their
intervals, at each probed value of the variable over the recorded bundles.
