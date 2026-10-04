//! 5G NR quickstart: builds the BG1 / Z = 384 / rate-1/2 LDPC BICM pipeline
//! through the typestate preset ([`Pipeline::nr_5g`]) and drives one frame
//! through [`TopologyExecutor::run`], asserting a clean round-trip at 6 dB
//! QPSK. Z = 384 belongs to lifting set `i_LS = 1` (`@/citation/ThreeGpp2017`
//! Table 5.3.2-1: 384 = 3 * 2^7).
//!
//! Run with: `cargo run -p gf2-sim --example nr_5g_quickstart --release`

use std::num::NonZeroUsize;

use gf2_coding::ldpc::nr_5g::lifting_set_index;
use gf2_coding::modem::DemapMethod;
use gf2_core::BitVec;

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch};
use gf2_sim::presets::nr_5g::{BaseGraph, Channel, Nr5gDecoderConfig, Nr5gRate, NrModulation};
use gf2_sim::{Pipeline, Scheduler, TopologyExecutor};

fn main() {
    let i_ls = lifting_set_index(384).expect("384 is a valid lifting size");
    assert_eq!(i_ls, 1, "Z = 384 is in lifting set i_LS = 1");

    let pipeline = Pipeline::nr_5g()
        .base_graph(BaseGraph::Bg1)
        .lifting_set(i_ls)
        .lifting_size(384)
        .rate(Nr5gRate::R1_2)
        .decoder(Nr5gDecoderConfig::normalized_min_sum(25))
        .demap(NrModulation::Qpsk, DemapMethod::ExactLogMap)
        .channel(Channel::awgn(6.0))
        .seed(0x5697_4242)
        .build()
        .expect("BG1 / Z = 384 / rate 1/2 / QPSK is an in-scope tuple");
    assert_eq!(pipeline.stage_count(), 7);

    // BG1 full payload at Z = 384: k = 22 * 384 = 8448 message bits.
    let k = 22 * 384;
    let mut msg = BitVec::with_capacity(k);
    for i in 0..k {
        msg.push_bit(i % 5 < 2);
    }

    let scheduler = Scheduler::new(NonZeroUsize::new(2).expect("2 is non-zero"), false, 42);
    let sink = TopologyExecutor::run(
        &pipeline,
        &scheduler,
        Box::new(BitPackedBatch::new(vec![msg.clone()])),
    )
    .expect("the 5G NR chain runs to completion")
    .into_single()
    .expect("a linear chain has exactly one sink");
    let decoded = sink
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("the chain ends in recovered message bits");

    let recovered = decoded.frames[0] == msg;
    println!("5G NR BG1 / Z = 384 / rate 1/2 / QPSK quickstart");
    println!("message bits  k = {k}");
    println!(
        "frame verdict {}",
        if recovered { "decoded OK" } else { "ERROR" }
    );
    assert!(
        recovered,
        "the chain recovers the message at the 6 dB waterfall"
    );
}
