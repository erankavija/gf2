//! Paired sweep of the LDPC batch routes of `@/issue/c62356d2`: per code,
//! configured worker count and frames per decode call, the QC, lane and
//! single-frame routes decode the same frames in rotating order, and each
//! completed cell is appended to the campaign's execution log, from which an
//! interrupted sweep resumes.

use crate::batch::route_selectors;
use experiments_common::Sha256Digest;
use experiments_common::{RepositoryRoot, RngAlgorithm, RngIdentity, RunIdentity, RunRecord};
use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode, QuasiCyclicLdpc,
};
use gf2_coding::llr::Llr;
use gf2_coding::traits::{BatchSoftDecoder, DecodeOutcome};
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Barrier;
use std::time::Instant;
use tuning_campaign_support::journal::{ExecutionLog, JournalEvent, TerminalState};

const CAMPAIGN_ID: &str = "c62356d2-qc-route-sweep";
const EXECUTION_LOG: &str = "execution.log";

/// The sweep's codes, workers, frames per call and timing parameters. Every
/// field enters the configuration digest of the run identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SweepConfig {
    /// `nr-bg<g>-z<Z>` or `dvb-t2-<short|normal>-r1-2`.
    pub codes: Vec<String>,
    pub workers: Vec<usize>,
    pub frames_per_call: Vec<usize>,
    /// Timed repetitions of each route in a cell; the route order rotates.
    pub repetitions: usize,
    /// Lower bound on Tanner-graph edges times frames that one worker decodes
    /// in one repetition, so that a repetition outlasts timer resolution.
    pub edge_frames_per_repetition: usize,
    /// Distinct frames per worker; a repetition cycles through them.
    pub frame_pool: usize,
    pub eb_n0_db: f64,
    pub max_iterations: usize,
    pub normalization: f32,
    pub seed: u64,
}

impl SweepConfig {
    /// The grid of the issue: both 5G NR base graphs at a lifting size below
    /// one AVX2 vector, a medium one with a remainder modulo eight, and the
    /// largest; the DVB-T2 short and normal rate-1/2 codes; 1, 6, 12 and 24
    /// workers; 1 and 8 frames per call.
    #[must_use]
    pub fn issue_grid() -> Self {
        let mut codes = Vec::new();
        for graph in [1, 2] {
            for z in [6, 52, 384] {
                codes.push(format!("nr-bg{graph}-z{z}"));
            }
        }
        codes.push("dvb-t2-short-r1-2".to_owned());
        codes.push("dvb-t2-normal-r1-2".to_owned());
        Self {
            codes,
            workers: vec![1, 6, 12, 24],
            frames_per_call: vec![1, 8],
            repetitions: 12,
            edge_frames_per_repetition: 8_000_000,
            frame_pool: 16,
            eb_n0_db: 2.0,
            max_iterations: 20,
            normalization: 0.75,
            seed: 0x5eed_c623,
        }
    }

    fn digest(&self) -> Sha256Digest {
        Sha256Digest::of(&serde_json::to_vec(self).expect("the configuration serializes"))
    }

    fn cells(&self) -> Vec<Cell> {
        let mut cells = Vec::new();
        for (code_index, code) in self.codes.iter().enumerate() {
            for &workers in &self.workers {
                for &frames in &self.frames_per_call {
                    cells.push(Cell {
                        code_index,
                        code: code.clone(),
                        workers,
                        frames,
                    });
                }
            }
        }
        cells
    }
}

/// Builds a code of [`SweepConfig::codes`] with its retained QC structure.
///
/// # Errors
///
/// Returns an error for a name outside the grammar of [`SweepConfig::codes`].
pub fn build_code(name: &str) -> Result<LdpcCode, String> {
    let code = match name {
        "dvb-t2-short-r1-2" => LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
        "dvb-t2-normal-r1-2" => LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
        _ => {
            let parsed = name.strip_prefix("nr-bg").and_then(|rest| {
                let (graph, z) = rest.split_once("-z")?;
                let (graph, z) = (graph.parse::<u8>().ok()?, z.parse::<u16>().ok()?);
                (matches!(graph, 1 | 2) && gf2_coding::ldpc::nr_5g::is_valid_lifting_size(z))
                    .then_some((graph, usize::from(z)))
            });
            let (graph, z) = parsed.ok_or_else(|| format!("unknown sweep code {name}"))?;
            LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(graph, z))
        }
    };
    if code.quasi_cyclic().is_none() {
        return Err(format!("{name} retains no QC structure"));
    }
    Ok(code)
}

