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
| `host-lscpu.txt` | Unedited `lscpu` output identifying the producing host |
| `plot.py` | Deterministic generator of `comparison.json` and both figures over the receipt and the reference dataset |
| `comparison.json` | Machine-readable per-cell comparison; the single derivation point the tables below project |
| `order2_ber_comparison.png`, `.svg` | Order-2 simulation BER against the published order-2 series |
| `order1_controls.png`, `.svg` | Order-1 internal-control BER and BLER series, with the published order-1 BER as a cross-check |
| `schema1/` | Superseded schema-1 artifact set, retained as historical evidence |
| `README.md` | This provenance record |

The receipt and checkpoint are **schema version 2**
(`schema_version`; `OSD_CAMPAIGN_SCHEMA_VERSION` at
`crates/gf2-sim/src/osd_campaign.rs:121`). Every number below is a projection of
`ebch_osd_awgn.json`, of [`comparison.json`](comparison.json) — which `plot.py`
derives from that receipt and the dataset — of the committed reference dataset
[`osd_ebch_128_64_fossorier1994.csv`](../../reference_data/osd_ebch_128_64_fossorier1994.csv),
or of a cited in-tree source location. The single exception is the host identity
under [Provenance](#provenance), a fact the receipt's schema cannot carry, which
is committed beside the receipt as `host-lscpu.txt`.

All fourteen cells hold a completed result, every completed cell holds exactly
100 block errors, and the campaign termination is `completed`
(`termination.state`).

## What was run

The producer is `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`, built as the
`gf2-sim` release binary `ebch_osd_awgn_campaign` and driven by the reusable
protocol in `gf2_sim::osd_campaign`.

One invocation produced the whole campaign. The receipt's `invocation_history`
holds a single entry with `first_attempt_index = 0` and `attempt_count = 14`,
and every entry of `cell_results` carries `invocation_index = 0`. The complete
argument vector is recorded at `provenance.runtime.invocation`:

```console
$ ./target/release/ebch_osd_awgn_campaign \
    --checkpoint dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.checkpoint.json \
    --receipt dev/simulation_results/osd-ebch-128-64/ebch_osd_awgn.json \
    --target-block-errors 100 \
    --max-samples 200000000 \
    --workers 24
```

The pinned grid is the seven [Fossorier1994] abscissas
$E_b/N_0 \in \{1.55, 2.22, 3.01, 3.47, 3.98, 4.56, 5.23\}$ dB, instantiated
twice: seven order-2 target cells and seven order-1 control cells (`cells`). The
campaign root seed is `0xC8322EFF` (`campaign_seed` $= 3358732031$), and each
cell's stream seed is derived from that root and the cell's stable identity
through domain-separated BLAKE3 (`derive_cell_seed`,
`crates/gf2-sim/src/osd_campaign.rs:1081`), so grid order cannot change a cell's
samples. A cell completes at its 100th block error
(`target_block_errors` $= 100$).

The decoder reprocesses in increasing Hamming weight over the 64
most-reliable-independent positions, lexicographically ascending within a
weight. The source leaves reliability ties and equal-distance ties undefined:
this implementation breaks a reliability tie by ascending original coordinate
index and retains the first generated candidate at an equal metric
(`crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs:24-30`). Those are
implementation decisions, not source claims.

The campaign binary evaluates a cell's blocks on many workers (`258be082`).
`--workers` changes throughput alone: a block's outcome is a pure function of
its cell, cell seed, and block index, and the protocol commits worker outcomes
in block-index order, so for a fixed campaign and seed every counter, the
stopping index, and the receipt payload are byte-identical across worker counts,
with the single-worker run as the reference
(`crates/gf2-sim/src/osd_campaign.rs:12-40`, `@/inv/deterministic-seeded-execution`).
The worker count is invocation-local: it is excluded from the campaign
configuration identity and reaches the receipt only through the recorded
argument vector. That invariance is witnessed end to end on this evaluator by
the committed benchmark receipt
[`2026-08-27-258be082-osd-campaign-worker-scaling.md`](../../bench_results/2026-08-27-258be082-osd-campaign-worker-scaling.md),
where all twelve trials at 1, 2, 8, and 24 workers reduce to the single cell
evidence hash `8c95f21e…` and 24 workers run $12.40\times$ faster than one
(`@/inv/benchmark-backed-performance`).

### Provenance

Each row names the receipt field it projects.

| Field | Value | Receipt path |
| --- | --- | --- |
| Receipt schema version | `2` | `schema_version` |
| Repository revision at run start | `c7bfcbce47b3fee430ad23434b2f6596f5bc27a2` | `provenance.runtime.git_revision` |
| Source-closure revision | `bd32921683317a762cbd337cd47e160ffe3e3304` | `provenance.runtime.deps_source_revision` |
| Source closure dirty | `false` | `provenance.runtime.deps_source_dirty` |
| Producing executable | `b1015246dfff6a4459921bfc658fee257a4e542bbf439e2f5000924d64ea7f86` | `provenance.runtime.binary_sha256` |
| Toolchain | `rustc 1.97.0 (2d8144b78 2026-07-07)` | `provenance.runtime.compiler_version` |
| RNG | `cha_cha20`, `rand_chacha 0.3.1 (gf2-coding BPSK channel ABI)` | `provenance.runtime.rng_algorithm`, `.rng_version` |
| Processor | `AMD Ryzen 9 5900X 12-Core Processor`, 12 physical cores, 24 logical threads | `provenance.runtime.cpu_model`, `.cpu_physical_cores`, `.cpu_logical_threads` |
| GPU and accelerator runtime | `not_present` | `provenance.runtime.gpu_model`, `.accelerator_runtime` |
| Comparison dataset identity | `dev/reference_data/osd_ebch_128_64_fossorier1994.md`, SHA-256 `acacfaa6…` | `provenance.configuration` |
| Measurement behavior identity | `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs`, SHA-256 `a2363e80…` | `provenance.measurement_behavior` |
| Configuration hash | `blake3:add07016…` | `configuration_hash` |

The revision is observed at run start and recorded as context rather than as the
dataset's identity (`crates/gf2-sim/src/permanent_campaign/schema.rs:623-625`),
so it names the revision the campaign ran at rather than the commit that carries
these files. The source-closure revision is the revision of `crates/` and
`Cargo.lock` (`schema.rs:630-633`).

The receipt's `cpu_model` names the processor but not the host. The host
identity is committed beside the receipt as [`host-lscpu.txt`](host-lscpu.txt),
the unedited output of `lscpu` on the machine that ran this campaign. It is
operator-captured rather than emitted by the producer, so it carries no digest
inside the receipt. The work counters the receipt records are the
machine-independent evidence, and wall-clock cost is not a claim of this
artifact set.

## Order-2 comparison against the published curve

Published values are the committed dataset
[`osd_ebch_128_64_fossorier1994.csv`](../../reference_data/osd_ebch_128_64_fossorier1994.csv)
and its provenance record
[`osd_ebch_128_64_fossorier1994.md`](../../reference_data/osd_ebch_128_64_fossorier1994.md);
nothing is re-digitized here. The column $p$ is that dataset's primary `value`
field, and $\delta$ is the digitization precision the receipt records for the
cell (`cells[].digitization_precision`, in units of
`digitization_precision_unit = log10_decades`). For all seven matched order-2
points the receipt's $\delta$ equals the dataset's own
`digitization_uncertainty_log10`.

The interval $[L, U]$ is the receipt's `ber_confidence_interval` for the cell.
Every completed cell records it under estimator `block_ratio_product_interval`
at level $0.95$ with `sampling_unit = block`. A point accepts its published
value exactly when

$$p \in \left[L \cdot 10^{-\delta},\; U \cdot 10^{+\delta}\right],$$

which is the predicate `gf2_sim::osd_campaign::accepts_published_value`
(`crates/gf2-sim/src/osd_campaign.rs:1105-1131`). The gap column is
$\log_{10}(\widehat{\text{BER}} / p)$ and carries no verdict; it is reported so
the strength of each agreement is visible independently of the accept/reject
outcome (`@/inv/uncertainty-reported`).

The table projects [`comparison.json`](comparison.json), which `plot.py` derives
from the receipt and the dataset in one pass so that the prose, the figures, and
the machine-readable comparison have a single derivation point rather than three
independent transcriptions. Its columns are that file's `ber`, `ber_interval`,
`published.value`, `published.source_locator`,
`published.digitization_precision_log10_decades`, `accept_window`, `log10_gap`,
and `accepts` fields, for the entries it marks `role = reproduction_target`.

| $E_b/N_0$ (dB) | BER estimate | receipt $[L, U]$ | published $p$ | source | $\delta$ | accept window $[L \cdot 10^{-\delta}, U \cdot 10^{+\delta}]$ | gap (decades) | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1.55 | 2.367e-02 | [7.776e-03, 4.596e-02] | 2.138e-02 | Fig. 4.14 | 0.1 | [6.177e-03, 5.787e-02] | +0.044 | accept |
| 2.22 | 5.985e-03 | [1.814e-03, 1.194e-02] | 6.310e-03 | Table 4.7 | 0 | [1.814e-03, 1.194e-02] | -0.023 | accept |
| 3.01 | 1.205e-03 | [3.702e-04, 2.399e-03] | 1.000e-03 | Table 4.7 | 0 | [3.702e-04, 2.399e-03] | +0.081 | accept |
| 3.47 | 2.685e-04 | [7.336e-05, 5.494e-04] | 2.512e-04 | Table 4.7 | 0 | [7.336e-05, 5.494e-04] | +0.029 | accept |
| 3.98 | 4.237e-05 | [1.178e-05, 8.638e-05] | 3.981e-05 | Table 4.7 | 0 | [1.178e-05, 8.638e-05] | +0.027 | accept |
| 4.56 | 5.347e-06 | [1.486e-06, 1.091e-05] | 1.995e-06 | Table 4.7 | 0 | [1.486e-06, 1.091e-05] | **+0.428** | accept |
| 5.23 | 3.016e-07 | [7.889e-08, 6.227e-07] | 2.512e-07 | Table 4.7 (bound) | 0 | [7.889e-08, 6.227e-07] | +0.079 | accept |

All seven matched order-2 points accept their published value. Six of the seven
agree to within $0.081$ decades. The seventh, at 4.56 dB, sits $+0.428$ decades
away — a gap $5.3\times$ the next largest in the series — and is examined below.

The 5.23 dB entry is marked `value_kind = union_bound` in the dataset, which
records that bound rows are analytical bound values rather than stochastic
measurements and are not simulation evidence
(`dev/reference_data/osd_ebch_128_64_fossorier1994.md:184-186`, `:256-258`). The
predicate is evaluated mechanically for that row for completeness, and its
verdict carries no claim about reproducing a published measurement.

At that row the point estimate $3.016\times10^{-7}$ lies **above** the tabulated
value $2.512\times10^{-7}$ by $0.079$ decades, a factor of $1.20$, and the
recorded interval $[7.889\times10^{-8},\, 6.227\times10^{-7}]$ contains that
value, so this campaign does not separate the two at $K = 100$ block errors.
What the ordering signifies is a separate question, and the committed evidence
does not settle it. Beyond the row's kind, all the dataset records is the source
caption that assigns it: Tables 4.7–4.9 are captioned "Order-$l$ simulation
results for (128,64,22) extended BCH code (\*: union bound)" (`:44-45`). Neither
which error probability the bound upper-bounds nor the decoding rule it is
derived for is recorded anywhere. The digitization receipts add nothing on
either point: they cover the Figure 4.14 marker reads and their axis
calibration, and no receipt field names the bound
(`dev/reference_data/osd_ebch_128_64_fossorier1994_digitization/README.md:39-65`).
The dataset does record an unresolved ambiguity in the source's neighbouring
analytical quantities: Figure 4.14's theoretical curves are labelled
"Eq. 4.46", which defines the codeword error probability $P_s(i)$, while §4.3.1
states that the simulated results are compared against Equation 4.47, the bit
error bound $P_b(i)$. That label is left as the source has it (`:414-418`).

Lacking those two facts, the ordering is **not established as a contradiction**.
One reading the committed evidence does not exclude is that the bound applies to
maximum-likelihood decoding. Order-$l$ reprocessing minimizes squared Euclidean
distance over the $\sum_{i=0}^{l}\binom{K}{i}$ candidates its enumeration
reaches, a subset of the code (`:154-162`), so its error probability is at least
that of the unrestricted minimum-distance rule.

That reading fixes the decoding rule and leaves the error quantity open, so it
does not by itself make the ordering the relation to expect. An ordering
argument needs both halves: the bound and the estimate must concern the same
error probability. This campaign measures a bit error rate, while the source's
neighbouring analytical quantities are the codeword error probability
$P_s(i)$ of Equation 4.46 and the bit error bound $P_b(i)$ of Equation 4.47,
whose attribution the dataset records as unresolved. A bound on codeword error
probability is not comparable to a bit error rate at all: at this cell the two
differ by a factor of $5.05$, the campaign's own BLER $1.524\times10^{-6}$
against its BER $3.016\times10^{-7}$, and $0.70$ decades dwarfs the $0.079$
decade gap under discussion. So even under the maximum-likelihood reading the
ordering is established only if the bound is a bit-error bound, which the
record does not say.

That reading has recorded evidence against it. The dataset's `union_bound` rows
are order-indexed: order 2 carries one only at 5.23 dB, while orders 3 and 4
carry one at each of 3.98, 4.56, and 5.23 dB, and at a shared abscissa their
values differ by orders of magnitude. At 5.23 dB the order-2 row reads
$2.5119\times10^{-7}$, the order-3 row $1.2589\times10^{-9}$, and the order-4
row $5.0119\times10^{-12}$
(`dev/reference_data/osd_ebch_128_64_fossorier1994.csv:26`, `:33`, `:40`,
transcribed from Tables 4.7, 4.8, and 4.9). A union bound on maximum-likelihood
decoding of this code is a property of the code and the channel rather than of a
reprocessing order, so it would take one value per abscissa. These take three.

That tells against the maximum-likelihood reading without putting anything in
its place. The dataset still records neither the bound's derivation nor its
decoding rule. An order-indexed bound could bound order-$l$ reprocessing
performance itself, under which the 5.23 dB ordering would regain the
significance the maximum-likelihood reading denies it, and in the direction the
anomaly reading assumed; or it could be a third quantity this dataset does not
describe. The question stays undecidable on the committed evidence.

So neither verdict is established. The ordering is not recorded as a
contradiction, and it is not explained away either: the one concrete reading
under which it would be unremarkable is itself one the dataset's order-indexed
bound rows tell against. This record selects no reading. It states the
observation, states the gaps in the evidence that leave the ordering's
significance undecidable, and reconciles nothing
(`@/inv/falsification-preserved`).

The supporting counters for each order-2 cell are below. Every cell stops at its
100th block error and holds 64 information bits per block
(`sampled_bits / samples` $= 64$ for all fourteen cells).

| cell | $E_b/N_0$ (dB) | blocks $n$ | sampled bits | bit errors | block errors | BLER | BLER $[L, U]$ |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `order-2-point-00` | 1.55 | 971 | 62144 | 1471 | 100 | 1.030e-01 | [8.217e-02, 1.259e-01] |
| `order-2-point-01` | 2.22 | 3590 | 229760 | 1375 | 100 | 2.786e-02 | [2.205e-02, 3.436e-02] |
| `order-2-point-02` | 3.01 | 18070 | 1156480 | 1393 | 100 | 5.534e-03 | [4.371e-03, 6.844e-03] |
| `order-2-point-03` | 3.47 | 75540 | 4834560 | 1298 | 100 | 1.324e-03 | [1.045e-03, 1.638e-03] |
| `order-2-point-04` | 3.98 | 482406 | 30873984 | 1308 | 100 | 2.073e-04 | [1.636e-04, 2.565e-04] |
| `order-2-point-05` | 4.56 | 3848227 | 246286528 | 1317 | 100 | 2.599e-05 | [2.051e-05, 3.216e-05] |
| `order-2-point-06` | 5.23 | 65596038 | 4198146432 | 1266 | 100 | 1.524e-06 | [1.203e-06, 1.886e-06] |

BLER endpoints are the receipt's `bler_confidence_interval`, recorded under
estimator `negative_binomial_clopper_pearson` at level $0.975$ with
`sampling_unit = block` — the component level of the composed BER interval, not
the campaign's requested level. See [Statistical
contract](#statistical-contract).

## The 4.56 dB order-2 point

At 4.56 dB the campaign estimates
$\widehat{\text{BER}} = 5.347\times10^{-6}$ from 3,848,227 blocks and 100 block
errors, against the Table 4.7 value $p = 1.995\times10^{-6}$: a gap of $+0.428$
decades, a factor of $2.68$. The point accepts only because the recorded
interval $[1.486\times10^{-6},\, 1.091\times10^{-5}]$ is wide enough to reach
down to the published value; $p$ clears the lower endpoint by a factor of
$1.34$. Acceptance here is a statement about the campaign's resolving power, not
a demonstration of agreement.

This is the abscissa the committed dataset already records as internally
contradictory, tracked as `PIT-07`. Table 4.7 prints $\log_{10} P_e = -5.70$,
i.e. $1.9953\times10^{-6}$, while the corresponding Figure 4.14 marker reads
$-5.49$, i.e. $3.2359\times10^{-6}$ — a separation of $0.210$ decades against a
$\pm 0.1$ decade read precision. The dataset keeps the printed table value in
`value` and `value_log10` and the pixel read in `figure_crosscheck_log10`, and
corrects neither against the other. The four other order-2 abscissas carrying a
figure cross-read agree within $0.11$ decades.

The simulation sits closer to the figure reading than to the table entry. Its
gap against the figure cross-read is $+0.218$ decades, against $+0.428$ decades
for the table entry.

The local slope structure points the same way. Taking the three highest order-2
abscissas and reading successive ratios, with the published series' 5.23 dB
endpoint being its `union_bound` row rather than a simulation row:

| series at 4.56 dB | 3.98 dB $\rightarrow$ 4.56 dB | 4.56 dB $\rightarrow$ 5.23 dB |
| --- | --- | --- |
| published, Table 4.7 value $1.9953\times10^{-6}$ | $19.95\times$ | $7.94\times$ |
| published, figure cross-read $3.2359\times10^{-6}$ | $12.30\times$ | $12.88\times$ |
| this campaign | $7.92\times$ | $17.73\times$ |

The abscissas are unevenly spaced, so the comparable quantity is the local slope
in decades per dB. Over the published series' six steps from 1.55 dB to 5.23 dB
it runs $0.791,\, 1.013,\, 1.304,\, 1.569,\, 2.241,\, 1.343$: monotonically
steepening across the waterfall, then a step into 4.56 dB that is $43\%$ steeper
than the one before it, then a fall of $40\%$. That sixth step ends on the
`union_bound` row and is carried here as arithmetic over the tabulated numerals
alone; the first limit below states why it bears no weight. Substituting the
figure cross-read at 4.56 dB replaces the last two with $1.879$ and $1.657$,
which continues the series' own progression through the fifth step and leaves a
far smaller dip. This campaign's own order-2 slopes over the same six steps run
$0.891,\, 0.881,\, 1.417,\, 1.572,\, 1.550,\, 1.864$, steepening without a
spike.

Two limits on that argument:

- The 5.23 dB published entry is a `union_bound` row rather than a `simulation`
  row, and the dataset records neither which error probability it bounds nor the
  decoding rule it is derived for. The final step's $7.94\times$ ratio and
  $1.343$ decades per dB are therefore not a simulation-to-simulation step of
  the published series, and no direction of bias can be assigned to them. The
  flattening after 4.56 dB may be an artifact of that row's kind; this record
  neither claims it is nor claims it is not. The load-bearing part of the
  argument does not use that step: 3.98 dB and 4.56 dB are both `simulation`
  rows, and the step between them is where the table and figure readings differ
  by $19.95\times$ against $12.30\times$.
- Table 4.7 prints $P_e$ as $10^{-x}$ with one decimal in the exponent, so each
  printed entry carries the source's own $\pm 0.05$ decade quantization
  (`dev/reference_data/osd_ebch_128_64_fossorier1994.md:182-184`). A ratio of
  two printed entries therefore carries up to $0.1$ decade, a factor of $1.26$.
  The table-versus-figure difference on the 3.98–4.56 step is $0.21$ decades,
  above that.

What this establishes and what it does not:

- It is evidence bearing on **which of two contradictory published readings is
  internally self-consistent**. The figure cross-read is the one that leaves the
  published order-2 series smooth and the one the simulation sits nearer.
- It is **not a correction of the source**. This record selects neither reading,
  and no value in the committed dataset is changed. The contradiction stays
  recorded as a contradiction (`@/inv/falsification-preserved`).
- It is **not a resolved disagreement**. Against either reading the simulation
  is high, by $0.428$ decades against the table and $0.218$ decades against the
  figure. Both are inside the recorded interval, so this campaign does not
  separate them.
- The comparison of record remains the one the acceptance predicate names: the
  dataset's primary `value` field, which is the table entry.

The direction of the residual and the fact that this abscissa is the series
outlier are recorded here rather than absorbed by adjusting a seed, the block-
error target, the tie policy, or any other campaign control. No such control was
changed in response to any measurement in this campaign.

## Order-1 internal controls

These seven cells are measurements this campaign produces as an internal control
on the decoder, channel, seed derivation, and interval machinery across the same
abscissas. **They are campaign results, not externally sourced claims.** The
campaign asserts no published value for them, and the acceptance predicate of
the previous section is not evaluated for them:
[`comparison.json`](comparison.json) marks every order-1 entry
`role = internal_control` and emits no `accepts` field for any of them.

| $E_b/N_0$ (dB) | blocks $n$ | sampled bits | bit errors | block errors | BER | BER $[L, U]$ | BLER | BLER $[L, U]$ |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1.55 | 359 | 22976 | 1454 | 100 | 6.328e-02 | [2.110e-02, 1.203e-01] | 2.786e-01 | [2.267e-01, 3.331e-01] |
| 2.22 | 688 | 44032 | 1415 | 100 | 3.214e-02 | [9.994e-03, 6.301e-02] | 1.453e-01 | [1.165e-01, 1.768e-01] |
| 3.01 | 2293 | 146752 | 1400 | 100 | 9.540e-03 | [2.946e-03, 1.892e-02] | 4.361e-02 | [3.458e-02, 5.370e-02] |
| 3.47 | 5372 | 343808 | 1378 | 100 | 4.008e-03 | [1.196e-03, 8.031e-03] | 1.862e-02 | [1.472e-02, 2.299e-02] |
| 3.98 | 17574 | 1124736 | 1388 | 100 | 1.234e-03 | [3.794e-04, 2.458e-03] | 5.690e-03 | [4.494e-03, 7.037e-03] |
| 4.56 | 77974 | 4990336 | 1356 | 100 | 2.717e-04 | [7.991e-05, 5.472e-04] | 1.282e-03 | [1.012e-03, 1.587e-03] |
| 5.23 | 521106 | 33350784 | 1319 | 100 | 3.955e-05 | [1.112e-05, 8.045e-05] | 1.919e-04 | [1.515e-04, 2.375e-04] |

Both control series are strictly decreasing in $E_b/N_0$, which is the
unambiguous physical expectation and the property the control exists to check.

The committed dataset does carry a digitized order-1 series, the `+: Order 1`
markers of Figure 4.14 at these same seven abscissas, at $\pm 0.1$ decades. Its
provenance record states the constraints on using it: it is a published BER on
the same axes, it is not a BLER control, and any claim a campaign control makes
about published order-1 behaviour is unsupported unless the campaign reproduces
that metric and those conventions
(`dev/reference_data/osd_ebch_128_64_fossorier1994.md:370-388`, "Order-1 control
status"). Set beside those points, the campaign's order-1 BER estimates differ
by $+0.001$, $+0.037$, $-0.010$, $+0.013$, $-0.019$, $-0.006$, and $+0.007$
decades from 1.55 dB upward, every one inside the dataset's own $\pm 0.1$ decade
read precision. That published series is drawn on the order-1 figure beside the
control series and labelled `cross-check only`, which is the whole of its role
here.

That observation licenses one thing and not another. It licenses the statement
that this campaign's decoder, channel, and seed machinery behave consistently
with an independently digitized order-1 series across the $3.2$ decades of BER
that series spans — a useful check on the machinery. It does not license calling
order 1 a
reproduction target, does not make these cells externally sourced values, and
does not assert anything about published order-1 behaviour: the source leaves
the tie policies undefined, so the implementation's choices are unconstrained by
it, and no acceptance verdict is recorded or claimed for any order-1 cell.

## Statistical contract

The authoritative statement of this contract is the module documentation at
`crates/gf2-sim/src/osd_campaign.rs:42-100`. What follows projects it.

**Stopping design.** A cell stops at its $K$-th block error, with
$K = $ `target_block_errors` $ = 100$. The protocol accepts a completed cell only
with exactly $K$ block errors and refuses an evaluator result carrying more or
fewer (`validate_cell_run`, `crates/gf2-sim/src/osd_campaign.rs:1611-1622`). The
rule is enforced structurally rather than by an evaluator's cooperation: workers
evaluate blocks speculatively and out of order, and the protocol commits their
outcomes in block-index order, completing the cell at the smallest block index
whose in-order cumulative block-error count reaches $K$. Outcomes past that
index are discarded and contribute to no counter, so an evaluator cannot choose
termination, report aggregate counters, or skip, reorder, or double-count blocks
(`crates/gf2-sim/src/osd_campaign.rs:21-29`). The total block count $N$ is
therefore a stopping time and the design is inverse-binomial.

**Why the intervals are what they are.** BER factors over that design as
$\text{BER} = \text{BLER} \cdot \mu$, where $\mu$ is the mean information-bit
error fraction of a failing block. For a requested two-sided level
$1 - \alpha = 0.95$, each factor is bounded at the component level
$1 - \alpha/2 = 0.975$ (`OsdIntervalSpec::component_level`,
`crates/gf2-sim/src/osd_campaign.rs:394`):

- **BLER** uses the equal-tailed exact inversion of the inverse-binomial
  sampling distribution, recorded as `negative_binomial_clopper_pearson`
  (`crates/gf2-sim/src/osd_campaign.rs:403-412`). Its endpoints are the ones
  tabulated above, at level $0.975$.
- **$\mu$** uses the Maurer-Pontil empirical-Bernstein bound [MaurerPontil2009]
  over the $K$ per-failing-block error fractions, intersected with the domain
  $[1/k, 1]$ for information-block length $k = 64$
  (`crates/gf2-sim/src/osd_campaign.rs:63-72`, `:338`; the implementation and
  its one-sided coverage statement are at
  `crates/gf2-stats/src/intervals.rs:218-235`). $K$ is fixed by the
  stopping rule, so this fixed-sample bound applies. The bound is
  variance-adaptive, which matters because a failing block's error fraction
  concentrates far below the $[0, 1]$ range a Hoeffding-type bound would have to
  assume: at 4.56 dB, order 2, the observed mean fraction is $0.2058$, about
  $13.17$ bit errors in a 64-bit block.

The recorded BER interval is the endpoint product
$[L_{\text{BLER}} \cdot L_\mu,\; U_{\text{BLER}} \cdot U_\mu]$, recorded as
`block_ratio_product_interval` at level $0.95$
(`crates/gf2-sim/src/osd_campaign.rs:439-452`). All four endpoints are
non-negative, so the product contains BER whenever both factor intervals contain
their factors; **by a union bound over the two failure events** its coverage is
at least $1 - \alpha/2 - \alpha/2 = 1 - \alpha$. That is Boole's inequality over
this construction's own events, unrelated to the source's `union_bound` row
kind. Nothing in the construction assumes independence among the bit errors
inside a block, which is the property a decoder's burst-shaped block failures
violate.

**Why fixed-trial coverage statements do not apply.** A Clopper-Pearson interval
states exact coverage for a fixed trial count with a random success count. Under
this stopping rule the roles are reversed: the block-error count is fixed at $K$
by design and the block count $N$ is the random quantity. A fixed-trial coverage
claim is therefore not the coverage this design delivers, and the receipt names
the inverse-binomial estimator it actually dispatched rather than a
Clopper-Pearson one. The protocol refuses to configure a live campaign with the
fixed-trial method at all — "a block-error stopping rule leaves the block count
a stopping time and requires the inverse-binomial exact interval"
(`validate_campaign`, `crates/gf2-sim/src/osd_campaign.rs:1342-1346`, called
from `OsdCampaign::new` at `:966`). That variant survives only for reading
schema-1 evidence (`crates/gf2-sim/src/osd_campaign.rs:248-253`). The coverage
statement also applies **only to a completed cell**: an attempt that stopped for
any other
reason retains its cumulative counters but records no intervals, because its
block-error count is not the design's fixed $K$
(`crates/gf2-sim/src/osd_campaign.rs:81-85`).

**The intervals are wide, and that is the cost of the budget.** At $K = 100$
block errors the recorded BER interval spans a factor of $5.70$ to $7.89$ end to
end across the fourteen cells: the lower endpoint sits a factor of $3.00$ to
$3.82$ below the point estimate and the upper endpoint a factor of $1.90$ to
$2.06$ above it. The BLER intervals, resting on one factor rather than two, span
a factor of $1.47$ to $1.57$. This is the price of stopping every cell at 100
block errors, and it is the reason the 4.56 dB point accepts a published value
$2.68\times$ away from its estimate. Narrowing it means raising $K$, which
raises sampling cost roughly in proportion; the 5.23 dB order-2 cell alone
already consumed 65,596,038 blocks. Nothing in this record treats the width as
smaller than it is.

**Campaign scope.** This is one seeded campaign. It carries no independent
seeded rerun cross-check, and no statement here rests on one; that gap is
recorded as `NOTE-06` and is not closed by this record.

## Cell status and resumption

| cell | order | $E_b/N_0$ (dB) | attempts | blocks | bit errors | block errors | state |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `order-2-point-00` | 2 | 1.55 | 1 | 971 | 1471 | 100 | completed |
| `order-2-point-01` | 2 | 2.22 | 1 | 3590 | 1375 | 100 | completed |
| `order-2-point-02` | 2 | 3.01 | 1 | 18070 | 1393 | 100 | completed |
| `order-2-point-03` | 2 | 3.47 | 1 | 75540 | 1298 | 100 | completed |
| `order-2-point-04` | 2 | 3.98 | 1 | 482406 | 1308 | 100 | completed |
| `order-2-point-05` | 2 | 4.56 | 1 | 3848227 | 1317 | 100 | completed |
| `order-2-point-06` | 2 | 5.23 | 1 | 65596038 | 1266 | 100 | completed |
| `order-1-point-00` | 1 | 1.55 | 1 | 359 | 1454 | 100 | completed |
| `order-1-point-01` | 1 | 2.22 | 1 | 688 | 1415 | 100 | completed |
| `order-1-point-02` | 1 | 3.01 | 1 | 2293 | 1400 | 100 | completed |
| `order-1-point-03` | 1 | 3.47 | 1 | 5372 | 1378 | 100 | completed |
| `order-1-point-04` | 1 | 3.98 | 1 | 17574 | 1388 | 100 | completed |
| `order-1-point-05` | 1 | 4.56 | 1 | 77974 | 1356 | 100 | completed |
| `order-1-point-06` | 1 | 5.23 | 1 | 521106 | 1319 | 100 | completed |

Totals across the fourteen cells: 70,650,208 sampled blocks, 4,521,613,312
sampled information bits, 145,762,344,992 tested candidates
(`cell_results[].work`).

**No cell in this campaign was interrupted, censored, or budget-exhausted.**
Every one of the fourteen `cell_results` entries carries
`termination.state = completed`, each cell holds exactly one attempt, and
`--max-samples 200000000` was never reached by any cell — the largest, at 5.23
dB order 2, stopped at 65,596,038 blocks. The resume contract below is stated
from the code, not from an event in this run.

The contract (`@/inv/campaign-resumability`):

- An invocation skips every cell already holding a **terminal** result and never
  re-evaluates it. Completed, censored, exhausted, and contradictory results are
  all terminal; only `interrupted` is not
  (`settled_cell_ids`, `crates/gf2-sim/src/osd_campaign.rs:1527`;
  `is_terminal`, `:577`). Completed work is therefore never repeated.
- Reaching `--max-samples` for a pending cell records an **interrupted** attempt
  holding that cell's cumulative counters. A later invocation restores them and
  continues at the next block index under the same cell seed, reaching the
  evidence an uninterrupted run would have
  (`crates/gf2-sim/src/osd_campaign.rs:32-34`, `latest_resume` at `:1384`). The
  bound is consulted without reference to any outcome, so the censoring it
  causes is independent of the sampled values and a resumed completed cell still
  holds exactly $K$ failing blocks drawn under the same design.
- Every attempt stays in the receipt's `cell_results` history, so an interrupted
  attempt remains visible beside the result that supersedes it. Interrupted,
  censored, and exhausted attempts record their counters but no intervals.
- Each attempt is atomically checkpointed before the next cell begins.

Resume with the same command and the same checkpoint path. The protocol
validates the checkpoint's payload identity, schema version, and complete
configuration hash before restoring anything, and refuses on any mismatch
(`crates/gf2-sim/src/checkpoint/mod.rs:396-408`). `OsdCampaign::config_hash`
covers the whole validated campaign including its runtime provenance
(`crates/gf2-sim/src/osd_campaign.rs:1007-1013`), so a resume is gated on two
identities beyond the campaign configuration itself:

- `provenance.runtime.git_revision` — a resume is accepted only while the
  repository HEAD equals `c7bfcbce…`; at any later revision the checkpoint is
  refused with a configuration-hash mismatch.
- `provenance.runtime.binary_sha256` — the resume requires the exact executable
  that produced the checkpoint. A rebuild differing in any byte, including one
  from a different absolute worktree path, produces a different digest and is
  refused. Verify before resuming:

```console
$ sha256sum target/release/ebch_osd_awgn_campaign
b1015246dfff6a4459921bfc658fee257a4e542bbf439e2f5000924d64ea7f86
```

The invocation argument vector is deliberately excluded from the hash, because
`--max-samples` is an invocation-local bound and may differ across resumptions;
every full vector is retained in `invocation_history`
(`crates/gf2-sim/src/osd_campaign.rs:999-1002`).

## Figures and the derived comparison

Both figures are projections of the receipt and the reference dataset, with
error bars on every plotted point (`@/inv/uncertainty-reported`), and each plots
this campaign's estimates against the published values.

- [`order2_ber_comparison.png`](order2_ber_comparison.png)
  ([SVG](order2_ber_comparison.svg)) — order-2 simulation BER with its
  `block_ratio_product_interval` endpoints, against the published order-2
  series with each point's recorded digitization uncertainty as a
  multiplicative error bar.
- [`order1_controls.png`](order1_controls.png)
  ([SVG](order1_controls.svg)) — the order-1 internal-control BER and BLER
  series with their recorded intervals, against the published order-1 BER with
  its recorded digitization uncertainty. The published series is labelled
  `cross-check only` and the title names the panel as not a reproduction target,
  because order 1 is an internal control: no acceptance verdict is computed for
  any order-1 cell.

Each figure is committed in both formats from the same draw. The SVG is the
form the issue tracker accepts as a linked document, since `jit doc add`
requires UTF-8 content; both figures are linked to the issue in that form.

Both figures and [`comparison.json`](comparison.json) are generated
deterministically in one pass by the committed [`plot.py`](plot.py) from
`ebch_osd_awgn.json` and `dev/reference_data/osd_ebch_128_64_fossorier1994.csv`.
Regenerate from the repository root:

```console
$ python3 dev/simulation_results/osd-ebch-128-64/plot.py
```

`--output-dir` redirects the output without changing it. Regenerating into a
scratch directory reproduces every committed output byte for byte:

| file | SHA-256 |
| --- | --- |
| `order2_ber_comparison.png` | `8e3da8afa6308eb3…` |
| `order2_ber_comparison.svg` | `5581d13b1a5acecc…` |
| `order1_controls.png` | `5dcf3a77389038d7…` |
| `order1_controls.svg` | `0ed4346b7580e474…` |
| `comparison.json` | `80a446e10f469cb5…` |

SVG output is deterministic only because the script pins matplotlib's
`svg.hashsalt` and writes no creation date; without both, element identifiers
and the embedded timestamp differ per run.

The script draws only completed cells, because the receipt records intervals for
completed cells alone.

The published order-2 series is labelled from the source locators the CSV
records for the rows actually plotted, so the legend names both Figure 4.14 and
Table 4.7 rather than either alone. Only the 1.55 dB point is an
axis-calibrated figure read; the other six are printed transcriptions from
Table 4.7.

## Relationship to the superseded schema-1 artifact set

[`schema1/`](schema1/) holds a complete earlier campaign over the same grid,
with its own provenance record. It is superseded on two counts, both of which
concern how its numbers were produced rather than how they were described: its
BER intervals are Clopper-Pearson over individual sampled bits, whose stated
coverage assumes bit independence that clustered block-burst decoder errors
violate, and its stopping rule is a cumulative bit-error target, which leaves
its block count a stopping time so that the fixed-trial Clopper-Pearson coverage
its BLER intervals state is not exact either
(`crates/gf2-sim/src/osd_campaign.rs:87-95`). Schema-1 receipts remain
deserializable as historical evidence and that set is retained unchanged; it is
not re-derived, because the difference is a change of measurement behavior and
reproducing those cells under the current tool is a new campaign
(`@/inv/behavioral-evidence-validity`).

Its record also carries a reading of the 5.23 dB `union_bound` row that this
record declines. It says of its own estimate, which fell below the tabulated
value, that this "is the relation an upper bound and a measurement are expected
to have" (`schema1/README.md:355-362`). That rests on the same premise as the
mirror-image claim would: which error probability the bound upper-bounds, and
under which decoding rule, is recorded nowhere in the committed evidence, so
neither ordering is established as expected or as anomalous. The archived text
is retained unedited as historical evidence rather than corrected in place.

## Contradictory evidence

**Table-versus-figure contradiction at 4.56 dB, order 2.** The committed dataset
preserves an unreconciled disagreement at this abscissa: Table 4.7 prints
$10^{-5.70} = 1.9953\times10^{-6}$ while the corresponding Figure 4.14 marker
reads $10^{-5.49} = 3.2359\times10^{-6}$, a gap of $0.210$ decades against a
$0.1$-decade read precision (`PIT-07`). This comparison tests the dataset's
primary `value` field as committed and selects neither reading. This campaign's
estimate is high against both and nearer the figure reading, and the published
series' local slope through 4.56 dB continues its own progression under the
figure reading while spiking under the table reading.
[The 4.56 dB order-2 point](#the-456-db-order-2-point) reports that in full, with
the limits on the slope argument. The contradiction stands unresolved and no
dataset value is changed.

**The series outlier accepts.** The 4.56 dB order-2 point is $+0.428$ decades
from its published value, a gap $5.3\times$ the next largest in the series,
and it accepts. The acceptance is a consequence of the interval width the
$K = 100$ block-error budget buys, not of agreement. Recording the verdict
without the gap would misrepresent the evidence, so both are tabulated.

**A bound compared as if it were a measurement.** The dataset marks the 5.23 dB
order-2 entry `value_kind = union_bound` and states that bound rows are not
simulation evidence. The predicate is evaluated mechanically for that row and
reported for completeness; its verdict carries no claim about reproducing a
published measurement.

**An ordering against a bound row whose significance the evidence cannot
judge.** At 5.23 dB, order 2, the point estimate $3.016\times10^{-7}$ exceeds
the tabulated `union_bound` value $2.512\times10^{-7}$ by $0.079$ decades. The
recorded interval contains that value, so the campaign does not resolve the two
apart at $K = 100$ block errors, and the excursion is well inside the width the
budget buys. Whether the ordering is anomalous at all is undecidable on the
committed evidence, which records the source's `*: union bound` caption flag and
the row's analytical kind but neither which error probability the bound
upper-bounds nor the decoding rule it is derived for. It is therefore **not**
recorded here as a contradiction — and equally it is not explained away.
[Order-2 comparison against the published curve](#order-2-comparison-against-the-published-curve)
gives that reasoning in full: a maximum-likelihood reading under which the
ordering is the relation to expect, and the dataset's order-indexed
`union_bound` rows, which tell against that reading without establishing one in
its place. The observation is recorded rather than reconciled: no campaign
control was adjusted, and the dataset row is unchanged.

**No independent seeded rerun.** This is one campaign at one seed. No second
seed and no independent rerun cross-check its cells, so no statement here is
supported by seed-to-seed reproducibility. Recorded as `NOTE-06`.

**Unverified article-level pin.** The reproduction target is the 1994
dissertation. [Fossorier1995] is closed access and unverified, and no claim here
derives from it (`PIT-07`).

## Citations

- [Fossorier1994] Fossorier — *Decoding of Linear Block Codes Based on Ordered
  Statistics*. Ph.D. dissertation, University of Hawai'i at Manoa, December
  1994. UMI 9519442.
- [Fossorier1995] Fossorier, Lin — *Soft-Decision Decoding of Linear Block Codes
  Based on Ordered Statistics*. IEEE Trans. Inf. Theory 41(5):1379–1396, 1995.
  Closed access and unverified: the committed access audit records a closed
  Unpaywall result, and the advertised full-text lead resolves to a different
  five-author 1999 work.
- [MaurerPontil2009] Maurer, Pontil — *Empirical Bernstein Bounds and Sample
  Variance Penalization*. COLT 2009. arXiv:0907.3740. The source of the
  empirical-Bernstein bound and its coverage statement for the $\mu$ factor of
  the composed BER interval.

Every key here resolves in the repository citation registry
`.jit/references.toml` (`@/inv/external-claims-cited`).
