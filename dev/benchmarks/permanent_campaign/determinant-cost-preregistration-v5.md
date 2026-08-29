# Determinant-cost uncertainty cohort preregistration

This document fixes the canonical determinant-companion timing cohort for the
permanent zero-fraction campaign. It is committed before the cohort's producing
source is built or timed. The cohort provides determinant marginal-cost budget
evidence only; it does not rank permanent backends.

## Qualification and evidence boundary

The v3 and v4 preregistrations, machine receipts, and reports remain immutable
at their committed paths as historical and falsification evidence. They do not
enter this cohort's estimates. This independent cohort records the random
stream identity, quantifies process-to-process timing uncertainty, and pins the
campaign budget baseline used by every projection.

The machine-readable receipt is
`dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv`, its
rendered report is
`dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md`, and its
only accepted schema identity is `determinant-companion-v5`.

## Frozen cell and process universe

The cohort contains exactly these 63 cells:

- $q=3$ and every integer $n\in[4,28]$;
- $q=5$ and every integer $n\in[4,24]$;
- $q=7$ and every integer $n\in[4,20]$.

Each cell has exactly five fresh serial process outcomes indexed
$e\in\{1,2,3,4,5\}$. A process attempts all 63 cells in ascending $(q,n)$ order
with one worker and no internal parallelism. Every process is attempted once in
index order. No process, repetition, or cell is added, replaced, or rerun based
on a measured value or outcome.

## Timing, calibration, and stopping

Each process-cell outcome uses 32 deterministic row-major matrix fixtures.
Fixture construction remains outside every timed window. Calibration begins at
one determinant call and doubles until its latest probe reaches
$\min(20\text{ ms},250\text{ ms})$, or the call count reaches $2^{32}$. The
observed probe then determines one fixed call count targeting 250 ms. Exactly
five timed repetitions use that call count.

The timed boundary is `FieldMatrix::det` only. Matrix generation, conversion,
calibration, process launch, receipt serialization, and validation remain
outside it. There is no statistical or result-dependent stopping rule.

The cohort runs exactly once under the repository's canonical exclusive
`dev/scripts/ccx1-bench-flock.sh` wrapper, pinned to logical CPUs 6--11. Timing
does not begin unless the runtime-observed affinity is `6-11` and the complete
tracked source closure is clean. The closure includes `Cargo.toml`,
`Cargo.lock`, `crates/gf2-core`, `crates/gf2-algebra`, the exact benchmark
wrapper, this preregistration, the canonical v5 runner, and
`dev/simulation_results/permanent-zero-fraction/protocol.md`.

## Fixture addressing and random stream

The seed root is `0xec22_205e_0000_0002`. Cell $(q,n)$ has seed

$$
s_{q,n}=\mathtt{0xec22\_205e\_0000\_0002}
          \mathbin{\mathtt{xor}} (q\ll48)
          \mathbin{\mathtt{xor}} (n\ll32).
$$

Fixture $i\in[0,31]$ uses the wrapping-`u64` seed $s_{q,n}+i$. Its entries are
generated in row-major order by `gf2_core::rng::Lcg`. Starting with state equal
to the fixture seed, each entry first updates the state as

$$
x_{k+1}=
  6\,364\,136\,223\,846\,793\,005\,x_k
  +1\,442\,695\,040\,888\,963\,407\pmod{2^{64}},
$$

then maps the returned state to $x_{k+1}\bmod q$. Every raw row records:

- RNG algorithm `mmix_lcg_u64`;
- RNG version
  `multiplier-6364136223846793005-increment-1442695040888963407-wrapping-u64-v1`;
- entry mapping `advance-then-next-u64-mod-q-row-major-v1`.

Timed repetition $r\in\{1,\ldots,5\}$ of process $e$ starts at
$a(e,r)=5(e-1)+(r-1)$ and cycles the fixture pool for its fixed call count.
Calibration starts at fixture zero outside the recorded address space.

## Failure retention and estimators