#[derive(Clone, Debug)]
struct Cell {
    code_index: usize,
    code: String,
    workers: usize,
    frames: usize,
}

impl Cell {
    fn id(&self) -> String {
        format!("{}/w{}/f{}", self.code, self.workers, self.frames)
    }
}

/// The routes of a cell, in the order of the first repetition.
fn routes() -> [BatchRoute; 3] {
    let backend = LaneBackend::detected();
    [
        BatchRoute::Qc(backend),
        BatchRoute::Lane(backend),
        BatchRoute::SingleFrame,
    ]
}

/// Frames of BPSK over AWGN at `eb_n0_db` for the all-zero codeword, as
/// channel LLRs; min-sum decoding is symmetric in the transmitted codeword.
fn channel_frames(code: &LdpcCode, config: &SweepConfig, stream: u64) -> Vec<Vec<Llr>> {
    let (n, m) = (code.n(), code.m());
    let rate = (n - m) as f64 / n as f64;
    let es_n0 = 10f64.powf(config.eb_n0_db / 10.0) * rate;
    let sigma = (1.0 / (2.0 * es_n0)).sqrt();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    rng.set_stream(stream);
    let mut uniform = || ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    (0..config.frame_pool)
        .map(|_| {
            (0..n)
                .map(|_| {
                    let (u, v) = (uniform(), uniform());
                    let noise = (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos();
                    Llr::new((2.0 * (1.0 + sigma * noise) / (sigma * sigma)) as f32)
                })
                .collect()
        })
        .collect()
}

/// One worker's decoders, frames and output buffers.
struct Worker {
    decoders: Vec<LdpcBatchDecoder>,
    pool: Vec<Vec<Llr>>,
    decoded: Vec<BitVec>,
    outcomes: Vec<DecodeOutcome>,
    iterations: Vec<usize>,
    /// Words of each route's first call.
    first_words: Vec<Vec<BitVec>>,
}

impl Worker {
    /// Decodes `calls` calls of `frames` pool frames on route `route`.
    fn run(&mut self, route: usize, calls: usize, frames: usize, workers: NonZeroUsize) {
        let pool = self.pool.len();
        for call in 0..calls {
            let views: Vec<&[Llr]> = (0..frames)
                .map(|f| self.pool[(call * frames + f) % pool].as_slice())
                .collect();
            self.decoders[route].decode_batch_with_workers(
                &views,
                &mut self.decoded[..frames],
                &mut self.outcomes[..frames],
                workers,
            );
            self.iterations[route] += self.outcomes[..frames]
                .iter()
                .map(|outcome| outcome.iterations)
                .sum::<usize>();
            if call == 0 && self.first_words[route].is_empty() {
                self.first_words[route] = self.decoded[..frames].to_vec();
            }
        }
    }
}

/// Timings of one cell: per route, the nanoseconds of each repetition.
#[derive(Debug, Serialize)]
struct CellResult {
    cell: String,
    code: String,
    lifting_size: usize,
    edges: usize,
    workers: usize,
    frames_per_call: usize,
    frames_per_repetition: usize,
    routes: Vec<String>,
    /// `elapsed_ns[r][i]`: route `r` in repetition `i`.
    elapsed_ns: Vec<Vec<u64>>,
    /// Decoder iterations of each route over its untimed and timed calls.
    iterations: Vec<usize>,
    /// Median over repetitions of each route's time over the QC route's
    /// time in the same repetition.
    median_time_ratio_to_qc: Vec<f64>,
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    }
}

