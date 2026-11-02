# Permanent-zero-fraction campaign protocol

Campaign id: `permanent-zero-fraction-20261102`. This protocol is committed
before any draw under that id. [config/campaign.json](config/campaign.json) is
the machine authority for every constant the tool consumes; its digest is the
`configuration_sha256` in each run record. The pre-draw manifest is
[results/permanent-zero-fraction-20261102/manifest.json](results/permanent-zero-fraction-20261102/manifest.json),
a projection of that configuration which the tool compares before writing. A
changed protocol, cell, sample size, test, or exclusion rule defines a new
campaign id and requires a new preregistration before that campaign's first
draw.

Prior by-product counts are evidence, not campaign data. This protocol
authorizes no draw by itself.

## Scientific question and estimand

For a uniform random $A \in \mathbb{F}_q^{n \times n}$, the finite-size estimand
is

$$
\delta_q(n) = \Pr[\operatorname{per}(A)=0] - \frac{1}{q},
\qquad q \in \{3,5,7\}.
$$

The campaign estimates the sign and magnitude of $\delta_q(n)$ at the listed
values of $n$. The conjecture $\Pr[\operatorname{per}(A)=0]=1/q+o(1)$ concerns
$n\to\infty$; an asymptotic statement is compatible with every finite prefix, so
no finite grid confirms or contradicts it. Conclusions are restricted to the
measured cells, following the study's
[finite-$n$ interpretation](/dev/studies/b488f02c/feasibility-study.md#71-what-this-campaign-can-and-cannot-establish)
of `@/citation/GGK2025` and `@/citation/HKS2026`.

For each cell the raw quantities are the permanent zero count $Z_{q,n}$ and the
sample count $N_{q,n}$, with point estimate $\widehat p_{q,n}=Z_{q,n}/N_{q,n}$.
A 95% Wilson interval, using the configuration's `wilson_z`, accompanies every
point. Intervals communicate uncertainty and take no acceptance decision.

The determinant companion is evaluated on the same matrices, with finite-size
null

$$
p_{\det}(q,n)=1-\prod_{i=1}^{n}\left(1-q^{-i}\right).
$$

## Cells and sample sizes

The universe is every integer $n$ from $4$ through the frontier of the
[envelope receipt](/dev/studies/b488f02c/envelope-2026-08-07.csv): $n=28$ for
$q=3$, $n=24$ for $q=5$, and $n=20$ for $q=7$, which is $K=63$ cells. The
extension family is empty, and the determinant companion runs on all cells.

| $q$ | $n$ | Cells | Fixed $N_{q,n}$ | Planning standard error at $p=1/q$ | `shard_size` |
|---:|:---|---:|---:|---:|---:|
| $3$ | $4\ldots20$ | $17$ | $20\,000\,000$ | $1.0541\times10^{-4}$ | $200\,000$ |
| $3$ | $21\ldots28$ | $8$ | $222\,223$ | at most $10^{-3}$ | $50\,000$ |
| $5$ | $4\ldots16$ | $13$ | $16\,000\,000$ | $10^{-4}$ | $200\,000$ |
| $5$ | $17\ldots24$ | $8$ | $160\,000$ | $10^{-3}$ | $50\,000$ |
| $7$ | $4\ldots16$ | $13$ | $12\,244\,898$ | at most $10^{-4}$ | $200\,000$ |
| $7$ | $17\ldots20$ | $4$ | $122\,449$ | at most $10^{-3}$ | $50\,000$ |

The frontier tier uses $N=\lceil q^{-1}(1-q^{-1})/(10^{-3})^2\rceil$. The
precision tier uses the $10^{-4}$ count where it does not exceed the campaign
maximum of $2\times10^7$; the $q=3$ tier is capped at that maximum, which does
not attain standard error $10^{-4}$ at $p=1/3$.

These counts do not change with an observed fraction, interval width, trend,
significance, or neighbouring cell. A cell completes sampling only at exactly
$N_{q,n}$ draws. There is no early-success, target-error, confidence-width,
significance, or futility stop. Adding a cell requires a new campaign id and a
restated multiplicity allocation.

## Error budget and exact decisions

The family-wise false-alarm budget is $0.05$, spent once. Bonferroni bounds the
combined probability without an independence assumption between the counts.

| Family | Family budget | Tests | Per-cell level |
|---|---:|---:|---:|
| Permanent floor, one-sided | $0.025$ | $63$ | $0.025/63=1/2520$ |
| Determinant, two-sided | $0.025$ | $63$ | $0.025/63=1/2520$ |

**Permanent floor.** Test $H_0:p\geq 1/q$ against $H_1:p<1/q$ at the boundary
$p_0=1/q$ with

$$
p_{\mathrm{per}}=\Pr_{X\sim\operatorname{Bin}(N_{q,n},1/q)}[X\leq Z_{q,n}].
$$

The cell rejects exactly when $p_{\mathrm{per}}\leq1/2520$. A rejection is a
pipeline failure, because the floor is proved for every $n$ at odd
characteristic by `@/citation/HKS2026`; it is not a scientific finding below
$1/q$.

**Determinant.** For the companion count $D_{q,n}$, test $H_0:p=p_{\det}(q,n)$
with the probability-ordering exact two-sided binomial test:

$$
p_{\det\text{-test}}=\sum_{k:\,f(k)\leq f(D_{q,n})} f(k),
\qquad
f(k)=\Pr_{X\sim\operatorname{Bin}(N_{q,n},p_{\det}(q,n))}[X=k].
$$

The cell rejects exactly when $p_{\det\text{-test}}\leq1/2520$. Equality rejects
in both rules, and comparison is on the log scale, so underflow cannot change a
verdict. Normal approximations are sizing aids and enter no decision path.

Both tests run once, when a cell reaches $N_{q,n}$. A rejection is never rerun
or converted into an exclusion.

## Validation before campaign draws

No campaign cell is evidence until `permanent_validation` has recorded a passing
`receipt.json` for this configuration under this tool. The September evidence in
`@/issue/02b8137c` is history: it verified the prior tool and does not apply to
this one. [preregistration.json](preregistration.json) lists the anchors,
addresses, expected counts, and rules below; `config/campaign.json` is what the
tool consumes.

The validation streams use purpose `Validation`, never a campaign address. The
sampler seed is `(root_seed, q, n, (purpose_tag << 56) | stream_index)`. The
validation root seed differs from the September one and from the campaign's, and
each anchor has its own nonzero stream index, so these addresses are disjoint
from the September addresses. The ten anchors are $q=3$ with $n\in\{1,2,3,4\}$,
and $q\in\{5,7\}$ with $n\in\{1,2,3\}$.

For each anchor, in order:

1. **Enumeration.** An independent enumerator visits all $q^{n^2}$ matrices once.
   Its zero count equals the preregistered count, each selectable backend agrees
   with it per matrix wherever the backend supports the anchor, and the
   determinant evaluator reproduces $p_{\det}(q,n)$. These comparisons are exact
   integers. The selectable backends are all four backends the campaign names,
   so the validation build has the `hip` feature and a usable device. Failure
   here omits steps 2 and 3.
2. **Regeneration.** Two fresh samplers regenerate the first $1\,024$ matrices
   from the same address, serially and at the invocation's worker count; a worker
   count of one gives two serial runs. A digest of address-ordered entry bytes and
   permanent values must match. A mismatch fails the anchor.
3. **Sampler test.** The anchor draws exactly $400\,000$ matrices from its address
   and compares the zero count with the enumerated probability using the
   probability-ordering exact two-sided binomial test. The family has engineering
   budget $0.01$, level $0.001$ per anchor, and an anchor passes exactly when the
   value exceeds $0.001$, so equality fails.

This budget is separate from the scientific one, because validation can only
prevent launch. Every anchor reaches a terminal record. A failed anchor is not
redrawn: it stays failed on resume, blocks campaign draws, and requires a
diagnosed correction and a new validation protocol run. A killed
process reruns its unterminated anchor at the same address, drawing the same
matrices. The draw count and order-three truth carry forward the
[anchor receipt](/dev/studies/b488f02c/order3-anchor-2026-08-08.txt) and
[exact anchors](/dev/benchmarks/permanent_campaign/exact-anchors.csv).

A change to the tool's measurement behaviour (sampling, evaluation, timing,
output semantics) after the receipt requires a new validation. The tool does not
read the receipt before drawing; the dependency of the campaign arms on the
validation task enforces this gate.

