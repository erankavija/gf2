//! Whole-consumer workloads of the entry-check cost family (jit:bfc2ceab).
//!
//! Each workload calls one shipped consumer whose kernel is reached through a
//! caller-trusted entry point of `gf2-kernels-simd`. The two arms of a pair
//! are two builds of this crate that differ in `--cfg gf2_entry_checked`
//! alone; [`observed_entry_form`] reads from the linked kernel crate which
//! form the build publishes.

use gf2_algebra::packed::bipedal3::Bipedal3Matrix;
use gf2_algebra::permanent::bipedal3::permanent_bipedal3_singleword_simd;
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::ple::{
    max_effective_panel_dispatch_cols, reset_max_effective_panel_dispatch_cols,
};
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::FieldVec;
use gf2_core::gfp::simd_ops::{
    last_executed_prime_gemm_route, reset_last_executed_prime_gemm_route,
};
use gf2_core::gfp::Fp;
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use std::panic::{catch_unwind, set_hook, take_hook};
use tuning_campaign_support::abtest::SplitMix64;

/// Names the entry-point form the plan expects the build to publish.
pub const FORM_VAR: &str = "GF2_ENTRY_FORM";

/// The small prime of the byte lane and the medium prime of the u16 lane.
pub const SMALL_PRIME: u64 = 7;
pub const MEDIUM_PRIME: u64 = 65521;

/// One cell's case, tagged by the consumer it measures.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "consumer", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Case {
    /// `SparseFieldMatrix::matmat` of a `rows × rows` sparse matrix with
    /// `nonzeros_per_row` entries per row and a `rows × dense_cols` matrix.
    SparseMatmat {
        prime: u64,
        rows: usize,
        nonzeros_per_row: usize,
        dense_cols: usize,
        seed: u64,
    },
    /// `FieldMatrix::ple` of an `n × n` matrix.
    Ple { prime: u64, n: usize, seed: u64 },
    /// `permanent_bipedal3_singleword_simd` of an `n × n` matrix over F_3.
    PermanentF3 { n: usize, seed: u64 },
    /// `gemm` of two `n × n` matrices.
    Gemm { prime: u64, n: usize, seed: u64 },
}

/// A prepared consumer call and the route it reports after running.
pub struct Workload {
    /// One consumer call.
    pub body: Box<dyn FnMut()>,
    /// The route provenance, read after the measured calls.
    pub observe: Box<dyn FnOnce() -> String>,
}

/// The entry-point form the linked kernel crate publishes, observed from its
/// behaviour: a one-cell zero window has no pivot, so the kernel reads no
/// inverse-table entry. Today's wrapper passes an empty table on to the
/// kernel, which returns rank 0 or, in a build with debug assertions, stops
/// at its own table-length assertion; the checked wrapper refuses a table
/// shorter than the prime under its own message.
pub fn observed_entry_form() -> Result<&'static str, String> {
    const CHECKED_REFUSAL: &str = "ple_panel_base_canonical: inv_table is shorter than p";
    const KERNEL_DEBUG_ASSERTION: &str = "ple_panel_base_canonical: inv_table length";
    let fns = gf2_kernels_simd::fp_small_ple::detect()
        .ok_or("the host detected no AVX2 kernel bundle, so no entry point is measured")?;
    let quiet = take_hook();
    set_hook(Box::new(|_| {}));
    let outcome = catch_unwind(|| {
        (fns.ple_panel_base_fn)(
            &mut [0],
            1,
            1,
            SMALL_PRIME as u8,
            &[],
            &mut [0],
            &mut Vec::new(),
        )
    });
    set_hook(quiet);
    match outcome {
        Ok(0) => Ok("current"),
        Ok(rank) => Err(format!("the probe panel of rank 0 reports rank {rank}")),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|text| (*text).to_owned())
                })
                .unwrap_or_default();
            if message.starts_with(CHECKED_REFUSAL) {
                Ok("checked")
            } else if message.contains(KERNEL_DEBUG_ASSERTION) {
                Ok("current")
            } else {
                Err(format!(
                    "the probe panel stops with an unknown message: {message}"
                ))
            }
        }
    }
}

/// Refuses a build whose observed form is not the one [`FORM_VAR`] names.
pub fn verify_entry_form() -> Result<&'static str, String> {
    let declared = std::env::var(FORM_VAR).map_err(|_| format!("{FORM_VAR} is unset"))?;
    let observed = observed_entry_form()?;
    if declared != observed || (observed == "checked") != cfg!(gf2_entry_checked) {
        return Err(format!(
            "the plan declares the {declared} form and this build publishes the {observed} form"
        ));
    }
    Ok(observed)
}

fn element<const P: u64>(rng: &mut SplitMix64) -> Fp<P> {
    Fp::<P>::new(rng.next_u64() % P)
}

fn nonzero<const P: u64>(rng: &mut SplitMix64) -> Fp<P> {
    Fp::<P>::new(1 + rng.next_u64() % (P - 1))
}

