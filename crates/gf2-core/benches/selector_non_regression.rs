//! Receipt-producing benchmark for the pinned selector non-regression set.
//!
//! Criterion is deliberately not used: this harness emits raw, pooled CSV
//! windows with explicit execution and fixture coordinates so a baseline and a
//! candidate can be compared cell by cell. It pins the operations selected by
//! the bit-backend and polynomial pilot families around every conservative
//! default in the tuning-profile selector cutover (epic `6dc81018`, design
//! `dev/active/220cab0b/design.md` §6 step 3). The cells use only public
//! `gf2-core` APIs and install no tuning profile, so the same receipt protocol
//! applies on either side of the cutover.
//!
//! The bit family measures the five entry points that read
//! `select_backend_for_size`: `xor_inplace`, `and_inplace`, `or_inplace`,
//! `not_inplace`, and `popcount`. The polynomial family measures multiplication,
//! fast multiplication, division/remainder, and both batch-evaluation entry
//! points at sizes that straddle their public thresholds. The mutating bit
//! logical kernels use branch-free per-word loops (with fixed unrolling in the
//! scalar implementation), so repeated in-place application changes operands
//! without changing the amount of work in a cell and does not bias its timing.
//!
//! Every fixture seed starts with `SEED_ROOT`, folds in the cell dimensions and
//! a role tag, then applies a SplitMix-style xor/shift/multiply finalizer. The
//! resulting seed is passed to `gf2_core::rng::Lcg`; this makes each cell and
//! fixture bank deterministic while keeping roles distinct.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gf2_core::field::poly::{
    mul_fast, FieldPoly, DIV_REM_THRESHOLD, KARATSUBA_THRESHOLD, NTT_THRESHOLD,
    SUBPRODUCT_THRESHOLD,
};
use gf2_core::gfp::Fp;
use gf2_core::rng::Lcg;

const SCHEMA_VERSION: &str = "selector-non-regression-v1";
const DEFAULT_REPETITIONS: u32 = 5;
const DEFAULT_TARGET_MS: u64 = 250;
const MAX_CALLS: u64 = 1 << 32;
const BIT_FIXTURES: usize = 8;
/// `u64` words per 64-byte cache line on the supported targets.
const WORDS_PER_LINE: usize = 8;
const SEED_ROOT: u64 = 0xe8fe_47f5_0000_0000;
const GIT_STATUS_ARGS: &[&str] = &["status", "--porcelain", "--untracked-files=all"];
/// Predeclared per-cell non-regression tolerance.
const PER_CELL_TOLERANCE: f64 = 0.05;
/// Predeclared whole-set tolerance on the geometric mean of cell ratios.
const SET_TOLERANCE: f64 = 0.02;
/// Multiples of the layout standard error a tolerance stands above 1 before a
/// verdict computed under it attributes movement to behaviour rather than to
/// across-build code layout.
const ATTRIBUTION_MARGIN: f64 = 3.0;

type F = Fp<65537>;

const CSV_HEADER: &str = "schema_version,execution,repetition,family,cell,arm,size_a,size_b,fixtures,fixture_start,calls,elapsed_ns,ns_per_call,target_ms,timestamp_unix_s,git_revision,source_dirty,rustc,hostname,cpu_model,kernel,governor";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BitOp {
    Xor,
    And,
    Or,
    Not,
    Popcount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    BitLogical {
        op: BitOp,
        words: usize,
    },
    PolyMul {
        len: usize,
    },
    PolyMulFast {
        len: usize,
    },
    PolyDivRem {
        dividend: usize,
        divisor: usize,
    },
    PolyBatchEval {
        auto: bool,
        coeffs: usize,
        points: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    execution: u32,
    repetitions: u32,
    target: Duration,
    output: PathBuf,
    append: bool,
    self_check: bool,
    list_cells: bool,
    compare: Option<PathBuf>,
    against: Option<PathBuf>,
    layout_audit: Option<PathBuf>,
    layout_candidate: Option<PathBuf>,
}

#[derive(Debug)]
struct Metadata {
    git_revision: String,
    source_dirty: bool,
    rustc: String,
    hostname: String,
    cpu_model: String,
    kernel: String,
    governor: String,
    timestamp_unix_s: u64,
}

enum Fixture {
    Bit {
        dst: BitBank,
        src: BitBank,
    },
    Mul {
        a: FieldPoly<F>,
        b: FieldPoly<F>,
    },
    DivRem {
        dividend: FieldPoly<F>,
        divisor: FieldPoly<F>,
    },
    BatchEval {
        poly: FieldPoly<F>,
        points: Vec<F>,
    },
}

/// `BIT_FIXTURES` buffers of `len` words, laid out contiguously with every
/// buffer starting on a 64-byte boundary.
///
/// The alignment is controlled rather than left to the allocator. A 64-byte
/// cache line holds eight `u64`, an AVX2 load that straddles two lines costs
/// materially more than one that does not, and the allocator's address phase
/// differs between processes. An uncontrolled bank therefore makes a whole
/// execution measure a different memory layout, which is a property of the heap
/// rather than of the selection boundary this set exists to gate.
struct BitBank {
    storage: Vec<u64>,
    offset: usize,
    stride: usize,
    len: usize,
}

impl BitBank {
    /// Builds a bank whose buffer `bank` holds `word(bank, index)` at `index`.
    fn new(len: usize, mut word: impl FnMut(usize, usize) -> u64) -> Self {
        let stride = len.next_multiple_of(WORDS_PER_LINE).max(WORDS_PER_LINE);
        let mut storage = vec![0_u64; BIT_FIXTURES * stride + WORDS_PER_LINE];
        let misalignment = (storage.as_ptr() as usize) % (WORDS_PER_LINE * 8);
        let offset = (WORDS_PER_LINE * 8 - misalignment) % (WORDS_PER_LINE * 8) / 8;
        for bank in 0..BIT_FIXTURES {
            for index in 0..len {
                storage[offset + bank * stride + index] = word(bank, index);
            }
        }
        let bank = Self {
            storage,
            offset,
            stride,
            len,
        };
        assert!(
            bank.is_line_aligned(),
            "bit fixture bank did not land on a cache-line boundary"
        );
        bank
    }

    /// Whether every buffer starts on a 64-byte boundary.
    fn is_line_aligned(&self) -> bool {
        let base = self.storage.as_ptr() as usize + self.offset * 8;
        base.is_multiple_of(WORDS_PER_LINE * 8) && self.stride.is_multiple_of(WORDS_PER_LINE)
    }

    fn start(&self, bank: usize) -> usize {
        self.offset + bank * self.stride
    }

    fn get(&self, bank: usize) -> &[u64] {
        let start = self.start(bank);
        &self.storage[start..start + self.len]
    }

    fn get_mut(&mut self, bank: usize) -> &mut [u64] {
        let start = self.start(bank);
        &mut self.storage[start..start + self.len]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CellStat {
    cell: String,
    family: String,
    arm: String,
    size_a: String,
    size_b: String,
    calls: u128,
    elapsed_ns: u128,
}

/// One cell's identity fields disagreeing between two receipts.
#[derive(Debug, PartialEq, Eq)]
struct IdentityMismatch {
    cell: String,
    field: &'static str,
    baseline: String,
    candidate: String,
}

/// Why two receipts cannot be compared at all.
///
/// This is a different outcome from a cell exceeding its tolerance: a tolerance
/// failure is a measured regression, while every variant here means the two
/// receipts do not describe the same measurement and no ratio between them
/// carries meaning.
#[derive(Debug, PartialEq, Eq)]
enum Incomparable {
    Schema { baseline: String, candidate: String },
    CellSet { difference: Vec<String> },
    NotPinnedSet,
    Identity { cells: Vec<IdentityMismatch> },
    Degenerate { cell: String, reason: &'static str },
}

impl Incomparable {
    /// Short token naming the failure class, printed on the `RESULT:` line.
    fn class(&self) -> &'static str {
        match self {
            Self::Schema { .. } => "schema mismatch",
            Self::CellSet { .. } => "cell-set mismatch",
            Self::NotPinnedSet => "not the pinned set",
            Self::Identity { .. } => "selector identity mismatch",
            Self::Degenerate { .. } => "degenerate cell",
        }
    }
}

impl fmt::Display for Incomparable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema {
                baseline,
                candidate,
            } => write!(
                f,
                "schema mismatch: baseline={baseline} candidate={candidate} expected={SCHEMA_VERSION}"
            ),
            Self::CellSet { difference } => {
                write!(f, "cell symmetric difference: {}", difference.join(", "))
            }
            Self::NotPinnedSet => write!(
                f,
                "receipt cell set does not match the {SCHEMA_VERSION} pinned set"
            ),
            Self::Identity { cells } => {
                writeln!(
                    f,
                    "selector identity mismatch: {} cell(s) resolve differently in the two receipts",
                    cells.len()
                )?;
                for mismatch in cells {
                    writeln!(
                        f,
                        "  {}: {} baseline={} candidate={}",
                        mismatch.cell, mismatch.field, mismatch.baseline, mismatch.candidate
                    )?;
                }
                Ok(())
            }
            Self::Degenerate { cell, reason } => write!(f, "{reason}: {cell}"),
        }
    }
}

#[derive(Debug)]
struct Receipt {
    schema_version: String,
    stats: BTreeMap<String, CellStat>,
}

#[derive(Debug)]
struct CellComparison {
    cell: String,
    arm: String,
    baseline: f64,
    candidate: f64,
    ratio: f64,
    passed: bool,
}

#[derive(Debug)]
struct Comparison {
    cells: Vec<CellComparison>,
    geomean: f64,
    passed: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    if args.self_check {
        return self_check();
    }
    if args.list_cells {
        list_cells();
        return Ok(());
    }
    if let Some(reference_path) = args.layout_audit.as_deref() {
        let candidate_path = args
            .layout_candidate
            .as_deref()
            .expect("parse_args validates --layout-audit/--layout-candidate pairing");
        let reference = load_ensemble(reference_path)?;
        let candidate = load_ensemble(candidate_path)?;
        // Under `--layout-audit` the receipt pair names the two committed
        // receipts whose ratios record what one natural rebuild moves, which is
        // the floor the ensemble's own dispersion is held against.
        let natural = match (args.compare.as_deref(), args.against.as_deref()) {
            (Some(baseline), Some(rebuild)) => {
                Some((load_receipt(baseline)?, load_receipt(rebuild)?))
            }
            _ => None,
        };
        return match audit_layout(
            &reference,
            &candidate,
            natural.as_ref().map(|(a, b)| (a, b)),
        ) {
            Ok(audit) => {
                print_layout_audit(reference_path, candidate_path, &audit);
                if audit.passed {
                    Ok(())
                } else {
                    Err("layout audit failed".into())
                }
            }
            Err(reason) => {
                println!(
                    "layout-audit: reference={} candidate={}",
                    reference_path.display(),
                    candidate_path.display()
                );
                println!("{reason}");
                println!("RESULT: FAIL ({})", reason.class());
                Err(format!("ensemble is not auditable: {}", reason.class()).into())
            }
        };
    }
    if let Some(baseline_path) = args.compare.as_deref() {
        let candidate_path = args
            .against
            .as_deref()
            .expect("parse_args validates --compare/--against pairing");
        let baseline = load_receipt(baseline_path)?;
        let candidate = load_receipt(candidate_path)?;
        return match compare(&baseline, &candidate) {
            Ok(comparison) => {
                print_comparison(baseline_path, candidate_path, &comparison);
                if comparison.passed {
                    Ok(())
                } else {
                    Err("comparison failed".into())
                }
            }
            Err(reason) => {
                print_incomparable(baseline_path, candidate_path, &reason);
                Err(format!("receipts are not comparable: {}", reason.class()).into())
            }
        };
    }

    let mut output = open_output(&args.output, args.append)?;
    let empty = output.metadata()?.len() == 0;
    let metadata = collect_metadata()?;
    if !args.append || empty {
        writeln!(output, "{CSV_HEADER}")?;
    }

