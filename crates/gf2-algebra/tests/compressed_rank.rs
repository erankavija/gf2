//! Conformance suite for exact compressed-state permanental-rank propagation.
//!
//! The transition oracle below represents a subspace as its complete sorted
//! set of vectors. It constructs spans by explicit closure and contraction
//! images by applying the contraction to every element, so it shares neither
//! RREF normalization nor basis-image code with the production implementation.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use gf2_algebra::permanent::{
    permanental_rank_status, CanonicalSubspace, CompressedRankState, CompressedTransitionTable,
    ExactProbability, SupportedPrimeField, Vector3, CANONICAL_SUBSPACE_HASH_DOMAIN,
};
use gf2_algebra::testutil::permanental_rank_bruteforce;
use gf2_core::gfp::Fp;

type ResidueVector = [u8; 3];

#[derive(Clone, Debug)]
struct ExplicitSubspace {
    basis: [ResidueVector; 3],
    dimension: usize,
    elements: BTreeSet<ResidueVector>,
}

impl ExplicitSubspace {
    fn key(&self) -> [u8; 10] {
        let mut key = [0; 10];
        key[0] = self.dimension as u8;
        for (offset, value) in self.basis.iter().flatten().copied().enumerate() {
            key[offset + 1] = value;
        }
        key
    }
}

fn add(left: ResidueVector, right: ResidueVector, q: u8) -> ResidueVector {
    std::array::from_fn(|index| (left[index] + right[index]) % q)
}

fn scale(vector: ResidueVector, coefficient: u8, q: u8) -> ResidueVector {
    std::array::from_fn(|index| (vector[index] * coefficient) % q)
}

fn dot(left: ResidueVector, right: ResidueVector, q: u8) -> u8 {
    (0..3)
        .map(|index| left[index] * right[index])
        .fold(0, |sum, value| (sum + value) % q)
}

fn contraction(left: ResidueVector, right: ResidueVector, q: u8) -> ResidueVector {
    [
        (left[1] * right[2] + left[2] * right[1]) % q,
        (left[0] * right[2] + left[2] * right[0]) % q,
        (left[0] * right[1] + left[1] * right[0]) % q,
    ]
}

fn all_vectors(q: u8) -> Vec<ResidueVector> {
    let mut vectors = Vec::with_capacity(usize::from(q).pow(3));
    for first in 0..q {
        for second in 0..q {
            for third in 0..q {
                vectors.push([first, second, third]);
            }
        }
    }
    vectors
}

fn explicit_span(
    generators: impl IntoIterator<Item = ResidueVector>,
    q: u8,
) -> BTreeSet<ResidueVector> {
    let mut span = BTreeSet::from([[0, 0, 0]]);
    for generator in generators {
        let prior: Vec<_> = span.iter().copied().collect();
        for vector in prior {
            for coefficient in 1..q {
                span.insert(add(vector, scale(generator, coefficient, q), q));
            }
        }
    }
    span
}

fn enumerate_subspaces(q: u8) -> Vec<ExplicitSubspace> {
    let mut subspaces = Vec::new();
    for dimension in 0..=3 {
        for pivot_mask in 0_u8..8 {
            if pivot_mask.count_ones() as usize != dimension {
                continue;
            }
            let pivots: Vec<_> = (0..3)
                .filter(|column| pivot_mask & (1 << column) != 0)
                .collect();
            let free_positions: Vec<_> = pivots
                .iter()
                .enumerate()
                .flat_map(|(row, &pivot)| {
                    (pivot + 1..3)
                        .filter(|column| !pivots.contains(column))
                        .map(move |column| (row, column))
                })
                .collect();
            let assignments = usize::from(q).pow(free_positions.len() as u32);
            for encoded in 0..assignments {
                let mut basis = [[0; 3]; 3];
                for (row, &pivot) in pivots.iter().enumerate() {
                    basis[row][pivot] = 1;
                }
                let mut remaining = encoded;
                for &(row, column) in &free_positions {
                    basis[row][column] = (remaining % usize::from(q)) as u8;
                    remaining /= usize::from(q);
                }
                let elements = explicit_span(basis[..dimension].iter().copied(), q);
                assert!(
                    !subspaces
                        .iter()
                        .any(|known: &ExplicitSubspace| known.elements == elements),
                    "explicit RREF enumerator duplicated a q={q} subspace"
                );
                subspaces.push(ExplicitSubspace {
                    basis,
                    dimension,
                    elements,
                });
            }
        }
    }
    subspaces.sort_by_key(ExplicitSubspace::key);
    assert_eq!(
        subspaces.len(),
        2 * (usize::from(q).pow(2) + usize::from(q) + 1) + 2
    );
    subspaces
}

