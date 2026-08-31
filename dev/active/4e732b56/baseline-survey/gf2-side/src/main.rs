//! gf2-side comparison harness for the external-baseline survey (jit:4e732b56).
//!
//! Measures the same two workloads and the same contract rows as the AFF3CT
//! and M4RI reference harnesses, on the pre-cutover `gf2-coding` BCH surface,
//! and emits the same CSV schema so the survey can put the columns side by
//! side.
//!
//! * `W1` — batch encoding through [`BchEncoder::encode_batch`] (the public
//!   allocating batch API) and through a sequential loop over
//!   [`BlockEncoder::encode`], so the survey can separate per-call allocation
//!   cost from encoding cost.
//! * `W2` — generator-matrix materialization through
//!   [`GeneratorMatrixAccess::generator_matrix`], on a freshly constructed
//!   code each trial so the code's internal cache never serves a trial.
//!
//! Every printed fact is observed at run time or is one of the protocol
//! constants declared here.

use std::env;
use std::time::Instant;

use gf2_coding::bch::dvb_t2::FrameSize;
use gf2_coding::bch::{BchCode, BchEncoder};
use gf2_coding::traits::{BlockEncoder, GeneratorMatrixAccess};
use gf2_coding::CodeRate;
use gf2_core::gf2m::Gf2mField;
use gf2_core::BitVec;

// ------------------------------------------------------------------ protocol

/// Independent trials per cell.
const TRIALS: usize = 7;
/// Untimed repetitions before a cell whose projected cost allows them.
const WARMUP_REPS: usize = 2;
/// Seed for the deterministic message fixtures.
const SEED: u64 = 0xAE03_BCD0;
/// Each timed region repeats the measured call until it spans at least this
/// long; the reported figure is always per one call.
const MIN_TIMED_NS: f64 = 5e6;
/// Whole-cell wall budget. A cell that cannot complete one repetition inside
/// this budget is reported as a projection instead of a measurement.
const CELL_BUDGET_S: f64 = 90.0;
/// Batch sizes of the workload-selection contract.
const BATCHES: [usize; 4] = [1, 16, 256, 4096];

/// A contract row: the shortened code the cell encodes.
struct CodeSpec {
    name: &'static str,
    /// `None` for a code built from consecutive roots, `Some` for a DVB-T2 row.
    dvb: Option<(FrameSize, CodeRate)>,
    m: usize,
    prim: u64,
    t: usize,
    n: usize,
    k: usize,
}

const CODES: &[CodeSpec] = &[
    CodeSpec {
        name: "B1",
        dvb: None,
        m: 4,
        prim: 0b1_0011,
        t: 3,
        n: 15,
        k: 5,
    },
    CodeSpec {
        name: "B2",
        dvb: None,
        m: 7,
        prim: 0b1000_0011,
        t: 10,
        n: 127,
        k: 64,
    },
    CodeSpec {
        name: "B3",
        dvb: None,
        m: 8,
        prim: 0b1_0001_1101,
        t: 4,
        n: 255,
        k: 223,
    },
    CodeSpec {
        name: "T2S",
        dvb: Some((FrameSize::Short, CodeRate::Rate1_2)),
        m: 14,
        prim: 0b100_0000_0010_1011,
        t: 12,
        n: 7200,
        k: 7032,
    },
    CodeSpec {
        name: "T2N",
        dvb: Some((FrameSize::Normal, CodeRate::Rate1_2)),
        m: 16,
        prim: 0b1_0000_0000_0010_1101,
        t: 12,
        n: 32400,
        k: 32208,
    },
];

impl CodeSpec {
    fn build(&self) -> BchCode {
        match self.dvb {
            Some((frame, rate)) => BchCode::dvb_t2(frame, rate),
            None => BchCode::new(
                self.n,
                self.k,
                self.t,
                Gf2mField::new(self.m, self.prim).with_tables(),
            ),
        }
    }
}

