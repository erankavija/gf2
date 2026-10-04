//! Runs the pinned eBCH(128,64,22) OSD reference campaign (BI-AWGN, BPSK, rate
//! 1/2, BER versus Eb/N0 in dB) through [`gf2_sim::osd_campaign`]: an order-2
//! target and an order-1 control on the seven `@/citation/Fossorier1994`
//! abscissas.
//!
//! # Options
//!
//! `--max-samples` is an additional per-invocation sample bound. Reaching it
//! records an interrupted cell so a later invocation can continue from the
//! durable counters. `--target-block-errors` is the cumulative independent
//! block-error target for a cell; reaching it records a completed cell.
//!
//! `--workers` sets how many blocks are decoded concurrently. Every block
//! draws from its own reserved region of the cell's ChaCha20 stream, keyed on
//! the block index alone; `tests/ebch_osd_campaign_cli.rs`
//! (`worker_count_is_recorded_provenance_and_leaves_cell_evidence_unchanged`)
//! compares the cell evidence across worker counts.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf2_coding::bch::encode::SystematicLayout;
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::bch::LayoutView;
use gf2_coding::osd::{GeneratorMatrixOsdDecoder, OsdConfig};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel};
use gf2_coding::traits::block::{BlockCode, BlockEncoder};
use gf2_coding::transform::Extended;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};
use rand08::Rng;
use rand_chacha08::ChaCha20Rng;
use sha2::{Digest, Sha256};

use gf2_sim::osd_campaign::{
    BinomialIntervalMethod, BinomialIntervalSpec, OsdBlockContext, OsdBlockOutcome, OsdBlockStream,
    OsdCampaign, OsdCampaignError, OsdCampaignProvenance, OsdCell, OsdCellId, OsdWorkCounters,
};
use gf2_sim::permanent_campaign::provenance::{
    observe_cpu_identity, observe_provenance, repository_top_level,
};
use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, GitRevision, Provenance, RngAlgorithm, Sha256Digest,
};

const PINNED_EB_N0_DB: [f64; 7] = [1.55, 2.22, 3.01, 3.47, 3.98, 4.56, 5.23];
const ORDER_2_DIGITIZATION_PRECISION: [f64; 7] = [0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
const ORDER_1_DIGITIZATION_PRECISION: [f64; 7] = [0.1; 7];
const DEFAULT_SEED: u64 = 0xC832_2EFF;
const DEFAULT_MAX_SAMPLES: u64 = 100_000;
const DEFAULT_TARGET_BLOCK_ERRORS: u64 = 100;

const USAGE: &str = "Usage: ebch_osd_awgn_campaign --checkpoint PATH --receipt PATH [OPTIONS]

Pinned campaign:
  eBCH(128,64,22), BI-AWGN/BPSK, rate 1/2, BER vs Eb/N0 (dB)
  order 2 is the Fossorier target; order 1 is an internal control
  cells: Eb/N0 = 1.55, 2.22, 3.01, 3.47, 3.98, 4.56, 5.23 dB

Options:
  --checkpoint PATH       Protocol checkpoint/progress output path (required)
  --receipt PATH           Versioned JSON receipt output path (required)
  --seed U64               Campaign root seed [default: 0xC8322EFF]
  --max-samples N         Additional samples per cell invocation [default: 100000]
  --target-block-errors N Cumulative block errors per cell [default: 100]
  --workers N             Concurrent block evaluators [default: available parallelism]
  --help                  Show this message

Worker count affects throughput only: results are byte-identical at any
worker count, and the resolved value is recorded in the receipt's invocation
argument vector.

OSD list and tie policy:
  ascending Hamming weight over the MRI positions, then lexicographic
  ascending zero-based indices within a weight; source-undefined reliability
  ties use ascending original coordinate index, and source-undefined distance
  ties retain the first generated candidate (implementation decisions).

The library owns the versioned receipt schema, block-sampled confidence
intervals, deterministic cell seeds, checkpoint validation, and resume history.";

#[derive(Debug)]
struct Args {
    checkpoint: PathBuf,
    receipt: PathBuf,
    seed: u64,
    max_samples: u64,
    target_block_errors: u64,
    workers: NonZeroUsize,
    invocation: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The pinned eBCH(128,64,22) code: the binary BCH(127,64) code with designed
/// distance 21 over GF(2^7) with primitive polynomial `x^7 + x + 1` (`0x83`),
/// presented as `[message | parity]` with both blocks in descending degree
/// order, extended by one overall parity coordinate. The descending layout is
/// the convention of the recorded reference generator in
/// `crates/gf2-coding/tests/data/ebch_128_64_reference.json`.
type Ebch128 = Extended<LayoutView<BinaryPrimeExt, BitVec, BitMatrix>>;

fn ebch_128_64() -> Ebch128 {
    let extension = BinaryPrimeExt::new(Gf2mField::new(7, 0b10000011).with_tables())
        .expect("x^7 + x + 1 presents GF(2^7)");
    let designed_distance = DesignedDistance::try_from(21).expect("a positive designed distance");
    let base = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance,
    })
    .expect("the pinned BCH(127,64) construction is valid");
    let mother = LayoutView::new(base, SystematicLayout::MessageParityDescending);
    let code = Extended::new(mother).expect("an extended BCH code fits in memory");
    assert_eq!((code.n(), code.k()), (128, 64), "pinned eBCH(128,64) shape");
    code
}

