//! QC-aware coalesced-layout memory-traffic probe for JIT issue 43fb19e2.
//!
//! HOST-side probe (NO GPU kernel, NO change to the production kernel) that
//! quantifies the memory-traffic difference between (a) the flat CSR Tanner
//! layout the production kernel uses today and (b) a QC-aware layout that keeps
//! the Z=384 lanes of each circulant contiguous so consecutive threads in a
//! wavefront touch consecutive addresses (coalesced).
//!
//! Method (justified in the design doc): a GPU global-memory access is serviced
//! in 64-byte (RDNA2) cache-line transactions. A wavefront of 32 lanes issuing
//! a load generates as many distinct 64-byte transactions as the number of
//! distinct 64-byte lines its 32 addresses fall on. We replay the EXACT index
//! arrays the production host flattener would build for the canonical mother
//! graph, group threads into wavefronts the way the kernel's
//! `gid = blockIdx*blockDim + threadIdx` mapping does, and count distinct
//! 64-byte lines touched by the per-edge gather (`v2c[base + f]` via the
//! `check_edge_to_var_edge[o]` indirection) — the kernel's dominant random
//! access. We compare:
//!
//! * FLAT: edges ordered in CSR check-row order (production today). Within a
//!   check row the gathered var-edge ids `f` are arbitrary => scattered.
//! * QC: edges ordered so a wavefront walks the Z lanes of ONE circulant,
//!   whose var-edge targets are a cyclic shift => contiguous run of f.
//!
//! The figure of merit is the "transaction efficiency" = useful 4-byte words /
//! (64 * transactions): how much of each fetched cache line is actually used.
//!
//! Run:  cargo run --release --bin qc_layout_probe

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_core::sparse::SpBitMatrixDual;

const BG: u8 = 1;
const TARGET_N: usize = 16896;
const TARGET_K: usize = 8448;
const Z: usize = 384;
const WAVEFRONT: usize = 32; // RDNA2 wave32
const LINE_BYTES: usize = 64; // RDNA2 L2 cache line
const F32: usize = 4;

/// Builds the production-equivalent flat CSR edge arrays: for each check row in
/// row order, the matching var-edge index `f` of each gathered v2c. This is the
/// exact `check_edge_to_var_edge` ordering the host flattener produces (CSR row
/// order; the matching var-edge is the position of (check,var) within the
/// variable's CSC column).
fn flat_check_edge_to_var_edge(h: &SpBitMatrixDual) -> Vec<usize> {
    let n = h.cols();
    let m = h.rows();
    // var-edge base offsets: var v's edges occupy [var_off[v] .. var_off[v]+deg).
    let mut var_off = vec![0usize; n + 1];
    for v in 0..n {
        var_off[v + 1] = var_off[v] + h.col_iter(v).count();
    }
    // position of check c within var v's column (CSC order).
    let mut map = Vec::new();
    for c in 0..m {
        for v in h.row_iter(c) {
            // find position of c in v's column.
            let pos = h.col_iter(v).position(|cc| cc == c).expect("edge present");
            map.push(var_off[v] + pos);
        }
    }
    map
}

/// Counts distinct 64-byte lines touched by `addrs_words` (f32 word indices),
/// grouped into wavefronts of `WAVEFRONT` consecutive entries.
fn transactions(addr_words: &[usize]) -> (u64, u64) {
    let words_per_line = LINE_BYTES / F32; // 16 f32 per 64 B line
    let mut tx = 0u64;
    let mut chunks = 0u64;
    let mut i = 0;
    while i < addr_words.len() {
        let end = (i + WAVEFRONT).min(addr_words.len());
        let mut lines: Vec<usize> = addr_words[i..end]
            .iter()
            .map(|w| w / words_per_line)
            .collect();
        lines.sort_unstable();
        lines.dedup();
        tx += lines.len() as u64;
        chunks += 1;
        i = end;
    }
    (tx, chunks)
}

