//! Workloads of the sparse product check-cost family (jit:bfc2ceab).
//!
//! Both forms compute one sparse-times-dense product over `Fp<P>` from the
//! same operands. `checked` is the shipped consumer
//! `SparseFieldMatrix::matmat`, whose kernel entry checks the column indices
//! once per product. `unchecked` is the dispatcher without that check: the
//! row loop over the kernel crate's unchecked row kernel, with the lane
//! conversions the dispatcher makes around it.

use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::sparse_matrix::SparseFieldMatrix;
use gf2_core::field::FieldVec;
use gf2_core::gfp::Fp;
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use tuning_campaign_support::abtest::SplitMix64;

/// Names the form an execution measures.
pub const FORM_VAR: &str = "GF2_SPMM_FORM";

/// The small prime of the byte lane and the medium prime of the u16 lane.
pub const SMALL_PRIME: u64 = 7;
pub const MEDIUM_PRIME: u64 = 65521;

/// The form of the product an arm measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// The shipped consumer, column indices checked once per product.
    Checked,
    /// The row loop over the unchecked row kernel.
    Unchecked,
}

impl Form {
    /// The form [`FORM_VAR`] names.
    pub fn from_environment() -> Result<Self, String> {
        match std::env::var(FORM_VAR).as_deref() {
            Ok("checked") => Ok(Self::Checked),
            Ok("unchecked") => Ok(Self::Unchecked),
            other => Err(format!("{FORM_VAR} names no form: {other:?}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Checked => "checked",
            Self::Unchecked => "unchecked",
        }
    }
}

/// One cell's case: a `rows × rows` sparse matrix with `nonzeros_per_row`
/// entries per row times a `rows × dense_cols` matrix.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "consumer", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Case {
    SparseMatmat {
        prime: u64,
        rows: usize,
        nonzeros_per_row: usize,
        dense_cols: usize,
        seed: u64,
    },
}

/// A prepared product and the route it reports after running.
pub struct Workload {
    /// One product.
    pub body: Box<dyn FnMut()>,
    /// The product's cells, row-major.
    pub cells: Box<dyn Fn() -> Vec<u64>>,
    /// The route provenance, read after the measured calls.
    pub observe: Box<dyn FnOnce() -> String>,
}

fn element<const P: u64>(rng: &mut SplitMix64) -> Fp<P> {
    Fp::<P>::new(rng.next_u64() % P)
}

fn nonzero<const P: u64>(rng: &mut SplitMix64) -> Fp<P> {
    Fp::<P>::new(1 + rng.next_u64() % (P - 1))
}

/// The lane cell of a prime field element.
trait Lane: Copy + Default + Into<u64> {
    fn of(value: u64) -> Self;
}

impl Lane for u8 {
    fn of(value: u64) -> Self {
        value as u8
    }
}

impl Lane for u16 {
    fn of(value: u64) -> Self {
        value as u16
    }
}

/// The dispatcher's row loop over `row_kernel`, which receives one sparse row
/// and writes one output row of `n` lane cells.
fn row_loop<const P: u64, T: Lane>(
    a: &SparseFieldMatrix<Fp<P>>,
    b: &[Fp<P>],
    n: usize,
    mut row_kernel: impl FnMut(&[T], &[usize], &[T], &mut [T]),
) -> FieldVec<Fp<P>> {
    let (row_ptr, col_idx, values) = a.as_raw_parts();
    let rows = row_ptr.len() - 1;
    let mut out = vec![Fp::<P>::new(0); rows * n];
    let b_lane: Vec<T> = b.iter().map(|x| T::of(x.value())).collect();
    let a_vals: Vec<T> = values.iter().map(|x| T::of(x.value())).collect();
    let mut out_lane = vec![T::default(); n];
    for r in 0..rows {
        let (start, end) = (row_ptr[r], row_ptr[r + 1]);
        out_lane.fill(T::default());
        if start != end {
            row_kernel(
                &a_vals[start..end],
                &col_idx[start..end],
                &b_lane,
                &mut out_lane,
            );
        }
        for (slot, &cell) in out[r * n..(r + 1) * n].iter_mut().zip(out_lane.iter()) {
            *slot = Fp::<P>::new(cell.into());
        }
    }
    FieldVec::from(out)
}

/// The product through the unchecked row kernel of the lane of `P`.
fn unchecked_product<const P: u64>(
    a: &SparseFieldMatrix<Fp<P>>,
    b: &[Fp<P>],
    n: usize,
) -> Result<FieldVec<Fp<P>>, String> {
    assert_eq!(a.cols() * n, b.len(), "b is not a.cols() rows of n cells");
    // SAFETY (both lanes): every constructor of `SparseFieldMatrix` stores
    // column indices below `a.cols()`, and the assertion above gives `b` that
    // many rows of `n` cells, the stride the calls pass.
    if P == SMALL_PRIME {
        let fns = gf2_kernels_simd::fp_small::detect().ok_or("the host lacks AVX2")?;
        Ok(row_loop::<P, u8>(a, b, n, |vals, cols, b, out| unsafe {
            (fns.spmm_row_unchecked_fn)(vals, cols, b, n, n, P as u8, out)
        }))
    } else {
        let fns = gf2_kernels_simd::fp_medium::detect().ok_or("the host lacks AVX2")?;
        Ok(row_loop::<P, u16>(a, b, n, |vals, cols, b, out| unsafe {
            (fns.spmm_row_unchecked_fn)(vals, cols, b, n, n, P as u16, out)
        }))
    }
}

struct Operands<const P: u64> {
    a: SparseFieldMatrix<Fp<P>>,
    b: FieldMatrix<Fp<P>>,
    /// The cells of `b`, row-major.
    b_cells: Vec<Fp<P>>,
}

impl<const P: u64> Operands<P> {
    fn checked(&self) -> FieldMatrix<Fp<P>> {
        black_box(&self.a).matmat(black_box(&self.b))
    }

