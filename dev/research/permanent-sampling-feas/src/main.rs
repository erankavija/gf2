//! Driver for the permanent-campaign feasibility measurements (JIT `b488f02c`).
//!
//! ```text
//! permanent_sampling_feas equivalence --out PATH
//! permanent_sampling_feas grid        --out PATH [--only q=3,n=28,...]
//! permanent_sampling_feas sustained   --out PATH [--seconds 300]
//! permanent_sampling_feas envelope    --throughput PATH --out PATH [--budget-hours 12]
//! permanent_sampling_feas zerofrac    --throughput PATH --sustained PATH --out PATH
//! ```
//!
//! Every subcommand writes a CSV whose preamble records the git SHA, toolchain,
//! hardware, governor, and invocation, and whose rows record seeds and sample
//! counts. `grid` and `sustained` must run on an otherwise idle host.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use permanent_sampling_feas::backend::Backend;
use permanent_sampling_feas::env::HostInfo;
use permanent_sampling_feas::equivalence::{check, EQUIVALENCE_CSV_HEADER};
use permanent_sampling_feas::protocol::{
    run_cell, run_sustained, shuffle, warm_machine, CellSpec, Outcome, CELL_CSV_HEADER,
    MAX_CELL_SECONDS, MIN_REPS, MIN_TIMED_SECONDS, PINNED_CORE, SUSTAINED_CSV_HEADER,
    SUSTAINED_INDICES_PER_RUN, WARMUP_SECONDS,
};
use permanent_sampling_feas::stats::{envelope_row, ENVELOPE_CSV_HEADER, Z_95};

/// Campaign root seed. Every stream in every subcommand derives from it, so a
/// rerun with this constant reproduces the exact matrix sequence.
const SEED_ROOT: u64 = 0xB488_F02C_0000_0001;

/// Field orders under study.
const QS: [u64; 3] = [3, 5, 7];
/// Matrix orders under study.
const NS: [usize; 5] = [12, 16, 20, 24, 28];
/// GPU batch sizes the study is required to cover. These are starting points:
/// `sustained` additionally streams at a larger batch to check they are not
/// the ceiling.
const GPU_BATCHES: [usize; 2] = [256, 1024];
/// Seconds of full-machine load before the grid, to reach thermal steady state.
const MACHINE_WARM_SECONDS: f64 = 90.0;
/// Stream indices reserved to each cell within every grid purpose.
const INDICES_PER_CELL: u64 = 100_000;
/// Number of grid specifications in one unfiltered execution.
///
/// `Backend::ALL` derives every prototype candidate from its registry and
/// contains one GPU entry, which the grid expands to both configured batch
/// sizes. Prototype candidates retain one explicitly unsupported cell until
/// their implementation supplies a harness batch evaluator.
const GRID_SPECS_PER_EXECUTION: usize = QS.len() * NS.len() * (Backend::ALL.len() + 1);
/// Stream-index space reserved to one fresh grid process.
const INDICES_PER_EXECUTION: u64 = GRID_SPECS_PER_EXECUTION as u64 * INDICES_PER_CELL;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "equivalence" => cmd_equivalence(&args),
        "grid" => cmd_grid(&args),
        "sustained" => cmd_sustained(&args),
        "envelope" => cmd_envelope(&args),
        "zerofrac" => cmd_zerofrac(&args),
        _ => {
            eprintln!("{}", include_str!("usage.txt"));
            std::process::exit(2);
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GridOptions {
    only: Option<String>,
    execution_id: u64,
    skip_machine_warmup: bool,
}

impl GridOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let execution_id = flag(args, "--execution-id")
            .unwrap_or("0")
            .parse::<u64>()
            .map_err(|e| format!("invalid --execution-id: {e}"))?;
        Ok(Self {
            only: flag(args, "--only").map(str::to_string),
            execution_id,
            skip_machine_warmup: args.iter().any(|arg| arg == "--skip-machine-warmup"),
        })
    }
}

fn grid_specs() -> Vec<CellSpec> {
    let mut specs = Vec::with_capacity(GRID_SPECS_PER_EXECUTION);
    for q in QS {
        for n in NS {
            for backend in Backend::ALL.into_iter().filter(|b| *b != Backend::Gpu) {
                specs.push(CellSpec {
                    q,
                    n,
                    backend,
                    batch_size: None,
                    seed_root: SEED_ROOT,
                    seed_index: 0,
                    order_index: 0,
                });
            }
            for m in GPU_BATCHES {
                specs.push(CellSpec {
                    q,
                    n,
                    backend: Backend::Gpu,
                    batch_size: Some(m),
                    seed_root: SEED_ROOT,
                    seed_index: 0,
                    order_index: 0,
                });
            }
        }
    }
    debug_assert_eq!(specs.len(), GRID_SPECS_PER_EXECUTION);
    specs
}

