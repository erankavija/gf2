# Determinant-cost provenance-remediation preregistration

This document fixes a separate determinant-companion timing cohort for the
permanent zero-fraction campaign. It is committed before the cohort's producing
source is built or timed. The cohort provides determinant marginal-cost budget
evidence only; it does not rank permanent backends.

## Qualification and evidence boundary

The v3 preregistration, machine receipt, and report remain immutable at their
existing paths. Their source-dirty closure does not include the benchmark
wrapper, so they do not establish the complete working-copy identity required
for this issue. This cohort does not pool, replace, rewrite, or delete any v3
row. Its receipt and report are independent evidence under a repaired closure.

The machine-readable receipt is
`dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv`, its
rendered report is
`dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.md`, and its
schema identity is `determinant-companion-v4`.

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
tracked source closure is clean. The closure includes the exact wrapper, this
preregistration, the canonical runner and Rust harness, the relevant crates,
workspace manifests, and `Cargo.lock`.

## Fixture addressing

The root is `0xec22_205e_0000_0001`. Cell $(q,n)$ has seed

$$
s_{q,n}=\mathtt{0xec22\_205e\_0000\_0001}
          \mathbin{\mathtt{xor}} (q\ll48)
          \mathbin{\mathtt{xor}} (n\ll32).
$$

Fixture $i\in[0,31]$ uses seed $s_{q,n}+i$. Timed repetition
$r\in\{1,\ldots,5\}$ of process $e$ starts at
$a(e,r)=5(e-1)+(r-1)$ and cycles the fixture pool for its fixed call count.
Calibration starts at fixture zero outside the recorded address space.

## Failure retention and pooling

The receipt contains one outcome row for every planned $(q,n,e)$ address,
including failed or signal-censored outcomes. A non-measured row retains its
status and available provenance, leaves unavailable timing fields empty, and
never enters a mean. A nonzero child status makes the runner fail only after it
writes the complete immutable receipt; later planned processes are attempted
when the host remains capable of doing so.

For each cell, the directly observed marginal cost pools raw totals from its
measured outcome set $M_{q,n}$:

$$
t_{q,n}=\frac{\sum_{e\in M_{q,n}}T_e}
                 {\sum_{e\in M_{q,n}}C_e}.
$$

The fixed-$N$ addition is $N_{q,n}t_{q,n}$. No mean of process means,
interpolation, missing-outcome imputation, v3 row, or replacement process enters
the value.

## Provenance and validation contract

Every row records the source revision and complete-closure dirty state; Rust
compiler and Cargo profile; executable SHA-256; host, CPU, kernel, governor,
boost state, affinity, process and worker counts; exact seed and fixture
addresses; calibration work; raw repetition times; process status; and exact
cohort and process argument vectors.

The canonical validator fails closed unless it proves the exact 63-cell,
315-address universe; uniform clean cohort provenance; schema and cohort
identity; measured-row arithmetic; five unique repetition starts per process
and 25 per complete cell; empty timing fields on non-measured rows; and
byte-for-byte agreement between this cohort's receipt and rendered report. The
active validator accepts this cohort's schema and identity only.

## Canonical execution

The producing revision contains this preregistration and all producing source.
It builds the release benchmark executable with Rust 1.95, runs one five-process
cohort inside the exclusive wrapper, renders the report, and validates the v4
artifacts. The recorded source revision, executable digest,
working-copy closure, invocations, and runtime observations identify the
measurement context without reconstructing historical working-copy state.