fn dense<const P: u64>(rows: usize, cols: usize, rng: &mut SplitMix64) -> FieldMatrix<Fp<P>> {
    let rows = (0..rows)
        .map(|_| FieldVec::from((0..cols).map(|_| element::<P>(rng)).collect::<Vec<_>>()))
        .collect();
    FieldMatrix::from_rows(rows)
}

fn sparse_matmat<const P: u64>(
    rows: usize,
    nonzeros_per_row: usize,
    dense_cols: usize,
    seed: u64,
    form: &'static str,
) -> Workload {
    let mut rng = SplitMix64::new(seed);
    let triplets = (0..rows).flat_map(|row| {
        (0..nonzeros_per_row)
            .map(|_| (row, rng.next_u64() as usize % rows, nonzero::<P>(&mut rng)))
            .collect::<Vec<_>>()
    });
    let a = SparseFieldMatrix::from_triplets(rows, rows, triplets.collect::<Vec<_>>());
    let b = dense::<P>(rows, dense_cols, &mut SplitMix64::new(seed ^ 0x5eed));
    let stored = a.nnz();
    Workload {
        body: Box::new(move || {
            black_box(black_box(&a).matmat(black_box(&b)));
        }),
        observe: Box::new(move || {
            format!(
                "gf2-core/SparseFieldMatrix<Fp<{P}>>::matmat/rows={rows}/nnz={stored}\
                 /dense-cols={dense_cols}/entry-form={form}"
            )
        }),
    }
}

fn ple<const P: u64>(n: usize, seed: u64, form: &'static str) -> Workload {
    let a = dense::<P>(n, n, &mut SplitMix64::new(seed));
    reset_max_effective_panel_dispatch_cols();
    Workload {
        body: Box::new(move || {
            black_box(black_box(&a).ple());
        }),
        observe: Box::new(move || {
            let panel = max_effective_panel_dispatch_cols()
                .map_or("none".to_owned(), |cols| cols.to_string());
            format!("gf2-core/FieldMatrix<Fp<{P}>>::ple/n={n}/panel-cols={panel}/entry-form={form}")
        }),
    }
}

fn prime_gemm<const P: u64>(n: usize, seed: u64, form: &'static str) -> Workload {
    let a = dense::<P>(n, n, &mut SplitMix64::new(seed));
    let b = dense::<P>(n, n, &mut SplitMix64::new(seed ^ 0x5eed));
    reset_last_executed_prime_gemm_route();
    Workload {
        body: Box::new(move || {
            black_box(gemm(black_box(&a), black_box(&b)));
        }),
        observe: Box::new(move || {
            let route = last_executed_prime_gemm_route()
                .map_or("none".to_owned(), |route| format!("{route:?}"));
            format!("gf2-core/gemm<Fp<{P}>>/n={n}/route={route}/entry-form={form}")
        }),
    }
}

fn permanent_f3(n: usize, seed: u64, form: &'static str) -> Result<Workload, String> {
    let fns = gf2_kernels_simd::bipedal::detect_avx2()
        .ok_or("the host detected no AVX2 bipedal bundle")?;
    let mut rng = SplitMix64::new(seed);
    // Entries in {1, 2}: a zero row would end the permanent before its walk.
    let cells: Vec<Fp<3>> = (0..n * n).map(|_| nonzero::<3>(&mut rng)).collect();
    let matrix = Bipedal3Matrix::from_row_major(&cells, n, n);
    Ok(Workload {
        body: Box::new(move || {
            black_box(permanent_bipedal3_singleword_simd(black_box(&matrix), &fns));
        }),
        observe: Box::new(move || {
            format!("gf2-algebra/permanent_bipedal3_singleword_simd/n={n}/entry-form={form}")
        }),
    })
}

/// Prepares the consumer a case declares, for the build's observed `form`.
pub fn prepare(case: &Case, form: &'static str) -> Result<Workload, String> {
    let lane = |prime: u64| match prime {
        SMALL_PRIME | MEDIUM_PRIME => Ok(()),
        other => Err(format!("prime {other} is not a prime this family measures")),
    };
    match *case {
        Case::SparseMatmat {
            prime,
            rows,
            nonzeros_per_row,
            dense_cols,
            seed,
        } => {
            lane(prime)?;
            Ok(if prime == SMALL_PRIME {
                sparse_matmat::<SMALL_PRIME>(rows, nonzeros_per_row, dense_cols, seed, form)
            } else {
                sparse_matmat::<MEDIUM_PRIME>(rows, nonzeros_per_row, dense_cols, seed, form)
            })
        }
        Case::Ple { prime, n, seed } => {
            lane(prime)?;
            Ok(if prime == SMALL_PRIME {
                ple::<SMALL_PRIME>(n, seed, form)
            } else {
                ple::<MEDIUM_PRIME>(n, seed, form)
            })
        }
        Case::Gemm { prime, n, seed } => {
            lane(prime)?;
            Ok(if prime == SMALL_PRIME {
                prime_gemm::<SMALL_PRIME>(n, seed, form)
            } else {
                prime_gemm::<MEDIUM_PRIME>(n, seed, form)
            })
        }
        Case::PermanentF3 { n, seed } => permanent_f3(n, seed, form),
    }
}