fn filter_specs(specs: &mut Vec<CellSpec>, filter: &str) -> Result<(), String> {
    let clauses = filter
        .split(',')
        .map(|clause| {
            let (key, value) = clause
                .split_once('=')
                .ok_or_else(|| format!("invalid --only clause `{clause}`"))?;
            if !matches!(key, "q" | "n" | "backend" | "batch_size") {
                return Err(format!("unknown --only key `{key}`"));
            }
            Ok((key, value))
        })
        .collect::<Result<Vec<_>, String>>()?;

    specs.retain(|spec| {
        clauses.iter().all(|(key, value)| match *key {
            "q" => spec.q.to_string() == *value,
            "n" => spec.n.to_string() == *value,
            "backend" => spec.backend.name() == *value,
            "batch_size" => spec
                .batch_size
                .is_some_and(|batch_size| batch_size.to_string() == *value),
            _ => unreachable!("filter keys validated above"),
        })
    });
    if specs.is_empty() {
        return Err(format!("--only {filter} matched no cell in the grid"));
    }
    Ok(())
}

/// Return the first stream index reserved to `order_index` in one fresh grid
/// process. Each execution owns a full unfiltered-grid block, so filtering
/// cannot make two execution ids reuse an address.
fn execution_index_base(execution_id: u64, order_index: usize) -> Result<u64, String> {
    if order_index >= GRID_SPECS_PER_EXECUTION {
        return Err(format!(
            "order index {order_index} is outside the {GRID_SPECS_PER_EXECUTION}-cell grid"
        ));
    }
    let execution_offset = execution_id
        .checked_mul(INDICES_PER_EXECUTION)
        .ok_or_else(|| format!("execution {execution_id} index range overflows u64"))?;
    let cell_offset = (order_index as u64)
        .checked_mul(INDICES_PER_CELL)
        .expect("bounded grid order cannot overflow");
    let first = execution_offset
        .checked_add(cell_offset)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| format!("execution {execution_id} index range overflows u64"))?;
    first
        .checked_add(INDICES_PER_CELL - 1)
        .ok_or_else(|| format!("execution {execution_id} index range overflows u64"))?;
    Ok(first)
}

fn out_path(args: &[String], default: &str) -> PathBuf {
    PathBuf::from(flag(args, "--out").unwrap_or(default))
}

fn open_csv(path: &Path, host: &HostInfo, header: &str, extra: &[String]) -> BufWriter<File> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut w = BufWriter::new(File::create(path).expect("create output CSV"));
    write!(w, "{}", host.csv_preamble()).expect("write preamble");
    for line in extra {
        writeln!(w, "# {line}").expect("write preamble extra");
    }
    writeln!(w, "{header}").expect("write header");
    w
}

/// Reopen an existing CSV for appending, for `grid --resume`.
///
/// Resuming is sound because the cell schedule is a pure function of
/// [`SEED_ROOT`]: the spec list is built in a fixed order and shuffled by a
/// seeded Fisher-Yates, so a resumed run walks the identical sequence with the
/// identical purpose/index addresses and simply skips what is already recorded.
/// A second preamble line records that the file was produced in more than one
/// session.
fn append_csv(path: &Path, host: &HostInfo) -> BufWriter<File> {
    let mut w = BufWriter::new(
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open output CSV for append"),
    );
    writeln!(
        w,
        "# resumed at {} on git {}; cell schedule is deterministic in seed_root, \
so the order and per-cell streams match the original session",
        host.timestamp_utc, host.git_sha
    )
    .expect("write resume note");
    w
}

