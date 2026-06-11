//! DAG topology executor semantics (issue `de160fc5`, criteria 1, 3, 4).
//!
//! Exercises [`TopologyExecutor`] over:
//!
//! * the **linear DVB-T2 BICM chain** — a graph-API build with scrambled
//!   insertion order topo-sorts to the same stage order as the typestate
//!   preset, and the executor's driven output is byte-identical to a
//!   sequential fold over the preset's stages;
//! * **fan-out** — one channel feeding two parallel demappers ("double
//!   demap" diversity): both branches execute, each with its own config;
//! * **fan-in** — two demapper branches feeding a single decoder: the decoder
//!   waits on both producers and receives their outputs concatenated in
//!   in-edge order;
//! * the **diamond ordering contract** — producers always run before
//!   consumers, fan-in waits on ALL producers;
//! * the **`Hybrid` routing arm** — a synthetic `ExecutionClass::Hybrid`
//!   stage is split per-batch (first `ceil(n/2)` frames, then the rest) and
//!   its outputs re-concatenated in order (no production Hybrid stage
//!   exists; the synthetic stage lives here);
//! * **intermediate-buffer reference counting** — a producer's output is
//!   dropped as soon as its last consumer has run;
//! * the **six-field per-stage tracing spans**
//!   `(worker_idx, snr_idx, batch_id, stream_id, stage_name, wall_us)`.
//!
//! Cyclic / disconnected construction is rejected at `Pipeline::build()` and
//! is covered by `tests/build_errors.rs` (+ the pre-existing
//! `tests/cyclic_chain.rs`); those shapes cannot reach this executor.

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gf2_coding::ldpc::dvb_t2::bit_interleaver::DvbT2Modulation;
use gf2_coding::ldpc::{DecoderAlgorithm, DecoderConfig};
use gf2_coding::modem::DemapMethod;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use gf2_sim::batch::{BitPackedBatch, HardDecisionBatch, LlrBatch};
use gf2_sim::graph::Chain;
use gf2_sim::presets::dvb_t2::{Channel, Modcod};
use gf2_sim::stage::{erase, AnyScratch, BatchSize, ExecutionClass, Stage, TypedBatch};
use gf2_sim::stages::{dvb_t2_bicm_stages, GrayQamDemap};
use gf2_sim::{Pipeline, Scheduler, StageError, TopologyExecutor};

/// The high-SNR operating point: the BP decoder early-terminates, keeping each
/// chain run fast-tier, while the channel still injects real noise.
const ES_N0_DB: f32 = 20.0;
const SEED: u64 = 0xDE16_0FC5;

fn decoder_config() -> DecoderConfig {
    DecoderConfig::new(DecoderAlgorithm::SumProduct, true)
}

/// The preset's demap N0 derivation (f64, rounded once — see
/// `Channel::demap_noise_var`), recomputed for graph-built chains.
fn demap_n0(es_n0_db: f32) -> f32 {
    let es_n0_lin = 10.0_f64.powf(f64::from(es_n0_db) / 10.0);
    let sigma_sq = 1.0 / (2.0 * es_n0_lin);
    (2.0 * sigma_sq) as f32
}

fn random_bbframe(k: usize, seed: u64) -> BitVec {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut bb = BitVec::with_capacity(k);
    for _ in 0..k {
        bb.push_bit(rng.random::<bool>());
    }
    bb
}

fn scheduler(workers: usize) -> Scheduler {
    Scheduler::new(NonZeroUsize::new(workers).unwrap(), false, SEED)
}

fn build_preset() -> Pipeline {
    Pipeline::dvb_t2()
        .modcod(Modcod::Normal {
            rate: CodeRate::Rate1_2,
            modulation: DvbT2Modulation::Qam16,
        })
        .decoder(decoder_config())
        .demap(DemapMethod::ExactLogMap)
        .channel(Channel::awgn(ES_N0_DB))
        .seed(SEED)
        .build()
        .expect("in-scope MODCOD builds")
}

// ---------------------------------------------------------------------------
// Linear chain: topo order matches the preset; executor matches a fold
// ---------------------------------------------------------------------------

/// Builds the 7-stage DVB-T2 BICM chain through the graph API with a
/// **scrambled insertion order** (inverse half added before the forward half),
/// so the recorded `StageId`s are NOT already topologically sorted and
/// `build()` must genuinely reorder.
fn build_graph_scrambled() -> Pipeline {
    let factory = dvb_t2_bicm_stages(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        decoder_config(),
        DemapMethod::ExactLogMap,
        demap_n0(ES_N0_DB),
    );

    let mut chain = Chain::new();
    // Insert in scrambled order: inverse (demap, deinterleave, decode) first…
    let mut inv_ids = Vec::new();
    for stage in factory.inverse {
        inv_ids.push(chain.add(stage));
    }
    // …then the channel, then the forward half (encode, interleave, map).
    let ch = chain.add(gf2_sim::stage::erase(gf2_sim::channels::Awgn::new(
        ES_N0_DB, 4,
    )));
    let mut fwd_ids = Vec::new();
    for stage in factory.forward {
        fwd_ids.push(chain.add(stage));
    }

    // Wire the canonical BICM order: encode → interleave → map → channel →
    // demap → deinterleave → decode.
    let order = [
        fwd_ids[0], fwd_ids[1], fwd_ids[2], ch, inv_ids[0], inv_ids[1], inv_ids[2],
    ];
    for pair in order.windows(2) {
        chain
            .connect(pair[0], pair[1])
            .expect("consecutive BICM hops are type-compatible");
    }
    chain.build().expect("valid linear DAG")
}

