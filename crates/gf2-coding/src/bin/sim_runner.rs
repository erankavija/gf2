//! Simulation campaign runner: reads a TOML campaign of LDPC, product-code and
//! GLDPC curves and runs each through [`SimulationRunner`].

#![deny(unsafe_code)]

use serde::Deserialize;
#[cfg(feature = "parallel")]
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use gf2_coding::crc::CrcCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::fading::{QpskRicianChannelModel, RicianConfig};
use gf2_coding::gldpc::{GldpcDecoder, GldpcDecoderConfig, QcGldpcCode};
use gf2_coding::grand::OrbGrandConfig;
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::product::{
    ChasePyndiahConfig, ChasePyndiahDecoder, ExtendedBchComponent, ProductCode, TurboDecoder,
    TurboDecoderConfig,
};
use gf2_coding::simulation::{
    BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationResults, SimulationRunner,
};

#[cfg(feature = "parallel")]
use gf2_coding::simulation::SimulationResult;

/// TOML channel parameters for one curve.
#[derive(Debug, Deserialize)]
struct ChannelToml {
    /// `"awgn"` or `"rician"`.
    kind: String,
    /// Rician LOS-to-scatter power ratio.
    k_factor: Option<f64>,
    /// Number of QPSK symbols per coherence block.
    coherence_block: Option<usize>,
    /// Number of coherence blocks per frame.
    taps: Option<usize>,
}

/// Either channel behind one type, so the helpers stay generic over `ChannelModel`.
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

/// An absent `channel` key selects BPSK/AWGN.
fn build_channel(cfg: Option<&ChannelToml>) -> Result<AnyChannel, String> {
    match cfg {
        None => Ok(AnyChannel::Awgn(BpskAwgnChannel)),
        Some(ch) => match ch.kind.as_str() {
            "awgn" => Ok(AnyChannel::Awgn(BpskAwgnChannel)),
            "rician" => {
                let rician_cfg = RicianConfig {
                    k_factor: ch.k_factor.ok_or("rician channel requires `k_factor`")?,
                    coherence_block: ch
                        .coherence_block
                        .ok_or("rician channel requires `coherence_block`")?,
                    taps: ch.taps.ok_or("rician channel requires `taps`")?,
                };
                Ok(AnyChannel::Rician(QpskRicianChannelModel::new(rician_cfg)))
            }
            other => Err(format!("unknown channel kind: {other}")),
        },
    }
}

/// Top-level campaign configuration, deserialized from a TOML file.
#[derive(Debug, Deserialize)]
struct CampaignConfig {
    campaign: CampaignMeta,
    curve: Vec<CurveConfig>,
}

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
    #[serde(rename = "type")]
    curve_type: CurveType,

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

    /// Component code name: `"ebch_16_11"`, `"ebch_16_7"`, `"ebch_32_26"`,
    /// `"ebch_64_57"`, `"drm_32_21"`, `"drm_32_21_dynamic"`.
    component: Option<String>,
    turbo: Option<TurboConfig>,

    /// GLDPC code variant: `"lentmaier_1024"`.
    variant: Option<String>,
    /// SOGRAND configuration for GLDPC check-node decoder.
    sogrand: Option<SograndConfig>,

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
    /// Frames per worker in a parallel product batch. By default, divide the
    /// outstanding frame-error target among workers, rounding up with at least
    /// one frame each: a frame can contribute at most one frame error. Batches
    /// are capped by the remaining frame limit. Ordered folding keeps results
    /// identical for every batch size and worker count.
    #[cfg(feature = "parallel")]
    frames_per_worker_chunk: Option<NonZeroUsize>,
    max_iterations: usize,
    alpha: f32,
    /// Final alpha for iteration-dependent schedule (Chase-Pyndiah style).
    alpha_final: Option<f32>,
    /// Maximum absolute extrinsic LLR value.
    extrinsic_clamp: Option<f32>,
    list_size: usize,
    max_queries: usize,
    /// Per-component list-BLER early-stop threshold of `@/citation/Yuan2025` § V:
    /// a SOGRAND component decode stops once the list holds `list_size`
    /// codewords or the predicted list-BLER falls below this value. The turbo
    /// loop still stops only when every row and column is a valid component
    /// codeword.
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
    /// Set for codes whose codewords all have even weight, such as extended BCH.
    #[serde(default)]
    even_code: bool,
    /// Extrinsic damping factor for check-to-variable messages.
    #[serde(default = "default_alpha")]
    alpha: f32,
    /// Maximum absolute LLR value for variable-node beliefs.
    #[serde(default = "default_saturation")]
    llr_saturation: f32,
}

