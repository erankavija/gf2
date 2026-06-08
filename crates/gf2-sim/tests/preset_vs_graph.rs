//! Preset-vs-graph equivalence (criterion-3 of `81d05bab`; also closes the held
//! criterion-1 of the upstream graph task `c09d3e95`).
//!
//! Proves the DVB-T2 typestate-builder preset
//! ([`Pipeline::dvb_t2`](gf2_sim::Pipeline::dvb_t2)) compiles to a [`Pipeline`]
//! that is **structurally identical** to, and **executes byte-identically** to,
//! a hand-wired graph [`Chain`](gf2_sim::graph::Chain) over the same
//! `(rate, modulation, decoder, demap, seed)` tuple — including the AWGN channel
//! stage spliced between the forward and inverse halves.
//!
//! # What "identical" means here
//!
//! 1. **Structural** — both pipelines have the same `stage_count()`, the same
//!    per-stage `(input_type, output_type, execution_class)` triple in order,
//!    the same `edges()`, and the same relevant `config()` fields. This proves
//!    `build()` produces the same topology from either path.
//! 2. **Execution** — a fixed seeded BBFRAME driven through both pipelines yields
//!    a bit-identical terminal `HardDecisionBatch.frames[0]`. The channel stage's
//!    scratch (`ChannelScratch`) is seeded identically for both pipelines, so the
//!    AWGN noise realisation is the same and the recovered bits match exactly.
//!
//! # Cost
//!
//! Each config runs one full DVB-T2 Normal (n = 64800) encode + LDPC BP decode
//! roundtrip. The channel here is high-SNR (so the BP decoder early-terminates),
//! keeping each roundtrip well under the 5 s fast-tier budget (the equivalent
//! noiseless graph roundtrip measures ~0.07 s); no `#[ignore]` is needed.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch};
use gf2_sim::channels::awgn::ChannelScratch;
use gf2_sim::graph::Chain;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::stage::{AnyScratch, ExecutionClass, TypedBatch};
use gf2_sim::stages::dvb_t2_bicm_stages;
use gf2_sim::{Pipeline, PipelineConfig};

/// The fixed Es/N0 (dB) every config's AWGN channel runs at. High enough that
/// the BP decoder early-terminates (keeping the test fast-tier) yet the channel
/// genuinely injects noise (so the execution comparison exercises the channel
/// scratch, not a no-op).
const ES_N0_DB: f32 = 12.0;

/// The shared `(decoder, demap)` configuration both pipelines use.
fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

/// One seeded pseudo-random BBFRAME of `k` bits.
fn random_bbframe(k: usize, seed: u64) -> BitVec {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bb = BitVec::with_capacity(k);
    for _ in 0..k {
        bb.push_bit(rng.random::<bool>());
    }
    bb
}

/// Builds the preset pipeline for one MODCOD via the typestate builder.
fn build_preset(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal { rate, modulation })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0_DB))
        .seed(seed)
        .build()
        .expect("in-scope MODCOD builds via the preset")
}

/// Builds the same DVB-T2 BICM chain by hand through the graph [`Chain`] API,
/// including the AWGN channel between the forward `GrayQamMap` and the inverse
/// `GrayQamDemap`. This mirrors exactly what the preset does internally, so the
/// two builds must agree structurally and in execution.
fn build_graph(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) -> Pipeline {
    let factory = dvb_t2_bicm_stages(rate, modulation, decoder_config(), DemapMethod::ExactLogMap);

    let mut chain = Chain::new();
    let mut ids = Vec::with_capacity(7);
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    // The channel stage is the SymbolBatch -> SymbolBatch hop between halves.
    ids.push(
        chain.add(gf2_sim::stage::erase(gf2_sim::channels::Awgn::new(
            ES_N0_DB,
            modulation.bits_per_cell(),
        ))),
    );
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }
    for pair in ids.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("each consecutive BICM hop is type-compatible");
    }

    // Mirror the preset's config so the structural config comparison is exact.
    let config = PipelineConfig {
        seed,
        esn0_db_points: Vec::new(),
        target_errors: 0,
        max_frames: 0,
        heartbeat_every_frames: 0,
        checkpoint_dir: None,
        tracing_log_path: None,
        parallelism: std::num::NonZeroUsize::new(1).expect("1 is non-zero"),
        strict_gpu: false,
    };

    chain
        .with_config(config)
        .build()
        .expect("the full BICM chain is a valid DAG")
}