fn production_vector<F: SupportedPrimeField>(residues: ResidueVector) -> Vector3<F> {
    Vector3::from_residues(residues).expect("explicit vectors use canonical residues")
}

fn production_subspace<F: SupportedPrimeField>(
    explicit: &ExplicitSubspace,
) -> CanonicalSubspace<F> {
    let basis: Vec<_> = explicit.basis[..explicit.dimension]
        .iter()
        .copied()
        .map(production_vector)
        .collect();
    CanonicalSubspace::from_vectors(&basis)
}

fn subspace_index(subspaces: &[ExplicitSubspace], elements: &BTreeSet<ResidueVector>) -> usize {
    subspaces
        .iter()
        .position(|subspace| subspace.elements == *elements)
        .expect("explicit closure must be one enumerated subspace")
}

#[derive(Default)]
struct RecordingHasher(Vec<u8>);

impl Hasher for RecordingHasher {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

fn canonical_subspace_contract<F: SupportedPrimeField>() {
    let q = F::ORDER;
    let explicit = enumerate_subspaces(q);
    let production: Vec<_> = explicit.iter().map(production_subspace::<F>).collect();
    let mut ordered = BTreeSet::new();

    for (expected, observed) in explicit.iter().zip(&production) {
        assert_eq!(observed.dimension(), expected.dimension);
        assert_eq!(observed.basis_residues(), expected.basis);
        assert_eq!(observed.key_bytes(), expected.key());
        assert!(ordered.insert(observed.clone()));

        let canonical_bytes = observed.to_canonical_bytes();
        assert_eq!(canonical_bytes[0], 1);
        assert_eq!(canonical_bytes[1], q);
        assert_eq!(&canonical_bytes[2..], expected.key().as_slice());
        assert_eq!(
            CanonicalSubspace::<F>::from_canonical_bytes(&canonical_bytes),
            Ok(observed.clone())
        );

        let mut hasher = RecordingHasher::default();
        observed.hash(&mut hasher);
        let mut expected_hash = vec![CANONICAL_SUBSPACE_HASH_DOMAIN, q];
        expected_hash.extend(expected.key());
        assert_eq!(hasher.0, expected_hash);

        let reversed: Vec<_> = expected.basis[..expected.dimension]
            .iter()
            .rev()
            .copied()
            .map(production_vector::<F>)
            .collect();
        assert_eq!(CanonicalSubspace::from_vectors(&reversed), *observed);
        if let Some(first) = reversed.first().copied() {
            let redundant = [reversed.as_slice(), &[first, first]].concat();
            assert_eq!(CanonicalSubspace::from_vectors(&redundant), *observed);
        }

        for vector in all_vectors(q) {
            assert_eq!(
                observed.contains(production_vector(vector)),
                expected.elements.contains(&vector)
            );
        }
        let perpendicular = observed.orthogonal_complement();
        for vector in all_vectors(q) {
            let expected_membership = expected
                .elements
                .iter()
                .all(|basis_vector| dot(*basis_vector, vector, q) == 0);
            assert_eq!(
                perpendicular.contains(production_vector(vector)),
                expected_membership
            );
        }
    }

    assert_eq!(ordered.into_iter().collect::<Vec<_>>(), production);

    let zero = CanonicalSubspace::<F>::zero();
    let bytes = zero.to_canonical_bytes();
    let mut malformed = bytes.to_vec();
    malformed[0] = 2;
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());
    malformed = bytes.to_vec();
    malformed[1] = if q == 3 { 5 } else { 3 };
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());
    malformed = bytes.to_vec();
    malformed[2] = 4;
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());
    malformed = bytes.to_vec();
    malformed[3] = q;
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());
    malformed = bytes.to_vec();
    malformed[3] = 1;
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&bytes[..11]).is_err());

    let line = production
        .iter()
        .find(|subspace| subspace.dimension() == 1)
        .expect("every field has a line");
    malformed = line.to_canonical_bytes().to_vec();
    malformed[3] = 2;
    assert!(CanonicalSubspace::<F>::from_canonical_bytes(&malformed).is_err());

    let rows = [
        production_vector::<F>([1, 2 % q, 0]),
        production_vector::<F>([0, 1, 1]),
    ];
    let state = rows
        .iter()
        .copied()
        .try_fold(CompressedRankState::initial(), |state, row| {
            state.successor(row)
        });
    if let Some(state) = state {
        let encoded = state.to_canonical_bytes();
        assert_eq!(
            CompressedRankState::from_replayed_bytes(&encoded, &rows),
            Ok(state.clone())
        );
        assert!(CompressedRankState::from_replayed_bytes(&encoded, &rows[..1]).is_err());
        let mut wrong = encoded;
        wrong[1] = if q == 3 { 5 } else { 3 };
        assert!(CompressedRankState::<F>::from_replayed_bytes(&wrong, &rows).is_err());
    }
}

