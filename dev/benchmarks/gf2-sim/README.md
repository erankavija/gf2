# gf2-sim benchmark receipts

Index of per-phase receipt files for the `gf2-sim` CPU+GPU FEC simulation
pipeline epic (`f9717e7e`). Each phase records its empirical evidence
(throughput numbers, speedup factors, determinism-contract verification) in
the file below.

| File | Phase / scope |
|------|----------------|
| `cpu-foundation-receipts.md` | Phase A — CPU pipeline foundation (Pipeline/Stage/Connector, parallel dispatch, channels, presets) |
| `gpu-stages-receipts.md` | Phase B — HIP/ROCm GPU stages and multi-arch dispatch |
| `hybrid-executor-receipts.md` | Phase C — hybrid CPU+GPU executor, OOM fallback, drain-commit checkpointing |
| `parallelism-receipts.md` | Parallel-scaling and worker-count determinism receipts |
| `5g-nr-realtime.md` | 5G NR real-time throughput receipts |
| `comparison/` | Cross-tool comparison receipts (vs. reference implementations) |

The design contract these receipts verify is the Phase 0 design doc
`dev/active/ec530af9-pipeline-design.md` (single source of truth).