```sh
./scripts/cargo-budget.sh cargo run --release -p permanent-statistics --features hip \
  --bin permanent_validation -- --config experiments/permanent-statistics/config/campaign.json \
  --output experiments/permanent-statistics/results/permanent-zero-fraction-20261102-validation \
  --workers W
```

`permanent_validation --check PATH` parses a configuration and reports it without
opening a stream, a device, or a file.

## Execution order and the preserved by-product

After validation passes, $(q,n)=(7,20)$ runs alone as the first cell, at its
fixed $N=122\,449$. No other campaign-purpose matrix is drawn before it is
terminal, and cells then run one at a time.

The reason for ordering, not an input to its verdict, is the timing by-product
in [zero-fraction-2026-08-07.csv](/dev/studies/b488f02c/zero-fraction-2026-08-07.csv):
$2\,438$ zeros in $17\,819$ matrices, $\widehat p=0.136820$, 95% Wilson interval
$[0.131853,0.141944]$, normal planning score $-2.30$ relative to $1/7$. That run
had no preregistered $N$ or stopping policy and is not pooled with the campaign.
It stays cited and unchanged whatever the resample shows; a contradictory
resample is recorded beside it. The feasibility study's stream-reuse and shared
warm-up-counter defects, which produced and then retracted an apparent signal,
remain part of the record.