/// Cells already recorded in `path`, keyed by `(q, n, backend, batch_size)`.
fn completed_cells(path: &Path) -> std::collections::HashSet<(u64, usize, String, usize)> {
    if !path.exists() {
        return std::collections::HashSet::new();
    }
    read_rows(path)
        .into_iter()
        .filter_map(|row| {
            Some((
                field(&row, "q").parse().ok()?,
                field(&row, "n").parse().ok()?,
                field(&row, "backend").to_string(),
                field(&row, "batch_size").parse().unwrap_or(0),
            ))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// equivalence
// ---------------------------------------------------------------------------

fn cmd_equivalence(args: &[String]) {
    let host = HostInfo::probe();
    let path = out_path(args, "equivalence.csv");
    let matrices: usize = flag(args, "--matrices")
        .and_then(|s| s.parse().ok())
        .unwrap_or(512);

    let notes = vec![
        format!(
            "check: per-matrix permanent values against a reference kernel, named per row in \
the reference column. The scalar single-word kernel is the reference wherever it exists; at \
q=7, n>16 it does not (permanent_bipedal7 asserts n <= Packed7::LANES = 16), so the generic \
permanent_ryser is the reference there"
        ),
        format!("matrices_per_cell: {matrices}"),
        format!("seed_root: 0x{SEED_ROOT:016x}, purpose: equivalence, index: 0"),
        "sizes: n in {8, 12, 16, 20} for every q. At q=7, n=20 the packed CPU kernels are \
recorded unsupported and the comparison runs between the GPU and the generic path"
            .to_string(),
    ];
    let mut w = open_csv(&path, &host, EQUIVALENCE_CSV_HEADER, &notes);

    let mut mismatches = 0usize;
    for q in QS {
        for n in [8usize, 12, 16, 20] {
            for row in check(q, n, matrices, SEED_ROOT) {
                mismatches += row.mismatches;
                println!(
                    "q={q} n={n} {:<24} {}",
                    row.backend,
                    if row.status.is_empty() {
                        "-"
                    } else {
                        &row.status
                    }
                );
                writeln!(w, "{}", row.to_csv_row()).expect("write row");
            }
        }
    }
    w.flush().expect("flush");
    println!("\nwrote {}", path.display());
    if mismatches > 0 {
        eprintln!("FAIL: {mismatches} per-matrix mismatches");
        std::process::exit(1);
    }
    println!("all backends agree per matrix");
}

// ---------------------------------------------------------------------------
// grid
// ---------------------------------------------------------------------------

fn cmd_grid(args: &[String]) {
    let host = HostInfo::probe();
    let path = out_path(args, "throughput.csv");
    let options = GridOptions::parse(args).unwrap_or_else(|message| panic!("{message}"));

    let mut specs = grid_specs();
    if let Some(filter) = &options.only {
        // Clauses combine with AND. In particular, `batch_size=1024` selects
        // one GPU launch shape rather than both configured GPU batches.
        filter_specs(&mut specs, filter).unwrap_or_else(|message| panic!("{message}"));
    }

    // Randomise execution order so boost and thermal drift decorrelate from the
    // grid axes, then restore ascending n as the outer key.
    //
    // The stable sort leaves each n-stratum in random order while guaranteeing
    // that every cell runs after a smaller n on the same (q, backend, batch
    // size). Censoring projects from a *measured rate* at another n, so without
    // that guarantee a cell with no reference falls back to probing — and at
    // q=7, n=28 on the GPU a single-matrix probe costs 42 minutes. Full
    // randomisation was measured to be affordable only because the earlier,
    // superseded rule was wrong; this schedule is the price of the correct
    // inference. Within-stratum randomisation is retained, the machine is warmed
    // to steady state first, and the sustained runs bound drift over the 180 s
    // window (magnitude: see the sustained receipt and the study), so the
    // residual correlation between n and elapsed time is small, recorded here.
    shuffle(&mut specs, SEED_ROOT);
    specs.sort_by_key(|s| s.n);
    for (i, spec) in specs.iter_mut().enumerate() {
        spec.order_index = i;
        spec.seed_index = execution_index_base(options.execution_id, i)
            .unwrap_or_else(|message| panic!("{message}"));
    }

    let execution_index_first =
        execution_index_base(options.execution_id, 0).unwrap_or_else(|message| panic!("{message}"));
    let execution_index_last =
        execution_index_base(options.execution_id, GRID_SPECS_PER_EXECUTION - 1)
            .and_then(|first| {
                first
                    .checked_add(INDICES_PER_CELL - 1)
                    .ok_or_else(|| "execution index range overflows u64".to_string())
            })
            .unwrap_or_else(|message| panic!("{message}"));

    let notes = vec![
        format!(
            "protocol: warmup >= {WARMUP_SECONDS:.0} s, then >= {MIN_REPS} reps and \
>= {MIN_TIMED_SECONDS:.0} s timed, cap {MAX_CELL_SECONDS:.0} s per cell"
        ),
        format!("composite = generate + evaluate + reduce + store, all timed per repetition"),
        format!(
            "store: one shard record per batch appended and fsync-free flushed to a scratch file"
        ),
        format!(
            "rates are summed matrices over summed time; per-repetition rates are never averaged"
        ),
        format!(
            "schedule: randomised with seed 0x{SEED_ROOT:016x} then stably sorted by ascending n, \
so each n-stratum is randomised and every cell has a smaller-n reference on its own \
(q, backend, batch size); see order_index"
        ),
        format!(
            "outcomes: measured | unsupported (a kernel bound forbids the cell, named in note) | \
censored (not attempted; carries NO measured rate)"
        ),
        format!(
            "censoring contract: a censored row's composite_matrices_per_s is NaN. Its \
projected_matrices_per_s is an ESTIMATE, obtained by scaling the MEASURED rate at \
projection_reference_n through Ryser's n*2^n work model. No bound on a batched rate is \
derived from probe_matrix_s, which is a single-matrix LATENCY. Neither 1/probe nor \
W/probe (W = compute units) bounds a batched rate: a compute unit hosts several \
workgroups at once and a probe pays launch costs a batch amortises. An earlier harness \
published W/probe as an upper bound and measurements exceeded it; the study's section 4.3 \
records that falsification and says which of its numbers survive in this file"
        ),
        format!(
            "projection accuracy: on the q=3 GPU chain, where the projection can be checked \
against a measurement, it lands LOW at every step. Censored cells' true rates are therefore \
expected to run somewhat higher than their projections - an extrapolation from that one \
chain to the others, not a measured property of them. The magnitude is re-derived from THIS \
file's own q=3 chain in the study's section 4.3 rather than quoted here, so that a stale \
figure cannot survive a re-measurement"
        ),
        format!(
            "seed_root: 0x{SEED_ROOT:016x}; each cell owns {INDICES_PER_CELL} indices within each named purpose"
        ),
        format!(
            "execution_id: {}; reserved index block: {}..={} inclusive; full matrix \
address is (seed_root, q, n, purpose, stream_index)",
            options.execution_id, execution_index_first, execution_index_last
        ),
        format!(
            "machine warmup: {}",
            if options.skip_machine_warmup {
                "skipped by --skip-machine-warmup; caller must preserve a prior locked warmup"
                    .to_string()
            } else {
                format!(
                    "full rayon load for {MACHINE_WARM_SECONDS:.0} s before the first timed cell"
                )
            }
        ),
        format!(
            "eval_s is the host wall-clock around GPU serialisation, allocation, H2D, launch, \
stream wait, D2H, and free. kernel_device_s is the separate device-event kernel-only total; \
h2d_device_s, d2h_device_s, host_submission_s, and device_submission_to_kernel_s are separate \
phase totals. No column subtracts host and device timestamps. Empty phase columns carry their \
reason in phase_timing_note and are never zero-filled or replaced by eval_s. If timing setup \
fails but synchronous gpu_hip evaluation succeeds, that same-backend result is retained and the \
note names the unavailable instrumented boundary"
        ),
        format!(
            "single-thread cells pinned to core {PINNED_CORE}; rayon cells use all logical CPUs"
        ),
    ];
    let resume = args.iter().any(|a| a == "--resume") && path.exists();
    let done = if resume {
        completed_cells(&path)
    } else {
        std::collections::HashSet::new()
    };
    let mut w = if resume {
        eprintln!(
            "resuming: {} cells already recorded in {}",
            done.len(),
            path.display()
        );
        append_csv(&path, &host)
    } else {
        open_csv(&path, &host, CELL_CSV_HEADER, &notes)
    };

    let shard_path = std::env::temp_dir().join("permanent_sampling_feas_shards.csv");
    let mut sink = BufWriter::new(File::create(&shard_path).expect("create shard sink"));

    // Seed both caches from any rows already on disk, so a resumed run reuses
    // probes and rate references measured in the earlier session.
    let mut probes: permanent_sampling_feas::protocol::ProbeCache = Default::default();
    let mut rates: permanent_sampling_feas::protocol::RateCache = Default::default();
    if resume {
        for row in read_rows(&path) {
            let (Ok(q), Ok(n)) = (
                field(&row, "q").parse::<u64>(),
                field(&row, "n").parse::<usize>(),
            ) else {
                continue;
            };
            let Some(name) = Backend::ALL
                .into_iter()
                .map(|b| b.name())
                .find(|nm| *nm == field(&row, "backend"))
            else {
                continue;
            };
            if let Ok(probe) = field(&row, "probe_matrix_s").parse::<f64>() {
                if probe.is_finite() {
                    probes.insert((q, name, n), probe);
                }
            }
            if field(&row, "outcome") == "measured" {
                if let (Ok(rate), Ok(batch)) = (
                    field(&row, "composite_matrices_per_s").parse::<f64>(),
                    field(&row, "batch_size").parse::<usize>(),
                ) {
                    // Only the fixed-batch backend keys on M; the adaptive ones
                    // key on 0, matching `CellSpec::batch_size.unwrap_or(0)`.
                    let key = if name == Backend::Gpu.name() {
                        batch
                    } else {
                        0
                    };
                    if rate.is_finite() && rate > 0.0 {
                        rates
                            .entry((q, name, key))
                            .and_modify(|e| {
                                if n > e.0 {
                                    *e = (n, rate);
                                }
                            })
                            .or_insert((n, rate));
                    }
                }
            }
        }
        eprintln!(
            "seeded {} probe and {} rate references from the existing CSV",
            probes.len(),
            rates.len()
        );
    }

    if options.skip_machine_warmup {
        eprintln!("skipping machine warmup by request");
    } else {
        eprintln!("warming the machine for {MACHINE_WARM_SECONDS:.0} s ...");
        warm_machine(MACHINE_WARM_SECONDS, SEED_ROOT);
    }

    let total = specs.len();
    for (i, spec) in specs.iter().enumerate() {
        // An adaptive cell has no fixed batch size, so it matches any recorded
        // row for its (q, n, backend); a fixed-batch cell must match its size.
        let already = match spec.batch_size {
            Some(m) => done.contains(&(spec.q, spec.n, spec.backend.name().to_string(), m)),
            None => done
                .iter()
                .any(|(q, n, b, _)| *q == spec.q && *n == spec.n && b == spec.backend.name()),
        };
        if already {
            continue;
        }
        eprintln!(
            "[{}/{}] q={} n={} {}",
            i + 1,
            total,
            spec.q,
            spec.n,
            spec.backend.name()
        );
        let r = run_cell(spec, &mut sink, &mut probes, &mut rates);
        match r.outcome {
            Outcome::Measured => eprintln!(
                "    {:.3} matrices/s composite ({:.3} eval-only), M={}, reps={}, sd={:.3} s",
                r.composite_rate, r.eval_rate, r.batch_size, r.reps, r.rep_sd_s
            ),
            Outcome::Unsupported => eprintln!("    unsupported: {}", r.note),
            Outcome::Censored => eprintln!("    censored: {}", r.note),
        }
        writeln!(w, "{}", r.to_csv_row()).expect("write row");
        w.flush().expect("flush");
    }
    let _ = sink.flush();
    let _ = std::fs::remove_file(&shard_path);
    println!("wrote {}", path.display());
}

// ---------------------------------------------------------------------------
// sustained
// ---------------------------------------------------------------------------

fn cmd_sustained(args: &[String]) {
    let host = HostInfo::probe();
    let path = out_path(args, "sustained.csv");
    let seconds: f64 = flag(args, "--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(300.0);

    // One run per backend family at a size the campaign would plausibly use,
    // plus a GPU batch above the 256/1024 starting points to test whether they
    // are the ceiling.
    //
    // GPU M=4096 at q=3, n=24 was attempted on 2026-08-07 and the device
    // faulted ("HW Exception by GPU node-1 ... reason :GPU Hang"), killing the
    // process. That happened once and was never retried, and nothing was
    // captured that identifies the cause; a watchdog timeout is one hypothesis
    // among several, not a finding. The probe is lowered to M=2048 to approach
    // the untested region from below without risking a second device reset -
    // a precaution justified by the cost of a fault, not by a known threshold.
    // See the study's section 4.5 and gpu-hang-2026-08-07.log.
    let runs: Vec<(u64, usize, Backend, usize)> = vec![
        (3, 24, Backend::Scalar, 8),
        (3, 24, Backend::Avx2, 4),
        (3, 24, Backend::Rayon, 96),
        (3, 24, Backend::RayonIntra, 24),
        (3, 24, Backend::Gpu, 1024),
        (3, 24, Backend::Gpu, 2048),
        (5, 20, Backend::Rayon, 96),
        (5, 20, Backend::Gpu, 1024),
        (7, 16, Backend::Rayon, 512),
        (7, 20, Backend::Gpu, 1024),
    ];

    let notes = vec![
        format!("sustained window: {seconds:.0} s per run, composite hot path throughout"),
        "first/last quarter rates cover the whole shards falling in the first and last \
quarter of ELAPSED TIME within the window, not the first and last quarter of the shard \
count; each is that group's matrices over that group's time. The split measures drift, \
it does not attribute it to a cause"
            .to_string(),
        format!(
            "seed_root: 0x{SEED_ROOT:016x}; sustained purpose uses indices \
j*{SUSTAINED_INDICES_PER_RUN}..(j+1)*{SUSTAINED_INDICES_PER_RUN}-1. Its distinct \
purpose tag makes these seed addresses separate from all grid purposes"
        ),
        "gpu batch 2048 probes whether 256/1024 are the ceiling; M=4096 was tried once \
and the device faulted, ending that run. The fault is a single unretried event and its \
cause is NOT established here: gpu-hang-2026-08-07.log is its only receipt and records \
what was and was not captured. No ceiling mechanism is asserted from it"
            .to_string(),
        "each run reserves a disjoint index range (purpose and index_first columns), so two runs at one \
(q, n) draw independent samples and their zero counts may be pooled"
            .to_string(),
    ];
    let resume = args.iter().any(|a| a == "--resume") && path.exists();
    let done: std::collections::HashSet<(u64, usize, String, usize)> = if resume {
        read_rows(&path)
            .into_iter()
            .filter_map(|row| {
                Some((
                    field(&row, "q").parse().ok()?,
                    field(&row, "n").parse().ok()?,
                    field(&row, "backend").to_string(),
                    field(&row, "batch_size").parse().ok()?,
                ))
            })
            .collect()
    } else {
        std::collections::HashSet::new()
    };
    let mut w = if resume {
        eprintln!("resuming: {} runs already recorded", done.len());
        append_csv(&path, &host)
    } else {
        open_csv(&path, &host, SUSTAINED_CSV_HEADER, &notes)
    };

    let shard_path = std::env::temp_dir().join("permanent_sampling_feas_sustained.csv");
    let mut sink = BufWriter::new(File::create(&shard_path).expect("create shard sink"));

    for (run_index, (q, n, backend, m)) in runs.into_iter().enumerate() {
        if let permanent_sampling_feas::backend::Support::Unsupported(reason) =
            permanent_sampling_feas::backend::support(backend, q, n)
        {
            eprintln!("skip q={q} n={n} {}: {reason}", backend.name());
            continue;
        }
        if done.contains(&(q, n, backend.name().to_string(), m)) {
            continue;
        }
        eprintln!("sustained q={q} n={n} {} M={m} ...", backend.name());
        let r = run_sustained(
            &permanent_sampling_feas::protocol::SustainedSpec {
                q,
                n,
                backend,
                batch_size: m,
                seconds,
                seed_root: SEED_ROOT,
                run_index: run_index as u64,
            },
            &mut sink,
        );
        eprintln!(
            "    {:.3} matrices/s over {:.0} s ({} shards); first quarter {:.3}, last {:.3}",
            r.sustained_rate, r.wall_s, r.shards, r.first_quarter_rate, r.last_quarter_rate
        );
        writeln!(w, "{}", r.to_csv_row()).expect("write row");
        w.flush().expect("flush");
    }
    let _ = sink.flush();
    let _ = std::fs::remove_file(&shard_path);
    println!("wrote {}", path.display());
}

// ---------------------------------------------------------------------------
// envelope
// ---------------------------------------------------------------------------

/// Read a CSV written by [`open_csv`] into one map per data row, skipping the
/// `#` preamble.
fn read_rows(path: &Path) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).expect("read CSV");
    let mut header: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if header.is_empty() {
            header = line.split(',').map(str::to_string).collect();
            continue;
        }
        rows.push(
            header
                .iter()
                .cloned()
                .zip(line.split(',').map(str::to_string))
                .collect(),
        );
    }
    rows
}

fn field<'a>(row: &'a HashMap<String, String>, name: &str) -> &'a str {
    row.get(name).map(String::as_str).unwrap_or("")
}

