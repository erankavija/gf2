//! The arm's workloads reach the kernels they are declared for.

use entry_check_arm::{observed_entry_form, prepare, Case, MEDIUM_PRIME, SMALL_PRIME};

fn observed(case: Case) -> String {
    let form = observed_entry_form().expect("an AVX2 host");
    let workload = prepare(&case, form).expect("a prepared workload");
    let mut body = workload.body;
    body();
    drop(body);
    (workload.observe)()
}

#[test]
fn ple_reaches_the_panel_kernel_on_both_lanes() {
    for prime in [SMALL_PRIME, MEDIUM_PRIME] {
        let path = observed(Case::Ple {
            prime,
            n: 96,
            seed: 3,
        });
        assert!(!path.contains("panel-cols=none"), "{path}");
    }
}

#[test]
fn gemm_reports_an_executed_prime_route() {
    for prime in [SMALL_PRIME, MEDIUM_PRIME] {
        let path = observed(Case::Gemm {
            prime,
            n: 32,
            seed: 4,
        });
        assert!(!path.contains("route=none"), "{path}");
    }
}

#[test]
fn sparse_product_and_permanent_run() {
    for prime in [SMALL_PRIME, MEDIUM_PRIME] {
        let case = Case::SparseMatmat {
            prime,
            rows: 64,
            nonzeros_per_row: 4,
            dense_cols: 16,
            seed: 5,
        };
        assert!(observed(case).contains("dense-cols=16"));
    }
    assert!(observed(Case::PermanentF3 { n: 8, seed: 6 }).contains("n=8"));
}

#[test]
fn a_prime_outside_the_family_is_refused() {
    let form = observed_entry_form().expect("an AVX2 host");
    assert!(prepare(
        &Case::Ple {
            prime: 11,
            n: 4,
            seed: 1
        },
        form
    )
    .is_err());
}
