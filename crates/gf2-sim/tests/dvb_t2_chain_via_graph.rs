//! The DVB-T2 BICM chain built through [`gf2_sim::graph::Chain`] (`add` →
//! `connect` → `build`) reconstructs the transmitted BBFRAME bit-exactly in a
//! noiseless forward→inverse roundtrip.

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch};
use gf2_sim::graph::Chain;
use gf2_sim::stage::{AnyScratch, AnyStage, TypedBatch};
use gf2_sim::stages::{dvb_t2_bicm_stages, DEFAULT_DEMAP_NOISE_VAR};

fn random_bbframe(k: usize, seed: u64) -> BitVec {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bb = BitVec::with_capacity(k);
    for _ in 0..k {
        bb.push_bit(rng.random::<bool>());
    }
    bb
}

fn run_pipeline_stages(
    stages: &[Box<dyn AnyStage>],
    initial: Box<dyn TypedBatch>,
) -> Box<dyn TypedBatch> {
    stages.iter().fold(initial, |batch, stage| {
        let mut scratch: Box<dyn AnyScratch> = stage.default_scratch();
        stage
            .process_any(batch.as_ref(), scratch.as_mut())
            .expect("process_any must succeed in the noiseless graph chain")
    })
}

fn assert_graph_chain_roundtrip(rate: CodeRate, modulation: DvbT2Modulation, seed: u64) {
    // No channel node: GrayQamMap connects straight into GrayQamDemap, so the
    // demapper uses the default N0.
    let factory = dvb_t2_bicm_stages(
        rate,
        modulation,
        DecoderConfig::new(DecoderAlgorithm::SumProduct, true),
        DemapMethod::ExactLogMap,
        DEFAULT_DEMAP_NOISE_VAR,
    );
    let k_bch = factory.codec.k_bch();

    let mut chain = Chain::new();

    let mut ids = Vec::new();
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    for stage in factory.inverse {
        ids.push(chain.add(stage));
    }

    for w in ids.windows(2) {
        chain
            .connect(w[0], w[1])
            .expect("each consecutive BICM connection is type-compatible");
    }

    let pipeline = chain.build().expect("the full BICM chain is a valid DAG");
    assert_eq!(pipeline.stage_count(), 6, "six BICM stages in the chain");
    assert_eq!(pipeline.edges().len(), 5, "five consecutive edges");

    let bbframe = random_bbframe(k_bch, seed);
    let input: Box<dyn TypedBatch> = Box::new(BitPackedBatch::new(vec![bbframe.clone()]));
    let terminal = run_pipeline_stages(pipeline.stages(), input);

    let recovered = terminal
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("the graph-built inverse chain must end in HardDecisionBatch");

    assert_eq!(
        recovered.frames[0], bbframe,
        "graph-built noiseless BICM chain must reconstruct the BBFRAME \
         bit-exactly for {rate:?} / {modulation:?} (DvbT2Concat ground truth)"
    );
}

#[test]
fn test_dvb_t2_chain_via_graph_roundtrip() {
    assert_graph_chain_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam16, 0xC0DE_F00D);
    assert_graph_chain_roundtrip(CodeRate::Rate1_2, DvbT2Modulation::Qam64, 0x5EED_1234);
}
