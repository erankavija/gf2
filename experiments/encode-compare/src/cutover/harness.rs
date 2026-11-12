//! One timing round over the cells of the BCH cutover comparison
//! (@/issue/fd9d5416), shared by every arm: the gf2 revisions, built as an
//! example inside each revision's own tree, and the external libraries.
//!
//! The file uses `std` alone, so the same source compiles against any gf2
//! revision. An arm turns a cell into timed configurations; a round builds
//! each configuration, encodes once outside the timed region to digest its
//! output, then repeats the call for at least [`MIN_REP_SECONDS`] and prints
//! one line per configuration. `cutover.sh` interleaves the rounds of all
//! arms and compares them.
//!
//! Codewords and generator matrices are digested in gf2's descending
//! `[message | parity]` layout, the legacy encoder's only layout (bit 0 holds
//! the highest-degree coefficient), so every arm of a cell must produce the
//! same digest.

use std::collections::HashMap;
use std::time::Instant;

/// Least wall time of one timed repetition.
pub const MIN_REP_SECONDS: f64 = 0.2;

/// Default for `CUTOVER_CALL_BUDGET_S`: a configuration whose projected
/// single call exceeds it is reported as a projection and not run.
pub const DEFAULT_CALL_BUDGET_SECONDS: f64 = 10.0;

/// Seed of the cells' messages.
pub const SEED: u64 = 0xAE03_BCD0;

/// A DVB-T2 frame size; the rows that carry one are the rate-1/2 outer codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Short,
    Normal,
}

/// One code of the epic's workload contract: a binary narrow-sense BCH code
/// over $\mathrm{GF}(2^m)$ with the given primitive polynomial.
#[derive(Debug)]
pub struct Row {
    pub name: &'static str,
    pub m: usize,
    pub prim: u64,
    pub t: usize,
    pub n: usize,
    pub k: usize,
    /// `Some` for the shortened DVB-T2 codes, which every arm builds through
    /// its DVB-T2 constructor.
    pub dvb: Option<Frame>,
}

/// The contract's rows: three primitive conformance codes and the two DVB-T2
/// rate-1/2 outer codes at their shortened lengths.
pub const ROWS: [Row; 5] = [
    Row {
        name: "B1",
        m: 4,
        prim: 0b1_0011,
        t: 3,
        n: 15,
        k: 5,
        dvb: None,
    },
    Row {
        name: "B2",
        m: 7,
        prim: 0b1000_0011,
        t: 10,
        n: 127,
        k: 64,
        dvb: None,
    },
    Row {
        name: "B3",
        m: 8,
        prim: 0b1_0001_1101,
        t: 4,
        n: 255,
        k: 223,
        dvb: None,
    },
    Row {
        name: "T2S",
        m: 14,
        prim: 0b100_0000_0010_1011,
        t: 12,
        n: 7200,
        k: 7032,
        dvb: Some(Frame::Short),
    },
    Row {
        name: "T2N",
        m: 16,
        prim: 0b1_0000_0000_0010_1101,
        t: 12,
        n: 32400,
        k: 32208,
        dvb: Some(Frame::Normal),
    },
];

/// The contract's batch sizes of workload W1.
pub const BATCHES: [usize; 4] = [1, 16, 256, 4096];

/// W1 encodes a batch of messages; W2 materializes the $k \times n$
/// systematic generator matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workload {
    W1,
    W2,
}

/// One measured cell.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub workload: Workload,
    pub row: &'static Row,
    /// Messages per call for W1; one for W2.
    pub batch: usize,
}

impl Cell {
    /// Bits of work per call: information bits for W1, matrix bits for W2.
    pub fn work(&self) -> f64 {
        match self.workload {
            Workload::W1 => (self.batch * self.row.k) as f64,
            Workload::W2 => (self.row.k * self.row.n) as f64,
        }
    }

    /// Messages (W1) or basis vectors (W2) one call encodes, the unit the
    /// call-budget projection scales by.
    fn units(&self) -> usize {
        match self.workload {
            Workload::W1 => self.batch,
            Workload::W2 => self.row.k,
        }
    }

    /// `W1/<row>/B=<batch>` or `W2/<row>`.
    pub fn id(&self) -> String {
        match self.workload {
            Workload::W1 => format!("W1/{}/B={}", self.row.name, self.batch),
            Workload::W2 => format!("W2/{}", self.row.name),
        }
    }
}

