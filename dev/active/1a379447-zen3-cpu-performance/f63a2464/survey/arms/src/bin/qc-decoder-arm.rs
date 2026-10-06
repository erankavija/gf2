//! QC-aware intra-frame LDPC throughput arm (jit:f63a2464).
//!
//! The steady-state operation of `3be770d5`, unchanged, with the candidate
//! decoder in place of the canonical one: one pinned worker per resolved CPU,
//! each owning a decoder built before timing, and a timed call decoding the
//! declared per-worker batch of recorded frames including the conversion of the
//! recorded f32 LLRs, the dispatch and the extraction of the information-window
//! decisions.
//!
//! The candidate declares the canonical numerical contract, so it reports the
//! same settings as the canonical arm and the shared driver checks every
//! worker's per-frame bit errors against the same prepared quality evidence.
//! A candidate that parted from the contract would fail that check here.

use gf2_coding::ldpc::{MinSumRule, QuasiCyclicLdpc};
use gf2_coding::llr::Llr;
use ldpc_candidate_prototypes::QcDecoder;
use ldpc_survey::arm::{
    self, ArmResult, DecoderCase, Normalization, NormalizationKind, Precision, Schedule, Stopping,
    StoppingKind, ARM_RESULT_SCHEMA,
};
use ldpc_throughput::driver::run_cell;
use ldpc_throughput::workload::{check_request, median_u64, Workload};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

/// Untimed repetitions of the per-batch conversion probe.
const PACK_PROBES: usize = 9;

/// The lifted base graph of the measured NR workload.
const BASE_GRAPH: u8 = 1;
const LIFTING: usize = 384;

/// One worker's reused candidate decoder and buffers.
struct QcWorker {
    decoder: QcDecoder,
    packed: Vec<Llr>,
    n: usize,
    k: usize,
    cap: usize,
    decisions: Vec<u8>,
    iterations: Vec<u32>,
}

impl QcWorker {
    fn new(
        code: &QuasiCyclicLdpc,
        rule: MinSumRule,
        n: usize,
        k: usize,
        batch: usize,
        cap: usize,
    ) -> Self {
        Self {
            decoder: QcDecoder::new(code, rule),
            packed: vec![Llr::zero(); n],
            n,
            k,
            cap,
            decisions: vec![0; batch * k],
            iterations: vec![0; batch],
        }
    }

    fn decode_batch(&mut self, llrs: &[f32]) {
        for (frame, recorded) in llrs.chunks_exact(self.n).enumerate() {
            for (packed, value) in self.packed.iter_mut().zip(recorded) {
                *packed = Llr::new(*value);
            }
            let outcome = self.decoder.decode(black_box(&self.packed), self.cap, true);
            let decided = &mut self.decisions[frame * self.k..(frame + 1) * self.k];
            for (position, bit) in decided.iter_mut().enumerate() {
                *bit = u8::from(self.decoder.hard_bits()[position]);
            }
            self.iterations[frame] = outcome.iterations as u32;
        }
        black_box(&self.decisions);
    }
}

fn run() -> Result<(), String> {
    let request = arm::read_request()?;
    let quality = arm::prepared_quality()?
        .ok_or("steady-state cells reuse prepared quality; GF2_LDPC_QUALITY is unset")?;
    let case: DecoderCase = serde_json::from_value(request.case.clone())
        .map_err(|error| format!("case does not decode: {error}"))?;
    let workload = Workload::load(&case)?;
    check_request(&request, &case, workload.manifest.frames)?;

    let settings = arm::ArmSettings {
        precision: Precision::F32,
        schedule: Schedule::Flooding,
        normalization: Normalization {
            kind: NormalizationKind::NormalizedMinSum,
            factor: Some(f64::from(case.normalization_factor)),
        },
        iteration_cap: case.iteration_cap,
        stopping: Stopping {
            kind: if case.syndrome_stopping {
                StoppingKind::Syndrome
            } else {
                StoppingKind::Fixed
            },
            crc: None,
        },
        batch_size: 1,
    };
    if quality.settings != settings {
        return Err("the prepared quality was produced under other settings".to_owned());
    }
    if !case.syndrome_stopping {
        return Err("this arm decodes under the syndrome stopping contract".to_owned());
    }
    let batch = case.batch_size as usize;
    let llrs = workload.frames(0, batch);

    let mut packed = vec![Llr::zero(); llrs.len()];
    let pack_ns = median_u64(
        (0..PACK_PROBES)
            .map(|_| {
                let start = Instant::now();
                for (slot, value) in packed.iter_mut().zip(black_box(llrs)) {
                    *slot = Llr::new(*value);
                }
                black_box(&packed);
                start.elapsed().as_nanos() as u64
            })
            .collect(),
    );
    drop(packed);

    let setup_start = Instant::now();
    let qc = QuasiCyclicLdpc::nr_5g(BASE_GRAPH, LIFTING);
    let lifted = gf2_coding::ldpc::LdpcCode::from_quasi_cyclic(&qc);
    let recorded =
        ldpc_survey::read_alist_code(&workload.alist()).map_err(|error| error.to_string())?;
    if lifted.n() != workload.manifest.n
        || lifted.k() != workload.manifest.k
        || lifted.parity_check_matrix().nnz() != workload.manifest.nnz
        || recorded.parity_check_matrix().nnz() != lifted.parity_check_matrix().nnz()
    {
        return Err("the lifted base graph is not the recorded parity-check matrix".to_owned());
    }
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let rule = MinSumRule::Normalized(case.normalization_factor);
    let cap = case.iteration_cap as usize;
    let cell = run_cell(
        "qc",
        &request,
        &workload,
        &quality,
        setup_start,
        pack_ns,
        |_| Ok(QcWorker::new(&qc, rule, n, k, batch, cap)),
        |worker: &mut QcWorker| worker.decode_batch(llrs),
        |worker: &QcWorker| &worker.decisions,
    )?;
    let avx2 = QcDecoder::new(&qc, rule).uses_avx2();
    let result = ArmResult {
        schema: ARM_RESULT_SCHEMA.to_owned(),
        windows: cell.windows,
        calibrated: true,
        cache_state_applied: request.cache_state.clone(),
        workers_observed: cell.workers_observed,
        cpus_observed: arm::observed_cpus(),
        selected_path: Some(format!(
            "QC-aware intra-frame f32 flooding normalized min-sum over lifted blocks of \
             NR BG{BASE_GRAPH} lifting {LIFTING}, check kernel {}; reused decoder per worker; \
             {} pinned workers, {} process threads",
            if avx2 { "avx2" } else { "scalar" },
            cell.workers_observed,
            cell.threads.ready
        )),
        conversion: Some(cell.conversion),
        quality: Some(quality),
    };
    arm::write_result(&result)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qc-decoder-arm: {error}");
            ExitCode::FAILURE
        }
    }
}
