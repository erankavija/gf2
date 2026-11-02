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

The universe is every integer $n$ from $4$ through the configured frontier: $n=28$ for
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
combined probability without an independence assumption between the counts. The
permanent budget divides over all cells and the determinant budget over the
determinant-enabled cells; here every cell has a companion, so both divide by
$63$.

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
prevent launch. Every anchor reaches a terminal record. A dispatch error during
validation is a terminal failure of its anchor. A failed anchor is not redrawn: it stays failed on resume, blocks campaign draws, and requires a
diagnosed correction and a new validation protocol run. A killed
process reruns its unterminated anchor at the same address, drawing the same
matrices.

A change to the tool's measurement behaviour (sampling, evaluation, timing,
output semantics) after the receipt requires a new validation. The tool does not
read the receipt before drawing; the dependency of the campaign arms on the
validation task enforces this gate.

```sh
./scripts/cargo-budget.sh cargo run --release -p permanent-statistics --features hip \
  --bin permanent_validation -- --config experiments/permanent-statistics/config/campaign.json \
  --output target/simulation_results/permanent-zero-fraction-20261102-validation \
  --workers W
```

`permanent_validation --check PATH` parses a configuration and reports it without
opening a stream, a device, or a file.

## Execution order and the preserved by-product

After validation passes, $(q,n)=(7,20)$ runs alone as the first cell, at its
fixed $N=122\,449$. No other campaign-purpose matrix is drawn before it is
terminal, and cells then run one at a time.

## Backends

Each cell's backend and accelerator sizing input are fixed by the configuration.
`backend_evidence` cites the evaluation harness. The configured launch cap bounds
batching; launch size does not change a cell's mathematical result.

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

The shape analysis uses the campaign's finalized dataset: its candidate families,
domains, likelihood, `shape-fit-v1` optimizer, descriptive support contour,
`binomial-v1` bootstrap of $20\,000$ replicates with its seeding from the root
seed and the manifest and checksum digests, prefix table, and distinguishability
rule. It runs only when every cell is completed, spends none of the error budget,
and cannot overrule an acceptance rejection.
