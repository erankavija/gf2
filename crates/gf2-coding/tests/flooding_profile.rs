use gf2_coding::ldpc::dvb_t2::{concat::DEFAULT_MAX_LDPC_ITERATIONS, FrameSize};
use gf2_coding::ldpc::flooding_profile::{
    flooding_default_caps, flooding_profile_universe, ColumnFractions, FloodingProfile,
    FloodingWitness, NrBaseGraph, NrFloodingProfile, ReducedRational as Rational,
    NR_KEY_RESOLUTION,
};
use gf2_coding::ldpc::nr_5g::{all_lifting_sizes, kb_for_z_selection, SOFT_DECODE_MAX_ITERATIONS};
use gf2_coding::ldpc::{LdpcCode, QuasiCyclicLdpc};
use gf2_coding::CodeRate;
use std::collections::BTreeSet;

fn r(n: u32, d: u32) -> Rational {
    Rational::new(n, d)
}

fn nr(profile: FloodingProfile) -> NrFloodingProfile {
    let FloodingProfile::Nr5g(profile) = profile else {
        panic!("expected NR")
    };
    profile
}

fn profile(code: &LdpcCode) -> FloodingProfile {
    code.flooding_profile().expect("standard profile")
}

#[test]
fn constructors_attach_profiles_without_inferring_raw_matrices() {
    assert_eq!(
        LdpcCode::from_edges(1, 2, &[(0, 0)]).flooding_profile(),
        None
    );
    let qc = QuasiCyclicLdpc::new(vec![vec![0, 0]], 2);
    assert_eq!(LdpcCode::from_quasi_cyclic(&qc).flooding_profile(), None);
    for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
        let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(bg.number(), 2));
        let p = nr(profile(&code));
        assert_eq!(p.base_graph(), bg);
        assert_eq!(p.punctured_systematic_columns(), 0);
        assert_eq!(p.filler_start(), r(bg.systematic_columns() as u32, 1));
        assert_eq!(p.parity_end(), r(bg.columns() as u32, 1));
        assert_eq!(
            profile(&code.clone().without_quasi_cyclic()),
            profile(&code)
        );
        for col in 0..bg.columns() {
            assert_eq!(
                p.column_fractions(col),
                ColumnFractions {
                    transmitted: r(1, 1),
                    filler: r(0, 1),
                    punctured: r(0, 1),
                }
            );
        }
    }
    let rm = QuasiCyclicLdpc::nr_5g_rate_matched(2, 256, 121);
    let p = nr(profile(rm.mother_code()));
    assert_eq!(rm.params().lifting_factor, 22);
    assert_eq!(p.filler_start(), r(121, 22));
    assert_eq!(p.parity_end(), r(399, 22));
    assert_eq!(p.punctured_systematic_columns(), 2);
}

#[test]
fn fractions_match_prepared_llrs_at_bit_granularity() {
    use gf2_coding::ldpc::nr_5g::FILLER_LLR;
    use gf2_coding::llr::Llr;
    for (bg, n, k) in [(2, 256, 121), (1, 100, 40), (1, 128, 44)] {
        let code = QuasiCyclicLdpc::nr_5g_rate_matched(bg, n, k);
        let p = nr(profile(code.mother_code()));
        let z = code.params().lifting_factor;
        let llrs = code.prepare_llrs(&vec![Llr::new(1.0); n]);
        for (col, bits) in llrs.chunks_exact(z).enumerate() {
            let transmitted = bits.iter().filter(|x| **x == Llr::new(1.0)).count();
            let filler = bits.iter().filter(|x| **x == Llr::new(FILLER_LLR)).count();
            assert_eq!(
                p.column_fractions(col),
                ColumnFractions {
                    transmitted: r(transmitted as u32, z as u32),
                    filler: r(filler as u32, z as u32),
                    punctured: r((z - transmitted - filler) as u32, z as u32),
                }
            );
        }
    }
}

