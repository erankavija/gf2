//! Configurable simulation campaign runner.
//!
//! Parses a TOML campaign config defining curves (LDPC, product code, GLDPC)
//! and runs them via the existing [`SimulationRunner`] infrastructure.
//!
//! # Usage
//!
//! ```bash
//! # Dry run -- verify parsing
//! cargo run -p gf2-coding --release --all-features --bin sim_runner -- \
//!     dev/campaigns/phase1_fig3.toml --dry-run
//!
//! # Run single curve
//! cargo run -p gf2-coding --release --all-features --bin sim_runner -- \
//!     dev/campaigns/phase1_fig3.toml --curve fig3_ldpc_nms --parallel
//!
//! # Run all curves in a campaign
//! cargo run -p gf2-coding --release --all-features --bin sim_runner -- \
//!     dev/campaigns/phase1_fig3.toml --parallel
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::crc::CrcCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
use gf2_coding::gldpc::{GldpcDecoder, GldpcDecoderConfig, QcGldpcCode};
use gf2_coding::grand::OrbGrandConfig;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::product::{
    ChasePyndiahConfig, ChasePyndiahDecoder, ProductCode, TurboDecoder, TurboDecoderConfig,
};
use gf2_coding::simulation::{
    BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationResults, SimulationRunner,
};

#[cfg(feature = "parallel")]
use gf2_coding::simulation::SimulationResult;
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Channel selection
// ---------------------------------------------------------------------------

/// TOML sub-table for per-curve channel selection.
///
/// ```toml
/// channel = { kind = "rician", preset = "fig8" }
/// ```
///
/// Omitting the `channel` key defaults to BPSK/AWGN (existing behaviour).
#[derive(Debug, Deserialize)]
struct ChannelToml {
    /// `"awgn"` (default) or `"rician"`.
    kind: String,
    /// Required when `kind = "rician"`: one of `"fig8"`, `"fig9"`, `"fig10"`.
    preset: Option<String>,
}

/// Unified channel variant that dispatches to either the BPSK/AWGN or the
/// QPSK/Rician path.  Wrapping both in an enum lets every helper function
/// remain generic (`impl ChannelModel`) without needing a trait object.
enum AnyChannel {
    Awgn(BpskAwgnChannel),
    Rician(QpskRicianChannelModel),
}

impl ChannelModel for AnyChannel {
    fn batch_alignment(&self) -> usize {
        match self {
            AnyChannel::Awgn(c) => c.batch_alignment(),
            AnyChannel::Rician(c) => c.batch_alignment(),
        }
    }

    fn demap_method(&self) -> gf2_coding::modem::DemapMethod {
        match self {
            AnyChannel::Awgn(c) => c.demap_method(),
            AnyChannel::Rician(c) => c.demap_method(),
        }
    }

    fn transmit_and_demodulate<R: rand::Rng>(
        &self,
        bits: &gf2_core::BitVec,
        eb_n0_db: f64,
        rate: f64,
        rng: &mut R,
    ) -> Vec<gf2_coding::llr::Llr> {
        match self {
            AnyChannel::Awgn(c) => c.transmit_and_demodulate(bits, eb_n0_db, rate, rng),
            AnyChannel::Rician(c) => c.transmit_and_demodulate(bits, eb_n0_db, rate, rng),
        }
    }
}

/// Builds an `AnyChannel` from an optional `ChannelToml`.  Returns
/// `AnyChannel::Awgn` when the TOML key is absent.
fn build_channel(cfg: Option<&ChannelToml>) -> Result<AnyChannel, String> {
    match cfg {
        None => Ok(AnyChannel::Awgn(BpskAwgnChannel)),
        Some(ch) => match ch.kind.as_str() {
            "awgn" => Ok(AnyChannel::Awgn(BpskAwgnChannel)),
            "rician" => {
                let preset = ch
                    .preset
                    .as_deref()
                    .ok_or("rician channel requires `preset`")?;
                let rician_cfg = match preset {
                    "fig8" => RicianConfig::fig8(),
                    "fig9" => RicianConfig::fig9(),
                    "fig10" => RicianConfig::fig10(),
                    other => return Err(format!("unknown rician preset: {other}")),
                };
                Ok(AnyChannel::Rician(QpskRicianChannelModel::new(rician_cfg)))
            }
            other => Err(format!("unknown channel kind: {other}")),
        },
    }
}

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

/// Top-level campaign configuration, deserialized from a TOML file.
#[derive(Debug, Deserialize)]
struct CampaignConfig {
    /// Campaign-level metadata.
    campaign: CampaignMeta,
    /// One or more simulation curves to run.
    curve: Vec<CurveConfig>,
}

/// Campaign-level metadata (name, output directory).
#[derive(Debug, Deserialize)]
struct CampaignMeta {
    /// Human-readable campaign name (used in log output).
    name: String,
    /// Directory where result CSV/JSON files are written.
    output_dir: String,
}

/// The type of error-correcting code for a curve.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum CurveType {
    Ldpc,
    Product,
    Gldpc,
}

/// Configuration for a single simulation curve.
#[derive(Debug, Deserialize)]
struct CurveConfig {
    /// Unique name used in output file names and `--curve` filtering.
    name: String,
    /// Code type: `"ldpc"`, `"product"`, or `"gldpc"`.
    #[serde(rename = "type")]
    curve_type: CurveType,

    // -- LDPC fields (optional) --
    /// 5G NR base graph (1 or 2).
    base_graph: Option<u8>,
    /// Target codeword length after rate matching.
    n: Option<usize>,
    /// Target message length after rate matching.
    k: Option<usize>,
    /// Decoder algorithm: `"nms"`, `"min_sum"`, `"sum_product"`, `"offset_ms"`.
    algorithm: Option<String>,
    /// Scaling factor for NMS / offset for offset-MS.
    scale: Option<f32>,

