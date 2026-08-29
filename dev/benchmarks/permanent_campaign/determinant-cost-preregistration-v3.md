# All-cell determinant-cost preregistration

This document fixes the determinant-companion timing cohort used to measure the
marginal cost of determinant evaluation for every cell in the permanent
zero-fraction campaign. It is committed before the cohort is built or timed.
The cohort extends the canonical `determinant_companion` harness and replaces
neither the campaign protocol nor the earlier representative-cell receipt.

## Frozen cell universe

The cohort contains the protocol's complete core universe:

- $q=3$ and every integer $n\in[4,28]$;
- $q=5$ and every integer $n\in[4,24]$;
- $q=7$ and every integer $n\in[4,20]$.

These $25+21+17=63$ cells are fixed before timing. All 63 are measured under
one uniform source and build identity, including the 11 cells covered by the
representative-cell receipt. The repeated cells are not selected by their
earlier values; repeating them avoids treating provenance reconstructed across
receipt versions as a new direct measurement.

## Process and timing contract

Each cell has exactly five fresh serial process outcomes, indexed
$e\in\{1,2,3,4,5\}$. A process evaluates all 63 cells in ascending $(q,n)$
order with one worker and no internal parallelism. Each cell performs:

1. deterministic fixture construction outside every timed window;
2. an untimed calibration warm-up that doubles its call count from one until
   its most recent probe reaches $\min(20\text{ ms},250\text{ ms})$, or the
   fixed maximum $2^{32}$ calls, then derives one fixed timed-window call count
   targeting $250\text{ ms}$;
3. exactly five timed repetitions using that fixed call count.

There is no statistical or result-dependent stopping rule. Every planned
process is attempted once in index order. A failed or signal-censored process
is retained with its exit status, and no process, repetition, or cell is
replaced or extended. The runner continues to later planned processes when the
host remains capable of doing so. A nonzero child status makes the cohort
command fail after the immutable receipt is written.

The timed boundary is `FieldMatrix::det` only. Fixture generation, matrix
construction, calibration, process launch, receipt serialization, and
validation are excluded. The measurement runs under the repository's
exclusive `dev/scripts/ccx1-bench-flock.sh` wrapper, pinned to logical CPUs
6--11. The determinant implementation is serial, so the worker count recorded
for every row is one.

## Fixture addressing

The root is `0xec22_205e_0000_0000`. Cell $(q,n)$ has seed

$$
s_{q,n}=\mathtt{0xec22\_205e\_0000\_0000}
          \mathbin{\mathtt{xor}} (q\ll48)
          \mathbin{\mathtt{xor}} (n\ll32).
$$

Each cell owns 32 deterministic row-major fixtures. Fixture $i\in[0,31]$ uses
seed $s_{q,n}+i$. Timed repetition $r\in\{1,\ldots,5\}$ of process $e$ starts
at

$$
a(e,r)=5(e-1)+(r-1),
$$

then cycles the 32-fixture pool for its fixed call count. Thus every recorded
window address $(q,n,e,r)$ and every start $a(e,r)\in[0,24]$ is unique within
its cell. Calibration always starts at fixture zero and is explicitly outside
the recorded address space.

## Receipt and pooling

The versioned all-cell machine-readable receipt is
`dev/benchmarks/permanent_campaign/determinant-cost-all-cells.csv` with schema
`determinant-companion-v3`. The representative-cell v2 receipt remains
immutable at its existing path; v3 evolves the same harness and measurement
contract rather than splicing new rows into old provenance. The v3 receipt
contains one outcome row for every planned
$(q,n,e)$, including failed or censored outcomes. A measured row records the
five raw repetition elapsed times, fixed calls per repetition, total sample
count, pooled elapsed determinant time, warm-up work, fixture starts, and the
complete runtime provenance. A non-measured row retains the process outcome,
status, and available provenance; unavailable measurement fields remain empty
and never enter a mean.

For a cell with measured outcome set $M_{q,n}$, the observed marginal cost is
formed only from pooled raw totals:

$$
t_{q,n}=\frac{\sum_{e\in M_{q,n}}T_e}
                 {\sum_{e\in M_{q,n}}C_e}.
$$

Here $T_e$ is the sum of the process's five recorded elapsed times and $C_e$
is its total determinant call count. The fixed-$N$ addition is
$N_{q,n}t_{q,n}$. No per-repetition reciprocal, mean of process means,
representative-order interpolation, or missing-outcome imputation is used.
Every contradiction and non-measured outcome remains visible in the rendered
receipt.

## Provenance and validation contract

Every row records the source revision and relevant-source dirty state; Rust
compiler and Cargo profile; executable SHA-256; host, CPU, kernel, governor,
boost state, affinity, process count, and worker count; exact seed and fixture
addresses; warm-up policy and observed warm-up work; repetition count, sample
count, and elapsed determinant time; process exit status; and the canonical
invocation identity.

The validator fails closed unless it proves all of the following:

- exactly the frozen 63 distinct cells and five outcome rows per cell;
- exactly one row for every $(q,n,e)$ address and 315 rows in total;
- one consistent clean source revision, compiler, profile, executable hash,
  host policy, process count, worker count, and timing contract;
- measured-row arithmetic, five raw repetitions, total elapsed time, total
  calls, and the 25 unique fixture starts per complete cell;
- non-measured outcomes have no fabricated timing values;
- the rendered 63-cell summary exactly reproduces the receipt's pooled costs,
  fixed-$N$ projections, and outcome counts.

Validation and rendering are deterministic functions of the immutable CSV.
Failure or censoring is a valid retained observation, but a missing planned row,
an inconsistent address, or a summary mismatch is a validation failure.

## Canonical execution

The producing revision builds the release-profile benchmark executable with
Rust 1.95, then runs the committed cohort runner once inside the exclusive
benchmark wrapper. The final receipt records the exact resolved executable and
runner argument vector. Timing begins only after this preregistration commit
and the producing source commit both exist.
