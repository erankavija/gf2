//! Generates the SVG figures embedded in the `d4851c3d-modem-framework`
//! reveal.js presentation by running actual Monte Carlo simulations
//! through the modem framework.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --example gen_presentation_figures --release
//! ```
//!
//! Output (relative to the workspace root):
//! - `docs/presentations/figures/ber_curves.svg` — uncoded BER vs Eb/N0
//!   for BPSK, QPSK, 16-QAM, 64-QAM.
//! - `docs/presentations/figures/per_bit_mi_16qam.svg` — per-bit
//!   Gaussian-approximation mutual information vs Eb/N0 for 16-QAM
//!   (one curve per bit position, showing the outer-vs-inner PAM-bit
//!   reliability gap that Gray-QAM is known for).
//! - `docs/presentations/figures/llr_histograms_16qam.svg` — per-bit
//!   conditional LLR histograms at Eb/N0 = 8 dB, showing the
//!   near-Gaussian outer-bit and bimodal inner-bit distributions.
//!
//! Every number in the figures comes from a fresh simulation over the
//! shared modem framework — no hand-coded or extrapolated values.

use gf2_coding::modem::analysis::{HistogramConfig, PerBitChannelStats, PerBitLlrStats};
use gf2_coding::modem::{
    AnalysisCapture, DemapMethod, FastGrayQamDemapper, GrayQamMapper, ModemChannelAdapter,
    ModemSpec,
};
use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner};
use plotters::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::error::Error;
use std::num::NonZeroUsize;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = workspace_root()?.join("docs/presentations/figures");
    std::fs::create_dir_all(&out_dir)?;

    eprintln!("[1/3] Running BER Monte Carlo sweeps...");
    let ber_data = simulate_ber_curves();
    plot_ber_curves(&out_dir.join("ber_curves.svg"), &ber_data)?;

    eprintln!("[2/3] Collecting per-bit MI for 16-QAM across Eb/N0...");
    let mi_data = simulate_per_bit_mi_16qam();
    plot_per_bit_mi(&out_dir.join("per_bit_mi_16qam.svg"), &mi_data)?;

    eprintln!("[3/3] Collecting conditional LLR histograms for 16-QAM at Eb/N0 = 8 dB...");
    let hist_report = simulate_llr_histograms_16qam();
    plot_llr_histograms(&out_dir.join("llr_histograms_16qam.svg"), &hist_report)?;

    eprintln!("All figures written to {}", out_dir.display());
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    // Walk up from the current manifest directory until we find the
    // workspace-level Cargo.toml (the one containing `[workspace]`).
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).canonicalize()?;
    loop {
        let toml = dir.join("Cargo.toml");
        if toml.is_file() {
            let txt = std::fs::read_to_string(&toml)?;
            if txt.contains("[workspace]") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            return Err("could not locate workspace root".into());
        }
    }
}

// --------------------------------------------------------------------
// Figure 1: BER curves.
// --------------------------------------------------------------------

struct BerCurve {
    label: &'static str,
    eb_n0_db: Vec<f64>,
    ber: Vec<f64>,
}

fn simulate_ber_curves() -> Vec<BerCurve> {
    let eb_n0_points = vec![0.0_f64, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0];

    let mut curves = Vec::new();

    // BPSK over AWGN.
    curves.push(ber_curve_bpsk(&eb_n0_points));

    // Gray square-QAM over AWGN via ModemChannelAdapter at orders 4, 16, 64.
    for &order in &[4usize, 16, 64] {
        curves.push(ber_curve_gray_qam(order, &eb_n0_points));
    }

    curves
}

fn ber_curve_bpsk(eb_n0_db: &[f64]) -> BerCurve {
    let channel = BpskAwgnChannel;
    let bers = run_ber_sweep(&channel, eb_n0_db, 80_000);
    BerCurve {
        label: "BPSK",
        eb_n0_db: eb_n0_db.to_vec(),
        ber: bers,
    }
}

