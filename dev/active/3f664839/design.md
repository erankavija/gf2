# Preregistered compressed-state rare-event design

**Issue:** 3f664839
**Type:** task
**Priority:** low
**Date:** 2026-08-30

## Problem Statement

For an ordered $n\times3$ matrix over a supported odd prime field, the event
of interest is that every $3\times3$ row submatrix has zero permanent. Direct
sampling cannot resolve this event at the registered target
$(q,n,k)=(3,1024,3)$. This design fixes an exact compressed-state recurrence
as the primary calculation and a separately accumulated, full-support
importance proposal as an uncertainty-bearing cross-check.

The event is the permanental-rank-deficiency event implemented by
`gf2_algebra::permanent::permanental_rank_status`, not a scalar rectangular
permanent. The motivating theorem and leading-scale comparison resolve through
the repository citation `@/citation/GGK2025`. The supported fields are exactly
$\mathbb F_3$, $\mathbb F_5$, and $\mathbb F_7$.

## Success Criteria

- [hard] REQ-01: Specifies canonical representations of $U$ and $V$, proves the prefix-validity condition $x\in V^\perp$, and specifies the updates $U'=U+\langle x\rangle$ and $V'=V+\phi(U,x)$.
- [hard] REQ-02: Specifies an exact recurrence over compressed states whose summed integer count divided by $q^{3n}$ equals the permanental-rank-deficiency probability.
- [hard] REQ-03: Specifies the independent proposal $x\sim\operatorname{Unif}(V^\perp)$, the incremental weight $q^{-\dim V}$, its full-support condition, and a self-contained unbiasedness argument.
- [hard] REQ-04: Names $(q,n,k)=(3,1024,3)$ as the target, records that $0.1\sqrt{1024}=3.2$, and quantifies why direct sampling cannot reach a probability of leading order $3^{-1023}$.
- [hard] REQ-05: Fixes before implementation the finite exhaustive transition and matrix-validation domains for each supported field, names every fast-tier and ignored slow-tier case, and fixes the exact anchors, independent-run counts, nominal interval level, empirical coverage rule, and contradiction rule; no runtime notion of `tractable` chooses additional exhaustive cases.
- [hard] REQ-06: Defines final-weight ESS, independent-run variance, extinction or degeneracy diagnostics, and the threshold that makes the stochastic cross-check unusable without invalidating the exact result.
- [hard] REQ-07: Binds every stochastic draw to manifested rare-event purpose tag $4$ under fixed stream-index subdomains, separate from campaign and validation data.
- [hard] REQ-08: States that the exact rational value is primary, the stochastic interval is a cross-check, and disagreement or failed coverage is preserved rather than reconciled through retuning.

## Mathematical contract

### Rows, contractions, and prefixes

Write a row as $u=(u_1,u_2,u_3)\in\mathbb F_q^3$ and define the symmetric
bilinear contraction

$$
\phi(u,v)=
\left(
u_2v_3+u_3v_2,\;
u_1v_3+u_3v_1,\;
u_1v_2+u_2v_1
\right).
$$

For three rows $u,v,x$, expansion by the coordinates of $x$ gives

$$
\begin{aligned}
\operatorname{per}(u,v,x)
&=(u_2v_3+u_3v_2)x_1
 +(u_1v_3+u_3v_1)x_2 \\
&\quad +(u_1v_2+u_2v_1)x_3
=\phi(u,v)\mathbin{\cdot}x.
\end{aligned}
$$

For an ordered prefix $R_t=(r_1,\ldots,r_t)$, define

$$
U_t=\operatorname{span}\{r_i:1\le i\le t\},
\qquad
V_t=\operatorname{span}\{\phi(r_i,r_j):1\le i<j\le t\}.
$$

The empty spans are $\{0\}$. A prefix is valid when every triple of
distinct row indices has zero permanent.

### Canonical subspaces and state identity

`CanonicalSubspace` represents a subspace of $\mathbb F_q^3$ by the
unique reduced-row-echelon basis over $\mathbb F_q$. Field elements use their
canonical residues in $\{0,\ldots,q-1\}$. Normalization performs elimination
in coordinate order $1,2,3$, scales every pivot to one, eliminates its column
from every other row, orders rows by increasing pivot coordinate, removes zero
rows, and pads unused storage with zero rows.

