# Library experiment audit

The table records crate artifacts that appear experiment-specific or are close
enough to the boundary to require an explicit disposition.

| Crate artifact | Verdict |
|---|---|
| `crates/gf2-algebra/examples/parallel_scaling_sweep.rs`, `s3_scalar_vs_avx2_sanity.rs` | **moved here** — both programs are examples in `experiments/permanent-benchmarks`. |
| `scripts/permanent-repro.sh` | **moved here** — the repro driver is `experiments/permanent-benchmarks/scripts/permanent-repro.sh`, which resolves the repository root from its new location. |
| `crates/gf2-coding/src/dvb_t2_bicm_harness.rs::{BaselineCellResult, parse_baseline_csv, BASELINE_MATRIX_CELL_COUNT}` | **moved here** — the baseline CSV model, parser, and matrix count live in the sole consumer, `dev/benchmarks/gf2-sim/baseline_runner`. The BICM channel and encoder remain generic DVB-T2 composition. |
| `crates/gf2-coding/src/fading.rs::RicianConfig::fig8/fig9/fig10` | **moved here** — the figure-named constructors are removed; their numeric parameters live in `sim_runner` campaign configuration and the consuming examples and tests. `RicianConfig` contains only generic channel parameters. |
| `crates/gf2-sim/src/executor/failure.rs::default_dump_dir` | **stays generic** — the default is under the system temporary directory, while campaigns retain the `PipelineConfig::diagnostic_dump_dir` override. |
| `crates/gf2-coding/src/bin/sim_runner.rs` | **stays generic** — it reads caller-supplied campaign TOML and builds several supported code and channel types. |
| `crates/gf2-core/src/tuning/baked.rs` | **stays generic** — it exposes the selected core tuning constants at shared production dispatch sites. |
| `crates/gf2-algebra/examples/permanent_demo.rs`, `crates/gf2-algebra/benches/permanent.rs` | **stays generic** — the example demonstrates the public permanent API and the Criterion bench covers the crate's general permanent implementation. |
| `crates/gf2-sim/src/bin/checkpoint_sweep.rs`, `hard_fail_probe.rs`; `crates/gf2-coding/src/bin/sim_checkpoint_helper.rs` | **stays generic** — these subprocess tools exercise checkpoint and failure contracts used by reusable simulation tests. |
| `crates/gf2-sim/examples/{dvb_t2_graph_api,dvb_t2_quickstart,dvb_t2_typestate,nr_5g_quickstart,gpu_hybrid}.rs` | **stays generic** — these examples demonstrate supported simulation APIs and backend composition. |
| `crates/gf2-core/examples/{sparse_block_csr_perfstat,m4rm_gray_table_perfstat,m4rm_multiply_perfstat}.rs`; `crates/gf2-kernels-hip/benches/gpu_vs_cpu_gray_qam.rs` | **stays generic** — these measure reusable operations and backend behavior over general inputs rather than one campaign's acceptance protocol. |
| `crates/gf2-core/examples/bench_csv_emitter.rs`, `crates/gf2-coding/examples/bench_sparse_csv_emitter.rs` | **tracked** — **Move benchmark CSV emitters into experiment packages**: move the fixed measurement protocols and output writers out of crate examples while keeping their seeded operation generators in the libraries. |
| `crates/gf2-algebra/benches/s1_n36_speedup.rs`, `determinant_companion.rs`, `batched_f3_permanent.rs` | **tracked** — **Move receipt-specific permanent benchmarks into `permanent-benchmarks`**: move these campaign harnesses and their Cargo targets while preserving their feature and receipt contracts. |
| `crates/gf2-core/benches/tuning_calibration.rs`, `crates/gf2-coding/benches/tuning_calibration.rs`, `crates/gf2-algebra/benches/tuning_calibration.rs` | **tracked** — **Move tuning calibration protocols into experiment packages**: keep each calibration protocol, fixtures, and receipt writing with its experiment while crate APIs retain reusable tuning mechanisms. |
| `crates/gf2-sim/benches/nr_5g_realtime.rs` | **tracked** — **Move the NR 5G real-time tuning bench into an experiment package**: keep the issue `23d3525f` benchmark protocol and result production outside the simulation library. |
| `crates/gf2-sim/src/bler_sweep.rs`, `src/testutil.rs::ComparisonCode`, `src/bin/ldpc_bler_sweep.rs`, `export_alist.rs` | **tracked** — **Move the AFF3CT comparison harness out of `gf2-sim` (C9)**: move the sweep, code selection, and export tools with their live comparison consumer. |
| `crates/gf2-coding/examples/gen_presentation_figures.rs` | **tracked** — **Move modem presentation figure generation into its experiment package**: move the fixed presentation sweeps and rendering entry point with their figures. |
| `crates/gf2-coding/examples/sogrand_crc_probe.rs` | **tracked** — **Move the SOGRAND CRC probe into its experiment package**: move its probe workload and result protocol out of the reusable coding examples. |
| `crates/gf2-coding/examples/{sparse_smoke_emit_expected,gf2pow32_smoke_emit_expected,bch_oracle_messages}.rs` | **stays generic** — these regenerate inputs for shared external-oracle and standard-conformance checks rather than research measurements. |