#[test]
fn canonical_subspaces_all_supported_fields() {
    canonical_subspace_contract::<Fp<3>>();
    canonical_subspace_contract::<Fp<5>>();
    canonical_subspace_contract::<Fp<7>>();
}

fn compressed_transition_contract<F: SupportedPrimeField>() {
    let q = F::ORDER;
    let universe = all_vectors(q);
    let explicit = enumerate_subspaces(q);
    let production: Vec<_> = explicit.iter().map(production_subspace::<F>).collect();

    let line_indices: Vec<_> = universe
        .iter()
        .map(|&vector| subspace_index(&explicit, &explicit_span([vector], q)))
        .collect();
    let sum_indices: Vec<Vec<_>> = explicit
        .iter()
        .map(|left| {
            explicit
                .iter()
                .map(|right| {
                    subspace_index(
                        &explicit,
                        &explicit_span(left.elements.iter().chain(&right.elements).copied(), q),
                    )
                })
                .collect()
        })
        .collect();
    let image_indices: Vec<Vec<_>> = explicit
        .iter()
        .map(|subspace| {
            universe
                .iter()
                .map(|&row| {
                    let image = subspace
                        .elements
                        .iter()
                        .map(|&vector| contraction(vector, row, q));
                    subspace_index(&explicit, &explicit_span(image, q))
                })
                .collect()
        })
        .collect();

    for (u_index, u) in explicit.iter().enumerate() {
        for (v_index, v) in explicit.iter().enumerate() {
            let state = CompressedRankState::from_subspaces_for_test(
                production[u_index].clone(),
                production[v_index].clone(),
            );
            let mut expected_multiplicities = BTreeMap::new();
            for (row_index, &row) in universe.iter().enumerate() {
                let observed_admissible = state.is_admissible(production_vector(row));
                let expected_admissible = v.elements.iter().all(|&vector| dot(vector, row, q) == 0);
                assert_eq!(observed_admissible, expected_admissible);

                let successor = state.successor(production_vector(row));
                assert_eq!(successor.is_some(), expected_admissible);
                if !expected_admissible {
                    continue;
                }

                let expected_u = sum_indices[u_index][line_indices[row_index]];
                let expected_v = sum_indices[v_index][image_indices[u_index][row_index]];
                let successor = successor.expect("admissible rows have a successor");
                assert_eq!(
                    successor.row_span().basis_residues(),
                    explicit[expected_u].basis
                );
                assert_eq!(
                    successor.contraction_span().basis_residues(),
                    explicit[expected_v].basis
                );
                *expected_multiplicities.entry(successor).or_insert(0_u64) += 1;
            }

            let observed: BTreeMap<_, _> = state
                .transition_multiplicities()
                .iter()
                .map(|transition| (transition.successor().clone(), transition.multiplicity()))
                .collect();
            assert_eq!(observed, expected_multiplicities);
            assert_eq!(
                observed.values().sum::<u64>(),
                u64::from(q).pow((3 - v.dimension) as u32)
            );
        }
    }
}

