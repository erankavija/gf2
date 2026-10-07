//! Both forms compute the product the generic reference computes.

use spmm_check_arm::{prepare, Case, Form, MEDIUM_PRIME, SMALL_PRIME};

fn cells(case: &Case, form: Form) -> (Vec<u64>, String) {
    let workload = prepare(case, form).expect("an AVX2 host");
    let mut body = workload.body;
    body();
    ((workload.cells)(), (workload.observe)())
}

/// `out[r][j] = Σ_h a[r][h] * b[col_h][j] mod p`, from the case's own operand
/// stream.
fn reference(
    prime: u64,
    rows: usize,
    nonzeros_per_row: usize,
    dense_cols: usize,
    seed: u64,
) -> Vec<u64> {
    use tuning_campaign_support::abtest::SplitMix64;
    let mut rng = SplitMix64::new(seed);
    let mut entries = Vec::new();
    for row in 0..rows {
        for _ in 0..nonzeros_per_row {
            let col = rng.next_u64() as usize % rows;
            entries.push((row, col, 1 + rng.next_u64() % (prime - 1)));
        }
    }
    let mut rng = SplitMix64::new(seed ^ 0x5eed);
    let b: Vec<u64> = (0..rows * dense_cols)
        .map(|_| rng.next_u64() % prime)
        .collect();
    let mut out = vec![0u64; rows * dense_cols];
    for (row, col, value) in entries {
        for j in 0..dense_cols {
            let cell = &mut out[row * dense_cols + j];
            *cell = (*cell + value * b[col * dense_cols + j]) % prime;
        }
    }
    out
}

#[test]
fn both_forms_match_the_reference_on_both_lanes() {
    for prime in [SMALL_PRIME, MEDIUM_PRIME] {
        for dense_cols in [1, 16, 33] {
            let (rows, nonzeros_per_row, seed) = (48, 5, 9);
            let case = Case::SparseMatmat {
                prime,
                rows,
                nonzeros_per_row,
                dense_cols,
                seed,
            };
            let expected = reference(prime, rows, nonzeros_per_row, dense_cols, seed);
            for (form, name) in [
                (Form::Checked, "form=checked"),
                (Form::Unchecked, "form=unchecked"),
            ] {
                let (found, path) = cells(&case, form);
                assert_eq!(found, expected, "{path}");
                assert!(path.ends_with(name), "{path}");
            }
        }
    }
}

#[test]
fn a_prime_outside_the_family_is_refused() {
    let case = Case::SparseMatmat {
        prime: 11,
        rows: 4,
        nonzeros_per_row: 1,
        dense_cols: 4,
        seed: 1,
    };
    assert!(prepare(&case, Form::Checked).is_err());
}
