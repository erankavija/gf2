# Untimed dry session of the batch decoder profile series (jit:f434ddd3)

Untimed and diagnostic: no flock wrapper, no benchmark window, one case, one
session. Its outputs contain timing fields but are not performance evidence.

The committed `profile-session.sh` of `@/issue/3be770d5` ran directly on the
`lane-dvb-w1` record case of `dev/bench_results/ed3d490e/profile-cases.tsv`:

    GF2_BENCH=1 bash profile-session.sh <dir> <bin> case.expanded.tsv no

`<bin>` held `ldpc-batch-profile` linked as `ldpc-profile`, its sha256 equal to
the entry in `dev/bench_results/ed3d490e/preparation/build-identity.json`.
The case was expanded by the `series_cases` function of
`dev/bench_results/ed3d490e/run-profile.sh`. Outputs are in `dry-session/`:

- `case.expanded.tsv`: the expanded case the session read.
- `record/lane-dvb-w1.status`: `0`, the driver's exit status.
- `record/lane-dvb-w1.json`: the driver record, with `perf_controlled` true,
  the lane route and one worker.
- `record/lane-dvb-w1.err`: the driver's worker-placement line.
- `record/lane-dvb-w1.total.txt`: the sample total of the control-enabled region.
- `record/lane-dvb-w1.addresses.tsv` and `.symbols.txt`: the per-address and
  per-symbol sample reports, the inputs of `summarize-profile.py`.