    // -- Product fields (optional) --
    /// Component code name: `"ebch_16_11"`, `"ebch_16_7"`, `"ebch_32_26"`,
    /// `"ebch_64_57"`, `"drm_32_21"`, `"drm_32_21_dynamic"`.
    component: Option<String>,
    /// Turbo decoder configuration.
    turbo: Option<TurboConfig>,

    // -- GLDPC fields (optional) --
    /// GLDPC code variant: `"lentmaier_1024"`.
    variant: Option<String>,
    /// SOGRAND configuration for GLDPC check-node decoder.
    sogrand: Option<SograndConfig>,

    // -- Common --
    /// Optional channel selection; omit for BPSK/AWGN (default).
    channel: Option<ChannelToml>,
    /// Eb/N0 sweep range.
    snr: SnrRange,
    /// Minimum number of frame errors to collect per SNR point.
    min_errors: usize,
    /// Maximum number of frames to transmit per SNR point.
    max_frames: usize,
}

/// Eb/N0 sweep specification (inclusive start/stop, additive step).
#[derive(Debug, Deserialize)]
struct SnrRange {
    start: f64,
    stop: f64,
    step: f64,
}

impl SnrRange {
    /// Expands the range into a `Vec<f64>` of SNR points.
    ///
    /// # Panics
    ///
    /// Panics if `step` is not positive.
    fn to_points(&self) -> Vec<f64> {
        assert!(
            self.step > 0.0,
            "SNR step must be positive, got {}",
            self.step
        );
        let mut points = Vec::new();
        let mut val = self.start;
        while val <= self.stop + self.step * 0.01 {
            // Round to avoid float drift (e.g., 0.5000000000000001).
            points.push((val * 1000.0).round() / 1000.0);
            val += self.step;
        }
        points
    }
}

/// Turbo decoder parameters embedded in a product-code curve.
#[derive(Debug, Deserialize)]
struct TurboConfig {
    max_iterations: usize,
    alpha: f32,
    /// Final alpha for iteration-dependent schedule (Chase-Pyndiah style).
    alpha_final: Option<f32>,
    /// Maximum absolute extrinsic LLR value.
    extrinsic_clamp: Option<f32>,
    list_size: usize,
    max_queries: usize,
    /// Paper-aligned per-component list-BLER early-stop threshold.
    ///
    /// When set, each SOGRAND component decode exits as soon as the list
    /// has `list_size` codewords OR the predicted list-BLER drops below
    /// this value. The turbo loop itself still terminates only when every
    /// row and every column of the hard decision is a valid component
    /// codeword (paper § V, step 1); the threshold is not applied at the
    /// turbo level. Typical values: `1e-4` for AWGN product codes,
    /// `1e-5` for the GLDPC configuration (see SO-GRAND paper Figs 1/7/8).
    list_bler_threshold: Option<f64>,
    /// Disable early termination (always run max_iterations).
    no_early_termination: Option<bool>,
    /// Use Pyndiah-style extrinsic: L_E = L_APP - L_Ch (not subtracting L_A).
    pyndiah_extrinsic: Option<bool>,
    /// Use BCJR trellis decoder instead of SOGRAND for component SISO.
    use_bcjr: Option<bool>,
    /// Use GPU-accelerated batch BCJR via HIP/ROCm.
    #[cfg(feature = "hip")]
    use_gpu_bcjr: Option<bool>,
    /// Use Chase-Pyndiah decoder instead of SOGRAND/BCJR turbo decoder.
    chase_pyndiah: Option<ChasePyndiahToml>,
}

/// Chase-Pyndiah decoder parameters (optional override in TOML).
#[derive(Debug, Deserialize)]
struct ChasePyndiahToml {
    /// Chase search depth (number of least reliable positions).
    p: Option<usize>,
    /// Maximum turbo iteration pairs (overrides `TurboConfig::max_iterations`).
    max_iterations: Option<usize>,
}

/// SOGRAND check-node decoder parameters for GLDPC curves.
#[derive(Debug, Deserialize)]
struct SograndConfig {
    list_size: usize,
    max_queries: usize,
    /// Enable even-code parity optimization (halves search space for codes
    /// where all codewords have even weight, such as extended BCH).
    #[serde(default)]
    even_code: bool,
    /// Extrinsic damping factor for check-to-variable messages (default 0.7).
    #[serde(default = "default_alpha")]
    alpha: f32,
    /// Maximum absolute LLR value for variable-node beliefs (default 25.0).
    #[serde(default = "default_saturation")]
    llr_saturation: f32,
}

fn default_alpha() -> f32 {
    0.7
}

fn default_saturation() -> f32 {
    25.0
}

// ---------------------------------------------------------------------------
// CLI parsing
// ---------------------------------------------------------------------------

/// Parsed command-line arguments.
struct CliArgs {
    /// Path to the TOML campaign file (first positional argument).
    toml_path: String,
    /// Curve name filter (may be specified more than once).
    curves: Vec<String>,
    /// Use parallel SNR sweep (rayon).
    parallel: bool,
    /// Override RNG seed.
    seed: u64,
    /// Print what would run without executing.
    dry_run: bool,
}

fn parse_args() -> Result<CliArgs, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err(
            "Usage: sim_runner <campaign.toml> [--curve <name>]... [--parallel] [--seed <n>] [--dry-run]"
                .to_string(),
        );
    }

    let mut toml_path: Option<String> = None;
    let mut curves = Vec::new();
    let mut parallel = false;
    let mut seed: u64 = 42;
    let mut dry_run = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--curve" => {
                i += 1;
                if i >= args.len() {
                    return Err("--curve requires a value".to_string());
                }
                curves.push(args[i].clone());
            }
            "--parallel" => parallel = true,
            "--dry-run" => dry_run = true,
            "--seed" => {
                i += 1;
                if i >= args.len() {
                    return Err("--seed requires a value".to_string());
                }
                seed = args[i].parse().map_err(|e| format!("invalid seed: {e}"))?;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option: {other}"));
            }
            positional => {
                if toml_path.is_some() {
                    return Err(format!("unexpected positional argument: {positional}"));
                }
                toml_path = Some(positional.to_string());
            }
        }
        i += 1;
    }

    let toml_path =
        toml_path.ok_or_else(|| "missing required <campaign.toml> argument".to_string())?;

    Ok(CliArgs {
        toml_path,
        curves,
        parallel,
        seed,
        dry_run,
    })
}

