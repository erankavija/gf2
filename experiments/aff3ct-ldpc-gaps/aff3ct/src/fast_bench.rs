//! Throughput of one AFF3CT NMS decoder configuration over a recorded bundle.
//!
//! `aff3ct-fast-bench BUNDLE CODE KIND SIMD PRECISION WORKERS BATCH CALLS
//! PINNING` prints one JSON line through [`aff3ct_ldpc_gaps::fast`]. `SIMD` is `none`
//! or `INTER`. The quality pass decodes the bundle's whole native waves untimed.

use aff3ct_ldpc_arm::{Aff3ctWorker, Handle, Selection};
use aff3ct_ldpc_gaps::fast::{self, FastCell, ITERATION_CAP, NORMALIZATION};
use std::process::ExitCode;
use std::sync::Mutex;

fn run(args: &[String]) -> Result<(), String> {
    let cell = FastCell::parse(args)?;
    let workload = cell.load()?;
    let (n, k) = (workload.manifest.n, workload.manifest.k);
    let simd = if cell.simd == "none" { "" } else { cell.simd.as_str() };
    let selection = Selection::fast(&cell.kind, simd, &cell.precision);
    let prototype = Handle::build(
        &workload.alist(),
        k,
        n,
        ITERATION_CAP,
        NORMALIZATION,
        true,
        &selection,
    )?;
    let wave = prototype.wave();
    let name = prototype.name();

    let quality_frames = workload.manifest.frames / wave * wave;
    let mut quality = Aff3ctWorker::new(&prototype, quality_frames, k)?;
    quality.decode_batch(workload.frames(0, quality_frames));
    let frame_errors = fast::erroneous_frames(&workload, &quality.decisions);

    let llrs = workload.frames(0, cell.batch);
    let prototype = Mutex::new(prototype);
    let median_call_ns = cell.median_call_ns(
        |_| {
            let prototype = prototype.lock().map_err(|e| e.to_string())?;
            Aff3ctWorker::new(&prototype, cell.batch, k)
        },
        |worker| worker.decode_batch(llrs),
    )?;
    cell.report(
        &name,
        ITERATION_CAP as usize,
        k,
        median_call_ns,
        quality_frames,
        frame_errors,
    );
    Ok(())
}

fn main() -> ExitCode {
    fast::main_with("aff3ct-fast-bench", run)
}
