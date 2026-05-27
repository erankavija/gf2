//! Test helpers for integration tests that consume ETSI DVB-T2 verified
//! vectors (the `VV001-CR35_CSP/TestPoint*/...CSP.txt` files).
//!
//! Gated behind `cfg(any(test, feature = "test-support"))` so the helpers
//! are reachable from both unit tests inside the crate and integration
//! tests under `tests/` (which import them by enabling the
//! `test-support` feature on the dev-dependency self-reference).
//!
//! All helpers here are test-only — production code must not call them.

#![cfg(any(test, feature = "test-support"))]

use gf2_core::BitVec;
use std::path::{Path, PathBuf};

/// Parses an ETSI CSP test-point file into a sequence of `BitVec` blocks.
///
/// Each `%`- or `#`-prefixed line begins a new block. Within a block,
/// `'0'` and `'1'` characters become bits; all other characters are
/// ignored. Empty blocks (no bits between two delimiters) are dropped.
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn parse_tp_blocks(path: &Path) -> Vec<BitVec> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    let mut blocks: Vec<BitVec> = Vec::new();
    let mut current: Option<BitVec> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('%') || line.starts_with('#') {
            if let Some(bv) = current.take() {
                if !bv.is_empty() {
                    blocks.push(bv);
                }
            }
            current = Some(BitVec::new());
            continue;
        }
        let bv = current.get_or_insert_with(BitVec::new);
        for ch in line.chars() {
            match ch {
                '0' => bv.push_bit(false),
                '1' => bv.push_bit(true),
                _ => {}
            }
        }
    }
    if let Some(bv) = current {
        if !bv.is_empty() {
            blocks.push(bv);
        }
    }
    blocks
}

/// Builds the canonical path to a VV001-CR35 test-point file under
/// `<config_dir>/TestPoint<NN>/VV001-CR35_TP<NN>_CSP.txt`.
///
/// `tp` may include an alphabetic suffix (e.g., `"07a"`); the
/// directory uses only the numeric prefix.
pub fn tp_path(config_dir: &Path, tp: &str) -> PathBuf {
    let tp_base = tp.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    config_dir
        .join(format!("TestPoint{}", tp_base))
        .join(format!("VV001-CR35_TP{}_CSP.txt", tp))
}