#[test]
fn test_linear_chain_topo_order_matches_preset() {
    // Criterion 1 + deliverable 3 bullet 1: build() must topo-sort the
    // scrambled insertion into the SAME stage order the preset produces.
    let preset = build_preset();
    let graph = build_graph_scrambled();

    assert_eq!(preset.stage_count(), graph.stage_count());
    for (i, (p, g)) in preset
        .stages()
        .iter()
        .zip(graph.stages().iter())
        .enumerate()
    {
        assert_eq!(
            p.input_type(),
            g.input_type(),
            "stage {i}: input type must match the preset's"
        );
        assert_eq!(
            p.output_type(),
            g.output_type(),
            "stage {i}: output type must match the preset's"
        );
        assert_eq!(
            p.execution_class(),
            g.execution_class(),
            "stage {i}: execution class must match the preset's"
        );
    }
    assert_eq!(
        preset.edges(),
        graph.edges(),
        "post-sort edges must be identical (every edge i → i+1)"
    );
}

#[test]
fn test_linear_chain_executor_output_matches_sequential_fold() {
    // The executor's driven output over the preset chain must be
    // byte-identical to a sequential fold over the same stages with the same
    // (default) scratches — proving correct topological per-stage execution.
    let pipeline = build_preset();
    let sched = scheduler(2);

    let k = 32208; // k_bch for Normal r1/2
    let bbframe = random_bbframe(k, SEED);

    // Sequential fold (the precedent machinery from preset_vs_graph.rs),
    // using each stage's own default scratch — identical to what the
    // executor allocates (ChannelScratch::default() for the AWGN stage).
    let fold_out = pipeline.stages().iter().fold(
        Box::new(BitPackedBatch::new(vec![bbframe.clone()])) as Box<dyn TypedBatch>,
        |batch, stage| {
            let mut scratch: Box<dyn AnyScratch> = stage.default_scratch();
            stage
                .process_any(batch.as_ref(), scratch.as_mut())
                .expect("fold drive succeeds")
        },
    );
    let fold_hard = fold_out
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("chain ends in HardDecisionBatch");

    // Executor drive.
    let exec_out = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![bbframe.clone()])),
    )
    .expect("executor run succeeds")
    .into_single()
    .expect("a linear chain has exactly one sink");
    let exec_hard = exec_out
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("chain ends in HardDecisionBatch");

    assert_eq!(
        exec_hard.frames[0], fold_hard.frames[0],
        "executor-driven and sequentially-folded outputs must be byte-identical"
    );
    assert_eq!(
        exec_hard.frames[0], bbframe,
        "high-SNR BICM roundtrip recovers the BBFRAME"
    );
}

// ---------------------------------------------------------------------------
// Diamond ordering contract (synthetic counting stages)
// ---------------------------------------------------------------------------

/// A synthetic CPU stage over `BitPackedBatch` that records (a) the global
/// tick at which it ran, (b) the input batch size it saw, and (c) a clone of
/// its input — then emits `frames_out`.
struct Probe {
    clock: Arc<AtomicU64>,
    tick: Arc<AtomicU64>,
    seen_size: Arc<AtomicUsize>,
    seen_input: Arc<Mutex<Option<BitPackedBatch>>>,
    frames_out: Vec<BitVec>,
}

/// The observer handles `Probe::new` returns alongside the stage: its run
/// tick, the input batch size it saw, and a clone of its input.
type ProbeHandles = (
    Arc<AtomicU64>,
    Arc<AtomicUsize>,
    Arc<Mutex<Option<BitPackedBatch>>>,
);

impl Probe {
    fn new(clock: &Arc<AtomicU64>, frames_out: Vec<BitVec>) -> (Self, ProbeHandles) {
        let tick = Arc::new(AtomicU64::new(0));
        let seen_size = Arc::new(AtomicUsize::new(usize::MAX));
        let seen_input = Arc::new(Mutex::new(None));
        (
            Self {
                clock: clock.clone(),
                tick: tick.clone(),
                seen_size: seen_size.clone(),
                seen_input: seen_input.clone(),
                frames_out,
            },
            (tick, seen_size, seen_input),
        )
    }
}

