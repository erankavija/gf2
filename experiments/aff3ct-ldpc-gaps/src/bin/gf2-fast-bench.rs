//! Throughput of one gf2 NMS decoder route over a recorded bundle.
//!
//! `gf2-fast-bench BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS` prints
//! one JSON line through [`aff3ct_ldpc_gaps::fast`]. `KIND` and `SIMD` name a
//! [`Route`]; `PRECISION` is `f32`, the precision of the input LLRs. The code retains its corpus block
//! description after an exact parity-check comparison. The quality pass
//! decodes the bundle's whole batches untimed.

use aff3ct_ldpc_gaps::batch::{
    config, layered_worker, qc_worker, structured_corpus_code, ArmPolicy, BatchRoute, BatchWorker,
    FrameDecoder, FrameWorker, LaneBackend, LAYERED_ITERATION_CAP,
};
use aff3ct_ldpc_gaps::fast::{self, FastCell, ITERATION_CAP, NORMALIZATION};
use gf2_coding::ldpc::{DecodedWord, LdpcBatchDecoder, LdpcCode};
use std::num::NonZeroUsize;
use std::process::ExitCode;

/// A gf2 decoder route, as `KIND SIMD` names it.
#[derive(Clone, Copy, Debug)]
enum Route {
    /// `single none`: the batch decoder pinned to the single-frame route.
    Single,
    /// `lanes none|avx2`: the batch decoder pinned to the inter-frame lanes.
    Lanes(LaneBackend),
    /// `qc none|avx2`: `QcDecoder`, one frame at a time.
    Qc(LaneBackend),
    /// `default auto`: `LdpcBatchDecoder` of default construction.
    Default,
    /// `layered none|avx2`: `LayeredDecoder` in i8 with the corpus code's
    /// layered configuration, one frame at a time.
    Layered(LaneBackend),
}

impl Route {
    fn parse(kind: &str, simd: &str) -> Result<Self, String> {
        let backend = || match simd {
            "none" => Ok(LaneBackend::Portable),
            "avx2" => Ok(LaneBackend::Avx2),
            other => Err(format!("SIMD for {kind} is none or avx2, not {other}")),
        };
        match (kind, simd) {
            ("single", "none") => Ok(Self::Single),
            ("lanes", _) => Ok(Self::Lanes(backend()?)),
            ("qc", _) => Ok(Self::Qc(backend()?)),
            ("default", "auto") => Ok(Self::Default),
            ("layered", _) => Ok(Self::Layered(backend()?)),
            _ => Err(format!("no gf2 route {kind} with SIMD {simd}")),
        }
    }

    fn iteration_cap(self) -> usize {
        match self {
            Self::Layered(_) => LAYERED_ITERATION_CAP,
            _ => ITERATION_CAP as usize,
        }
    }

    fn build(self, code: &LdpcCode, corpus: &str, batch: usize) -> Result<Box<dyn Worker>, String> {
        let decoder_config = config(NORMALIZATION, true);
        let cap = self.iteration_cap();
        let batch_worker = |decoder| -> Result<Box<dyn Worker>, String> {
            Ok(Box::new(BatchWorker::from_decoder(code, batch, decoder)?))
        };
        match self {
            Self::Single => batch_worker(BatchRoute::SingleFrame.build_decoder(
                code,
                decoder_config,
                cap,
                batch,
            )?),
            Self::Lanes(backend) => batch_worker(BatchRoute::Lane(backend).build_decoder(
                code,
                decoder_config,
                cap,
                batch,
            )?),
            Self::Qc(backend) => {
                let worker = qc_worker(
                    code,
                    decoder_config,
                    cap,
                    batch,
                    backend == LaneBackend::Portable,
                )?;
                let running = worker.decoder().backend();
                if running != backend {
                    return Err(format!("the QC decoder runs {running:?}, not {backend:?}"));
                }
                Ok(Box::new(worker))
            }
            Self::Default => batch_worker(LdpcBatchDecoder::new(
                code.clone(),
                decoder_config,
                cap,
                DecodedWord::Codeword,
            )),
            Self::Layered(backend) => {
                let worker = layered_worker(code, corpus, batch, backend == LaneBackend::Portable)?;
                let running = worker.decoder().backend();
                if running != backend {
                    return Err(format!(
                        "the layered decoder runs {running:?}, not {backend:?}"
                    ));
                }
                Ok(Box::new(worker))
            }
        }
    }
}

/// One worker's decoder and its last batch's information-window decisions.
trait Worker {
    fn decode(&mut self, llrs: &[f32], workers: NonZeroUsize);
    fn decisions(&self) -> &[u8];
}

impl Worker for BatchWorker {
    fn decode(&mut self, llrs: &[f32], workers: NonZeroUsize) {
        self.decode_batch_with_workers(llrs, workers);
    }
    fn decisions(&self) -> &[u8] {
        &self.decisions
    }
}

impl<D: FrameDecoder> Worker for FrameWorker<D> {
    fn decode(&mut self, llrs: &[f32], _: NonZeroUsize) {
        self.decode_batch(llrs);
    }
    fn decisions(&self) -> &[u8] {
        &self.decisions
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let cell = FastCell::parse(args)?;
    let route = Route::parse(&cell.kind, &cell.simd)?;
    if cell.precision != "f32" {
        return Err(format!("gf2 decodes f32, not {}", cell.precision));
    }
    let workload = cell.load()?;
    let code = structured_corpus_code(&workload)?;
    let k = workload.manifest.k;
    let workers = NonZeroUsize::new(cell.workers).expect("a positive worker count");

    let quality_frames = workload.manifest.frames / cell.batch * cell.batch;
    let mut quality = route.build(&code, &cell.code, cell.batch)?;
    let mut decisions = Vec::with_capacity(quality_frames * k);
    for first in (0..quality_frames).step_by(cell.batch) {
        quality.decode(workload.frames(first, cell.batch), workers);
        decisions.extend_from_slice(quality.decisions());
    }
    let frame_errors = fast::erroneous_frames(&workload, &decisions);
    drop(quality);

    let llrs = workload.frames(0, cell.batch);
    let median_call_ns = cell.median_call_ns(
        |_| route.build(&code, &cell.code, cell.batch),
        |worker| worker.decode(llrs, workers),
    )?;
    let decoder = format!("gf2-{}-{}", cell.kind, cell.simd);
    cell.report(
        &decoder,
        route.iteration_cap(),
        k,
        median_call_ns,
        quality_frames,
        frame_errors,
    );
    Ok(())
}

fn main() -> ExitCode {
    fast::main_with("gf2-fast-bench", run)
}