## Backends

Each cell's backend is the one nominated in the
[backend selection receipt](/dev/benchmarks/permanent_campaign/backend-selection-v1.md),
which `backend_evidence` cites. Selection applied that receipt's rules to timing
only: permanent or determinant values, counts, intervals, and test outcomes never
entered it. It is not remeasured, and a backend later found faster does not
replace the configured one. Accelerator cells carry their sizing input from
[accelerator-launch-costs-v1.csv](/dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv)
under the configured launch cap. Launch size changes batching and no result.

When a cell's backend is unusable in this build or on this host, the operator
chooses per invocation with `--on-unavailable`:

- `refuse`, the default, exits without writing;
- `halt` records the terminal reason `backend_unavailable`;
- `fallback=BACKEND` runs the remaining shards on that backend, which must support
  the cell and be one of the configuration's `selectable_backends`.

A fallback draws the same addresses and spends no error budget. The log records
the choice and each shard records the backend that ran. A backend reporting
itself unavailable during a shard halts the campaign under `backend_unavailable`.

## Identity and bounded runs

Each run record pins the closed set of `@/inv/behavioral-evidence-validity`:
executable digest, build flags, configuration digest, seed, RNG implementation and
version, toolchain version, and observed host facts. A cell's first `start` event
fixes its identity, and resuming it under a different identity refuses. Other
cells may use another build or host. The source revision is a rebuild pointer.
Worker count, argument vector, and availability policy are logged observations
outside identity.

Work proceeds in bounded invocations of `permanent_campaign` with `--max-shards`;
exit status 10 means progress remains. `execution.jsonl`, opened before the first
draw, is the authoritative record. An advisory lock refuses a second writer.

## Failure and halt rules

The campaign stream index is `(cell_ordinal << 32) | shard_id`, with cells in
configuration order. A shard is accepted only when its address and matrix count
match the configuration, its histogram sums to its matrix count, its zero bin
equals its permanent count, and its determinant count is present. A statistically
surprising but mechanically valid shard is accepted.

A killed process is not a failure: it reruns the same address, draws identical
matrices, and spends no attempt. A reported error or a caught panic is a failure,
recorded in the log as `shard_failed` with its attempt number and message. A shard
permits `max_shard_attempts` failures. The retry redraws the identical matrices
and spends no error budget, and it is never caused by an observed fraction or test
result. A fresh stream would be an additional hypothesis test with no allocated
budget, so it requires a separately preregistered campaign id.

## Terminal states and completeness

Every cell has exactly one terminal state.

- **Completed:** all shards are present and exactly $N_{q,n}$ draws are pooled.
  The row carries counts, estimates, intervals, verdicts, and log p-values. A
  rejecting cell remains completed with its rejection intact.
- **Halted:** the row records one reason, `acceptance_failure`,
  `backend_unavailable`, or `execution_failure`, pools the accepted shards, and
  carries no estimate or verdict.

Any exact rejection or exhausted retry allocation halts the campaign: every other
non-terminal cell receives the terminal reason of the originating event, and
accepted shards are preserved. `permanent_dataset finalize` refuses a campaign
with a non-terminal cell.

## The $q=3$ arm is a reproduction

`@/citation/Scheinerman2024` reports exact zero counts through $n=5$ and Monte
Carlo counts through $n=30$. This campaign stops at $n=28$, so it does not extend
the published $q=3$ curve in $n$. The
[target table](data/scheinerman2024-q3-targets-v1.csv) transcribes the source
count and $N$ for every $q=3$ cell. Exact rows use a degenerate interval with no
sampling uncertainty. Monte Carlo rows derive the plug-in standard error
$s_{\mathrm{ref}}$ and a 95% Wilson interval from the published counts; these are
repository-derived metadata, not intervals reported by the source.

Every $q=3$ result is reported as an independent reproduction, with source and
campaign values side by side. The interval relation is `overlap` when the closed
intervals intersect and `disjoint` otherwise; it is descriptive and takes no
acceptance decision. Exact rows are labelled `prior_exact`. Monte Carlo rows are
labelled `exceeds_prior_precision` when
$s_{\mathrm{campaign}}<0.9s_{\mathrm{ref}}$, `matches_prior_precision` when
$0.9s_{\mathrm{ref}}\leq s_{\mathrm{campaign}}\leq1.1s_{\mathrm{ref}}$, and
`below_prior_precision` otherwise. If a campaign interval excludes the
count-derived point estimate, the discrepancy, both records, and the investigation
are preserved under `@/inv/falsification-preserved`.

