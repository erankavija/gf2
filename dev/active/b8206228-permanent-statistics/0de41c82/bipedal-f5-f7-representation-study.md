# Permanent-shaped bit-sliced arithmetic study

This note records the representation hypotheses that issue `0de41c82` must
test when evaluating production-shaped HIP kernels for permanents over
$\mathbb{F}_3$, $\mathbb{F}_5$, and $\mathbb{F}_7$. It is a design input and
not a performance conclusion: every retained claim must be supported by the
issue's committed correctness and benchmark receipts.

## Decision summary

The literal two-plane bipedal representation is specific to $\mathbb{F}_3$.
Its permanent-specific structure generalizes further than its element-wise
field representation, however:

1. Keep the Gray-code row-sum accumulator in canonical bit planes.
2. Update all row sums with one word-level add or subtract circuit.
3. Detect any zero row sum with one active-lane mask.
4. Only when every row sum is nonzero, reduce the product through the cyclic
   group $\mathbb{F}_q^*$ using selector-mask population counts.

For $\mathbb{F}_5$, the existing three-plane `Packed5` representation is a
sensible starting point, but its scalar lane-by-lane horizontal fold leaves
the permanent-specific reduction unused. For $\mathbb{F}_7$, the current
16-lane nibble/LUT representation is a poor match for the permanent: a
three-plane Mersenne-add accumulator covers 64 rows and makes the hot update
register-local. The public general-purpose packed representation need not be
changed merely to test a permanent-specialized internal state.

## Why $\mathbb{F}_3$ is exceptional

`Bipedal3` encodes a lane as `(mag, sgn)`, with `mag` distinguishing zero from
nonzero and `sgn` distinguishing $1$ from $-1$. The sign bit is irrelevant
when the magnitude is zero, providing a don't-care codeword that simplifies
the Boolean circuits. This is a Boolean encoding of $\mathbb{F}_3$ arithmetic,
not a field embedding into $\mathbb{F}_2$.

$\mathbb{F}_3$ receives two simultaneous algebraic advantages:

- $3 = 2^2 - 1$, so Mersenne-style addition is cheap.
- $|\mathbb{F}_3^*| = 2$, so the only nonzero distinction is sign.

$\mathbb{F}_5^*$ has order four and $\mathbb{F}_7^*$ has order six. A literal
`(nonzero, sign)` pair therefore cannot encode either field. Their row sums
need three canonical value planes, or a larger zero-plus-log encoding whose
addition is substantially more expensive.

The production source is `crates/gf2-algebra/src/packed/bipedal3.rs`, and the
arithmetic background is [Scheinerman2024].

## The workload mismatch in the historical representation decisions

The historical $\mathbb{F}_7$ decision weighted a Gray-code Ryser step as a
packed addition followed by many packed multiplications. That model is
appropriate when packed lanes are independent matrices and each row owns a
packed accumulator. It is not the workload of the shipped row-packed CPU
kernel, where lanes are rows of one matrix.

The shipped row-packed loop performs:

```mermaid
flowchart LR
    A[One packed add or subtract] --> B[Horizontal product of active row lanes]
    B --> C[One scalar Ryser accumulation]
```

`permanent_bipedal5` and `permanent_bipedal7` call one packed add/subtract per
Gray step and then decode and multiply the active lanes serially in
`fold_mul_first_n`. They do not call general packed multiplication. The
current HIP kernels go further from the bipedal design: they keep byte arrays
and run explicit $O(n)$ update and product loops. Only the $\mathbb{F}_3$ HIP
kernel maintains a bit-sliced row-sum state.

Consequences for the study:

- General packed multiplication throughput must not determine the permanent
  representation by itself.
- Bulk independent-vector operator benchmarks are insufficient; the study
  must measure the dependency-chained accumulator used by Ryser.
- The current `Packed7::LANES = 16` limit is representation-induced, not an
  algebraic limit. Three `u64` planes cover orders through $n=64$ in one
  bit-sliced bundle.
- The existing feasibility study's suggestion of a multi-word nibble
  accumulator is not the only way to restore $\mathbb{F}_7$ CPU coverage
  above $n=16$.

Relevant production paths are:

- `crates/gf2-algebra/src/permanent/bipedal5.rs`
- `crates/gf2-algebra/src/permanent/bipedal7.rs`
- `crates/gf2-algebra/src/packed/packed5.rs`
- `crates/gf2-algebra/src/packed/packed7.rs`
- `crates/gf2-kernels-hip/hip/permanent/permanent_bipedal5.hip`
- `crates/gf2-kernels-hip/hip/permanent/permanent_bipedal7.hip`

## Constant-word horizontal product

Let `active` contain bits $0,\ldots,n-1$, and let `(b0, b1, b2)` be canonical
value planes. The active zero mask is

$$
Z = \neg(b_0 \lor b_1 \lor b_2) \land \text{active}.
$$

If $Z \ne 0$, the row product is zero. Otherwise every active lane lies in the
cyclic group $\mathbb{F}_q^*$, so its product is obtained by summing discrete
logarithms. Inactive lanes are excluded by `active`; they need not be rewritten
to the multiplicative identity.

### $\mathbb{F}_5$

Using generator $2$ gives

| Value | $1$ | $2$ | $4$ | $3$ |
|---|---:|---:|---:|---:|
| Exponent | $0$ | $1$ | $2$ | $3$ |

Let $M_v$ be the selector mask for value $v$. When $Z=0$, compute

$$
e = \operatorname{popcnt}(M_2)
  + 2\operatorname{popcnt}(M_4)
  + 3\operatorname{popcnt}(M_3)
  \pmod 4,
$$

then map $e \in \{0,1,2,3\}$ back to $1,2,4,3$. No general lane-wise packed
multiplication is required.

### $\mathbb{F}_7$

