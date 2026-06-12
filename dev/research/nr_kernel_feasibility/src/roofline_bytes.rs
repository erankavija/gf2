//! Roofline byte-count probe for JIT issue 43fb19e2.
//!
//! Counts the EXACT memory traffic the flat GPU LDPC BP kernel
//! (`crates/gf2-kernels-hip/hip/ldpc_bp.hip`) moves per BP iteration at the
//! canonical 5G NR config (BG1, i_LS=1, Z=384, rate 1/2), so the design doc's
//! roofline model is anchored on the real graph dimensions and the kernel's
//! real access pattern — not on a guessed edge count.
//!
//! It reads the PRODUCTION mother code (`QuasiCyclicLdpc::nr_5g_rate_matched`)
//! and tallies, per kernel, the f32/int/byte loads and stores the kernel source
//! performs per Tanner edge, per node, per iteration. The arithmetic is printed
//! in a form the doc quotes directly.
//!
//! Run:  cargo run --release --bin roofline_bytes

use gf2_coding::ldpc::QuasiCyclicLdpc;

// Canonical config (receipt: dev/benchmarks/gf2-sim/5g-nr-realtime.md).
const BG: u8 = 1;
const TARGET_N: usize = 16896; // E (transmitted)
const TARGET_K: usize = 8448; // K (transport block)