fn main() {
    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(BG, TARGET_N, TARGET_K);
    let h = rm.mother_code().parity_check_matrix();
    let m = h.rows();
    let e = h.nnz();

    println!("# QC-layout memory-traffic probe — BG{BG} i_LS=1 Z={Z} r1/2");
    println!(
        "m={m} E={e} wavefront={WAVEFRONT} line={LINE_BYTES}B f32-per-line={}",
        LINE_BYTES / F32
    );
    println!();

    // ---- FLAT (production today): gather addresses in CSR row order ----
    // The check-update kernel reads v2c[base + check_edge_to_var_edge[o]] for
    // each gathered edge o. In CSR row order the f targets are scattered.
    let flat_map = flat_check_edge_to_var_edge(h);
    let (flat_tx, flat_chunks) = transactions(&flat_map);
    let flat_useful_words = flat_map.len() as u64;
    let flat_eff = 100.0 * flat_useful_words as f64 / (flat_tx as f64 * (LINE_BYTES / F32) as f64);

    // ---- QC (proposed): reorder edges so a wavefront walks the Z lanes of one
    // circulant. Within a circulant block the var-edge targets f are a cyclic
    // shift of a contiguous Z-run, so 32 consecutive lanes hit a contiguous run
    // of f (modulo one wrap). We model this by sorting each check ROW's gathered
    // f targets and laying out circulant-contiguously: group the global edge
    // list by (base_row_layer, position-in-row), then within each group the Z
    // lanes are consecutive var-edges.
    //
    // Concretely: the QC layout stores, for base-row layer L and the j-th
    // nonzero of that base row, a Z-length run whose var-edge targets are
    // [f0, f0+1, ..., f0+Z-1] cyclically. We synthesize that contiguous run and
    // measure its transactions — the best case the QC structure can deliver.
    let num_layers = m / Z;
    let mut qc_addrs: Vec<usize> = Vec::with_capacity(flat_map.len());
    // edge index walk is CSR row order; reorder to layer-major, lane-contiguous.
    // Build per-base-row the list of (within-row position -> Z var-edge targets).
    // We reconstruct from flat_map: rows are contiguous in flat_map per CSR.
    let mut row_starts = vec![0usize; m + 1];
    for c in 0..m {
        row_starts[c + 1] = row_starts[c] + h.row_iter(c).count();
    }
    for layer in 0..num_layers {
        // max degree across this layer's Z rows; pad-skip absent positions.
        let base = layer * Z;
        let maxdeg = (0..Z)
            .map(|r| h.row_iter(base + r).count())
            .max()
            .unwrap_or(0);
        for pos in 0..maxdeg {
            // For column-position `pos` of the circulant, gather the f target of
            // each of the Z rows that has a pos-th edge. In a true circulant these
            // are a contiguous cyclic Z-run; we emit them in lane order (r=0..Z).
            for r in 0..Z {
                let c = base + r;
                let s = row_starts[c];
                let deg = row_starts[c + 1] - s;
                if pos < deg {
                    qc_addrs.push(flat_map[s + pos]);
                }
            }
        }
    }
    assert_eq!(
        qc_addrs.len(),
        flat_map.len(),
        "QC reorder must preserve edge count"
    );
    let (qc_tx, qc_chunks) = transactions(&qc_addrs);
    let qc_eff = 100.0 * qc_addrs.len() as f64 / (qc_tx as f64 * (LINE_BYTES / F32) as f64);

    println!("## v2c gather (the kernel's dominant random read)");
    println!(
        "FLAT (CSR order, production): {flat_tx} line-transactions over {flat_chunks} wavefronts"
    );
    println!("  transaction efficiency = {flat_eff:.1}%  (useful words / fetched words)");
    println!("QC  (lane-contiguous):       {qc_tx} line-transactions over {qc_chunks} wavefronts");
    println!("  transaction efficiency = {qc_eff:.1}%");
    println!();
    let bw_reduction = flat_tx as f64 / qc_tx as f64;
    println!("v2c-gather traffic reduction (FLAT/QC) = {bw_reduction:.3}x");
    println!();
    println!("NOTE: this probe models ONLY the v2c-gather read traffic, the");
    println!("kernel's dominant random access. The contiguous c2v/v2c stores and");
    println!("the channel read are already coalesced in both layouts, so the QC");
    println!("win applies to the gather term, not the whole iteration.");
}
