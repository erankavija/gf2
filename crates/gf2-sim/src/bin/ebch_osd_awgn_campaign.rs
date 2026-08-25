//! Run the pinned eBCH OSD reference campaign through `gf2-sim`'s reusable
//! checkpointed protocol.
//!
//! The executable owns only the domain binding: the named
//! [`ExtendedBchCode::ebch_128_64`] factory, the BI-AWGN/BPSK channel, the
//! order-2 target and order-1 control cells, and this evaluator's bounded
//! stopping controls.  Cell identity, deterministic seed derivation,
//! checkpoint validation, BER/BLER intervals, receipt schema, and resume
//! history remain in [`gf2_sim::osd_campaign`].
//!
//! # Usage
//!
//! ```text
//! ebch_osd_awgn_campaign --checkpoint PATH --receipt PATH [OPTIONS]
//! ```
//!
//! The campaign is the eBCH(128,64,22) code over BI-AWGN with BPSK at rate
//! 1/2.  Its reported comparison metric is BER versus Eb/N0 in dB; BLER is
//! recorded as a companion quantity.  `--checkpoint` names the protocol's
//! resumable progress file and `--receipt` names the versioned JSON receipt.
//! The pinned grid is the seven Fossorier 1994 abscissas shared by the order-2
//! target and the order-1 internal control.
//!
//! Fossorier's reprocessing list is mapped to increasing Hamming weight over
//! the 64 MRI positions, with lexicographic ascending zero-based indices
//! within each weight.  The source leaves reliability ties and equal-distance
//! ties undefined: the shared decoder uses ascending original coordinate
//! index for equal reliability magnitudes and retains the first generated
//! candidate for an equal metric.  Those are implementation decisions, not
//! source claims.
//!
//! `--max-samples` is an additional per-invocation sample bound.  Reaching it
//! records an interrupted cell so a later invocation can continue from the
//! durable counters.  `--target-errors` is the cumulative information-bit
//! error target for a cell; reaching it records a completed cell.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::osd::{GeneratorMatrixOsdDecoder, OsdConfig};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel};
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use rand08::{Rng, SeedableRng};
use rand_chacha08::ChaCha20Rng;
use sha2::{Digest, Sha256};

use gf2_sim::osd_campaign::{
    BinomialIntervalMethod, BinomialIntervalSpec, OsdCampaign, OsdCampaignError,
    OsdCampaignProvenance, OsdCell, OsdCellExecution, OsdCellResume, OsdCellRun,
    OsdCellTermination, OsdWorkCounters,
};
use gf2_sim::permanent_campaign::provenance::{observe_provenance, repository_top_level};
use gf2_sim::permanent_campaign::schema::{
    ArtifactIdentity, Availability, GitRevision, Provenance, RngAlgorithm, Sha256Digest,
};

const PINNED_EB_N0_DB: [f64; 7] = [1.55, 2.22, 3.01, 3.47, 3.98, 4.56, 5.23];
const ORDER_2_DIGITIZATION_PRECISION: [f64; 7] = [0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
const ORDER_1_DIGITIZATION_PRECISION: [f64; 7] = [0.1; 7];
const DEFAULT_SEED: u64 = 0xC832_2EFF;
const DEFAULT_MAX_SAMPLES: u64 = 100_000;
const DEFAULT_TARGET_ERRORS: u64 = 100;

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
  --target-errors N       Cumulative information-bit errors per cell [default: 100]
  --help                  Show this message

OSD list and tie policy:
  ascending Hamming weight over the MRI positions, then lexicographic
  ascending zero-based indices within a weight; source-undefined reliability
  ties use ascending original coordinate index, and source-undefined distance
  ties retain the first generated candidate (implementation decisions).

The library owns the versioned receipt schema, Clopper-Pearson intervals,
deterministic cell seeds, checkpoint validation, and resume history.";

#[derive(Debug)]
struct Args {
    checkpoint: PathBuf,
    receipt: PathBuf,
    seed: u64,
    max_samples: u64,
    target_errors: u64,
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

fn run() -> Result<(), String> {
    let Some(args) = parse_args()? else {
        println!("{USAGE}");
        return Ok(());
    };
    let provenance = campaign_provenance()?;
    let campaign = pinned_campaign(args.seed, provenance).map_err(|error| error.to_string())?;
    let code = ExtendedBchCode::ebch_128_64();
    let receipt = gf2_sim::osd_campaign::run_osd_campaign(&args.checkpoint, &campaign, |context| {
        evaluate_cell(context, &code, args.max_samples, args.target_errors)
    })
    .map_err(|error| error.to_string())?;
    write_receipt(&args.receipt, &receipt)?;
    println!(
        "wrote {} cell attempts to checkpoint {} and receipt {}",
        receipt.cell_results.len(),
        args.checkpoint.display(),
        args.receipt.display()
    );
    Ok(())
}

/// Parses the small domain-specific command line without introducing a CLI
/// dependency into the simulation crate.
fn parse_args() -> Result<Option<Args>, String> {
    let mut checkpoint = None;
    let mut receipt = None;
    let mut seed = DEFAULT_SEED;
    let mut max_samples = DEFAULT_MAX_SAMPLES;
    let mut target_errors = DEFAULT_TARGET_ERRORS;
    let arguments: Vec<String> = std::env::args().skip(1).collect();
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
            "--target-errors" => {
                let value = required_value(&arguments, &mut index, "--target-errors")?;
                target_errors = value
                    .parse()
                    .map_err(|_| format!("--target-errors must be an integer, got {value:?}"))?;
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown option {option:?}\n\n{USAGE}"));
            }
            positional => return Err(format!("unexpected positional argument {positional:?}")),
        }
        index += 1;
    }

    Ok(Some(Args {
        checkpoint: checkpoint
            .ok_or_else(|| "--checkpoint is required".to_owned())
            .map(PathBuf::from)?,
        receipt: receipt
            .ok_or_else(|| "--receipt is required".to_owned())
            .map(PathBuf::from)?,
        seed,
        max_samples,
        target_errors,
    }))
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
    let interval = BinomialIntervalSpec::new(BinomialIntervalMethod::ClopperPearson, 0.95)?;
    OsdCampaign::new(seed, cells, interval, provenance)
}

