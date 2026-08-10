//! Deterministic structural fixtures for the permanent wave study.
//!
//! The random fixtures deliberately reuse the feasibility harness's
//! [`MeasurementPurpose::Equivalence`][permanent_sampling_feas::sampler::MeasurementPurpose::Equivalence]
//! domain.  That vocabulary and its seed-address encoding are canonical for
//! this study family: this crate must not mint another purpose tag merely
//! because it consumes a more structured corpus.

use permanent_sampling_feas::sampler::{MatrixSampler, MeasurementPurpose};

/// Root seed used by the committed fixture-reproduction command.
pub const DEFAULT_FIXTURE_SEED: u64 = 0x6D0F_F83C_CAFE_2026;

/// Number of uniform matrices added to every explicitly sampled corpus cell.
///
/// Structural cases are intentionally few and targeted.  These independent,
/// domain-separated matrices provide the observations used by the zero-path
/// frequency report without turning the fast test tier into a timing study.
pub const UNIFORM_FIXTURES_PER_CELL: u64 = 4;

/// An observable algorithm boundary exercised by a fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureRequirement {
    /// The accepted `0 × 0` permanent case.
    EmptyMatrix,
    /// An accepted `1 × 1` permanent case.
    SingletonMatrix,
    /// First and last sequential indices of one non-empty Gray partition.
    GrayPartition {
        /// First sequential Gray index in the partition, inclusive.
        first_index: u64,
        /// Last sequential Gray index in the partition, inclusive.
        last_index: u64,
    },
    /// A Gray transition that enters a column.
    GrayAdditionTransition,
    /// A Gray transition that removes a column.
    GraySubtractionTransition,
    /// A row count that leaves an active partial packed word.
    PartialWord {
        /// Rows whose lanes contribute to the horizontal product.
        active_lanes: usize,
        /// Physical lanes in the final packed word.
        lanes_per_word: usize,
    },
    /// A row-product evaluation containing at least one zero row sum.
    ZeroContainingRowProduct,
    /// A value in a specified nonzero multiplicative exponent class.
    NonzeroExponentClass {
        /// Exponent relative to the field's documented generator.
        exponent: u8,
    },
    /// A matrix drawn uniformly from the domain-separated equivalence stream.
    UniformSample,
}

/// One named row-major square fixture.
///
/// Matrix bytes use the canonical field-value encoding: one byte per entry in
/// row-major order, with every byte strictly smaller than [`Self::q`].  This
/// keeps reproducibility inspectable without exposing a representation-specific
/// packed matrix type to future candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixture {
    id: String,
    q: u64,
    n: usize,
    matrix: Vec<u8>,
    requirements: Vec<FixtureRequirement>,
}

impl Fixture {
    fn new(
        id: impl Into<String>,
        q: u64,
        n: usize,
        matrix: Vec<u8>,
        requirements: Vec<FixtureRequirement>,
    ) -> Self {
        assert!(matches!(q, 3 | 5 | 7), "fixture field q={q} is unsupported");
        assert_eq!(matrix.len(), n * n, "fixture matrix must be square");
        assert!(
            matrix.iter().all(|&value| u64::from(value) < q),
            "fixture values must be canonical F_{q} representatives"
        );
        Self {
            id: id.into(),
            q,
            n,
            matrix,
            requirements,
        }
    }

    /// Stable fixture identifier reported on the first mismatch.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Prime field order of this fixture.
    #[must_use]
    pub const fn q(&self) -> u64 {
        self.q
    }

    /// Square matrix order.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Canonical row-major matrix bytes.
    #[must_use]
    pub fn matrix_bytes(&self) -> &[u8] {
        &self.matrix
    }

    /// Structural requirements intentionally exercised by this fixture.
    #[must_use]
    pub fn requirements(&self) -> &[FixtureRequirement] {
        &self.requirements
    }

    /// Whether this fixture carries a given structural requirement.
    #[must_use]
    pub fn has_requirement(&self, requirement: &FixtureRequirement) -> bool {
        self.requirements.contains(requirement)
    }
}

/// Complete deterministic fixture set associated with one root seed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureCorpus {
    seed: u64,
    fixtures: Vec<Fixture>,
}

impl FixtureCorpus {
    /// Construct the complete corpus from one explicit root seed.
    ///
    /// Changing the seed changes only fixtures tagged
    /// [`FixtureRequirement::UniformSample`]; structural boundary fixtures
    /// remain deliberately literal and therefore auditable.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        let mut fixtures = Vec::new();
        add_field_boundaries(&mut fixtures, 3, seed, 63, 64, &[1, 2]);
        add_field_boundaries(&mut fixtures, 5, seed, 63, 64, &[1, 2, 4, 3]);
        add_field_boundaries(&mut fixtures, 7, seed, 15, 16, &[1, 3, 2, 6, 4, 5]);

