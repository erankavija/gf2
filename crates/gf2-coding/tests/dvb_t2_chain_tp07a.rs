//! TP06 → TP07a validation of `DvbT2BitInterleaver` against the DVB-T2
//! reference streams (`@/citation/DvbVerification2010`).
//! TP07a is the output of `@/citation/Etsi2015` §6.1.3 (parity interleaving
//! and column-twist interleaving). The tests assert
//! `interleave(TP06) = TP07a` and `deinterleave(TP07a) = TP06` bit-exactly on
//! the first 10 blocks of VV020-FEF (16-QAM, rate 1/2), VV009-4KFFT (64-QAM,
//! rate 2/3) and VV014-64QAM34 (64-QAM, rate 3/4). They also check the block
//! structure and the block-0 forward match of every Normal-frame 16-QAM or
//! 64-QAM stream at rate 1/2, 2/3 or 3/4 found under the stream tree.

mod common;

use std::path::PathBuf;

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{
    DvbT2BitInterleaver, DvbT2Modcod, DvbT2Modulation,
};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;

use gf2_coding::test_support::{parse_tp_blocks, tp_path_for};

const NO_VECTORS: &str = "DVB-T2 ETSI test vectors absent; \
    set DVB_TEST_VECTORS_PATH to the stream tree to run this";

fn count_data_lines(path: &std::path::Path) -> std::io::Result<usize> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut count = 0usize;
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('%') && !trimmed.starts_with('#') {
            count += 1;
        }
    }
    Ok(count)
}

fn first_block_bits(path: &std::path::Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_block = false;
    let mut count = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('%') || line.starts_with('#') {
            if in_block && count > 0 {
                return Some(count);
            }
            in_block = true;
            continue;
        }
        if in_block {
            count += line.chars().filter(|&c| c == '0' || c == '1').count();
        }
    }
    if in_block && count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_blocks_in_file(path: &std::path::Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    let count = text
        .lines()
        .filter(|l| l.trim().starts_with("# block"))
        .count();
    Some(count)
}