fn evaluate_cell(
    context: OsdCellExecution<'_>,
    code: &ExtendedBchCode,
    max_samples: u64,
    target_errors: u64,
) -> OsdCellRun {
    let decoder = GeneratorMatrixOsdDecoder::new(
        code.clone(),
        OsdConfig::new(usize::from(context.cell.osd_order)),
    );
    let channel = BpskAwgnChannel;
    let mut rng = ChaCha20Rng::seed_from_u64(context.seed);

    // Replaying the already durable prefix advances the same ChaCha20 stream
    // that an uninterrupted evaluator would have consumed.  The protocol
    // supplies the counters; this closure supplies only domain work.
    for _ in 0..context.resume.samples {
        let _ = simulate_frame(code, &decoder, &channel, context.cell.eb_n0_db, &mut rng);
    }

    let mut samples = context.resume.samples;
    let mut bit_errors = context.resume.bit_errors;
    let mut block_errors = context.resume.block_errors;
    let mut work = context.resume.work;
    for _ in 0..max_samples {
        let frame = simulate_frame(code, &decoder, &channel, context.cell.eb_n0_db, &mut rng);
        samples = samples.checked_add(1).expect("sample counter overflow");
        bit_errors = bit_errors
            .checked_add(frame.bit_errors)
            .expect("bit-error counter overflow");
        block_errors = block_errors
            .checked_add(u64::from(frame.block_error))
            .expect("block-error counter overflow");
        work.eliminations = work
            .eliminations
            .checked_add(frame.work.eliminations)
            .expect("elimination counter overflow");
        work.generated_patterns = work
            .generated_patterns
            .checked_add(frame.work.generated_patterns)
            .expect("pattern counter overflow");
        work.tested_candidates = work
            .tested_candidates
            .checked_add(frame.work.tested_candidates)
            .expect("candidate counter overflow");

        if bit_errors >= target_errors {
            return cell_run(
                samples,
                code.k(),
                bit_errors,
                block_errors,
                work,
                OsdCellTermination::Completed,
                context.resume,
            );
        }
    }

    cell_run(
        samples,
        code.k(),
        bit_errors,
        block_errors,
        work,
        OsdCellTermination::Interrupted,
        context.resume,
    )
}

fn cell_run(
    samples: u64,
    information_bits: usize,
    bit_errors: u64,
    block_errors: u64,
    work: OsdWorkCounters,
    termination: OsdCellTermination,
    resume: OsdCellResume,
) -> OsdCellRun {
    let added_samples = samples
        .checked_sub(resume.samples)
        .expect("sample counter must be cumulative");
    let sampled_bits = resume
        .sampled_bits
        .checked_add(
            added_samples
                .checked_mul(information_bits as u64)
                .expect("sampled-bit counter overflow"),
        )
        .expect("sampled-bit counter overflow");
    OsdCellRun {
        samples,
        sampled_bits,
        bit_errors,
        block_errors,
        work,
        termination,
    }
}

struct FrameResult {
    bit_errors: u64,
    block_error: bool,
    work: OsdWorkCounters,
}

fn simulate_frame(
    code: &ExtendedBchCode,
    decoder: &GeneratorMatrixOsdDecoder<ExtendedBchCode>,
    channel: &BpskAwgnChannel,
    eb_n0_db: f64,
    rng: &mut ChaCha20Rng,
) -> FrameResult {
    let mut message = BitVec::zeros(code.k());
    for bit in 0..code.k() {
        message.set(bit, rng.gen());
    }
    let codeword = code.encode(&message);
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
    FrameResult {
        bit_errors,
        block_error: bit_errors != 0,
        work,
    }
}

fn campaign_provenance() -> Result<OsdCampaignProvenance, String> {
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
            invocation: vec!["ebch_osd_awgn_campaign".to_owned()],
            accelerator_runtime: Availability::NotPresent,
            cpu_model: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
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
