# eBCH(128,64) ordered-statistics decoding campaign

This directory holds one complete OSD reproduction campaign and its versioned
evidence. The scientific target is the order-2 bit-error-rate curve of the
extended BCH code $(128, 64, 22)$ on BI-AWGN with BPSK at rate $1/2$ published
in [Fossorier1994]. Order 1 is an internal control of this campaign, not a
reproduction target.

| Path | Role |
| --- | --- |
| `ebch_osd_awgn.json` | Versioned statistical receipt; the authoritative record |
| `ebch_osd_awgn.checkpoint.json` | Durable resumable progress, one entry per cell attempt |
| `README.md` | This provenance record |

Every number below is a projection of `ebch_osd_awgn.json`, with one labelled
exception: the host CPU identity under [Provenance](#provenance), which the
receipt's schema cannot carry. The receipt is the source of truth for seeds,
counts, intervals, work counters, and provenance. All fourteen cells hold a
completed result and the campaign termination is `completed`.

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
| 5.23 | 2.51e-07 | Table 4.7 (bound) | 0 | 8066414 | 516250496 | 111 | 2.15e-07 | [1.77e-07, 2.59e-07] | accept |

Under the recorded interval, four of the six abscissas whose published entry is
a simulation reject their published value. Those measurements stand as recorded;
no seed, error target, tie policy, or other control was changed in response to
them. The deviation is not one-signed: at 3.01 dB the measured interval lies
below the published value, and at 2.22, 3.98, and 4.56 dB it lies above, with
the largest separation at 4.56 dB.

The recorded interval assumes independent bit trials, which this campaign's
sampling violates. [Interval width and correlated bit
errors](#interval-width-and-correlated-bit-errors) shows that three of those
four rejections do not survive an interval computed over independent blocks,
and that the 4.56 dB rejection does.

## Order-1 internal control

These cells are measurements this campaign produces as an internal control on
the decoder, channel, and interval machinery across the same abscissas. They are
not externally sourced claims, and no published value is asserted or tested for
them.

| $E_b/N_0$ (dB) | blocks $n$ | sampled bits | bit errors | BER | 95% Clopper-Pearson $[L,U]$ | BLER | status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1.55 | 15 | 960 | 104 | 1.08e-01 | [8.94e-02, 1.30e-01] | 4.67e-01 | completed |
| 2.22 | 76 | 4864 | 109 | 2.24e-02 | [1.84e-02, 2.70e-02] | 1.05e-01 | completed |
| 3.01 | 74 | 4736 | 111 | 2.34e-02 | [1.93e-02, 2.82e-02] | 9.46e-02 | completed |
| 3.47 | 288 | 18432 | 101 | 5.48e-03 | [4.47e-03, 6.65e-03] | 2.78e-02 | completed |
| 3.98 | 571 | 36544 | 103 | 2.82e-03 | [2.30e-03, 3.42e-03] | 1.23e-02 | completed |
| 4.56 | 10711 | 685504 | 113 | 1.65e-04 | [1.36e-04, 1.98e-04] | 8.40e-04 | completed |
| 5.23 | 48166 | 3082624 | 104 | 3.37e-05 | [2.76e-05, 4.09e-05] | 1.66e-04 | completed |

The control's point estimates are not monotone in $E_b/N_0$: its 3.01 dB BER
exceeds its 2.22 dB BER, where a lower error rate at higher $E_b/N_0$ is the
unambiguous physical expectation. The two recorded intervals overlap almost
entirely, so the campaign does not resolve these two abscissas apart at all,
even though the corresponding published order-1 values differ by a factor of
about three. Those cells stop after 76 and 74 blocks respectively. The control
therefore exhibits the interval-width defect described below, in the arm where
the expected ordering is known independently of any published number.

The control cells follow every order-2 cell in the pinned grid, and an
invocation returns at its first non-terminal cell, so a bounded invocation
reaches the controls only once every order-2 cell holds a terminal result. The
most expensive target cell therefore gates the whole control arm, which is
tracked as `fd1f39e0`.

## Interval width and correlated bit errors

Every cell in this campaign stops on a cumulative count of 100 information-bit
errors, and every cell reaches that count from between seven and nine failed
blocks, each failed block contributing roughly twelve to sixteen bit errors. A
Clopper-Pearson interval over sampled bits treats those bits as independent
trials. They are not: a block either decodes correctly and contributes no
errors, or fails and contributes a burst. The independent sampling unit is the
block, so the effective sample size behind each BER estimate is the block-error
count, not the bit-error count.

The consequence is that every recorded BER interval is far narrower than the
evidence supports. Each spans about $\pm 19\%$ of its point estimate, the
precision a hundred independent trials would give, while the seven to nine
independent failures actually observed support roughly $\pm 70\%$.

The column below labelled block-resolved rescales the receipt's own BLER
Clopper-Pearson interval, which is computed over blocks and is therefore valid,
by the observed mean bit errors per failed block. It uses only committed receipt
fields and no new sampling.

| $E_b/N_0$ (dB) | block errors | bit errors per failed block | recorded bit-level $[L,U]$ | block-resolved $[L,U]$ | recorded verdict | block-resolved verdict |
| --- | --- | --- | --- | --- | --- | --- |
| 1.55 | 8 | 13.4 | [2.54e-02, 3.73e-02] | [1.38e-02, 5.67e-02] | accept | accept |
| 2.22 | 7 | 14.7 | [8.94e-03, 1.33e-02] | [4.45e-03, 2.20e-02] | **outside** | accept |
| 3.01 | 9 | 12.2 | [5.91e-04, 8.67e-04] | [3.29e-04, 1.36e-03] | **outside** | accept |
| 3.47 | 9 | 11.3 | [2.00e-04, 2.98e-04] | [1.12e-04, 4.66e-04] | accept | accept |
| 3.98 | 9 | 12.0 | [5.17e-05, 7.61e-05] | [2.88e-05, 1.20e-04] | **outside** | accept |
| 4.56 | 7 | 14.3 | [8.39e-06, 1.25e-05] | [4.14e-06, 2.12e-05] | **outside** | **outside** |
| 5.23 | 9 | 12.3 | [1.77e-07, 2.59e-07] | [9.83e-08, 4.08e-07] | accept | accept |

The REQ-02 verdicts of record remain the recorded ones, because the criterion
names the receipt interval at its recorded method and level. This section does
not replace them; it identifies which of them are artifacts of the independence
assumption. The 2.22, 3.01, and 3.98 dB rejections are such artifacts. The
4.56 dB rejection is not: the published $1.995\times10^{-6}$ stays below the
block-resolved lower endpoint $4.14\times10^{-6}$ by a factor of about two, so
that abscissa is this campaign's one robust disagreement with the published
curve.

Resolving this needs a stopping rule counting block errors, or a clustered
interval for BER, in the producing tool. Neither is applied here, because that
would change committed measurement behavior under an issue that does not own it.
Tracked as `8a908f79`. The recorded BLER intervals are unaffected, since they
are computed over blocks.

## Cell status and resumption

| cell | order | $E_b/N_0$ (dB) | attempts | blocks | bit errors | state |
| --- | --- | --- | --- | --- | --- | --- |
| `order-2-point-00` | 2 | 1.55 | 1 | 54 | 107 | completed |
| `order-2-point-01` | 2 | 2.22 | 1 | 147 | 103 | completed |
| `order-2-point-02` | 2 | 3.01 | 1 | 2390 | 110 | completed |
| `order-2-point-03` | 2 | 3.47 | 1 | 6486 | 102 | completed |
| `order-2-point-04` | 2 | 3.98 | 2 (interrupted, completed) | 26757 | 108 | completed |
| `order-2-point-05` | 2 | 4.56 | 1 | 151600 | 100 | completed |
| `order-2-point-06` | 2 | 5.23 | 3 (interrupted, interrupted, completed) | 8066414 | 111 | completed |
| `order-1-point-00` | 1 | 1.55 | 1 | 15 | 104 | completed |
| `order-1-point-01` | 1 | 2.22 | 1 | 76 | 109 | completed |
| `order-1-point-02` | 1 | 3.01 | 1 | 74 | 111 | completed |
| `order-1-point-03` | 1 | 3.47 | 1 | 288 | 101 | completed |
| `order-1-point-04` | 1 | 3.98 | 1 | 571 | 103 | completed |
| `order-1-point-05` | 1 | 4.56 | 1 | 10711 | 113 | completed |
| `order-1-point-06` | 1 | 5.23 | 1 | 48166 | 104 | completed |

Resume with the same command and the same checkpoint path:

```console
$ ./target/release/ebch_osd_awgn_campaign \
    --checkpoint dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.checkpoint.json \
    --receipt dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.json \
    --max-samples N
```

The protocol skips every cell holding a terminal result and continues an
interrupted cell from its durable counters, so completed work is never repeated.
Each attempt stays in the receipt's `cell_results` history, so an interrupted
attempt remains visible beside the result that supersedes it; `order-2-point-04`
records one such attempt and `order-2-point-06` records two.

Three properties bound a resume. The evaluator replays a resumed cell's durable
prefix through the same decoder to re-advance its ChaCha20 stream, so the cost
of continuing a cell includes re-running the samples it already holds. At
`order-2-point-06` that replay was 8,000,000 blocks, and it dominated the final
invocation. Tracked as `f7844d2d`.

The remaining two follow from `OsdCampaign::config_hash`, which covers the whole
validated campaign including its runtime provenance, and which the checkpoint
reader compares exactly. It covers `provenance.runtime.git_revision`, so a
resume is accepted only while the repository HEAD equals the revision recorded
in the checkpoint; at any later revision the checkpoint is refused with a
configuration-hash mismatch. It also covers `provenance.runtime.binary_sha256`,
the SHA-256 of the running executable, so the resume requires the exact binary
that produced the checkpoint. Rebuilding is not a substitute: a rebuild that
differs in any byte, including one from a different absolute worktree path,
produces a different digest and is refused. Verify the executable before
resuming:

```console
$ sha256sum target/release/ebch_osd_awgn_campaign
ae0ecf0c6994053e4aa9c43b12e05c52f8c200022304ac3e19beaedb872ed1bd
```

Because the recorded revision precedes the commits carrying these files, the
campaign's later invocations ran from a detached checkout of that revision with
the checkpoint and receipt restored into the working tree as untracked paths.
Untracked paths under `dev/` do not affect `deps_source_dirty`, which is
observed over `crates/` and `Cargo.lock` alone, so the configuration hash still
matched. A future resume follows the same procedure. That the hash gates
resumption on provenance context which does not affect resumed results is
tracked as `9af52659`.

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
producer records no specific CPU model, which is tracked as `14029b3f`.

Because the receipt's own field cannot carry it, the campaign operator records
the host identity observed with `lscpu` during this campaign: an AMD Ryzen 9
5900X, twelve cores and twenty-four threads on one socket. This sentence is
operator-recorded context rather than a projection of the receipt, and the work
counters the receipt records are the machine-independent evidence; wall-clock
cost is not a claim of this artifact set.

## Contradictory evidence

**Table-versus-figure contradiction at 4.56 dB, order 2.** The committed dataset
preserves an unreconciled disagreement at this abscissa: Table 4.7 prints
$10^{-5.7} = 1.9953\times10^{-6}$ while the corresponding Figure 4.14 marker
reads $10^{-5.49} = 3.2359\times10^{-6}$, a gap of $0.21$ decades against a
$0.1$-decade read precision. The dataset keeps the printed table value in
`value` and `value_log10` and the pixel read in `figure_crosscheck_log10`, and
corrects neither against the other. This comparison tests the dataset's primary
`value` field as committed and selects neither reading. The verdict at this
abscissa does not depend on that choice: the recorded interval
$[8.39\times10^{-6}, 1.25\times10^{-5}]$ excludes the table value and the figure
cross-read alike, and the wider block-resolved interval
$[4.14\times10^{-6}, 2.12\times10^{-5}]$ excludes both as well.

**A measured point outside its acceptance window under either interval.** The
4.56 dB order-2 point rejects its published value whether the interval is the
recorded bit-level one or the block-resolved one. It is recorded with the
contradiction rather than reconciled, and the campaign controls that produced it
are unchanged.

**Rejections that do not survive a valid interval.** The 2.22, 3.01, and 3.98 dB
order-2 points reject their published values under the recorded interval and
accept them under the block-resolved one. Both verdicts are reported above; the
recorded verdict is the one REQ-02 names, and it is not evidence of a physical
discrepancy at those three abscissas.

**Recorded BER intervals understate uncertainty.** Every cell reaches its
100-bit-error stopping rule from seven to nine failed blocks, so the recorded
Clopper-Pearson BER intervals are roughly a factor of three too narrow. This is
a property of the producing tool's stopping rule and interval choice, recorded
here rather than corrected, and tracked as `8a908f79`. The order-1 control's
non-monotone 2.22/3.01 dB pair is the clearest symptom.

**A bound compared as if it were a measurement.** The dataset marks the 5.23 dB
order-2 entry `value_kind = union_bound` and states that bound rows are not
simulation evidence. The predicate is evaluated mechanically for that row and
reported above for completeness, but its verdict there carries no claim about
reproducing a published measurement. The recorded interval
$[1.77\times10^{-7}, 2.59\times10^{-7}]$ contains the tabulated bound
$2.5119\times10^{-7}$ and the point estimate lies below it, which is the
relation an upper bound and a measurement are expected to have.

**Recorded digitization precision carries decade units.** The dataset records
$\delta$ in $\log_{10}$ decades (`digitization_uncertainty_log10`), while the
predicate compares it linearly against a probability. Only cells with a nonzero
$\delta$ are affected: the 1.55 dB order-2 cell and every order-1 control cell.
Recomputing the 1.55 dB verdict with $\delta$ applied in decades accepts as
well, and every other order-2 cell records $\delta = 0$, where the linear and
logarithmic forms of the test agree exactly. No verdict in this record depends
on the interpretation. Tracked as `cf37be3b`.

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
