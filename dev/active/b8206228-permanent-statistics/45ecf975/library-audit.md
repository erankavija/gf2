# Library experiment audit

The table records crate artifacts that appear experiment-specific or are close
enough to the boundary to require an explicit disposition.

| Crate artifact | Verdict |
|---|---|
| `parallel_scaling_sweep.rs`, `s3_scalar_vs_avx2_sanity.rs` | **moved here** — both programs are examples in `experiments/permanent-benchmarks`. |
| `permanent-repro.sh` | **moved here** — the repro driver is `experiments/permanent-benchmarks/scripts/permanent-repro.sh`, which resolves the repository root from its new location. |
| `crates/gf2-coding/src/dvb_t2_bicm_harness.rs::{BaselineCellResult, parse_baseline_csv, BASELINE_MATRIX_CELL_COUNT}` | **moved here** — the baseline CSV model, parser, and matrix count live in the sole consumer, `baseline_runner`. The BICM channel and encoder remain generic DVB-T2 composition. |
| `crates/gf2-coding/src/fading.rs::RicianConfig::fig8/fig9/fig10` | **moved here** — the figure-named constructors are removed; their numeric parameters live in `sim_runner` campaign configuration and the consuming examples and tests. `RicianConfig` contains only generic channel parameters. |
| `crates/gf2-sim/src/executor/failure.rs::default_dump_dir` | **stays generic** — the default is under the system temporary directory, while campaigns retain the `PipelineConfig::diagnostic_dump_dir` override. |
| `crates/gf2-coding/src/bin/sim_runner.rs` | **stays generic** — it reads caller-supplied campaign TOML and builds several supported code and channel types. |
| `crates/gf2-core/src/tuning/baked.rs` | **stays generic** — it exposes the selected core tuning constants at shared production dispatch sites. |
| `crates/gf2-algebra/examples/permanent_demo.rs`, `crates/gf2-algebra/benches/permanent.rs` | **stays generic** — the example demonstrates the public permanent API and the Criterion bench covers the crate's general permanent implementation. |
| `crates/gf2-sim/src/bin/checkpoint_sweep.rs`, `hard_fail_probe.rs`; `crates/gf2-coding/src/bin/sim_checkpoint_helper.rs` | **stays generic** — these subprocess tools exercise checkpoint and failure contracts used by reusable simulation tests. |
| `crates/gf2-sim/examples/{dvb_t2_graph_api,dvb_t2_quickstart,dvb_t2_typestate,nr_5g_quickstart,gpu_hybrid}.rs` | **stays generic** — these examples demonstrate supported simulation APIs and backend composition. |
| `crates/gf2-core/examples/{sparse_block_csr_perfstat,m4rm_gray_table_perfstat,m4rm_multiply_perfstat}.rs`; `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs` | **stays generic** — these measure reusable operations and backend behavior over general inputs rather than one campaign's acceptance protocol. |
| `bench_csv_emitter.rs`, `bench_sparse_csv_emitter.rs` | **deleted** — the fixed CSV emitters have no live callers; reusable seeded generators remain in the libraries. |
| `s1_n36_speedup.rs`, `determinant_companion.rs`, `batched_f3_permanent.rs` | **moved here** — the three receipt producers and the batched harness tests live in `experiments/permanent-benchmarks`; their targets retain the feature gates and receipt contracts. |
| `tuning_calibration.rs`, `tuning_calibration.rs`, `tuning_calibration.rs` | **moved here** — all three producers and their protocol tests live in `experiments/tuning-calibration`; library APIs retain reusable tuning mechanisms and test observation hooks. |
| `nr_5g_realtime.rs` | **moved here** — the decode-rate protocol and its GPU feature live in `experiments/nr-5g-realtime`. |
| `crates/gf2-sim/src/bler_sweep.rs`, `src/testutil.rs::ComparisonCode`, `src/bin/ldpc_bler_sweep.rs`, `export_alist.rs` | **tracked** — **Move the AFF3CT comparison harness out of `gf2-sim` (C9)**: move the sweep, code selection, and export tools with their live comparison consumer. |
| `gen_presentation_figures.rs` | **deleted** — the fixed presentation renderer has no live caller; its plotting dependency is absent from the coding library. |
| `sogrand_crc_probe.rs` | **deleted** — the probe workload has no live caller. |
| `crates/gf2-coding/examples/{sparse_smoke_emit_expected,gf2pow32_smoke_emit_expected,bch_oracle_messages}.rs` | **stays generic** — these regenerate inputs for shared external-oracle and standard-conformance checks rather than research measurements. |