    for cell in pinned_cells() {
        let mut fixture = build_fixture(cell);
        correctness_probe(cell, &fixture);
        let (calls, windows) = sweep_cell(
            cell,
            &mut fixture,
            args.target,
            args.execution,
            args.repetitions,
        );
        for (index, (fixture_start, elapsed)) in windows.into_iter().enumerate() {
            let repetition = index as u32 + 1;
            let elapsed_ns = elapsed.as_nanos();
            let (size_a, size_b) = cell_sizes(cell);
            writeln!(
                output,
                "{},{},{},{},{},{},{},{},{},{},{},{},{:.6},{},{},{},{},{},{},{},{},{}",
                SCHEMA_VERSION,
                args.execution,
                repetition,
                family(cell),
                csv_field(&cell_id(cell)),
                expected_arm(cell),
                size_a,
                size_b,
                fixture_count(cell),
                fixture_start,
                calls,
                elapsed_ns,
                elapsed_ns as f64 / calls as f64,
                args.target.as_millis(),
                metadata.timestamp_unix_s,
                csv_field(&metadata.git_revision),
                metadata.source_dirty,
                csv_field(&metadata.rustc),
                csv_field(&metadata.hostname),
                csv_field(&metadata.cpu_model),
                csv_field(&metadata.kernel),
                csv_field(&metadata.governor),
            )?;
        }
        output.flush()?;
    }
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut execution = 1;
    let mut repetitions = DEFAULT_REPETITIONS;
    let mut target_ms = DEFAULT_TARGET_MS;
    let mut output = None;
    let mut append = false;
    let mut self_check = false;
    let mut list_cells = false;
    let mut compare = None;
    let mut against = None;
    let mut layout_audit = None;
    let mut layout_candidate = None;
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--execution" => execution = parse_value(&mut iter, &arg)?,
            "--repetitions" => repetitions = parse_value(&mut iter, &arg)?,
            "--target-ms" => target_ms = parse_value(&mut iter, &arg)?,
            "--output" => output = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--append" => append = true,
            "--self-check" => self_check = true,
            "--list-cells" => list_cells = true,
            "--compare" => compare = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--against" => against = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--layout-audit" => layout_audit = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--layout-candidate" => {
                layout_candidate = Some(PathBuf::from(next_value(&mut iter, &arg)?))
            }
            "--bench" => {}
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if execution == 0 {
        return Err("--execution must be positive for recorded windows".into());
    }
    if repetitions == 0 {
        return Err("--repetitions must be positive".into());
    }
    if target_ms == 0 {
        return Err("--target-ms must be positive".into());
    }
    if compare.is_some() != against.is_some() {
        return Err("--compare and --against must be supplied together".into());
    }
    if layout_audit.is_some() != layout_candidate.is_some() {
        return Err("--layout-audit and --layout-candidate must be supplied together".into());
    }
    Ok(Args {
        execution,
        repetitions,
        target: Duration::from_millis(target_ms),
        output: output.unwrap_or_else(|| PathBuf::from("selector-non-regression.csv")),
        append,
        self_check,
        list_cells,
        compare,
        against,
        layout_audit,
        layout_candidate,
    })
}

fn parse_value<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    next_value(iter, flag)?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn open_output(path: &Path, append: bool) -> io::Result<File> {
    let path = resolve_repository_path(path);
    if !append && path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite existing raw receipt: {}",
                path.display()
            ),
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(false)
        .open(path)
}

/// Resolves a path the caller gave on the command line.
///
/// An absolute path is used as given. A relative path is taken as
/// repository-relative, because `cargo bench` runs this binary with its working
/// directory at the package root while the committed receipts and the procedure
/// that names them are written relative to the repository root. Every path
/// argument resolves this way, inputs and output alike, so one command works
/// from where the procedure says to run it.
fn resolve_repository_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}

fn pinned_cells() -> Vec<Cell> {
    let mut cells = Vec::with_capacity(34);
    for words in [1, 4, 7, 8, 16, 64] {
        cells.push(Cell::BitLogical {
            op: BitOp::Xor,
            words,
        });
    }
    for op in [BitOp::And, BitOp::Or, BitOp::Not, BitOp::Popcount] {
        cells.push(Cell::BitLogical { op, words: 1 });
        cells.push(Cell::BitLogical { op, words: 8 });
    }
    for len in [16, 32, 33, 64, 256] {
        cells.push(Cell::PolyMul { len });
    }
    for len in [32, 64, 65, 128, 512] {
        cells.push(Cell::PolyMulFast { len });
    }
    for (dividend, divisor) in [(4096, 1024), (4096, 2047), (4096, 2048), (8192, 4096)] {
        cells.push(Cell::PolyDivRem { dividend, divisor });
    }
    for auto in [false, true] {
        for (coeffs, points) in [(2048, 2048), (4095, 4095), (4096, 4096)] {
            cells.push(Cell::PolyBatchEval {
                auto,
                coeffs,
                points,
            });
        }
    }
    cells
}

fn bit_op_name(op: BitOp) -> &'static str {
    match op {
        BitOp::Xor => "xor_inplace",
        BitOp::And => "and_inplace",
        BitOp::Or => "or_inplace",
        BitOp::Not => "not_inplace",
        BitOp::Popcount => "popcount",
    }
}

fn cell_id(cell: Cell) -> String {
    match cell {
        Cell::BitLogical { op, words } => {
            format!("bit_backend/{}/words={words}", bit_op_name(op))
        }
        Cell::PolyMul { len } => format!("polynomial/mul/len={len}"),
        Cell::PolyMulFast { len } => format!("polynomial/mul_fast/len={len}"),
        Cell::PolyDivRem { dividend, divisor } => {
            format!("polynomial/div_rem_auto/dividend={dividend}/divisor={divisor}")
        }
        Cell::PolyBatchEval {
            auto,
            coeffs,
            points,
        } => {
            let name = if auto {
                "batch_evaluate_auto"
            } else {
                "batch_evaluate"
            };
            format!("polynomial/{name}/coeffs={coeffs}/points={points}")
        }
    }
}

/// Every pinned cell id, in the sorted order a parsed receipt yields them.
fn pinned_cell_ids() -> Vec<String> {
    let mut ids: Vec<String> = pinned_cells().into_iter().map(cell_id).collect();
    ids.sort();
    ids
}

fn family(cell: Cell) -> &'static str {
    match cell {
        Cell::BitLogical { .. } => "bit_backend",
        Cell::PolyMul { .. }
        | Cell::PolyMulFast { .. }
        | Cell::PolyDivRem { .. }
        | Cell::PolyBatchEval { .. } => "polynomial",
    }
}

fn expected_arm(cell: Cell) -> &'static str {
    match cell {
        Cell::BitLogical { words, .. } => gf2_core::kernels::select_backend_for_size(words).name(),
        Cell::PolyMul { len } => {
            if len - 1 < KARATSUBA_THRESHOLD {
                "schoolbook"
            } else {
                "karatsuba"
            }
        }
        Cell::PolyMulFast { len } => {
            if 2 * len - 1 <= NTT_THRESHOLD {
                "mul_dispatch"
            } else {
                "ntt"
            }
        }
        Cell::PolyDivRem { dividend, divisor } => {
            if dividend < DIV_REM_THRESHOLD || divisor < DIV_REM_THRESHOLD {
                "schoolbook"
            } else {
                "newton"
            }
        }
        Cell::PolyBatchEval { coeffs, points, .. } => {
            if points < SUBPRODUCT_THRESHOLD || coeffs < SUBPRODUCT_THRESHOLD {
                "horner"
            } else {
                "subproduct"
            }
        }
    }
}

/// Smallest word count at which the library observes the SIMD arm.
fn observed_simd_min_words() -> usize {
    (0..=4096)
        .find(|&words| gf2_core::kernels::select_backend_for_size(words).name() == "simd")
        .unwrap_or(usize::MAX)
}

fn cell_sizes(cell: Cell) -> (usize, usize) {
    match cell {
        Cell::BitLogical { words, .. }
        | Cell::PolyMul { len: words }
        | Cell::PolyMulFast { len: words } => (words, 0),
        Cell::PolyDivRem { dividend, divisor } => (dividend, divisor),
        Cell::PolyBatchEval { coeffs, points, .. } => (coeffs, points),
    }
}

fn fixture_count(cell: Cell) -> usize {
    match cell {
        Cell::BitLogical { .. } => BIT_FIXTURES,
        _ => 1,
    }
}

fn seed_for(cell: Cell, role: u64) -> u64 {
    let (a, b) = cell_sizes(cell);
    let shape = match cell {
        Cell::BitLogical { op, .. } => op as usize,
        Cell::PolyMul { .. } => 11,
        Cell::PolyMulFast { .. } => 12,
        Cell::PolyDivRem { .. } => 13,
        Cell::PolyBatchEval { auto, .. } => 14 + usize::from(auto),
    };
    let mut value = SEED_ROOT ^ role.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for word in [shape, a, b] {
        value ^= word as u64;
        value = value
            .wrapping_mul(0xbf58_476d_1ce4_e5b9)
            .rotate_left(27)
            .wrapping_add(0x94d0_49bb_1331_11eb);
    }
    value ^ (value >> 31)
}

/// Builds a deterministic polynomial holding exactly `len` coefficients.
///
/// Coefficients are drawn from `1..=65536`, so none of them is the zero element
/// of `Fp<65537>` and `FieldPoly::new`'s trailing-zero normalisation cannot
/// shorten the operand. The pinned cells select their arm from exact
/// coefficient counts, so the assertion below is a hard precondition rather
/// than a diagnostic.
fn make_poly(len: usize, seed: u64) -> FieldPoly<F> {
    let mut rng = Lcg::new(seed);
    let coeffs: Vec<F> = (0..len)
        .map(|_| F::new((rng.next_u64() % 65_536) + 1))
        .collect();
    let poly = FieldPoly::new(coeffs);
    assert_eq!(poly.len(), len, "fixture polynomial lost coefficients");
    poly
}

fn make_points(len: usize, seed: u64) -> Vec<F> {
    let stride = 1_000_003_u64;
    let modulus_minus_one = 65_536_u64;
    let offset = Lcg::new(seed).next_u64() % modulus_minus_one;
    (0..len)
        .map(|index| F::new((offset + (index as u64).wrapping_mul(stride)) % modulus_minus_one + 1))
        .collect()
}

fn build_fixture(cell: Cell) -> Fixture {
    match cell {
        Cell::BitLogical { words, .. } => {
            let fill = |role: u64| {
                let mut rngs: Vec<Lcg> = (0..BIT_FIXTURES)
                    .map(|bank| Lcg::new(seed_for(cell, role + bank as u64)))
                    .collect();
                BitBank::new(words, move |bank, _| rngs[bank].next_u64())
            };
            Fixture::Bit {
                dst: fill(0xD000_0000),
                src: fill(0xA000_0000),
            }
        }
        Cell::PolyMul { len } | Cell::PolyMulFast { len } => Fixture::Mul {
            a: make_poly(len, seed_for(cell, 0xA)),
            b: make_poly(len, seed_for(cell, 0xB)),
        },
        Cell::PolyDivRem { dividend, divisor } => Fixture::DivRem {
            dividend: make_poly(dividend, seed_for(cell, 0xD)),
            divisor: make_poly(divisor, seed_for(cell, 0xE)),
        },
        Cell::PolyBatchEval { coeffs, points, .. } => Fixture::BatchEval {
            poly: make_poly(coeffs, seed_for(cell, 0xC)),
            points: make_points(points, seed_for(cell, 0xF)),
        },
    }
}

fn correctness_probe(cell: Cell, fixture: &Fixture) {
    match (cell, fixture) {
        (Cell::BitLogical { words, .. }, Fixture::Bit { dst, src }) => {
            assert!(dst.is_line_aligned() && src.is_line_aligned());
            assert!((0..BIT_FIXTURES).all(|bank| dst.get(bank).len() == words));
            assert!((0..BIT_FIXTURES).all(|bank| src.get(bank).len() == words));
            assert_eq!(
                gf2_core::kernels::select_backend_for_size(words).name(),
                expected_arm(cell),
                "bit selector arm mismatch for {}",
                cell_id(cell)
            );
        }
        (Cell::PolyMul { len }, Fixture::Mul { a, b }) => {
            assert_eq!(a.len(), len);
            assert_eq!(b.len(), len);
        }
        (Cell::PolyMulFast { len }, Fixture::Mul { a, b }) => {
            assert_eq!(a.len(), len);
            assert_eq!(b.len(), len);
            assert_eq!(
                mul_fast(a, b),
                a.mul(b),
                "mul_fast mismatch for {}",
                cell_id(cell)
            );
        }
        (
            Cell::PolyDivRem { dividend, divisor },
            Fixture::DivRem {
                dividend: a,
                divisor: b,
            },
        ) => {
            assert_eq!(a.len(), dividend);
            assert_eq!(b.len(), divisor);
            let (q, r) = a.div_rem_auto(b);
            assert_eq!(mul_fast(&q, b) + &r, *a, "div/rem identity mismatch");
            if let Some(remainder_degree) = r.degree() {
                assert!(
                    remainder_degree < b.degree().expect("nonzero divisor"),
                    "remainder degree is not below divisor degree"
                );
            }
        }
        (
            Cell::PolyBatchEval {
                coeffs,
                points,
                auto,
                ..
            },
            Fixture::BatchEval { poly, points: xs },
        ) => {
            assert_eq!(poly.len(), coeffs);
            assert_eq!(xs.len(), points);
            let expected: Vec<_> = xs.iter().map(|x| poly.eval(x)).collect();
            let actual = if auto {
                poly.batch_evaluate_auto(xs)
            } else {
                poly.batch_evaluate(xs)
            };
            assert_eq!(
                actual,
                expected,
                "batch evaluation mismatch for {}",
                cell_id(cell)
            );
        }
        _ => panic!("fixture shape does not match {}", cell_id(cell)),
    }
}