fn ber_curve_gray_qam(order: usize, eb_n0_db: &[f64]) -> BerCurve {
    let spec = ModemSpec::<f32>::gray_square_qam(order);
    let mapper = GrayQamMapper::<f32>::from_preset_order(order);
    let demapper = FastGrayQamDemapper::<f32>::new(spec);
    let channel = ModemChannelAdapter::new(mapper, demapper, DemapMethod::MaxLog);

    let bers = run_ber_sweep(&channel, eb_n0_db, 80_000);
    let label = match order {
        2 => "BPSK",
        4 => "QPSK",
        16 => "16-QAM",
        64 => "64-QAM",
        256 => "256-QAM",
        _ => "?-QAM",
    };
    BerCurve {
        label,
        eb_n0_db: eb_n0_db.to_vec(),
        ber: bers,
    }
}

fn run_ber_sweep<C: ChannelModel>(channel: &C, eb_n0_db: &[f64], max_frames: usize) -> Vec<f64> {
    let config = SimulationConfig {
        eb_n0_range_db: eb_n0_db.to_vec(),
        min_errors: usize::MAX,
        max_frames,
        max_decoder_iterations: 0,
        rng_seed: Some(0xBEEF_5EED_F1F1_F1F1),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    let results = SimulationRunner::run_uncoded_ber_with_channel(channel, &config, &mut rng);
    results
        .iter()
        .map(|r| {
            // Clip zero BERs for log-axis rendering: we only care about
            // the sweep down to ~1e-4 given the sample budget here.
            r.ber.max(0.5 / r.num_bits.max(1) as f64)
        })
        .collect()
}

fn plot_ber_curves(out: &std::path::Path, curves: &[BerCurve]) -> Result<(), Box<dyn Error>> {
    let root = SVGBackend::new(out, (920, 560)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "Uncoded BER vs Eb/N0 — modem framework Monte Carlo",
            ("sans-serif", 22).into_font(),
        )
        .margin(20)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(-0.5_f64..14.5_f64, (1e-5_f64..1.0_f64).log_scale())?;

    chart
        .configure_mesh()
        .x_desc("Eb/N0 (dB)")
        .y_desc("Bit error rate")
        .x_labels(15)
        .y_labels(10)
        .light_line_style(RGBColor(220, 220, 220))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    let palette = [
        RGBColor(31, 119, 180),  // blue — BPSK
        RGBColor(255, 127, 14),  // orange — QPSK
        RGBColor(44, 160, 44),   // green — 16-QAM
        RGBColor(214, 39, 40),   // red — 64-QAM
        RGBColor(148, 103, 189), // purple
    ];

    for (i, curve) in curves.iter().enumerate() {
        let color = palette[i % palette.len()];
        let points: Vec<(f64, f64)> = curve
            .eb_n0_db
            .iter()
            .zip(curve.ber.iter())
            .map(|(&x, &y)| (x, y))
            .collect();
        chart
            .draw_series(LineSeries::new(points.clone(), color.stroke_width(2)))?
            .label(curve.label)
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(2))
            });
        chart.draw_series(
            points
                .iter()
                .map(|&(x, y)| Circle::new((x, y), 4, color.filled())),
        )?;
    }

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .label_font(("sans-serif", 14))
        .draw()?;

    root.present()?;
    Ok(())
}

// --------------------------------------------------------------------
// Figure 2: per-bit MI vs Eb/N0 for 16-QAM.
// --------------------------------------------------------------------

struct PerBitMiSweep {
    eb_n0_db: Vec<f64>,
    per_bit_mi: Vec<Vec<f64>>, // outer: Eb/N0 point, inner: bit position
}