fn measure(config: &SweepConfig, cell: &Cell, code: &LdpcCode) -> Result<CellResult, String> {
    let edges = EdgeLayout::from_parity_check(code.parity_check_matrix()).edges();
    let frames_per_repetition = config
        .edge_frames_per_repetition
        .div_ceil(edges)
        .div_ceil(cell.frames)
        .max(1)
        * cell.frames;
    let calls = frames_per_repetition / cell.frames;
    let workers = NonZeroUsize::new(cell.workers).ok_or("a cell has at least one worker")?;
    let decoder_config = DecoderConfig::new(
        DecoderAlgorithm::NormalizedMinSum(config.normalization),
        true,
    );
    let routes = routes();
    let mut pool_workers = Vec::with_capacity(cell.workers);
    for worker in 0..cell.workers {
        let mut decoders = Vec::new();
        for route in routes {
            let decoder = LdpcBatchDecoder::with_selectors(
                code.clone(),
                decoder_config,
                config.max_iterations,
                DecodedWord::Codeword,
                route_selectors(route),
            );
            let taken = decoder.batch_route_with_workers(cell.frames, workers);
            if taken != route {
                return Err(format!("{}: {route:?} takes {taken:?}", cell.id()));
            }
            decoders.push(decoder);
        }
        let stream = (cell.code_index * 64 + worker) as u64;
        pool_workers.push(Worker {
            decoders,
            pool: channel_frames(code, config, stream),
            decoded: vec![BitVec::new(); cell.frames],
            outcomes: vec![DecodeOutcome::default(); cell.frames],
            iterations: vec![0; routes.len()],
            first_words: vec![Vec::new(); routes.len()],
        });
    }

    let barrier = Barrier::new(cell.workers + 1);
    let mut elapsed_ns = vec![Vec::with_capacity(config.repetitions); routes.len()];
    // Repetition zero is untimed warm-up.
    let schedule: Vec<(usize, usize)> = (0..=config.repetitions)
        .flat_map(|repetition| {
            (0..routes.len()).map(move |slot| (repetition, (repetition + slot) % routes.len()))
        })
        .collect();
    std::thread::scope(|scope| {
        for worker in &mut pool_workers {
            let (barrier, schedule) = (&barrier, &schedule);
            scope.spawn(move || {
                for &(_, route) in schedule {
                    barrier.wait();
                    worker.run(route, calls, cell.frames, workers);
                    barrier.wait();
                }
            });
        }
        for &(repetition, route) in &schedule {
            barrier.wait();
            let start = Instant::now();
            barrier.wait();
            let ns = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            if repetition > 0 {
                elapsed_ns[route].push(ns);
            }
        }
    });

    for (worker, state) in pool_workers.iter().enumerate() {
        for route in 1..routes.len() {
            if state.first_words[route] != state.first_words[0] {
                return Err(format!(
                    "{}: worker {worker} decodes other words on {:?} than on {:?}",
                    cell.id(),
                    routes[route],
                    routes[0]
                ));
            }
        }
    }
    let iterations = (0..routes.len())
        .map(|route| pool_workers.iter().map(|w| w.iterations[route]).sum())
        .collect();
    let median_time_ratio_to_qc = (0..routes.len())
        .map(|route| {
            median(
                elapsed_ns[route]
                    .iter()
                    .zip(&elapsed_ns[0])
                    .map(|(&t, &qc)| t as f64 / qc as f64)
                    .collect(),
            )
        })
        .collect();
    Ok(CellResult {
        cell: cell.id(),
        code: cell.code.clone(),
        lifting_size: code
            .quasi_cyclic()
            .map_or(0, QuasiCyclicLdpc::expansion_factor),
        edges,
        workers: cell.workers,
        frames_per_call: cell.frames,
        frames_per_repetition,
        routes: routes.iter().map(|route| format!("{route:?}")).collect(),
        elapsed_ns,
        iterations,
        median_time_ratio_to_qc,
    })
}