// ---------------------------------------------------------------------------
// TOML loading
// ---------------------------------------------------------------------------

/// Loads and parses a campaign TOML file.
fn load_campaign(path: &str) -> Result<CampaignConfig, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    toml::from_str(&content).map_err(|e| format!("failed to parse {path}: {e}"))
}

// ---------------------------------------------------------------------------
// Decoder algorithm resolution
// ---------------------------------------------------------------------------

/// Maps a string algorithm name + optional scale to a `DecoderAlgorithm`.
fn resolve_algorithm(name: &str, scale: Option<f32>) -> Result<DecoderAlgorithm, String> {
    match name {
        "nms" => Ok(DecoderAlgorithm::NormalizedMinSum(scale.unwrap_or(0.75))),
        "min_sum" => Ok(DecoderAlgorithm::MinSum),
        "sum_product" => Ok(DecoderAlgorithm::SumProduct),
        "offset_ms" => {
            let beta = scale.ok_or("offset_ms requires a `scale` (beta) value")?;
            Ok(DecoderAlgorithm::OffsetMinSum(beta))
        }
        other => Err(format!("unknown algorithm: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Curve execution
// ---------------------------------------------------------------------------

/// Builds a `SimulationConfig` from a `CurveConfig` and CLI overrides.
fn build_sim_config(curve: &CurveConfig, output_dir: &str, seed: u64) -> SimulationConfig {
    let snr_points = curve.snr.to_points();
    let output_csv = format!("{}/{}.csv", output_dir, curve.name);
    SimulationConfig {
        eb_n0_range_db: snr_points,
        min_errors: curve.min_errors,
        max_frames: curve.max_frames,
        max_decoder_iterations: 50,
        rng_seed: Some(seed),
        output_path: Some(PathBuf::from(output_csv)),
    }
}

/// Runs a single curve according to its type, returning the simulation results.
fn run_curve(
    curve: &CurveConfig,
    output_dir: &str,
    parallel: bool,
    seed: u64,
) -> Result<SimulationResults, String> {
    let config = build_sim_config(curve, output_dir, seed);
    let channel = build_channel(curve.channel.as_ref())?;

    let results = match curve.curve_type {
        CurveType::Ldpc => {
            let bg = curve.base_graph.ok_or("ldpc curve requires `base_graph`")?;
            let n = curve.n.ok_or("ldpc curve requires `n`")?;
            let k = curve.k.ok_or("ldpc curve requires `k`")?;
            let alg_name = curve
                .algorithm
                .as_deref()
                .ok_or("ldpc curve requires `algorithm`")?;
            let algorithm = resolve_algorithm(alg_name, curve.scale)?;

            let rm_code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);

            if parallel {
                let rm_f = rm_code.clone();
                SimulationRunner::run_coded_iterative_parallel(
                    &rm_code,
                    move || Nr5gRateMatchedDecoder::with_algorithm(rm_f.clone(), algorithm),
                    &channel,
                    &config,
                )
            } else {
                let mut decoder =
                    Nr5gRateMatchedDecoder::with_algorithm(rm_code.clone(), algorithm);
                SimulationRunner::run_coded_iterative(&rm_code, &mut decoder, &channel, &config)
            }
        }
        CurveType::Product => {
            let comp_name = curve
                .component
                .as_deref()
                .ok_or("product curve requires `component`")?;
            let turbo_cfg = curve
                .turbo
                .as_ref()
                .ok_or("product curve requires `turbo` config")?;

            // Dispatch on component name. We need two instances: one for the
            // ProductCode encoder and one for the TurboDecoder.
            match comp_name {
                "ebch_16_11" => run_product(
                    ExtendedBchCode::ebch_16_11(),
                    ExtendedBchCode::ebch_16_11(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_16_7" => run_product(
                    ExtendedBchCode::ebch_16_7(),
                    ExtendedBchCode::ebch_16_7(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_32_26" => run_product(
                    ExtendedBchCode::ebch_32_26(),
                    ExtendedBchCode::ebch_32_26(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_64_57" => run_product(
                    ExtendedBchCode::ebch_64_57(),
                    ExtendedBchCode::ebch_64_57(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "crc_25_15" => run_product(
                    CrcCode::crc_25_15(),
                    CrcCode::crc_25_15(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "drm_32_21" => run_product(
                    DrmCode::drm_32_21(),
                    DrmCode::drm_32_21(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "drm_32_21_dynamic" => run_product(
                    DrmCode::drm_32_21_dynamic(),
                    DrmCode::drm_32_21_dynamic(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                other => return Err(format!("unknown component: {other}")),
            }
        }
        CurveType::Gldpc => {
            let variant = curve
                .variant
                .as_deref()
                .ok_or("gldpc curve requires `variant`")?;
            let code = match variant {
                "lentmaier_1024" => QcGldpcCode::lentmaier_1024(),
                other => return Err(format!("unknown gldpc variant: {other}")),
            };

            let make_decoder = |code: QcGldpcCode| -> GldpcDecoder {
                if let Some(sc) = &curve.sogrand {
                    let orb_config = OrbGrandConfig {
                        list_size: sc.list_size,
                        max_queries: sc.max_queries,
                        even_code: sc.even_code,
                        ..OrbGrandConfig::default()
                    };
                    let dec_config = GldpcDecoderConfig {
                        alpha: sc.alpha,
                        llr_saturation: sc.llr_saturation,
                    };
                    GldpcDecoder::with_config(code, orb_config, dec_config)
                } else {
                    GldpcDecoder::new(code)
                }
            };

            if parallel {
                let code_f = code.clone();
                let sogrand_cfg = curve.sogrand.as_ref().map(|s| {
                    (
                        s.list_size,
                        s.max_queries,
                        s.even_code,
                        s.alpha,
                        s.llr_saturation,
                    )
                });
                SimulationRunner::run_coded_iterative_parallel(
                    &code,
                    move || {
                        if let Some((ls, mq, ec, alpha, llr_sat)) = sogrand_cfg {
                            let orb_config = OrbGrandConfig {
                                list_size: ls,
                                max_queries: mq,
                                even_code: ec,
                                ..OrbGrandConfig::default()
                            };
                            let dec_config = GldpcDecoderConfig {
                                alpha,
                                llr_saturation: llr_sat,
                            };
                            GldpcDecoder::with_config(code_f.clone(), orb_config, dec_config)
                        } else {
                            GldpcDecoder::new(code_f.clone())
                        }
                    },
                    &channel,
                    &config,
                )
            } else {
                let mut decoder = make_decoder(code.clone());
                SimulationRunner::run_coded_iterative(&code, &mut decoder, &channel, &config)
            }
        }
    };

    Ok(results)
}

/// Builds a `TurboDecoderConfig` from the TOML turbo parameters.
fn build_turbo_decoder_config(turbo_cfg: &TurboConfig) -> TurboDecoderConfig {
    TurboDecoderConfig {
        max_iterations: turbo_cfg.max_iterations,
        alpha: turbo_cfg.alpha,
        alpha_final: turbo_cfg.alpha_final,
        extrinsic_clamp: turbo_cfg.extrinsic_clamp,
        list_size: turbo_cfg.list_size,
        max_queries: turbo_cfg.max_queries,
        list_bler_threshold: turbo_cfg.list_bler_threshold,
        no_early_termination: turbo_cfg.no_early_termination.unwrap_or(false),
        pyndiah_extrinsic: turbo_cfg.pyndiah_extrinsic.unwrap_or(false),
        use_bcjr: turbo_cfg.use_bcjr.unwrap_or(false),
        #[cfg(feature = "hip")]
        use_gpu_bcjr: turbo_cfg.use_gpu_bcjr.unwrap_or(false),
    }
}

/// Builds a `ChasePyndiahConfig` from optional TOML overrides.
fn build_chase_pyndiah_config(cp_toml: &ChasePyndiahToml) -> ChasePyndiahConfig {
    let mut cp_config = ChasePyndiahConfig::default();
    if let Some(p) = cp_toml.p {
        cp_config.p = p;
    }
    if let Some(iters) = cp_toml.max_iterations {
        cp_config.max_iterations = iters;
    }
    cp_config
}

/// Helper: runs a product-code curve for any `ProductComponent` and channel type.
fn run_product<C, CH>(
    encoder_component: C,
    decoder_component: C,
    turbo_cfg: &TurboConfig,
    channel: &CH,
    config: &SimulationConfig,
    parallel: bool,
) -> SimulationResults
where
    C: gf2_coding::product::ProductComponent + Clone + Send + Sync + 'static,
    CH: ChannelModel + Sync,
{
    let product = ProductCode::new(encoder_component);

    #[cfg(feature = "parallel")]
    if parallel {
        return run_product_frame_parallel(product, decoder_component, turbo_cfg, channel, config);
    }

    #[cfg(not(feature = "parallel"))]
    let _ = parallel;

    if let Some(cp_toml) = &turbo_cfg.chase_pyndiah {
        let cp_config = build_chase_pyndiah_config(cp_toml);
        let cp_decoder = ChasePyndiahDecoder::new(decoder_component, cp_config);
        SimulationRunner::run_with_decoder(
            &product,
            |llrs| cp_decoder.decode(llrs).into(),
            channel,
            config,
        )
    } else {
        let turbo_config = build_turbo_decoder_config(turbo_cfg);
        let turbo = TurboDecoder::new(decoder_component, turbo_config);
        SimulationRunner::run_with_decoder(
            &product,
            |llrs| turbo.decode(llrs).into(),
            channel,
            config,
        )
    }
}

/// Boxed decode closure used by per-thread decoder instances.
#[cfg(feature = "parallel")]
type ProductDecodeFn = Box<dyn FnMut(&[gf2_coding::llr::Llr]) -> gf2_coding::traits::DecoderResult>;

/// Frame-parallel product code simulation.
///
/// For each SNR point, frames are distributed across rayon worker threads.
/// Each worker creates its own decoder instance (via `map_init`) since
/// `TurboDecoder` and `ChasePyndiahDecoder` are not `Send`. Shared
/// atomic counters enable early stopping once enough frame errors are
/// collected.
///
/// SNR points are processed sequentially to preserve resume and progress
/// reporting semantics.
#[cfg(feature = "parallel")]
fn run_product_frame_parallel<C, CH>(
    product: ProductCode<C>,
    decoder_component: C,
    turbo_cfg: &TurboConfig,
    channel: &CH,
    config: &SimulationConfig,
) -> SimulationResults
where
    C: gf2_coding::product::ProductComponent + Clone + Send + Sync + 'static,
    CH: ChannelModel + Sync,
{
    use gf2_coding::simulation::count_bit_errors;
    use gf2_coding::traits::BlockEncoder;
    use gf2_core::BitVec;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use rayon::prelude::*;

    let n = product.n();
    let k = product.k();
    let rate = k as f64 / n as f64;
    let base_seed = config.rng_seed.unwrap_or(0xDEAD_BEEF);

    // Determine decoder variant from config.
    let is_chase_pyndiah = turbo_cfg.chase_pyndiah.is_some();
    let cp_config = turbo_cfg
        .chase_pyndiah
        .as_ref()
        .map(build_chase_pyndiah_config);
    let turbo_config = if !is_chase_pyndiah {
        Some(build_turbo_decoder_config(turbo_cfg))
    } else {
        None
    };

    let mut points = Vec::with_capacity(config.eb_n0_range_db.len());
    let mut completed_points: Vec<(f64, std::time::Duration, usize, f64)> = Vec::new();

    for (point_idx, &eb_n0_db) in config.eb_n0_range_db.iter().enumerate() {
        let point_start = std::time::Instant::now();

        // Shared atomic counters for cross-thread accumulation.
        let total_frames = AtomicUsize::new(0);
        let total_frame_errors = AtomicUsize::new(0);
        let total_bit_errors = AtomicUsize::new(0);
        let total_iterations = AtomicUsize::new(0);
        let total_queries = AtomicUsize::new(0);
        let stop_flag = AtomicBool::new(false);

        // Derive a unique per-point seed so different SNR points use
        // independent RNG streams regardless of execution order.
        let point_seed = base_seed.wrapping_add(point_idx as u64 * 1_000_000);

        // Clone values that need to move into the closure.
        let decoder_comp = decoder_component.clone();
        let cp_cfg = cp_config.clone();
        let turbo_cfg_inner = turbo_config.clone();

        // Process frames in parallel using rayon's map_init to create
        // one decoder per worker thread.
        (0..config.max_frames)
            .into_par_iter()
            .map_init(
                move || -> ProductDecodeFn {
                    if let Some(ref cp) = cp_cfg {
                        let dec = ChasePyndiahDecoder::new(decoder_comp.clone(), cp.clone());
                        Box::new(move |llrs| dec.decode(llrs).into())
                    } else {
                        let dec = TurboDecoder::new(
                            decoder_comp.clone(),
                            turbo_cfg_inner.clone().unwrap(),
                        );
                        Box::new(move |llrs| dec.decode(llrs).into())
                    }
                },
                |decode_fn, frame_idx| {
                    // Check early-stop before doing work.
                    if stop_flag.load(Ordering::Relaxed) {
                        return;
                    }

                    let mut rng = StdRng::seed_from_u64(point_seed + frame_idx as u64);
                    let message = BitVec::random(k, &mut rng);
                    let codeword = product.encode(&message);
                    let llrs = channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);

                    let result = decode_fn(&llrs);
                    let bit_errs = count_bit_errors(&message, &result.decoded_bits);

                    // Update shared accumulators.
                    total_frames.fetch_add(1, Ordering::Relaxed);
                    total_bit_errors.fetch_add(bit_errs, Ordering::Relaxed);
                    total_iterations.fetch_add(result.iterations, Ordering::Relaxed);
                    total_queries.fetch_add(
                        result.queries.unwrap_or(result.iterations),
                        Ordering::Relaxed,
                    );
                    if bit_errs > 0 {
                        let fe = total_frame_errors.fetch_add(1, Ordering::Relaxed) + 1;
                        if fe >= config.min_errors {
                            stop_flag.store(true, Ordering::Relaxed);
                        }
                    }
                },
            )
            .collect::<Vec<()>>();

        // Collect final counter values.
        let frames = total_frames.load(Ordering::Relaxed);
        let frame_errors = total_frame_errors.load(Ordering::Relaxed);
        let bit_errors = total_bit_errors.load(Ordering::Relaxed);
        let iterations = total_iterations.load(Ordering::Relaxed);
        let queries = total_queries.load(Ordering::Relaxed);
        let bits = frames * k;

        let ber = if bits > 0 {
            bit_errors as f64 / bits as f64
        } else {
            0.0
        };
        let bler = if frames > 0 {
            frame_errors as f64 / frames as f64
        } else {
            0.0
        };
        let avg_iterations = if frames > 0 {
            Some(iterations as f64 / frames as f64)
        } else {
            None
        };
        let avg_queries_per_bit = if bits > 0 {
            Some(queries as f64 / bits as f64)
        } else {
            None
        };

        let sim_result = SimulationResult {
            eb_n0_db,
            ber,
            bler,
            avg_iterations,
            avg_queries_per_bit,
            num_bits: bits,
            num_bit_errors: bit_errors,
            num_frames: frames,
            num_frame_errors: frame_errors,
        };

        let point_elapsed = point_start.elapsed();

        // Report point completion.
        let remaining: Vec<f64> = config.eb_n0_range_db[point_idx + 1..].to_vec();
        let secs = point_elapsed.as_secs();
        let elapsed_str = if secs >= 3600 {
            format!(
                "{}h{:02}m{:02}s",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        } else if secs >= 60 {
            format!("{}m{:02}s", secs / 60, secs % 60)
        } else {
            format!("{:.1}s", point_elapsed.as_secs_f64())
        };
        eprintln!(
            "[{:.1} dB] DONE: BER={:.2e} BLER={:.2e} ({} errs / {} frames) in {} [{} pts remain]",
            eb_n0_db,
            ber,
            bler,
            frame_errors,
            frames,
            elapsed_str,
            remaining.len(),
        );

        // Incremental CSV append.
        if let Some(ref path) = config.output_path {
            sim_result.append_csv_row_to(path);
        }

        completed_points.push((eb_n0_db, point_elapsed, frames, bler));
        points.push(sim_result);
    }

    let results = SimulationResults { points };
    // Final overwrite with clean, complete file.
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
}

// ---------------------------------------------------------------------------
// Dry-run display
// ---------------------------------------------------------------------------

/// Prints a summary of what would be run, without actually executing.
fn print_dry_run(campaign: &CampaignConfig, curves: &[&CurveConfig], seed: u64, parallel: bool) {
    println!("Campaign: {}", campaign.campaign.name);
    println!("Output:   {}", campaign.campaign.output_dir);
    println!("Seed:     {seed}");
    println!("Parallel: {parallel}");
    println!("Curves:   {}", curves.len());
    println!();
    for curve in curves {
        let snr_points = curve.snr.to_points();
        println!("  [{}]", curve.name);
        println!("    type:       {:?}", curve.curve_type);
        match curve.curve_type {
            CurveType::Ldpc => {
                println!(
                    "    base_graph: {}",
                    curve.base_graph.map_or("-".into(), |v| v.to_string())
                );
                println!(
                    "    n:          {}",
                    curve.n.map_or("-".into(), |v| v.to_string())
                );
                println!(
                    "    k:          {}",
                    curve.k.map_or("-".into(), |v| v.to_string())
                );
                println!(
                    "    algorithm:  {}",
                    curve.algorithm.as_deref().unwrap_or("-")
                );
                if let Some(s) = curve.scale {
                    println!("    scale:      {s}");
                }
            }
            CurveType::Product => {
                println!(
                    "    component:  {}",
                    curve.component.as_deref().unwrap_or("-")
                );
                if let Some(t) = &curve.turbo {
                    println!(
                        "    turbo:      iters={}, alpha={}, list={}, queries={}",
                        t.max_iterations, t.alpha, t.list_size, t.max_queries
                    );
                }
            }
            CurveType::Gldpc => {
                println!(
                    "    variant:    {}",
                    curve.variant.as_deref().unwrap_or("-")
                );
                if let Some(sc) = &curve.sogrand {
                    println!(
                        "    sogrand:    list={}, queries={}, even={}",
                        sc.list_size, sc.max_queries, sc.even_code
                    );
                    println!(
                        "    bp_config:  alpha={}, llr_sat={}",
                        sc.alpha, sc.llr_saturation
                    );
                }
            }
        }
        println!(
            "    snr:        {:.1} to {:.1} step {:.1} ({} points)",
            curve.snr.start,
            curve.snr.stop,
            curve.snr.step,
            snr_points.len()
        );
        println!("    min_errors: {}", curve.min_errors);
        println!("    max_frames: {}", curve.max_frames);
        println!(
            "    output:     {}/{}.csv",
            campaign.campaign.output_dir, curve.name
        );
        println!();
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let campaign = match load_campaign(&args.toml_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // Filter curves if --curve was specified.
    let selected: Vec<&CurveConfig> = if args.curves.is_empty() {
        campaign.curve.iter().collect()
    } else {
        campaign
            .curve
            .iter()
            .filter(|c| args.curves.contains(&c.name))
            .collect()
    };

    if selected.is_empty() {
        eprintln!("No matching curves found.");
        if !args.curves.is_empty() {
            let available: Vec<&str> = campaign.curve.iter().map(|c| c.name.as_str()).collect();
            eprintln!("Available curves: {}", available.join(", "));
        }
        std::process::exit(1);
    }

    if args.dry_run {
        print_dry_run(&campaign, &selected, args.seed, args.parallel);
        return;
    }

    // Ensure output directory exists.
    std::fs::create_dir_all(&campaign.campaign.output_dir).unwrap_or_else(|e| {
        panic!(
            "Failed to create output directory '{}': {e}",
            campaign.campaign.output_dir
        )
    });

    let total = selected.len();
    for (i, curve) in selected.iter().enumerate() {
        let snr_count = curve.snr.to_points().len();
        let type_str = format!("{:?}", curve.curve_type).to_lowercase();
        let detail = match curve.curve_type {
            CurveType::Product => curve
                .component
                .as_deref()
                .map_or(String::new(), |c| format!(", {c}")),
            CurveType::Gldpc => curve
                .variant
                .as_deref()
                .map_or(String::new(), |v| format!(", {v}")),
            CurveType::Ldpc => curve
                .algorithm
                .as_deref()
                .map_or(String::new(), |a| format!(", {a}")),
        };
        eprintln!(
            "[{}/{}] Running: {} ({}{}, {} SNR points)",
            i + 1,
            total,
            curve.name,
            type_str,
            detail,
            snr_count,
        );
        let curve_start = std::time::Instant::now();
        let results = match run_curve(
            curve,
            &campaign.campaign.output_dir,
            args.parallel,
            args.seed,
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error running curve {}: {e}", curve.name);
                std::process::exit(1);
            }
        };

        // Write JSON alongside the CSV output.
        let json_path = format!("{}/{}.json", campaign.campaign.output_dir, curve.name);
        results.write_to(Path::new(&json_path));

        let curve_elapsed = curve_start.elapsed();
        let secs = curve_elapsed.as_secs();
        let elapsed_str = if secs >= 3600 {
            format!(
                "{}h{:02}m{:02}s",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        } else if secs >= 60 {
            format!("{}m{:02}s", secs / 60, secs % 60)
        } else {
            format!("{secs}s")
        };
        eprintln!(
            "[{}/{}] Done: {} in {} (CSV + JSON: {}/{}.{{csv,json}})",
            i + 1,
            total,
            curve.name,
            elapsed_str,
            campaign.campaign.output_dir,
            curve.name,
        );
    }

    eprintln!(
        "Campaign '{}' complete ({total} curves).",
        campaign.campaign.name
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TOML: &str = r#"
[campaign]
name = "test_campaign"
output_dir = "/tmp/sim_test"

[[curve]]
name = "test_ldpc_nms"
type = "ldpc"
base_graph = 2
n = 256
k = 121
algorithm = "nms"
scale = 0.75
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000

[[curve]]
name = "test_ldpc_sp"
type = "ldpc"
base_graph = 2
n = 256
k = 121
algorithm = "sum_product"
snr = { start = 0.0, stop = 2.0, step = 1.0 }
min_errors = 50
max_frames = 10000

[[curve]]
name = "test_product"
type = "product"
component = "ebch_16_11"
turbo = { max_iterations = 20, alpha = 0.5, list_size = 4, max_queries = 1000000 }
snr = { start = 0.0, stop = 4.0, step = 0.5 }
min_errors = 100
max_frames = 500000

[[curve]]
name = "test_gldpc"
type = "gldpc"
variant = "lentmaier_1024"
snr = { start = 1.0, stop = 3.0, step = 0.5 }
min_errors = 10
max_frames = 1000
"#;

    #[test]
    fn test_parse_campaign_config() {
        let config: CampaignConfig = toml::from_str(SAMPLE_TOML).unwrap();
        assert_eq!(config.campaign.name, "test_campaign");
        assert_eq!(config.campaign.output_dir, "/tmp/sim_test");
        assert_eq!(config.curve.len(), 4);

        // LDPC NMS
        let ldpc = &config.curve[0];
        assert_eq!(ldpc.name, "test_ldpc_nms");
        assert_eq!(ldpc.curve_type, CurveType::Ldpc);
        assert_eq!(ldpc.base_graph, Some(2));
        assert_eq!(ldpc.n, Some(256));
        assert_eq!(ldpc.k, Some(121));
        assert_eq!(ldpc.algorithm.as_deref(), Some("nms"));
        assert_eq!(ldpc.scale, Some(0.75));
        assert_eq!(ldpc.min_errors, 100);
        assert_eq!(ldpc.max_frames, 500000);

        // LDPC SP
        let sp = &config.curve[1];
        assert_eq!(sp.curve_type, CurveType::Ldpc);
        assert_eq!(sp.algorithm.as_deref(), Some("sum_product"));
        assert!(sp.scale.is_none());

        // Product
        let prod = &config.curve[2];
        assert_eq!(prod.curve_type, CurveType::Product);
        assert_eq!(prod.component.as_deref(), Some("ebch_16_11"));
        let turbo = prod.turbo.as_ref().unwrap();
        assert_eq!(turbo.max_iterations, 20);
        assert!((turbo.alpha - 0.5).abs() < f32::EPSILON);
        assert_eq!(turbo.list_size, 4);
        assert_eq!(turbo.max_queries, 1_000_000);

        // GLDPC
        let gldpc = &config.curve[3];
        assert_eq!(gldpc.curve_type, CurveType::Gldpc);
        assert_eq!(gldpc.variant.as_deref(), Some("lentmaier_1024"));
    }

    #[test]
    fn test_snr_range_to_points() {
        let range = SnrRange {
            start: 0.0,
            stop: 4.0,
            step: 0.5,
        };
        let points = range.to_points();
        assert_eq!(points.len(), 9);
        assert!((points[0] - 0.0).abs() < 1e-9);
        assert!((points[4] - 2.0).abs() < 1e-9);
        assert!((points[8] - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_snr_range_single_point() {
        let range = SnrRange {
            start: 3.0,
            stop: 3.0,
            step: 0.5,
        };
        let points = range.to_points();
        assert_eq!(points.len(), 1);
        assert!((points[0] - 3.0).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "SNR step must be positive")]
    fn test_snr_range_zero_step_panics() {
        let range = SnrRange {
            start: 0.0,
            stop: 4.0,
            step: 0.0,
        };
        range.to_points();
    }

    #[test]
    #[should_panic(expected = "SNR step must be positive")]
    fn test_snr_range_negative_step_panics() {
        let range = SnrRange {
            start: 0.0,
            stop: 4.0,
            step: -0.5,
        };
        range.to_points();
    }

    #[test]
    fn test_resolve_algorithm_nms() {
        let alg = resolve_algorithm("nms", Some(0.8)).unwrap();
        match alg {
            DecoderAlgorithm::NormalizedMinSum(s) => assert!((s - 0.8).abs() < f32::EPSILON),
            _ => panic!("expected NormalizedMinSum"),
        }
    }

    #[test]
    fn test_resolve_algorithm_nms_default_scale() {
        let alg = resolve_algorithm("nms", None).unwrap();
        match alg {
            DecoderAlgorithm::NormalizedMinSum(s) => assert!((s - 0.75).abs() < f32::EPSILON),
            _ => panic!("expected NormalizedMinSum"),
        }
    }

    #[test]
    fn test_resolve_algorithm_sum_product() {
        let alg = resolve_algorithm("sum_product", None).unwrap();
        assert!(matches!(alg, DecoderAlgorithm::SumProduct));
    }

    #[test]
    fn test_resolve_algorithm_min_sum() {
        let alg = resolve_algorithm("min_sum", None).unwrap();
        assert!(matches!(alg, DecoderAlgorithm::MinSum));
    }

    #[test]
    fn test_resolve_algorithm_offset_ms() {
        let alg = resolve_algorithm("offset_ms", Some(0.3)).unwrap();
        match alg {
            DecoderAlgorithm::OffsetMinSum(b) => assert!((b - 0.3).abs() < f32::EPSILON),
            _ => panic!("expected OffsetMinSum"),
        }
    }

    #[test]
    fn test_resolve_algorithm_offset_ms_missing_scale() {
        let result = resolve_algorithm("offset_ms", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_algorithm_unknown() {
        let result = resolve_algorithm("foo_bar", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_sim_config() {
        let config: CampaignConfig = toml::from_str(SAMPLE_TOML).unwrap();
        let curve = &config.curve[0];
        let sim = build_sim_config(curve, "/tmp/out", 123);
        assert_eq!(sim.eb_n0_range_db.len(), 9);
        assert_eq!(sim.min_errors, 100);
        assert_eq!(sim.max_frames, 500000);
        assert_eq!(sim.rng_seed, Some(123));
        assert_eq!(
            sim.output_path.as_ref().unwrap().to_str().unwrap(),
            "/tmp/out/test_ldpc_nms.csv"
        );
    }

    #[test]
    fn test_run_curve_crc_25_15_product() {
        let config: CampaignConfig = toml::from_str(
            r#"
[campaign]
name = "crc_curve_test"
output_dir = "/tmp/ignored"

[[curve]]
name = "crc_25_15_smoke"
type = "product"
component = "crc_25_15"
turbo = { max_iterations = 1, alpha = 0.5, list_size = 1, max_queries = 1000 }
snr = { start = 4.0, stop = 4.0, step = 0.5 }
min_errors = 1
max_frames = 1
"#,
        )
        .unwrap();

        let output_dir =
            std::env::temp_dir().join(format!("gf2-sim-runner-crc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output_dir);
        std::fs::create_dir_all(&output_dir).unwrap();

        let results =
            run_curve(&config.curve[0], output_dir.to_str().unwrap(), false, 123).unwrap();

        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].ber.is_finite());
        assert!(results.points[0].bler.is_finite());
        assert!(output_dir.join("crc_25_15_smoke.csv").is_file());

        std::fs::remove_dir_all(&output_dir).unwrap();
    }

    /// Verifies that a product-code curve routed through the Rician fading
    /// channel (`channel = { kind = "rician", preset = "fig8" }`) produces
    /// monotonically decreasing BLER across the SNR sweep and that high-SNR
    /// BLER is strictly lower than low-SNR BLER — a basic sanity check that
    /// the fading path wires up correctly end-to-end.
    #[test]
    #[ignore = "sim: Rician product-code BLER decay across 3 SNR points, ~30-60 s"]
    fn test_run_curve_rician_product_bler_decays() {
        let toml_str = r#"
[campaign]
name = "rician_sanity"
output_dir = "/tmp/ignored"

[[curve]]
name = "rician_drm_sanity"
type = "product"
component = "drm_32_21"
turbo = { max_iterations = 5, alpha = 0.5, list_size = 2, max_queries = 200 }
channel = { kind = "rician", preset = "fig8" }
snr = { start = 2.0, stop = 8.0, step = 3.0 }
min_errors = 2
max_frames = 30
"#;
        let config: CampaignConfig = toml::from_str(toml_str).unwrap();

        let output_dir =
            std::env::temp_dir().join(format!("gf2-sim-runner-rician-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output_dir);
        std::fs::create_dir_all(&output_dir).unwrap();

        let results = run_curve(&config.curve[0], output_dir.to_str().unwrap(), false, 42).unwrap();

        // Expect three SNR points: 2.0, 5.0, 8.0 dB.
        assert_eq!(results.points.len(), 3);

        // All BLERs must be finite and in [0, 1].
        for pt in &results.points {
            assert!(
                pt.bler.is_finite(),
                "BLER is not finite at {} dB",
                pt.eb_n0_db
            );
            assert!(
                (0.0..=1.0).contains(&pt.bler),
                "BLER {} out of [0,1] at {} dB",
                pt.bler,
                pt.eb_n0_db
            );
        }

        // High-SNR BLER (8 dB) must be strictly below low-SNR BLER (2 dB).
        let bler_low = results.points[0].bler;
        let bler_high = results.points[2].bler;
        assert!(
            bler_high < bler_low,
            "Expected BLER to decay: low={bler_low:.4} high={bler_high:.4}"
        );

        std::fs::remove_dir_all(&output_dir).unwrap();
    }

    /// Verifies that the TOML parser correctly deserialises
    /// `channel = { kind = "rician", preset = "fig8" }` and that
    /// `build_channel` returns the Rician variant.
    #[test]
    fn test_build_channel_rician_preset_parsing() {
        let toml_str = r#"
kind = "rician"
preset = "fig8"
"#;
        let ch: ChannelToml = toml::from_str(toml_str).unwrap();
        assert_eq!(ch.kind, "rician");
        assert_eq!(ch.preset.as_deref(), Some("fig8"));

        let chan = build_channel(Some(&ch)).unwrap();
        assert!(matches!(chan, AnyChannel::Rician(_)));
    }

    /// Verifies that omitting the `channel` key defaults to BPSK/AWGN.
    #[test]
    fn test_build_channel_default_awgn() {
        let chan = build_channel(None).unwrap();
        assert!(matches!(chan, AnyChannel::Awgn(_)));
    }

    #[test]
    fn test_curve_filter_all() {
        let config: CampaignConfig = toml::from_str(SAMPLE_TOML).unwrap();
        let filter: Vec<String> = vec![];
        let selected: Vec<&CurveConfig> = if filter.is_empty() {
            config.curve.iter().collect()
        } else {
            config
                .curve
                .iter()
                .filter(|c| filter.contains(&c.name))
                .collect()
        };
        assert_eq!(selected.len(), 4);
    }

    #[test]
    fn test_curve_filter_specific() {
        let config: CampaignConfig = toml::from_str(SAMPLE_TOML).unwrap();
        let filter = ["test_ldpc_nms".to_string(), "test_gldpc".to_string()];
        let selected: Vec<&CurveConfig> = config
            .curve
            .iter()
            .filter(|c| filter.contains(&c.name))
            .collect();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "test_ldpc_nms");
        assert_eq!(selected[1].name, "test_gldpc");
    }

    #[test]
    fn test_curve_filter_no_match() {
        let config: CampaignConfig = toml::from_str(SAMPLE_TOML).unwrap();
        let filter = ["nonexistent".to_string()];
        let selected: Vec<&CurveConfig> = config
            .curve
            .iter()
            .filter(|c| filter.contains(&c.name))
            .collect();
        assert!(selected.is_empty());
    }
}