fn default_alpha() -> f32 {
    0.7
}

fn default_saturation() -> f32 {
    25.0
}

struct CliArgs {
    /// Path to the TOML campaign file (first positional argument).
    toml_path: String,
    /// Curve name filter (may be specified more than once).
    curves: Vec<String>,
    /// Use the rayon-parallel runners.
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

fn load_campaign(path: &str) -> Result<CampaignConfig, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    toml::from_str(&content).map_err(|e| format!("failed to parse {path}: {e}"))
}

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
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    }
}

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

            // One component instance goes to the encoder, one to the decoder.
            match comp_name {
                "ebch_16_11" => run_product(
                    ExtendedBchComponent::ebch_16_11(),
                    ExtendedBchComponent::ebch_16_11(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_16_7" => run_product(
                    ExtendedBchComponent::ebch_16_7(),
                    ExtendedBchComponent::ebch_16_7(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_32_26" => run_product(
                    ExtendedBchComponent::ebch_32_26(),
                    ExtendedBchComponent::ebch_32_26(),
                    turbo_cfg,
                    &channel,
                    &config,
                    parallel,
                ),
                "ebch_64_57" => run_product(
                    ExtendedBchComponent::ebch_64_57(),
                    ExtendedBchComponent::ebch_64_57(),
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

/// SplitMix64 finalizer.
#[cfg(feature = "parallel")]
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// RNG seed of frame `frame_idx` at SNR point `point_idx`: a hash of the three
/// indices, so streams of different points do not overlap at any `max_frames`.
#[cfg(feature = "parallel")]
fn frame_seed(base_seed: u64, point_idx: usize, frame_idx: usize) -> u64 {
    let point_seed = splitmix64(base_seed ^ (point_idx as u64).rotate_left(32));
    splitmix64(point_seed ^ frame_idx as u64)
}

/// Decoder statistics of one simulated frame.
#[cfg(feature = "parallel")]
struct FrameOutcome {
    bit_errors: usize,
    iterations: usize,
    queries: usize,
}

/// Frame-parallel product-code simulation: SNR points run sequentially and the
/// frames of each point are decoded in chunks over rayon workers, one decoder
/// per worker. Each frame draws from its own seeded stream, and a chunk's
/// outcomes are folded in frame order until `min_errors` frame errors are
/// collected, so a point's counts do not depend on the worker count or
/// scheduling.
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
    let base_seed = config.rng_seed.unwrap_or_else(rand::random);

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

        let mut frames = 0usize;
        let mut frame_errors = 0usize;
        let mut bit_errors = 0usize;
        let mut iterations = 0usize;
        let mut queries = 0usize;

        let decoder_comp = decoder_component.clone();
        let cp_cfg = cp_config.clone();
        let turbo_cfg_inner = turbo_config.clone();
        let workers = rayon::current_num_threads();

        let mut next_frame = 0usize;
        'point: while next_frame < config.max_frames {
            let frames_per_worker = turbo_cfg.frames_per_worker_chunk.map_or_else(
                || {
                    config
                        .min_errors
                        .saturating_sub(frame_errors)
                        .div_ceil(workers)
                        .max(1)
                },
                NonZeroUsize::get,
            );
            let chunk_len = workers.saturating_mul(frames_per_worker);
            let chunk_end = config.max_frames.min(next_frame.saturating_add(chunk_len));
            let outcomes: Vec<FrameOutcome> = (next_frame..chunk_end)
                .into_par_iter()
                .map_init(
                    || -> ProductDecodeFn {
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
                        let mut rng =
                            StdRng::seed_from_u64(frame_seed(base_seed, point_idx, frame_idx));
                        let message = BitVec::random(k, &mut rng);
                        let codeword = product.encode(&message);
                        let llrs =
                            channel.transmit_and_demodulate(&codeword, eb_n0_db, rate, &mut rng);
                        let result = decode_fn(&llrs);
                        FrameOutcome {
                            bit_errors: count_bit_errors(&message, &result.decoded_bits),
                            iterations: result.iterations,
                            queries: result.queries.unwrap_or(result.iterations),
                        }
                    },
                )
                .collect();
            for outcome in outcomes {
                frames += 1;
                bit_errors += outcome.bit_errors;
                iterations += outcome.iterations;
                queries += outcome.queries;
                if outcome.bit_errors > 0 {
                    frame_errors += 1;
                    if frame_errors >= config.min_errors {
                        break 'point;
                    }
                }
            }
            next_frame = chunk_end;
        }

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

        if let Some(ref path) = config.output_path {
            sim_result.append_csv_row_to(path);
        }

        completed_points.push((eb_n0_db, point_elapsed, frames, bler));
        points.push(sim_result);
    }

    let results = SimulationResults { points };
    if let Some(ref path) = config.output_path {
        results.write_to(path);
    }
    results
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use gf2_core::test_scratch::scratch;

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

        let sp = &config.curve[1];
        assert_eq!(sp.curve_type, CurveType::Ldpc);
        assert_eq!(sp.algorithm.as_deref(), Some("sum_product"));
        assert!(sp.scale.is_none());

        let prod = &config.curve[2];
        assert_eq!(prod.curve_type, CurveType::Product);
        assert_eq!(prod.component.as_deref(), Some("ebch_16_11"));
        let turbo = prod.turbo.as_ref().unwrap();
        assert_eq!(turbo.max_iterations, 20);
        assert!((turbo.alpha - 0.5).abs() < f32::EPSILON);
        assert_eq!(turbo.list_size, 4);
        assert_eq!(turbo.max_queries, 1_000_000);

        let gldpc = &config.curve[3];
        assert_eq!(gldpc.curve_type, CurveType::Gldpc);
        assert_eq!(gldpc.variant.as_deref(), Some("lentmaier_1024"));
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_frame_chunk_override_must_be_nonzero() {
        let config = SAMPLE_TOML.replace(
            "max_iterations = 20, alpha = 0.5",
            "max_iterations = 20, frames_per_worker_chunk = 0, alpha = 0.5",
        );
        assert!(toml::from_str::<CampaignConfig>(&config).is_err());
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

        let output_dir = scratch("gf2-sim-runner-crc");

        let results =
            run_curve(&config.curve[0], output_dir.to_str().unwrap(), false, 123).unwrap();

        assert_eq!(results.points.len(), 1);
        assert!(results.points[0].ber.is_finite());
        assert!(results.points[0].bler.is_finite());
        let csv = std::fs::read_to_string(output_dir.join("crc_25_15_smoke.csv")).unwrap();
        assert_eq!(csv.lines().count(), 1 + results.points.len());
        assert!(csv.starts_with("eb_n0_db"));
    }

    #[test]
    #[ignore = "sim: Rician product-code BLER decay across 3 SNR points"]
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
channel = { kind = "rician", k_factor = 5.0, coherence_block = 128, taps = 4 }
snr = { start = 2.0, stop = 8.0, step = 3.0 }
min_errors = 2
max_frames = 30
"#;
        let config: CampaignConfig = toml::from_str(toml_str).unwrap();

        let output_dir = scratch("gf2-sim-runner-rician");

        let results = run_curve(&config.curve[0], output_dir.to_str().unwrap(), false, 42).unwrap();

        assert_eq!(results.points.len(), 3);

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

        let bler_low = results.points[0].bler;
        let bler_high = results.points[2].bler;
        assert!(
            bler_high < bler_low,
            "Expected BLER to decay: low={bler_low:.4} high={bler_high:.4}"
        );
    }

    #[test]
    fn test_build_channel_rician_parameters() {
        let toml_str = r#"
kind = "rician"
k_factor = 5.0
coherence_block = 128
taps = 4
"#;
        let ch: ChannelToml = toml::from_str(toml_str).unwrap();
        assert_eq!(ch.kind, "rician");
        assert_eq!(ch.k_factor, Some(5.0));
        assert_eq!(ch.coherence_block, Some(128));
        assert_eq!(ch.taps, Some(4));

        let chan = build_channel(Some(&ch)).unwrap();
        assert!(matches!(chan, AnyChannel::Rician(_)));
    }

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

    #[cfg(feature = "parallel")]
    #[test]
    fn frame_seeds_are_distinct_across_points_beyond_one_million_frames() {
        let base = 42;
        assert_ne!(frame_seed(base, 0, 1_000_000), frame_seed(base, 1, 0));
        assert_ne!(frame_seed(base, 0, 1_500_000), frame_seed(base, 1, 500_000));
    }

    #[cfg(feature = "parallel")]
    fn frame_parallel_points(
        threads: usize,
        frames_per_worker_chunk: Option<NonZeroUsize>,
    ) -> SimulationResults {
        let mut turbo_cfg: TurboConfig =
            toml::from_str("max_iterations = 4\nalpha = 0.5\nlist_size = 4\nmax_queries = 1000")
                .unwrap();
        turbo_cfg.frames_per_worker_chunk = frames_per_worker_chunk;
        let config = SimulationConfig {
            eb_n0_range_db: vec![0.0, 1.0],
            min_errors: 5,
            max_frames: 4000,
            max_decoder_iterations: 4,
            rng_seed: Some(7),
            output_path: None,
            checkpoint_dir: None,
            tracing_log_path: None,
            heartbeat_every_frames: None,
        };
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            run_product_frame_parallel(
                ProductCode::new(ExtendedBchComponent::ebch_16_11()),
                ExtendedBchComponent::ebch_16_11(),
                &turbo_cfg,
                &BpskAwgnChannel,
                &config,
            )
        })
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn frame_parallel_without_seed_draws_fresh_rng_stream() {
        struct RngProbeChannel(std::sync::Mutex<Vec<[u64; 4]>>);

        impl ChannelModel for RngProbeChannel {
            fn transmit_and_demodulate<R: rand::Rng>(
                &self,
                bits: &gf2_core::BitVec,
                eb_n0_db: f64,
                rate: f64,
                rng: &mut R,
            ) -> Vec<gf2_coding::llr::Llr> {
                self.0.lock().unwrap().push(rng.gen());
                BpskAwgnChannel.transmit_and_demodulate(bits, eb_n0_db, rate, rng)
            }
        }

        let turbo_cfg: TurboConfig =
            toml::from_str("max_iterations = 1\nalpha = 0.5\nlist_size = 1\nmax_queries = 100")
                .unwrap();
        let probe = |seed| {
            let channel = RngProbeChannel(std::sync::Mutex::new(Vec::new()));
            let mut config = SimulationConfig::quick_test();
            config.eb_n0_range_db = vec![0.0];
            config.max_frames = 1;
            config.rng_seed = seed;
            run_product_frame_parallel(
                ProductCode::new(ExtendedBchComponent::ebch_16_11()),
                ExtendedBchComponent::ebch_16_11(),
                &turbo_cfg,
                &channel,
                &config,
            );
            let words = channel.0.into_inner().unwrap();
            assert_eq!(words.len(), 1);
            words[0]
        };
        let fixed = probe(Some(0xDEAD_BEEF));
        assert_eq!(fixed, probe(Some(0xDEAD_BEEF)));
        let first = probe(None);
        assert_ne!(first, fixed, "None must not select the fixed fallback seed");
        assert_ne!(first, probe(None), "None must draw a fresh stream per run");
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn frame_parallel_results_are_identical_across_worker_counts() {
        let reference = frame_parallel_points(1, None);
        assert!(reference.points.iter().all(|p| p.num_frame_errors >= 5));
        for frames_per_worker in [None, NonZeroUsize::new(1), NonZeroUsize::new(7)] {
            for threads in [1, 2, 4, 8] {
                let results = frame_parallel_points(threads, frames_per_worker);
                assert_eq!(
                    reference.to_json(),
                    results.to_json(),
                    "{threads} workers, {frames_per_worker:?} frames per worker"
                );
            }
        }
    }
}