The semantic key of a subspace $S$ is

$$
K_q(S)=
\left(\dim S,\;b_{11},b_{12},b_{13},\ldots,b_{31},b_{32},b_{33}\right),
$$

where $b_{i,j}$ are the nine row-major canonical residues of the padded
$3\times3$ basis array. This fixes all identity operations:

- equality holds exactly when the keys agree;
- `Hash` feeds the domain byte for `CanonicalSubspace`, $q$, and every
  component of $K_q(S)$, including padded zeros, in that order;
- `Ord` is lexicographic order on $K_q(S)$, and the state order is
  lexicographic on $(K_q(U),K_q(V))$;
- any persisted byte form is version byte $1$, field-order byte $q$, then
  the key bytes above. Decoding rejects a wrong field, a residue at least
  $q$, nonzero padding, or a basis that is not already canonical.

The hash output of a caller-selected hasher is not a persistent identity; the
canonical fed sequence is. Exact propagation uses the total order in a
`BTreeMap`, so iteration and reduction do not depend on randomized hash seeds.
State equality compares both subspace keys, and state hashing feeds the $U$
key followed by the $V$ key.

The sealed supported-field trait has implementations only for
$\mathbb F_3$, $\mathbb F_5$, and $\mathbb F_7$.

`CompressedRankState` contains private canonical $U$ and $V$ fields.
Public construction starts at $(\{0\},\{0\})$ or advances an existing state,
which preserves reachability. A checked decoding path verifies the canonical
encoding; scientific entry points accept only the initial state or a state
reconstructed by replaying canonical transitions from a checkpoint. Raw
subspace-pair construction remains test-only.

### Prefix-validity equivalence and updates

Assume $R_t$ is valid and append $x$. All old triples remain zero. Every
new triple consists of $x$ and two old rows, and its permanent is
$\phi(r_i,r_j)\cdot x$. Therefore

$$
\begin{aligned}
R_{t+1}\text{ is valid}
&\iff
\phi(r_i,r_j)\cdot x=0
\quad\text{for every }i<j \\
&\iff v\cdot x=0
\quad\text{for every }v\in V_t \\
&\iff x\in V_t^\perp.
\end{aligned}
$$

No converse implication is omitted: membership in $V_t^\perp$ zeroes every
new triple, and the induction hypothesis zeroes every old triple.

The row-span update is

$$
U_{t+1}=U_t+\langle x\rangle.
$$

For fixed $x$, the map $L_x:u\mapsto\phi(u,x)$ is linear. Hence the new
pair contractions span the image

$$
\phi(U_t,x)=L_x(U_t)
=\operatorname{span}\{\phi(b,x):b\in B\}
$$

for any basis $B$ of $U_t$, and

$$
V_{t+1}=V_t+\phi(U_t,x).
$$

If $B'$ is another basis, its vectors are obtained from $B$ by an
invertible change-of-basis matrix. Linearity of $L_x$ makes the two image
lists related by the same invertible row operations, so their spans agree.
Thus $\phi(U_t,x)$, the update, and the normalized successor state are
basis-independent.

### Exact integer recurrence

Let $\mathcal S_q$ be the finite set of reachable canonical states. For
$S=(U,V)$, an admissible row is any $x\in V^\perp$, and the deterministic
successor is

$$
T_q(S,x)=
\left(
U+\langle x\rangle,\;
V+\phi(U,x)
\right).
$$

For two states define the exact transition multiplicity

