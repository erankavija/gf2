use gf2_algebra::gray::gray_code_iter;
use permanent_wave_gpu::fixtures::{FixtureCorpus, FixtureRequirement, DEFAULT_FIXTURE_SEED};
use permanent_wave_gpu::oracle::check_cpu_reference_paths;

#[test]
fn deterministic_corpus_covers_the_structural_boundaries() {
    let first = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
    let second = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);

    assert_eq!(
        first, second,
        "the complete corpus must reproduce from its seed"
    );
    for (left, right) in first.fixtures().iter().zip(second.fixtures()) {
        assert_eq!(
            left.matrix_bytes(),
            right.matrix_bytes(),
            "{} must retain byte-identical matrix data",
            left.id()
        );
    }

    for q in [3, 5, 7] {
        assert!(first.fixtures().iter().any(|fixture| {
            fixture.q() == q && fixture.has_requirement(&FixtureRequirement::EmptyMatrix)
        }));
        assert!(first.fixtures().iter().any(|fixture| {
            fixture.q() == q && fixture.has_requirement(&FixtureRequirement::SingletonMatrix)
        }));

        let gray_fixture = first
            .fixtures()
            .iter()
            .find(|fixture| {
                fixture.q() == q
                    && fixture.has_requirement(&FixtureRequirement::GrayAdditionTransition)
                    && fixture.has_requirement(&FixtureRequirement::GraySubtractionTransition)
            })
            .expect("each field must include Gray partition boundaries");
        let FixtureRequirement::GrayPartition {
            first_index,
            last_index,
        } = gray_fixture
            .requirements()
            .iter()
            .find(|requirement| matches!(requirement, FixtureRequirement::GrayPartition { .. }))
            .expect("Gray fixture must retain its partition endpoints")
        else {
            unreachable!("matched GrayPartition above");
        };
        let transitions: Vec<_> = gray_code_iter(gray_fixture.n()).collect();
        assert_eq!(transitions[*first_index as usize - 1].1, 1);
        assert_eq!(transitions[*last_index as usize - 1].1, -1);

        assert!(first.fixtures().iter().any(|fixture| {
            fixture.q() == q
                && fixture.requirements().iter().any(|requirement| {
                    matches!(requirement, FixtureRequirement::PartialWord { .. })
                })
        }));
        assert!(first.fixtures().iter().any(|fixture| {
            fixture.q() == q
                && fixture.has_requirement(&FixtureRequirement::ZeroContainingRowProduct)
        }));
    }

    for (q, exponent_values) in [
        (3, &[1, 2][..]),
        (5, &[1, 2, 4, 3][..]),
        (7, &[1, 3, 2, 6, 4, 5][..]),
    ] {
        for (exponent, &value) in exponent_values.iter().enumerate() {
            let fixture = first
                .fixtures()
                .iter()
                .find(|fixture| {
                    fixture.q() == q
                        && fixture.has_requirement(&FixtureRequirement::NonzeroExponentClass {
                            exponent: exponent as u8,
                        })
                })
                .expect("each nonzero exponent class must have a fixture");
            assert_eq!(
                fixture.matrix_bytes(),
                [value],
                "q={q} exponent={exponent} must use its canonical field value"
            );
        }
    }

    for n in [16, 20, 24] {
        assert!(first
            .fixtures()
            .iter()
            .any(|fixture| fixture.q() == 7 && fixture.n() == n));
    }
}

#[test]
fn cpu_reference_paths_match_ryser_where_packed_reference_exists() {
    let corpus = FixtureCorpus::seeded(DEFAULT_FIXTURE_SEED);
    let rows = check_cpu_reference_paths(&corpus, 24);

    assert!(!rows.is_empty());
    assert!(rows.iter().any(|row| row.q() == 7 && row.n() == 16));
    for n in [20, 24] {
        assert!(
            !rows.iter().any(|row| row.q() == 7 && row.n() == n),
            "F_7 n={n} has no packed scalar reference"
        );
    }
    for row in rows {
        assert_eq!(
            row.mismatch_count(),
            0,
            "{} q={} n={} first={:?}",
            row.reference(),
            row.q(),
            row.n(),
            row.first_mismatch_fixture_id()
        );
    }
}
