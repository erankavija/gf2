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

From the root of a gf2 clone, check out the claim's measured source revision
in a worktree inside the clone, since the benchmark-window runner takes job
worktrees relative to the clone root, and enter it:

```sh
export GF2=$PWD
git worktree add --detach .agents/worktrees/f3-receipt 88474a74ceee
cd .agents/worktrees/f3-receipt
```

Run every following command inside this worktree: the harness takes
`git_revision` and `source_dirty` from git queries in its working directory,
which Cargo sets to the package root. The revision has no
`scripts/cargo-budget.sh`, so the commands call the clone's copy. It commits
no `Cargo.lock`, so Cargo resolves dependency versions at build time. Build the
harness and run its self-check, which asserts the size set, batch width and
fixture-seed derivation without timing anything:

```sh
"$GF2"/scripts/cargo-budget.sh cargo +1.95.0 bench -p gf2-algebra \
    --bench batched_f3_permanent --features simd,test-support -- --self-check
```

## Run the measurement

The receipt ran five fresh executions of five $250$ ms repetitions under
`dev/scripts/ccx1-bench-flock.sh`, which holds the host benchmark mutex
exclusively for the whole command and pins it to CPUs 6 to 11 with
`taskset -c 6-11`. On a host with none of those CPUs `taskset` fails before
the command starts, and the window log records the job's exit with a nonzero
`rc`. A timed run is a job in the benchmark-window
[queue](https://github.com/erankavija/gf2/blob/456e24fe3c6df031b7b5840b9e931e78eedbe5e5/dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv), one tab-separated line of issue or job label, worktree
relative to the clone root, estimated minutes and command. The
[window runner](https://github.com/erankavija/gf2/blob/456e24fe3c6df031b7b5840b9e931e78eedbe5e5/dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh) exports `GF2_BENCH_WINDOW=1`, which the
wrapper requires, and runs each job under `bash -c` from its worktree. It
takes the root of the checkout that contains it from
`git rev-parse --show-toplevel` and resolves worktrees against that root.
`--print-root` prints the root and exits before the window opens; the printed
path is the clone root:

```sh
"$GF2"/dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh --print-root
```

Append the receipt's loop as a job. The command finds the clone root from the
worktree, runs the clone's wrapper, and sets `CARGO_CI_NO_LOCK=1` because the
wrapper already holds the mutex that `cargo-budget.sh` would take shared;
`CARGO_CI_NO_NICE=1` keeps `cargo-budget.sh` from lowering the run's priority:

```sh
cmd='r=$(git rev-parse --path-format=absolute --git-common-dir)/..; GF2=$r "$r"/dev/scripts/ccx1-bench-flock.sh bash -c "for e in 1 2 3 4 5; do CARGO_CI_NO_LOCK=1 CARGO_CI_NO_NICE=1 \"\$GF2\"/scripts/cargo-budget.sh cargo +1.95.0 bench -p gf2-algebra --bench batched_f3_permanent --features simd,test-support -- --execution \$e --repetitions 5 --target-ms 250 --output /tmp/f3-repro.csv --append; done"'
printf 'f3-repro\t.agents/worktrees/f3-receipt\t15\t%s\n' "$cmd" \
    >> "$GF2"/dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv
```

Open a window by running the runner from a shell:

```sh
"$GF2"/dev/active/1a379447-zen3-cpu-performance/bench-window/run-window.sh
```

The runner holds no lock; the job's wrapper takes the mutex when the job
starts. It creates the state directory, `.agents/bench-window/` in the clone
unless `GF2_WINDOW_STATE` names another, and appends its log there. If
`sccache` is installed, it stops each running server that holds a lock
descriptor or lacks `SCCACHE_IDLE_TIMEOUT=0` and starts one under that
setting. It then runs the queue lines in file order in the foreground and
returns after logging `window end`. A line whose worktree directory is absent
under the clone root is logged as `missing` and skipped.

A job's key is the first 16 hexadecimal digits of the SHA-256 of its label,
worktree and command joined by tabs. The runner appends the job's output to
`<key>.out` and, on exit status $0$, creates `<key>.done`, which makes every
later window skip the job. The runner's own exit status carries no job status;
the log does:

```sh
grep 'issue=f3-repro ' "$GF2"/.agents/bench-window/window.log
```

The `job start` line states the key and the `job exit` line states `rc`.
[`follow-window.sh`](https://github.com/erankavija/gf2/blob/456e24fe3c6df031b7b5840b9e931e78eedbe5e5/dev/active/1a379447-zen3-cpu-performance/bench-window/follow-window.sh), run from a second shell, redraws
each queue line's state from the same log, markers and output files until
interrupted. Its status word is the `systemctl --user is-active` result for
the unit that `GF2_WINDOW_UNIT` names and is independent of a runner started
from a shell. The receipt's protocol requires the window runner and the
wrapper's lock on every host.

Before timing each size, the harness asserts that the three backends return
equal permanents on one fixture. Each row records `git_revision`,
`source_dirty`, `rustc`, `cpu_model`, `kernel` and the `governor` of CPU 6. The
harness sets `source_dirty` from `git status --porcelain --untracked-files=all`,
so an output path inside the worktree marks every row dirty. Check that every
row records revision `88474a74ceee817040327db164c21f9fdd5ccf84` and
`source_dirty=false`.

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
This script prints the batched-to-scalar and scalar-to-direct ratios from both
files:

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
