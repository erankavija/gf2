//! 5G NR quickstart: build the BG1 / Z = 384 / rate-1/2 pipeline, run one frame.
//!
//! The 5G NR sibling of `examples/dvb_t2_quickstart.rs`. It builds the 5G NR
//! LDPC BICM pipeline through the typestate preset ([`Pipeline::nr_5g`]), then
//! drives **one** frame end-to-end (encode → interleave → map → AWGN → demap →
//! deinterleave → decode) with the generic per-stage executor
//! [`TopologyExecutor::run`]. Unlike the DVB-T2 preset there is no NR
//! sweep-level `Pipeline::run`; the per-stage executor is the NR drive path
//! (this is the same shape as the `Pipeline::nr_5g` rustdoc example).
//!
//! Z = 384 belongs to lifting set `i_LS = 1` (the a = 3 set of TS 38.212
//! Table 5.3.2-1: 384 = 3 * 2^7); the index is derived, not hardcoded. At a
//! 6 dB QPSK waterfall the single frame decodes back to the transmitted
//! message, so the example asserts a clean round-trip.
//!
//! Runtime: well under 1 s (one BG1 mother-code decode).
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
    // Derive the lifting set index for Z = 384 rather than hardcoding it.
    let i_ls = lifting_set_index(384).expect("384 is a valid lifting size");
    assert_eq!(i_ls, 1, "Z = 384 is in lifting set i_LS = 1");

    // Build the seven-stage 5G NR LDPC BICM pipeline.
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

    // Drive one frame end-to-end through the generic per-stage executor.
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
