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
non-zero on any difference from the committed files, host facts and `llr-scale-probe.json`
included. Regenerate the files with
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

## Observed limit

The arm's frame-error rate and iteration counts differ from those of the matched normalized
min-sum arms on the same bundles: compare these files with the `aff3ct-flooding-nms-f32-*`
files of `c077a88b/v3-preparation/quality/`. The recorded LLRs are `r/sigma^2` where the
true BPSK-AWGN LLR is `2r/sigma^2`, half the true scale (`llr-scale-probe.json`; code claims
in `survey/source-evidence.json`, topic `recorded-llr-scale`). Normalized min-sum is
invariant to LLR scale and sum-product is not. The probe replays throwaway copies of the
bundles with the LLRs multiplied by 1, 2 and 4: at the true scale the arm decodes with fewer
iterations than at the recorded scale; the bundles, the arm and these records are unchanged.
The record states the arm as it decodes the recorded corpus.