$$
m_q(S,S')
=
\#\{x\in V^\perp:T_q(S,x)=S'\}.
$$

Every row vector is counted once, including $x=0$, and the outgoing
multiplicities satisfy the check

$$
\sum_{S'\in\mathcal S_q}m_q(S,S')
=|V^\perp|
=q^{3-\dim V}.
$$

Let $C_t(S)$ be the integer number of valid ordered $t$-row prefixes in
state $S$. The complete recurrence is

$$
C_0(S)=
\begin{cases}
1,&S=(\{0\},\{0\}),\\
0,&\text{otherwise},
\end{cases}
$$

and, for $0\le t<n$,

$$
C_{t+1}(S')
=\sum_{S\in\mathcal S_q}C_t(S)m_q(S,S').
$$

The terminal valid-matrix count and probability are

$$
D_{q,n,3}=\sum_{S\in\mathcal S_q}C_n(S),
\qquad
P_{q,n,3}=\frac{D_{q,n,3}}{q^{3n}}.
$$

The induction above gives a bijection between each counted transition path and
one valid ordered row prefix, while every invalid row is absent from the
outgoing transition set. Therefore the numerator counts exactly the deficient
$n\times3$ matrices. The denominator is $q^{3n}$, not $q^n$ and not a
state-dependent product, because the unconditional target has $3n$
independent field entries.

All counts, powers, sums, products, the greatest common divisor, and the reduced
numerator and denominator use arbitrary-precision unsigned integers. The
public result retains the raw pair
$(D_{q,n,3},q^{3n})$ and returns

$$
\left(
\frac{D_{q,n,3}}{\gcd(D_{q,n,3},q^{3n})},
\frac{q^{3n}}{\gcd(D_{q,n,3},q^{3n})}
\right)
$$

as canonical base-ten integer strings. Floating-point, scientific-notation,
and logarithmic renderings are derived fields and never replace those strings.

There are at most $\left(2(q^2+q+1)+2\right)^2$ raw subspace pairs.
Transition precomputation visits at most $q^3$ rows per retained state.
After that finite precomputation, propagation costs
$O(n\,|\mathcal E_q|)$ arbitrary-precision additions and
$O(|\mathcal S_q|)$ count storage, where $\mathcal E_q$ is the retained
nonzero-multiplicity edge set.

## Registered target and direct-sampling bound

The target is exactly

$$
(q,n,k)=(3,1024,3).
$$

It lies on the recorded theorem side because

$$
0.1\sqrt{1024}=0.1\cdot32=3.2
\quad\text{and}\quad
3<3.2.
$$

The preregistered leading scale from `@/citation/GGK2025` is

$$
3\cdot3^{-1024}=3^{-1023}.
$$

Numerically,

$$
\log_{10}(3^{-1023})=-488.09504358,
\qquad
3^{-1023}\approx8.03\times10^{-489}.
$$

One expected direct-sampling hit therefore needs
$3^{1023}\approx1.25\times10^{488}$ independent matrices. A $95\%$ chance
of at least one hit needs approximately

$$
\frac{-\log(0.05)}{3^{-1023}}
\approx3.73\times10^{488}
$$

matrices, and a Bernoulli estimate with approximately $10\%$ relative
standard error needs about

$$
\frac{100}{3^{-1023}}
\approx1.25\times10^{490}
$$

matrices. These requirements quantify why direct sampling cannot resolve the
leading scale; they do not depend on a hardware-throughput claim.

## Independent importance proposal

### Proposal law and full support

At prefix state $S_t=(U_t,V_t)$, form the canonical RREF basis of
$V_t^\perp$. If its dimension is $d_t=3-\dim V_t$, draw $d_t$
independent uniform coefficients in $\mathbb F_q$ and map them through that
basis. The coefficient map is bijective, so

$$
X_{t+1}\mid S_t\sim\operatorname{Unif}(V_t^\perp),
\qquad
Q(X_{t+1}=x\mid S_t)=q^{-d_t}.
$$

Every valid target path has $x_{t+1}\in V_t^\perp$ at every step by the
prefix-validity equivalence, so the proposal assigns it positive probability.
This is the required full-support condition. The proposal produces only valid
paths and never uses rejection over complete matrices.

Under the unconditional target, every row has conditional probability
$q^{-3}$. The target-to-proposal ratio for an admissible step is

$$
\frac{q^{-3}}{q^{-(3-\dim V_t)}}
=q^{-\dim V_t}.
$$

The direction is target divided by proposal. Reversing this ratio gives a
biased estimator. The final path weight is

$$
W(R_n)=\prod_{t=0}^{n-1}q^{-\dim V_t}
=q^{-E(R_n)},
\qquad
E(R_n)=\sum_{t=0}^{n-1}\dim V_t.
$$

For the proposal law $Q$ and valid-path set $\mathcal A$,

$$
\begin{aligned}
\mathbb E_Q[W]
&=\sum_{R_n\in\mathcal A}Q(R_n)
  \frac{\Pr_{\mathrm{target}}(R_n)}{Q(R_n)}\\
&=\sum_{R_n\in\mathcal A}\Pr_{\mathrm{target}}(R_n)
=\frac{D_{q,n,3}}{q^{3n}}.
\end{aligned}
$$

Thus the sample mean of final weights is unbiased. The proof uses no
asymptotic approximation and includes all paths, including repeated and zero
rows.

### Fixed stochastic allocations and interval

The target cross-check has exactly $R=32$ mutually addressed independent
runs and exactly $M=16\,384$ trajectories per run, for

$$
N_{\mathrm{target}}=RM=524\,288
$$

final weights. No result, interval, elapsed time, or diagnostic increases or
decreases this count.

For run $r$, define

$$
\widehat p_r=\frac1M\sum_{j=0}^{M-1}W_{r,j},
\qquad
\overline p=\frac1R\sum_{r=0}^{R-1}\widehat p_r.
$$

Variance comes from the $R$ independent run means:

$$
s_{\mathrm{run}}^2
=\frac1{R-1}\sum_{r=0}^{R-1}
(\widehat p_r-\overline p)^2,
\qquad
\widehat{\operatorname{Var}}(\overline p)
=\frac{s_{\mathrm{run}}^2}{R}.
$$

The cross-check interval is the nominal $95\%$ Student interval with
$31$ degrees of freedom and fixed critical value
$t_{0.975,31}=2.039513446$, interpreted as the exact decimal rational
$1\,019\,756\,723/500\,000\,000$:

$$
I_{0.95}
=
\left[
\max\left(0,\overline p-
2.039513446\sqrt{\frac{s_{\mathrm{run}}^2}{32}}\right),
\min\left(1,\overline p+
2.039513446\sqrt{\frac{s_{\mathrm{run}}^2}{32}}\right)
\right].
$$

Each trajectory stores the integer exponent $E$, and each run stores its
exact exponent histogram. Sums of $W$, $W^2$, run means, and
$s_{\mathrm{run}}^2$ are accumulated as exact rationals. Interval endpoints
use a common symbolic scale $q^{-e_*}$, where $e_*$ is the minimum observed
exponent, before numerical square root and outward-rounded decimal rendering.
No binary64 conversion of an absolute probability is allowed to underflow a
weight to zero. Containment verdicts compare the exact rational quantities
before rendering, using squared nonnegative sides after isolating the square
root. Published endpoints contain at least $18$ significant decimal digits
and round outward.

### ESS, extinction, degeneracy, and usability

For the complete set of final weights, final-weight effective sample size is

$$
\operatorname{ESS}
=\frac{\left(\sum_{i=1}^{N_{\mathrm{target}}}W_i\right)^2}
{\sum_{i=1}^{N_{\mathrm{target}}}W_i^2}.
$$

The receipt records pooled and per-run ESS, pooled
$\operatorname{ESS}/N_{\mathrm{target}}$, the minimum and maximum exponent,
the complete exponent histogram, the largest normalized weight share
$\max_i W_i/\sum_jW_j$, every run mean, and
$\max_r\widehat p_r/\sum_s\widehat p_s$.

Theoretical weights are strictly positive. Extinction means that no completed
weight remains in an addressed run, an addressed trajectory is missing, an
exponent is out of $0\le E\le3n$, an exact sum is zero, or a numerical
rendering is non-finite. Any extinction condition makes the stochastic
cross-check unusable and is an implementation failure, not a zero estimate.

Weight degeneracy makes the target cross-check unusable exactly when

$$
\frac{\operatorname{ESS}}{524\,288}<0.01
$$

or equivalently when $\operatorname{ESS}<5\,242.88$. Equality at $0.01$
is usable. This fixed threshold and every extinction condition are evaluated
after the fixed allocation; no extra trajectory is drawn to cross the
threshold. A below-threshold cross-check retains its point estimate, interval,
and diagnostics with verdict `unusable`. The exact rational result remains
valid and primary.

## Randomness, addressing, and restart contract

The addressing authority is the frozen
[campaign manifest](/dev/simulation_results/permanent-zero-fraction/permanent-zero-fraction-20260829/manifest.json)
with campaign id `permanent-zero-fraction-20260829`, interpreted under the
[frozen protocol](/dev/simulation_results/permanent-zero-fraction/protocol.md)
and the canonical sampler in
`crates/gf2-stats/src/sampler.rs`. Every stochastic coefficient draw uses the
frozen campaign root
$s=\mathtt{0x7a81626200000001}$, `FieldOrder`, matrix dimension $n$,
`StreamPurpose::RareEvent`, and `StreamIndex` through
`gf2_stats::sampler::MatrixAddress`. `StreamPurpose::RareEvent` has manifested
tag $4$. Tag $1$ remains validation, tag $2$ remains timing, and tag
$3$ remains campaign cells. The high purpose byte and low $56$-bit stream
index therefore separate rare-event draws from campaign and validation data by
construction.

The low-$56$-bit rare-event index has this closed partition:

| Rare-event use | Inclusive index start | Exclusive index end | Encoding |
|---|---:|---:|---|
| Target $(3,1024,3)$ | $0$ | $2^{54}$ | $I=(r\ll14)\mathbin{\vert}j$, $0\le r<32$, $0\le j<16\,384$ |
| Coverage anchors | $2^{54}$ | $2^{55}$ | $I=2^{54}\mathbin{\vert}(b\ll17)\mathbin{\vert}(r\ll12)\mathbin{\vert}j$, $0\le b<200$, $0\le r<32$, $0\le j<4\,096$ |
| Deterministic tests | $2^{55}$ | $3\cdot2^{54}$ | $I=2^{55}\mathbin{\vert}(f\ll16)\mathbin{\vert}c$, field slot $f\in\{0,1,2\}$, $0\le c<2^{16}$ |
| Reserved, unallocated | $3\cdot2^{54}$ | $2^{56}$ | no address is allocated |

The target uses exactly indices $0$ through $524\,287$. Every field coverage
anchor assigns its $200$ interval replicates exactly the contiguous
indices $2^{54}$ through
$2^{54}+26\,214\,399$. Different $q$ values and dimensions also occupy
different address words. Deterministic test case numbers are constants in the
test table; no test takes an index from either scientific subdomain.

One trajectory owns one address and consumes coefficients in increasing prefix
order and increasing canonical-nullspace-basis coefficient order. A completed
trajectory persists atomically as $(r,j,E)$ or $(b,r,j,E)$. Parallel
workers claim addresses but never share an RNG. Final reduction sorts these
tuples by their semantic indices. Resume skips every persisted completed tuple
and restarts only an incomplete trajectory from its identical address; it
never substitutes a fresh index or repeats a completed trajectory.
Consequently fixed addresses give bit-identical exponents, histograms, exact
sums, ESS, variance, and intervals across worker counts, scheduling,
interruption, and resume.

## Fixed validation protocol

### Exact anchors

For $k=3$, the first three row counts are vacuous:

$$
D_{q,0,3}=1,\qquad
D_{q,1,3}=q^3,\qquad
D_{q,2,3}=q^6.
$$

For $k=1$, exactly the all-zero column is deficient, so
$D_{q,n,1}=1$ and $P_{q,n,1}=q^{-n}$.

For $k=2$ over an odd field, pairwise permanent-zero rows either all lie in
one of the two isotropic coordinate lines, contain exactly one non-isotropic
nonzero row, or contain exactly two mutually orthogonal non-isotropic nonzero
rows. These disjoint classes give the independently derived count

$$
D_{q,n,2}
=2q^n-1+n(q-1)^2+\binom n2(q-1)^3.
$$

The literal $k=2$ anchors are

| $(q,n,k)$ | Deficient count | Total matrices |
|---|---:|---:|
| $(3,3,2)$ | $89$ | $729$ |
| $(3,4,2)$ | $225$ | $6\,561$ |
| $(5,3,2)$ | $489$ | $15\,625$ |
| $(7,3,2)$ | $1\,441$ | $117\,649$ |

For $k=n=3$, fix the first row $u$. The linear map
$v\mapsto\phi(u,v)$ has kernel size $q^3$ for $u=0$, size $q$ for
nonzero $u$ with at least one zero coordinate, and size $1$ when every
coordinate of $u$ is nonzero. Hence

$$
\#\{(u,v):\phi(u,v)=0\}
=5q^3-6q^2+3q-1
$$

and

$$
D_{q,3,3}
=q^3N_0+q^2(q^6-N_0)
=q^8+q^2(q-1)(5q^3-6q^2+3q-1),
$$

where $N_0=5q^3-6q^2+3q-1$. A zero contraction admits all $q^3$
third rows; a nonzero contraction has a two-dimensional kernel with $q^2$
third rows.

The literal order-three anchors are

| $(q,n,k)$ | Deficient count | Total matrices | Reduced probability |
|---|---:|---:|---:|
| $(3,3,3)$ | $8\,163$ | $19\,683$ | $907/2\,187$ |
| $(5,3,3)$ | $439\,525$ | $1\,953\,125$ | $17\,581/78\,125$ |
| $(7,3,3)$ | $6\,188\,455$ | $40\,353\,607$ | $126\,295/823\,543$ |

### Exhaustive transition domains

The number of subspaces of $\mathbb F_q^3$ is
$2(q^2+q+1)+2$. An independent test enumerator visits dimensions and pivot
patterns in a fixed closed loop, assigns every free entry each residue, and
materializes the resulting span as an explicit sorted set of vectors. It
asserts the formula count and deduplicates only by explicit-set equality. It
precomputes sums and contraction images from those element sets, independently
of the production normalization and transition operations. For every ordered
subspace pair
$(U,V)$ and every $x\in\mathbb F_q^3$, it compares:

- canonical representation, equality, hash-fed sequence, and ordering;
- $x\in V^\perp$ against direct dot products with every vector of $V$;
- $U+\langle x\rangle$ against explicit set closure;
- $V+\phi(U,x)$ against the complete explicit image
  $\{\phi(u,x):u\in U\}$ and set closure, which pins basis-independence
  without enumerating redundant bases.

The closed domains are:

| Test | Tier | Subspaces | Ordered $(U,V,x)$ cases |
|---|---|---:|---:|
| `compressed_transitions_q3_all_subspaces` | fast | $28$ | $28^2\cdot3^3=21\,168$ |
| `compressed_transitions_q5_all_subspaces` | fast | $64$ | $64^2\cdot5^3=512\,000$ |
| `compressed_transitions_q7_all_subspaces_slow` | ignored `slow` | $116$ | $116^2\cdot7^3=4\,615\,408$ |

### Exhaustive full-matrix domains

Each matrix test enumerates all $q^{3n}$ ordered $n\times3$ matrices
exactly once. It compares the compressed transition decision and terminal
count with both the production permanental-rank predicate and
`gf2_algebra::testutil::permanental_rank_bruteforce`, which shares no permanent
or row-subset path with the production predicate. The fixed matrix domains are:

| Test | Tier | $(q,n,k)$ | Matrices |
|---|---|---:|---:|
| `compressed_matrices_q3_n3_k3` | fast | $(3,3,3)$ | $3^9=19\,683$ |
| `compressed_matrices_q3_n4_k3` | fast | $(3,4,3)$ | $3^{12}=531\,441$ |
| `compressed_matrices_q5_n3_k3_slow` | ignored `slow` | $(5,3,3)$ | $5^9=1\,953\,125$ |
| `compressed_matrices_q7_n3_k3_slow` | ignored `slow` | $(7,3,3)$ | $7^9=40\,353\,607$ |

No environment probe, estimated runtime, prior result, or runtime meaning of
`tractable` changes these cases or adds another exhaustive case. The
repository applies its five-second per-test limit to every fast test. Each
ignored test uses `#[ignore = "slow: ..."]` under the $600$-second slow-tier
limit.

### Complete fast and ignored-slow test list

The implementation adds exactly these other fast tests:

| Test | Fixed cases |
|---|---|
| `canonical_subspaces_all_supported_fields` | all subspaces for $q\in\{3,5,7\}$; normalization idempotence, encoding rejection, equality, hash-fed sequence, and total order |
| `compressed_initial_and_anchor_counts` | $q\in\{3,5,7\}$, $n\in\{0,1,2,3\}$, using the literal anchors above |
| `compressed_k1_k2_formula_anchors` | $k=1$ for $q\in\{3,5,7\}$, $1\le n\le8$; the four literal $k=2$ cases above |
| `proposal_rank_contract_q3` | hand-checked canonical states with $\dim V\in\{0,1,2,3\}$, all admissible rows, proposal mass, and ratio direction |
| `proposal_rank_contract_q5` | the same four contraction ranks over $\mathbb F_5$ |
| `proposal_rank_contract_q7` | the same four contraction ranks over $\mathbb F_7$ |
| `proposal_one_step_unbiasedness_q3_q5` | every subspace-pair state for $q\in\{3,5\}$, summing proposal mass times $q^{-\dim V}$ over all admissible rows |
| `rare_event_stream_partition_and_golden_vectors` | manifested tags $1,2,3,4$; boundary indices for all four rare-event subdomains; fixed test cases $c=0,\ldots,15$ in each field slot |
| `rare_event_worker_resume_determinism` | target-shaped $n=1024$, fixed test cases $c=16,\ldots,31$, worker counts $1,2,7$, interruption after prefix rows $0,1,63,64,65,511$, same-address trajectory restart, and resume |

Together with the three transition tests and four matrix tests already named,
the implementation adds `proposal_one_step_unbiasedness_q7_slow` as an ignored
`slow` test over every $\mathbb F_7$ subspace-pair state and every admissible
row. It adds exactly these ignored stochastic coverage tests:

| Test | Exact anchor | Interval replicates | Runs per replicate | Trajectories per run |
|---|---:|---:|---:|---:|
| `rare_event_coverage_q3_n3_k3_slow` | $907/2\,187$ | $200$ | $32$ | $4\,096$ |
| `rare_event_coverage_q5_n3_k3_slow` | $17\,581/78\,125$ | $200$ | $32$ | $4\,096$ |
| `rare_event_coverage_q7_n3_k3_slow` | $126\,295/823\,543$ | $200$ | $32$ | $4\,096$ |

Each field therefore consumes exactly
$200\cdot32\cdot4\,096=26\,214\,400$ validation trajectories. For field
$q$, let $C_q$ be the number of the $200$ nominal $95\%$ intervals
that contain its exact anchor. Coverage is adequate exactly when

$$
C_q\ge180.
$$

This is the fixed binomial adequacy rule of at least $90\%$ empirical
coverage, applied separately to each field; equality passes and results are
never pooled across fields. The complete $200$-bit containment vector,
intervals, addressed exponent histograms, and $C_q$ remain in the validation
receipt whether the criterion passes or fails.

## Result precedence and contradiction handling

The exact compressed-state result publishes first as the raw integer count,
total $q^{3n}$, and reduced exact rational. It is the primary scientific
result. The stochastic point, nominal interval, variance, sample count, ESS,
and diagnostics are explicitly labeled `cross-check`.

At every exact anchor and at the target, disagreement means that the closed
stochastic interval excludes the exact rational. A usable target cross-check
with such exclusion receives verdict `contradiction`. A failed coverage field,
an extinction condition, or pooled final-weight
$\operatorname{ESS}/N<0.01$ receives verdict `unusable`. Both verdicts
preserve every fixed address, exponent histogram, run result, interval,
diagnostic, source identity, and exact value.

No disagreement, failed coverage count, low ESS, unexpected interval width, or
favorable result changes the proposal, ratio, target, address, sample count,
run count, interval level, critical value, coverage threshold, ESS threshold,
or test domain. A mechanical retry regenerates only the same address. There is
no replacement run, fresh index, selective omission, threshold change, or
result-dependent extension. The exact result is neither altered nor
invalidated by a stochastic contradiction or unusability verdict.

## Architecture and APIs

The mathematical primitive belongs to `gf2-algebra`.
`gf2_algebra::permanent::compressed_rank` exposes:

- the sealed supported-field contract and canonical three-coordinate vector;
- `CanonicalSubspace` and `CompressedRankState`;
- admissibility, canonical nullspace basis, contraction image, and successor;
- cached transition multiplicities;
- exact propagation returning the promoted arbitrary-precision
  `gf2_algebra::permanent::ExactProbability`.

The existing `ExactProbability` is promoted from fixed-width counts to the
single arbitrary-precision exact-probability abstraction; no parallel exact
rational type is introduced. `gf2-algebra` gains no production dependency on
`gf2-stats` or `gf2-sim`.

Generic exponent-histogram, exact weighted sums, ESS, independent-run variance,
and scaled-interval support belong to `gf2-stats` without permanent semantics.
Existing sampler types `MatrixAddress`, `StreamPurpose`, and `StreamIndex`
remain the one stream-address abstraction.

`gf2-sim` exposes a reusable `permanent_rare_event` library API that composes
the algebra transition with the statistics and sampler APIs. A thin binary
accepts only a frozen configuration, emits checkpoints and the final
cross-check receipt, and owns no proposal, weight, interval, or ESS formula.

## Implementation Steps

1. Promote `ExactProbability` in
   `crates/gf2-algebra/src/permanent/exact.rs` to arbitrary-precision counts,
   preserve the existing count/total semantics, add reduced base-ten output,
   and update its existing callers and tests.
2. Add the canonical subspace, contraction, reachable state, transition
   multiplicity, and exact recurrence APIs in
   `crates/gf2-algebra/src/permanent/compressed_rank.rs`; re-export them from
   `crates/gf2-algebra/src/permanent/mod.rs`.
3. Add the independent explicit-set subspace/transition oracle and the fixed
   exact tests in `crates/gf2-algebra/tests/compressed_rank.rs`, reusing the
   production predicate and existing independent brute-force oracle only at
   the final matrix-comparison boundary.
4. Add generic exact exponent-histogram, weighted-run, ESS, variance, and
   scaled Student-interval APIs in `crates/gf2-stats/src/weighted.rs` and
   re-export the module from `crates/gf2-stats/src/lib.rs`.
5. Add the addressed proposal and checkpointable trajectory/run composition
   in `crates/gf2-sim/src/permanent_rare_event.rs` and expose it from
   `crates/gf2-sim/src/lib.rs`.
6. Add the complete fast and ignored-slow suite in
   `crates/gf2-sim/tests/permanent_rare_event.rs`, with literal case tables and
   no runtime case selector.
7. Add a thin artifact consumer in
   `crates/gf2-sim/src/bin/permanent_rare_event.rs` that accepts the fixed
   target contract, persists canonical address-ordered checkpoints, and emits
   exact provenance and all contradiction fields.
8. Run focused release-mode tests, the named ignored-slow tests only on the
   slow tier, the repository CI contract, rustdoc, and deterministic
   worker/resume regeneration before any target artifact is accepted.

## Risks and fixed mitigations

- **State aliasing:** noncanonical bases could split one mathematical state.
  Unique RREF normalization, canonical keys, exhaustive subspace enumeration,
  and ordered maps remove that aliasing.
- **Incorrect likelihood direction:** proposal divided by target would invert
  the weight. The displayed target-to-proposal derivation and every-rank
  hand-checked tests pin the increment $q^{-\dim V}$.
- **Numerical underflow:** target weights lie far below binary64 range. Exact
  exponents and rational histograms carry all sums and moments; symbolic
  scaling is mandatory for interval rendering.
- **Weight collapse:** the fixed ESS rule labels the cross-check unusable and
  retains its diagnostics. It does not weaken the exact result or authorize
  more samples.
- **Shared-transition masking:** exact and stochastic consumers share the one
  canonical algebra transition, while the explicit-set transition oracle and
  independent full-matrix predicate/oracle validate that shared primitive over
  the closed domains.
- **Unsupported fields and column counts:** the compressed recurrence is
  intentionally limited to $q\in\{3,5,7\}$ and $k=3$. Semantic types reject
  other fields, and callers use the existing general permanental-rank
  predicate for other $k$.
- **Interrupted execution:** trajectory-specific addresses and canonical
  completed-tuple checkpoints preserve all completed work and reproduce an
  incomplete trajectory at the same address.

## Requirement evidence map

| Requirement | Binding sections |
|---|---|
| REQ-01 | [Canonical subspaces and state identity](#canonical-subspaces-and-state-identity); [Prefix-validity equivalence and updates](#prefix-validity-equivalence-and-updates) |
| REQ-02 | [Exact integer recurrence](#exact-integer-recurrence) |
| REQ-03 | [Proposal law and full support](#proposal-law-and-full-support) |
| REQ-04 | [Registered target and direct-sampling bound](#registered-target-and-direct-sampling-bound) |
| REQ-05 | [Fixed validation protocol](#fixed-validation-protocol) |
| REQ-06 | [ESS, extinction, degeneracy, and usability](#ess-extinction-degeneracy-and-usability) |
| REQ-07 | [Randomness, addressing, and restart contract](#randomness-addressing-and-restart-contract) |
| REQ-08 | [Result precedence and contradiction handling](#result-precedence-and-contradiction-handling) |
