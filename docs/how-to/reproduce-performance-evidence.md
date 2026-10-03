# Reproduce a performance claim

Each claim on the [performance evidence](../reference/performance-evidence.md)
page states its build and links its receipt at the commit that is its source
identity. Reproducing a claim means building that source with the recorded
toolchain, rerunning the receipt's measurement on your host, and recomputing
the claim's statistic from both raw data sets. A claim whose baseline is an
external library also needs that library built as its Baseline line states.
The claim's ratios hold for its stated host; on other hardware the comparison
shows whether its ordering holds there.

The worked example reproduces the
[batched $\mathbb{F}_3$ permanent kernel](../reference/performance-evidence.md#f3-permanent-batched-avx2)
claim. Its baseline is a gf2 kernel, so it needs no external library.

## Build the benchmark environment

The harness needs an x86-64 CPU with AVX2 and refuses to record without it. It
records `rustc +1.95.0 --version`, so the 1.95.0 toolchain must be installed
through rustup:

```sh
rustup toolchain install 1.95.0
```

From a gf2 clone, check out the claim's measured source revision in a separate
worktree:

```sh
git worktree add --detach ../gf2-f3-receipt 88474a74ceee
```

That revision commits no `Cargo.lock`, so Cargo resolves dependency versions
at build time.

Run every following command from the clone's root. Build the harness at that
revision and run its self-check, which asserts the size set, batch width and
fixture-seed derivation without timing anything:

```sh
./scripts/cargo-budget.sh cargo +1.95.0 bench \
    --manifest-path ../gf2-f3-receipt/Cargo.toml -p gf2-algebra \
    --bench batched_f3_permanent --features simd,test-support -- --self-check
```

## Run the measurement

The receipt ran five fresh executions of five $250$ ms repetitions under an
exclusive host lock on CPUs 6 to 11. Choose an idle core set on your host for
`taskset`, keep other load off the machine, and write the output outside the
measured worktree:

```sh
for e in 1 2 3 4 5; do
  CARGO_CI_NO_NICE=1 taskset -c 6-11 ./scripts/cargo-budget.sh cargo +1.95.0 \
      bench --manifest-path ../gf2-f3-receipt/Cargo.toml -p gf2-algebra \
      --bench batched_f3_permanent --features simd,test-support -- \
      --execution "$e" --repetitions 5 --target-ms 250 \
      --output /tmp/f3-repro.csv --append
done
```

`CARGO_CI_NO_NICE=1` stops `cargo-budget.sh` from lowering the run's CPU and
I/O priority. Before timing each size, the harness asserts that the three
backends return equal permanents on one fixture. Each row records `git_revision`,
`source_dirty`, `rustc`, `cpu_model`, `kernel` and the `governor` of CPU 6. The
harness sets `source_dirty` from `git status --porcelain --untracked-files=all`
in the measured worktree, so an output path inside it marks every row dirty.
Check that every row records revision
`88474a74ceee817040327db164c21f9fdd5ccf84` and `source_dirty=false`.

## Compare with the receipt

Extract the receipt's raw data from its pinned commit and check its digest
against the one the
[receipt](https://github.com/erankavija/gf2/blob/a8937d14ce000cc4fbcde5b2afc0d9e6da624eef/dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md)
states:

```sh
git show a8937d14ce00:dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.csv \
    > /tmp/f3-receipt.csv
sha256sum /tmp/f3-receipt.csv
```

The claim's statistic is a ratio of pooled rates per size $n$, where a
backend's pooled rate is its summed `matrices` over its summed `elapsed_ns`.
This script prints the
batched-to-scalar and scalar-to-direct ratios from both files:

```python
import csv, sys
from collections import defaultdict

B, S, D = (f"permanent_bipedal3_{k}_four_matrix"
           for k in ("batch_avx2", "scalar_singleword", "single_matrix_avx2"))

def rates(path):
    totals = defaultdict(lambda: [0, 0])
    for row in csv.DictReader(open(path)):
        t = totals[int(row["n"]), row["backend"]]
        t[0] += int(row["matrices"])
        t[1] += int(row["elapsed_ns"])
    return {key: m / ns * 1e9 for key, (m, ns) in totals.items()}

ref, new = rates(sys.argv[1]), rates(sys.argv[2])
print("n  B/S receipt  B/S here  S/D receipt  S/D here")
for n in sorted({n for n, _ in ref}):
    print(n, *(f"{x[n, a] / x[n, b]:.3f}"
               for a, b in ((B, S), (S, D)) for x in (ref, new)))
```

```sh
python3 pooled.py /tmp/f3-receipt.csv /tmp/f3-repro.csv
```

The receipt columns match the receipt's pooled-rate table to the printed
precision. The claim's ordering holds on your host when `B/S` and `S/D` both
exceed $1$ at every size; the paired columns show how the magnitudes compare.