#[test]
fn compressed_transitions_q3_all_subspaces() {
    compressed_transition_contract::<Fp<3>>();
}

#[test]
fn compressed_transitions_q5_all_subspaces() {
    compressed_transition_contract::<Fp<5>>();
}

#[test]
#[ignore = "slow: exhaustive q=7 compressed transitions visit 4,615,408 ordered cases"]
fn compressed_transitions_q7_all_subspaces_slow() {
    compressed_transition_contract::<Fp<7>>();
}

fn increment_encoded_row(digits: &mut [u8], q: u8) -> bool {
    for digit in digits {
        *digit += 1;
        if *digit < q {
            return true;
        }
        *digit = 0;
    }
    false
}

fn compressed_matrix_contract<F: SupportedPrimeField>(n: usize, expected_count: Option<&str>) {
    let q = F::ORDER;
    let mut encoded = vec![0_u8; 3 * n];
    let matrix_total = usize::from(q).pow((3 * n) as u32);
    let mut compressed_count = 0_u64;
    for matrix_index in 0..matrix_total {
        let rows: Vec<_> = encoded
            .chunks_exact(3)
            .map(|row| production_vector::<F>([row[0], row[1], row[2]]))
            .collect();
        let compressed = rows
            .iter()
            .copied()
            .try_fold(CompressedRankState::initial(), |state, row| {
                state.successor(row)
            })
            .is_some();
        let field_matrix: Vec<_> = rows.iter().flat_map(|row| row.elements()).collect();
        let production = permanental_rank_status::<F>(&field_matrix, n, 3).is_deficient();
        let oracle = permanental_rank_bruteforce::<F>(&field_matrix, n, 3).is_deficient();
        assert_eq!(
            compressed, production,
            "compressed mismatch at encoded matrix {matrix_index}"
        );
        assert_eq!(
            production, oracle,
            "predicate/oracle mismatch at encoded matrix {matrix_index}"
        );
        compressed_count += u64::from(compressed);
        if matrix_index + 1 != matrix_total {
            assert!(increment_encoded_row(&mut encoded, q));
        }
    }

    let table = CompressedTransitionTable::<F>::new();
    let exact = table.deficiency_probability(n);
    assert_eq!(exact.zero_count().to_string(), compressed_count.to_string());
    assert_eq!(exact.matrix_count().to_string(), matrix_total.to_string());
    if let Some(expected) = expected_count {
        assert_eq!(exact.zero_count().to_string(), expected);
    }
}

#[test]
fn compressed_matrices_q3_n3_k3() {
    compressed_matrix_contract::<Fp<3>>(3, Some("8163"));
}

#[test]
fn compressed_matrices_q3_n4_k3() {
    compressed_matrix_contract::<Fp<3>>(4, None);
}

#[test]
#[ignore = "slow: exhaustive q=5, n=3 compressed matrix validation enumerates 5^9 matrices"]
fn compressed_matrices_q5_n3_k3_slow() {
    compressed_matrix_contract::<Fp<5>>(3, Some("439525"));
}

#[test]
#[ignore = "slow: exhaustive q=7, n=3 compressed matrix validation enumerates 7^9 matrices"]
fn compressed_matrices_q7_n3_k3_slow() {
    compressed_matrix_contract::<Fp<7>>(3, Some("6188455"));
}

fn assert_exact(probability: &ExactProbability, count: &str, total: &str, reduced: (&str, &str)) {
    assert_eq!(probability.zero_count().to_string(), count);
    assert_eq!(probability.matrix_count().to_string(), total);
    assert_eq!(
        probability.reduced_decimal(),
        (reduced.0.to_owned(), reduced.1.to_owned())
    );
}