#[test]
fn boundary_extremes_partition_every_column() {
    for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
        let kb = bg.systematic_columns() as u32;
        let nb = bg.columns() as u32;
        for filler_start in [r(0, 1), r(1, 2), r(1, 1), r(kb * 2 - 1, 2), r(kb, 1)] {
            for parity_end in [r(kb, 1), r(nb * 2 - 1, 2), r(nb, 1)] {
                for punctured in [0, 2] {
                    let p = NrFloodingProfile::new(bg, punctured, filler_start, parity_end);
                    for col in 0..bg.columns() {
                        let mut transmitted = 0;
                        let mut filler = 0;
                        let mut punctured_bits = 0;
                        for half in 0..2 {
                            let position = r(col as u32 * 2 + half, 2);
                            if col < kb as usize && position >= filler_start {
                                filler += 1;
                            } else if col < punctured as usize || position >= parity_end {
                                punctured_bits += 1;
                            } else {
                                transmitted += 1;
                            }
                        }
                        assert_eq!(
                            p.column_fractions(col),
                            ColumnFractions {
                                transmitted: r(transmitted, 2),
                                filler: r(filler, 2),
                                punctured: r(punctured_bits, 2),
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn equal_reduced_boundaries_have_equal_identity_and_fractions() {
    assert_eq!(r(12, 22), r(6, 11));
    let a = QuasiCyclicLdpc::nr_5g_rate_matched(1, 100, 43);
    let b = QuasiCyclicLdpc::nr_5g_rate_matched(1, 200, 86);
    assert_ne!(a.params().lifting_factor, b.params().lifting_factor);
    assert_eq!(profile(a.mother_code()), profile(b.mother_code()));
    for col in 0..NrBaseGraph::Bg1.columns() {
        assert_eq!(
            nr(profile(a.mother_code())).column_fractions(col),
            nr(profile(b.mother_code())).column_fractions(col)
        );
    }
}

#[test]
fn table_key_rounds_to_nearest_quarter_with_upward_ties() {
    assert_eq!(NR_KEY_RESOLUTION, 4);
    for (input, expected) in [
        (r(43, 8), r(11, 2)),
        (r(42, 8), r(21, 4)),
        (r(41, 8), r(21, 4)),
    ] {
        let p = FloodingProfile::Nr5g(NrFloodingProfile::new(
            NrBaseGraph::Bg2,
            2,
            input,
            r(399, 22),
        ));
        let key = nr(p.table_key().profile());
        assert_eq!(key.filler_start(), expected);
        assert_eq!(key.parity_end(), r(73, 4));
        assert_eq!(p.table_key().profile().table_key(), p.table_key());
    }
}

fn build(witness: FloodingWitness) -> LdpcCode {
    match witness {
        FloodingWitness::DvbT2 { frame_size, rate } => match frame_size {
            FrameSize::Short => LdpcCode::dvb_t2_short(rate),
            FrameSize::Normal => LdpcCode::dvb_t2_normal(rate),
        },
        FloodingWitness::Nr5gMother {
            base_graph,
            lifting_factor,
        } => LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(
            base_graph.number(),
            lifting_factor,
        )),
        FloodingWitness::Nr5gRateMatched { base_graph, n, k } => {
            QuasiCyclicLdpc::nr_5g_rate_matched(base_graph.number(), n, k)
                .mother_code()
                .clone()
        }
    }
}

#[test]
fn universe_witnesses_are_accepted_and_reproduce_keys() {
    let universe = flooding_profile_universe();
    let keys: BTreeSet<_> = universe.iter().map(|e| e.key).collect();
    assert_eq!(keys.len(), universe.len());
    assert!(universe.windows(2).all(|pair| pair[0].key < pair[1].key));
    for (i, entry) in universe.iter().enumerate() {
        assert_eq!(entry.witness.profile().table_key(), entry.key);
        if let FloodingWitness::Nr5gRateMatched { base_graph, n, k } = entry.witness {
            let kb = base_graph.systematic_columns();
            let nb = base_graph.columns();
            let z = all_lifting_sizes()
                .into_iter()
                .map(usize::from)
                .find(|&z| {
                    kb_for_z_selection(base_graph.number(), k) * z >= k
                        && k + (nb - kb - 2) * z >= n
                })
                .expect("witness selects a lifting size");
            assert!(n > k && k >= 2 * z && k <= kb * z);
            let expected = NrFloodingProfile::new(
                base_graph,
                2,
                r(k as u32, z as u32),
                r((kb * z + n - k + 2 * z) as u32, z as u32),
            );
            assert_eq!(FloodingProfile::Nr5g(expected).table_key(), entry.key);
        }
        // Build a bounded sample; every witness above is checked against the constructor's inequalities.
        if !matches!(entry.witness, FloodingWitness::Nr5gRateMatched { .. })
            || i % (universe.len() / 32).max(1) == 0
        {
            assert_eq!(profile(&build(entry.witness)).table_key(), entry.key);
        }
    }
    for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
        for z in all_lifting_sizes() {
            let code =
                LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(bg.number(), z as usize));
            assert!(keys.contains(&profile(&code).table_key()));
        }
    }
}

#[test]
fn universe_covers_constructor_selection_boundaries() {
    let keys: BTreeSet<_> = flooding_profile_universe().iter().map(|e| e.key).collect();
    for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
        let kb = bg.systematic_columns();
        let nb = bg.columns();
        for z in all_lifting_sizes().into_iter().map(usize::from) {
            for k in [2 * z, kb * z, 121, 192, 193, 560, 561, 640, 641] {
                if kb_for_z_selection(bg.number(), k) * z < k || k < 2 * z {
                    continue;
                }
                for n in [k + 1, k + (nb - kb - 2) * z] {
                    let code = QuasiCyclicLdpc::nr_5g_rate_matched(bg.number(), n, k);
                    assert!(
                        keys.contains(&profile(code.mother_code()).table_key()),
                        "BG{} n={n} k={k}",
                        bg.number()
                    );
                }
            }
        }
    }
}

#[test]
fn cap_universe_uses_named_decoder_defaults() {
    let mut expected = vec![DEFAULT_MAX_LDPC_ITERATIONS, SOFT_DECODE_MAX_ITERATIONS];
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(flooding_default_caps(), expected);
    let code =
        gf2_coding::ldpc::dvb_t2::concat::DvbT2Concat::new(FrameSize::Short, CodeRate::Rate1_2)
            .unwrap();
    assert_eq!(code.max_ldpc_iterations(), DEFAULT_MAX_LDPC_ITERATIONS);
}

proptest::proptest! {
    #[test]
    fn arbitrary_fractional_patterns_partition_columns(
        bg2 in proptest::bool::ANY,
        filler in 0u32..=1024,
        parity in 0u32..=1024,
        punctured in 0u8..=22,
    ) {
        let bg = if bg2 { NrBaseGraph::Bg2 } else { NrBaseGraph::Bg1 };
        let kb = bg.systematic_columns() as u32;
        let nb = bg.columns() as u32;
        let p = NrFloodingProfile::new(bg, punctured.min(kb as u8), r(filler * kb, 1024), r(kb * 1024 + parity * (nb - kb), 1024));
        for col in 0..bg.columns() {
            let f = p.column_fractions(col);
            let parts = [f.transmitted, f.filler, f.punctured];
            let denominator: u128 = parts.iter().map(|x| u128::from(x.denominator())).product();
            let sum: u128 = parts.iter().map(|x| u128::from(x.numerator()) * (denominator / u128::from(x.denominator()))).sum();
            proptest::prop_assert_eq!(sum, denominator);
        }
    }
}

#[test]
fn universe_covers_all_small_accepted_rate_matching_inputs() {
    let keys: BTreeSet<_> = flooding_profile_universe().iter().map(|e| e.key).collect();
    let lifting: Vec<_> = all_lifting_sizes().into_iter().map(usize::from).collect();
    for bg in [NrBaseGraph::Bg1, NrBaseGraph::Bg2] {
        let kb = bg.systematic_columns();
        let nb = bg.columns();
        for k in 1..=kb * 16 {
            for n in k + 1..=k + (nb - kb - 2) * 16 {
                let z = *lifting
                    .iter()
                    .find(|&&z| {
                        kb_for_z_selection(bg.number(), k) * z >= k && k + (nb - kb - 2) * z >= n
                    })
                    .unwrap();
                if k < 2 * z {
                    continue;
                }
                let p = FloodingProfile::Nr5g(NrFloodingProfile::new(
                    bg,
                    2,
                    r(k as u32, z as u32),
                    r((kb * z + n - k + 2 * z) as u32, z as u32),
                ));
                assert!(
                    keys.contains(&p.table_key()),
                    "BG{} n={n} k={k}",
                    bg.number()
                );
            }
        }
    }
}
