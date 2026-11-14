# 5c1c87b7 inventory: `exists()` and `is_file()` assertions under `crates/`

Source: `rg -n 'exists\(\)|is_file\(\)' crates/` at main 96fc9b751. Line numbers are pre-change.

Verdicts: **strengthened** (now asserts observable content), **kept** (existence is not the only assertion on that output, or the existence/absence is the contract), **removed**, **layout** (asserted repository layout, removed), **n/a** (production or bench code, or a skip guard, not an assertion in a test).

## Existence-only assertions on an output

| Location | Test | Verdict |
|---|---|---|
| gf2-core/tests/sparse_visualization_tests.rs:13 | test_sparse_save_identity_matrix | strengthened: decoded PNG compared pixel by pixel with `to_dense()` (dimensions included) |
| gf2-core/tests/sparse_visualization_tests.rs:43 | test_sparse_save_single_bit | strengthened: same |
| gf2-core/tests/visualization_tests.rs:13 | test_save_identity_matrix | strengthened: pixels compared with the source `BitMatrix` |
| gf2-core/tests/visualization_tests.rs:43 | test_save_single_bit | strengthened: same |
| gf2-core/tests/sparse_dense_image_equivalence.rs:25-26 | test_sparse_and_dense_produce_identical_images | strengthened: both images compared pixelwise with the source matrix, plus byte equality of the two |
| gf2-coding/src/simulation.rs:3305 | test_run_coded_iterative_parallel_with_output_path | strengthened: file has header plus one row per returned point |
| gf2-coding/src/bin/sim_runner.rs:1232 | test_run_curve_crc_25_15_product | strengthened: CSV has header plus one row per returned point |

Companion tests in the same files that asserted only dimensions (zeros, dual, random, sparse random) now compare every pixel. `test_save_word_boundary_widths` (widths 1, 63, 64, 65) is new. The three image test files share `gf2-core/tests/matrix_image/mod.rs`, and write into `test_scratch` directories instead of the working directory.

## Layout assertions (REQ-03)

| Location | Test | Verdict |
|---|---|---|
| gf2-coding/tests/grand_phase1_smoke.rs:266-267 | test_phase1_canonical_campaign_surface | layout: asserted `dev/campaigns/phase1_fig1.toml` and `phase1_fig3.toml` exist; test and `repo_path` helper removed. |

No other test asserts a repository path. The `dev/` vector-data checks below are data-availability guards for external standards vectors.

## Remaining hits

| Location | Context | Verdict |
|---|---|---|
| gf2-sim/tests/executor_failure_modes.rs:54, 81 | `!dir.exists()` after failure | kept: absence of a directory is the asserted contract, beside the error assertions |
| gf2-sim/src/executor/failure.rs:337, 350, 401, 464, 565 | `!dir.exists()` / `!impossible.exists()` | kept: same |
| gf2-sim/src/checkpoint/mod.rs:444 | `!path.exists()` after interrupted write | kept: atomicity contract, followed by directory listing assertions |
| gf2-sim/src/snr_checkpoint.rs:1136 | `!snr_0000.tmp` absent | kept: after successful load assertions |
| gf2-sim/src/snr_checkpoint.rs:1170 | orphaned tmp still present | kept: after the loaded-content assertions (`frames_completed`, `completed`) |
| gf2-core/src/io/field_matrix.rs:715 | `!temporary.exists()` | kept: after round-trip equality and typed-error assertions |
| gf2-core/src/test_scratch.rs:187, 198, 207, 215, 226, 235, 236 | scratch lifecycle tests | kept: existence/absence of the scratch tree is the behavior under test |
| gf2-coding/src/simulation.rs:3255, 3278 | output path tests | kept: followed by reading the file and asserting header / JSON opening |
| gf2-coding/src/simulation.rs:3821, 3962 | JSONL progress | kept: followed by line-count and content assertions |
| gf2-coding/src/simulation.rs:4224, 4300, 4559, 4749, 4883 | checkpoint files | kept: each followed by content assertions (`"completed": true`, tamper detection) |
| gf2-coding/src/simulation.rs:4495 | helper binary present | kept: precondition for the spawned-process assertions that follow (exit codes) |
| gf2-coding/src/simulation.rs:4529 | `if ckpt_file.exists()` | n/a: control flow in a polling loop |
| gf2-coding/tests/dvb_t2_chain_tp07a.rs:389, 393 | TP06/TP07a files present | kept: precondition with message; block parsing and content comparison follow |
| gf2-coding/tests/dvb_t2_chain_tp07a.rs:113, 379 | skip guards | n/a |
| gf2-coding/tests/dvb_t2_bit_interleaver_tp07a.rs:21, 92; ldpc_systematic_encoding.rs:12; dvb_t2_ldpc_verification_suite.rs:18; dvb_t2_ldpc_verification.rs:14; test_vectors/loader.rs:32, 75; test_vectors/mod.rs:16; dvb_t2_cell_mapping_vv001.rs:46; gf2-sim/tests/dvb_t2_chain_cells_vv001.rs:30; gf2-sim/tests/checkpoint_compat.rs:897 | skip guards and data-availability probes | n/a: guards select skipping, assert nothing |
| gf2-core/benches/selector_non_regression.rs:498; gf2-coding/benches/*.rs; gf2-coding/examples/ldpc_cache_file_io.rs:77; gf2-kernels-hip/build.rs:170; gf2-coding/src/simulation.rs:229, 237, 608; gf2-coding/src/ldpc/encoding/cache.rs:229; gf2-coding/src/ldpc/dvb_t2/concat.rs:561 | production, bench, example, build code | n/a: not test assertions |