fn initial_and_anchor_contract<F: SupportedPrimeField>(
    order_three_count: &str,
    reduced: (&str, &str),
) {
    let q = u64::from(F::ORDER);
    let table = CompressedTransitionTable::<F>::new();
    assert_eq!(table.initial_state(), &CompressedRankState::initial());
    assert_eq!(
        table
            .transitions(table.initial_state())
            .iter()
            .map(|edge| edge.multiplicity())
            .sum::<u64>(),
        q.pow(3)
    );
    assert_exact(&table.deficiency_probability(0), "1", "1", ("1", "1"));
    assert_exact(
        &table.deficiency_probability(1),
        &q.pow(3).to_string(),
        &q.pow(3).to_string(),
        ("1", "1"),
    );
    assert_exact(
        &table.deficiency_probability(2),
        &q.pow(6).to_string(),
        &q.pow(6).to_string(),
        ("1", "1"),
    );
    assert_exact(
        &table.deficiency_probability(3),
        order_three_count,
        &q.pow(9).to_string(),
        reduced,
    );

    for n in 0..=3 {
        let counts = table.counts_after_rows(n);
        let summed: String = counts
            .values()
            .cloned()
            .sum::<num_bigint::BigUint>()
            .to_string();
        assert_eq!(
            summed,
            table.deficiency_probability(n).zero_count().to_string()
        );
        assert!(counts
            .keys()
            .all(|state| table.transitions(state).len() > 0));
    }
}

#[test]
fn compressed_initial_and_anchor_counts() {
    initial_and_anchor_contract::<Fp<3>>("8163", ("907", "2187"));
    initial_and_anchor_contract::<Fp<5>>("439525", ("17581", "78125"));
    initial_and_anchor_contract::<Fp<7>>("6188455", ("126295", "823543"));
}

fn deficient_count_by_predicates<F: SupportedPrimeField>(n: usize, k: usize) -> u64 {
    let q = F::ORDER;
    let mut encoded = vec![0_u8; n * k];
    let total = usize::from(q).pow((n * k) as u32);
    let mut count = 0;
    for matrix_index in 0..total {
        let matrix: Vec<_> = encoded
            .iter()
            .copied()
            .map(|entry| F::from_residue(entry).expect("enumerator residue is canonical"))
            .collect();
        let production = permanental_rank_status(&matrix, n, k);
        let oracle = permanental_rank_bruteforce(&matrix, n, k);
        assert_eq!(
            production, oracle,
            "q={q}, n={n}, k={k}, matrix={matrix_index}"
        );
        count += u64::from(production.is_deficient());
        if matrix_index + 1 != total {
            assert!(increment_encoded_row(&mut encoded, q));
        }
    }
    count
}

fn k2_formula(q: u64, n: u64) -> u64 {
    2 * q.pow(n as u32) - 1 + n * (q - 1).pow(2) + n * (n - 1) / 2 * (q - 1).pow(3)
}

#[test]
fn compressed_k1_k2_formula_anchors() {
    for q in [3_u64, 5, 7] {
        for n in 1..=8 {
            let probability = ExactProbability::from_counts(1_u64, q.pow(n));
            assert_exact(
                &probability,
                "1",
                &q.pow(n).to_string(),
                ("1", &q.pow(n).to_string()),
            );
        }
    }

    let cases = [
        (3_u64, 3_usize, 89_u64),
        (3, 4, 225),
        (5, 3, 489),
        (7, 3, 1_441),
    ];
    for (q, n, literal) in cases {
        assert_eq!(k2_formula(q, n as u64), literal);
        let observed = match q {
            3 => deficient_count_by_predicates::<Fp<3>>(n, 2),
            5 => deficient_count_by_predicates::<Fp<5>>(n, 2),
            7 => deficient_count_by_predicates::<Fp<7>>(n, 2),
            _ => unreachable!(),
        };
        assert_eq!(observed, literal);
    }
}