        for &(q, n) in &[(3, 4), (5, 4), (7, 4), (7, 16), (7, 20), (7, 24)] {
            for stream_index in 0..UNIFORM_FIXTURES_PER_CELL {
                fixtures.push(uniform_fixture(seed, q, n, stream_index));
            }
        }

        Self { seed, fixtures }
    }

    /// Root seed that addresses the corpus's uniform streams.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Fixtures in stable report and reproduction order.
    #[must_use]
    pub fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }
}

fn add_field_boundaries(
    fixtures: &mut Vec<Fixture>,
    q: u64,
    seed: u64,
    partial_word_lanes: usize,
    packed_word_lanes: usize,
    exponent_values: &[u8],
) {
    fixtures.push(Fixture::new(
        format!("q{q}-n0-empty"),
        q,
        0,
        Vec::new(),
        vec![FixtureRequirement::EmptyMatrix],
    ));
    fixtures.push(Fixture::new(
        format!("q{q}-n1-zero"),
        q,
        1,
        vec![0],
        vec![FixtureRequirement::SingletonMatrix],
    ));
    for (exponent, &value) in exponent_values.iter().enumerate() {
        fixtures.push(Fixture::new(
            format!("q{q}-n1-exp{exponent}"),
            q,
            1,
            vec![value],
            vec![
                FixtureRequirement::SingletonMatrix,
                FixtureRequirement::NonzeroExponentClass {
                    exponent: exponent as u8,
                },
            ],
        ));
    }

    // Partition the non-empty n=4 Gray range [1, 15] into [1, 7] and [8, 15].
    // Index 8 enters bit 3; index 15 removes bit 0.  The requirements expose
    // both endpoints, and integration tests independently recover their signs
    // from gf2-algebra's canonical Gray iterator.
    fixtures.push(Fixture::new(
        format!("q{q}-n4-gray-partition-1"),
        q,
        4,
        patterned_matrix(q, 4),
        vec![
            FixtureRequirement::GrayPartition {
                first_index: 8,
                last_index: 15,
            },
            FixtureRequirement::GrayAdditionTransition,
            FixtureRequirement::GraySubtractionTransition,
        ],
    ));

    fixtures.push(Fixture::new(
        format!("q{q}-n{partial_word_lanes}-partial-word"),
        q,
        partial_word_lanes,
        patterned_matrix(q, partial_word_lanes),
        vec![FixtureRequirement::PartialWord {
            active_lanes: partial_word_lanes,
            lanes_per_word: packed_word_lanes,
        }],
    ));

    // A zero first row guarantees that the horizontal product has a zero
    // factor at every non-empty subset, independent of a candidate's packing.
    fixtures.push(Fixture::new(
        format!("q{q}-n3-zero-product"),
        q,
        3,
        vec![0, 0, 0, 1, 2 % q as u8, 1, 2 % q as u8, 1, 2 % q as u8],
        vec![FixtureRequirement::ZeroContainingRowProduct],
    ));

    // Give every nonempty structural cell an independently addressed uniform
    // observation.  The global n=4 sampling below supplies that cell already.
    for n in [1, 3, partial_word_lanes] {
        fixtures.push(uniform_fixture(seed, q, n, UNIFORM_FIXTURES_PER_CELL));
    }
}

fn patterned_matrix(q: u64, n: usize) -> Vec<u8> {
    (0..n * n)
        .map(|index| ((index * 3 + n) as u64 % q) as u8)
        .collect()
}

fn uniform_fixture(seed: u64, q: u64, n: usize, stream_index: u64) -> Fixture {
    let matrix = match q {
        3 => MatrixSampler::new(seed, q, n, MeasurementPurpose::Equivalence, stream_index)
            .next_matrix::<3>(n)
            .into_iter()
            .map(|value| value.value() as u8)
            .collect(),
        5 => MatrixSampler::new(seed, q, n, MeasurementPurpose::Equivalence, stream_index)
            .next_matrix::<5>(n)
            .into_iter()
            .map(|value| value.value() as u8)
            .collect(),
        7 => MatrixSampler::new(seed, q, n, MeasurementPurpose::Equivalence, stream_index)
            .next_matrix::<7>(n)
            .into_iter()
            .map(|value| value.value() as u8)
            .collect(),
        _ => unreachable!("FixtureCorpus validates the field set"),
    };
    Fixture::new(
        format!("q{q}-n{n}-uniform-{stream_index:02}"),
        q,
        n,
        matrix,
        vec![FixtureRequirement::UniformSample],
    )
}