impl Stage<BitPackedBatch, BitPackedBatch> for Probe {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
        self.tick.store(
            self.clock.fetch_add(1, Ordering::SeqCst) + 1,
            Ordering::SeqCst,
        );
        self.seen_size.store(i.frames.len(), Ordering::SeqCst);
        *self.seen_input.lock().unwrap() = Some(i.clone());
        Ok(BitPackedBatch::new(self.frames_out.clone()))
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[test]
fn test_diamond_fan_out_fan_in_topological_order_and_merge() {
    // a → {b, c} → d. The fan-in consumer d must wait on BOTH producers and
    // receive their outputs concatenated in in-edge order.
    let clock = Arc::new(AtomicU64::new(0));
    let (a, (a_tick, _, _)) = Probe::new(&clock, vec![BitVec::zeros(4)]);
    let (b, (b_tick, b_size, _)) = Probe::new(&clock, vec![BitVec::ones(8)]);
    let (c, (c_tick, c_size, _)) = Probe::new(&clock, vec![BitVec::zeros(8)]);
    let (d, (d_tick, d_size, d_input)) = Probe::new(&clock, vec![BitVec::zeros(2)]);

    let mut chain = Chain::new();
    let ia = chain.add(erase(a));
    let ib = chain.add(erase(b));
    let ic = chain.add(erase(c));
    let id = chain.add(erase(d));
    chain.connect(ia, ib).unwrap();
    chain.connect(ia, ic).unwrap();
    chain.connect(ib, id).unwrap(); // in-edge order at d: b first…
    chain.connect(ic, id).unwrap(); // …then c.
    let pipeline = chain.build().expect("a diamond is a valid DAG");

    let sched = scheduler(4);
    let outputs = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![BitVec::zeros(4)])),
    )
    .expect("diamond runs");

    // Single sink: d.
    let out = outputs.into_single().expect("d is the only sink");
    assert_eq!(out.batch_size(), 1, "d's own output");

    // Topological order: a before both branches; d strictly after both.
    let (ta, tb, tc, td) = (
        a_tick.load(Ordering::SeqCst),
        b_tick.load(Ordering::SeqCst),
        c_tick.load(Ordering::SeqCst),
        d_tick.load(Ordering::SeqCst),
    );
    assert!(ta > 0 && tb > 0 && tc > 0 && td > 0, "every stage executed");
    assert!(ta < tb && ta < tc, "the source runs before both branches");
    assert!(
        td > tb && td > tc,
        "the fan-in consumer waits on ALL producers (d after b AND c)"
    );

    // Both branches saw the shared (fan-out) producer output of 1 frame.
    assert_eq!(b_size.load(Ordering::SeqCst), 1);
    assert_eq!(c_size.load(Ordering::SeqCst), 1);

    // The fan-in merge: d saw 2 frames, b's output first (in-edge order).
    assert_eq!(d_size.load(Ordering::SeqCst), 2, "fan-in concatenates");
    let seen = d_input
        .lock()
        .unwrap()
        .take()
        .expect("d recorded its input");
    assert_eq!(
        seen.frames[0],
        BitVec::ones(8),
        "b's frame first (edge order)"
    );
    assert_eq!(seen.frames[1], BitVec::zeros(8), "c's frame second");
}

// ---------------------------------------------------------------------------
// Fan-out: double demap (diversity)
// ---------------------------------------------------------------------------

#[test]
fn test_fan_out_double_demap_both_branches_execute() {
    // encode → interleave → map → channel → {demap_true_n0, demap_default_n0}.
    // Both demappers are sinks; both must execute, each with its own config.
    let factory = dvb_t2_bicm_stages(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        decoder_config(),
        DemapMethod::ExactLogMap,
        demap_n0(ES_N0_DB),
    );
    let k = factory.codec.k_bch();
    let n = factory.codec.n_ldpc();

    let mut chain = Chain::new();
    let mut ids = Vec::new();
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    let ch = chain.add(erase(gf2_sim::channels::Awgn::new(ES_N0_DB, 4)));
    ids.push(ch);
    // Two demappers with DIFFERENT assumed N0, so their LLRs observably
    // differ — proving each branch ran its own stage, not a shared one.
    let demap_true = chain.add(erase(GrayQamDemap::with_noise_var(
        DvbT2Modulation::Qam16,
        DemapMethod::ExactLogMap,
        demap_n0(ES_N0_DB),
    )));
    let demap_default = chain.add(erase(GrayQamDemap::new(
        DvbT2Modulation::Qam16,
        DemapMethod::ExactLogMap,
    )));
    for pair in ids.windows(2) {
        chain.connect(pair[0], pair[1]).expect("type-compatible");
    }
    chain.connect(ch, demap_true).expect("channel → demap A");
    chain.connect(ch, demap_default).expect("channel → demap B");
    let pipeline = chain.build().expect("fan-out DAG builds");

    let sched = scheduler(4);
    let bbframe = random_bbframe(k, SEED ^ 1);
    let outputs = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![bbframe])),
    )
    .expect("fan-out run succeeds");

    let sinks = outputs.into_outputs();
    assert_eq!(sinks.len(), 2, "both demap branches are sinks and executed");
    let llrs: Vec<&LlrBatch> = sinks
        .iter()
        .map(|(_, b)| {
            b.as_any()
                .downcast_ref::<LlrBatch>()
                .expect("each branch outputs LLRs")
        })
        .collect();
    for l in &llrs {
        assert_eq!(l.frames.len(), 1);
        assert_eq!(l.frames[0].len(), n, "full FECFRAME of LLRs per branch");
    }
    // The two branches used their own N0: the LLR scalings differ.
    let differs = llrs[0].frames[0]
        .iter()
        .zip(llrs[1].frames[0].iter())
        .any(|(x, y)| (x.value() - y.value()).abs() > 1e-3);
    assert!(
        differs,
        "branches with different demap N0 must produce different LLRs \
         (each branch executed its own stage)"
    );
}