Using generator $3$ gives

| Value | $1$ | $3$ | $2$ | $6$ | $4$ | $5$ |
|---|---:|---:|---:|---:|---:|---:|
| Exponent | $0$ | $1$ | $2$ | $3$ | $4$ | $5$ |

When $Z=0$, compute

$$
e = \operatorname{popcnt}(M_3)
  + 2\operatorname{popcnt}(M_2)
  + 3\operatorname{popcnt}(M_6)
  + 4\operatorname{popcnt}(M_4)
  + 5\operatorname{popcnt}(M_5)
  \pmod 6,
$$

then map the exponent back through the same table.

For a uniform random matrix and any fixed nonempty column subset, row sums are
independent and uniform. The exact marginal probability that the horizontal
product reaches the nonzero slow path is therefore

$$
\Pr[Z=0] = \left(\frac{q-1}{q}\right)^n.
$$

Representative values are:

| Field and order | Nonzero slow path | Zero fast path |
|---|---:|---:|
| $\mathbb{F}_5$, $n=20$ | $1.153\%$ | $98.847\%$ |
| $\mathbb{F}_5$, $n=24$ | $0.472\%$ | $99.528\%$ |
| $\mathbb{F}_7$, $n=16$ | $8.489\%$ | $91.511\%$ |
| $\mathbb{F}_7$, $n=20$ | $4.582\%$ | $95.418\%$ |
| $\mathbb{F}_7$, $n=24$ | $2.473\%$ | $97.527\%$ |

The observed fast-path frequency is a useful diagnostic, but it does not
replace end-to-end timing because branches, population counts, selector
construction, register pressure, and correlated Gray-step state can affect
wall-clock performance.

## Arithmetic candidates

The executable study should retain rejected candidates and their falsifying
evidence.

### $\mathbb{F}_3$ controls

1. Current two-plane update plus six-stage halving horizontal fold.
2. The same update plus active-magnitude zero test and sign population-count
   reduction. This isolates whether the zero-dominant permanent workload
   benefits from the paper-style early return on the target GPU.

### $\mathbb{F}_5$ candidates

1. Current HIP byte/native modular arithmetic.
2. Canonical three-plane row sums using the production `Packed5` Boolean
   add/subtract circuit and the zero-mask/$C_4$ log-popcount fold.
3. If update profiling shows the selector circuit dominates, a canonical
   three-plane ripple-add plus conditional modulo-five reduction. This is an
   exploratory circuit candidate, not a presumed improvement; it requires
   exhaustive truth-table validation and generated-code inspection.

The existing public `Packed5` representation should remain the default
starting point. Its general multiplication circuit is not part of the
permanent hot path and should not be weighted as though it were.

### $\mathbb{F}_7$ candidates

1. Current compact LUT/native byte arithmetic.
2. Canonical three-plane row sums with Candidate D's Mersenne-fold add/subtract
   and the zero-mask/$C_6$ log-popcount fold.
3. Native modular arithmetic without the large LUT working set, when useful as
   a control for cache or constant-memory effects.

The prototype in `dev/research/f7_packing/src/cand_d.rs` is the starting
correctness oracle for Candidate D. Its expensive general packed multiply is
not required by the proposed row-packed permanent state.

## Required benchmark shapes

Every representation must be compared at the level where it is intended to
win. The study should report three nested measurements:

1. **Gray update:** one dependency-chained packed add/subtract on the same
   accumulator.
2. **Horizontal product:** zero-mask detection and the complete nonzero
   reduction, with fast and slow paths reported separately where practical.
3. **End-to-end Ryser:** matrix preparation, Gray initialization, update,
   product, partial-sum reduction, launch overhead, and transfer overhead under
   the issue's preregistered protocol.

Measure both execution mappings:

- the current one-thread-per-matrix path as a representation control;
- at least one wave-cooperative intra-matrix decomposition satisfying
  `REQ-01`.

Correctness coverage includes empty and singleton matrices where the API
supports them, all field values for arithmetic primitives, active-lane
boundaries, $n \in \{1,16,20,24,28\}$ where supported, Gray-range boundaries,
addition and subtraction transitions, zero-containing products, and every
nonzero exponent class. Performance points should include the scientifically
relevant $n$ values from `dev/studies/b488f02c/feasibility-study.md`; tiny
orders are correctness and overhead controls rather than headline crossover
points.

Receipts must distinguish:

- kernel-only and end-to-end throughput;
- single-matrix latency and batch throughput;
- launch duration and watchdog-safe chunking;
- wave utilization and achieved occupancy;
- registers, private memory, shared memory, and spills;
- bit-plane input preparation and persistent-buffer costs;
- observed zero-fast-path frequency and its exact marginal expectation;
- the best applicable CPU path, not merely a nominal SIMD-labelled path.

## Architectural boundary

A permanent-specialized bit-sliced state does not automatically justify a
second public packed-field abstraction. The final design must choose one of:

1. change the canonical `Packed7` representation at its source after broader
   `PackedField` evidence supports the change; or
2. keep the LUT representation public and define the three-plane state as an
   internal permanent-kernel representation with explicit scope, shared
   behavioral tests, and a tracked convergence condition if duplication
   appears.

The same rule applies to any alternative $\mathbb{F}_5$ add circuit. This
preserves the repository's convention-convergence and canonical-abstraction
requirements while permitting workload-specific kernel state.

## Decision rule

The study may select different arithmetic and execution mappings per field.
A go decision requires exact equivalence, reproducible receipts, a safe launch
duration, and an end-to-end crossover against the best applicable CPU path.
An operator-only win is insufficient. A no-go decision retains the candidate,
resource report, and falsifying measurements so the negative result remains
reproducible.

[Scheinerman2024]: https://arxiv.org/abs/2407.20205