/// Destination and source bank indices for call number `index`.
///
/// The two banks are offset from each other so a call never reads and writes
/// the same buffer.
fn bank_indices(index: usize) -> (usize, usize) {
    (
        index & (BIT_FIXTURES - 1),
        index.wrapping_add(3) & (BIT_FIXTURES - 1),
    )
}

/// One cell's whole timed sweep: call calibration followed by `repetitions`
/// timed windows, returning the calibrated call count and each window's
/// `(fixture_start, elapsed)`.
///
/// The dispatch on the cell shape happens once, here, so every timed loop is a
/// monomorphic sequence of calls into exactly one library entry point. Keeping
/// the harness's own `match` outside the window is load-bearing for the
/// small-buffer bit-logical cells: their per-call cost is a few nanoseconds, and
/// harness dispatch inside the window would dilute the very selection overhead
/// this set exists to bound.
fn sweep_cell(
    cell: Cell,
    fixture: &mut Fixture,
    target: Duration,
    execution: u32,
    repetitions: u32,
) -> (u64, Vec<(usize, Duration)>) {
    match (cell, fixture) {
        (Cell::BitLogical { op, .. }, Fixture::Bit { dst, src }) => match op {
            BitOp::Xor => sweep(target, execution, repetitions, |index| {
                let (i, j) = bank_indices(index);
                let src_bank: &[u64] = black_box(src.get(j));
                let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                gf2_core::kernels::ops::xor_inplace(dst_bank, src_bank);
                black_box(dst_bank);
            }),
            BitOp::And => sweep(target, execution, repetitions, |index| {
                let (i, j) = bank_indices(index);
                let src_bank: &[u64] = black_box(src.get(j));
                let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                gf2_core::kernels::ops::and_inplace(dst_bank, src_bank);
                black_box(dst_bank);
            }),
            BitOp::Or => sweep(target, execution, repetitions, |index| {
                let (i, j) = bank_indices(index);
                let src_bank: &[u64] = black_box(src.get(j));
                let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                gf2_core::kernels::ops::or_inplace(dst_bank, src_bank);
                black_box(dst_bank);
            }),
            BitOp::Not => sweep(target, execution, repetitions, |index| {
                let (i, _) = bank_indices(index);
                let dst_bank: &mut [u64] = black_box(dst.get_mut(i));
                gf2_core::kernels::ops::not_inplace(dst_bank);
                black_box(dst_bank);
            }),
            BitOp::Popcount => sweep(target, execution, repetitions, |index| {
                let (i, _) = bank_indices(index);
                let dst_bank: &[u64] = black_box(dst.get(i));
                black_box(gf2_core::kernels::ops::popcount(dst_bank));
            }),
        },
        (Cell::PolyMul { .. }, Fixture::Mul { a, b }) => {
            sweep(target, execution, repetitions, |_| {
                black_box(black_box(&*a).mul(black_box(&*b)));
            })
        }
        (Cell::PolyMulFast { .. }, Fixture::Mul { a, b }) => {
            sweep(target, execution, repetitions, |_| {
                black_box(mul_fast(black_box(&*a), black_box(&*b)));
            })
        }
        (Cell::PolyDivRem { .. }, Fixture::DivRem { dividend, divisor }) => {
            sweep(target, execution, repetitions, |_| {
                black_box(black_box(&*dividend).div_rem_auto(black_box(&*divisor)));
            })
        }
        (Cell::PolyBatchEval { auto: false, .. }, Fixture::BatchEval { poly, points }) => {
            sweep(target, execution, repetitions, |_| {
                black_box(black_box(&*poly).batch_evaluate(black_box(&points[..])));
            })
        }
        (Cell::PolyBatchEval { auto: true, .. }, Fixture::BatchEval { poly, points }) => {
            sweep(target, execution, repetitions, |_| {
                black_box(black_box(&*poly).batch_evaluate_auto(black_box(&points[..])));
            })
        }
        _ => panic!("fixture shape does not match {}", cell_id(cell)),
    }
}

/// Calibrates the call count against `target`, then records `repetitions`
/// timed windows of exactly that many calls.
fn sweep(
    target: Duration,
    execution: u32,
    repetitions: u32,
    mut body: impl FnMut(usize),
) -> (u64, Vec<(usize, Duration)>) {
    let calls = calibrated_calls(target, &mut body);
    let mut windows = Vec::with_capacity(repetitions as usize);
    for repetition in 1..=repetitions {
        let fixture_start = recorded_fixture_start(execution, repetition, repetitions);
        windows.push((fixture_start, time_calls(calls, fixture_start, &mut body)));
    }
    (calls, windows)
}

/// Times `calls` invocations of `body`, walking the fixture bank from
/// `start_index`.
fn time_calls(calls: u64, start_index: usize, mut body: impl FnMut(usize)) -> Duration {
    let start = Instant::now();
    for call in 0..calls {
        body(start_index.wrapping_add(call as usize));
    }
    start.elapsed()
}

fn calibrated_calls(target: Duration, mut call: impl FnMut(usize)) -> u64 {
    let probe_target = target.min(Duration::from_millis(20));
    let mut calls = 1_u64;
    loop {
        let start = Instant::now();
        for index in 0..calls {
            call((index as usize) & (BIT_FIXTURES - 1));
        }
        let elapsed = start.elapsed();
        if elapsed >= probe_target || calls >= MAX_CALLS {
            let elapsed_ns = elapsed.as_nanos().max(1);
            let wanted = target.as_nanos().saturating_mul(calls as u128) / elapsed_ns;
            return wanted.clamp(1, MAX_CALLS as u128) as u64;
        }
        calls = calls.saturating_mul(2).min(MAX_CALLS);
    }
}

