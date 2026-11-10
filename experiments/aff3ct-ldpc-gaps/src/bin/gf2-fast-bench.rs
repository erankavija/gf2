//! Throughput of one gf2 NMS decoder route over a recorded bundle.
//!
//! `gf2-fast-bench BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS` prints
//! one JSON line through [`aff3ct_ldpc_gaps::fast`]. `KIND` is `single`
//! (single-frame route), `lanes` (inter-frame lane route), `qc` (`QcDecoder`,
//! one frame at a time) or `default` (`LdpcBatchDecoder` as shipped). `SIMD` is
//! `none` or `avx2` for `lanes` and `qc`, `none` for `single` and `auto` for
//! `default`; `PRECISION` is `f32`. The code retains its block description
//! after an exact parity-check comparison. The quality pass decodes the
//! bundle's whole batches untimed.

use aff3ct_ldpc_gaps::batch::{
    config, structured_corpus_code, ArmPolicy, BatchRoute, BatchWorker, LaneBackend, QcWorker,
};
use aff3ct_ldpc_gaps::fast::{self, FastCell, ITERATION_CAP, NORMALIZATION};
use gf2_coding::ldpc::{DecodedWord, LdpcBatchDecoder, LdpcCode};
use std::num::NonZeroUsize;
use std::process::ExitCode;

enum Worker {
    Batch(Box<BatchWorker>),
    Qc(Box<QcWorker>),
}

impl Worker {
    fn decode(&mut self, llrs: &[f32], workers: NonZeroUsize) {
        match self {
            Self::Batch(worker) => worker.decode_batch_with_workers(llrs, workers),
            Self::Qc(worker) => worker.decode_batch(llrs),
        }
    }

    fn decisions(&self) -> &[u8] {
        match self {
            Self::Batch(worker) => &worker.decisions,
            Self::Qc(worker) => &worker.decisions,
        }
    }
}

fn build(cell: &FastCell, code: &LdpcCode, batch: usize) -> Result<Worker, String> {
    let decoder_config = config(NORMALIZATION, true);
    let cap = ITERATION_CAP as usize;
    let pinned = |route: BatchRoute| -> Result<Worker, String> {
        let decoder = route.build_decoder(code, decoder_config, cap, batch)?;
        Ok(Worker::Batch(Box::new(BatchWorker::from_decoder(
            code, batch, decoder,
        )?)))
    };
    let backend = match cell.simd.as_str() {
        "none" => LaneBackend::Portable,
        "avx2" => LaneBackend::Avx2,
        "auto" => LaneBackend::detected(),
        other => return Err(format!("SIMD is none, avx2 or auto, not {other}")),
    };
    match (cell.kind.as_str(), cell.simd.as_str()) {
        ("single", "none") => pinned(BatchRoute::SingleFrame),
        ("lanes", "none" | "avx2") => pinned(BatchRoute::Lane(backend)),
        ("qc", "none" | "avx2") => {
            let worker = QcWorker::new(code, decoder_config, cap, batch, cell.simd == "none")?;
            if worker.backend() != backend {
                return Err(format!(
                    "the QC decoder runs {:?}, not {backend:?}",
                    worker.backend()
                ));
            }
            Ok(Worker::Qc(Box::new(worker)))
        }
        ("default", "auto") => {
            let decoder =
                LdpcBatchDecoder::new(code.clone(), decoder_config, cap, DecodedWord::Codeword);
            Ok(Worker::Batch(Box::new(BatchWorker::from_decoder(
                code, batch, decoder,
            )?)))
        }
        (kind, simd) => Err(format!("no gf2 route {kind} with SIMD {simd}")),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cell = FastCell::parse(args)?;
    if cell.precision != "f32" {
        return Err(format!("gf2 decodes f32, not {}", cell.precision));
    }
    let workload = cell.load()?;
    let code = structured_corpus_code(&workload)?;
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let workers = NonZeroUsize::new(cell.workers).expect("a positive worker count");

    let quality_frames = workload.manifest.frames / cell.batch * cell.batch;
    let mut quality = build(&cell, &code, cell.batch)?;
    let mut decisions = Vec::with_capacity(quality_frames * k);
    for first in (0..quality_frames).step_by(cell.batch) {
        quality.decode(workload.frames(first, cell.batch), workers);
        decisions.extend_from_slice(quality.decisions());
    }
    let frame_errors = fast::erroneous_frames(&workload, &decisions);
    drop(quality);

    let llrs = workload.frames(0, cell.batch);
    debug_assert_eq!(llrs.len(), cell.batch * n);
    let median_call_ns = cell.median_call_ns(
        |_| build(&cell, &code, cell.batch),
        |worker| worker.decode(llrs, workers),
    )?;
    let decoder = format!("gf2-{}-{}", cell.kind, cell.simd);
    cell.report(&decoder, k, median_call_ns, quality_frames, frame_errors);
    Ok(())
}

fn main() -> ExitCode {
    fast::main_with("gf2-fast-bench", run)
}
