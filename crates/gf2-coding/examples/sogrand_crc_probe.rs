//! Probe for SOGRAND behaviour on the CRC(25,15) component at 1.0 dB E_b/N_0.
//!
//! This is a throwaway diagnostic used to investigate paper-alignment failures
//! on Phase 2 Figure 4. It measures, over many frames:
//!
//! 1. single-component SOGRAND decodes (no turbo): list occupancy,
//!    cumulative probability, query count, stop reason,
//!    APP / extrinsic LLR magnitudes, list-BLER prediction, decode correctness;
//! 2. full (625, 225) CRC-product turbo decodes: per-half-iteration query
//!    counts and (rough) BLER.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p gf2-coding --release --example sogrand_crc_probe
//! ```

use gf2_coding::channel::AwgnChannel;
use gf2_coding::crc::CrcCode;
use gf2_coding::grand::{OneLineIntercept, OrbGrand, OrbGrandConfig, SoGrand};
use gf2_coding::llr::Llr;
use gf2_coding::product::{ProductCode, ProductComponent, TurboDecoder, TurboDecoderConfig};
use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn llrs_from_awgn(
    codeword: &BitVec,
    channel: &AwgnChannel,
    variance: f64,
    rng: &mut StdRng,
) -> Vec<Llr> {
    let n = codeword.len();
    let mut llrs = Vec::with_capacity(n);
    for i in 0..n {
        // BPSK: 0 -> +1, 1 -> -1.
        let sym = if codeword.get(i) { -1.0 } else { 1.0 };
        let y = channel.transmit(sym, rng);
        // L = 2 y / sigma^2; positive means bit 0 more likely.
        llrs.push(Llr::new((2.0 * y / variance) as f32));
    }
    llrs
}

fn component_probe(
    eb_n0_db: f64,
    frames: usize,
    list_size: usize,
    max_queries: usize,
    list_bler_stop_threshold: Option<f64>,
) {
    println!(
        "\n=== Component SOGRAND probe: Eb/N0 = {:.2} dB, list_size = {}, max_queries = {}, \
         list_bler_stop = {:?} ===",
        eb_n0_db, list_size, max_queries, list_bler_stop_threshold
    );
    let component = CrcCode::crc_25_15();
    let h = component.comp_parity_check().clone();
    let n = component.comp_n();
    let k = component.comp_k();
    let rate = k as f64 / n as f64;

    // Variance matches BpskAwgnChannel convention (Es/N0 after BPSK).
    let eb_n0 = 10f64.powf(eb_n0_db / 10.0);
    let variance = 1.0 / (2.0 * rate * eb_n0);

    let channel = AwgnChannel::from_variance(variance);
    let config = OrbGrandConfig {
        list_size,
        max_queries,
        even_code: component.comp_is_even(),
        systematic: true,
        list_bler_stop_threshold,
        one_line_intercept: OneLineIntercept::Auto,
    };
    let sogrand = SoGrand::new(OrbGrand::new(h, config));

    let mut rng = StdRng::seed_from_u64(0xC001D15);
    let mut queries_sum: u128 = 0;
    let mut list_sizes = [0usize; 32];
    let mut correct = 0;
    let mut found_any = 0;
    let mut app_sum = 0.0f64;
    let mut ext_sum = 0.0f64;
    let mut cum_log_prob_sum = 0.0f64;
    let mut list_bler_pred_sum = 0.0f64;

    for _ in 0..frames {
        // Encode a random message (all-zero codeword is not representative
        // under even-code optimization because hard_parity differs each frame).
        let msg = BitVec::zeros(k); // all-zero message — simplest, parity stays 0
        let cw = component.encode(&msg);
        let rx = llrs_from_awgn(&cw, &channel, variance, &mut rng);
        let r = sogrand.decode_siso(&rx);

        queries_sum += r.query_count as u128;
        // We need orbgrand.decode directly to inspect list occupancy &
        // cumulative probability. Do a secondary decode (cheap in comparison).
        let orb = sogrand.orbgrand().decode(&rx);
        let bucket = orb.codewords.len().min(list_sizes.len() - 1);
        list_sizes[bucket] += 1;
        cum_log_prob_sum += orb.cumulative_log_probability;
        list_bler_pred_sum += r.list_bler_prediction;

        if !orb.codewords.is_empty() {
            found_any += 1;
            let best = orb.best_codeword().unwrap();
            // Compare first k bits of codeword to original message.
            let mut ok = true;
            for i in 0..k {
                if best.codeword.get(i) != msg.get(i) {
                    ok = false;
                    break;
                }
            }
            if ok {
                correct += 1;
            }
        }

        for a in &r.app_llrs {
            app_sum += a.value().abs() as f64;
        }
        for e in &r.extrinsic_llrs {
            ext_sum += e.value().abs() as f64;
        }
    }

    let avg_q = queries_sum as f64 / frames as f64;
    let avg_cum_p = (cum_log_prob_sum / frames as f64).exp();
    let avg_app = app_sum / (frames * n) as f64;
    let avg_ext = ext_sum / (frames * n) as f64;
    let avg_list_bler_pred = list_bler_pred_sum / frames as f64;

    println!("  frames             = {}", frames);
    println!("  avg queries        = {:.1}", avg_q);
    println!("  avg cum prob (lin) = {:.6}", avg_cum_p);
    println!("  avg list-BLER pred = {:.4}", avg_list_bler_pred);
    println!("  avg |APP| LLR      = {:.3}", avg_app);
    println!("  avg |EXT| LLR      = {:.4}", avg_ext);
    println!(
        "  found any/correct  = {} / {} ({:.3} frame-wise 'correct')",
        found_any,
        correct,
        correct as f64 / frames as f64
    );
    print!("  list occupancy     : ");
    for (i, &c) in list_sizes.iter().enumerate() {
        if c > 0 {
            print!("[{}]={} ", i, c);
        }
    }
    println!();
}