fn recorded_fixture_start(execution: u32, repetition: u32, repetitions: u32) -> usize {
    debug_assert!(execution >= 1);
    debug_assert!((1..=repetitions).contains(&repetition));
    (((execution - 1) as usize * repetitions as usize) + (repetition - 1) as usize)
        & (BIT_FIXTURES - 1)
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn command_output(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} {} failed with {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn collect_metadata() -> io::Result<Metadata> {
    let git_revision = command_output("git", &["rev-parse", "HEAD"])?;
    let status = command_output("git", GIT_STATUS_ARGS)?;
    let rustc = command_output("rustc", &["+1.95.0", "--version"])?;
    let hostname = command_output("hostname", &[])?;
    let kernel = command_output("uname", &["-srvmo"])?;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")?;
    let cpu_model = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .unwrap_or("unknown")
        .to_owned();
    let governor = fs::read_to_string("/sys/devices/system/cpu/cpu6/cpufreq/scaling_governor")
        .unwrap_or_else(|_| "unknown".to_owned())
        .trim()
        .to_owned();
    let timestamp_unix_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    Ok(Metadata {
        git_revision,
        source_dirty: source_dirty_from_porcelain(&status),
        rustc,
        hostname,
        cpu_model,
        kernel,
        governor,
        timestamp_unix_s,
    })
}

fn source_dirty_from_porcelain(status: &str) -> bool {
    !status.is_empty()
}

fn parse_csv(data: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = data.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(ch),
            }
        } else {
            match ch {
                '"' if field.is_empty() => quoted = true,
                ',' => {
                    row.push(std::mem::take(&mut field));
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(ch),
            }
        }
    }
    if quoted {
        return Err("unterminated quoted CSV field".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

fn csv_value<'a>(
    row: &'a [String],
    columns: &BTreeMap<&str, usize>,
    name: &str,
) -> Result<&'a str, String> {
    row.get(*columns.get(name).expect("required column exists"))
        .map(String::as_str)
        .ok_or_else(|| format!("receipt row lacks {name}"))
}

fn load_receipt(path: &Path) -> Result<Receipt, String> {
    let resolved = resolve_repository_path(path);
    let data = fs::read_to_string(&resolved)
        .map_err(|error| format!("{}: {error}", resolved.display()))?;
    let rows = parse_csv(&data)?;
    let header = rows.first().ok_or_else(|| "receipt is empty".to_owned())?;
    let mut columns = BTreeMap::new();
    for (index, name) in header.iter().enumerate() {
        columns.insert(name.as_str(), index);
    }
    for required in [
        "schema_version",
        "family",
        "cell",
        "arm",
        "size_a",
        "size_b",
        "calls",
        "elapsed_ns",
    ] {
        if !columns.contains_key(required) {
            return Err(format!("receipt header lacks {required}"));
        }
    }
    let mut schema_version = None;
    let mut stats = BTreeMap::new();
    for row in rows.iter().skip(1) {
        if row.iter().all(String::is_empty) {
            continue;
        }
        let schema = csv_value(row, &columns, "schema_version")?.to_owned();
        if let Some(expected) = &schema_version {
            if expected != &schema {
                return Err("receipt contains multiple schema versions".into());
            }
        } else {
            schema_version = Some(schema.clone());
        }
        let cell = csv_value(row, &columns, "cell")?.to_owned();
        let stat = CellStat {
            cell: cell.clone(),
            family: csv_value(row, &columns, "family")?.to_owned(),
            arm: csv_value(row, &columns, "arm")?.to_owned(),
            size_a: csv_value(row, &columns, "size_a")?.to_owned(),
            size_b: csv_value(row, &columns, "size_b")?.to_owned(),
            calls: csv_value(row, &columns, "calls")?
                .parse()
                .map_err(|_| format!("invalid calls for {cell}"))?,
            elapsed_ns: csv_value(row, &columns, "elapsed_ns")?
                .parse()
                .map_err(|_| format!("invalid elapsed_ns for {cell}"))?,
        };
        let existing = stats.entry(cell).or_insert_with(|| CellStat {
            calls: 0,
            elapsed_ns: 0,
            ..stat.clone()
        });
        if existing.family != stat.family
            || existing.arm != stat.arm
            || existing.size_a != stat.size_a
            || existing.size_b != stat.size_b
        {
            return Err(format!(
                "cell metadata changes across rows: {}",
                existing.cell
            ));
        }
        existing.calls = existing.calls.saturating_add(stat.calls);
        existing.elapsed_ns = existing.elapsed_ns.saturating_add(stat.elapsed_ns);
    }
    Ok(Receipt {
        schema_version: schema_version.ok_or_else(|| "receipt has no data rows".to_owned())?,
        stats,
    })
}

fn pooled_ns_per_call(stat: &CellStat) -> f64 {
    stat.elapsed_ns as f64 / stat.calls as f64
}

fn compare(baseline: &Receipt, candidate: &Receipt) -> Result<Comparison, Incomparable> {
    if baseline.schema_version != SCHEMA_VERSION
        || candidate.schema_version != SCHEMA_VERSION
        || baseline.schema_version != candidate.schema_version
    {
        return Err(Incomparable::Schema {
            baseline: baseline.schema_version.clone(),
            candidate: candidate.schema_version.clone(),
        });
    }
    if baseline.stats.keys().ne(candidate.stats.keys()) {
        let mut difference = Vec::new();
        for cell in baseline.stats.keys().chain(candidate.stats.keys()) {
            if baseline.stats.contains_key(cell) != candidate.stats.contains_key(cell) {
                difference.push(cell.clone());
            }
        }
        difference.sort();
        difference.dedup();
        return Err(Incomparable::CellSet { difference });
    }
    // The schema token names one pinned set, so a receipt carrying that token
    // and a different set of cells was produced by a different protocol and is
    // not comparable, however well its two files agree with each other.
    if !baseline.stats.keys().eq(pinned_cell_ids().iter()) {
        return Err(Incomparable::NotPinnedSet);
    }
    // Default behaviour identical by construction is the cutover's whole
    // contract, and both receipts are taken with no profile installed, so every
    // cell resolves to the same arm on both sides. A cell whose recorded arm,
    // family, or operand sizes moved is two different code paths, and a ratio
    // between them measures no regression at all. It fails here as its own
    // class rather than passing on a tolerance.
    let mut identity = Vec::new();
    for (cell, baseline_stat) in &baseline.stats {
        let candidate_stat = &candidate.stats[cell];
        for (field, left, right) in [
            ("arm", &baseline_stat.arm, &candidate_stat.arm),
            ("family", &baseline_stat.family, &candidate_stat.family),
            ("size_a", &baseline_stat.size_a, &candidate_stat.size_a),
            ("size_b", &baseline_stat.size_b, &candidate_stat.size_b),
        ] {
            if left != right {
                identity.push(IdentityMismatch {
                    cell: cell.clone(),
                    field,
                    baseline: left.clone(),
                    candidate: right.clone(),
                });
            }
        }
    }
    if !identity.is_empty() {
        return Err(Incomparable::Identity { cells: identity });
    }
    let mut cells = Vec::with_capacity(baseline.stats.len());
    let mut log_sum = 0.0;
    for (cell, baseline_stat) in &baseline.stats {
        let candidate_stat = &candidate.stats[cell];
        if baseline_stat.calls == 0 || candidate_stat.calls == 0 {
            return Err(Incomparable::Degenerate {
                cell: cell.clone(),
                reason: "cell has zero calls",
            });
        }
        let baseline_rate = pooled_ns_per_call(baseline_stat);
        let candidate_rate = pooled_ns_per_call(candidate_stat);
        if baseline_rate <= 0.0 || candidate_rate <= 0.0 {
            return Err(Incomparable::Degenerate {
                cell: cell.clone(),
                reason: "cell has non-positive elapsed time",
            });
        }
        let ratio = candidate_rate / baseline_rate;
        log_sum += ratio.ln();
        cells.push(CellComparison {
            cell: cell.clone(),
            arm: candidate_stat.arm.clone(),
            baseline: baseline_rate,
            candidate: candidate_rate,
            ratio,
            passed: ratio <= 1.0 + PER_CELL_TOLERANCE,
        });
    }
    let geomean = (log_sum / cells.len() as f64).exp();
    let passed = cells.iter().all(|cell| cell.passed) && geomean <= 1.0 + SET_TOLERANCE;
    Ok(Comparison {
        cells,
        geomean,
        passed,
    })
}

fn print_comparison(baseline: &Path, candidate: &Path, comparison: &Comparison) {
    println!(
        "comparison: baseline={} candidate={} per_cell_tolerance={PER_CELL_TOLERANCE:.6} set_tolerance={SET_TOLERANCE:.6}",
        baseline.display(),
        candidate.display()
    );
    println!("cell arm baseline_ns_per_call candidate_ns_per_call ratio verdict");
    for cell in &comparison.cells {
        println!(
            "{} {} {:.6} {:.6} {:.6} {}",
            cell.cell,
            cell.arm,
            cell.baseline,
            cell.candidate,
            cell.ratio,
            if cell.passed { "PASS" } else { "FAIL" }
        );
    }
    println!(
        "geometric_mean {:.6} {}",
        comparison.geomean,
        if comparison.geomean <= 1.0 + SET_TOLERANCE {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "RESULT: {}",
        if comparison.passed { "PASS" } else { "FAIL" }
    );
}

/// One arm of a layout ensemble: one revision measured from several builds,
/// keyed by the execution index each build records.
///
/// The recording path is unchanged and the row schema is unchanged: a build
/// occupies one execution index, so the `execution` column that separates
/// processes in a single-build receipt separates builds here as well.
#[derive(Debug)]
struct EnsembleArm {
    schema_version: String,
    revisions: BTreeSet<String>,
    dirty_builds: BTreeSet<u32>,
    builds: BTreeMap<u32, BTreeMap<String, CellStat>>,
}

impl EnsembleArm {
    /// The one revision every row of a well-formed arm records.
    fn revision(&self) -> String {
        self.revisions
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned())
    }

    /// Pooled ns/call for `cell` over the builds whose index satisfies `keep`,
    /// on the same totals-over-totals statistic a single-build receipt uses.
    fn pooled(&self, cell: &str, keep: impl Fn(u32) -> bool) -> f64 {
        let mut calls = 0_u128;
        let mut elapsed = 0_u128;
        for (index, stats) in &self.builds {
            if keep(*index) {
                let stat = &stats[cell];
                calls += stat.calls;
                elapsed += stat.elapsed_ns;
            }
        }
        elapsed as f64 / calls as f64
    }

    /// Natural logarithm of every build's pooled ns/call for `cell`, in build
    /// index order, which is the order both arms of a paired ensemble share.
    fn log_rates(&self, cell: &str) -> Vec<f64> {
        self.builds
            .values()
            .map(|stats| pooled_ns_per_call(&stats[cell]).ln())
            .collect()
    }
}

/// Why an ensemble pair cannot be audited at all.
///
/// This is a different outcome from a precondition failing on the measurement:
/// every variant here means the files do not describe a layout ensemble of the
/// pinned set, so no dispersion computed from them carries meaning.
#[derive(Debug, PartialEq, Eq)]
enum AuditError {
    Schema {
        found: String,
    },
    Dirty {
        build: u32,
    },
    Revision {
        first: String,
        second: String,
    },
    CellSet {
        build: u32,
    },
    Identity(IdentityMismatch),
    Degenerate {
        cell: String,
        build: u32,
    },
    BuildCount {
        builds: usize,
        halves: (usize, usize),
    },
    ArmBuilds {
        reference: usize,
        candidate: usize,
    },
    NaturalPair,
}

impl AuditError {
    /// Short token naming the failure class, printed on the `RESULT:` line.
    fn class(&self) -> &'static str {
        match self {
            Self::Schema { .. } => "schema mismatch",
            Self::Dirty { .. } => "dirty source row",
            Self::Revision { .. } => "arm spans several revisions",
            Self::CellSet { .. } => "not the pinned set",
            Self::Identity(_) => "selector identity mismatch",
            Self::Degenerate { .. } => "degenerate cell",
            Self::BuildCount { .. } => "unusable build count",
            Self::ArmBuilds { .. } => "arms carry different builds",
            Self::NaturalPair => "natural pair is not the pinned set",
        }
    }
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema { found } => {
                write!(f, "schema mismatch: found={found} expected={SCHEMA_VERSION}")
            }
            Self::Dirty { build } => write!(
                f,
                "build {build} records source_dirty=true; an ensemble arm gates on a clean tree"
            ),
            Self::Revision { first, second } => write!(
                f,
                "one arm records two revisions: {first} and {second}; each arm is one revision built several ways"
            ),
            Self::CellSet { build } => write!(
                f,
                "build {build} does not carry the {SCHEMA_VERSION} pinned set"
            ),
            Self::Identity(mismatch) => write!(
                f,
                "{}: {} differs across builds: {} against {}",
                mismatch.cell, mismatch.field, mismatch.baseline, mismatch.candidate
            ),
            Self::Degenerate { cell, build } => {
                write!(f, "cell has zero calls or non-positive time: {cell} in build {build}")
            }
            Self::BuildCount { builds, halves } => write!(
                f,
                "ensemble has {builds} build(s) split {}/{} by execution index parity; the audit needs at least four builds in two equal halves",
                halves.0, halves.1
            ),
            Self::ArmBuilds {
                reference,
                candidate,
            } => write!(
                f,
                "arms carry different build indices: reference has {reference}, candidate has {candidate}"
            ),
            Self::NaturalPair => write!(
                f,
                "the receipts naming the natural across-build pair do not both carry the {SCHEMA_VERSION} pinned set"
            ),
        }
    }
}

/// One cell's layout dispersion and the margin its verdict carries.
#[derive(Debug)]
struct CellAudit {
    cell: String,
    arm: String,
    reference_spread: f64,
    reference_log_sd: f64,
    candidate_log_sd: f64,
    paired_log_sd: f64,
    layout_se: f64,
    margin: f64,
    null_ratio: f64,
    natural_sigma: Option<f64>,
    passed: bool,
}

/// Whether the ensemble moves an arm at least as far as one natural rebuild
/// moved the same cells, compared on the root mean square over the pinned set.
#[derive(Debug)]
struct Coverage {
    ensemble_rms: f64,
    natural_rms: f64,
    passed: bool,
}

/// Whether the ensemble places the two arms independently enough for their
/// paired ratio to scatter, rather than carrying one fixed layout difference.
#[derive(Debug)]
struct Decorrelation {
    paired_rms: f64,
    reference_rms: f64,
    passed: bool,
}

