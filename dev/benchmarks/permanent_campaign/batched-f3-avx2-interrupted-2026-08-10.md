# Interrupted four-matrix $\mathbb{F}_3$ AVX2 timing cohort

This artifact preserves an incomplete first attempt at the batched
$\mathbb{F}_3$ permanent timing receipt. It is not a measurement result and
must not be used to claim a rate, speedup, ordering, or dispersion.

## Attempt provenance

| Item | Recorded value |
|---|---|
| Raw attempt | `batched-f3-avx2-interrupted-2026-08-10.csv` |
| Raw SHA-256 | `3a23a82ed09ea2cf7ff0c6b6b2a3a962f21a9eed3edf5ac070ec0bfb9cdd3472` |
| Recorded source revision | `4a5c2233821d302fad1a582b67128416457736ee` |
| Recorded source state | `source_dirty=false` on every retained row |
| Benchmark binary SHA-256 | `bd0294ff0a68a94f14715fa9216a48600aaeb535f5bf442ca628a832986be266` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor; AVX2 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`; CPUs 6–11 |
| Wrapper niceness | Best-effort `nice -n -5` was denied; the lock and affinity remained in effect |

The harness's first recorded timestamp is 2026-08-10 01:54:44 UTC. The raw
rows retain the exact seed root, fixture coordinates, source revision,
hardware identity, kernel, and governor captured by the harness.

## Command and interruption scope

The command started from the worktree root with the output path absent was:

```sh
test ! -e dev/benchmarks/permanent_campaign/batched-f3-avx2.csv && ./dev/scripts/ccx1-bench-flock.sh bash -lc 'for e in 1 2 3 4 5; do cargo +1.95.0 bench -p gf2-algebra --bench batched_f3_permanent --features simd,test-support -- --execution "$e" --repetitions 5 --target-ms 250 --output dev/benchmarks/permanent_campaign/batched-f3-avx2.csv --append; done'
```

An external command-session interruption ended the locked process before the
command completed. The raw artifact contains 87 of the required 450 unique
coordinates: execution 1 only, all 15 coordinates for each of $n \in
\{8,12,16,20,24\}$, all five batched and all five scalar coordinates at
$n=28$, and two of the five single-matrix AVX2 coordinates at $n=28$.
Executions 2–5 have no rows.

No raw row was discarded, rewritten, or pooled. The artifact was renamed from
the canonical output name to preserve it as an interrupted cohort; the
canonical `batched-f3-avx2.csv` path is intentionally absent. A later complete
cohort must use the same committed harness and satisfy the full 450-coordinate
validation before it is interpreted or receipted.
