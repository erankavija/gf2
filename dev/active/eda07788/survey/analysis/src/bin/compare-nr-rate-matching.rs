//! Does AFF3CT's 5G NR bit selection pick the same bits as gf2's?
//!
//! The issue names AFF3CT as the survey's starting point, and AFF3CT v4.7.0
//! does implement the 3GPP TS 38.212 SS5.4.2.1 rate matching that gf2's
//! `QuasiCyclicLdpc::nr_5g_rate_matched` also performs. Neither project
//! exposes the bit-selection step as a timed entry point, so this binary
//! compares the two *selections* instead of their speed: it recovers gf2's
//! transmitted-column list observationally and evaluates AFF3CT's rule on
//! gf2's own code parameters.
//!
//! AFF3CT's rule is transcribed, not linked. Its source is
//! `src/Module/Puncturer/LDPC/Puncturer_5G.cpp:33-47` at commit
//! e8a65c5047262d97a15563b9edc961f69b2792cc (tag v4.7.0):
//!
//! ```text
//! int k = 0, j = 0;
//! while (k < N) {
//!     if (!(j % (N_cw - 2*Zc) + 2*Zc < K_LDPC && K <= j % (N_cw - 2*Zc) + 2*Zc)) {
//!         X_N2[k] = X_N1[j % (N_cw - 2*Zc) + 2*Zc];
//!         k++;
//!     }
//!     j++;
//! }
//! ```
//!
//! with `K_LDPC = Kb * Zc` and `N_LDPC = Nb * Zc` from
//! `src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp:114-116`, so `K_LDPC`
//! is gf2's `full_k`, `N_LDPC` is gf2's `full_n`, `K` is gf2's `target_k` and
//! `N` is gf2's `target_n`. Because the rule is evaluated on gf2's parameters,
//! this isolates the selection rule from the lifting-size choice; the two
//! projects' lifting-size choices are reported separately.
//!
//! gf2's own list is recovered through the public `prepare_llrs`, which maps
//! `target_n` channel LLRs onto the `full_n` mother-code positions using the
//! same `transmitted_cols` the encoder applies. Tagging channel LLR `i` with
//! the value `TAG_BASE + i` makes each transmitted position identifiable and
//! keeps every tag clear of the `0` and `+20` sentinels `prepare_llrs` writes
//! into untransmitted and filler positions.

use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::Llr;

const TAG_BASE: f32 = 1000.0;

/// One surveyed configuration: base graph, target codeword and message length.
const CONFIGURATIONS: [(u8, usize, usize); 6] = [
    (2, 256, 121),
    (2, 512, 200),
    (2, 1024, 400),
    (1, 1024, 512),
    (1, 2048, 1024),
    (1, 4096, 2048),
];

/// AFF3CT's transcribed selection rule evaluated on gf2's parameters.
fn aff3ct_selection(
    target_n: usize,
    target_k: usize,
    full_k: usize,
    full_n: usize,
    two_z: usize,
) -> Vec<usize> {
    let span = full_n - two_z;
    let mut selected = Vec::with_capacity(target_n);
    let mut j = 0_usize;
    while selected.len() < target_n {
        let position = j % span + two_z;
        // AFF3CT skips exactly the filler range [K, K_LDPC).
        if !(position < full_k && target_k <= position) {
            selected.push(position);
        }
        j += 1;
        assert!(j < 100 * (target_n + span), "selection did not terminate");
    }
    selected
}

/// gf2's transmitted-column list, recovered through the public LLR mapping.
fn gf2_selection(code: &gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode) -> Vec<usize> {
    let params = code.params();
    let tagged: Vec<Llr> = (0..params.target_n)
        .map(|index| Llr::new(TAG_BASE + index as f32))
        .collect();
    let full = code.prepare_llrs(&tagged);
    assert_eq!(full.len(), params.full_n);
    let mut selected = vec![usize::MAX; params.target_n];
    for (position, llr) in full.iter().enumerate() {
        let value = llr.value();
        if value >= TAG_BASE {
            let index = (value - TAG_BASE) as usize;
            assert!(
                selected[index] == usize::MAX,
                "tag {index} appears at two positions"
            );
            selected[index] = position;
        }
    }
    assert!(
        selected.iter().all(|position| *position != usize::MAX),
        "a transmitted position did not carry its tag"
    );
    selected
}

/// AFF3CT's lifting-size choice, transcribed from
/// `src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp:69-114`.
fn aff3ct_lifting(base_graph: u8, target_k: usize) -> (usize, usize) {
    let kb = if base_graph == 1 {
        22
    } else if target_k > 640 {
        10
    } else if target_k > 192 {
        8
    } else {
        6
    };
    let lists: [&[usize]; 8] = [
        &[2, 4, 8, 16, 32, 64, 128, 256],
        &[3, 6, 12, 24, 48, 96, 192, 384],
        &[5, 10, 20, 40, 80, 160, 320],
        &[7, 14, 28, 56, 112, 224],
        &[9, 18, 36, 72, 144, 288],
        &[11, 22, 44, 88, 176, 352],
        &[13, 26, 52, 104, 208],
        &[15, 30, 60, 120, 240],
    ];
    let mut best = usize::MAX;
    let mut lifting = 0;
    for list in lists {
        for &value in list {
            let candidate = kb * value;
            if candidate >= target_k && candidate < best {
                best = candidate;
                lifting = value;
            }
        }
    }
    (kb, lifting)
}

fn main() {
    let mut agreements = 0_usize;
    let mut divergences = 0_usize;
    for (base_graph, target_n, target_k) in CONFIGURATIONS {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, target_n, target_k);
        let params = code.params().clone();
        let gf2 = gf2_selection(&code);
        let aff3ct = aff3ct_selection(
            params.target_n,
            params.target_k,
            params.full_k,
            params.full_n,
            params.num_punctured_systematic,
        );
        let (aff3ct_kb, aff3ct_z) = aff3ct_lifting(base_graph, target_k);
        let lifting_note = if aff3ct_z == params.lifting_factor && aff3ct_kb == params.kb {
            "same (Kb, Z)".to_owned()
        } else {
            format!(
                "gf2 (Kb={}, Z={}) vs AFF3CT (Kb={aff3ct_kb}, Z={aff3ct_z})",
                params.kb, params.lifting_factor
            )
        };
        let header = format!(
            "BG{base_graph} target_n={target_n} target_k={target_k} \
             (Z={}, full_k={}, full_n={}, 2Z={}, parity_kept={})",
            params.lifting_factor,
            params.full_k,
            params.full_n,
            params.num_punctured_systematic,
            params.parity_kept
        );
        match gf2
            .iter()
            .zip(&aff3ct)
            .position(|(left, right)| left != right)
        {
            None if gf2.len() == aff3ct.len() => {
                agreements += 1;
                println!(
                    "AGREE  {header}: {} selected bits identical; lifting {lifting_note}",
                    gf2.len()
                );
            }
            position => {
                divergences += 1;
                let index = position.unwrap_or_else(|| gf2.len().min(aff3ct.len()));
                println!(
                    "DIFFER {header}: first divergence at output bit {index}: \
                     gf2 takes mother-code column {:?}, AFF3CT takes {:?}; lifting {lifting_note}",
                    gf2.get(index),
                    aff3ct.get(index)
                );
            }
        }
    }
    println!();
    println!(
        "{agreements} configurations agree, {divergences} diverge, out of {}",
        CONFIGURATIONS.len()
    );
}