#[derive(Debug)]
struct LayoutAudit {
    builds: usize,
    reference_revision: String,
    candidate_revision: String,
    cells: Vec<CellAudit>,
    set_layout_se: f64,
    set_margin: f64,
    null_geomean: f64,
    coverage: Option<Coverage>,
    decorrelation: Decorrelation,
    passed: bool,
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Sample standard deviation on the unbiased denominator, zero below two
/// samples.
fn sample_sd(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let centre = mean(values);
    let sum: f64 = values.iter().map(|value| (value - centre).powi(2)).sum();
    (sum / (values.len() - 1) as f64).sqrt()
}

fn root_mean_square(values: &[f64]) -> f64 {
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

/// Reads one ensemble arm: several builds of one revision, one execution index
/// each, all carrying the pinned set.
fn load_ensemble(path: &Path) -> Result<EnsembleArm, String> {
    let resolved = resolve_repository_path(path);
    let data = fs::read_to_string(&resolved)
        .map_err(|error| format!("{}: {error}", resolved.display()))?;
    let rows = parse_csv(&data)?;
    let header = rows.first().ok_or_else(|| "receipt is empty".to_owned())?;
    let mut columns = BTreeMap::new();
    for (index, name) in header.iter().enumerate() {
        columns.insert(name.as_str(), index);
    }
    for required in [
        "schema_version",
        "execution",
        "family",
        "cell",
        "arm",
        "size_a",
        "size_b",
        "calls",
        "elapsed_ns",
        "git_revision",
        "source_dirty",
    ] {
        if !columns.contains_key(required) {
            return Err(format!("receipt header lacks {required}"));
        }
    }
    let mut schema_version = None;
    let mut revisions = BTreeSet::new();
    let mut dirty_builds = BTreeSet::new();
    let mut builds: BTreeMap<u32, BTreeMap<String, CellStat>> = BTreeMap::new();
    for row in rows.iter().skip(1) {
        if row.iter().all(String::is_empty) {
            continue;
        }
        let schema = csv_value(row, &columns, "schema_version")?.to_owned();
        if schema_version.get_or_insert(schema.clone()) != &schema {
            return Err("receipt contains multiple schema versions".into());
        }
        revisions.insert(csv_value(row, &columns, "git_revision")?.to_owned());
        let execution: u32 = csv_value(row, &columns, "execution")?
            .parse()
            .map_err(|_| "invalid execution index".to_owned())?;
        if csv_value(row, &columns, "source_dirty")? != "false" {
            dirty_builds.insert(execution);
        }
        let cell = csv_value(row, &columns, "cell")?.to_owned();
        let stat = CellStat {
            cell: cell.clone(),
            family: csv_value(row, &columns, "family")?.to_owned(),
            arm: csv_value(row, &columns, "arm")?.to_owned(),
            size_a: csv_value(row, &columns, "size_a")?.to_owned(),
            size_b: csv_value(row, &columns, "size_b")?.to_owned(),
            calls: csv_value(row, &columns, "calls")?
                .parse()
                .map_err(|_| format!("invalid calls for {cell}"))?,
            elapsed_ns: csv_value(row, &columns, "elapsed_ns")?
                .parse()
                .map_err(|_| format!("invalid elapsed_ns for {cell}"))?,
        };
        let build = builds.entry(execution).or_default();
        let existing = build.entry(cell).or_insert_with(|| CellStat {
            calls: 0,
            elapsed_ns: 0,
            ..stat.clone()
        });
        if existing.family != stat.family
            || existing.arm != stat.arm
            || existing.size_a != stat.size_a
            || existing.size_b != stat.size_b
        {
            return Err(format!(
                "cell metadata changes across rows: {}",
                existing.cell
            ));
        }
        existing.calls = existing.calls.saturating_add(stat.calls);
        existing.elapsed_ns = existing.elapsed_ns.saturating_add(stat.elapsed_ns);
    }
    Ok(EnsembleArm {
        schema_version: schema_version.ok_or_else(|| "receipt has no data rows".to_owned())?,
        revisions,
        dirty_builds,
        builds,
    })
}

/// Rejects an arm that is not one revision's pinned set measured from several
/// clean builds, so no dispersion is computed from rows that mix them.
fn check_arm(arm: &EnsembleArm) -> Result<(), AuditError> {
    if arm.schema_version != SCHEMA_VERSION {
        return Err(AuditError::Schema {
            found: arm.schema_version.clone(),
        });
    }
    if let Some(build) = arm.dirty_builds.iter().next() {
        return Err(AuditError::Dirty { build: *build });
    }
    let mut recorded = arm.revisions.iter();
    let first = recorded
        .next()
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    if let Some(second) = recorded.next() {
        return Err(AuditError::Revision {
            first,
            second: second.clone(),
        });
    }
    let pinned = pinned_cell_ids();
    let mut first: Option<&BTreeMap<String, CellStat>> = None;
    for (index, stats) in &arm.builds {
        if !stats.keys().eq(pinned.iter()) {
            return Err(AuditError::CellSet { build: *index });
        }
        for (cell, stat) in stats {
            if stat.calls == 0 || stat.elapsed_ns == 0 {
                return Err(AuditError::Degenerate {
                    cell: cell.clone(),
                    build: *index,
                });
            }
        }
        // Both arms are taken with no profile installed, so a cell that resolves
        // to a different arm in two builds is two code paths and its dispersion
        // is not a layout measurement.
        if let Some(reference) = first {
            for (cell, stat) in stats {
                let other = &reference[cell];
                for (field, left, right) in [
                    ("arm", &other.arm, &stat.arm),
                    ("family", &other.family, &stat.family),
                    ("size_a", &other.size_a, &stat.size_a),
                    ("size_b", &other.size_b, &stat.size_b),
                ] {
                    if left != right {
                        return Err(AuditError::Identity(IdentityMismatch {
                            cell: cell.clone(),
                            field,
                            baseline: left.clone(),
                            candidate: right.clone(),
                        }));
                    }
                }
            }
        } else {
            first = Some(stats);
        }
    }
    let halves = (
        arm.builds.keys().filter(|index| *index % 2 == 0).count(),
        arm.builds.keys().filter(|index| *index % 2 == 1).count(),
    );
    if arm.builds.len() < 4 || halves.0 != halves.1 {
        return Err(AuditError::BuildCount {
            builds: arm.builds.len(),
            halves,
        });
    }
    Ok(())
}

/// Audits a paired layout ensemble: how far across-build layout moves each cell,
/// and how far that leaves the verdict's own statistic from the tolerance.
fn audit_layout(
    reference: &EnsembleArm,
    candidate: &EnsembleArm,
    natural: Option<(&Receipt, &Receipt)>,
) -> Result<LayoutAudit, AuditError> {
    check_arm(reference)?;
    check_arm(candidate)?;
    if !reference.builds.keys().eq(candidate.builds.keys()) {
        return Err(AuditError::ArmBuilds {
            reference: reference.builds.len(),
            candidate: candidate.builds.len(),
        });
    }
    let pinned = pinned_cell_ids();
    let natural_sigma = match natural {
        Some((baseline, rebuild)) => {
            if baseline.schema_version != SCHEMA_VERSION
                || rebuild.schema_version != SCHEMA_VERSION
                || !baseline.stats.keys().eq(pinned.iter())
                || !rebuild.stats.keys().eq(pinned.iter())
            {
                return Err(AuditError::NaturalPair);
            }
            // One pair of builds of one revision estimates that revision's
            // across-build dispersion as the pair's log ratio over root two.
            Some(
                pinned
                    .iter()
                    .map(|cell| {
                        let ratio = pooled_ns_per_call(&rebuild.stats[cell])
                            / pooled_ns_per_call(&baseline.stats[cell]);
                        (ratio.ln().abs() / f64::sqrt(2.0)).max(0.0)
                    })
                    .collect::<Vec<_>>(),
            )
        }
        None => None,
    };
    let builds = reference.builds.len();
    let cell_tolerance = (1.0 + PER_CELL_TOLERANCE).ln();
    let set_tolerance = (1.0 + SET_TOLERANCE).ln();
    let mut cells = Vec::with_capacity(pinned.len());
    let mut null_log_sum = 0.0;
    for (position, cell) in pinned.iter().enumerate() {
        let reference_logs = reference.log_rates(cell);
        let candidate_logs = candidate.log_rates(cell);
        let paired: Vec<f64> = candidate_logs
            .iter()
            .zip(&reference_logs)
            .map(|(candidate, reference)| candidate - reference)
            .collect();
        let reference_log_sd = sample_sd(&reference_logs);
        let candidate_log_sd = sample_sd(&candidate_logs);
        let spread_log = reference_logs.iter().copied().fold(f64::MIN, f64::max)
            - reference_logs.iter().copied().fold(f64::MAX, f64::min);
        // The verdict divides one arm's pooled rate by the other's, so its
        // layout error carries both arms' across-build dispersion. Taking them
        // as independent is the conservative reading: layout effects the two
        // arms share only shrink it.
        let layout_se =
            (reference_log_sd.powi(2) + candidate_log_sd.powi(2)).sqrt() / (builds as f64).sqrt();
        let margin = if layout_se > 0.0 {
            cell_tolerance / layout_se
        } else {
            f64::INFINITY
        };
        let null_ratio = reference.pooled(cell, |index| index % 2 == 0)
            / reference.pooled(cell, |index| index % 2 == 1);
        let sigma = natural_sigma.as_ref().map(|values| values[position]);
        let covered = sigma.is_none_or(|sigma| reference_log_sd >= sigma / 2.0);
        null_log_sum += null_ratio.ln();
        cells.push(CellAudit {
            cell: cell.clone(),
            arm: reference.builds.values().next().expect("arm has builds")[cell]
                .arm
                .clone(),
            reference_spread: spread_log.exp(),
            reference_log_sd,
            candidate_log_sd,
            paired_log_sd: sample_sd(&paired),
            layout_se,
            margin,
            null_ratio,
            natural_sigma: sigma,
            passed: margin >= ATTRIBUTION_MARGIN
                && null_ratio.ln().abs() <= cell_tolerance
                && covered,
        });
    }
    let set_reference_sd = sample_sd(&set_level_deviations(reference, &pinned));
    let set_candidate_sd = sample_sd(&set_level_deviations(candidate, &pinned));
    let set_layout_se =
        (set_reference_sd.powi(2) + set_candidate_sd.powi(2)).sqrt() / (builds as f64).sqrt();
    let set_margin = if set_layout_se > 0.0 {
        set_tolerance / set_layout_se
    } else {
        f64::INFINITY
    };
    let null_geomean = (null_log_sum / cells.len() as f64).exp();
    let reference_rms = root_mean_square(
        &cells
            .iter()
            .map(|cell| cell.reference_log_sd)
            .collect::<Vec<_>>(),
    );
    let coverage = natural_sigma.as_ref().map(|sigma| {
        let natural_rms = root_mean_square(sigma);
        Coverage {
            ensemble_rms: reference_rms,
            natural_rms,
            passed: reference_rms >= natural_rms,
        }
    });
    let paired_rms = root_mean_square(
        &cells
            .iter()
            .map(|cell| cell.paired_log_sd)
            .collect::<Vec<_>>(),
    );
    let decorrelation = Decorrelation {
        paired_rms,
        reference_rms,
        passed: paired_rms >= reference_rms,
    };
    let passed = cells.iter().all(|cell| cell.passed)
        && set_margin >= ATTRIBUTION_MARGIN
        && null_geomean.ln().abs() <= set_tolerance
        && coverage.as_ref().is_none_or(|coverage| coverage.passed)
        && decorrelation.passed;
    Ok(LayoutAudit {
        builds,
        reference_revision: reference.revision(),
        candidate_revision: candidate.revision(),
        cells,
        set_layout_se,
        set_margin,
        null_geomean,
        coverage,
        decorrelation,
        passed,
    })
}

/// Each build's whole-set displacement: the mean over the pinned cells of the
/// build's log rate against that cell's ensemble mean. Its dispersion over the
/// builds is what the set rule's own layout error is computed from.
fn set_level_deviations(arm: &EnsembleArm, pinned: &[String]) -> Vec<f64> {
    let centres: Vec<f64> = pinned
        .iter()
        .map(|cell| mean(&arm.log_rates(cell)))
        .collect();
    arm.builds
        .values()
        .map(|stats| {
            mean(
                &pinned
                    .iter()
                    .zip(&centres)
                    .map(|(cell, centre)| pooled_ns_per_call(&stats[cell]).ln() - centre)
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn print_layout_audit(reference: &Path, candidate: &Path, audit: &LayoutAudit) {
    println!(
        "layout-audit: reference={} candidate={} reference_revision={} candidate_revision={} builds={} per_cell_tolerance={PER_CELL_TOLERANCE:.6} set_tolerance={SET_TOLERANCE:.6} attribution_margin={ATTRIBUTION_MARGIN:.6}",
        reference.display(),
        candidate.display(),
        audit.reference_revision,
        audit.candidate_revision,
        audit.builds
    );
    println!(
        "cell arm reference_spread reference_log_sd candidate_log_sd paired_log_sd layout_se margin null_ratio natural_sigma verdict"
    );
    for cell in &audit.cells {
        let natural = match cell.natural_sigma {
            Some(sigma) => format!("{sigma:.6}"),
            None => "-".to_owned(),
        };
        println!(
            "{} {} {:.6} {:.6} {:.6} {:.6} {:.6} {:.3} {:.6} {natural} {}",
            cell.cell,
            cell.arm,
            cell.reference_spread,
            cell.reference_log_sd,
            cell.candidate_log_sd,
            cell.paired_log_sd,
            cell.layout_se,
            cell.margin,
            cell.null_ratio,
            if cell.passed { "PASS" } else { "FAIL" }
        );
    }
    println!(
        "set layout_se={:.6} margin={:.3} null_geomean={:.6} {}",
        audit.set_layout_se,
        audit.set_margin,
        audit.null_geomean,
        if audit.set_margin >= ATTRIBUTION_MARGIN
            && audit.null_geomean.ln().abs() <= (1.0 + SET_TOLERANCE).ln()
        {
            "PASS"
        } else {
            "FAIL"
        }
    );
    match &audit.coverage {
        Some(coverage) => println!(
            "coverage ensemble_rms={:.6} natural_rms={:.6} {}",
            coverage.ensemble_rms,
            coverage.natural_rms,
            if coverage.passed { "PASS" } else { "FAIL" }
        ),
        None => println!("coverage unmeasured (no natural pair supplied)"),
    }
    println!(
        "decorrelation paired_rms={:.6} reference_rms={:.6} {}",
        audit.decorrelation.paired_rms,
        audit.decorrelation.reference_rms,
        if audit.decorrelation.passed {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!("RESULT: {}", if audit.passed { "PASS" } else { "FAIL" });
}

/// Prints the pinned set as the committed procedure tabulates it, so the
/// document and this harness can be checked against each other.
fn list_cells() {
    println!("cell\tfamily\tarm\tsize_a\tsize_b");
    for cell in pinned_cells() {
        let (size_a, size_b) = cell_sizes(cell);
        println!(
            "{}\t{}\t{}\t{size_a}\t{size_b}",
            cell_id(cell),
            family(cell),
            expected_arm(cell)
        );
    }
}

/// Reports two receipts that cannot be compared at all.
///
/// No per-cell verdict and no geometric mean is printed, because neither is
/// meaningful once the receipts are known to describe different measurements.
fn print_incomparable(baseline: &Path, candidate: &Path, reason: &Incomparable) {
    println!(
        "comparison: baseline={} candidate={}",
        baseline.display(),
        candidate.display()
    );
    println!("{reason}");
    println!("RESULT: FAIL ({})", reason.class());
}

fn self_check() -> Result<(), Box<dyn std::error::Error>> {
    let cells = pinned_cells();
    let mut ids: Vec<_> = cells.iter().copied().map(cell_id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), cells.len(), "pinned cell ids are not unique");
    assert_eq!(cells.len(), 34);
    let simd_min = observed_simd_min_words();
    assert_ne!(simd_min, usize::MAX, "SIMD arm is not observable");
    assert!(cells
        .iter()
        .any(|cell| { matches!(cell, Cell::BitLogical { words: 1, .. }) && 1 < simd_min }));
    assert!(cells.iter().any(|cell| {
        matches!(cell, Cell::BitLogical { .. }) && expected_arm(*cell) == "scalar"
    }));
    assert!(cells
        .iter()
        .any(|cell| { matches!(cell, Cell::BitLogical { .. }) && expected_arm(*cell) == "simd" }));
    for (conservative, asymptotic) in [
        ("schoolbook", "karatsuba"),
        ("mul_dispatch", "ntt"),
        ("schoolbook", "newton"),
        ("horner", "subproduct"),
    ] {
        assert!(cells.iter().any(|cell| expected_arm(*cell) == conservative));
        assert!(cells.iter().any(|cell| expected_arm(*cell) == asymptotic));
    }
    for auto in [false, true] {
        let eval_cells: Vec<_> = cells
            .iter()
            .filter(|cell| matches!(cell, Cell::PolyBatchEval { auto: a, .. } if *a == auto))
            .copied()
            .collect();
        assert!(eval_cells
            .iter()
            .any(|cell| expected_arm(*cell) == "horner"));
        assert!(eval_cells
            .iter()
            .any(|cell| expected_arm(*cell) == "subproduct"));
    }
    assert!(BIT_FIXTURES.is_power_of_two());
    let starts: Vec<_> = (1..=5)
        .flat_map(|execution| {
            (1..=5).map(move |repetition| recorded_fixture_start(execution, repetition, 5))
        })
        .collect();
    let mut unique = starts.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), BIT_FIXTURES);
    assert_eq!(unique, (0..BIT_FIXTURES).collect::<Vec<_>>());
    println!(
        "protocol: schema={SCHEMA_VERSION} cells={} repetitions={} target_ms={} per_cell_tolerance={PER_CELL_TOLERANCE:.6} set_tolerance={SET_TOLERANCE:.6} simd_min_words={simd_min}",
        cells.len(), DEFAULT_REPETITIONS, DEFAULT_TARGET_MS
    );
    println!("self-check PASS");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // `cargo bench` compiles this module with `cfg(test)` but without a test
    // harness, so nothing calls the `#[test]` functions and their helpers read
    // as dead code in that build.
    #[allow(dead_code)]
    fn receipt_with_ratio(ratio: f64) -> Receipt {
        let stats = pinned_cells()
            .into_iter()
            .map(|cell| {
                let id = cell_id(cell);
                let (size_a, size_b) = cell_sizes(cell);
                (
                    id.clone(),
                    CellStat {
                        cell: id,
                        family: family(cell).to_owned(),
                        arm: expected_arm(cell).to_owned(),
                        size_a: size_a.to_string(),
                        size_b: size_b.to_string(),
                        calls: 1_000,
                        elapsed_ns: (1_000_000.0 * ratio).round() as u128,
                    },
                )
            })
            .collect();
        Receipt {
            schema_version: SCHEMA_VERSION.to_owned(),
            stats,
        }
    }

    #[test]
    fn pinned_set_has_unique_cell_ids() {
        let mut ids: Vec<_> = pinned_cells().into_iter().map(cell_id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(count, 34);
        assert_eq!(ids.len(), count);
    }

    /// Arms present among the pinned cells that `keep` selects.
    /// One row per pinned cell, as a receipt this harness could have written.
    /// `arm_override` replaces the recorded arm of the named cell.
    #[allow(dead_code)]
    fn pinned_receipt_csv(arm_override: Option<(&str, &str)>) -> String {
        let mut text = String::from(CSV_HEADER);
        text.push('\n');
        for cell in pinned_cells() {
            let id = cell_id(cell);
            let (size_a, size_b) = cell_sizes(cell);
            let arm = match arm_override {
                Some((target, replacement)) if target == id => replacement,
                _ => expected_arm(cell),
            };
            text.push_str(&format!(
                "{SCHEMA_VERSION},1,1,{},{id},{arm},{size_a},{size_b},{},0,1000,1000000,1000.0,250,0,rev,false,rustc,host,cpu,kernel,gov\n",
                family(cell),
                fixture_count(cell)
            ));
        }
        text
    }

    #[allow(dead_code)]
    fn arms_of(keep: impl Fn(&Cell) -> bool) -> Vec<&'static str> {
        pinned_cells()
            .into_iter()
            .filter(|cell| keep(cell))
            .map(expected_arm)
            .collect()
    }

    /// One threshold's straddle expectation: which cells belong to it, and the
    /// conservative and asymptotic arm names the pinned set must contain.
    #[allow(dead_code)]
    type StraddleCase = (fn(&Cell) -> bool, &'static str, &'static str);

    #[test]
    fn pinned_set_straddles_every_polynomial_default() {
        let cases: [StraddleCase; 4] = [
            (
                |cell| matches!(cell, Cell::PolyMul { .. }),
                "schoolbook",
                "karatsuba",
            ),
            (
                |cell| matches!(cell, Cell::PolyMulFast { .. }),
                "mul_dispatch",
                "ntt",
            ),
            (
                |cell| matches!(cell, Cell::PolyDivRem { .. }),
                "schoolbook",
                "newton",
            ),
            (
                |cell| matches!(cell, Cell::PolyBatchEval { .. }),
                "horner",
                "subproduct",
            ),
        ];
        for (keep, conservative, asymptotic) in cases {
            let arms = arms_of(keep);
            assert!(arms.contains(&conservative), "missing {conservative}");
            assert!(arms.contains(&asymptotic), "missing {asymptotic}");
        }
    }

    /// The pinned sizes bracket each default as tightly as the guard allows, so
    /// moving a default leaves the set no longer straddling it and this test
    /// reports that the set needs re-pinning.
    #[test]
    fn pinned_sizes_bracket_each_default_at_adjacent_guard_values() {
        let mul_degrees: Vec<usize> = pinned_cells()
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::PolyMul { len } => Some(len - 1),
                _ => None,
            })
            .collect();
        assert!(mul_degrees.contains(&(KARATSUBA_THRESHOLD - 1)));
        assert!(mul_degrees.contains(&KARATSUBA_THRESHOLD));

        let out_lens: Vec<usize> = pinned_cells()
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::PolyMulFast { len } => Some(2 * len - 1),
                _ => None,
            })
            .collect();
        // `out_len` is always odd for equal-length operands, so the tightest
        // bracket around an even threshold is `T - 1` and `T + 1`.
        assert!(out_lens.contains(&(NTT_THRESHOLD - 1)));
        assert!(out_lens.contains(&(NTT_THRESHOLD + 1)));

        let divisors: Vec<usize> = pinned_cells()
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::PolyDivRem { divisor, .. } => Some(divisor),
                _ => None,
            })
            .collect();
        assert!(divisors.contains(&(DIV_REM_THRESHOLD - 1)));
        assert!(divisors.contains(&DIV_REM_THRESHOLD));