/// Discover all `VV*_CSP` directories under `base` that contain TP05, TP06,
/// TP07a, and TP08 files and whose TP06 block size is 64800 (Normal FECFRAME).
fn discover_in_scope_vectors(base: &std::path::Path) -> Vec<(PathBuf, DvbT2Modulation, CodeRate)> {
    // In-scope Normal FECFRAME k values (TP05 bits per block).
    const K_RATE1_2: usize = 32400;
    const K_RATE2_3: usize = 43200;
    const K_RATE3_4: usize = 48600;
    const N_NORMAL: usize = 64800;

    // In-scope samples-per-block for TP08 (bits per QAM cell / bits per block):
    // 16-QAM: 64800 / 4 = 16200, 64-QAM: 64800 / 6 = 10800.
    const SPB_16QAM: usize = 16200;
    const SPB_64QAM: usize = 10800;

    let read_dir = match std::fs::read_dir(base) {
        Ok(rd) => rd,
        Err(_) => return vec![],
    };

    let mut results = Vec::new();

    for entry in read_dir.flatten() {
        let config_dir = entry.path();
        let dir_name = match config_dir.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => continue,
        };
        if !dir_name.starts_with("VV") || !dir_name.ends_with("_CSP") {
            continue;
        }

        let tp05_path = tp_path_for(&config_dir, "05");
        let tp06_path = tp_path_for(&config_dir, "06");
        let tp07a_path = tp_path_for(&config_dir, "07a");
        let tp08_dir = config_dir.join("TestPoint08");

        if !tp05_path.exists() || !tp06_path.exists() || !tp07a_path.exists() {
            continue;
        }

        match first_block_bits(&tp06_path) {
            Some(n) if n == N_NORMAL => {}
            _ => continue,
        }

        let k = match first_block_bits(&tp05_path) {
            Some(k) => k,
            None => continue,
        };
        let code_rate = match k {
            K_RATE1_2 => CodeRate::Rate1_2,
            K_RATE2_3 => CodeRate::Rate2_3,
            K_RATE3_4 => CodeRate::Rate3_4,
            _ => continue,
        };

        let tp08_files: Vec<_> = match std::fs::read_dir(&tp08_dir) {
            Ok(rd) => rd
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with("_TP08_CSP.txt"))
                .collect(),
            Err(_) => continue,
        };
        let tp08_path = match tp08_files.first() {
            Some(e) => e.path(),
            None => continue,
        };
        let block_header_count = match count_blocks_in_file(&tp08_path) {
            Some(n) if n > 0 => n,
            _ => continue,
        };
        let total_data_lines = match count_data_lines(&tp08_path) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let spb = total_data_lines / block_header_count;
        let modulation = match spb {
            SPB_16QAM => DvbT2Modulation::Qam16,
            SPB_64QAM => DvbT2Modulation::Qam64,
            _ => continue,
        };

        results.push((config_dir, modulation, code_rate));
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Applies `DvbT2BitInterleaver::interleave` to TP06 block 0 of every
/// discovered vector and compares it with TP07a block 0. A match also asserts
/// `deinterleave(TP07a) = TP06`; a mismatch is logged with its bit count.
#[test]
#[ignore = "slow: reads the DVB-T2 reference streams from $DVB_TEST_VECTORS_PATH"]
fn test_tp06_to_tp07a_forward_match_in_scope_normal() {
    let Some(base) = common::dvb_vectors_dir() else {
        common::skip(
            "test_tp06_to_tp07a_forward_match_in_scope_normal",
            NO_VECTORS,
        );
        return;
    };

    let candidates = discover_in_scope_vectors(&base);

    if candidates.is_empty() {
        eprintln!(
            "No in-scope Normal FECFRAME vectors found under {:?}; \
             expected at least VV014-64QAM34_CSP and VV037-DTG167_CSP",
            base
        );
        return;
    }

    eprintln!(
        "Discovered {} in-scope Normal FECFRAME vector(s):",
        candidates.len()
    );
    for (dir, modulation, rate) in &candidates {
        eprintln!(
            "  {:?}  modulation={:?}  rate={:?}",
            dir.file_name().unwrap(),
            modulation,
            rate
        );
    }

    let mut pass_count = 0usize;
    let mut fail_count = 0usize;

    for (config_dir, modulation, code_rate) in &candidates {
        let dir_name = config_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let tp06_path = tp_path_for(config_dir, "06");
        let tp07a_path = tp_path_for(config_dir, "07a");

        let tp06_blocks = parse_tp_blocks(&tp06_path);
        let tp07a_blocks = parse_tp_blocks(&tp07a_path);

        assert!(
            !tp06_blocks.is_empty(),
            "{dir_name}: TP06 parse produced no blocks"
        );
        assert!(
            !tp07a_blocks.is_empty(),
            "{dir_name}: TP07a parse produced no blocks"
        );

        let n_fec = 64800usize;
        assert_eq!(
            tp06_blocks[0].len(),
            n_fec,
            "{dir_name}: TP06 block 0 has {} bits, expected {n_fec}",
            tp06_blocks[0].len()
        );
        assert_eq!(
            tp07a_blocks[0].len(),
            n_fec,
            "{dir_name}: TP07a block 0 has {} bits, expected {n_fec}",
            tp07a_blocks[0].len()
        );

        let modcod = DvbT2Modcod::new(FrameSize::Normal, *code_rate, *modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);
        assert_eq!(
            interleaver.frame_bits(),
            n_fec,
            "{dir_name}: interleaver frame_bits() mismatch"
        );

        let interleaved = interleaver.interleave(&tp06_blocks[0]);

        let diffs: usize = (0..n_fec)
            .filter(|&i| interleaved.get(i) != tp07a_blocks[0].get(i))
            .count();

        if diffs == 0 {
            eprintln!(
                "[PASS] {dir_name}: §6.1.3-only interleave of TP06 matches TP07a \
                 bit-exact ({n_fec} bits)"
            );
            pass_count += 1;

            let deinterleaved = interleaver.deinterleave(&tp07a_blocks[0]);
            assert_eq!(
                deinterleaved, tp06_blocks[0],
                "{dir_name}: inverse check failed: deinterleave(tp07a) != tp06"
            );
            eprintln!("[PASS] {dir_name}: inverse deinterleave(tp07a) == tp06 verified");
        } else {
            let pct = 100.0 * diffs as f64 / n_fec as f64;
            eprintln!(
                "[SKIP] {dir_name}: interleave(TP06) differs from TP07a by {diffs}/{n_fec} bits \
                 ({pct:.1}%)."
            );
            fail_count += 1;
        }
    }

    eprintln!(
        "\nSummary: {pass_count} vector(s) PASS forward match, \
         {fail_count} vector(s) skipped (TP07a differs)"
    );
}

#[test]
#[ignore = "slow: reads the DVB-T2 reference streams from $DVB_TEST_VECTORS_PATH"]
fn test_tp06_tp07a_structural_sanity_in_scope_normal() {
    let Some(base) = common::dvb_vectors_dir() else {
        common::skip(
            "test_tp06_tp07a_structural_sanity_in_scope_normal",
            NO_VECTORS,
        );
        return;
    };

    let candidates = discover_in_scope_vectors(&base);
    if candidates.is_empty() {
        eprintln!("No in-scope Normal FECFRAME vectors found; skipping");
        return;
    }

    let n_fec = 64800usize;
    for (config_dir, modulation, code_rate) in &candidates {
        let dir_name = config_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let tp06_blocks = parse_tp_blocks(&tp_path_for(config_dir, "06"));
        let tp07a_blocks = parse_tp_blocks(&tp_path_for(config_dir, "07a"));

        assert!(!tp06_blocks.is_empty(), "{dir_name}: TP06 empty");
        assert!(!tp07a_blocks.is_empty(), "{dir_name}: TP07a empty");

        assert_eq!(
            tp06_blocks.len(),
            tp07a_blocks.len(),
            "{dir_name}: TP06 and TP07a block counts differ ({} vs {})",
            tp06_blocks.len(),
            tp07a_blocks.len()
        );

        for (i, block) in tp06_blocks.iter().enumerate() {
            assert_eq!(
                block.len(),
                n_fec,
                "{dir_name}: TP06 block {i} has {} bits, expected {n_fec}",
                block.len()
            );
        }
        for (i, block) in tp07a_blocks.iter().enumerate() {
            assert_eq!(
                block.len(),
                n_fec,
                "{dir_name}: TP07a block {i} has {} bits, expected {n_fec}",
                block.len()
            );
        }

        eprintln!(
            "[OK] {dir_name} ({:?} x {:?}): {} blocks × {n_fec} bits — structural check passed",
            modulation,
            code_rate,
            tp06_blocks.len()
        );
    }
}

