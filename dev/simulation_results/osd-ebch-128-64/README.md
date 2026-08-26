# eBCH(128,64) ordered-statistics decoding campaign

This directory holds one resumable OSD reproduction campaign and its versioned
evidence. The scientific target is the order-2 bit-error-rate curve of the
extended BCH code $(128, 64, 22)$ on BI-AWGN with BPSK at rate $1/2$ published
in [Fossorier1994]. Order 1 is an internal control of this campaign, not a
reproduction target.

| Path | Role |
| --- | --- |
| `ebch_osd_awgn.json` | Versioned statistical receipt; the authoritative record |
| `ebch_osd_awgn.checkpoint.json` | Durable resumable progress, one entry per cell attempt |
| `README.md` | This provenance record |

Every number below is a projection of `ebch_osd_awgn.json`; the receipt is the
source of truth for seeds, counts, intervals, work counters, and provenance.

## What was run

The producer is `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`, built as the
`gf2-sim` release binary `ebch_osd_awgn_campaign` and driven by the reusable
protocol in `gf2_sim::osd_campaign`. The receipt and the checkpoint both carry
**schema version 1**, the value of
`gf2_sim::osd_campaign::OSD_CAMPAIGN_SCHEMA_VERSION`.

The pinned grid is the seven [Fossorier1994] abscissas
$E_b/N_0 \in \{1.55, 2.22, 3.01, 3.47, 3.98, 4.56, 5.23\}$ dB, instantiated
twice: seven order-2 target cells and seven order-1 control cells. The campaign
root seed is `0xC8322EFF`, and each cell's stream seed is derived from that root
and the cell's stable identity through domain-separated BLAKE3, so grid order
cannot change a cell's samples. A cell completes at a cumulative 100
information-bit errors. Both rates carry equal-tailed exact Clopper-Pearson
intervals at level $0.95$; BER is the comparison metric and BLER is recorded as
a companion quantity. The seed and the error target are the producer's defaults;
neither was changed during this campaign.

The decoder reprocesses in increasing Hamming weight over the 64
most-reliable-independent positions, lexicographically ascending within a
weight. The source leaves reliability ties and equal-distance ties undefined:
this implementation breaks a reliability tie by ascending original coordinate
index and retains the first generated candidate at an equal metric. Those are
implementation decisions, not source claims.

Invocations are bounded by `--max-samples`, which caps the samples one
invocation adds to a cell. Reaching that bound records an interrupted cell
whose cumulative counters a later invocation continues.

## Order-2 comparison against the published curve

Published values are the committed dataset
[`osd_ebch_128_64_fossorier1994.csv`](../../reference_data/osd_ebch_128_64_fossorier1994.csv)
and its provenance record
[`osd_ebch_128_64_fossorier1994.md`](../../reference_data/osd_ebch_128_64_fossorier1994.md);
nothing is re-digitized here. The column $p$ is that dataset's primary `value`
field and $\delta$ is the digitization precision the receipt records for the
cell. A point accepts its published value exactly when
$p \in [L - \delta, U + \delta]$ for the receipt's Clopper-Pearson interval
$[L, U]$ at level $0.95$.

| $E_b/N_0$ (dB) | published $p$ | source | $\delta$ | blocks $n$ | sampled bits | bit errors | BER | 95% Clopper-Pearson $[L,U]$ | $p\in[L-\delta,U+\delta]$ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1.55 | 2.14e-02 | Fig. 4.14 | 0.1 | 54 | 3456 | 107 | 3.10e-02 | [2.54e-02, 3.73e-02] | accept |
| 2.22 | 6.31e-03 | Table 4.7 | 0 | 147 | 9408 | 103 | 1.09e-02 | [8.94e-03, 1.33e-02] | **outside** |
| 3.01 | 1.00e-03 | Table 4.7 | 0 | 2390 | 152960 | 110 | 7.19e-04 | [5.91e-04, 8.67e-04] | **outside** |
| 3.47 | 2.51e-04 | Table 4.7 | 0 | 6486 | 415104 | 102 | 2.46e-04 | [2.00e-04, 2.98e-04] | accept |
| 3.98 | 3.98e-05 | Table 4.7 | 0 | 26757 | 1712448 | 108 | 6.31e-05 | [5.17e-05, 7.61e-05] | **outside** |
| 4.56 | 2.00e-06 | Table 4.7 | 0 | 151600 | 9702400 | 100 | 1.03e-05 | [8.39e-06, 1.25e-05] | **outside** |
| 5.23 | 2.51e-07 | Table 4.7 (bound) | 0 | 8000000 | 512000000 | 96 | 1.88e-07 | [1.52e-07, 2.29e-07] | **outside** (interim) |

