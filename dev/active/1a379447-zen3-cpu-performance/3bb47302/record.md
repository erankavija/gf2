# Profile series of the batch decoder (jit:3bb47302)

The series is measured: `dev/bench_results/f434ddd3/profile/repetitions.log`,
its execution log, records each session done and a terminal `series done`
record. The series ran `dev/bench_results/ed3d490e/run-profile.sh` over
`dev/bench_results/ed3d490e/profile-cases.tsv` in the benchmark window, each
session under its own `dev/scripts/ccx1-bench-flock.sh --full-host`
acquisition; `host.txt` beside the log records the host and the series
identity, which pins the profiling executable by digest.

## Summary

`dev/bench_results/f434ddd3/profile/profile.md` is the generated summary and
`profile-summary.json` its data; `attribution.tsv` lists every resolved
address with its inline chain and category. They hold, for the lane route and
the single-frame route on both codes, the sampled shares by category at one
and twenty-four workers with Wilson intervals and sample counts, the counter
groups and time per frame at every worker count with median intervals, and
the per-worker slowdown and lane-over-single-frame ratios with bootstrap
intervals.

The summarizer accepts a session only when the execution log records it done
after a start under the first session's identity, and stops when a case's
address listing and recorded sample total disagree. Its categories for this
series are the spans of `BATCH_RULES` in
`dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py`,
chosen by the profiling executable's recorded digest; the spanned files have no
difference between the revision `host.txt` records and the revision that
commits the spans (`git diff <revision> HEAD -- <spanned files>` is empty).
The session script makes `rep-01/callgraph/` only for the steady-state
profile's arms, so that directory is empty here and has no committed entry.

## Reproduction

From the worktree root, with the pinned executable present:

```
python3 -B dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py \
    dev/bench_results/f434ddd3/profile \
    --json dev/bench_results/f434ddd3/profile/profile-summary.json \
    --markdown dev/bench_results/f434ddd3/profile/profile.md \
    --binary dev/active/ed3d490e-zen3-ldpc-frame-simd/survey/arms/target/release/ldpc-batch-profile \
    --binary-sha256 647f1526ee1851bf9e0bd8b4ed94008e4b01079f6da723f74c95b27b7eb6c483
git diff --exit-code dev/bench_results/f434ddd3/profile
```

A second run leaves every generated file byte-identical. Without the pinned
executable the summarizer reuses the committed `attribution-chains.json` and
names it as `reused` in the address resolution record.

The earlier series keep their summaries: `reproduce-summaries.sh` of
`@/issue/859bc8cb`, run against `08b0c191e`, the summarizer's revision before
this change, prints:

```
dev/bench_results/07ca8585/v4-r1-resampled-profile	identical
dev/bench_results/3be770d5/v3-r1-steady-profile	identical
dev/bench_results/3be770d5/v4-r1-steady-profile	identical
```