fn run() -> Result<(), String> {
    let Some(args) = parse_args()? else {
        println!("{USAGE}");
        return Ok(());
    };
    let provenance = campaign_provenance(args.invocation)?;
    let campaign = pinned_campaign(args.seed, args.target_block_errors, provenance)
        .map_err(|error| error.to_string())?;
    let code = ebch_128_64();
    let receipt = gf2_sim::osd_campaign::run_osd_campaign(
        &args.checkpoint,
        &campaign,
        args.max_samples,
        args.workers,
        || {
            let mut evaluator = CellEvaluator::new(&code);
            move |context: OsdBlockContext<'_>| evaluator.sample(context)
        },
    )
    .map_err(|error| error.to_string())?;
    write_receipt(&args.receipt, &receipt)?;
    println!(
        "wrote {} cell attempts to checkpoint {} and receipt {} using {} workers",
        receipt.cell_results.len(),
        args.checkpoint.display(),
        args.receipt.display(),
        args.workers
    );
    Ok(())
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut checkpoint = None;
    let mut receipt = None;
    let mut seed = DEFAULT_SEED;
    let mut max_samples = DEFAULT_MAX_SAMPLES;
    let mut invocation: Vec<String> = std::env::args().collect();
    let arguments: Vec<String> = invocation.iter().skip(1).cloned().collect();
    let mut target_block_errors = DEFAULT_TARGET_BLOCK_ERRORS;
    let mut workers = None;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--help" | "-h" => return Ok(None),
            "--checkpoint" => {
                checkpoint = Some(required_value(&arguments, &mut index, "--checkpoint")?);
            }
            "--receipt" => {
                receipt = Some(required_value(&arguments, &mut index, "--receipt")?);
            }
            "--seed" => {
                let value = required_value(&arguments, &mut index, "--seed")?;
                seed = parse_u64(&value, "--seed")?;
            }
            "--max-samples" => {
                let value = required_value(&arguments, &mut index, "--max-samples")?;
                max_samples = value
                    .parse()
                    .map_err(|_| format!("--max-samples must be an integer, got {value:?}"))?;
                if max_samples == 0 {
                    return Err("--max-samples must be positive".to_owned());
                }
            }
            "--target-block-errors" => {
                let value = required_value(&arguments, &mut index, "--target-block-errors")?;
                target_block_errors = value.parse().map_err(|_| {
                    format!("--target-block-errors must be an integer, got {value:?}")
                })?;
                if target_block_errors == 0 {
                    return Err("--target-block-errors must be positive".to_owned());
                }
            }
            "--workers" => {
                let value = required_value(&arguments, &mut index, "--workers")?;
                workers =
                    Some(value.parse::<NonZeroUsize>().map_err(|_| {
                        format!("--workers must be a positive integer, got {value:?}")
                    })?);
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown option {option:?}\n\n{USAGE}"));
            }
            positional => return Err(format!("unexpected positional argument {positional:?}")),
        }
        index += 1;
    }

    // The recorded argument vector reproduces the run, so it always names the
    // resolved worker count even when the flag was left to its default.
    let workers = match workers {
        Some(workers) => workers,
        None => {
            let workers = available_parallelism();
            invocation.push("--workers".to_owned());
            invocation.push(workers.to_string());
            workers
        }
    };

    Ok(Some(Args {
        checkpoint: checkpoint
            .ok_or_else(|| "--checkpoint is required".to_owned())
            .map(PathBuf::from)?,
        receipt: receipt
            .ok_or_else(|| "--receipt is required".to_owned())
            .map(PathBuf::from)?,
        seed,
        max_samples,
        target_block_errors,
        workers,
        invocation,
    }))
}