The receipt contains one outcome row for every planned $(q,n,e)$ address,
including failed or signal-censored outcomes. A non-measured row retains its
status and available provenance, leaves unavailable timing fields empty, and
never enters an estimate. A nonzero child status makes the runner fail only
after it writes the complete immutable receipt; later planned processes are
attempted when the host remains capable of doing so.

For audit, each cell reports the pooled raw marginal cost over its measured
outcome set $M_{q,n}$:

$$
t^{\mathrm{pool}}_{q,n}=
\frac{\sum_{e\in M_{q,n}}T_e}{\sum_{e\in M_{q,n}}C_e}.
$$

The primary estimator and its uncertainty require all five planned processes.
For process $e$, let

$$
x_e=\frac{T_e}{C_e},
\qquad
\bar{x}=\frac{1}{5}\sum_{e=1}^{5}x_e,
\qquad
s=\sqrt{\frac{1}{4}\sum_{e=1}^{5}(x_e-\bar{x})^2}.
$$

The standard two-sided 95% Student-$t$ interval is

$$
\left[
\bar{x}-2.7764451051977987\frac{s}{\sqrt{5}},
\bar{x}+2.7764451051977987\frac{s}{\sqrt{5}}
\right].
$$

The process mean and this interval are distinct from the pooled audit value;
the interval is never presented as an interval for the pooled estimator. If
any process outcome is not measured, the table retains the pooled audit value
for the observed rows but marks the process mean, interval, projection, and
ceiling verdict as not estimable. No interpolation, missing-outcome imputation,
v3 or v4 row, or replacement process enters either estimator.

## Pinned campaign budget baseline

Every raw row and the rendered report identify the authoritative fixed-$N$ and
twelve-hour baseline by all of the following immutable values:

- path `dev/simulation_results/permanent-zero-fraction/protocol.md`;
- Git revision `7901430a324616b00b92c9fc23e3b3dbee66c291`;
- content SHA-256
  `a93d89fee1a898bf08726d76339dcd385d0f340959658a5010ae08aeb3579596`;
- operational ceiling 43,200 seconds per cell;
- reserve fraction 0.15 and productive-compute allowance 36,720 seconds.

The baseline fixes these sample counts:

| $q$ | $n$ | Fixed $N_{q,n}$ |
|---:|:---|---:|
| $3$ | $4\ldots20$ | $20\,000\,000$ |
| $3$ | $21\ldots28$ | $222\,223$ |
| $5$ | $4\ldots16$ | $16\,000\,000$ |
| $5$ | $17\ldots24$ | $160\,000$ |
| $7$ | $4\ldots16$ | $12\,244\,898$ |
| $7$ | $17\ldots20$ | $122\,449$ |

For each complete cell, the report multiplies the process mean and both
interval endpoints by $N_{q,n}$ and converts nanoseconds to seconds. Its
conservative twelve-hour verdict is `yes` exactly when the projected upper 95%
interval endpoint is at most 43,200 seconds. The pooled projection is not used
for this verdict.

## Provenance and validation contract

Every row records the random-stream identity and baseline identity above; the
source revision and complete-closure dirty state; Rust compiler and Cargo
profile; executable SHA-256; host, CPU, kernel, governor, boost state, affinity,
process and worker counts; exact seed and fixture addresses; calibration work;
raw repetition times; process status; and exact cohort and process argument
vectors.

The canonical validator fails closed unless it proves the exact 63-cell,
315-address universe; uniform clean cohort provenance; the v5 schema and cohort
identity; the frozen RNG and baseline identities; measured-row arithmetic;
five unique repetition starts per process and 25 per complete cell; exact
process-mean, sample-dispersion, Student-$t$ interval, and projected arithmetic;
empty timing fields on non-measured rows; and byte-for-byte agreement between
this cohort's receipt and rendered report. The active validator rejects every
superseded schema and representation.

## Canonical execution

The producing revision contains this preregistration and all producing source.
It builds the release benchmark executable with Rust 1.95, runs one
five-process cohort inside the exclusive wrapper, renders the report, and
validates the v5 artifacts. The recorded source revision, executable digest,
working-copy closure, invocations, random stream, budget baseline, and runtime
observations identify the measurement context without reconstructing a prior
working copy.