/// Parse the throughput CSV into `(q, n) -> (best backend, best composite rate)`.
fn best_rates(path: &Path) -> HashMap<(u64, usize), (String, f64)> {
    let mut best: HashMap<(u64, usize), (String, f64)> = HashMap::new();
    for row in read_rows(path) {
        let get = |name: &str| field(&row, name);
        if get("outcome") != "measured" {
            continue;
        }
        let q: u64 = get("q").parse().expect("q column");
        let n: usize = get("n").parse().expect("n column");
        let rate: f64 = get("composite_matrices_per_s").parse().unwrap_or(f64::NAN);
        let backend = get("backend").to_string();
        if !rate.is_finite() {
            continue;
        }
        let entry = best.entry((q, n)).or_insert((backend.clone(), f64::NAN));
        if entry.1.is_nan() || rate > entry.1 {
            *entry = (backend, rate);
        }
    }
    best
}

fn cmd_envelope(args: &[String]) {
    let host = HostInfo::probe();
    let throughput =
        PathBuf::from(flag(args, "--throughput").expect("envelope requires --throughput PATH"));
    let path = out_path(args, "envelope.csv");
    let budget_hours: f64 = flag(args, "--budget-hours")
        .and_then(|s| s.parse().ok())
        .unwrap_or(12.0);

    let best = best_rates(&throughput);
    let notes = vec![
        format!("derived from: {}", throughput.display()),
        format!("budget: {budget_hours} h wall clock per (q, n) cell"),
        format!(
            "operational reserve: {:.0}% of the budget is withheld for checkpointing, dataset \
compaction, restart after a failed shard, and residual throttling",
            permanent_sampling_feas::stats::OPERATIONAL_RESERVE * 100.0
        ),
        "required N = ceil(p (1 - p) / SE^2); planning p = 1/q, conservative p = 1/2".to_string(),
        "rate used is the best measured COMPOSITE rate over all backends at that (q, n)"
            .to_string(),
        "scheinerman2024_* columns are the prior art for q=3 only (arXiv:2407.20205v2 Table 4); \
comparison is on achieved PRECISION (standard error), not raw trial count"
            .to_string(),
        "precision_comparison: exceeds/matches/below_prior_precision (matches = within 10% on SE), \
prior_exact where the published value is a full enumeration. no_prior means this harness \
carries no baseline for that (q, n) - for q in {5,7} none is tabulated here; whether any \
exists in the literature is a separate question the study addresses"
            .to_string(),
    ];
    let mut w = open_csv(&path, &host, ENVELOPE_CSV_HEADER, &notes);

    for q in QS {
        for n in NS {
            for se in [1e-3f64, 1e-4] {
                match best.get(&(q, n)) {
                    Some((backend, rate)) if rate.is_finite() => {
                        let row = envelope_row(q, n, se, backend.clone(), *rate, budget_hours);
                        println!(
                            "q={q} n={n} SE={se:.0e}: {} at {:.2}/s -> N={} in {:.2} h ({})",
                            row.best_backend,
                            row.best_rate,
                            row.required_n_planning,
                            row.hours_planning,
                            if row.feasible {
                                "feasible"
                            } else {
                                "infeasible"
                            }
                        );
                        if let (Some(prior_se), Some(ratio)) = (row.prior_se, row.precision_ratio) {
                            println!(
                                "        vs Scheinerman2024: SE {prior_se:.3e} at {} trials; \
ours {:.3e} -> {ratio:.2}x ({})",
                                row.prior_trials.unwrap_or(0),
                                row.attainable_se,
                                row.prior_comparison.name()
                            );
                        }
                        writeln!(w, "{}", row.to_csv_row()).expect("write row");
                    }
                    _ => eprintln!("q={q} n={n}: no measured cell in {}", throughput.display()),
                }
            }
        }
    }
    w.flush().expect("flush");
    println!("wrote {}", path.display());
}

