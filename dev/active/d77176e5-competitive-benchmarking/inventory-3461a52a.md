# Flooding normalization inventory (3461a52a)

Every place where 0.75 is a flooding normalized min-sum default or a documented
recommendation, with the codes each serves. It satisfies REQ-01 of
@/issue/3461a52a; the [planning record](plan-3461a52a.md) cites it.

Every row sets 0.75 unless the Current value column says otherwise. D is DVB-T2,
N is NR (5G) BG1 or BG2, RM is rate-matched, and Pair is DVB-T2 normal 1/2
together with the NR BG1 Z=384 mother code (the CLI profile `nr-bg1-r12` is not
RM 1/2). Rates, LLRs and noise values are excluded.
`crates/gf2-coding/src/ldpc/core.rs` defaults to plain min-sum, and routes
supply no factor. Role "test" means unit-test or integration-test fixtures.

## Classification

A row is an explicit fixture, and keeps its named factor, when its role is
test or fixed harness data, when it lies under `dev/campaigns/` or
`dev/research/`, or when it is the DVB-T2 diagnosis pair
(`experiments/aff3ct-ldpc-gaps/src/bin/dvb_t2_fer_diag.rs` and
`experiments/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis.sh`), or the
layered-versus-flooding FER harness `crates/gf2-coding/tests/ldpc_layered_fer.rs`,
whose flooding reference follows the layered check rule by the @/issue/259acf37
ruling. The layered
configuration row belongs to @/issue/b8157e14. Every other row is a consumer of
the shared flooding normalization policy.

## Sites

|Location|Role|Current value|Codes served|
|---|---|---|---|
|`crates/gf2-coding/src/ldpc/nr_5g/mod.rs` (DEFAULT_NMS_SCALE, new)|default, constructor, doc|.75|N RM|
|`crates/gf2-coding/src/bin/sim_runner.rs` (resolve_algorithm)|CLI default, test|.75|N RM|
|`crates/gf2-sim/src/presets/nr_5g.rs` (normalized_min_sum)|constructor, doc|.75|N|
|`crates/gf2-sim/src/stages/nr_5g.rs` (Nr5gDecode::new)|constructor, doc, test|.75|N RM|
|`crates/gf2-coding/src/ldpc/layered.rs` (nr_5g, dvb_t2)|layered configuration, doc|N 6/8; D 7/8|N; D|
|`crates/gf2-coding/src/ldpc/nr_5g/batch.rs`|doc example, test|.75|N RM|
|`crates/gf2-coding/examples/ldpc_bler_check.rs`|example, doc|.75|N BG2 (256,121) RM|
|`crates/gf2-sim/examples/gpu_hybrid.rs`|example, comment|.75|D normal 1/2, 16-QAM|
|`docs/how-to/run-simulation-campaigns.md`|example|.75|N BG2 (1024,441) RM|
|`docs/reference/performance-evidence.md`|harness doc|.75|Pair|
|`crates/gf2-coding/benches/ldpc_dvb_t2_qc.rs`|harness|.75|D normal 1/2|
|`experiments/nr-5g-realtime/benches/nr_5g_realtime.rs`|harness|.75|N BG1 Z=384, RM 1/2|
|`experiments/aff3ct-ldpc-gaps/src/fast.rs` (NORMALIZATION)|harness|.75|Pair|
|`experiments/aff3ct-ldpc-gaps/src/bin/gf2-fast-bench.rs`, `experiments/aff3ct-ldpc-gaps/aff3ct/src/fast_bench.rs`|harness|.75 via `fast::NORMALIZATION`|Pair|
|`experiments/aff3ct-ldpc-gaps/src/bler_sweep.rs`, `experiments/aff3ct-ldpc-gaps/quick-compare.sh`, `experiments/aff3ct-ldpc-gaps/contracts.md`|harness, harness doc|.75|Pair|
|`experiments/aff3ct-ldpc-gaps/src/route_sweep.rs` (SweepConfig::issue_grid)|harness default|.75|N; D normal and short 1/2|
|`experiments/aff3ct-ldpc-gaps/src/bin/dvb_t2_fer_diag.rs`|example, test|.75|D normal 1/2|
|`experiments/aff3ct-ldpc-gaps/dvb-t2-r12-diagnosis.sh`|harness|.75 and .875|D normal 1/2|
|`dev/research/nr_kernel_feasibility/src/layered_convergence.rs` (NMS_ALPHA)|harness, doc|.75|N BG1 Z=384, mother and RM 1/2|
|`crates/gf2-coding/tests/ldpc_layered_fer.rs` (flooding_algorithm)|harness, test|D .875; N .75|Pair|
|`experiments/aff3ct-ldpc-gaps/families/` and `experiments/aff3ct-ldpc-gaps/plans/` entries named 55c3e433-adopted-vs-aff3ct, 55c3e433-check-variants, 55c3e433-integrated-adoption, 55c3e433-nr-route-pilot|fixed harness data|.75|Pair; nr-route-pilot only N mother|
|`dev/campaigns/` TOML files phase1_fig1, phase1_fig3, phase2_fig4 to fig6 with their _verify variants, phase3_fig7, phase4_fig8 to fig10, quick_fig1, quick_fig3, quick_fig7|harness|.75|N RM|
|`crates/gf2-coding/src/ldpc/` batch.rs, min_sum.rs, route.rs, dvb_t2/concat.rs|test|.75|synthetic QC, rows, profiles, D short 1/2|
|`crates/gf2-coding/tests/` dvb_t2_concat_batch_decoder.rs, ldpc_batch_contract.rs, ldpc_check_update_contract.rs, ldpc_decode_allocations.rs, ldpc_decode_dispatch.rs, ldpc_qc_contract.rs, nr5g_external_vectors.rs, tuning_profile_committed.rs, support/fresh_tuning_process.rs|test|.75|D short 1/2 and all D; N mother and RM; N BG1 Z=384; synthetic|
|`crates/gf2-kernels-simd/src/ldpc_lanes.rs`, `crates/gf2-kernels-simd/tests/ldpc_qc.rs`, `crates/gf2-kernels-hip/src/launch_ldpc_bp.rs`|test|.75|synthetic rows and QC|
|`crates/gf2-sim/src/` gpu/ldpc_bp.rs, gpu/nr_5g_ldpc.rs, stages/batch_decode.rs|test|.75|synthetic QC; N BG2 (256,121) RM|
|`crates/gf2-sim/tests/` batch_decode_worker_determinism.rs, checkpoint_compat.rs, checkpoint_identity.rs, determinism.rs, gpu_byte_identity.rs, gpu_ldpc_byte_identity.rs, gpu_nr_5g_byte_identity.rs, hybrid_resume.rs, parallel_determinism.rs, preset_vs_graph_byte_identity.rs|test|.75|D normal 1/2, 2/3, 3/4; N BG1 and BG2, mother and RM; synthetic QC|
|`experiments/dvb-t2-awgn/src/bin/dvb_t2_awgn_campaign.rs`|test|.75|D parser|
|`experiments/aff3ct-ldpc-gaps/src/` workload.rs, batch.rs, tuned.rs; `experiments/aff3ct-ldpc-gaps/aff3ct/src/lib.rs`; `experiments/aff3ct-ldpc-gaps/tests/` fast_worker_allocations.rs, bler_sweep_batch.rs|test|.75|synthetic QC; N BG1 Z=384; N BG2 (256,121) mother|
|`dev/tools/tuning-campaign-support/tests/protocol_contracts.rs`|test|.75|N BG1 mother|