fn main() {
    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(BG, TARGET_N, TARGET_K);
    let mother = rm.mother_code();
    let h = mother.parity_check_matrix();

    let n = mother.n(); // variable nodes (full_n)
    let m = mother.m(); // check nodes
    let e = h.nnz(); // Tanner edges E

    // Degree distribution sanity (per-node edge counts).
    let mut sum_check_deg = 0usize;
    let mut max_check_deg = 0usize;
    for c in 0..m {
        let d = h.row_iter(c).count();
        sum_check_deg += d;
        max_check_deg = max_check_deg.max(d);
    }
    let mut sum_var_deg = 0usize;
    let mut max_var_deg = 0usize;
    for v in 0..n {
        let d = h.col_iter(v).count();
        sum_var_deg += d;
        max_var_deg = max_var_deg.max(d);
    }
    assert_eq!(sum_check_deg, e, "row degrees must sum to E");
    assert_eq!(sum_var_deg, e, "col degrees must sum to E");

    println!("# Roofline byte-count probe — BG{BG} i_LS=1 Z=384 r1/2 (canonical)");
    println!("variable nodes  n (full_n) = {n}");
    println!("check nodes     m          = {m}");
    println!("Tanner edges    E (nnz H)  = {e}");
    println!(
        "avg check degree = {:.3}  max = {max_check_deg}",
        e as f64 / m as f64
    );
    println!(
        "avg var degree   = {:.3}  max = {max_var_deg}",
        e as f64 / n as f64
    );
    println!();

    // ----- Per-edge traffic of the flat kernel, per BP iteration -----
    //
    // The kernel stores per-edge messages as f32 (4 B). For a check of degree d
    // the check-update kernel, for EACH of its d output edges, gathers the v2c
    // of the OTHER (d-1) edges. That is a per-edge inner loop: the v2c array is
    // read d*(d-1) times per check (each read = 4 B f32 + the int index map
    // check_edge_to_var_edge[o] = 4 B), and writes d c2v outputs (4 B each).
    //
    // We tally the dominant f32 message traffic (the index arrays are graph-
    // constant and L2-resident across the batch, but we count them once at the
    // worst case for honesty and call them out separately).

    // Check-update kernel: per check c of degree d_c.
    //   v2c gathered reads:  sum_c d_c*(d_c-1)       f32 loads
    //   index map reads:     sum_c d_c*(d_c-1)       int  loads (check_edge_to_var_edge)
    //   c2v writes:          sum_c d_c = E           f32 stores
    let mut cu_v2c_reads = 0u64;
    for c in 0..m {
        let d = h.row_iter(c).count() as u64;
        cu_v2c_reads += d * d.saturating_sub(1);
    }
    let cu_c2v_writes = e as u64;

    // Var-update kernel: per variable v of degree d_v.
    //   c2v reads:   2 * sum_v d_v = 2E   (belief sum pass + message pass re-read)
    //   index reads: 2 * sum_v d_v = 2E   (var_edge_to_check_edge)
    //   v2c writes:  sum_v d_v = E
    //   channel_llr read: n              (1 per variable)
    //   hard_bits write:  n              (1 byte per variable)
    let vu_c2v_reads = 2 * e as u64;
    let vu_v2c_writes = e as u64;
    let vu_chan_reads = n as u64;
    let vu_hard_writes = n as u64;

    // Syndrome kernel (runs once per iter under early-term): per check, reads
    // d_c hard_bits (1 B) and may store 1 B unsatisfied flag.
    let syn_hard_reads = e as u64; // sum_c d_c = E (one hard_bit per edge)

    // f32 traffic (4 B), the dominant term.
    let f32_bytes =
        4 * (cu_v2c_reads + cu_c2v_writes + vu_c2v_reads + vu_v2c_writes + vu_chan_reads);
    // int index traffic (4 B) — graph-constant, mostly cacheable; reported separately.
    let mut cu_index_reads = 0u64;
    for c in 0..m {
        let d = h.row_iter(c).count() as u64;
        cu_index_reads += d * d.saturating_sub(1);
    }
    let int_bytes = 4 * (cu_index_reads + 2 * e as u64);
    // byte traffic (hard_bits, 1 B).
    let byte_bytes = vu_hard_writes + syn_hard_reads;

    let total_per_iter_per_frame = f32_bytes + int_bytes + byte_bytes;
    let f32_only_per_iter_per_frame = f32_bytes + byte_bytes; // excl. cacheable index arrays

    println!("## Per-frame, per-iteration memory traffic (kernel access pattern)");
    println!("check-update v2c gathered f32 reads = {cu_v2c_reads}  (= sum_c d_c*(d_c-1))");
    println!("check-update c2v f32 writes         = {cu_c2v_writes}  (= E)");
    println!("var-update   c2v f32 reads          = {vu_c2v_reads}  (= 2E)");
    println!("var-update   v2c f32 writes         = {vu_v2c_writes}  (= E)");
    println!("var-update   channel f32 reads      = {vu_chan_reads}  (= n)");
    println!("var-update   hard-bit byte writes   = {vu_hard_writes}  (= n)");
    println!("syndrome     hard-bit byte reads    = {syn_hard_reads}  (= E)");
    println!();
    println!("f32 message bytes / frame / iter        = {f32_bytes}");
    println!("int index bytes / frame / iter (cacheable) = {int_bytes}");
    println!("byte (hard) bytes / frame / iter        = {byte_bytes}");
    println!("TOTAL bytes / frame / iter (worst case) = {total_per_iter_per_frame}");
    println!("f32+byte bytes / frame / iter (indices cached) = {f32_only_per_iter_per_frame}");
    println!();

    // ----- Roofline against the RX 6950 XT (gfx1030) envelope -----
    //
    // Measured anchor: 17.45 Mbps decoded transport-block throughput at
    // batch=128, ~20 iters, k=8448 bits/block (receipt).
    let measured_mbps = 17.45_f64;
    let k_bits = TARGET_K as f64;
    let blocks_per_s = measured_mbps * 1e6 / k_bits;
    let iters = 20.0_f64; // operating-point iteration cap (BLER<=1e-2 cell)

    // Bytes moved per second at the measured rate (f32+byte traffic; the
    // cacheable index arrays are counted separately and added as a band).
    let bytes_per_block_decode = f32_only_per_iter_per_frame as f64 * iters;
    let achieved_bw_lo = blocks_per_s * bytes_per_block_decode; // indices fully cached
    let bytes_per_block_decode_hi = total_per_iter_per_frame as f64 * iters;
    let achieved_bw_hi = blocks_per_s * bytes_per_block_decode_hi; // indices uncached

    // RX 6950 XT peak VRAM bandwidth ~576 GB/s (256-bit GDDR6 @ ~18 Gbps).
    let peak_vram_bw = 576e9_f64;

    println!("## Roofline vs RX 6950 XT (gfx1030) — anchored on measured 17.45 Mbps");
    println!("decoded blocks/s @ 17.45 Mbps      = {blocks_per_s:.1}");
    println!("iters at operating point           = {iters}");
    println!(
        "achieved VRAM BW (indices cached)  = {:.1} GB/s",
        achieved_bw_lo / 1e9
    );
    println!(
        "achieved VRAM BW (indices uncached)= {:.1} GB/s",
        achieved_bw_hi / 1e9
    );
    println!(
        "RX 6950 XT peak VRAM BW            ~= {:.0} GB/s",
        peak_vram_bw / 1e9
    );
    println!(
        "achieved / peak (cached..uncached) = {:.1}% .. {:.1}%",
        100.0 * achieved_bw_lo / peak_vram_bw,
        100.0 * achieved_bw_hi / peak_vram_bw
    );
    println!();

    // ----- Compute (FLOP) side of the roofline -----
    //
    // NMS check-update per check of degree d: ~ d*(d-1) compares/abs (the
    // gathered min-magnitude + sign) ~= 2 flops/gathered-read. Var-update:
    // ~2*d adds/subs per variable. So FLOPs/iter ~ 2*cu_v2c_reads + 2*2E.
    let flops_per_iter_per_frame = 2 * cu_v2c_reads + 4 * e as u64;
    let flops_per_s = blocks_per_s * flops_per_iter_per_frame as f64 * iters;
    // RX 6950 XT FP32 peak ~23.8 TFLOP/s (5120 ALUs * 2 * ~2.31 GHz boost).
    let peak_fp32 = 23.8e12_f64;
    println!("## Compute (FP32) side");
    println!("FLOPs / frame / iter (NMS approx)  = {flops_per_iter_per_frame}");
    println!(
        "achieved FP32 rate                 = {:.2} TFLOP/s",
        flops_per_s / 1e12
    );
    println!(
        "RX 6950 XT peak FP32              ~= {:.1} TFLOP/s",
        peak_fp32 / 1e12
    );
    println!(
        "achieved / peak FP32               = {:.2}%",
        100.0 * flops_per_s / peak_fp32
    );
    println!();

    // Arithmetic intensity (f32 message traffic only).
    let ai = flops_per_iter_per_frame as f64 / f32_only_per_iter_per_frame as f64;
    let ridge = peak_fp32 / peak_vram_bw; // FLOP/byte at the roofline ridge
    println!("arithmetic intensity (FLOP/byte)   = {ai:.3}");
    println!("roofline ridge point (FLOP/byte)   = {ridge:.2}");
    println!(
        "AI {} ridge  =>  {}-bound region",
        if ai < ridge { "<" } else { ">=" },
        if ai < ridge { "BANDWIDTH" } else { "COMPUTE" }
    );
    println!();

    // ----- Lever (d): reduced-graph row pruning for r1/2 -----
    //
    // 5G NR rate matching transmits only target_n of the full_n mother columns.
    // A block of high-numbered parity columns is UNTRANSMITTED (channel LLR = 0),
    // and the first 2*Z systematic columns are mandatory-punctured. Check rows
    // whose edges are ALL incident to untransmitted (zero-information) parity
    // columns carry no channel evidence and are candidates for host-side static
    // pruning at this rate. We quantify the work (edge) fraction available.
    let p = rm.params();
    println!("## Lever (d) reduced-graph for r1/2 (host-side static pruning)");
    println!(
        "full_n={} target_n={} full_k={} target_k={}",
        p.full_n, p.target_n, p.full_k, p.target_k
    );
    println!(
        "num_shortened(filler)={} num_punctured_sys(2Z)={} num_punctured_parity={} parity_kept={}",
        p.num_shortened, p.num_punctured_systematic, p.num_punctured_parity, p.parity_kept
    );

    // Untransmitted parity columns are the highest-numbered parity columns.
    // The transmitted span of the mother code is columns [0 .. full_k + parity_kept);
    // columns at [full_k + parity_kept .. full_n) are untransmitted parity (LLR=0).
    let untx_parity_start = p.full_k + p.parity_kept;
    let untx_parity_cols: std::collections::HashSet<usize> =
        (untx_parity_start..p.full_n).collect();
    // Edges incident to an untransmitted parity column.
    let mut edges_to_untx = 0usize;
    // Check rows that touch ONLY untransmitted parity columns (fully prunable),
    // and rows that touch at least one (partially affected).
    let mut rows_only_untx = 0usize;
    let mut edges_in_rows_only_untx = 0usize;
    for c in 0..m {
        let vars: Vec<usize> = h.row_iter(c).collect();
        let deg = vars.len();
        let n_untx = vars.iter().filter(|v| untx_parity_cols.contains(v)).count();
        edges_to_untx += n_untx;
        if n_untx == deg && deg > 0 {
            rows_only_untx += 1;
            edges_in_rows_only_untx += deg;
        }
    }
    println!(
        "untransmitted parity columns       = {} (cols {}..{})",
        p.full_n - untx_parity_start,
        untx_parity_start,
        p.full_n
    );
    println!(
        "edges incident to untx-parity col  = {edges_to_untx}  ({:.1}% of E)",
        100.0 * edges_to_untx as f64 / e as f64
    );
    println!("check rows touching ONLY untx cols  = {rows_only_untx}  (fully prunable rows)");
    println!(
        "edges in fully-prunable rows        = {edges_in_rows_only_untx}  ({:.1}% of E)",
        100.0 * edges_in_rows_only_untx as f64 / e as f64
    );
    println!(
        "=> conservative reduced-graph traffic factor (1 / (1 - prunable_edge_frac)) = {:.3}x",
        1.0 / (1.0 - edges_in_rows_only_untx as f64 / e as f64).max(1e-9)
    );
}