        for auto in [false, true] {
            let sizes: Vec<(usize, usize)> = pinned_cells()
                .into_iter()
                .filter_map(|cell| match cell {
                    Cell::PolyBatchEval {
                        auto: a,
                        coeffs,
                        points,
                    } if a == auto => Some((coeffs, points)),
                    _ => None,
                })
                .collect();
            let below = SUBPRODUCT_THRESHOLD - 1;
            assert!(sizes.contains(&(below, below)));
            assert!(sizes.contains(&(SUBPRODUCT_THRESHOLD, SUBPRODUCT_THRESHOLD)));
        }

        let words: Vec<usize> = pinned_cells()
            .into_iter()
            .filter_map(|cell| match cell {
                Cell::BitLogical { words, .. } => Some(words),
                _ => None,
            })
            .collect();
        let simd_min = observed_simd_min_words();
        assert!(words.contains(&(simd_min - 1)));
        assert!(words.contains(&simd_min));
    }

    #[test]
    fn subproduct_threshold_is_straddled_at_both_entry_points() {
        for auto in [false, true] {
            let cells: Vec<_> = pinned_cells()
                .into_iter()
                .filter(|cell| matches!(cell, Cell::PolyBatchEval { auto: a, .. } if *a == auto))
                .collect();
            assert!(cells.iter().any(|cell| expected_arm(*cell) == "horner"));
            assert!(cells.iter().any(|cell| expected_arm(*cell) == "subproduct"));
        }
    }

    #[test]
    fn pinned_set_contains_small_buffer_bit_logical_cell() {
        let simd_min = observed_simd_min_words();
        assert!(pinned_cells()
            .into_iter()
            .any(|cell| { matches!(cell, Cell::BitLogical { words: 1, .. }) && 1 < simd_min }));
    }

    #[test]
    fn bit_backend_cells_cover_both_arms() {
        let arms: Vec<_> = pinned_cells()
            .into_iter()
            .filter(|cell| matches!(cell, Cell::BitLogical { .. }))
            .map(expected_arm)
            .collect();
        assert!(arms.contains(&"scalar"));
        assert!(arms.contains(&"simd"));
    }

    #[test]
    fn cell_ids_use_the_canonical_format() {
        assert_eq!(
            cell_id(Cell::BitLogical {
                op: BitOp::Xor,
                words: 1
            }),
            "bit_backend/xor_inplace/words=1"
        );
        assert_eq!(
            cell_id(Cell::BitLogical {
                op: BitOp::And,
                words: 8
            }),
            "bit_backend/and_inplace/words=8"
        );
        assert_eq!(
            cell_id(Cell::BitLogical {
                op: BitOp::Or,
                words: 8
            }),
            "bit_backend/or_inplace/words=8"
        );
        assert_eq!(
            cell_id(Cell::BitLogical {
                op: BitOp::Not,
                words: 8
            }),
            "bit_backend/not_inplace/words=8"
        );
        assert_eq!(
            cell_id(Cell::BitLogical {
                op: BitOp::Popcount,
                words: 8
            }),
            "bit_backend/popcount/words=8"
        );
        assert_eq!(cell_id(Cell::PolyMul { len: 32 }), "polynomial/mul/len=32");
        assert_eq!(
            cell_id(Cell::PolyMulFast { len: 64 }),
            "polynomial/mul_fast/len=64"
        );
        assert_eq!(
            cell_id(Cell::PolyDivRem {
                dividend: 4096,
                divisor: 2048
            }),
            "polynomial/div_rem_auto/dividend=4096/divisor=2048"
        );
        assert_eq!(
            cell_id(Cell::PolyBatchEval {
                auto: false,
                coeffs: 4096,
                points: 4096
            }),
            "polynomial/batch_evaluate/coeffs=4096/points=4096"
        );
        assert_eq!(
            cell_id(Cell::PolyBatchEval {
                auto: true,
                coeffs: 4096,
                points: 4096
            }),
            "polynomial/batch_evaluate_auto/coeffs=4096/points=4096"
        );
    }

    /// The bit-backend arm recorded in a receipt is the one the library itself
    /// selects, not a prediction the harness computes from a size literal.
    #[test]
    fn bit_arm_is_read_from_the_library_selector() {
        for cell in pinned_cells() {
            let Cell::BitLogical { words, .. } = cell else {
                continue;
            };
            assert_eq!(
                expected_arm(cell),
                gf2_core::kernels::select_backend_for_size(words).name(),
                "{}",
                cell_id(cell)
            );
        }
    }

    /// Each polynomial arm follows its own public threshold, in the direction
    /// the guard in `poly.rs` uses.
    #[test]
    fn polynomial_arms_follow_their_public_thresholds() {
        for cell in pinned_cells() {
            let arm = expected_arm(cell);
            match cell {
                Cell::PolyMul { len } => {
                    assert_eq!(arm == "schoolbook", len - 1 < KARATSUBA_THRESHOLD);
                }
                Cell::PolyMulFast { len } => {
                    assert_eq!(arm == "mul_dispatch", 2 * len - 1 <= NTT_THRESHOLD);
                }
                Cell::PolyDivRem { dividend, divisor } => {
                    assert_eq!(
                        arm == "schoolbook",
                        dividend < DIV_REM_THRESHOLD || divisor < DIV_REM_THRESHOLD
                    );
                }
                Cell::PolyBatchEval { coeffs, points, .. } => {
                    assert_eq!(
                        arm == "horner",
                        points < SUBPRODUCT_THRESHOLD || coeffs < SUBPRODUCT_THRESHOLD
                    );
                }
                Cell::BitLogical { .. } => {}
            }
        }
    }

    #[test]
    fn parser_accepts_recorded_command_shape() {
        let args = parse_args(
            [
                "--execution",
                "3",
                "--repetitions",
                "5",
                "--target-ms",
                "250",
                "--output",
                "selector.csv",
                "--append",
                "--compare",
                "baseline.csv",
                "--against",
                "candidate.csv",
                "--bench",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("recorded command arguments parse");
        assert_eq!(args.execution, 3);
        assert_eq!(args.repetitions, 5);
        assert_eq!(args.target, Duration::from_millis(250));
        assert!(args.append);
        assert_eq!(args.compare, Some(PathBuf::from("baseline.csv")));
        assert_eq!(args.against, Some(PathBuf::from("candidate.csv")));
    }

    #[test]
    fn parser_rejects_zero_recording_dimensions() {
        assert!(parse_args(["--execution", "0"].into_iter().map(str::to_owned)).is_err());
        assert!(parse_args(["--repetitions", "0"].into_iter().map(str::to_owned)).is_err());
        assert!(parse_args(["--target-ms", "0"].into_iter().map(str::to_owned)).is_err());
    }

    #[test]
    fn parser_rejects_compare_without_against() {
        assert!(parse_args(["--compare", "baseline.csv"].into_iter().map(str::to_owned)).is_err());
        assert!(parse_args(
            ["--against", "candidate.csv"]
                .into_iter()
                .map(str::to_owned)
        )
        .is_err());
    }

    #[test]
    fn poly_fixtures_have_intended_lengths() {
        for cell in pinned_cells() {
            match cell {
                Cell::PolyMul { len } | Cell::PolyMulFast { len } => {
                    let Fixture::Mul { a, b } = build_fixture(cell) else {
                        unreachable!()
                    };
                    assert_eq!(a.len(), len);
                    assert_eq!(b.len(), len);
                }
                Cell::PolyDivRem { dividend, divisor } => {
                    let Fixture::DivRem {
                        dividend: a,
                        divisor: b,
                    } = build_fixture(cell)
                    else {
                        unreachable!()
                    };
                    assert_eq!(a.len(), dividend);
                    assert_eq!(b.len(), divisor);
                }
                Cell::PolyBatchEval { coeffs, points, .. } => {
                    let Fixture::BatchEval { poly, points: xs } = build_fixture(cell) else {
                        unreachable!()
                    };
                    assert_eq!(poly.len(), coeffs);
                    assert_eq!(xs.len(), points);
                }
                Cell::BitLogical { .. } => {}
            }
        }
    }

    #[test]
    fn pooling_sums_totals_rather_than_averaging_rows() {
        let stat = CellStat {
            cell: "cell".into(),
            family: "family".into(),
            arm: "arm".into(),
            size_a: "1".into(),
            size_b: "0".into(),
            calls: 1 + 3,
            elapsed_ns: 10 + 90,
        };
        assert_eq!(pooled_ns_per_call(&stat), 25.0);
    }

    #[test]
    fn comparison_accepts_candidate_within_both_tolerances() {
        let comparison = compare(&receipt_with_ratio(1.0), &receipt_with_ratio(1.015)).unwrap();
        assert!(comparison.cells.iter().all(|cell| cell.passed));
        assert!(comparison.geomean <= 1.0 + SET_TOLERANCE);
        assert!(comparison.passed);
    }

    /// A single cell over the per-cell tolerance fails the whole set even when
    /// the geometric mean stays comfortably inside the set tolerance.
    #[test]
    fn comparison_flags_one_cell_above_per_cell_tolerance() {
        let baseline = receipt_with_ratio(1.0);
        let mut candidate = receipt_with_ratio(1.0);
        let worst = cell_id(Cell::BitLogical {
            op: BitOp::Xor,
            words: 1,
        });
        candidate
            .stats
            .get_mut(&worst)
            .expect("small-buffer cell is pinned")
            .elapsed_ns = 1_100_000;
        let comparison = compare(&baseline, &candidate).unwrap();
        assert!(comparison.geomean <= 1.0 + SET_TOLERANCE);
        assert_eq!(
            comparison
                .cells
                .iter()
                .filter(|cell| !cell.passed)
                .map(|cell| cell.cell.as_str())
                .collect::<Vec<_>>(),
            vec![worst.as_str()]
        );
        assert!(!comparison.passed);
    }

    #[test]
    fn comparison_flags_uniform_drift_above_set_tolerance() {
        let comparison = compare(&receipt_with_ratio(1.0), &receipt_with_ratio(1.03)).unwrap();
        assert!(!comparison.passed);
        assert!(comparison.cells.iter().all(|cell| cell.passed));
        assert!(comparison.geomean > 1.0 + SET_TOLERANCE);
    }

    #[test]
    fn comparison_rejects_mismatched_cell_sets() {
        let baseline = receipt_with_ratio(1.0);
        let mut candidate = receipt_with_ratio(1.0);
        candidate.stats.pop_first();
        assert!(matches!(
            compare(&baseline, &candidate).unwrap_err(),
            Incomparable::CellSet { .. }
        ));
    }

    #[test]
    fn comparison_rejects_mismatched_schema_version() {
        let baseline = receipt_with_ratio(1.0);
        let mut candidate = receipt_with_ratio(1.0);
        candidate.schema_version = "other-schema".into();
        assert!(matches!(
            compare(&baseline, &candidate).unwrap_err(),
            Incomparable::Schema { .. }
        ));
    }

    /// `load_receipt` pools every recorded window of a cell by summing totals,
    /// which is what makes a five-execution receipt one number per cell.
    #[test]
    fn load_receipt_pools_every_window_of_a_cell() {
        let cell = "bit_backend/xor_inplace/words=1";
        let rows = format!(
            "{CSV_HEADER}\n\
             {SCHEMA_VERSION},1,1,bit_backend,{cell},scalar,1,0,8,0,1000,10000,10.000000,250,0,rev,false,rustc,host,cpu,kernel,gov\n\
             {SCHEMA_VERSION},1,2,bit_backend,{cell},scalar,1,0,8,1,3000,90000,30.000000,250,0,rev,false,rustc,host,cpu,kernel,gov\n"
        );
        let path = std::env::temp_dir().join(format!(
            "gf2-{SCHEMA_VERSION}-pooling-{}.csv",
            std::process::id()
        ));
        fs::write(&path, rows).expect("write fixture receipt");
        let receipt = load_receipt(&path).expect("fixture receipt loads");
        fs::remove_file(&path).expect("remove fixture receipt");

        assert_eq!(receipt.schema_version, SCHEMA_VERSION);
        let stat = &receipt.stats[cell];
        assert_eq!(stat.calls, 4_000);
        assert_eq!(stat.elapsed_ns, 100_000);
        // 100000 / 4000, not the mean of 10.0 and 30.0.
        assert_eq!(pooled_ns_per_call(stat), 25.0);
    }

    #[test]
    fn repository_relative_paths_resolve_from_the_repository_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert_eq!(
            resolve_repository_path(Path::new("dev/benchmarks/tuning_profiles/receipt.csv")),
            root.join("dev/benchmarks/tuning_profiles/receipt.csv")
        );
        let absolute = Path::new("/tmp/gf2-selector-non-regression-absolute.csv");
        assert_eq!(resolve_repository_path(absolute), absolute);
    }

    /// A receipt named relative to the repository root loads, which is the form
    /// the committed procedure uses. `cargo bench` runs this binary with its
    /// working directory at the package root, so the path resolves only because
    /// `load_receipt` goes through the resolver.
    #[test]
    fn load_receipt_accepts_a_repository_relative_path() {
        let relative = PathBuf::from(format!(
            "target/gf2-{SCHEMA_VERSION}-relative-{}.csv",
            std::process::id()
        ));
        let absolute = resolve_repository_path(&relative);
        assert!(absolute.is_absolute());
        fs::create_dir_all(absolute.parent().expect("the path has a parent"))
            .expect("create the target directory");
        fs::write(&absolute, pinned_receipt_csv(None)).expect("write the fixture receipt");

        // Discriminating precondition: the same relative path does not resolve
        // against the working directory, so a pass below is the resolver's work.
        assert!(
            fs::read_to_string(&relative).is_err(),
            "the fixture must not be reachable from the working directory"
        );

        let receipt = load_receipt(&relative).expect("repository-relative receipt loads");
        fs::remove_file(&absolute).expect("remove the fixture receipt");
        assert_eq!(receipt.stats.len(), pinned_cells().len());
    }

    /// Two receipts that agree on every timing still fail when a cell resolves
    /// to a different arm, because the ratio would then compare two different
    /// code paths rather than measure a regression.
    #[test]
    fn comparison_rejects_a_cell_whose_selector_arm_changed() {
        let baseline = receipt_with_ratio(1.0);
        let mut candidate = receipt_with_ratio(1.0);
        let flipped = cell_id(Cell::BitLogical {
            op: BitOp::Xor,
            words: 8,
        });
        // Identical timings: without the identity precondition this comparison
        // reports every ratio as 1.0 and passes.
        assert!(compare(&baseline, &candidate).unwrap().passed);
        "scalar".clone_into(&mut candidate.stats.get_mut(&flipped).expect("pinned cell").arm);

        let error = compare(&baseline, &candidate).unwrap_err();
        assert_eq!(error.class(), "selector identity mismatch");
        let Incomparable::Identity { cells } = &error else {
            panic!("expected an identity mismatch, got {error:?}");
        };
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].cell, flipped);
        assert_eq!(cells[0].field, "arm");
        assert_eq!(cells[0].baseline, "simd");
        assert_eq!(cells[0].candidate, "scalar");
    }

    /// The same rejection reached through the CSV, proving the receipt's `arm`
    /// column is what the identity precondition reads.
    #[test]
    fn arm_column_of_a_receipt_reaches_the_identity_check() {
        let dir = std::env::temp_dir();
        let stamp = std::process::id();
        let baseline_path = dir.join(format!("gf2-{SCHEMA_VERSION}-arm-base-{stamp}.csv"));
        let candidate_path = dir.join(format!("gf2-{SCHEMA_VERSION}-arm-cand-{stamp}.csv"));
        let flipped = cell_id(Cell::PolyMul { len: 33 });
        fs::write(&baseline_path, pinned_receipt_csv(None)).expect("write baseline");
        fs::write(
            &candidate_path,
            pinned_receipt_csv(Some((flipped.as_str(), "schoolbook"))),
        )
        .expect("write candidate");

        let baseline = load_receipt(&baseline_path).expect("baseline loads");
        let candidate = load_receipt(&candidate_path).expect("candidate loads");
        fs::remove_file(&baseline_path).expect("remove baseline");
        fs::remove_file(&candidate_path).expect("remove candidate");

        let error = compare(&baseline, &candidate).unwrap_err();
        assert_eq!(error.class(), "selector identity mismatch");
        let rendered = error.to_string();
        assert!(rendered.contains(&flipped), "{rendered}");
        assert!(rendered.contains("baseline=karatsuba"), "{rendered}");
        assert!(rendered.contains("candidate=schoolbook"), "{rendered}");
    }

    #[test]
    fn comparison_rejects_a_receipt_whose_cell_set_is_not_the_pinned_set() {
        let mut baseline = receipt_with_ratio(1.0);
        let mut candidate = receipt_with_ratio(1.0);
        let extra = "polynomial/mul/len=99".to_owned();
        for receipt in [&mut baseline, &mut candidate] {
            receipt.stats.insert(
                extra.clone(),
                CellStat {
                    cell: extra.clone(),
                    family: "polynomial".into(),
                    arm: "karatsuba".into(),
                    size_a: "99".into(),
                    size_b: "0".into(),
                    calls: 1_000,
                    elapsed_ns: 1_000_000,
                },
            );
        }
        assert_eq!(
            compare(&baseline, &candidate).unwrap_err(),
            Incomparable::NotPinnedSet
        );
    }

    #[test]
    fn full_repository_status_marks_tracked_and_untracked_changes_dirty() {
        assert_eq!(
            GIT_STATUS_ARGS,
            ["status", "--porcelain", "--untracked-files=all"]
        );
        assert!(source_dirty_from_porcelain(" M crates/gf2-core/src/lib.rs"));
        assert!(source_dirty_from_porcelain("?? scratch-notes.txt"));
        assert!(!source_dirty_from_porcelain(""));
    }

    /// Every bit-logical fixture buffer starts on a cache-line boundary, so a
    /// receipt does not sample the allocator's per-process address phase.
    #[test]
    fn bit_fixture_banks_are_cache_line_aligned() {
        for cell in pinned_cells() {
            let Cell::BitLogical { words, .. } = cell else {
                continue;
            };
            let Fixture::Bit { dst, src } = build_fixture(cell) else {
                unreachable!()
            };
            for bank in [&dst, &src] {
                assert!(bank.is_line_aligned(), "{}", cell_id(cell));
                for index in 0..BIT_FIXTURES {
                    assert_eq!(bank.get(index).len(), words);
                    let address = bank.get(index).as_ptr() as usize;
                    assert!(
                        address.is_multiple_of(WORDS_PER_LINE * 8),
                        "{}",
                        cell_id(cell)
                    );
                }
            }
        }
    }

    /// Distinct buffers never overlap, so an in-place write to one does not
    /// change another's contents between calls.
    #[test]
    fn bit_fixture_buffers_are_disjoint() {
        let bank = BitBank::new(7, |bank, index| (bank * 100 + index) as u64);
        for a in 0..BIT_FIXTURES {
            for b in 0..BIT_FIXTURES {
                if a == b {
                    continue;
                }
                let (first, second) =
                    (bank.get(a).as_ptr() as usize, bank.get(b).as_ptr() as usize);
                assert!(first.abs_diff(second) >= 7 * 8);
            }
            assert_eq!(bank.get(a)[0], (a * 100) as u64);
            assert_eq!(bank.get(a)[6], (a * 100 + 6) as u64);
        }
    }

    #[test]
    fn recorded_fixture_start_covers_the_whole_bank() {
        let starts: Vec<_> = (1..=5)
            .flat_map(|execution| {
                (1..=5).map(move |repetition| recorded_fixture_start(execution, repetition, 5))
            })
            .collect();
        let mut unique = starts.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), BIT_FIXTURES);
        assert_eq!(unique, (0..BIT_FIXTURES).collect::<Vec<_>>());
    }

    /// One ensemble arm as this harness would have written it: one row per
    /// pinned cell per build, every build under one execution index, with
    /// `rate` giving the build's ns/call for a cell.
    #[allow(dead_code)]
    fn ensemble_csv(
        builds: &[u32],
        revision: &str,
        dirty: bool,
        rate: impl Fn(u32, &str) -> f64,
    ) -> String {
        const CALLS: u64 = 1_000_000;
        let mut text = String::from(CSV_HEADER);
        text.push('\n');
        for build in builds {
            for cell in pinned_cells() {
                let id = cell_id(cell);
                let (size_a, size_b) = cell_sizes(cell);
                let ns_per_call = rate(*build, &id);
                let elapsed = (ns_per_call * CALLS as f64).round() as u128;
                text.push_str(&format!(
                    "{SCHEMA_VERSION},{build},1,{},{id},{},{size_a},{size_b},{},0,{CALLS},{elapsed},{ns_per_call:.6},250,0,{revision},{dirty},rustc,host,cpu,kernel,gov\n",
                    family(cell),
                    expected_arm(cell),
                    fixture_count(cell)
                ));
            }
        }
        text
    }

    /// A four-build ensemble whose per-build displacement does not line up with
    /// the parity of the build index, so its halves agree while its builds do
    /// not.
    #[allow(dead_code)]
    fn balanced_arm(revision: &str, tilt: f64) -> EnsembleArm {
        let text = ensemble_csv(&[1, 2, 3, 4], revision, false, |build, _| {
            let sign = if build == 1 || build == 4 { 1.0 } else { -1.0 };
            1_000.0 * (1.0 + sign * tilt)
        });
        parse_ensemble(&text)
    }

    #[allow(dead_code)]
    fn parse_ensemble(text: &str) -> EnsembleArm {
        let path = std::env::temp_dir().join(format!(
            "gf2-selector-ensemble-{}-{:?}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after the epoch")
                .as_nanos()
        ));
        fs::write(&path, text).expect("temporary ensemble is writable");
        let arm = load_ensemble(&path);
        fs::remove_file(&path).expect("temporary ensemble is removable");
        arm.expect("ensemble parses")
    }

    #[test]
    fn sample_sd_uses_the_unbiased_denominator() {
        assert_eq!(sample_sd(&[1.0]), 0.0);
        assert!((sample_sd(&[1.0, 3.0]) - f64::sqrt(2.0)).abs() < 1e-12);
    }

    /// A build occupies one execution index, so an arm's rows separate into one
    /// pooled statistic per build rather than one per arm.
    #[test]
    fn load_ensemble_pools_each_build_separately() {
        let arm = balanced_arm("rev", 0.01);
        assert_eq!(arm.builds.len(), 4);
        let cell = cell_id(Cell::BitLogical {
            op: BitOp::Popcount,
            words: 1,
        });
        assert!((arm.pooled(&cell, |index| index == 1) - 1_010.0).abs() < 1e-6);
        assert!((arm.pooled(&cell, |index| index == 2) - 990.0).abs() < 1e-6);
        // Pooling both builds sums totals rather than averaging their rates.
        assert!((arm.pooled(&cell, |index| index <= 2) - 1_000.0).abs() < 1e-6);
    }

    /// The half-split null divides the arm by the parity of the build index,
    /// which is the balanced split of the predeclared enumeration.
    #[test]
    fn layout_audit_splits_the_reference_arm_by_build_parity() {
        let reference = balanced_arm("rev", 0.01);
        let candidate = balanced_arm("rev2", 0.01);
        let audit = audit_layout(&reference, &candidate, None).expect("arms are auditable");
        for cell in &audit.cells {
            assert!((cell.null_ratio - 1.0).abs() < 1e-9, "{}", cell.cell);
            assert!((cell.reference_spread - 1_010.0 / 990.0).abs() < 1e-9);
        }
        assert!((audit.null_geomean - 1.0).abs() < 1e-9);
    }

    /// A cell whose builds scatter widely leaves the verdict statistic too
    /// little room under the tolerance to attribute movement to behaviour.
    #[test]
    fn layout_audit_flags_a_cell_without_an_attribution_margin() {
        let noisy = cell_id(Cell::BitLogical {
            op: BitOp::Or,
            words: 1,
        });
        let arm = |revision: &str| {
            let text = ensemble_csv(&[1, 2, 3, 4], revision, false, |build, cell| {
                let sign = if build == 1 || build == 4 { 1.0 } else { -1.0 };
                let tilt = if cell == noisy { 0.15 } else { 0.0001 };
                1_000.0 * (1.0 + sign * tilt)
            });
            parse_ensemble(&text)
        };
        let audit = audit_layout(&arm("rev"), &arm("rev2"), None).expect("arms are auditable");
        let failing: Vec<_> = audit
            .cells
            .iter()
            .filter(|cell| !cell.passed)
            .map(|cell| cell.cell.as_str())
            .collect();
        assert_eq!(failing, vec![noisy.as_str()]);
        let flagged = audit
            .cells
            .iter()
            .find(|cell| cell.cell == noisy)
            .expect("the noisy cell is pinned");
        assert!(flagged.margin < ATTRIBUTION_MARGIN);
        assert!(!audit.passed);
    }

    /// An ensemble whose arms move together carries one fixed layout difference
    /// rather than a sampled one, and no averaging over its builds removes it.
    #[test]
    fn layout_audit_flags_arms_that_do_not_scatter_against_each_other() {
        let reference = balanced_arm("rev", 0.001);
        let text = ensemble_csv(&[1, 2, 3, 4], "rev2", false, |build, _| {
            let sign = if build == 1 || build == 4 { 1.0 } else { -1.0 };
            1_060.0 * (1.0 + sign * 0.001)
        });
        let candidate = parse_ensemble(&text);
        let audit = audit_layout(&reference, &candidate, None).expect("arms are auditable");
        assert!(audit.decorrelation.paired_rms < 1e-12);
        assert!(!audit.decorrelation.passed);
        assert!(!audit.passed);
    }

    /// An ensemble that moves a cell less than one natural rebuild moved it
    /// understates the variance the verdict has to survive.
    #[test]
    fn layout_audit_flags_an_ensemble_tamer_than_a_natural_rebuild() {
        let reference = balanced_arm("rev", 0.0005);
        let candidate = balanced_arm("rev2", 0.0007);
        let baseline = receipt_with_ratio(1.0);
        let rebuild = receipt_with_ratio(1.2);
        let audit = audit_layout(&reference, &candidate, Some((&baseline, &rebuild)))
            .expect("arms are auditable");
        let coverage = audit
            .coverage
            .as_ref()
            .expect("a natural pair was supplied");
        assert!(coverage.natural_rms > coverage.ensemble_rms);
        assert!(!coverage.passed);
        assert!(audit.cells.iter().all(|cell| !cell.passed));
        assert!(!audit.passed);
    }

    /// With the natural pair recording no movement, coverage is satisfied by
    /// any ensemble and the remaining preconditions decide.
    #[test]
    fn layout_audit_passes_a_well_separated_ensemble() {
        let reference = balanced_arm("rev", 0.0005);
        let text = ensemble_csv(&[1, 2, 3, 4], "rev2", false, |build, _| {
            let sign = if build <= 2 { 1.0 } else { -1.0 };
            1_000.0 * (1.0 + sign * 0.0005)
        });
        let candidate = parse_ensemble(&text);
        let baseline = receipt_with_ratio(1.0);
        let rebuild = receipt_with_ratio(1.0);
        let audit = audit_layout(&reference, &candidate, Some((&baseline, &rebuild)))
            .expect("arms are auditable");
        assert!(audit.cells.iter().all(|cell| cell.passed));
        assert!(audit.set_margin >= ATTRIBUTION_MARGIN);
        assert!(audit.decorrelation.passed);
        assert!(
            audit
                .coverage
                .as_ref()
                .expect("a natural pair was supplied")
                .passed
        );
        assert!(audit.passed);
    }

    #[test]
    fn layout_audit_rejects_an_unbalanced_build_count() {
        let text = ensemble_csv(&[1, 2, 3], "rev", false, |_, _| 1_000.0);
        let arm = parse_ensemble(&text);
        assert_eq!(
            audit_layout(&arm, &arm, None).unwrap_err().class(),
            "unusable build count"
        );
    }

    #[test]
    fn layout_audit_rejects_a_dirty_arm() {
        let text = ensemble_csv(&[1, 2, 3, 4], "rev", true, |_, _| 1_000.0);
        let arm = parse_ensemble(&text);
        assert_eq!(
            audit_layout(&arm, &arm, None).unwrap_err().class(),
            "dirty source row"
        );
    }

    #[test]
    fn layout_audit_rejects_an_arm_spanning_two_revisions() {
        let mut text = ensemble_csv(&[1, 2], "rev", false, |_, _| 1_000.0);
        let second = ensemble_csv(&[3, 4], "other", false, |_, _| 1_000.0);
        text.push_str(second.split_once('\n').expect("header is present").1);
        let arm = parse_ensemble(&text);
        assert_eq!(
            audit_layout(&arm, &arm, None).unwrap_err().class(),
            "arm spans several revisions"
        );
    }

    #[test]
    fn layout_audit_rejects_arms_built_from_different_ensembles() {
        let reference = balanced_arm("rev", 0.001);
        let text = ensemble_csv(&[1, 2, 3, 4, 5, 6], "rev2", false, |_, _| 1_000.0);
        let candidate = parse_ensemble(&text);
        assert_eq!(
            audit_layout(&reference, &candidate, None)
                .unwrap_err()
                .class(),
            "arms carry different builds"
        );
    }

    #[test]
    fn layout_audit_rejects_a_natural_pair_outside_the_pinned_set() {
        let reference = balanced_arm("rev", 0.001);
        let candidate = balanced_arm("rev2", 0.001);
        let mut baseline = receipt_with_ratio(1.0);
        baseline.stats.pop_first();
        let rebuild = receipt_with_ratio(1.0);
        assert_eq!(
            audit_layout(&reference, &candidate, Some((&baseline, &rebuild)))
                .unwrap_err()
                .class(),
            "natural pair is not the pinned set"
        );
    }

    #[test]
    fn parser_accepts_the_layout_audit_pair() {
        let args = parse_args(
            [
                "--layout-audit",
                "reference.csv",
                "--layout-candidate",
                "candidate.csv",
                "--compare",
                "baseline.csv",
                "--against",
                "rebuild.csv",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("the recorded audit command parses");
        assert_eq!(
            args.layout_audit.as_deref(),
            Some(Path::new("reference.csv"))
        );
        assert_eq!(
            args.layout_candidate.as_deref(),
            Some(Path::new("candidate.csv"))
        );
        assert_eq!(args.compare.as_deref(), Some(Path::new("baseline.csv")));
    }

    #[test]
    fn parser_rejects_a_layout_audit_without_its_candidate_arm() {
        assert!(parse_args(
            ["--layout-audit", "reference.csv"]
                .into_iter()
                .map(str::to_owned)
        )
        .is_err());
    }
}