fn simulate_per_bit_mi_16qam() -> PerBitMiSweep {
    let eb_n0_points: Vec<f64> = (0..=7).map(|i| i as f64 * 2.0).collect(); // 0..=14 step 2
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    let demapper = FastGrayQamDemapper::<f32>::new(spec);
    let channel = ModemChannelAdapter::new(mapper, demapper, DemapMethod::MaxLog);

    let mut per_bit_mi: Vec<Vec<f64>> = Vec::with_capacity(eb_n0_points.len());

    for &eb_n0 in &eb_n0_points {
        // One AnalysisCapture per SNR point so the MI numbers are
        // per-point rather than aggregated.
        let mut stats = PerBitLlrStats::new(4);
        let config = SimulationConfig {
            eb_n0_range_db: vec![eb_n0],
            min_errors: usize::MAX,
            max_frames: 60_000,
            max_decoder_iterations: 0,
            rng_seed: Some(0xD4851C3D_u64.wrapping_mul(eb_n0.to_bits())),
            output_path: None,
            checkpoint_dir: None,
            tracing_log_path: None,
            heartbeat_every_frames: None,
        };
        let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
        {
            let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::MaxLog);
            let _ = SimulationRunner::run_uncoded_ber_with_analysis(
                &channel,
                &config,
                Some(&mut capture),
                &mut rng,
            );
        }
        let report = stats.report();
        let mi: Vec<f64> = report
            .iter()
            .map(|r| r.mutual_info_bits_gaussian_approximation)
            .collect();
        per_bit_mi.push(mi);
    }

    PerBitMiSweep {
        eb_n0_db: eb_n0_points,
        per_bit_mi,
    }
}

fn plot_per_bit_mi(out: &std::path::Path, sweep: &PerBitMiSweep) -> Result<(), Box<dyn Error>> {
    let root = SVGBackend::new(out, (920, 560)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(
            "16-QAM per-bit mutual information (Gaussian approximation)",
            ("sans-serif", 22).into_font(),
        )
        .margin(20)
        .x_label_area_size(45)
        .y_label_area_size(60)
        .build_cartesian_2d(-0.5_f64..14.5_f64, -0.02_f64..1.05_f64)?;

    chart
        .configure_mesh()
        .x_desc("Eb/N0 (dB)")
        .y_desc("Mutual information I(B_k; L_k)  (bits)")
        .x_labels(15)
        .y_labels(11)
        .light_line_style(RGBColor(220, 220, 220))
        .axis_desc_style(("sans-serif", 15).into_font())
        .draw()?;

    let labels = [
        "bit 0 (MSB — I outer PAM)",
        "bit 1 (I inner PAM)",
        "bit 2 (Q outer PAM)",
        "bit 3 (LSB — Q inner PAM)",
    ];
    let palette = [
        RGBColor(31, 119, 180), // blue
        RGBColor(214, 39, 40),  // red
        RGBColor(44, 160, 44),  // green
        RGBColor(255, 127, 14), // orange
    ];

    for bit in 0..4usize {
        let color = palette[bit];
        let points: Vec<(f64, f64)> = sweep
            .eb_n0_db
            .iter()
            .zip(sweep.per_bit_mi.iter())
            .map(|(&x, mis)| (x, mis[bit]))
            .collect();
        chart
            .draw_series(LineSeries::new(points.clone(), color.stroke_width(2)))?
            .label(labels[bit])
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(2))
            });
        chart.draw_series(
            points
                .iter()
                .map(|&(x, y)| Circle::new((x, y), 4, color.filled())),
        )?;
    }

    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .label_font(("sans-serif", 13))
        .draw()?;

    root.present()?;
    Ok(())
}

// --------------------------------------------------------------------
// Figure 3: conditional LLR histograms at Eb/N0 = 8 dB for 16-QAM.
// --------------------------------------------------------------------

fn simulate_llr_histograms_16qam() -> Vec<PerBitChannelStats> {
    let spec = ModemSpec::<f32>::gray_square_qam(16);
    let mapper = GrayQamMapper::<f32>::from_preset_order(16);
    let demapper = FastGrayQamDemapper::<f32>::new(spec);
    let channel = ModemChannelAdapter::new(mapper, demapper, DemapMethod::MaxLog);

    let config = SimulationConfig {
        eb_n0_range_db: vec![8.0],
        min_errors: usize::MAX,
        max_frames: 200_000,
        max_decoder_iterations: 0,
        rng_seed: Some(0xAA55_CC33_F0F0_5A5A),
        output_path: None,
        checkpoint_dir: None,
        tracing_log_path: None,
        heartbeat_every_frames: None,
    };

    let hist_cfg = HistogramConfig {
        min: -30.0,
        max: 30.0,
        num_bins: NonZeroUsize::new(80).unwrap(),
    };
    let mut stats = PerBitLlrStats::new(4).with_histogram(hist_cfg);
    let mut rng = StdRng::seed_from_u64(config.rng_seed.unwrap());
    {
        let mut capture = AnalysisCapture::with_method(&mut stats, DemapMethod::MaxLog);
        let _ = SimulationRunner::run_uncoded_ber_with_analysis(
            &channel,
            &config,
            Some(&mut capture),
            &mut rng,
        );
    }
    stats.report()
}

