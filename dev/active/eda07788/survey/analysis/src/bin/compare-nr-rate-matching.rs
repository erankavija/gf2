//! Does AFF3CT's 5G NR bit selection pick the same bits as gf2's?
//!
//! AFF3CT v4.7.0 implements the 5G NR LDPC rate-matching bit selection that
//! gf2's `QuasiCyclicLdpc::nr_5g_rate_matched` also performs. gf2 exposes no
//! public bit-selection entry point, so this binary compares the two
//! *selections* instead of their speed: it recovers gf2's transmitted-column
//! list observationally and evaluates AFF3CT's rule, transcribed rather than
//! linked, on the same configuration.
//!
//! Two AFF3CT functions are transcribed from commit
//! e8a65c5047262d97a15563b9edc961f69b2792cc (tag v4.7.0). Their exact source
//! lines are recorded in `../../source-evidence.json`.
//!
//! - `tools::build_5G_base_graph(K, N)` in
//!   `src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp` chooses the base graph
//!   from `K` and `R = K / N`, chooses the lifting size `Zc` as the smallest
//!   `Kb * Zc >= K` over the eight lifting sets, where the block-size-dependent
//!   `Kb` is used for this choice only, and then sets `K_LDPC` and `N_LDPC` to
//!   the base-graph column counts (22 and 68 for BG1, 10 and 52 for BG2) times
//!   `Zc`.
//! - `Puncturer_5G::_puncture` in `src/Module/Puncturer/LDPC/Puncturer_5G.cpp`
//!   reads the circular buffer `[2*Zc, N_cw)` from its start, skipping the
//!   filler range `[K, K_LDPC)`, until `N` bits are emitted:
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
//! gf2's `nr_5g_rate_matched` takes the base graph from its caller, while
//! AFF3CT derives it from `(K, N)`. Each configuration therefore reports three
//! comparisons: AFF3CT's own derivation from `(K, N)`, its lifting rule with
//! the base graph fixed to gf2's, and its selection rule evaluated on gf2's
//! parameters, which isolates the rule from both derivations.
//!
//! gf2's own list is recovered through the public `prepare_llrs`, which maps
//! `target_n` channel LLRs onto the `full_n` mother-code positions using the
//! same `transmitted_cols` the encoder applies. Tagging channel LLR `i` with
//! the value `TAG_BASE + i` makes each transmitted position identifiable and
//! keeps every tag above the zero and filler LLR values `prepare_llrs` writes
//! into untransmitted and filler positions.

use gf2_coding::ldpc::nr_5g::{kb_for_z_selection, Nr5gRateMatchedCode};
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::Llr;

const TAG_BASE: f32 = 1000.0;

/// One surveyed configuration: base graph, target codeword and message length.
///
/// The last two exercise AFF3CT's derivation where the first six do not: a
/// BG1 code whose rate also selects BG1 in AFF3CT, and a BG2 message length
/// inside `560 < K <= 640`, where the two lifting tables differ.
const CONFIGURATIONS: [(u8, usize, usize); 8] = [
    (2, 256, 121),
    (2, 512, 200),
    (2, 1024, 400),
    (1, 1024, 512),
    (1, 2048, 1024),
    (1, 4096, 2048),
    (1, 1200, 900),
    (2, 1200, 600),
];

/// Mother-code parameters derived by one project for one configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Derivation {
    base_graph: u8,
    kb_for_lifting: usize,
    lifting: usize,
    k_ldpc: usize,
    n_ldpc: usize,
}

impl std::fmt::Display for Derivation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BG{}, lifting Kb={}, Z={}, K_LDPC={}, N_LDPC={}",
            self.base_graph, self.kb_for_lifting, self.lifting, self.k_ldpc, self.n_ldpc
        )
    }
}

/// AFF3CT's base-graph choice from `(K, N)`, in its single-precision arithmetic.
fn aff3ct_base_graph(target_k: usize, target_n: usize) -> u8 {
    let rate = target_k as f32 / target_n as f32;
    if target_k <= 292 || (target_k <= 3824 && rate <= 0.67_f32) || rate <= 0.25_f32 {
        2
    } else {
        1
    }
}

