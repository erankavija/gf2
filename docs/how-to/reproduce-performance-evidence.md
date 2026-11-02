# Reproduce a performance measurement

The benchmark harness and its configuration define a performance comparison.
Run the harness on the host being evaluated and keep its output below `target/`.
The batched F₃ permanent harness compares scalar, direct AVX2, and batched AVX2
kernels on deterministic fixtures; it requires an x86-64 CPU with AVX2.

## Build and check the harness

From the repository root, build the release harness and run its untimed
configuration checks:

```sh
./scripts/cargo-budget.sh cargo bench -p permanent-benchmarks \
    --bench batched_f3_permanent --features simd,test-support -- --self-check
```

The configuration checks cover the size set, batch width, and fixture-seed
derivation. Before timing a size, the harness verifies that all three backends
return equal permanents on the same fixture.

## Schedule a measurement

Timed runs use the benchmark window and `dev/scripts/ccx1-bench-flock.sh`.
The window runner exports `GF2_BENCH_WINDOW=1`; the wrapper requires it.
A queue line contains a job label, worktree path relative to the clone root,
estimated minutes, and command, separated by tabs.

Use the following command as a queue entry's command, from the repository root:

```sh
dev/scripts/ccx1-bench-flock.sh env CARGO_CI_NO_LOCK=1 \
    ./scripts/cargo-budget.sh cargo bench -p permanent-benchmarks \
    --bench batched_f3_permanent --features simd,test-support -- --execution 1 --repetitions 5 \
    --target-ms 250 --output target/benchmarks/f3-repro.csv
```

`CARGO_CI_NO_LOCK=1` lets the benchmark wrapper own the host mutex for the run.
The output records the execution identity, fixture seeds, backends, matrix
counts, and elapsed times. The source revision is a pointer for rebuilding.
Compare pooled rates for each size by dividing each backend's total matrix
count by its total elapsed time. Ratios depend on the measured host and build.