fn available_parallelism() -> NonZeroUsize {
    std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

fn required_value(arguments: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_u64(value: &str, option: &str) -> Result<u64, String> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .map_or((value, 10), |digits| (digits, 16));
    u64::from_str_radix(digits, radix)
        .map_err(|_| format!("{option} must be an unsigned integer, got {value:?}"))
}

fn pinned_campaign(
    seed: u64,
    target_block_errors: u64,
    provenance: OsdCampaignProvenance,
) -> Result<OsdCampaign, OsdCampaignError> {
    let mut cells = Vec::with_capacity(PINNED_EB_N0_DB.len() * 2);
    for (index, (&eb_n0_db, &precision)) in PINNED_EB_N0_DB
        .iter()
        .zip(ORDER_2_DIGITIZATION_PRECISION.iter())
        .enumerate()
    {
        cells.push(OsdCell::new(
            format!("order-2-point-{index:02}")
                .parse()
                .expect("static cell id"),
            eb_n0_db,
            2,
            precision,
        )?);
    }
    for (index, (&eb_n0_db, &precision)) in PINNED_EB_N0_DB
        .iter()
        .zip(ORDER_1_DIGITIZATION_PRECISION.iter())
        .enumerate()
    {
        cells.push(OsdCell::new(
            format!("order-1-point-{index:02}")
                .parse()
                .expect("static cell id"),
            eb_n0_db,
            1,
            precision,
        )?);
    }
    let interval =
        BinomialIntervalSpec::new(BinomialIntervalMethod::NegativeBinomialClopperPearson, 0.95)?;
    OsdCampaign::new(seed, cells, interval, target_block_errors, provenance)
}

/// One worker's block evaluator. Every block seeks the cell's stream to its own
/// reserved region, so an evaluator carries no state from one block to the next.
struct CellEvaluator<'a> {
    code: &'a Ebch128,
    channel: BpskAwgnChannel,
    binding: Option<CellBinding>,
}

struct CellBinding {
    cell_id: OsdCellId,
    decoder: GeneratorMatrixOsdDecoder<Ebch128>,
    stream: OsdBlockStream,
}

impl<'a> CellEvaluator<'a> {
    fn new(code: &'a Ebch128) -> Self {
        Self {
            code,
            channel: BpskAwgnChannel,
            binding: None,
        }
    }

    fn sample(&mut self, context: OsdBlockContext<'_>) -> OsdBlockOutcome {
        let rebind = self
            .binding
            .as_ref()
            .is_none_or(|binding| binding.cell_id != context.cell.id);
        if rebind {
            self.binding = Some(CellBinding {
                cell_id: context.cell.id.clone(),
                decoder: GeneratorMatrixOsdDecoder::new(
                    self.code.clone(),
                    OsdConfig::new(usize::from(context.cell.osd_order)),
                ),
                stream: OsdBlockStream::new(context.seed),
            });
        }
        let binding = self.binding.as_mut().expect("the cell was just bound");

        binding.stream.seek_to_block(context.block_index);
        let frame = simulate_frame(
            self.code,
            &binding.decoder,
            &self.channel,
            context.cell.eb_n0_db,
            binding.stream.rng_mut(),
        );
        binding
            .stream
            .debug_assert_block_budget(context.block_index);
        OsdBlockOutcome {
            information_bits: self.code.k() as u64,
            information_bit_errors: frame.bit_errors,
            work: frame.work,
        }
    }
}