    fn unchecked(&self) -> FieldVec<Fp<P>> {
        unchecked_product(black_box(&self.a), black_box(&self.b_cells), self.b.cols())
            .expect("the kernel detected at preparation")
    }
}

fn sparse_matmat<const P: u64>(
    rows: usize,
    nonzeros_per_row: usize,
    dense_cols: usize,
    seed: u64,
    form: Form,
) -> Result<Workload, String> {
    let mut rng = SplitMix64::new(seed);
    let triplets = (0..rows).flat_map(|row| {
        (0..nonzeros_per_row)
            .map(|_| (row, rng.next_u64() as usize % rows, nonzero::<P>(&mut rng)))
            .collect::<Vec<_>>()
    });
    let a = SparseFieldMatrix::from_triplets(rows, rows, triplets.collect::<Vec<_>>());
    let mut rng = SplitMix64::new(seed ^ 0x5eed);
    let b_rows: Vec<Vec<Fp<P>>> = (0..rows)
        .map(|_| (0..dense_cols).map(|_| element::<P>(&mut rng)).collect())
        .collect();
    let b_cells: Vec<Fp<P>> = b_rows.concat();
    let b = FieldMatrix::from_rows(b_rows.into_iter().map(FieldVec::from).collect());
    let stored = a.nnz();
    // The unchecked form needs its kernel before the first call.
    unchecked_product(&a, &b_cells, dense_cols)?;
    let operands = std::rc::Rc::new(Operands { a, b, b_cells });
    let (timed, read) = (operands.clone(), operands);
    let body: Box<dyn FnMut()> = match form {
        Form::Checked => Box::new(move || {
            black_box(timed.checked());
        }),
        Form::Unchecked => Box::new(move || {
            black_box(timed.unchecked());
        }),
    };
    let cells: Box<dyn Fn() -> Vec<u64>> = match form {
        Form::Checked => Box::new(move || {
            let product = read.checked();
            (0..rows)
                .flat_map(|r| (0..dense_cols).map(move |c| (r, c)))
                .map(|(r, c)| product.get(r, c).value())
                .collect()
        }),
        Form::Unchecked => Box::new(move || {
            read.unchecked()
                .as_slice()
                .iter()
                .map(|x| x.value())
                .collect()
        }),
    };
    Ok(Workload {
        body,
        cells,
        observe: Box::new(move || {
            format!(
                "sparse product Fp<{P}>/rows={rows}/nnz={stored}/dense-cols={dense_cols}\
                 /form={}",
                form.name()
            )
        }),
    })
}

/// Prepares the product a case declares in `form`.
pub fn prepare(case: &Case, form: Form) -> Result<Workload, String> {
    let Case::SparseMatmat {
        prime,
        rows,
        nonzeros_per_row,
        dense_cols,
        seed,
    } = *case;
    match prime {
        SMALL_PRIME => sparse_matmat::<SMALL_PRIME>(rows, nonzeros_per_row, dense_cols, seed, form),
        MEDIUM_PRIME => {
            sparse_matmat::<MEDIUM_PRIME>(rows, nonzeros_per_row, dense_cols, seed, form)
        }
        other => Err(format!("prime {other} is not a prime this family measures")),
    }
}
