//! Equivalence gate for the AFF3CT 5G NR LLR de-rate-matching adapter.
//!
//! For every configuration it compares the pinned AFF3CT library's own
//! `build_5G_base_graph(K, N)` derivation with gf2's parameters. A
//! configuration whose base graph, lifting size or mother-code dimensions
//! differ is reported `NON-EQUIVALENT` and is not eligible for timing.
//!
//! For the rest it runs three checks on two seeded channel frames:
//!
//! 1. Raw AFF3CT semantics. The output buffer is pre-filled with NaN, so every
//!    position AFF3CT leaves unwritten stays NaN. The written positions must
//!    be exactly gf2's transmitted positions with bit-identical channel LLRs,
//!    the `2*Z` punctured prefix at zero and the filler range at +infinity.
//!    The unwritten positions must be exactly gf2's untransmitted parity.
//! 2. gf2's fillers are the range `[K, K_LDPC)`, so the adapter's
//!    normalization targets the positions gf2 writes.
//! 3. The adapter's whole-consumer output equals gf2's `prepare_llrs` output
//!    bit-for-bit at every mother-code position.
//!
//! It exits nonzero when an equivalent-parameter configuration fails a check.

use gf2_coding::Llr;
use survey_nr_derate::aff3ct::{base_graph, Adapter, Depuncturer};
use survey_nr_derate::{
    gf2_code, identical_positions, observed_fillers, seeded_llr_banks, CONFIGURATIONS,
};

/// Seed of the validation frames; the timed cells use their own seeds.
const VALIDATION_SEED: u64 = 0x6e72_5f64_6572_6174;

fn main() {
    let mut equivalent = 0_usize;
    let mut non_equivalent = 0_usize;
    let mut failures = 0_usize;
    for configuration in CONFIGURATIONS {
        let code = gf2_code(configuration);
        let params = code.params().clone();
        let header = format!(
            "{} (gf2: BG{}, Z={}, full_k={}, full_n={}, fillers={})",
            configuration.name,
            params.base_graph,
            params.lifting_factor,
            params.full_k,
            params.full_n,
            params.full_k - params.target_k
        );
        let Some(derived) = base_graph(configuration.target_k, configuration.target_n) else {
            non_equivalent += 1;
            println!(
                "NON-EQUIVALENT {header}: AFF3CT rejects K={}, N={}",
                configuration.target_k, configuration.target_n
            );
            continue;
        };
        let same = derived.base_graph == i32::from(params.base_graph)
            && derived.lifting as usize == params.lifting_factor
            && derived.k_ldpc as usize == params.full_k
            && derived.n_ldpc as usize == params.full_n;
        if !same {
            non_equivalent += 1;
            println!(
                "NON-EQUIVALENT {header}: AFF3CT derives BG{}, Z={}, K_LDPC={}, N_LDPC={}; not timed",
                derived.base_graph, derived.lifting, derived.k_ldpc, derived.n_ldpc
            );
            continue;
        }

        let (filler_positions, filler) = observed_fillers(&code);
        let depuncturer = Depuncturer::new(configuration.target_k, configuration.target_n)
            .unwrap_or_else(|error| panic!("{error}"));
        let filler_range: Vec<usize> = depuncturer.filler_range().collect();
        let adapter = Adapter::new(depuncturer, filler.unwrap_or_else(Llr::zero));
        let two_z = 2 * params.lifting_factor;
        let mut problems = Vec::new();
        if filler_positions != filler_range {
            problems.push("gf2's filler positions differ from [K, K_LDPC)".to_owned());
        }

        let mut counts = (0, 0, 0, 0);
        for channel in seeded_llr_banks(VALIDATION_SEED ^ params.full_n as u64, 2, params.target_n)
        {
            let reference = code.prepare_llrs(&channel);
            let input: Vec<f32> = channel.iter().map(|llr| llr.value()).collect();
            let mut raw = vec![f32::NAN; params.full_n];
            adapter.depuncturer().depuncture(&input, &mut raw);

            let (mut selected, mut prefix, mut fillers, mut unwritten) = (0, 0, 0, 0);
            for (position, (&value, expected)) in raw.iter().zip(&reference).enumerate() {
                let in_prefix = position < two_z;
                let in_fillers = filler_range.contains(&position);
                let ok = if value.is_nan() {
                    unwritten += 1;
                    // Unwritten positions must be gf2's untransmitted zeros.
                    !in_prefix && !in_fillers && expected.value() == 0.0
                } else if in_prefix {
                    prefix += 1;
                    value == 0.0 && expected.value() == 0.0
                } else if in_fillers {
                    fillers += 1;
                    value == f32::INFINITY
                } else {
                    selected += 1;
                    value.to_bits() == expected.value().to_bits()
                };
                if !ok {
                    problems.push(format!(
                        "raw AFF3CT output disagrees at position {position}"
                    ));
                    break;
                }
            }
            if selected != params.target_n {
                problems.push(format!(
                    "AFF3CT wrote {selected} selected positions, expected {}",
                    params.target_n
                ));
            }
            counts = (selected, prefix, fillers, unwritten);

            let adapted = adapter.prepare_llrs(&channel);
            let identical = identical_positions(&adapted, &reference);
            if adapted.len() != reference.len() || identical != reference.len() {
                problems.push(format!(
                    "adapter output matches gf2 at {identical}/{} positions",
                    reference.len()
                ));
            }
        }

        if problems.is_empty() {
            equivalent += 1;
            let (selected, prefix, fillers, unwritten) = counts;
            println!(
                "PASS {header}: 2 frames, adapter output identical at {}/{} positions; raw AFF3CT writes {selected} channel LLRs, {prefix} prefix zeros and {fillers} +inf fillers, leaves {unwritten} positions unwritten, all gf2 untransmitted zeros; gf2 filler value {}",
                params.full_n,
                params.full_n,
                filler.map_or_else(|| "none".to_owned(), |llr| llr.value().to_string())
            );
        } else {
            failures += 1;
            println!("FAIL {header}: {}", problems.join("; "));
        }
    }
    println!();
    println!(
        "{equivalent} equivalent and passing, {non_equivalent} non-equivalent (not timed), {failures} failing, out of {}",
        CONFIGURATIONS.len()
    );
    if failures > 0 {
        std::process::exit(1);
    }
}
