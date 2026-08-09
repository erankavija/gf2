# Permanent-zero-fraction campaign preregistration

**Status:** frozen protocol, committed before any campaign-purpose matrix draw.
This document governs the first permanent-zero-fraction campaign whose
committed pre-draw execution receipt records this repository-relative path,
content SHA-256, and corresponding root-manifest identity. Protocol identity
belongs to execution and selection receipt evidence; the canonical manifest
and shard schemas do not carry a protocol-hash field. A changed protocol, cell,
sample size, test, or exclusion rule defines a new campaign id and requires a
new preregistration before that campaign's first draw.

The measured envelope and prior by-product counts remain evidence, not campaign
data. This protocol authorizes no draw by itself.

## Scientific question and estimand

For a uniform random square matrix $A \in \mathbb{F}_q^{n \times n}$, the
finite-size estimand is

$$
\delta_q(n) = \Pr[\operatorname{per}(A)=0] - \frac{1}{q},
\qquad q \in \{3,5,7\}.
$$

The campaign estimates the sign and magnitude of $\delta_q(n)$ at the finite
values of $n$ listed below. The conjecture
$\Pr[\operatorname{per}(A)=0]=1/q+o(1)$ concerns $n\to\infty$. An asymptotic
statement is compatible with every finite prefix, so no finite grid can confirm
or contradict it in either direction. Conclusions are restricted to the
measured cells. This distinction follows the feasibility study's
[finite-$n$ interpretation](/dev/studies/b488f02c/feasibility-study.md#71-what-this-campaign-can-and-cannot-establish)
of `@/citation/GGK2025` and `@/citation/HKS2026`.

For each cell the published raw quantities are the permanent zero count
$Z_{q,n}$ and sample count $N_{q,n}$; the point estimate is
$\widehat p_{q,n}=Z_{q,n}/N_{q,n}$. A 95% Wilson interval accompanies every
Monte Carlo point. Intervals communicate uncertainty; they do not take the
acceptance decisions specified below.

The determinant companion is evaluated on the same matrices. Its finite-size
null probability is

$$
p_{\det}(q,n)=1-\prod_{i=1}^{n}\left(1-q^{-i}\right),
$$

never its $n\to\infty$ limit.

## Frozen cell universe and sample sizes

The core universe is every integer $n$ from $4$ through the processor-feasible
frontier measured in the
[envelope receipt](/dev/studies/b488f02c/envelope-2026-08-07.csv): $n=28$ for
$q=3$, $n=24$ for $q=5$, and $n=20$ for $q=7$. It contains
$25+21+17=63$ cells. The determinant companion runs on all $63$ cells and on
the identical draws used for the permanent.

Sample counts are fixed in two tiers. The frontier tier uses the study's exact
$N=\lceil q^{-1}(1-q^{-1})/(10^{-3})^2\rceil$. The precision tier uses the
study's $10^{-4}$ count where it does not exceed the reviewed campaign maximum
of $2\times10^7$; the $q=3$ tier is capped at that maximum. The cap is a design
choice, not a claim that $2\times10^7$ attains standard error $10^{-4}$ at
$p=1/3$.

| $q$ | $n$ | Cells | Fixed $N_{q,n}$ | Planning standard error at $p=1/q$ |
|---:|:---|---:|---:|---:|
| $3$ | $4\ldots20$ | $17$ | $20\,000\,000$ | $1.0541\times10^{-4}$ |
| $3$ | $21\ldots28$ | $8$ | $222\,223$ | at most $10^{-3}$ |
| $5$ | $4\ldots16$ | $13$ | $16\,000\,000$ | $10^{-4}$ |
| $5$ | $17\ldots24$ | $8$ | $160\,000$ | $10^{-3}$ |
| $7$ | $4\ldots16$ | $13$ | $12\,244\,898$ | at most $10^{-4}$ |
| $7$ | $17\ldots20$ | $4$ | $122\,449$ | at most $10^{-3}$ |

These counts are unchanged if the observed fraction, interval width, apparent
trend, elapsed significance, or a neighbouring cell suggests that more or less
sampling would be interesting. A cell completes sampling only at exactly its
fixed $N_{q,n}$ valid, unique draws. There is no early-success, target-error,
confidence-width, significance, or futility stop.

The operational ceiling is twelve wall-clock hours per cell, including recovery.
The envelope reserves 15% of that ceiling for checkpointing, compaction, and one
failed-shard recovery, leaving $36\,720$ seconds of planned productive compute.
Reaching twelve hours before $N_{q,n}$ is a mechanical halt, not a sample-size
revision and not an invitation to pool a partial estimate as a campaign result.

### Extension family

The accelerator-contingent extension family is preregistered as the empty set:
$E=\varnothing$. An accelerator may be the frozen backend for a core cell, but
that does not create an extension cell. Thus the multiplicity count for this
campaign is exactly $K=63$.

No gate can add a cell after the first campaign draw. A later accelerator result
that motivates an extension requires a new campaign id, an explicit nonempty
extension list, new fixed sample counts, and a restated multiplicity allocation
committed before that new campaign draws. This protocol uses no gatekept second
test family.

## Global error budget and exact decisions

The global family-wise false-alarm budget is $\alpha_{\mathrm{global}}=0.05$.
It is spent once, not once per test family:

| Family | Family budget | Tests | Per-cell level |
|---|---:|---:|---:|
| Permanent-floor, one-sided | $\alpha_{\mathrm{per}}=0.025$ | $63$ | $\alpha_{\mathrm{per}}/63=1/2520\approx3.96825\times10^{-4}$ |
| Determinant, two-sided | $\alpha_{\det}=0.025$ | $63$ | $\alpha_{\det}/63=1/2520\approx3.96825\times10^{-4}$ |

Bonferroni therefore bounds the combined family-wise false-alarm probability by
$0.025+0.025=0.05$, without an independence assumption between the companion
counts.

### Permanent-floor decision

For $Z=Z_{q,n}$, evaluate

$$
H_0:p\geq \frac1q
\quad\text{against}\quad
H_1:p<\frac1q
$$

at the composite null's least-favourable boundary $p_0=1/q$. The exact
lower-tail value is

$$
p_{\mathrm{per}}=\Pr_{X\sim\operatorname{Bin}(N_{q,n},1/q)}[X\leq Z].
$$

The cell rejects the floor check exactly when
$p_{\mathrm{per}}\leq1/2520$. This is a pipeline failure because the floor is
proved for every $n$ at odd characteristic by `@/citation/HKS2026`; it is not a
scientific finding below $1/q$.

### Determinant decision

For the companion zero count $D=D_{q,n}$, test
$H_0:p=p_{\det}(q,n)$ with the probability-ordering convention for the exact
two-sided binomial test:

$$
p_{\det\text{-test}}
=\sum_{k:\,f(k)\leq f(D)} f(k),
\qquad
f(k)=\Pr_{X\sim\operatorname{Bin}(N_{q,n},p_{\det}(q,n))}[X=k].
$$

The cell rejects the determinant check exactly when
$p_{\det\text{-test}}\leq1/2520$. Equality is included in both rejection
rules. Implementations compare on a log scale or directly to the threshold so
an underflowed floating-point probability cannot change a verdict.

Normal approximations are sizing aids only. The permanent level corresponds to
a one-sided standard-normal critical value $z\approx3.3550$. The determinant
level has per-tail probability $1/5040$ and $z\approx3.5422$. Neither number may
appear in a decision path; all recorded verdicts come from the exact binomial
rules above. This split supersedes the feasibility study's presentation of a
fresh 5% budget for each family while preserving that earlier design as part of
the record.

## Validation before campaign draws

The validation phase uses the reserved validation stream purpose, never the
campaign-cell purpose. No estimated campaign cell is accepted as evidence until
all of the following anchors pass:

- $q=3$, $n\in\{1,2,3,4\}$;
- $q=5$, $n\in\{1,2,3\}$;
- $q=7$, $n\in\{1,2,3\}$.

For each of the ten anchors, an independent exhaustive enumerator visits all
$q^{n^2}$ matrices exactly once. The production permanent evaluator and pooling
estimator must reproduce the independent zero count exactly, with no tolerance.
Every backend eligible for a campaign cell must also agree per matrix with the
independent oracle on the anchor cases it supports. The determinant evaluator
must reproduce $p_{\det}(q,n)$ exactly.

Before the statistical sampler check, each of the ten preassigned validation
addresses undergoes a deterministic regeneration cross-check. Two fresh sampler
instances regenerate exactly the first $1\,024$ matrices from the same recorded
address. The first run is serial; the second uses the maximum worker count the
validation runner supports, or is a second fresh serial run if the sampler
contract exposes no worker-count choice, which the receipt records. The two
runs must agree exactly in address order and in every canonical row-major entry
byte (one residue in $[0,q)$ per byte). Any mismatch blocks all campaign draws.
These are validation-purpose streams, and this engineering gate publishes no
scientific estimate and is not a statistical retry.

The sampler check then draws a fixed $400\,000$ matrices at each anchor from its
preassigned validation address. Its zero count is compared with the enumerated
probability by the same probability-ordering exact two-sided binomial
definition. The validation family has engineering budget $0.01$, Bonferroni
level $0.001$ per anchor: an anchor passes exactly when its value is greater
than $0.001$, so equality fails. It is separate from the campaign's scientific
5% budget because it can only prevent launch; it cannot create a published
acceptance claim. A failed validation anchor is not redrawn within this
protocol: it blocks campaign draws, preserves the receipt, and requires a
diagnosed correction and a new validation protocol run.

The draw count and the order-three ground truth carry forward the committed
[anchor receipt](/dev/studies/b488f02c/order3-anchor-2026-08-08.txt); the
expanded anchor universe above is binding for this campaign.

## Execution order and the preserved by-product

After the manifest is frozen and validation passes, $(q,n)=(7,20)$ runs alone
as the first campaign cell, at its fixed $N=122\,449$. No other
campaign-purpose matrix is drawn before it reaches a terminal state.

The reason for ordering, not an input to its verdict, is the timing by-product
in
[zero-fraction-2026-08-07.csv](/dev/studies/b488f02c/zero-fraction-2026-08-07.csv):
$2\,438$ zeros in $17\,819$ matrices, $\widehat p=0.136820$, with a 95% Wilson
interval $[0.131853,0.141944]$ and a normal planning score $-2.30$ relative to
$1/7$. That run had no preregistered $N$ or stopping policy and is not pooled
with the campaign. It remains cited and unchanged whether the resample agrees,
disagrees, passes, rejects, or halts. A contradictory resample is recorded
beside it, never reconciled by deleting or relabelling either result.

The feasibility study also preserves the earlier stream-reuse and shared
warm-up-counter defects that produced and then retracted an apparent signal.
Those falsifications remain part of the pipeline history; this protocol does
not rewrite them into supporting evidence.

## Backend freeze

Before the first campaign draw, every core cell receives one backend in the
hashed root manifest. Its `CellSpec.backend_receipt` is an `ArtifactIdentity`
containing the repository-relative path and lowercase SHA-256 of the exact
committed selection receipt for that cell. The hash must verify against the
file at the manifest's git revision; a floating path, directory identity, or
unhashed prose reference is not a receipt binding.

A selection receipt identifies every raw timing receipt it considered by its
own repository-relative path and SHA-256. It records the chosen backend and the
following provenance for every candidate: physical host identity; CPU and GPU
model as applicable; accelerator runtime and driver; source revision; compiler;
build profile, features, and binary hash; worker count; frequency/governor or
equivalent power policy; $(q,n)$ workload and batch size; warm-up policy; and
timed repetition count. Candidates may be ranked only within one comparison
cohort for which those hardware, build, workload, and timing conditions match.
Receipts from different hosts, builds, workload shapes, or timing protocols are
not ranked against one another. If the available receipts do not form a
comparable cohort, the manifest cannot freeze until a same-cohort remeasurement
is committed and bound by the selection receipt.

Every measurement eligible for selection must have its sampling count and
stopping rule fixed before timing begins. The current remeasurement began before
this protocol rework; its durable premeasurement contract is the already
committed campaign [plan](/dev/active/b8206228-permanent-statistics/plan.md#material-risks-and-owner-decisions),
the criteria `@/issue/296a41c9/requirement/REQ-01` through
`@/issue/296a41c9/requirement/REQ-03`, and the harness repair at commit
`414d31f8`. They fix exactly four configurations: $(q,n)=(3,28)$ on the
accelerator at $M=1024$ and on intra-matrix rayon, $(5,24)$ on batch rayon, and
$(7,20)$ on the accelerator at $M=1024$. The run comprises exactly twelve fresh
processes per configuration, $48$ processes total, interleaved across the four
configurations on a quiesced host under the single canonical benchmark lock.

One initial locked process performs the 90-second whole-machine warm-up. Later
locked processes skip only that machine warm-up; every process still performs
at least three seconds of configuration warm-up and then at least five timed
repetitions and five timed seconds, subject to the fixed 120-second per-process
cap implemented by `414d31f8`. The run permits no result-dependent extension,
early stop, or replacement process. A failed process remains in the record and
does not cause a thirteenth process for that configuration. The selection
receipt records all $48$ planned process outcomes, including failures, and
binds this premeasurement evidence rather than implying that this later
protocol rework preceded the timing.

Within that cohort, selection proceeds in this order:

1. Exclude a backend unless it passes the shared per-matrix behavioural suite
   against the generic reference, including the validation anchors.
2. Exclude it unless the intended host and build satisfy its documented safety,
   capability, launch-duration, and resource conditions.
3. Exclude it unless one of the selection receipt's hashed raw receipts measures
   cell-applicable composite draw-pack-evaluate-count throughput. Rank the
   remaining backends by that measured composite throughput and use the greater
   measured mean. An exact tie is broken by the lower documented worst-case
   launch duration and then the lower documented resource demand, both safety
   properties; if those also tie, the manifest cannot freeze until another
   preregistered timing replication resolves the ordering. If only one backend
   remains eligible, the selection receipt records the exclusions rather than
   implying a timing comparison that did not occur.

Timing fixtures use their own stream purpose. Permanent or determinant values,
zero counts, zero fractions, intervals, or test outcomes never enter backend
selection. A backend discovered to be faster after the freeze does not replace
the manifest choice. A manifest-named backend that is absent or becomes unsafe
halts the cell; the driver does not silently substitute another backend. This
campaign-specific rule preserves the frozen selection and does not alter safe
fallbacks in the production libraries.

## Reproducibility identity

The root provenance names `rng_algorithm` as the token `chacha20`, records
`rng_version` as the exact implementation and crate version, and stores
`invocation` as the exact argument vector rather than shell prose. These fields
are mandatory for this campaign. Together with the manifest's root seed,
purpose tags, stream addresses, git revision, build and toolchain provenance,
they bind the sampler that maps an address to matrices. A manifest missing any
of them cannot freeze.

This claim is deliberately limited: the argument vector records how the driver
was invoked, while the source revision and dependency lock resolve the RNG
implementation. Neither an unrecorded local default nor a prose command is
treated as sampler identity. Bit-for-bit regeneration additionally requires the
same manifest and stream address; backend-result reproduction requires the
manifest-selected backend and its bound build/receipt provenance.

## Shard validity, quarantine, retry, and halt rules

A candidate shard is validated by joining its path and content to the frozen
manifest's matching `CellSpec` and `ShardSpec` and to the committed execution
receipt. It is valid only if all of these mechanically checkable conditions
hold:

- its content/path $(q,n)$ and stream purpose tag/index match the manifest, and
  its recorded matrix count equals the joined `ShardSpec` expectation;
- the execution receipt's campaign id and manifest hash identify that manifest,
  its protocol path/hash identify this file, and its reported backend equals the
  joined `CellSpec` selection;
- its address is unique among accepted shards and its draw range overlaps no
  other accepted range;
- the write completed atomically and its schema, checksum, histogram sum,
  permanent count, and determinant evaluated state are internally consistent;
- the backend reported no kernel, driver, arithmetic, safety, or conformance
  error.

An incomplete write, crash, checksum or schema failure, address mismatch or
overlap, count inconsistency, backend error, or safety/conformance failure
quarantines the shard. It is excluded from the raw dataset and pooling. The
runner preserves its bytes, logs, observed counts if any, failure reason, and
attempt number until they are content-addressed in a committed campaign
execution receipt. That receipt is evidence rather than a raw-schema shard;
finalization refuses a campaign with quarantine evidence that has not been
receipted. Nothing is deleted or overwritten. A statistically surprising but
mechanically valid shard is not quarantined.

Each shard permits at most two executions: the initial attempt and one recovery
attempt. Recovery must use the identical stream address, manifest-selected
backend, root seed and purpose tag, RNG algorithm and version, source revision,
build/toolchain provenance, and invocation, so it redraws the identical
matrices. It is allowed only after a predeclared mechanical quarantine and
never because of an observed zero fraction or test result. Because the
replacement is the same observation rather than a fresh sample or second test,
it spends no additional error budget. Only the valid replacement enters the
pooled count; all attempts remain preserved in the execution receipt.

A second failure, inability to retry within the twelve-hour cell ceiling, or a
frozen backend becoming unavailable halts the cell. A fresh stream would be a
new independent sample and an additional hypothesis test. This protocol
allocates it no error budget, so no fresh-stream rerun may change a verdict.
Such a resample requires a separately preregistered campaign id and its own
error allocation.

After a cell reaches $N_{q,n}$, both exact tests run once. A rejection is never
rerun until it passes and never converted into an exclusion. Either rejection
stops the launch of further work after any in-flight atomic shard is secured;
the rejecting cell and all downstream halted cells preserve their records.

## Terminal states and campaign completeness

Every manifest cell has exactly one terminal state:

- **Completed (executed):** every manifested shard is present and exactly
  $N_{q,n}$ valid unique draws are pooled. `SummaryRow` carries the mechanical
  `matrix_count`, `permanent_zero_count`, and canonical `DeterminantCount`.
  `CellTerminalState::Completed` carries `permanent_estimate`,
  `permanent_verdict`, and `determinant_estimate`. A rejecting completed cell
  triggers the campaign-wide halt rule but remains completed with its rejection
  intact.
- **Halted:** the cell did not reach a valid verdict because a named rule above
  fired. `CellTerminalState::Halted` records only its canonical `reason`:
  `acceptance_failure` for a propagated exact-test rejection,
  `backend_unavailable` when the frozen backend cannot run safely, or
  `execution_failure` for an exhausted mechanical retry or time ceiling. It
  carries no final estimate or verdict.

A halted `SummaryRow` pools the raw counts from every valid manifested atomic
shard that completed before the halt. Because shard work may complete in
parallel, the preserved unique subset need not be a prefix of the manifest's
ordered shard list. Those accepted shard files remain raw dataset members;
quarantined attempts remain in the committed execution receipt. Conformance
removes nonexistent future shard paths from the dataset's actual required-file
layout while still rejecting any unmanifested shard. If a cell whose determinant
plan is `evaluate` halts before any shard completes, its mechanical counts are
`matrix_count=0`, `permanent_zero_count=0`, and an evaluated
`DeterminantCount` with `sample_count=0` and `zero_count=0`; these zero counts do
not constitute an estimate. Only `Completed` requires the full manifested shard
set and exact $N_{q,n}$.

When one cell triggers a campaign-wide halt, every not-yet-executed manifest
cell receives the terminal reason `acceptance_failure`; it is not silently
omitted. The terminal state does not claim to encode a triggering-cell pointer
or execution history. A cell that is neither completed nor halted makes the
campaign incomplete. Finalization must refuse such a dataset.

## The $q=3$ arm is a reproduction

`@/citation/Scheinerman2024` reports exact zero counts through $n=5$ and Monte
Carlo counts through $n=30$. This campaign stops at $n=28$, so it does not extend
the published $q=3$ curve in $n$. The versioned
[reproduction target table](./scheinerman2024-q3-targets-v1.csv) transcribes the
source zero count and $N$ for every core $q=3$ cell, $n=4\ldots28$, and derives
the comparison point estimate from those counts. Table 3 rows are exact
enumerations and therefore have zero sampling error. For Table 4 rows the
reference precision is the plug-in binomial standard error
$s_{\mathrm{ref}}=\sqrt{\widehat p(1-\widehat p)/N}$; this is explicitly a
derived precision because the source publishes no confidence interval. The
table additionally derives a 95% Wilson score interval from each published
Table 4 count and sample size. It identifies that method and level explicitly;
these intervals are repository-derived comparison metadata, not intervals
reported by Scheinerman. Exact Table 3 rows instead use a degenerate interval at
the exact count-derived probability, labelled as having no sampling
uncertainty and no applicable confidence level. The table also records the
source's rounded estimate, power-of-ten trial count, and reproducibility
limitations and is baseline evidence, never campaign data.

Every $q=3$ result is reported as an independent reproduction and precision
comparison. Source and campaign counts, sample sizes, point estimates,
intervals, and precision are placed side by side. The descriptive interval
relation is `overlap` when the closed campaign and reference intervals intersect
and `disjoint` otherwise; it is not an acceptance test and does not change the
campaign's 95% Wilson contract. For an exact source row, the degenerate reference
interval makes this equivalent to asking whether the campaign interval contains
the exact source probability. Exact source rows are labelled `prior_exact`. At
Monte Carlo rows, compute the campaign plug-in standard error
$s_{\mathrm{campaign}}$ and label it `exceeds_prior_precision` when
$s_{\mathrm{campaign}}<0.9s_{\mathrm{ref}}$,
`matches_prior_precision` when
$0.9s_{\mathrm{ref}}\leq s_{\mathrm{campaign}}\leq1.1s_{\mathrm{ref}}$, and
`below_prior_precision` otherwise. Neither the reference interval nor the
campaign interval is misattributed to the source. No blanket improvement claim
is preregistered.

Agreement supports both pipelines. If a campaign interval excludes the target
table's count-derived point estimate, the discrepancy, both source records, and
the investigation are preserved under `@/inv/falsification-preserved`; the
result is not adjusted to manufacture agreement.

## Preregistered convergence-shape comparison

The shape analysis runs only on a finalized dataset in which every core cell is
completed. A halted campaign reports the comparison as unavailable rather than
fitting a selected subset.

For each $q$ separately, compare these two finite-$n$ candidate families:

$$
p_{\mathrm{geo},q}(n)=\frac1q+A_q q^{-a_q(n-4)},
$$

$$
p_{\mathrm{poly},q}(n)=\frac1q+B_q (n/4)^{-b_q}.
$$

These are the numerically anchored forms of
$1/q+c_q q^{-a_qn}$ and $1/q+d_qn^{-b_q}$. The closed search domains are

$$
0\leq A_q,B_q\leq1-1/q,
\qquad 0\leq a_q,b_q\leq64.
$$

They keep every fitted probability in $[1/q,1]$ on the measured grid and make
the boundary cases part of the declared optimization rather than an implicit
optimizer limit. A maximum on the exponent cap is reported as cap-truncated;
it is not extrapolated beyond 64.

For model $M$, maximize the observed-count binomial log likelihood

$$
\ell_M=\sum_n\left[
Z_{q,n}\log p_{M,q}(n)
+(N_{q,n}-Z_{q,n})\log(1-p_{M,q}(n))
\right],
$$

where the omitted binomial coefficient is common to both candidates. Weighted
least squares on $\widehat p$ is not the preregistered comparison.

For each fit, form the **UNCALIBRATED DESCRIPTIVE likelihood-support contour**

$$
\left\{\theta:2[\ell_M(\widehat\theta)-\ell_M(\theta)]
\leq5.991465\right\},
$$

over the closed domains above. The fixed likelihood-drop cutoff $5.991465$ is a
conventional descriptive threshold only. It is not a chi-square calibration;
the contour is not a confidence region, does not carry a confidence level, is
not a hypothesis test, and is not a member of the campaign error budget. No
coverage probability is claimed.
Support ranges are the coordinate projections of the contour. If the contour
includes amplitude zero, the exponent is unidentified there and its support
range is the full domain $[0,64]$; at an amplitude-zero maximum, no exponent
point estimate is reported. Any support range touching a search bound is
labelled boundary-truncated, and an exponent optimum on the cap remains labelled
cap-truncated.

Report the maximized log likelihood, fitted parameters and those support ranges,
and $\mathrm{AIC}_M=2k_M-2\ell_M$ with $k_M=2$, including boundary fits, for the
full measured range. Also repeat the identical fit for every nested prefix
$n=4,\ldots,m$ with at least six cells ($m\geq9$). For each prefix report
$\Delta\mathrm{AIC}=\mathrm{AIC}_{\mathrm{poly}}-
\mathrm{AIC}_{\mathrm{geo}}$. The operational report calls the candidates
distinguishable on a prefix only when $|\Delta\mathrm{AIC}|\geq2$ and names the
favoured family by the sign; otherwise it says indistinguishable. It publishes
the complete prefix table, including reversals and gaps, rather than assuming a
range such as $n\lesssim14$--$16$ in advance.

This is an aspirational, descriptive model comparison, not a third acceptance
family and not a test of the asymptotic conjecture. It spends none of the 5%
pipeline error budget and cannot rescue or overrule an acceptance rejection.

## Conditional novelty statement

The bounded
[literature-search receipt](/dev/studies/b488f02c/literature-search-2026-08-08.md)
records a 2026-08-08 search for exact or Monte Carlo estimates at $q\in\{5,7\}$.
It used ten queries in one general web index and direct retrieval of available
arXiv abstracts and full texts. It returned no published $q=5$ or $q=7$
zero-fraction numerics.

The limits are part of every novelty statement: MathSciNet, zbMATH, Web of
Science, and paywalled full text were not searched; the full text of
`@/citation/GGK2025` was not exhaustively checked; and the closest counting
candidates `@/citation/Budrevich2018`,
`@/citation/BudrevichGuterman2012`, and `@/citation/Bassalygo2013` were not read
in full. One general index is not a systematic bibliographic search.

The strongest licensed wording is: **the recorded search found no prior
numerics for $q\in\{5,7\}$, subject to its stated limits**. The campaign may not
say that none exist, that every cell is new, or that it has priority.
`@/citation/HKS2026` corroborates the narrow search result but does not remove
those limits.

## Scope boundary and evidence map

The archived
[permanent-versus-determinant uniformity campaign](/dev/archive/ae82bd73-gf2-algebra-permanent/benchmarks/perm_uniformity/results-2026-05-17-gpu.csv)
measured whole-distribution total-variation distance. It is preserved evidence
for a different question and supplies no zero-fraction cell to this campaign.

| Protocol decision | Source of evidence or authority |
|---|---|
| Finite-$n$ estimand and asymptotic limitation | [Feasibility study §7.1](/dev/studies/b488f02c/feasibility-study.md#71-what-this-campaign-can-and-cannot-establish); `@/citation/GGK2025`; `@/citation/HKS2026` |
| Core frontier and $10^{-3}$ counts | [Measured envelope](/dev/studies/b488f02c/envelope-2026-08-07.csv) |
| $2\times10^7$ maximum and one global split budget | Reviewed [campaign plan](/dev/active/b8206228-permanent-statistics/plan.md) and [authoritative manifest](/dev/active/b8206228-permanent-statistics/breakdown.json) |
| Exact floor and determinant rules | [Feasibility study §7.2](/dev/studies/b488f02c/feasibility-study.md#72-sampling-plan), revised here by the reviewed global-budget decision |
| Enumeration and sampler anchors | [Order-three anchor receipt](/dev/studies/b488f02c/order3-anchor-2026-08-08.txt) and [determinant anchor receipt](/dev/studies/b488f02c/determinant-anchor-2026-08-08.txt) |
| First $q=7,n=20$ resample and preserved reading | [Raw by-product receipt](/dev/studies/b488f02c/zero-fraction-2026-08-07.csv) and [study §4.7](/dev/studies/b488f02c/feasibility-study.md#47-zero-fractions-observed-in-passing) |
| $q=3$ reproduction boundary and complete target values | [Versioned target table](./scheinerman2024-q3-targets-v1.csv), [Study §1.1](/dev/studies/b488f02c/feasibility-study.md#11-prior-numerics-exist-for-q--3), and `@/citation/Scheinerman2024` |
| Backend selection and sampler identity | Manifest `CellSpec.backend_receipt` plus root `rng_algorithm`, `rng_version`, and `invocation` provenance, as bound by [Backend freeze](#backend-freeze) and [Reproducibility identity](#reproducibility-identity) |
| Likelihood rather than weighted-least-squares comparison | Reviewed [campaign plan](/dev/active/b8206228-permanent-statistics/plan.md#material-risks-and-owner-decisions) |
| Conditional $q\in\{5,7\}$ novelty wording | [Recorded literature search](/dev/studies/b488f02c/literature-search-2026-08-08.md) and registry-resolving citations above |

## Requirement cross-check

| Requirement | Binding section |
|---|---|
| REQ-01 | [Scientific question and estimand](#scientific-question-and-estimand) |
| REQ-02 | [Frozen cell universe and sample sizes](#frozen-cell-universe-and-sample-sizes) |
| REQ-03 | [Frozen cell universe and sample sizes](#frozen-cell-universe-and-sample-sizes) |
| REQ-04 | [Extension family](#extension-family) |
| REQ-05 | [Global error budget and exact decisions](#global-error-budget-and-exact-decisions) |
| REQ-06 | [Global error budget and exact decisions](#global-error-budget-and-exact-decisions) |
| REQ-07 | [Shard validity, quarantine, retry, and halt rules](#shard-validity-quarantine-retry-and-halt-rules) |
| REQ-08 | [Backend freeze](#backend-freeze) |
| REQ-09 | [Execution order and the preserved by-product](#execution-order-and-the-preserved-by-product) |
| REQ-10 | [Validation before campaign draws](#validation-before-campaign-draws) |
| REQ-11 | [The $q=3$ arm is a reproduction](#the-q3-arm-is-a-reproduction) |
| REQ-12 | [Preregistered convergence-shape comparison](#preregistered-convergence-shape-comparison) |
| REQ-13 | [Conditional novelty statement](#conditional-novelty-statement) |
| REQ-14 | [Terminal states and campaign completeness](#terminal-states-and-campaign-completeness) |

## Registered literature

- `@/citation/GGK2025`
- `@/citation/HKS2026`
- `@/citation/Scheinerman2024`
- `@/citation/Bassalygo2013`
- `@/citation/BudrevichGuterman2012`
- `@/citation/Budrevich2018`