// ---------------------------------------------------------------------------
// zerofrac
// ---------------------------------------------------------------------------

/// Pool the zero counts the timing runs incidentally produced into a
/// preliminary $\Pr[\mathrm{per} = 0]$ estimate per `(q, n)`, with a Wilson
/// interval.
///
/// These are a by-product of the throughput measurements, not a campaign
/// result: the sample counts are whatever the timing protocol happened to
/// need. They are reported because they are genuine draws from the campaign's
/// sampler through the campaign's kernels, so they demonstrate the end-to-end
/// statistic and give a first look at the conjectured value. Every backend
/// cell draws from its own reserved purpose/index address range, so pooling across backends
/// pools independent samples.
fn cmd_zerofrac(args: &[String]) {
    let host = HostInfo::probe();
    let path = out_path(args, "zero-fraction.csv");
    let mut inputs: Vec<PathBuf> = Vec::new();
    if let Some(p) = flag(args, "--throughput") {
        inputs.push(PathBuf::from(p));
    }
    if let Some(p) = flag(args, "--sustained") {
        inputs.push(PathBuf::from(p));
    }
    assert!(
        !inputs.is_empty(),
        "zerofrac requires --throughput PATH and/or --sustained PATH"
    );

    let mut pooled: HashMap<(u64, usize), (u64, u64)> = HashMap::new();
    for input in &inputs {
        for row in read_rows(input) {
            let outcome = field(&row, "outcome");
            if !outcome.is_empty() && outcome != "measured" {
                continue;
            }
            let q: u64 = field(&row, "q").parse().expect("q column");
            let n: usize = field(&row, "n").parse().expect("n column");
            let matrices: u64 = field(&row, "matrices").parse().unwrap_or(0);
            let zeros: u64 = field(&row, "zeros").parse().unwrap_or(0);
            let e = pooled.entry((q, n)).or_insert((0, 0));
            e.0 += matrices;
            e.1 += zeros;
        }
    }

    let sources = inputs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let notes = vec![
        format!("pooled from: {sources}"),
        "these counts are a by-product of the timing protocol; sample sizes are \
whatever each cell needed, not a designed sampling plan"
            .to_string(),
        format!("interval: Wilson score, 95 % (z = {Z_95:.6})"),
        format!("seed_root: 0x{SEED_ROOT:016x}"),
    ];
    let mut w = open_csv(
        &path,
        &host,
        "q,n,matrices,zeros,p_hat,wilson_lo_95,wilson_hi_95,one_over_q,\
one_over_q_inside_interval,scheinerman2024_p,scheinerman2024_inside_interval",
        &notes,
    );

    let mut keys: Vec<_> = pooled.keys().copied().collect();
    keys.sort_unstable();
    for (q, n) in keys {
        let (matrices, zeros) = pooled[&(q, n)];
        if matrices == 0 {
            continue;
        }
        let p_hat = zeros as f64 / matrices as f64;
        let (lo, hi) = permanent_sampling_feas::stats::wilson_interval(
            zeros,
            matrices,
            permanent_sampling_feas::stats::Z_95,
        );
        let inv_q = 1.0 / q as f64;
        let inside = lo <= inv_q && inv_q <= hi;
        // Published [Scheinerman2024] value at this (q, n), where one exists.
        // Its own sampling error is far below ours at every shared n, so it is
        // compared as a point against our interval rather than interval-to-interval.
        let prior = permanent_sampling_feas::prior::prior_zero_fraction(q, n);
        let prior_inside = prior.map(|p| lo <= p && p <= hi);
        println!(
            "q={q} n={n}: p_hat={p_hat:.5} [{lo:.5}, {hi:.5}] over {matrices} matrices; \
1/q={inv_q:.5} {}{}",
            if inside { "inside" } else { "OUTSIDE" },
            match (prior, prior_inside) {
                (Some(p), Some(true)) => format!("; Scheinerman2024 {p:.6} inside"),
                (Some(p), Some(false)) => format!("; Scheinerman2024 {p:.6} OUTSIDE"),
                _ => String::new(),
            }
        );
        let prior_s = prior.map_or_else(|| "none".to_string(), |p| format!("{p:.6}"));
        let prior_inside_s = prior_inside.map_or_else(|| "n/a".to_string(), |b| b.to_string());
        writeln!(
            w,
            "{q},{n},{matrices},{zeros},{p_hat:.6},{lo:.6},{hi:.6},{inv_q:.6},{inside},\
{prior_s},{prior_inside_s}"
        )
        .expect("write row");
    }
    w.flush().expect("flush");
    println!("wrote {}", path.display());
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[cfg(feature = "prototype-registry")]
    use permanent_wave_gpu::MeasurementPath;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn grid_cli_parses_exact_batch_execution_and_warmup_controls() {
        let parsed = GridOptions::parse(&args(&[
            "permanent_sampling_feas",
            "grid",
            "--only",
            "q=3,n=28,backend=gpu_hip,batch_size=1024",
            "--execution-id",
            "47",
            "--skip-machine-warmup",
        ]))
        .expect("valid grid options");

        assert_eq!(parsed.execution_id, 47);
        assert!(parsed.skip_machine_warmup);
        assert_eq!(
            parsed.only.as_deref(),
            Some("q=3,n=28,backend=gpu_hip,batch_size=1024")
        );
    }

    #[test]
    fn exact_gpu_batch_filter_selects_only_m1024() {
        let mut specs = grid_specs();
        filter_specs(&mut specs, "q=3,n=28,backend=gpu_hip,batch_size=1024")
            .expect("valid exact filter");

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].q, 3);
        assert_eq!(specs[0].n, 28);
        assert_eq!(specs[0].backend, Backend::Gpu);
        assert_eq!(specs[0].batch_size, Some(1024));
    }

    #[cfg(feature = "prototype-registry")]
    #[test]
    fn timing_grid_includes_every_registered_prototype_path() {
        let specs = grid_specs();
        for path in MeasurementPath::ALL {
            assert!(
                specs
                    .iter()
                    .any(|spec| spec.backend.name() == path.name() && spec.batch_size.is_none()),
                "{} is missing from the timing grid",
                path.name()
            );
        }
    }

    #[test]
    fn execution_index_ranges_are_disjoint_and_checked() {
        let execution_0_last = execution_index_base(0, GRID_SPECS_PER_EXECUTION - 1)
            .expect("last cell in execution zero");
        let execution_1_first = execution_index_base(1, 0).expect("first cell in execution one");

        assert_eq!(execution_0_last + INDICES_PER_CELL, execution_1_first);
        assert!(execution_index_base(0, GRID_SPECS_PER_EXECUTION).is_err());
        assert!(execution_index_base(u64::MAX, 0).is_err());
    }
}