// ---------------------------------------------------------------------------
// Fan-in: two demap branches feeding one decoder
// ---------------------------------------------------------------------------

#[test]
fn test_fan_in_two_demap_branches_feed_one_decoder() {
    // encode → interleave → map → channel → {(demapA → deintA),
    // (demapB → deintB)} → decode. The decoder waits on both branches and
    // decodes the 2-frame merged batch; at high SNR both recover the BBFRAME.
    let factory = dvb_t2_bicm_stages(
        CodeRate::Rate1_2,
        DvbT2Modulation::Qam16,
        decoder_config(),
        DemapMethod::ExactLogMap,
        demap_n0(ES_N0_DB),
    );
    let k = factory.codec.k_bch();
    let codec = factory.codec.clone();
    let interleaver = factory.interleaver.clone();

    let mut chain = Chain::new();
    let mut ids = Vec::new();
    for stage in factory.forward {
        ids.push(chain.add(stage));
    }
    let ch = chain.add(erase(gf2_sim::channels::Awgn::new(ES_N0_DB, 4)));
    ids.push(ch);
    for pair in ids.windows(2) {
        chain.connect(pair[0], pair[1]).expect("type-compatible");
    }

    let mk_branch = |chain: &mut Chain| {
        let demap = chain.add(erase(GrayQamDemap::with_noise_var(
            DvbT2Modulation::Qam16,
            DemapMethod::ExactLogMap,
            demap_n0(ES_N0_DB),
        )));
        let deint = chain.add(erase(gf2_sim::stages::BitDeinterleave::new(
            interleaver.clone(),
        )));
        chain.connect(demap, deint).expect("demap → deinterleave");
        (demap, deint)
    };
    let (demap_a, deint_a) = mk_branch(&mut chain);
    let (demap_b, deint_b) = mk_branch(&mut chain);
    chain.connect(ch, demap_a).expect("channel → branch A");
    chain.connect(ch, demap_b).expect("channel → branch B");

    let decode = chain.add(erase(gf2_sim::stages::DvbT2Decode::new(codec)));
    chain.connect(deint_a, decode).expect("branch A → decoder");
    chain.connect(deint_b, decode).expect("branch B → decoder");
    let pipeline = chain.build().expect("fan-in DAG builds");

    let sched = scheduler(4);
    let bbframe = random_bbframe(k, SEED ^ 2);
    let outputs = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![bbframe.clone()])),
    )
    .expect("fan-in run succeeds");

    let out = outputs.into_single().expect("the decoder is the only sink");
    let hard = out
        .as_any()
        .downcast_ref::<HardDecisionBatch>()
        .expect("decoder outputs HardDecisionBatch");
    assert_eq!(
        hard.frames.len(),
        2,
        "the decoder consumed BOTH branches' frames in one merged batch"
    );
    assert_eq!(
        hard.frames[0], bbframe,
        "branch A frame decodes to the BBFRAME"
    );
    assert_eq!(
        hard.frames[1], bbframe,
        "branch B frame decodes to the BBFRAME"
    );
}

// ---------------------------------------------------------------------------
// Hybrid routing arm (synthetic stage; split per-batch)
// ---------------------------------------------------------------------------

/// A synthetic `ExecutionClass::Hybrid` identity stage recording the batch
/// size of every `process` invocation. No production Hybrid stage exists;
/// this test-only stage exercises the executor's split-per-batch arm.
struct HybridProbe {
    calls: Arc<Mutex<Vec<usize>>>,
}

impl Stage<BitPackedBatch, BitPackedBatch> for HybridProbe {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, i: &BitPackedBatch, _: &mut ()) -> Result<BitPackedBatch, StageError> {
        self.calls.lock().unwrap().push(i.frames.len());
        Ok(i.clone())
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::Hybrid
    }
}

#[test]
fn test_hybrid_stage_split_per_batch() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut chain = Chain::new();
    chain.add(erase(HybridProbe {
        calls: calls.clone(),
    }));
    let pipeline = chain.build().expect("single Hybrid stage builds");
    let sched = scheduler(2);

    // 5 distinguishable frames → split into ceil(5/2)=3 then 2, processed as
    // two sub-batches and re-concatenated in order.
    let frames: Vec<BitVec> = (1..=5).map(BitVec::ones).collect();
    let out = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(frames.clone())),
    )
    .expect("hybrid run succeeds")
    .into_single()
    .expect("single sink");

    assert_eq!(
        *calls.lock().unwrap(),
        vec![3, 2],
        "the Hybrid arm splits the 5-frame batch per-batch into 3 + 2"
    );
    let out = out
        .as_any()
        .downcast_ref::<BitPackedBatch>()
        .expect("identity output type");
    assert_eq!(
        out.frames, frames,
        "the two half outputs are re-concatenated in order"
    );
}