fn turbo_probe(
    eb_n0_db: f64,
    frames: usize,
    list_size: usize,
    max_queries: usize,
    list_bler_threshold: Option<f64>,
) {
    println!(
        "\n=== Turbo probe: Eb/N0 = {:.2} dB, list_size = {}, max_queries = {}, \
         list_bler_threshold = {:?} ===",
        eb_n0_db, list_size, max_queries, list_bler_threshold
    );
    let component = CrcCode::crc_25_15();
    let product = ProductCode::new(component.clone());
    let n = component.comp_n();
    let k = component.comp_k();
    let rate = (k * k) as f64 / (n * n) as f64;

    let eb_n0 = 10f64.powf(eb_n0_db / 10.0);
    let variance = 1.0 / (2.0 * rate * eb_n0);
    let channel = AwgnChannel::from_variance(variance);
    let config = TurboDecoderConfig {
        max_iterations: 20,
        alpha: 0.5,
        list_size,
        max_queries,
        list_bler_threshold,
        ..TurboDecoderConfig::default()
    };
    let decoder = TurboDecoder::new(component.clone(), config);

    let mut rng = StdRng::seed_from_u64(0xDEC0DE42);
    let mut bler_err = 0usize;
    let mut total_queries_sum: u128 = 0;
    let mut iters_sum: u128 = 0;
    let mut converged_cnt = 0;

    for _ in 0..frames {
        let msg = BitVec::zeros(k * k);
        let cw = product.encode_product(&msg);
        let rx = llrs_from_awgn(&cw, &channel, variance, &mut rng);
        let r = decoder.decode(&rx);
        total_queries_sum += r.total_queries as u128;
        iters_sum += r.iterations as u128;
        if r.converged {
            converged_cnt += 1;
        }
        // BLER vs all-zero message.
        let mut any = false;
        for i in 0..k * k {
            if r.decoded_bits.get(i) {
                any = true;
                break;
            }
        }
        if any {
            bler_err += 1;
        }
    }

    let bler = bler_err as f64 / frames as f64;
    let avg_q = total_queries_sum as f64 / frames as f64;
    let avg_iters = iters_sum as f64 / frames as f64;
    // Total component decodes: 2 * n * avg_iterations (assume full iters
    // when not converged; iterations counts halved-pairs, each doing n+n
    // component decodes, so approximate with 2*n per iter).
    let comp_decodes = 2.0 * n as f64 * avg_iters;
    let avg_q_per_comp = avg_q / comp_decodes;

    println!("  frames             = {}", frames);
    println!("  BLER               = {:.4}", bler);
    println!("  avg_iterations     = {:.2}", avg_iters);
    println!("  converged frames   = {}", converged_cnt);
    println!("  avg total queries  = {:.1}", avg_q);
    println!(
        "  avg queries/comp   = {:.1}  ({} comp-decodes/frame)",
        avg_q_per_comp, comp_decodes as usize
    );
}

fn main() {
    // --- Component probes: single CRC(25,15) SOGRAND decode, no turbo. ---
    // Baseline (legacy stop rule: exhaust max_queries or cum_prob ≈ 1).
    component_probe(1.0, 2000, 4, 100_000, None);
    // Paper-aligned stop: list_size=4 OR list-BLER < 1e-4.
    component_probe(1.0, 2000, 4, 100_000, Some(1e-4));
    // Same rule, tighter threshold.
    component_probe(1.0, 2000, 4, 100_000, Some(1e-5));

    // --- Turbo probes at 1.0 dB with modest frame count. ---
    turbo_probe(1.0, 300, 4, 100_000, None);
    turbo_probe(1.0, 300, 4, 100_000, Some(1e-4));
    turbo_probe(1.0, 300, 4, 100_000, Some(1e-5));
}