/// splitmix64, so a run's fixtures are a function of `SEED` alone.
fn next_rand(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn messages(k: usize, count: usize, salt: u64) -> Vec<BitVec> {
    let mut state = SEED ^ salt;
    (0..count)
        .map(|_| {
            let mut m = BitVec::zeros(k);
            for i in 0..k {
                m.set(i, next_rand(&mut state) & 1 == 1);
            }
            m
        })
        .collect()
}

fn digest_codewords(cws: &[BitVec]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for cw in cws {
        for i in 0..cw.len() {
            h ^= u64::from(cw.get(i));
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}

#[allow(clippy::too_many_arguments)]
fn emit(
    rev: &str,
    workload: &str,
    algorithm: &str,
    cs: &CodeSpec,
    batch: usize,
    trial: usize,
    ns_per_frame: f64,
    info_mbit_per_s: f64,
    digest: u64,
) {
    println!(
        "gf2,{rev},{workload},{algorithm},{},{},{},{},{batch},{trial},{ns_per_frame:.3},{info_mbit_per_s:.4},{digest:016x}",
        cs.name, cs.n, cs.k, cs.t
    );
}

/// Emits a cell the budget could not measure as a labelled projection rather
/// than dropping it, so every contract cell appears in the CSV as either a
/// measurement or an estimate. Projections carry trial `-1` and an
/// `-projected` algorithm suffix; the aggregation groups them separately and
/// never mixes them into a measured cell's statistics.
#[allow(clippy::too_many_arguments)]
fn emit_projection(
    rev: &str,
    workload: &str,
    algorithm: &str,
    cs: &CodeSpec,
    batch: usize,
    ns_per_frame: f64,
    info_mbit_per_s: f64,
) {
    println!(
        "gf2,{rev},{workload},{algorithm}-projected,{},{},{},{},{batch},-1,{ns_per_frame:.3},{info_mbit_per_s:.4},projection------",
        cs.name, cs.n, cs.k, cs.t
    );
}

/// Times `run` with enough inner repetitions to clear [`MIN_TIMED_NS`],
/// returning the per-call nanoseconds of each trial. Stops early once the
/// accumulated timed work passes [`CELL_BUDGET_S`].
fn time_cell(mut run: impl FnMut()) -> Vec<f64> {
    let t0 = Instant::now();
    run();
    let one_ns = t0.elapsed().as_secs_f64() * 1e9;
    let reps = ((MIN_TIMED_NS / one_ns.max(1.0)) as u64 + 1).max(1);

    let mut out = Vec::new();
    let mut spent = 0.0;
    for _ in 0..TRIALS {
        if spent >= CELL_BUDGET_S {
            break;
        }
        let t = Instant::now();
        for _ in 0..reps {
            run();
        }
        let total = t.elapsed().as_secs_f64();
        spent += total;
        out.push(total * 1e9 / reps as f64);
    }
    eprintln!("#   reps={reps} spent={spent:.2}s trials={}", out.len());
    out
}

fn main() {
    let rev = env::var("GF2_REV").unwrap_or_else(|_| "unknown".into());
    let only = env::var("GF2_SURVEY_CODES").unwrap_or_default();
    let selector = env::args().nth(1).unwrap_or_else(|| "all".into());
    let do_w1 = selector == "all" || selector == "w1";
    let do_w2 = selector == "all" || selector == "w2";

    eprintln!("# harness: survey-gf2-side");
    eprintln!("# gf2_revision: {rev}");
    eprintln!("# trials_per_cell: {TRIALS}");
    eprintln!("# warmup_reps: {WARMUP_REPS}");
    eprintln!("# cell_budget_s: {CELL_BUDGET_S}");
    eprintln!("# seed: {SEED:#018x}");
    eprintln!(
        "# rayon_num_threads: {:?}",
        env::var("RAYON_NUM_THREADS").ok()
    );
    if !only.is_empty() {
        eprintln!("# codes_selected: {only}");
    }

    println!(
        "lib,version,workload,algorithm,code,n,k,t,batch,trial,ns_per_frame,info_mbit_per_s,digest"
    );

    for cs in CODES {
        if !only.is_empty() && !only.split(',').any(|s| s == cs.name) {
            continue;
        }
        let code = cs.build();
        assert_eq!(code.n(), cs.n, "{} n mismatch", cs.name);
        assert_eq!(code.k(), cs.k, "{} k mismatch", cs.name);
        eprintln!(
            "# code {}: n={} k={} t={}",
            cs.name,
            code.n(),
            code.k(),
            code.t()
        );

        let encoder = BchEncoder::new(code.clone());

        // One encode fixes the per-frame cost, which decides how much of the
        // contract's batch ladder fits the budget.
        let probe_msgs = messages(cs.k, 1, 0);
        let t = Instant::now();
        let probe_cw = encoder.encode(&probe_msgs[0]);
        let per_frame_s = t.elapsed().as_secs_f64();
        assert_eq!(probe_cw.len(), cs.n);
        eprintln!(
            "#   {} probe: {:.6} s per single encode",
            cs.name, per_frame_s
        );

        if do_w1 {
            for &batch in BATCHES.iter() {
                let projected = per_frame_s * batch as f64;
                if projected > CELL_BUDGET_S {
                    eprintln!(
                        "# PROJECTED-ONLY {} batch={}: one batch costs about {:.1} s, over the {:.0} s cell budget",
                        cs.name, batch, projected, CELL_BUDGET_S
                    );
                    emit_projection(
                        &rev,
                        "W1",
                        "encode-batch",
                        cs,
                        batch,
                        per_frame_s * 1e9,
                        cs.k as f64 / per_frame_s * 1e-6,
                    );
                    continue;
                }

                let msgs = messages(cs.k, batch, batch as u64);

                for _ in 0..WARMUP_REPS.min(if projected > 1.0 { 0 } else { WARMUP_REPS }) {
                    std::hint::black_box(encoder.encode_batch(&msgs));
                }

                let digest = digest_codewords(&encoder.encode_batch(&msgs));

                eprintln!("#   {} encode-batch batch={}", cs.name, batch);
                for (trial, ns) in time_cell(|| {
                    std::hint::black_box(encoder.encode_batch(std::hint::black_box(&msgs)));
                })
                .into_iter()
                .enumerate()
                {
                    let per_frame = ns / batch as f64;
                    emit(
                        &rev,
                        "W1",
                        "encode-batch",
                        cs,
                        batch,
                        trial,
                        per_frame,
                        (batch * cs.k) as f64 / ns * 1e3,
                        digest,
                    );
                }

                eprintln!("#   {} encode-loop batch={}", cs.name, batch);
                for (trial, ns) in time_cell(|| {
                    let out: Vec<BitVec> = msgs
                        .iter()
                        .map(|m| encoder.encode(std::hint::black_box(m)))
                        .collect();
                    std::hint::black_box(out);
                })
                .into_iter()
                .enumerate()
                {
                    let per_frame = ns / batch as f64;
                    emit(
                        &rev,
                        "W1",
                        "encode-loop",
                        cs,
                        batch,
                        trial,
                        per_frame,
                        (batch * cs.k) as f64 / ns * 1e3,
                        digest,
                    );
                }
            }
        }

        if do_w2 {
            // Materializing G costs one encode per information bit.
            let projected = per_frame_s * cs.k as f64;
            eprintln!(
                "#   {} W2 projected {:.3} s per materialization",
                cs.name, projected
            );
            if projected > CELL_BUDGET_S {
                eprintln!(
                    "# PROJECTED-ONLY {} W2: about {:.1} s per materialization, over the {:.0} s cell budget",
                    cs.name, projected, CELL_BUDGET_S
                );
                emit_projection(
                    &rev,
                    "W2",
                    "generator-matrix",
                    cs,
                    cs.k,
                    per_frame_s * 1e9,
                    (cs.k * cs.n) as f64 / projected * 1e-6,
                );
            } else {
                // A fresh code per call: `generator_matrix` caches its result,
                // so a reused code would time a clone rather than a build.
                let digest = {
                    let g = cs.build().generator_matrix();
                    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                    for i in 0..cs.k {
                        for j in 0..cs.n {
                            h ^= u64::from(g.get(i, j));
                            h = h.wrapping_mul(0x1000_0000_01b3);
                        }
                    }
                    h
                };

                eprintln!("#   {} generator-matrix", cs.name);
                for (trial, ns) in time_cell(|| {
                    std::hint::black_box(cs.build().generator_matrix());
                })
                .into_iter()
                .enumerate()
                {
                    emit(
                        &rev,
                        "W2",
                        "generator-matrix",
                        cs,
                        cs.k,
                        trial,
                        ns / cs.k as f64,
                        (cs.k * cs.n) as f64 / ns * 1e3,
                        digest,
                    );
                }
            }
        }
    }
}
