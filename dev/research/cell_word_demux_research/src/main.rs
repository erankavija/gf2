/// One more check: verify that our LDPC encoder can reproduce TP06 from TP04 or TP05
/// for VV007-16KFFT. If the LDPC encoding is wrong, that would explain the bit interleaver mismatch.

use gf2_coding::test_support::{parse_tp_blocks, tp_path_for};
use gf2_core::BitVec;

fn count_diffs(a: &BitVec, b: &BitVec) -> usize {
    assert_eq!(a.len(), b.len());
    (0..a.len()).filter(|&i| a.get(i) != b.get(i)).count()
}

fn main() {
    let base = std::path::PathBuf::from("/data/specs/dvb/streams");
    let config_dir = base.join("VV007-16KFFT_CSP");

    let tp04 = parse_tp_blocks(&tp_path_for(&config_dir, "04"));
    let tp05 = parse_tp_blocks(&tp_path_for(&config_dir, "05"));
    let tp06 = parse_tp_blocks(&tp_path_for(&config_dir, "06"));

    eprintln!("VV007-16KFFT:");
    eprintln!("  TP04: {} blocks, sizes: {:?}", tp04.len(), tp04.iter().take(3).map(|b| b.len()).collect::<Vec<_>>());
    eprintln!("  TP05: {} blocks, sizes: {:?}", tp05.len(), tp05.iter().take(3).map(|b| b.len()).collect::<Vec<_>>());
    eprintln!("  TP06: {} blocks, sizes: {:?}", tp06.len(), tp06.iter().take(3).map(|b| b.len()).collect::<Vec<_>>());

    // Check: are some TP06 bits the same as TP05 bits?
    // LDPC: systematic encoding, so first K bits of TP06 should equal TP05.
    if !tp05.is_empty() && !tp06.is_empty() {
        let k = tp05[0].len();
        let n = tp06[0].len();
        eprintln!("\n  TP05[0] (k={}) vs first k bits of TP06[0] (n={})", k, n);

        // For systematic LDPC: TP06[0..k] should be BCH(TP05[0..k_bch]) + LDPC parity
        // The first k bits of TP06 are the BCH-encoded input bits.
        // Let's check if TP05[0] == TP06[0..k]
        let mut diffs = 0;
        for i in 0..k {
            if tp05[0].get(i) != tp06[0].get(i) {
                diffs += 1;
            }
        }
        eprintln!("  TP05[0] vs TP06[0..k]: {} diffs (should be 0 for systematic encoding)", diffs);

        // Also check with BCH parity (TP04 → BCH encode → TP05)
        if !tp04.is_empty() {
            let k4 = tp04[0].len();
            eprintln!("  TP04[0] (k4={}) vs first k4 bits of TP05[0]:", k4);
            let mut diffs4 = 0;
            for i in 0..k4.min(tp05[0].len()) {
                if tp04[0].get(i) != tp05[0].get(i) {
                    diffs4 += 1;
                }
            }
            eprintln!("    {} diffs (should be 0 for systematic BCH)", diffs4);
        }
    }
}
