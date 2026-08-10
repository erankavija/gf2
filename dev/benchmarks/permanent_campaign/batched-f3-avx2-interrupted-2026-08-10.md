# Interrupted four-matrix $\mathbb{F}_3$ AVX2 timing cohort

These artifacts preserve a failed first attempt at the batched $\mathbb{F}_3$
permanent timing receipt. They are not a measurement result and must not be
used to claim a rate, speedup, ordering, or dispersion.

## Attempt provenance

| Item | Recorded value |
|---|---|
| First raw fragment | `batched-f3-avx2-interrupted-2026-08-10.csv`; executions 1--2, 180 rows |
| First raw SHA-256 | `e1526ec9ac014ecdbdf1b090f9a04974a46ae0ba3a27cd03ca99e155bc4d9e42` |
| First recorded source | `4a5c2233821d302fad1a582b67128416457736ee`; `source_dirty=false` |
| Second raw fragment | `batched-f3-avx2-interrupted-2026-08-10-revision-3ff81b7e.csv`; executions 3--5, 270 rows |
| Second raw SHA-256 | `ffd91775b7f55d917de98ea6e3b53607088aeb04f4905ae633f5d6b2becf8e85` |
| Second recorded source | `3ff81b7ea3dc53fb1d471881471b27c289751a17`; `source_dirty=false` |
| Benchmark binary SHA-256 | `bd0294ff0a68a94f14715fa9216a48600aaeb535f5bf442ca628a832986be266` |
| Toolchain | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Host | `fraktaali`; AMD Ryzen 9 5900X 12-Core Processor; AVX2 |
| Lock and affinity | `dev/scripts/ccx1-bench-flock.sh`; exclusive `/tmp/gf2-ccx1.lock`; CPUs 6–11 |
| Wrapper niceness | Best-effort `nice -n -5` was denied; the lock and affinity remained in effect |

The harness's first recorded timestamp is 2026-08-10 01:54:44 UTC. The raw
rows retain the exact seed root, fixture coordinates, source revision,
hardware identity, kernel, and governor captured by the harness. Although all
450 expected coordinates appear across the two fragments, their source
revisions differ, so their timings are not one coherent clean-source cohort.

## Command and interruption scope

The command started from the worktree root with the output path absent was:

```sh
test ! -e dev/benchmarks/permanent_campaign/batched-f3-avx2.csv && ./dev/scripts/ccx1-bench-flock.sh bash -lc 'for e in 1 2 3 4 5; do cargo +1.95.0 bench -p gf2-algebra --bench batched_f3_permanent --features simd,test-support -- --execution "$e" --repetitions 5 --target-ms 250 --output dev/benchmarks/permanent_campaign/batched-f3-avx2.csv --append; done'
```

An external command-session interruption was initially reported while the
locked process still held the raw file open. Renaming the first fragment during
that live session split the one command's output by pathname: executions 1--2
continued in the first fragment, while executions 3--5 created the second.
The preservation commit between those process launches changed the recorded
revision from `4a5c2233` to `3ff81b7e`. Contrary to the initial concern, every
retained row records `source_dirty=false`; the mixed revisions alone make the
combined data non-authoritative.

No raw row was discarded, rewritten, pooled, or used to make an interpretive
claim. Both fragments are explicitly failed-attempt artifacts, and the
canonical `batched-f3-avx2.csv` path is intentionally absent. A later complete
cohort must run against one unchanging clean source revision and satisfy the
full 450-coordinate validation before it is interpreted or receipted.