/// Builds a per-stage scratch vector matching the pipeline's stage order: `()`
/// for the six pure-CPU codec/modem stages and a seeded [`ChannelScratch`] for
/// the single AWGN channel stage. The channel scratch is keyed on `seed` so both
/// pipelines draw the identical noise realisation.
///
/// The channel stage is the unique `SymbolBatch -> SymbolBatch` stage in the
/// chain (input type == output type == `SymbolBatch`); every other stage has
/// distinct input/output types, so this uniquely identifies the channel slot
/// without hard-coding its index.
fn scratches_for(pipeline: &Pipeline, seed: u64) -> Vec<Box<dyn AnyScratch>> {
    use std::any::TypeId;
    let symbol = TypeId::of::<gf2_sim::batch::SymbolBatch>();
    pipeline
        .stages()
        .iter()
        .map(|stage| -> Box<dyn AnyScratch> {
            if stage.input_type() == symbol && stage.output_type() == symbol {
                Box::new(ChannelScratch {
                    rng: ChaCha20Rng::seed_from_u64(seed),
                })
            } else {
                Box::new(())
            }
        })
        .collect()
}

/// Drives `pipeline` over `initial`, folding `process_any` across the stages
/// with the matched per-stage scratch, and returns the terminal batch.
fn drive(pipeline: &Pipeline, initial: Box<dyn TypedBatch>, seed: u64) -> Box<dyn TypedBatch> {
    let mut scratches = scratches_for(pipeline, seed);
    pipeline
        .stages()
        .iter()
        .zip(scratches.iter_mut())
        .fold(initial, |batch, (stage, scratch)| {
            stage
                .process_any(batch.as_ref(), scratch.as_mut())
                .expect("process_any must succeed in the BICM chain")
        })
}

/// Asserts the two pipelines are structurally identical: same stage count, same
/// per-stage `(input, output, execution_class)` triples in order, same edges,
/// and same config seed.
fn assert_structural_equality(preset: &Pipeline, graph: &Pipeline) {
    assert_eq!(
        preset.stage_count(),
        graph.stage_count(),
        "stage counts must match"
    );
    assert_eq!(
        preset.stage_count(),
        7,
        "DVB-T2 BICM-with-channel has 7 stages"
    );

    // Per-stage (input_type, output_type, execution_class) triples must match in
    // order. ExecutionClass is Copy + PartialEq, so it is compared directly.
    for (i, (p, g)) in preset
        .stages()
        .iter()
        .zip(graph.stages().iter())
        .enumerate()
    {
        assert_eq!(
            p.input_type(),
            g.input_type(),
            "stage {i} input type must match"
        );
        assert_eq!(
            p.output_type(),
            g.output_type(),
            "stage {i} output type must match"
        );
        assert_eq!(
            p.execution_class(),
            g.execution_class(),
            "stage {i} execution class must match"
        );
    }
    // Every BICM stage is pure-CPU.
    for (i, s) in preset.stages().iter().enumerate() {
        assert_eq!(
            s.execution_class(),
            ExecutionClass::CpuOnly,
            "stage {i} is CPU-only"
        );
    }

    assert_eq!(
        preset.edges(),
        graph.edges(),
        "edge lists must be identical (same from/to/type/size in order)"
    );
    assert_eq!(
        preset.config().seed,
        graph.config().seed,
        "config seed must match"
    );
    assert_eq!(
        preset.config().parallelism,
        graph.config().parallelism,
        "config parallelism must match"
    );
}

/// Full structural + execution equivalence check for one MODCOD.
fn assert_preset_matches_graph(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    let preset = build_preset(rate, modulation, seed);
    let graph = build_graph(rate, modulation, seed);

    // --- Structural equality (build() agrees) ---------------------------
    assert_structural_equality(&preset, &graph);

    // --- Execution equality (driven output is byte-identical) -----------
    let k_bch = {
        // Recover k_bch from a fresh factory; both pipelines encode the same.
        let f = dvb_t2_bicm_stages(rate, modulation, decoder_config(), DemapMethod::ExactLogMap);
        f.codec.k_bch()
    };
    let bbframe = random_bbframe(k_bch, seed);

    let preset_input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe.clone()]));
    let graph_input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe.clone()]));

    let preset_out = drive(&preset, preset_input, seed);
    let graph_out = drive(&graph, graph_input, seed);

    let preset_hard = preset_out
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("preset chain ends in HardDecisionBatch");
    let graph_hard = graph_out
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("graph chain ends in HardDecisionBatch");

    assert_eq!(
        preset_hard.frames[0], graph_hard.frames[0],
        "preset-built and graph-built pipelines must produce byte-identical \
         decoded frames for {rate:?} / {modulation:?} at seed {seed:#x}"
    );

    // Sanity: at this SNR the chain recovers the BBFRAME exactly (both paths).
    assert_eq!(
        preset_hard.frames[0], bbframe,
        "high-SNR BICM roundtrip must recover the transmitted BBFRAME"
    );
}

#[test]
fn test_preset_matches_graph_r1_2_16qam() {
    assert_preset_matches_graph(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
}

#[test]
fn test_preset_matches_graph_r1_2_64qam() {
    assert_preset_matches_graph(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}

#[test]
fn test_preset_matches_graph_r2_3_16qam() {
    assert_preset_matches_graph(CodeRate::Rate2_3, DvbT2Modulation::Qam16, 0xABCD_0001);
}