#[test]
fn test_hybrid_stage_single_frame_processes_whole() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut chain = Chain::new();
    chain.add(erase(HybridProbe {
        calls: calls.clone(),
    }));
    let pipeline = chain.build().expect("builds");
    let sched = scheduler(1);

    let out = TopologyExecutor::run(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![BitVec::ones(3)])),
    )
    .expect("runs")
    .into_single()
    .expect("single sink");

    assert_eq!(
        *calls.lock().unwrap(),
        vec![1],
        "a single-frame batch is processed whole (no split of one frame)"
    );
    assert_eq!(out.batch_size(), 1);
}

// ---------------------------------------------------------------------------
// Intermediate-buffer reference counting
// ---------------------------------------------------------------------------

/// A batch carrying a drop-observable marker (held, never read: its Arc
/// strong count IS the observable).
struct TrackedBatch {
    #[allow(dead_code)]
    marker: Arc<()>,
}
impl BatchSize for TrackedBatch {
    fn batch_size(&self) -> usize {
        1
    }
}

/// A plain terminal batch (payload never read; only its type matters).
struct PlainBatch(#[allow(dead_code)] u8);
impl BatchSize for PlainBatch {
    fn batch_size(&self) -> usize {
        1
    }
}

/// Source: `PlainBatch → TrackedBatch` (clones the stage's marker into the
/// output, so the output's liveness is observable via the Arc strong count).
struct MakeTracked {
    marker: Arc<()>,
}
impl Stage<PlainBatch, TrackedBatch> for MakeTracked {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, _: &PlainBatch, _: &mut ()) -> Result<TrackedBatch, StageError> {
        Ok(TrackedBatch {
            marker: self.marker.clone(),
        })
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Middle: consumes the tracked batch, emits a plain one.
struct ConsumeTracked;
impl Stage<TrackedBatch, PlainBatch> for ConsumeTracked {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, _: &TrackedBatch, _: &mut ()) -> Result<PlainBatch, StageError> {
        Ok(PlainBatch(1))
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Tail: records the marker's strong count at the moment it runs.
struct CountObserver {
    marker: Arc<()>,
    observed: Arc<AtomicUsize>,
}
impl Stage<PlainBatch, PlainBatch> for CountObserver {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, _: &PlainBatch, _: &mut ()) -> Result<PlainBatch, StageError> {
        self.observed
            .store(Arc::strong_count(&self.marker), Ordering::SeqCst);
        Ok(PlainBatch(2))
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

/// Fan-out consumer: records the marker's strong count *while consuming* the
/// tracked batch (the shared buffer is alive during its own run), then emits a
/// plain batch.
struct ObserveTracked {
    marker: Arc<()>,
    observed: Arc<AtomicUsize>,
}
impl Stage<TrackedBatch, PlainBatch> for ObserveTracked {
    type Scratch = ();
    type CpuFallback = Self;
    fn process(&self, _: &TrackedBatch, _: &mut ()) -> Result<PlainBatch, StageError> {
        self.observed
            .store(Arc::strong_count(&self.marker), Ordering::SeqCst);
        Ok(PlainBatch(1))
    }
    fn execution_class(&self) -> ExecutionClass {
        ExecutionClass::CpuOnly
    }
}

#[test]
fn test_intermediate_buffer_dropped_after_last_consumer() {
    // A → B → C, where A's output carries a marker. The executor's refcount
    // must drop A's output buffer once B (its only consumer) has run, BEFORE
    // C executes — observed by C reading the marker's strong count.
    let marker = Arc::new(());
    let observed = Arc::new(AtomicUsize::new(0));

    let mut chain = Chain::new();
    let a = chain.add(erase(MakeTracked {
        marker: marker.clone(),
    }));
    let b = chain.add(erase(ConsumeTracked));
    let c = chain.add(erase(CountObserver {
        marker: marker.clone(),
        observed: observed.clone(),
    }));
    chain.connect(a, b).unwrap();
    chain.connect(b, c).unwrap();
    let pipeline = chain.build().expect("linear chain builds");
    let sched = scheduler(1);

    TopologyExecutor::run(&pipeline, &sched, Box::new(PlainBatch(0))).expect("runs");

    // Holders at C's execution: the test's `marker` + the MakeTracked stage's
    // own + the CountObserver's own = 3. If A's output buffer were still
    // retained, C would have observed 4.
    assert_eq!(
        observed.load(Ordering::SeqCst),
        3,
        "A's intermediate output must be dropped (refcount 0) before C runs"
    );
}

#[test]
fn test_fan_out_buffer_alive_for_all_consumers_then_dropped() {
    // A → {B, C} (fan-out: refcount 2 on A's output); B → D. The shared
    // intermediate buffer must stay alive while EACH consumer runs — B and C
    // both observe it — and be dropped once its LAST consumer has run: D,
    // executing in the wave after {B, C}, observes the count without it.
    let marker = Arc::new(());
    let observed_b = Arc::new(AtomicUsize::new(0));
    let observed_c = Arc::new(AtomicUsize::new(0));
    let observed_d = Arc::new(AtomicUsize::new(0));

    let mut chain = Chain::new();
    let a = chain.add(erase(MakeTracked {
        marker: marker.clone(),
    }));
    let b = chain.add(erase(ObserveTracked {
        marker: marker.clone(),
        observed: observed_b.clone(),
    }));
    let c = chain.add(erase(ObserveTracked {
        marker: marker.clone(),
        observed: observed_c.clone(),
    }));
    let d = chain.add(erase(CountObserver {
        marker: marker.clone(),
        observed: observed_d.clone(),
    }));
    chain.connect(a, b).unwrap();
    chain.connect(a, c).unwrap();
    chain.connect(b, d).unwrap();
    let pipeline = chain.build().expect("fan-out DAG builds");
    let sched = scheduler(2);

    TopologyExecutor::run(&pipeline, &sched, Box::new(PlainBatch(0))).expect("runs");

    // Constant holders: the test's `marker` + the A, B, C, D stages' own
    // clones = 5; +1 while A's output buffer is alive.
    assert_eq!(
        observed_b.load(Ordering::SeqCst),
        6,
        "the shared buffer is alive while consumer B runs"
    );
    assert_eq!(
        observed_c.load(Ordering::SeqCst),
        6,
        "the shared buffer is alive while consumer C runs"
    );
    assert_eq!(
        observed_d.load(Ordering::SeqCst),
        5,
        "the shared buffer is dropped after its LAST consumer, before D's wave"
    );
}

// ---------------------------------------------------------------------------
// Per-stage tracing spans (six fields)
// ---------------------------------------------------------------------------

mod span_capture {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// One captured span: its metadata name and recorded fields (rendered via
    /// `Debug`).
    pub struct CapturedSpan {
        pub name: &'static str,
        pub fields: HashMap<String, String>,
    }

    /// A minimal capturing `tracing::Subscriber` (no `tracing-subscriber`
    /// dependency): records every span's name and fields, including fields
    /// recorded after creation (`wall_us`).
    pub struct Capture {
        pub spans: Arc<Mutex<Vec<CapturedSpan>>>,
    }

    struct Visitor<'a>(&'a mut HashMap<String, String>);
    impl tracing::field::Visit for Visitor<'_> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.0
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    impl tracing::Subscriber for Capture {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            let mut fields = HashMap::new();
            attrs.record(&mut Visitor(&mut fields));
            let mut spans = self.spans.lock().unwrap();
            spans.push(CapturedSpan {
                name: attrs.metadata().name(),
                fields,
            });
            tracing::span::Id::from_u64(spans.len() as u64)
        }
        fn record(&self, id: &tracing::span::Id, values: &tracing::span::Record<'_>) {
            let mut spans = self.spans.lock().unwrap();
            let idx = (id.into_u64() - 1) as usize;
            if let Some(span) = spans.get_mut(idx) {
                values.record(&mut Visitor(&mut span.fields));
            }
        }
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, _: &tracing::Event<'_>) {}
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }
}

#[test]
fn test_per_stage_spans_carry_six_fields() {
    use gf2_sim::BatchHandle;

    // Global subscriber: the executor's spans are emitted on the scheduler's
    // rayon pool threads, which a thread-local subscriber would miss. This is
    // the only test in this binary installing one, and nextest runs each test
    // in its own process.
    let spans = Arc::new(Mutex::new(Vec::new()));
    tracing::subscriber::set_global_default(span_capture::Capture {
        spans: spans.clone(),
    })
    .expect("first and only global subscriber in this process");

    // A 3-stage chain covering the CpuOnly and Hybrid routing arms (the
    // GpuOnly arm needs a device; its span shape shares this code path).
    let calls = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(AtomicU64::new(0));
    let (p1, ..) = Probe::new(&clock, vec![BitVec::ones(2), BitVec::zeros(2)]);
    let (p2, ..) = Probe::new(&clock, vec![BitVec::ones(2)]);
    let mut chain = Chain::new();
    let a = chain.add(erase(p1));
    let h = chain.add(erase(HybridProbe {
        calls: calls.clone(),
    }));
    let b = chain.add(erase(p2));
    chain.connect(a, h).unwrap();
    chain.connect(h, b).unwrap();
    let pipeline = chain.build().expect("builds");
    let sched = scheduler(2);

    TopologyExecutor::run_with_handle(
        &pipeline,
        &sched,
        Box::new(BitPackedBatch::new(vec![BitVec::zeros(2)])),
        BatchHandle::new(7, 3),
    )
    .expect("runs");

    let spans = spans.lock().unwrap();
    let stage_spans: Vec<_> = spans
        .iter()
        .filter(|s| s.name == "pipeline_stage")
        .collect();
    assert_eq!(
        stage_spans.len(),
        3,
        "every stage start/end emits exactly one pipeline_stage span"
    );
    for span in &stage_spans {
        for field in [
            "worker_idx",
            "snr_idx",
            "batch_id",
            "stream_id",
            "stage_name",
            "wall_us",
        ] {
            assert!(
                span.fields.contains_key(field),
                "pipeline_stage span missing field `{field}`; has {:?}",
                span.fields.keys().collect::<Vec<_>>()
            );
        }
        assert_eq!(span.fields["snr_idx"], "3", "snr_idx from the BatchHandle");
        assert_eq!(
            span.fields["batch_id"], "7",
            "batch_id from the BatchHandle"
        );
        assert_eq!(
            span.fields["stream_id"],
            format!("{}", usize::MAX),
            "no GPU pool: stream_id records the NO_STREAM sentinel"
        );
    }
    // The Hybrid stage's span names the synthetic stage type.
    assert!(
        stage_spans
            .iter()
            .any(|s| s.fields["stage_name"].contains("HybridProbe")),
        "stage_name carries the concrete stage type name"
    );
    // The split still happened inside the span-wrapped Hybrid execution.
    assert_eq!(
        *calls.lock().unwrap(),
        vec![1, 1],
        "2-frame batch split 1+1"
    );
}

// ---------------------------------------------------------------------------
// GpuOnly routing (hip): known stages → the worker's owned stream; unknown
// GpuOnly stages → a typed error (never a silent default-stream process_any)
// ---------------------------------------------------------------------------

/// GPU-gated routing tests for the `GpuOnly` arm (round-1 finding 1): every
/// known GpuOnly stage type routes onto `scheduler.worker_stream(worker_idx)`
/// via its stream-aware entry point, and an unknown GpuOnly stage with an
/// active stream pool is a typed [`gf2_sim::BuildError::ExecutionValidation`].
#[cfg(feature = "hip")]
mod gpu {
    use super::*;
    use gf2_sim::batch::SymbolBatch;
    use gf2_sim::gpu::awgn::{GpuAwgn, GpuAwgnScratch};
    use gf2_sim::gpu::demap::GpuGrayQamDemapper;
    use gf2_sim::{BuildError, FatalError};

    fn gpu_present() -> bool {
        gf2_kernels_hip::host::device_mem_info().is_ok()
    }

    /// A scheduler with `gpu_enabled = true`: on the gfx1030 host this builds
    /// an active HIP stream pool, so `worker_stream` hands out owned streams.
    fn gpu_scheduler(workers: usize) -> Scheduler {
        Scheduler::new(NonZeroUsize::new(workers).unwrap(), true, SEED)
    }

    /// A synthetic GpuOnly identity stage the executor has NO stream-aware
    /// dispatch for (it is none of GpuLdpcBp / GpuAwgn / GpuGrayQamDemapper).
    struct UnknownGpuProbe;
    impl Stage<SymbolBatch, SymbolBatch> for UnknownGpuProbe {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &SymbolBatch, _: &mut ()) -> Result<SymbolBatch, StageError> {
            Ok(i.clone())
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::GpuOnly
        }
    }

    /// The CPU identity twin `Chain::build` requires as the registered §8
    /// fallback target for a GpuOnly stage (a substitution target only — not a
    /// DAG node, and not part of what these tests exercise).
    struct CpuIdentityProbe;
    impl Stage<SymbolBatch, SymbolBatch> for CpuIdentityProbe {
        type Scratch = ();
        type CpuFallback = Self;
        fn process(&self, i: &SymbolBatch, _: &mut ()) -> Result<SymbolBatch, StageError> {
            Ok(i.clone())
        }
        fn execution_class(&self) -> ExecutionClass {
            ExecutionClass::CpuOnly
        }
    }

    /// An unknown GpuOnly stage type while the worker owns a stream must be a
    /// typed `ExecutionValidation` error naming the stage — never a silent
    /// fall-through to a default-stream `process_any` (the contract-rot guard).
    #[test]
    fn test_unknown_gpu_only_stage_with_active_pool_is_typed_error() {
        if !gpu_present() {
            eprintln!(
                "skipping test_unknown_gpu_only_stage_with_active_pool_is_typed_error: \
                 no usable GPU"
            );
            return;
        }
        let mut chain = Chain::new();
        let gpu = chain.add(erase(UnknownGpuProbe));
        let cpu = chain.add(erase(CpuIdentityProbe));
        chain.register_fallback(gpu, cpu);
        let pipeline = chain.build().expect("single-stage chain builds");
        let sched = gpu_scheduler(1);
        assert!(sched.gpu_active(), "GPU host must build an active pool");

        let input = SymbolBatch::new(vec![vec![0.5_f32; 8]], vec![vec![-0.5_f32; 8]]);
        match TopologyExecutor::run(&pipeline, &sched, Box::new(input)) {
            Err(StageError::Fatal(FatalError::BuildError(BuildError::ExecutionValidation {
                reason,
            }))) => {
                assert!(
                    reason.contains("UnknownGpuProbe") && reason.contains("stream-aware"),
                    "reason must name the offending stage and the missing dispatch, got: {reason}"
                );
            }
            Err(other) => panic!("expected typed ExecutionValidation, got {other:?}"),
            Ok(_) => panic!("expected typed ExecutionValidation, got a successful run"),
        }
    }

    /// The `GpuAwgn` stage routed through the topology executor (worker-owned
    /// stream, `apply_on_stream`) must corrupt the batch **byte-identically**
    /// to the stage's own default-stream `process` path.
    #[test]
    fn test_gpu_awgn_routes_on_worker_stream_byte_identical() {
        if !gpu_present() {
            eprintln!(
                "skipping test_gpu_awgn_routes_on_worker_stream_byte_identical: no usable GPU"
            );
            return;
        }
        let stage = GpuAwgn::new(6.0, 4).with_seek(SEED, 1, 0);
        let input = SymbolBatch::new(
            vec![vec![0.25_f32; 64], vec![-0.75_f32; 64]],
            vec![vec![1.0_f32; 64], vec![0.5_f32; 64]],
        );

        // Reference: the stage's own (default-stream) erased process path.
        let mut scratch = GpuAwgnScratch::default();
        let reference = stage.process(&input, &mut scratch).expect("process");

        // Executor: single-stage chain on a gpu-enabled scheduler (the §8
        // fallback registration is a build() requirement for GpuOnly stages).
        let mut chain = Chain::new();
        let gpu = chain.add(erase(stage));
        let cpu = chain.add(erase(gf2_sim::channels::Awgn::new(6.0, 4)));
        chain.register_fallback(gpu, cpu);
        let pipeline = chain.build().expect("chain builds");
        let sched = gpu_scheduler(1);
        assert!(sched.gpu_active());
        let out = TopologyExecutor::run(&pipeline, &sched, Box::new(input))
            .expect("executor runs the GpuOnly stage on the worker stream")
            .into_single()
            .expect("single sink");
        let routed = out
            .as_any()
            .downcast_ref::<SymbolBatch>()
            .expect("stays SymbolBatch");

        for f in 0..2 {
            for k in 0..64 {
                assert_eq!(
                    reference.i[f][k].to_bits(),
                    routed.i[f][k].to_bits(),
                    "frame={f} I[{k}]"
                );
                assert_eq!(
                    reference.q[f][k].to_bits(),
                    routed.q[f][k].to_bits(),
                    "frame={f} Q[{k}]"
                );
            }
        }
    }

    /// The GPU max-log demap stage routed through the topology executor
    /// (worker-owned stream, `demap_batch_on_stream`) must emit LLRs
    /// **byte-identical** to the stage's own default-stream `process` path.
    #[test]
    fn test_gpu_demap_routes_on_worker_stream_byte_identical() {
        if !gpu_present() {
            eprintln!(
                "skipping test_gpu_demap_routes_on_worker_stream_byte_identical: no usable GPU"
            );
            return;
        }
        let stage = GpuGrayQamDemapper::new(DvbT2Modulation::Qam16, DemapMethod::MaxLog, 0.25);
        let i: Vec<f32> = (0..40).map(|k| 0.09 * k as f32 - 1.8).collect();
        let q: Vec<f32> = (0..40).map(|k| 1.6 - 0.08 * k as f32).collect();
        let input = SymbolBatch::new(vec![i.clone(), i], vec![q.clone(), q]);

        // Reference: the stage's own (default-stream) erased process path.
        let reference = stage.process(&input, &mut ()).expect("process");

        let mut chain = Chain::new();
        let gpu = chain.add(erase(stage));
        let cpu = chain.add(erase(gf2_sim::gpu::demap::CpuGrayQamDemapper::new(
            DvbT2Modulation::Qam16,
            DemapMethod::MaxLog,
            0.25,
        )));
        chain.register_fallback(gpu, cpu);
        let pipeline = chain.build().expect("chain builds");
        let sched = gpu_scheduler(1);
        assert!(sched.gpu_active());
        let out = TopologyExecutor::run(&pipeline, &sched, Box::new(input))
            .expect("executor runs the GpuOnly demap on the worker stream")
            .into_single()
            .expect("single sink");
        let routed = out
            .as_any()
            .downcast_ref::<LlrBatch>()
            .expect("demap emits an LlrBatch");

        assert_eq!(reference.frames.len(), routed.frames.len());
        for (f, (rf, of)) in reference
            .frames
            .iter()
            .zip(routed.frames.iter())
            .enumerate()
        {
            assert_eq!(rf.len(), of.len(), "frame {f} LLR count");
            for (b, (r, o)) in rf.iter().zip(of.iter()).enumerate() {
                assert_eq!(
                    r.value().to_bits(),
                    o.value().to_bits(),
                    "frame={f} LLR[{b}] differs: reference={} routed={}",
                    r.value(),
                    o.value()
                );
            }
        }
    }
}
