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
use std::path::PathBuf;

use gf2_coding::bch::extended::ExtendedBchCode;
use gf2_coding::drm::DrmCode;
use gf2_coding::gldpc::{GldpcDecoder, QcGldpcCode};
use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedDecoder;
use gf2_coding::ldpc::{DecoderAlgorithm, QuasiCyclicLdpc};
use gf2_coding::product::{ProductCode, TurboDecoder, TurboDecoderConfig};
use gf2_coding::simulation::{BpskAwgnChannel, SimulationConfig, SimulationRunner};

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
    /// `"ebch_64_57"`, `"drm_32_21"`.
    component: Option<String>,
    /// Turbo decoder configuration.
    turbo: Option<TurboConfig>,

    // -- GLDPC fields (optional) --
    /// GLDPC code variant: `"lentmaier_1024"`.
    variant: Option<String>,

    // -- Common --
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
    fn to_points(&self) -> Vec<f64> {
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
    list_size: usize,
    max_queries: usize,
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

/// Runs a single curve according to its type.
fn run_curve(
    curve: &CurveConfig,
    output_dir: &str,
    parallel: bool,
    seed: u64,
) -> Result<(), String> {
    let config = build_sim_config(curve, output_dir, seed);
    let channel = BpskAwgnChannel;

    match curve.curve_type {
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
                );
            } else {
                let mut decoder =
                    Nr5gRateMatchedDecoder::with_algorithm(rm_code.clone(), algorithm);
                SimulationRunner::run_coded_iterative(&rm_code, &mut decoder, &channel, &config);
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
                ),
                "ebch_16_7" => run_product(
                    ExtendedBchCode::ebch_16_7(),
                    ExtendedBchCode::ebch_16_7(),
                    turbo_cfg,
                    &channel,
                    &config,
                ),
                "ebch_32_26" => run_product(
                    ExtendedBchCode::ebch_32_26(),
                    ExtendedBchCode::ebch_32_26(),
                    turbo_cfg,
                    &channel,
                    &config,
                ),
                "ebch_64_57" => run_product(
                    ExtendedBchCode::ebch_64_57(),
                    ExtendedBchCode::ebch_64_57(),
                    turbo_cfg,
                    &channel,
                    &config,
                ),
                "drm_32_21" => run_product(
                    DrmCode::drm_32_21(),
                    DrmCode::drm_32_21(),
                    turbo_cfg,
                    &channel,
                    &config,
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

            if parallel {
                let code_f = code.clone();
                SimulationRunner::run_coded_iterative_parallel(
                    &code,
                    move || GldpcDecoder::new(code_f.clone()),
                    &channel,
                    &config,
                );
            } else {
                let mut decoder = GldpcDecoder::new(code.clone());
                SimulationRunner::run_coded_iterative(&code, &mut decoder, &channel, &config);
            }
        }
    }

    Ok(())
}

/// Helper: runs a product-code curve for any `ProductComponent` type.
fn run_product<C>(
    encoder_component: C,
    decoder_component: C,
    turbo_cfg: &TurboConfig,
    channel: &BpskAwgnChannel,
    config: &SimulationConfig,
) where
    C: gf2_coding::product::ProductComponent + Clone,
{
    let product = ProductCode::new(encoder_component);
    let turbo_config = TurboDecoderConfig {
        max_iterations: turbo_cfg.max_iterations,
        alpha: turbo_cfg.alpha,
        list_size: turbo_cfg.list_size,
        max_queries: turbo_cfg.max_queries,
        list_bler_threshold: None,
    };
    let turbo = TurboDecoder::new(decoder_component, turbo_config);
    SimulationRunner::run_with_decoder(&product, |llrs| turbo.decode(llrs).into(), channel, config);
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
    std::fs::create_dir_all(&campaign.campaign.output_dir).ok();

    let total = selected.len();
    for (i, curve) in selected.iter().enumerate() {
        eprintln!(
            "[{}/{}] Running curve: {} (type={:?})",
            i + 1,
            total,
            curve.name,
            curve.curve_type
        );
        if let Err(e) = run_curve(
            curve,
            &campaign.campaign.output_dir,
            args.parallel,
            args.seed,
        ) {
            eprintln!("Error running curve {}: {e}", curve.name);
            std::process::exit(1);
        }
        eprintln!("  Done: {}", curve.name);
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
