# Premeasurement plan v1

This is the frozen machine-readable premeasurement contract for issue
7a816262. It contains 60 lexicographically ordered cells and two nominated
configurations per cell: 120 configurations and 1,440 fresh processes
(12 processes per configuration). The harness names in the CSV are verified
against `dev/research/permanent-sampling-feas/src/backend.rs`; the manifest
tokens are carried over from
`dev/active/b8206228-permanent-statistics/gap-list.md`.

The per-cell balanced block is `A B B A`, repeated six times to produce 12
processes for each arm. The schedule is fixed before timing: cells are ordered
by numeric q and n, each cell's two CSV rows are A then B, and no process is
replaced or extended based on its result.

The default run ID is the stable `premeasure-v1`; set `CAMPAIGN_RUN_ID` to
start a separately named campaign. Thus a later `premeasure` invocation finds
the same durable process receipts unless the operator explicitly selects a new
run ID.

## Fixed protocol

- 60 cells × 2 configurations × 12 fresh processes = 1,440 processes.
- One locked `dev/scripts/ccx1-bench-flock.sh --full-host` session per night.
- The first process of the first session performs the harness's 90-second
  whole-machine warm-up. Every later process passes
  `--skip-machine-warmup`; a resumed session also skips it. Each process
  records the session warm-up state.
- Every process fixes 3 seconds of configuration warm-up, at least 5 timed
  repetitions, at least 5 timed seconds, and a 120-second per-process cap.
- There is no result-dependent extension, early stop, replacement, or
  discarded failure. A failed process remains in the durable record.
- `premeasure-collect` emits completed per-process rows only. It reports
  completeness and failures; pooled means are computed by downstream analysis.

## M provenance

| Manifest token | Harness backend | M | Basis |
|---|---|---:|---|
| `batch_parallel` | `cpu_rayon_batch_scalar` | 96 | `dev/benchmarks/permanent_campaign/backend-ordering.csv` rows C; `backend-ordering.md` §Verdict and §Protocol |
| `intra_matrix_parallel` | `cpu_rayon_intra_matrix` | 96 | `dev/benchmarks/permanent_campaign/backend-ordering.csv` rows B; `backend-ordering.md` §Verdict and §Protocol |
| `accelerator` | `gpu_hip` | 1024 | `dev/benchmarks/permanent_campaign/backend-ordering.csv` rows A and D; `backend-ordering.md` §Verdict and §Protocol |
| `generic_ryser` | `cpu_ryser_generic` | 13204, 610, or 31 | `dev/studies/b488f02c/throughput-2026-08-07.csv` committed operating points at q=3 n=12 and n=16 and q=7 n=20. Rows at unmeasured lower n use the nearest higher committed order's fixed operating point, recorded here before timing; this is not a result-dependent choice. |

The generic Ryser values are therefore 13,204 for q=3 n=4..12,
610 for q=3 n=13..15, and 31 for q=7 n=17..19. The harness's documented
adaptive default remains available when `--batch-size` is omitted, but this
plan always invokes the explicit override so M is fixed before timing.