Four of the six abscissas whose published entry is a simulation fall outside
their acceptance window. Those measurements stand as recorded; no seed, error
target, tie policy, or other control was changed in response to them. The
deviation is not one-signed: at 3.01 dB the measured interval lies below the
published value, and at 2.22, 3.98, and 4.56 dB it lies above, with the largest
separation at 4.56 dB.

The 5.23 dB row is marked interim because that cell is interrupted at its
invocation bound rather than complete, holding 96 of its 100 target bit errors.
Its published entry is a bound rather than a measurement, so its verdict is not
evidence about curve reproduction; see
[Contradictory evidence](#contradictory-evidence).

## Order-1 internal control

These cells are measurements this campaign produces as an internal control on
the decoder, channel, and interval machinery across the same abscissas. They are
not externally sourced claims, and no published value is asserted or tested for
them.

| $E_b/N_0$ (dB) | blocks $n$ | sampled bits | bit errors | BER | 95% Clopper-Pearson $[L,U]$ | BLER | status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1.55 | not started | — | — | — | — | — | pending |
| 2.22 | not started | — | — | — | — | — | pending |
| 3.01 | not started | — | — | — | — | — | pending |
| 3.47 | not started | — | — | — | — | — | pending |
| 3.98 | not started | — | — | — | — | — | pending |
| 4.56 | not started | — | — | — | — | — | pending |
| 5.23 | not started | — | — | — | — | — | pending |

The control cells follow every order-2 cell in the campaign grid, and an
invocation returns at its first interrupted cell, so they remain unmeasured
while `order-2-point-06` is interrupted.

## Cell status and resumption

| cell | order | $E_b/N_0$ (dB) | attempts | blocks | bit errors | state |
| --- | --- | --- | --- | --- | --- | --- |
| `order-2-point-00` | 2 | 1.55 | 1 | 54 | 107 | completed |
| `order-2-point-01` | 2 | 2.22 | 1 | 147 | 103 | completed |
| `order-2-point-02` | 2 | 3.01 | 1 | 2390 | 110 | completed |
| `order-2-point-03` | 2 | 3.47 | 1 | 6486 | 102 | completed |
| `order-2-point-04` | 2 | 3.98 | 2 (interrupted, completed) | 26757 | 108 | completed |
| `order-2-point-05` | 2 | 4.56 | 1 | 151600 | 100 | completed |
| `order-2-point-06` | 2 | 5.23 | 2 (interrupted, interrupted) | 8000000 | 96 | interrupted |
| `order-1-point-00` | 1 | 1.55 | 0 | 0 | 0 | not started |
| `order-1-point-01` | 1 | 2.22 | 0 | 0 | 0 | not started |
| `order-1-point-02` | 1 | 3.01 | 0 | 0 | 0 | not started |
| `order-1-point-03` | 1 | 3.47 | 0 | 0 | 0 | not started |
| `order-1-point-04` | 1 | 3.98 | 0 | 0 | 0 | not started |
| `order-1-point-05` | 1 | 4.56 | 0 | 0 | 0 | not started |
| `order-1-point-06` | 1 | 5.23 | 0 | 0 | 0 | not started |

Resume with the same command and the same checkpoint path:

```console
$ cargo build --release -p gf2-sim --bin ebch_osd_awgn_campaign
$ ./target/release/ebch_osd_awgn_campaign \
    --checkpoint dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.checkpoint.json \
    --receipt dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.json \
    --max-samples N
```

The protocol skips every cell holding a terminal result and continues an
interrupted cell from its durable counters, so completed work is never repeated.
Each attempt stays in the receipt's `cell_results` history, so an interrupted
attempt remains visible whether a later attempt completes the cell, as at
3.98 dB, or bounds it again, as at 5.23 dB.

`order-2-point-06` is the one cell still short of its stopping rule, holding 96
of 100 target bit errors after 8,000,000 blocks. Continuing it replays those
8,000,000 blocks before drawing a new sample, so the next invocation should
carry a `--max-samples` bound large enough to finish the cell in one attempt.

Two properties bound a resume. The evaluator replays a resumed cell's durable
prefix through the same decoder to re-advance its ChaCha20 stream, so the cost
of continuing a cell includes re-running the samples it already holds. The
campaign configuration hash covers `provenance.runtime.git_revision`, so a
resume is accepted only while the repository HEAD equals the revision recorded
in the checkpoint; at any later revision the checkpoint is refused with a
configuration-hash mismatch and the campaign continues only from a checkout of
the recorded revision.

## Provenance

Each row names the receipt field it projects.

| Field | Value | Receipt path |
| --- | --- | --- |
| Repository revision at run start | `673bb9283fba7b264b62a39b721449696d311616` | `provenance.runtime.git_revision` |
| Source-closure revision | `0d8749157c2ae52ee5f692b66d7e2348c733dfd6` | `provenance.runtime.deps_source_revision` |
| Source closure dirty | `false` | `provenance.runtime.deps_source_dirty` |
| Producing executable | `ae0ecf0c6994053e4aa9c43b12e05c52f8c200022304ac3e19beaedb872ed1bd` | `provenance.runtime.binary_sha256` |
| Toolchain | `rustc 1.97.0 (2d8144b78 2026-07-07)` | `provenance.runtime.compiler_version` |
| RNG | `cha_cha20`, `rand_chacha 0.3.1 (gf2-coding BPSK channel ABI)` | `provenance.runtime.rng_algorithm`, `.rng_version` |
| Hardware | `linux-x86_64`, no GPU, no accelerator runtime | `provenance.runtime.cpu_model`, `.gpu_model`, `.accelerator_runtime` |
| Comparison dataset identity | `dev/reference_data/osd_ebch_128_64_fossorier1994.md`, SHA-256 `26824d11…` | `provenance.configuration` |
| Measurement behavior identity | `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`, SHA-256 `3600d106…` | `provenance.measurement_behavior` |
| Configuration hash | `blake3:54d446f6…` | `configuration_hash` |
| Receipt schema version | `1` | `schema_version` |

The revision is observed at run start and recorded as context, so it names the
revision the campaign ran at rather than the commit that carries these files.
The recorded hardware field is the producer's OS and architecture token; the
producer records no specific CPU model.

## Contradictory evidence

**Table-versus-figure contradiction at 4.56 dB, order 2.** The committed dataset
preserves an unreconciled disagreement at this abscissa: Table 4.7 prints
$10^{-5.7} = 1.9953\times10^{-6}$ while the corresponding Figure 4.14 marker
reads $10^{-5.49} = 3.2359\times10^{-6}$, a gap of $0.21$ decades against a
$0.1$-decade read precision. The dataset keeps the printed table value in
`value` and `value_log10` and the pixel read in `figure_crosscheck_log10`, and
corrects neither against the other. This comparison tests the dataset's primary
`value` field as committed and selects neither reading. The verdict at this
abscissa does not depend on that choice: the measured interval
$[8.39\times10^{-6}, 1.25\times10^{-5}]$ excludes the table value and the figure
cross-read alike.

**Measured points outside their acceptance window.** The 2.22, 3.01, 3.98, and
4.56 dB order-2 points reject their published values under the REQ-02
predicate. They are recorded here with the contradiction rather than reconciled,
and the campaign controls that produced them are unchanged.

**A bound compared as if it were a measurement.** The dataset marks the 5.23 dB
order-2 entry `value_kind = union_bound` and states that bound rows are not
simulation evidence. The predicate is evaluated mechanically for that row and
reported above for completeness, but its verdict there carries no claim about
reproducing a published measurement. The measured interval
$[1.52\times10^{-7}, 2.29\times10^{-7}]$ lies entirely below the tabulated
bound $2.5119\times10^{-7}$, which is the relation an upper bound and a
measurement are expected to have; the predicate nonetheless reports the row as
outside, because it tests membership of a published point rather than the
inequality a bound asserts.

**Recorded digitization precision carries decade units.** The dataset records
$\delta$ in $\log_{10}$ decades (`digitization_uncertainty_log10`), while the
predicate compares it linearly against a probability. Only cells with a nonzero
$\delta$ are affected: the 1.55 dB order-2 cell and every order-1 control cell.
Recomputing the 1.55 dB verdict with $\delta$ applied in decades accepts as
well, and every other order-2 cell records $\delta = 0$, where the linear and
logarithmic forms of the test agree exactly. No verdict in this record depends
on the interpretation.

**Unverified article-level pin.** The reproduction target is the 1994
dissertation. [Fossorier1995] is closed access and unverified, and no claim here
derives from it.

## Citations

- [Fossorier1994] Fossorier — *Decoding of Linear Block Codes Based on Ordered
  Statistics*. Ph.D. dissertation, University of Hawai'i at Manoa, December
  1994. UMI 9519442.
- [Fossorier1995] Fossorier, Lin — *Soft-Decision Decoding of Linear Block Codes
  Based on Ordered Statistics*. IEEE Trans. Inf. Theory 41(5):1379–1396, 1995.
  Closed access and unverified: the committed access audit records a closed
  Unpaywall result, and the advertised full-text lead resolves to a different
  five-author 1999 work.