## Convergence-shape comparison

The shape analysis is the
[preregistered comparison](/dev/simulation_results/permanent-zero-fraction/protocol.md#preregistered-convergence-shape-comparison),
applied unchanged to this campaign's finalized dataset: its candidate families,
domains, likelihood, `shape-fit-v1` optimizer, descriptive support contour,
`binomial-v1` bootstrap of $20\,000$ replicates with its seeding from the root
seed and the manifest and checksum digests, prefix table, and distinguishability
rule. It runs only when every cell is completed, spends none of the error budget,
and cannot overrule an acceptance rejection.

## Conditional novelty statement

The bounded
[literature-search receipt](/dev/studies/b488f02c/literature-search-2026-08-08.md)
records a 2026-08-08 search for exact or Monte Carlo estimates at $q\in\{5,7\}$,
using ten queries in one general web index and direct retrieval of available arXiv
abstracts and full texts. MathSciNet, zbMATH, Web of Science, and paywalled full
text were not searched; the full text of `@/citation/GGK2025` was not exhaustively
checked; and `@/citation/Budrevich2018`, `@/citation/BudrevichGuterman2012`, and
`@/citation/Bassalygo2013` were not read in full.

The strongest licensed wording is: **the recorded search found no prior numerics
for $q\in\{5,7\}$, subject to its stated limits**. The campaign does not say that
none exist, that every cell is new, or that it has priority.

## Differences from the prior protocol

The [prior protocol](/dev/simulation_results/permanent-zero-fraction/protocol.md)
governed the draft `permanent-zero-fraction-20260829`, which drew no campaign
matrix. Cells, sample sizes, tests, levels, anchors, and validation counts are
unchanged. These rules differ:

| Difference | Reason |
|---|---|
| New campaign id, campaign root seed, and validation addresses. | The prior protocol bound its arms to emitter `c9b2307a`, which the rebuilt tool replaces, and a changed protocol requires a new id. |
| One Wilson z, the configuration's `wilson_z`, for every report. | The prior tool's summary used a shorter constant than its sidecars and script. |
| Each family budget divides over all cells, $K$. | The prior tool divided the determinant budget by determinant-enabled cells. Every cell has a companion, so the level stays $1/2520$. |
| A killed process spends no attempt; a caught panic does. | A rerun of the same address redraws identical matrices, while a panic is a reported failure. |
| A failed attempt is a `shard_failed` log entry with its message; no quarantine file or attempt bytes persist. | The log is the authoritative record, and a retry redraws the same matrices. |
| `shard_size` is $200\,000$ in precision tiers, was $50\,000$. | Shard count and log size fit `@/inv/committed-evidence-budget`. Counts and tests are untouched; a shard is the unit of repeated work on interruption. |
| The twelve-hour cell ceiling is removed. | Bounded runs are resumable under `@/inv/campaign-resumability`, the tool has no timer, and a wall-clock stop would make completion depend on the host. Cells stop only at $N_{q,n}$ or a named halt. |
| Identical-source-revision and clean-closure clauses are removed; resume and acceptance compare the closed set. | `@/inv/behavioral-evidence-validity`. |
| Protocol and backend evidence are cited paths, not digests verified by a tool, and the manifest stores no provenance or shard list. | The configuration digest is in the closed set; the rest is outside it. The manifest size fits the same budget. |
| Selection is copied from the prior receipt without a same-cohort remeasurement on this tool. | Selection affects speed only; validation and shared suites establish equal results. |
| An explicit `--on-unavailable` per invocation replaces "halt the cell", with `fallback=` limited to validated backends. | A fallback draws the same addresses and is recorded. The default remains a refusal, and no substitution is silent. |
| Any terminal failure propagates its own reason to other cells, where the prior text named `acceptance_failure`. | The reason then states what stopped the campaign. |
| Validation compares each backend per matrix and leaves shard pooling to the crate's tests; a regeneration mismatch fails the anchor without skipping the draw. | The tool has one evaluation loop. The verdict is unchanged: a mismatch fails. |
| A killed anchor reruns at its address, where the prior journal failed it permanently. | A nonterminal anchor recorded no result, and the rerun draws identical matrices. |
| Validation gating is by tracker dependency, not a receipt check in the tool. | The tool holds no such checker under `@/inv/behavioral-evidence-validity`. |
| `summary.csv` adds `permanent_log_p_value` and `determinant_log_p_value`. | Decisions are on the log scale, and the columns expose them. |

## Registered literature

- `@/citation/GGK2025`
- `@/citation/HKS2026`
- `@/citation/Scheinerman2024`
- `@/citation/Bassalygo2013`
- `@/citation/BudrevichGuterman2012`
- `@/citation/Budrevich2018`