/// Every cell in measuring order: per row, W1 at ascending batch, then W2.
/// `CUTOVER_CELLS`, a comma-separated list of cell-ID prefixes, keeps the
/// cells matching one of them.
pub fn cells() -> Vec<Cell> {
    let filter: Vec<String> = std::env::var("CUTOVER_CELLS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
        .map(str::to_owned)
        .collect();
    ROWS.iter()
        .flat_map(|row| {
            BATCHES
                .iter()
                .map(move |&batch| Cell {
                    workload: Workload::W1,
                    row,
                    batch,
                })
                .chain(std::iter::once(Cell {
                    workload: Workload::W2,
                    row,
                    batch: 1,
                }))
        })
        .filter(|cell| {
            let id = cell.id();
            filter.is_empty() || filter.iter().any(|prefix| id.starts_with(prefix.as_str()))
        })
        .collect()
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The first `count` messages of `row`, $k$ bits each, packed little-endian
/// with bit $i$ at word $i / 64$, bit $i \bmod 64$ and a clear tail. Message
/// $i$ is the same at every batch size.
pub fn messages(row: &Row, count: usize) -> Vec<Vec<u64>> {
    let words = row.k.div_ceil(64);
    let tail = row.k % 64;
    (0..count as u64)
        .map(|index| {
            let mut state = SEED ^ ((row.m as u64) << 48) ^ index;
            let mut message: Vec<u64> = (0..words).map(|_| splitmix(&mut state)).collect();
            if tail != 0 {
                message[words - 1] &= (1u64 << tail) - 1;
            }
            message
        })
        .collect()
}

/// FNV-1a over the little-endian bytes of `values`.
pub fn fnv1a(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Digest of `rows` vectors of `bits` bits each, every one packed like
/// [`messages`]: the shape, then every word in order.
pub fn digest<'a>(rows: usize, bits: usize, vectors: impl IntoIterator<Item = &'a [u64]>) -> u64 {
    fnv1a(
        [rows as u64, bits as u64]
            .into_iter()
            .chain(vectors.into_iter().flat_map(|words| words.iter().copied())),
    )
}

/// One timed configuration of an arm on one cell.
pub trait Config {
    /// What the configuration runs, e.g. the entry point or the build.
    fn label(&self) -> String;
    /// One call: encodes the cell's batch or materializes its generator.
    fn call(&mut self);
    /// [`digest`] of the output the last [`call`](Self::call) wrote.
    fn output_digest(&mut self) -> u64;
}

/// A configuration, or the label and reason of one that is not timed.
pub type Prepared<'a> = Result<Box<dyn Config + 'a>, (String, String)>;

/// One implementation: a gf2 revision or an external library.
pub trait Arm {
    /// `pre`, `current` or `external`.
    fn role(&self) -> &'static str;
    /// The implementation's name, e.g. `gf2 74871c6b` or `bchlib`.
    fn name(&self) -> String;
    /// The configurations this arm times on `cell`, over `messages`.
    fn configs<'a>(&'a self, cell: &Cell, messages: &'a [Vec<u64>]) -> Vec<Prepared<'a>>;
}

fn budget_seconds() -> f64 {
    std::env::var("CUTOVER_CALL_BUDGET_S")
        .ok()
        .map(|value| {
            value
                .parse()
                .expect("CUTOVER_CALL_BUDGET_S is a number of seconds")
        })
        .unwrap_or(DEFAULT_CALL_BUDGET_SECONDS)
}

/// Runs round `round` of every cell of [`cells`] on `arm`, printing one
/// tab-separated line per configuration:
/// `round role arm config workload row batch work calls seconds digest status`,
/// where `status` is `timed`, `projected:<seconds>` or `skipped:<reason>`.
///
/// A configuration's projected call is its fastest per-unit time so far on
/// the row, or without one the arm's fastest on the row, times the cell's
/// units; one above `CUTOVER_CALL_BUDGET_S` is not run.
pub fn run_round(arm: &dyn Arm, round: usize) {
    let budget = budget_seconds();
    let largest = BATCHES[BATCHES.len() - 1];
    let mut per_unit: HashMap<(&'static str, String), f64> = HashMap::new();
    let mut row_messages: Option<(&'static str, Vec<Vec<u64>>)> = None;
    for cell in cells() {
        if row_messages.as_ref().map(|(name, _)| *name) != Some(cell.row.name) {
            row_messages = Some((cell.row.name, messages(cell.row, largest)));
        }
        let all = &row_messages.as_ref().expect("set above").1;
        let batch_messages = match cell.workload {
            Workload::W1 => &all[..cell.batch],
            Workload::W2 => &all[..0],
        };
        let print = |config: &str, calls: u64, seconds: f64, digest: u64, status: &str| {
            println!(
                "{round}\t{}\t{}\t{config}\t{:?}\t{}\t{}\t{}\t{calls}\t{seconds:.6}\t{digest:016x}\t{status}",
                arm.role(),
                arm.name(),
                cell.workload,
                cell.row.name,
                cell.batch,
                cell.work()
            );
        };
        for prepared in arm.configs(&cell, batch_messages) {
            let mut config = match prepared {
                Ok(config) => config,
                Err((label, reason)) => {
                    print(&label, 0, 0.0, 0, &format!("skipped:{reason}"));
                    continue;
                }
            };
            let label = config.label();
            let key = (cell.row.name, label.clone());
            let known = per_unit.get(&key).copied().or_else(|| {
                per_unit
                    .iter()
                    .filter(|((row, _), _)| *row == cell.row.name)
                    .map(|(_, &seconds)| seconds)
                    .reduce(f64::min)
            });
            if let Some(seconds) = known {
                let projected = seconds * cell.units() as f64;
                if projected > budget {
                    print(&label, 0, 0.0, 0, &format!("projected:{projected:.1}"));
                    continue;
                }
            }
            let start = Instant::now();
            config.call();
            let first = start.elapsed().as_secs_f64();
            let output = config.output_digest();
            // A first call that fills a repetition is the repetition.
            let (mut calls, mut seconds) = if first >= MIN_REP_SECONDS {
                (1u64, first)
            } else {
                (0, 0.0)
            };
            let start = Instant::now();
            while seconds < MIN_REP_SECONDS {
                config.call();
                calls += 1;
                seconds = start.elapsed().as_secs_f64();
            }
            let unit = first.min(seconds / calls as f64) / cell.units() as f64;
            per_unit
                .entry(key)
                .and_modify(|best| *best = best.min(unit))
                .or_insert(unit);
            print(&label, calls, seconds, output, "timed");
        }
    }
}

/// The `main` of an arm binary: `<binary> round <n>` runs round `n`.
pub fn main(arm: &dyn Arm) {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("round") => {
            let round = args
                .get(2)
                .and_then(|value| value.parse().ok())
                .expect("usage: round <n>");
            run_round(arm, round);
        }
        _ => {
            eprintln!("usage: {} round <n>", args[0]);
            std::process::exit(2);
        }
    }
}