fn plot_llr_histograms(
    out: &std::path::Path,
    report: &[PerBitChannelStats],
) -> Result<(), Box<dyn Error>> {
    let root = SVGBackend::new(out, (1100, 720)).into_drawing_area();
    root.fill(&WHITE)?;
    let root = root.margin(10, 10, 10, 10);
    root.titled(
        "16-QAM conditional LLR densities at Eb/N0 = 8 dB (max-log, 200k symbols)",
        ("sans-serif", 22),
    )?;

    let subplots = root.split_evenly((2, 2));
    let names = [
        "bit 0 (MSB — I outer PAM)",
        "bit 1 (I inner PAM)",
        "bit 2 (Q outer PAM)",
        "bit 3 (LSB — Q inner PAM)",
    ];
    let color_bit0 = RGBColor(31, 119, 180);
    let color_bit1 = RGBColor(214, 39, 40);

    for (idx, panel) in subplots.iter().enumerate() {
        let stats = &report[idx];
        let Some(h0) = stats.hist_bit0.as_ref() else {
            continue;
        };
        let Some(h1) = stats.hist_bit1.as_ref() else {
            continue;
        };

        let bins_0 = h0.bins();
        let bins_1 = h1.bins();
        let n_bins = bins_0.len();
        let bin_edges: Vec<(f64, f64)> = (0..n_bins).map(|b| h0.bin_edges(b)).collect();
        let total_0 = h0.total().max(1) as f64;
        let total_1 = h1.total().max(1) as f64;
        let density_0: Vec<(f64, f64)> = (0..n_bins)
            .map(|b| {
                let (lo, hi) = bin_edges[b];
                let w = hi - lo;
                let x = 0.5 * (lo + hi);
                let y = (bins_0[b] as f64 / total_0) / w;
                (x, y)
            })
            .collect();
        let density_1: Vec<(f64, f64)> = (0..n_bins)
            .map(|b| {
                let (lo, hi) = bin_edges[b];
                let w = hi - lo;
                let x = 0.5 * (lo + hi);
                let y = (bins_1[b] as f64 / total_1) / w;
                (x, y)
            })
            .collect();

        let y_max = density_0
            .iter()
            .chain(density_1.iter())
            .map(|&(_, y)| y)
            .fold(0.0_f64, f64::max)
            * 1.15;
        let y_max = y_max.max(1e-3);

        let mut chart = ChartBuilder::on(panel)
            .caption(names[idx], ("sans-serif", 16).into_font())
            .margin(8)
            .x_label_area_size(35)
            .y_label_area_size(50)
            .build_cartesian_2d(-30.0_f64..30.0_f64, 0.0_f64..y_max)?;

        chart
            .configure_mesh()
            .x_desc("L_k  (LLR, nats)")
            .y_desc("density")
            .x_labels(7)
            .y_labels(5)
            .light_line_style(RGBColor(230, 230, 230))
            .axis_desc_style(("sans-serif", 12).into_font())
            .label_style(("sans-serif", 10))
            .draw()?;

        chart
            .draw_series(LineSeries::new(density_0, color_bit0.stroke_width(2)))?
            .label("p(L_k | B_k = 0)")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 16, y)], color_bit0.stroke_width(2))
            });
        chart
            .draw_series(LineSeries::new(density_1, color_bit1.stroke_width(2)))?
            .label("p(L_k | B_k = 1)")
            .legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 16, y)], color_bit1.stroke_width(2))
            });

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.85))
            .border_style(BLACK)
            .label_font(("sans-serif", 11))
            .position(SeriesLabelPosition::UpperRight)
            .draw()?;
    }

    root.present()?;
    Ok(())
}
