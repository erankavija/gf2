//! Novel chain via the graph API: splice a custom puncturing stage into a DAG.
//!
//! The presets ([`Pipeline::dvb_t2`], [`Pipeline::nr_5g`]) wire the *standard*
//! BICM chains. For research you often want a **non-standard** chain — a custom
//! stage the presets do not know about. This example shows the full recipe:
//!
//! 1. implement the [`Stage<I, O>`] trait for your own batch transform;
//! 2. [`erase`] it into an [`AnyStage`](gf2_sim::stage::AnyStage) and
//!    [`Chain::add`] it alongside the built-in stages;
//! 3. [`Chain::connect`] the type-checked edges and [`Chain::build`];
//! 4. drive one batch through with [`TopologyExecutor::run`].
//!
//! The custom stage here is a periodic **puncture** (`BitPackedBatch` →
//! `BitPackedBatch`) that drops every `period`-th bit — the kind of stage you
//! would splice between an encoder and a modulator to realise a higher code
//! rate. It is wrapped between two `Tag` passthrough stages purely to show the
//! puncturer composing with other graph nodes; the chain is self-contained
//! (no LDPC/QAM math) so it runs in milliseconds.
//!
//! Run with: `cargo run -p gf2-sim --example novel_chain_via_graph --release`

use std::num::NonZeroUsize;

use gf2_core::BitVec;

use gf2_sim::batch::BitPackedBatch;
use gf2_sim::error::StageError;
use gf2_sim::graph::Chain;
use gf2_sim::stage::{erase, ExecutionClass, Stage};
use gf2_sim::{Scheduler, TopologyExecutor};

/// An identity passthrough stage — stands in for any built-in `BitPackedBatch`
/// graph node the custom stage composes with.
struct Tag;

impl Stage<BitPackedBatch, BitPackedBatch> for Tag {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
        Ok(input.clone())
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// A periodic puncturing stage: drops every `period`-th bit of every frame.
///
/// This is the "novel" stage the presets do not provide. It is an ordinary
/// `Stage<BitPackedBatch, BitPackedBatch>` — the executor treats it exactly
/// like any built-in stage once it is erased and added to the chain.
struct Puncture {
    period: usize,
}

impl Stage<BitPackedBatch, BitPackedBatch> for Puncture {
    type Scratch = ();
    type CpuFallback = Self;

    fn process(&self, input: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
        let frames = input
            .frames
            .iter()
            .map(|frame| {
                let mut out = BitVec::with_capacity(frame.len());
                for i in 0..frame.len() {
                    if (i + 1) % self.period != 0 {
                        out.push_bit(frame.get(i));
                    }
                }
                out
            })
            .collect();
        Ok(BitPackedBatch::new(frames))
    }

    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

fn main() {
    const PERIOD: usize = 4; // drop every 4th bit → 75% rate

    // 1+2+3. Wire Tag -> Puncture -> Tag through the graph API, type-checked.
    let mut chain = Chain::new();
    let a = chain.add(erase(Tag));
    let b = chain.add(erase(Puncture { period: PERIOD }));
    let c = chain.add(erase(Tag));
    chain
        .connect(a, b)
        .expect("BitPackedBatch -> BitPackedBatch is compatible");
    chain
        .connect(b, c)
        .expect("BitPackedBatch -> BitPackedBatch is compatible");
    let pipeline = chain.build().expect("the custom chain is a valid DAG");

    // 4. Drive one 16-bit frame through the chain.
    let mut frame = BitVec::with_capacity(16);
    for i in 0..16 {
        frame.push_bit(i % 3 == 0);
    }
    let in_len = frame.len();

    let scheduler = Scheduler::new(NonZeroUsize::new(2).expect("2 is non-zero"), false, 7);
    let sink = TopologyExecutor::run(
        &pipeline,
        &scheduler,
        Box::new(BitPackedBatch::new(vec![frame])),
    )
    .expect("the custom chain runs end-to-end")
    .into_single()
    .expect("a linear chain has exactly one sink");
    let out = sink
        .as_any()
        .downcast_ref::<BitPackedBatch>()
        .expect("the chain ends in a BitPackedBatch");

    let out_len = out.frames[0].len();
    let dropped = in_len - out_len;
    println!("Novel chain: Tag -> Puncture(period={PERIOD}) -> Tag");
    println!("stages       {}", pipeline.stage_count());
    println!("input bits   {in_len}");
    println!("output bits  {out_len} ({dropped} punctured)");
    assert_eq!(
        out_len,
        in_len - in_len / PERIOD,
        "every PERIOD-th bit dropped"
    );
}
