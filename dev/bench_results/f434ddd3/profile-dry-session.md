# Untimed dry session of the batch decoder profile series (jit:f434ddd3)

Untimed and diagnostic: no flock wrapper, no benchmark window, one case, one
session; the figures below are not evidence of any rate. It runs the committed
`profile-session.sh` of `@/issue/3be770d5` directly on the `lane-dvb-w1` record
case of `dev/bench_results/ed3d490e/profile-cases.tsv`, with the case expanded
by the `series_cases` function of `run-profile.sh` and `ldpc-batch-profile`
linked as `ldpc-profile` (digest equal to its `build-identity.json` entry).

    GF2_BENCH=1 bash profile-session.sh <dir> <bin> <expanded-case> no

Result, 2.4 s wall:

- Status file `0`; the driver json reports `perf_controlled: true`, route
  `Lane(Avx2)`, one worker on cpu 0, 8 frames, one pass.
- The control channel enabled counting around the dispatch only: 169 samples
  of the fixed-period `cycles:u` recording, 78.1 % in `lane_variable_step_avx2`
  and 13.0 % in `lane_check_step_avx2`.
- The record path wrote `.total.txt`, `.addresses.tsv` (per-address counts) and
  `.symbols.txt`, the inputs of `summarize-profile.py`.
