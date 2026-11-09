//! DVB-T2 R1/2 normal-frame flooding NMS on `QcDecoder` against the batch
//! decoder's current route, at one worker with one frame and twelve workers
//! with eight frames each.
//!
//! Each worker owns its decoder and repeats one batch of noisy all-zero
//! frames; a measured round runs every worker once, in lockstep. Before
//! timing, both arms must decode each frame to the same word and outcome.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gf2_coding::ldpc::{
    DecodedWord, DecoderAlgorithm, DecoderConfig, LdpcBatchDecoder, LdpcCode, QcDecoder,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::num::NonZeroUsize;
use std::sync::Barrier;
use std::time::{Duration, Instant};

/// The NMS factor, cap and stopping rule of the `experiments/aff3ct-ldpc-gaps`
/// comparisons.
const FACTOR: f32 = 0.75;
const CAP: usize = 50;
/// BPSK over AWGN at this Eb/N0, near the R1/2 waterfall.
const EB_N0_DB: f64 = 1.2;
const CELLS: [(usize, usize); 2] = [(1, 1), (12, 8)];

fn frames(code: &LdpcCode, count: usize) -> Vec<Vec<Llr>> {
    let rate = code.k() as f64 / code.n() as f64;
    let sigma2 = 1.0 / (2.0 * rate * 10f64.powf(EB_N0_DB / 10.0));
    let mut rng = StdRng::seed_from_u64(0x0d31_1215);
    (0..count)
        .map(|_| {
            (0..code.n())
                .map(|_| {
                    // Box-Muller.
                    let u: f64 = 1.0 - rng.gen_range(0.0..1.0);
                    let v: f64 = rng.gen_range(0.0..1.0);
                    let noise = (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos();
                    let y = 1.0 + sigma2.sqrt() * noise;
                    Llr::new((2.0 * y / sigma2) as f32)
                })
                .collect()
        })
        .collect()
}

/// One worker's decoder on one arm.
enum Worker {
    Qc(Box<QcDecoder>),
    Current(Box<LdpcBatchDecoder>),
}

struct State {
    worker: Worker,
    decoded: Vec<BitVec>,
    outcomes: Vec<DecodeOutcome>,
}

impl State {
    fn new(arm: &str, code: &LdpcCode, frames: usize) -> Self {
        let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(FACTOR), true);
        let worker = match arm {
            "qc" => Worker::Qc(Box::new(
                QcDecoder::new(code, config).expect("DVB-T2 QC structure"),
            )),
            _ => Worker::Current(Box::new(LdpcBatchDecoder::new(
                code.clone(),
                config,
                CAP,
                DecodedWord::Codeword,
            ))),
        };
        Self {
            worker,
            decoded: vec![BitVec::new(); frames],
            outcomes: vec![DecodeOutcome::default(); frames],
        }
    }

    fn decode(&mut self, frames: &[&[Llr]], workers: NonZeroUsize) {
        match &mut self.worker {
            Worker::Qc(decoder) => {
                for ((frame, word), outcome) in
                    frames.iter().zip(&mut self.decoded).zip(&mut self.outcomes)
                {
                    *outcome = decoder.decode_codeword_into(frame, CAP, word);
                }
            }
            Worker::Current(decoder) => decoder.decode_batch_with_workers(
                frames,
                &mut self.decoded,
                &mut self.outcomes,
                workers,
            ),
        }
    }
}

/// Runs `rounds` lockstep rounds on `workers` threads and returns the time
/// from the first round's release to the last round's completion.
fn run_rounds(states: &mut [State], frames: &[&[Llr]], rounds: u64) -> Duration {
    let workers = NonZeroUsize::new(states.len()).unwrap();
    let start_line = Barrier::new(states.len() + 1);
    let end_line = Barrier::new(states.len() + 1);
    std::thread::scope(|scope| {
        for state in states.iter_mut() {
            let (start_line, end_line) = (&start_line, &end_line);
            scope.spawn(move || {
                start_line.wait();
                for _ in 0..rounds {
                    state.decode(frames, workers);
                }
                end_line.wait();
            });
        }
        start_line.wait();
        let start = Instant::now();
        end_line.wait();
        start.elapsed()
    })
}

fn bench_dvb_t2_qc(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
    let mut group = c.benchmark_group("ldpc_dvb_t2_qc");
    group.sample_size(20);
    for (workers, batch) in CELLS {
        let llrs = frames(&code, batch);
        let views: Vec<&[Llr]> = llrs.iter().map(Vec::as_slice).collect();
        let width = NonZeroUsize::new(workers).unwrap();

        let mut qc = State::new("qc", &code, batch);
        let mut current = State::new("current", &code, batch);
        qc.decode(&views, width);
        current.decode(&views, width);
        assert_eq!(qc.decoded, current.decoded, "arms decode different words");
        assert_eq!(
            qc.outcomes, current.outcomes,
            "arms report different outcomes"
        );

        group.throughput(Throughput::Elements((workers * batch) as u64));
        for arm in ["current", "qc"] {
            let mut states: Vec<State> = (0..workers)
                .map(|_| State::new(arm, &code, batch))
                .collect();
            group.bench_function(BenchmarkId::new(arm, format!("w{workers}-b{batch}")), |b| {
                b.iter_custom(|rounds| run_rounds(&mut states, &views, rounds))
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_dvb_t2_qc);
criterion_main!(benches);