#[test]
#[ignore = "slow: reads the DVB-T2 reference streams from $DVB_TEST_VECTORS_PATH"]
fn test_tp06_to_tp07a_parity_interleave_vv020_vv009_vv014() {
    const MAX_BLOCKS: usize = 10;
    const N_FEC: usize = 64800;

    let Some(base) = common::dvb_vectors_dir() else {
        common::skip(
            "test_tp06_to_tp07a_parity_interleave_vv020_vv009_vv014",
            NO_VECTORS,
        );
        return;
    };

    let test_cases = [
        ("VV020-FEF_CSP", CodeRate::Rate1_2, DvbT2Modulation::Qam16),
        ("VV009-4KFFT_CSP", CodeRate::Rate2_3, DvbT2Modulation::Qam64),
        (
            "VV014-64QAM34_CSP",
            CodeRate::Rate3_4,
            DvbT2Modulation::Qam64,
        ),
    ];
    let expected_tested = test_cases.len();
    let mut tested_count = 0usize;

    for (dir_name, code_rate, modulation) in &test_cases {
        let config_dir = base.join(dir_name);
        if !config_dir.exists() {
            eprintln!("Vector directory {:?} not found; skipping", config_dir);
            continue;
        }
        tested_count += 1;

        let tp06_path = tp_path_for(&config_dir, "06");
        let tp07a_path = tp_path_for(&config_dir, "07a");

        assert!(
            tp06_path.exists(),
            "{dir_name}: TP06 file not found at {tp06_path:?}"
        );
        assert!(
            tp07a_path.exists(),
            "{dir_name}: TP07a file not found at {tp07a_path:?}"
        );

        let tp06_blocks = parse_tp_blocks(&tp06_path);
        let tp07a_blocks = parse_tp_blocks(&tp07a_path);

        assert!(
            !tp06_blocks.is_empty(),
            "{dir_name}: TP06 parse produced no blocks"
        );
        assert!(
            !tp07a_blocks.is_empty(),
            "{dir_name}: TP07a parse produced no blocks"
        );

        let modcod = DvbT2Modcod::new(FrameSize::Normal, *code_rate, *modulation);
        let interleaver = DvbT2BitInterleaver::new(modcod);

        assert_eq!(
            interleaver.frame_bits(),
            N_FEC,
            "{dir_name}: interleaver frame_bits must be {N_FEC}"
        );

        let n_blocks = tp06_blocks.len().min(tp07a_blocks.len()).min(MAX_BLOCKS);

        for block_idx in 0..n_blocks {
            let tp06_block = &tp06_blocks[block_idx];
            let tp07a_block = &tp07a_blocks[block_idx];

            assert_eq!(
                tp06_block.len(),
                N_FEC,
                "{dir_name} block {block_idx}: TP06 has {} bits, expected {N_FEC}",
                tp06_block.len()
            );
            assert_eq!(
                tp07a_block.len(),
                N_FEC,
                "{dir_name} block {block_idx}: TP07a has {} bits, expected {N_FEC}",
                tp07a_block.len()
            );

            let interleaved = interleaver.interleave(tp06_block);
            let diffs: usize = (0..N_FEC)
                .filter(|&i| interleaved.get(i) != tp07a_block.get(i))
                .count();
            assert_eq!(
                diffs, 0,
                "{dir_name} block {block_idx}: interleave(TP06) differs from TP07a \
                 by {diffs}/{N_FEC} bits (expected 0; §6.1.3 parity+column-twist must \
                 reproduce TP07a bit-exact)"
            );

            let deinterleaved = interleaver.deinterleave(tp07a_block);
            assert_eq!(
                deinterleaved, *tp06_block,
                "{dir_name} block {block_idx}: deinterleave(TP07a) != TP06 \
                 (inverse permutation must invert forward exactly)"
            );
        }

        eprintln!(
            "[PASS] {dir_name} ({modulation:?} {code_rate:?}): \
             interleave(TP06) == TP07a for {n_blocks}/{} block(s) — bit-exact",
            tp06_blocks.len().min(tp07a_blocks.len())
        );
    }

    // A present vector tree must exercise every case, so a missing stream
    // cannot pass vacuously.
    assert_eq!(
        tested_count,
        expected_tested,
        "expected to exercise {expected_tested} ETSI vectors but only {tested_count} \
         were present at {}; set DVB_TEST_VECTORS_PATH to a tree containing \
         VV020-FEF_CSP, VV009-4KFFT_CSP, and VV014-64QAM34_CSP",
        base.display(),
    );
}