struct FrameResult {
    bit_errors: u64,
    work: OsdWorkCounters,
}

fn simulate_frame(
    code: &Ebch128,
    decoder: &GeneratorMatrixOsdDecoder<Ebch128>,
    channel: &BpskAwgnChannel,
    eb_n0_db: f64,
    rng: &mut ChaCha20Rng,
) -> FrameResult {
    let mut message = BitVec::zeros(code.k());
    for bit in 0..code.k() {
        message.set(bit, rng.gen());
    }
    let codeword = code
        .encode(&message)
        .expect("a k-bit message encodes under the pinned code");
    let llrs = channel.transmit_and_demodulate(
        &codeword,
        eb_n0_db,
        code.k() as f64 / code.n() as f64,
        rng,
    );
    let result = decoder
        .decode(&llrs)
        .expect("pinned eBCH dimensions must match the OSD decoder");
    let decoder_work = result.work();
    let work = OsdWorkCounters {
        eliminations: decoder_work.eliminations() as u64,
        generated_patterns: decoder_work.generated_patterns() as u64,
        tested_candidates: decoder_work.tested_candidates() as u64,
    };
    let decoded = result
        .into_decoded_bits()
        .expect("an uncapped OSD run tests its order-zero candidate");
    let bit_errors = (0..code.k())
        .filter(|&bit| decoded.get(bit) != message.get(bit))
        .count() as u64;
    FrameResult { bit_errors, work }
}

fn campaign_provenance(invocation: Vec<String>) -> Result<OsdCampaignProvenance, String> {
    let repository = repository_top_level(Path::new(env!("CARGO_MANIFEST_DIR")))
        .map_err(|error| error.to_string())?;
    let configuration = artifact_identity(
        &repository,
        "dev/reference_data/osd_ebch_128_64_fossorier1994.md",
    )?;
    let measurement_behavior = artifact_identity(
        &repository,
        "crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs",
    )?;
    let placeholder_revision = "0000000000000000000000000000000000000000"
        .parse::<GitRevision>()
        .expect("static revision");
    let cpu = observe_cpu_identity().map_err(|error| error.to_string())?;
    let runtime = observe_provenance(
        &repository,
        Provenance {
            git_revision: placeholder_revision,
            binary_sha256: None,
            deps_source_revision: None,
            deps_source_dirty: None,
            compiler_version: rustc_version(),
            rng_algorithm: RngAlgorithm::ChaCha20,
            rng_version: "rand_chacha 0.3.1 (gf2-coding BPSK channel ABI)".to_owned(),
            invocation,
            accelerator_runtime: Availability::NotPresent,
            cpu_model: cpu.model,
            cpu_physical_cores: Some(cpu.physical_cores),
            cpu_logical_threads: Some(cpu.logical_threads),
            gpu_model: Availability::NotPresent,
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(OsdCampaignProvenance {
        runtime,
        configuration,
        measurement_behavior,
    })
}

fn artifact_identity(repository: &Path, path: &str) -> Result<ArtifactIdentity, String> {
    let bytes = fs::read(repository.join(path)).map_err(|error| format!("{path}: {error}"))?;
    let digest = Sha256::digest(bytes);
    let sha256 = format!("{digest:x}")
        .parse::<Sha256Digest>()
        .map_err(|error| format!("{path}: invalid SHA-256 digest: {error}"))?;
    Ok(ArtifactIdentity {
        path: path
            .parse()
            .map_err(|error| format!("{path}: invalid artifact path: {error}"))?,
        sha256,
    })
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "rustc unavailable".to_owned())
}

fn write_receipt(
    path: &Path,
    receipt: &gf2_sim::osd_campaign::OsdCampaignReceipt,
) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| format!("create receipt directory: {error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("serialize receipt: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("write receipt {}: {error}", path.display()))
}