/// AFF3CT's lifting rule and mother-code dimensions for a given base graph.
fn aff3ct_lifting(base_graph: u8, target_k: usize) -> Derivation {
    let (k_columns, n_columns) = if base_graph == 1 { (22, 68) } else { (10, 52) };
    // The pinned table maps 560 < K <= 640 to 8 through a branch of its own.
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
    assert!(
        lifting != 0,
        "AFF3CT finds no lifting size for K={target_k}"
    );
    Derivation {
        base_graph,
        kb_for_lifting: kb,
        lifting,
        k_ldpc: k_columns * lifting,
        n_ldpc: n_columns * lifting,
    }
}

/// gf2's parameters in the same terms.
fn gf2_derivation(code: &Nr5gRateMatchedCode) -> Derivation {
    let params = code.params();
    Derivation {
        base_graph: params.base_graph,
        kb_for_lifting: kb_for_z_selection(params.base_graph, params.target_k),
        lifting: params.lifting_factor,
        k_ldpc: params.full_k,
        n_ldpc: params.full_n,
    }
}

/// AFF3CT's transcribed selection rule on the given mother-code parameters.
fn aff3ct_selection(target_n: usize, target_k: usize, derivation: Derivation) -> Vec<usize> {
    let two_z = 2 * derivation.lifting;
    let span = derivation.n_ldpc - two_z;
    let mut selected = Vec::with_capacity(target_n);
    let mut j = 0_usize;
    while selected.len() < target_n {
        let position = j % span + two_z;
        // AFF3CT skips exactly the filler range [K, K_LDPC).
        if !(position < derivation.k_ldpc && target_k <= position) {
            selected.push(position);
        }
        j += 1;
        assert!(j < 100 * (target_n + span), "selection did not terminate");
    }
    selected
}

/// gf2's transmitted-column list, recovered through the public LLR mapping.
fn gf2_selection(code: &Nr5gRateMatchedCode) -> Vec<usize> {
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

fn main() {
    let mut own_agree = 0_usize;
    let mut fixed_agree = 0_usize;
    let mut rule_agree = 0_usize;
    for (base_graph, target_n, target_k) in CONFIGURATIONS {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(base_graph, target_n, target_k);
        let gf2 = gf2_derivation(&code);
        let gf2_positions = gf2_selection(&code);
        let own = aff3ct_lifting(aff3ct_base_graph(target_k, target_n), target_k);
        let fixed = aff3ct_lifting(base_graph, target_k);
        let rule_on_gf2 = aff3ct_selection(target_n, target_k, gf2) == gf2_positions;
        println!("BG{base_graph} target_n={target_n} target_k={target_k}; gf2 derives {gf2}");
        if own == gf2 {
            own_agree += 1;
            let same = aff3ct_selection(target_n, target_k, own) == gf2_positions;
            println!(
                "  AGREE  AFF3CT derives the same parameters from (K, N); its selection {} gf2's {} positions",
                if same { "equals" } else { "DIFFERS FROM" },
                gf2_positions.len()
            );
        } else {
            println!("  DIFFER AFF3CT derives {own} from (K, N), a different mother code");
        }
        if fixed == gf2 {
            fixed_agree += 1;
            println!("  AGREE  with the base graph fixed to BG{base_graph}, AFF3CT's lifting rule derives the same parameters");
        } else {
            println!("  DIFFER with the base graph fixed to BG{base_graph}, AFF3CT's lifting rule derives {fixed}");
        }
        if rule_on_gf2 {
            rule_agree += 1;
        }
        println!(
            "  {} AFF3CT's selection rule on gf2's parameters selects {} {} positions",
            if rule_on_gf2 { "AGREE " } else { "DIFFER" },
            if rule_on_gf2 { "the same" } else { "different" },
            gf2_positions.len()
        );
    }
    let total = CONFIGURATIONS.len();
    println!();
    println!("{own_agree}/{total} configurations: AFF3CT's own (K, N) derivation equals gf2's parameters");
    println!("{fixed_agree}/{total} configurations: AFF3CT's lifting rule on gf2's base graph equals gf2's parameters");
    println!("{rule_agree}/{total} configurations: AFF3CT's selection rule on gf2's parameters selects gf2's positions");
}