/// Runs or resumes the sweep of `config` with its execution log in
/// `out_dir`, skipping cells the log records as complete.
///
/// # Errors
///
/// Refuses a log whose run identity or configuration differs from this run's,
/// and returns I/O, route and word-agreement failures; a failed cell closes
/// the campaign as failed.
pub fn run(config: &SweepConfig, out_dir: &Path, root: &RepositoryRoot) -> Result<(), String> {
    let error = |e: io::Error| e.to_string();
    let record = RunRecord::observe(
        root,
        config.digest(),
        config.seed,
        RngIdentity {
            algorithm: RngAlgorithm::ChaCha20,
            version: "rand_chacha 0.9.0".to_owned(),
        },
    )
    .map_err(error)?;
    let session = format!(
        "session-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let log_path = out_dir.join(EXECUTION_LOG);
    let (mut log, completed) = if log_path.exists() {
        let records =
            ExecutionLog::validate_prefix(&fs::read(&log_path).map_err(error)?, CAMPAIGN_ID)
                .map_err(error)?;
        let start = records
            .iter()
            .find(|record| record.event == JournalEvent::CampaignStart)
            .ok_or("the execution log has no campaign start")?;
        let logged: RunIdentity = serde_json::from_value(start.details["run"]["identity"].clone())
            .map_err(|e| e.to_string())?;
        if logged != record.identity || start.details["configuration"] != json!(config) {
            return Err(
                "the execution log belongs to another run identity or configuration".into(),
            );
        }
        let completed: BTreeSet<String> = records
            .iter()
            .filter(|record| record.event == JournalEvent::CellComplete)
            .filter_map(|record| record.case.as_ref()?.as_str().map(str::to_owned))
            .collect();
        let log = ExecutionLog::resume(&log_path, CAMPAIGN_ID, &session).map_err(error)?;
        (log, completed)
    } else {
        fs::create_dir_all(out_dir).map_err(error)?;
        let mut log = ExecutionLog::create_new(out_dir, CAMPAIGN_ID, &session).map_err(error)?;
        log.append(
            JournalEvent::CampaignStart,
            None,
            json!({ "run": record, "configuration": config }),
        )
        .map_err(error)?;
        (log, BTreeSet::new())
    };

    let mut codes: Vec<Option<LdpcCode>> = vec![None; config.codes.len()];
    for cell in config.cells() {
        let id = cell.id();
        if completed.contains(&id) {
            continue;
        }
        if codes[cell.code_index].is_none() {
            codes[cell.code_index] = Some(build_code(&cell.code)?);
        }
        let code = codes[cell.code_index].as_ref().expect("built above");
        log.append(JournalEvent::CellStart, Some(json!(id)), json!({}))
            .map_err(error)?;
        match measure(config, &cell, code) {
            Ok(result) => {
                eprintln!("{id}: time over QC {:?}", result.median_time_ratio_to_qc);
                log.append(
                    JournalEvent::CellComplete,
                    Some(json!(id)),
                    serde_json::to_value(&result).map_err(|e| e.to_string())?,
                )
                .map_err(error)?;
            }
            Err(failure) => {
                log.terminal(
                    TerminalState::Failed,
                    json!({ "cell": id, "error": failure }),
                )
                .map_err(error)?;
                return Err(failure);
            }
        }
    }
    log.terminal(TerminalState::Complete, Value::Null)
        .map_err(error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> SweepConfig {
        SweepConfig {
            codes: vec!["nr-bg2-z6".to_owned(), "nr-bg1-z13".to_owned()],
            workers: vec![1, 2],
            frames_per_call: vec![1, 3],
            repetitions: 2,
            edge_frames_per_repetition: 1,
            frame_pool: 4,
            ..SweepConfig::issue_grid()
        }
    }

    #[test]
    fn every_issue_grid_code_retains_qc_structure() {
        for name in SweepConfig::issue_grid().codes {
            build_code(&name).unwrap();
        }
        for name in ["nr-bg3-z6", "nr-bg1-z0", "nr-bg1-z17", "hamming"] {
            assert!(build_code(name).is_err(), "{name}");
        }
    }

    #[test]
    fn each_route_of_a_cell_decodes_the_same_words() {
        let config = tiny();
        for cell in config.cells() {
            let code = build_code(&cell.code).unwrap();
            let result = measure(&config, &cell, &code).unwrap();
            assert_eq!(result.elapsed_ns.iter().map(Vec::len).max(), Some(2));
            assert!(result.iterations.windows(2).all(|pair| pair[0] == pair[1]));
            assert_eq!(result.median_time_ratio_to_qc[0], 1.0);
        }
    }

    #[test]
    fn a_resumed_sweep_skips_completed_cells_and_refuses_another_configuration() {
        let dir = std::env::temp_dir().join(format!("qc-route-sweep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = RepositoryRoot::containing(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut config = tiny();
        config.codes.truncate(1);
        let log_path = dir.join(EXECUTION_LOG);
        let completed = |log: &[u8]| -> Vec<String> {
            ExecutionLog::validate_prefix(log, CAMPAIGN_ID)
                .unwrap()
                .iter()
                .filter(|record| record.event == JournalEvent::CellComplete)
                .map(|record| record.case.clone().unwrap().as_str().unwrap().to_owned())
                .collect()
        };
        run(&config, &dir, &root).unwrap();
        let finished = fs::read(&log_path).unwrap();
        let all: Vec<String> = config.cells().iter().map(Cell::id).collect();
        assert_eq!(completed(&finished), all);
        // A complete campaign does not resume.
        assert!(run(&config, &dir, &root).is_err());

        // A session interrupted after its first cell resumes at the second.
        let text = String::from_utf8(finished).unwrap();
        let first = text
            .lines()
            .position(|line| line.contains("\"cell-complete\""))
            .unwrap();
        let prefix: String = text
            .lines()
            .take(first + 1)
            .map(|line| format!("{line}\n"))
            .collect();
        fs::write(&log_path, prefix).unwrap();
        let mut other = config.clone();
        other.repetitions += 1;
        assert!(run(&other, &dir, &root)
            .unwrap_err()
            .contains("another run identity or configuration"));
        run(&config, &dir, &root).unwrap();
        assert_eq!(completed(&fs::read(&log_path).unwrap()), all);
        let _ = fs::remove_dir_all(&dir);
    }
}
