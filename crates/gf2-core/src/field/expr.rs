//! Expression-template proxy algebra over [`FieldMatrix<F>`].
//!
//! This module implements the expression-template layer designed in
//! `dev/plans/expression_templates_design.md` (story `cdcebf6a`). It lets
//! users write matrix algebra in idiomatic Rust and have the compiler infer
//! a proxy tree that evaluates to exactly **one** kernel call per canonical
//! fusion on the evaluation boundary:
//!
//! ```rust
//! # use gf2_core::field::matrix::FieldMatrix;
//! # use gf2_core::gfp::Fp;
//! let a = FieldMatrix::<Fp<7>>::identity(4);
//! let b = FieldMatrix::<Fp<7>>::identity(4);
//! let c = FieldMatrix::<Fp<7>>::identity(4);
//! // One `gemm_with_beta(β=1)` kernel call, zero owned intermediates.
//! let r: FieldMatrix<Fp<7>> = (&a * &b + &c).into();
//! assert_eq!(r.get(0, 0), Fp::<7>::new(2));
//! ```
//!
//! # Proxy taxonomy (design §3)
//!
//! | Proxy                                                                  | Represents                     |
//! |------------------------------------------------------------------------|--------------------------------|
//! | [`Product<A, B>`]                                                      | `A · B`                        |
//! | [`Sum<A, B>`]                                                          | `A + B`                        |
//! | [`Scale<F, M>`]                                                        | `α · M`                        |
//! | [`NegProxy<M>`]                                                        | `-M`                           |
//! | [`TransposedProduct<A, B>`]                                            | `Aᵀ · B` — fused               |
//! | [`ScaledTransposedProduct<F, A, B>`]                                   | `α · Aᵀ · B` — fused           |
//! | [`FusedProductPlus<P, C>`]                                             | `A·B + C` (β = 1)              |
//! | [`FusedProductPlusScaled<P, S>`]                                       | `A·B + β·C`                    |
//! | [`FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, C>>`] | `α·Aᵀ·B + β·C` — fused       |
//! | [`FusedLinear<A, B>`]                                                  | `α·A + β·B`                    |
//!
//! The already-in-tree [`Transposed<M>`](crate::field::matrix::Transposed)
//! proxy is extended here with `MatrixLike<F>` + `Evaluate<F>` impls.
//!
//! # Avoiding accidental evaluation (design §9)
//!
//! Binding a subexpression to a typed [`FieldMatrix<F>`] forces evaluation
//! and loses fusion opportunities:
//!
//! ```text
//! // Eager — TWO kernel calls, two allocations:
//! let t: FieldMatrix<F> = &a * &b;      // gemm called here
//! let r: FieldMatrix<F> = t + &c;       // then axpy-linear
//!
//! // Lazy — ONE kernel call, one allocation:
//! let r: FieldMatrix<F> = (&a * &b + &c).into();
//! ```
//!
//! Prefer the `.into()` idiom or [`FieldMatrix::eval`], and let Rust infer
//! the proxy type at intermediate steps. Every proxy type also carries
//! `#[must_use]` as defence-in-depth.
//!
//! # Trace counters (design §7)
//!
//! Every call to a kernel primitive in this module increments an atomic
//! counter. Tests may inspect these counters via [`kernel_counts`] to verify
//! that a fused expression collapses to exactly one kernel call. The
//! counters are compiled unconditionally so that `cargo nextest run
//! --release` (the CI default) can still query them; the atomic cost
//! happens at the kernel-entry boundary, not in inner loops, so there is no
//! measurable performance effect.
//!
//! # Why `FieldMatrix<F>` does not implement `Evaluate<F>`
//!
//! The `From<E> for FieldMatrix<F>` bridge at the bottom of this module is
//! a blanket `impl<F: ConstField, E: Evaluate<F>> From<E> for FieldMatrix<F>`.
//! If bare `FieldMatrix<F>` (or `&FieldMatrix<F>`) also implemented
//! `Evaluate<F>`, that blanket would overlap the reflexive
//! `impl<T> From<T> for T` from `core` — Rust rejects this with **E0119**
//! "conflicting implementations of trait".
//!
//! The resolution adopted by `d48a3cfd/T2` is to **keep the `From` blanket**
//! (which is load-bearing for the `(&a * &b + &c).into()` idiom) and **drop
//! the `Evaluate<F>` impls on `FieldMatrix<F>` and `&FieldMatrix<F>`**.
//! Proxies whose operand is `&FieldMatrix<F>` (e.g.
//! [`Transposed<&FieldMatrix<F>>`](crate::field::matrix::Transposed),
//! [`Scale<F, &FieldMatrix<F>>`], [`NegProxy<&FieldMatrix<F>>`]) invoke
//! kernels directly without round-tripping through an
//! `Evaluate` impl on a bare matrix — this matches design §6.3 ("proxies
//! call kernels directly").
//!
//! User-facing consequence: `FieldMatrix::from(&a)` no longer bridges through
//! `Evaluate<F>`. Write `a.clone()` for an owned copy, or
//! `(F::one() * &a).into()` / `FieldMatrix::eval(F::one() * &a)` for the
//! lazy-friendly route through [`Scale`] → [`Evaluate`].
//!
//! This note supersedes the "bare matrix is an `Evaluate<F>`" claim in
//! `dev/plans/expression_templates_design.md` §6.5 (amended at
//! `d48a3cfd/T2`).

use std::ops::{Add, Mul, Neg, Sub};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::field::matrix::{FieldMatrix, Transposed};
use crate::field::vec::dot_product_slices;
use crate::field::{ConstField, FieldVec, FiniteField};
use crate::matrix_like::MatrixLike;

// ─── Trace counters (design §7) ─────────────────────────────────────────────

/// Aggregate snapshot of kernel-call counts across all evaluator paths.
///
/// See [`kernel_counts`] and [`reset_kernel_counts`] for the runtime API.
/// Every canonical fusion listed in §8 of `expression_templates_design.md`
/// must collapse to exactly one `kernel_*` counter increment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KernelCounts {
    /// Calls to the plain `A·B` kernel.
    pub gemm: u64,
    /// Calls to the fused `A·B + β·C` kernel.
    pub gemm_with_beta: u64,
    /// Calls to the fused `Aᵀ·B` kernel.
    pub gemm_trans_a: u64,
    /// Calls to the fused `α·Aᵀ·B + β·C` kernel.
    pub gemm_trans_a_with_beta: u64,
    /// Calls to the fused `α·A + β·B` axpy-linear kernel.
    pub axpy_linear: u64,
    /// Calls to the scalar-scale kernel.
    pub scale_into: u64,
    /// Calls to the element-wise negation kernel.
    pub neg_into: u64,
    /// Calls to the plain copy kernel (includes materialising [`Transposed`]).
    pub copy_into: u64,
}

static KC_GEMM: AtomicU64 = AtomicU64::new(0);
static KC_GEMM_BETA: AtomicU64 = AtomicU64::new(0);
static KC_GEMM_TA: AtomicU64 = AtomicU64::new(0);
static KC_GEMM_TA_BETA: AtomicU64 = AtomicU64::new(0);
static KC_AXPY: AtomicU64 = AtomicU64::new(0);
static KC_SCALE: AtomicU64 = AtomicU64::new(0);
static KC_NEG: AtomicU64 = AtomicU64::new(0);
static KC_COPY: AtomicU64 = AtomicU64::new(0);

#[inline(always)]
fn bump(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

/// Returns a snapshot of the cumulative kernel-call counters.
///
/// Counters are process-wide and are not reset automatically between test
/// runs; see [`reset_kernel_counts`] for the single-threaded reset.
///
/// **Concurrency caveat.** Any test that asserts on the before/after delta
/// *must* be serialised against every other test that bumps these counters
/// — the counters are global, so a concurrent test's kernel call is
/// indistinguishable from a bump the test under observation caused
/// itself. The in-crate trace tests use `#[serial_test::serial]` on every
/// `test_fusion_*` cluster to enforce this; new counter-based tests must
/// do the same (see `expr.rs` test module).
///
/// # Examples
///
/// ```
/// use gf2_core::field::expr::{kernel_counts, reset_kernel_counts};
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// reset_kernel_counts();
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let c = FieldMatrix::<Fp<7>>::identity(3);
/// let before = kernel_counts();
/// let _r: FieldMatrix<Fp<7>> = (&a * &b + &c).into();
/// let after = kernel_counts();
/// // Exactly one fused gemm_with_beta, no plain gemm, no axpy.
/// assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
/// assert_eq!(after.gemm - before.gemm, 0);
/// assert_eq!(after.axpy_linear - before.axpy_linear, 0);
/// ```
///
/// # Panics
///
/// Never panics; only reads atomics.
///
/// # Complexity
///
/// O(1).
pub fn kernel_counts() -> KernelCounts {
    KernelCounts {
        gemm: KC_GEMM.load(Ordering::Relaxed),
        gemm_with_beta: KC_GEMM_BETA.load(Ordering::Relaxed),
        gemm_trans_a: KC_GEMM_TA.load(Ordering::Relaxed),
        gemm_trans_a_with_beta: KC_GEMM_TA_BETA.load(Ordering::Relaxed),
        axpy_linear: KC_AXPY.load(Ordering::Relaxed),
        scale_into: KC_SCALE.load(Ordering::Relaxed),
        neg_into: KC_NEG.load(Ordering::Relaxed),
        copy_into: KC_COPY.load(Ordering::Relaxed),
    }
}

/// Resets every kernel-call counter to zero.
///
/// Intended for single-threaded tests that want to anchor their assertions
/// at zero; in multi-threaded environments prefer the delta pattern with
/// two [`kernel_counts`] snapshots.
///
/// # Panics
///
/// Never panics.
///
/// # Complexity
///
/// O(1).
pub fn reset_kernel_counts() {
    KC_GEMM.store(0, Ordering::Relaxed);
    KC_GEMM_BETA.store(0, Ordering::Relaxed);
    KC_GEMM_TA.store(0, Ordering::Relaxed);
    KC_GEMM_TA_BETA.store(0, Ordering::Relaxed);
    KC_AXPY.store(0, Ordering::Relaxed);
    KC_SCALE.store(0, Ordering::Relaxed);
    KC_NEG.store(0, Ordering::Relaxed);
    KC_COPY.store(0, Ordering::Relaxed);
}

// ─── Evaluate<F> trait (design §6) ──────────────────────────────────────────

/// Consumer side of the expression-template algebra.
///
/// Every lazy proxy in this module — [`Product`], [`Sum`], [`Scale`],
/// [`NegProxy`], [`Transposed<&FieldMatrix<F>>`](crate::field::matrix::Transposed),
/// [`FusedProductPlus`], [`FusedProductPlusScaled`], [`FusedLinear`],
/// [`TransposedProduct`] — implements `Evaluate<F>`. Bare
/// `&FieldMatrix<F>` and owned `FieldMatrix<F>` **do not**; see the
/// module-header rationale "Why `FieldMatrix<F>` does not implement
/// `Evaluate<F>`" for the Rust E0119 reason. Users get an owned matrix
/// from a bare input via `a.clone()` or `(F::one() * &a).into()`.
///
/// The [`From<E> for FieldMatrix<F>`](FieldMatrix) blanket (sealed via
/// `sealed::ProxyExpr` to avoid E0119 against a hypothetical downstream
/// `Evaluate<F>` impl on `FieldMatrix<F>`) allocates a fresh output of
/// the correct shape and calls [`evaluate_into`](Self::evaluate_into).
///
/// # Overwrite semantics
///
/// `evaluate_into` **overwrites** `out`; the caller must not rely on
/// `out`'s previous contents surviving the call. See §6.3.
///
/// # Shape check
///
/// Implementors must assert `out.shape() == self.shape()` (design §7.2).
pub trait Evaluate<F: FiniteField> {
    /// Consumes the expression and writes its value into `out`.
    ///
    /// # Panics
    ///
    /// Panics if `out.shape() != self.shape()`.
    fn evaluate_into(self, out: &mut FieldMatrix<F>);

    /// Logical shape of the expression, in rows × cols.
    fn shape(&self) -> (usize, usize);
}

// ─── Kernel primitives (module-private) ─────────────────────────────────────

/// Kernel `out <- A`. Overwrites.
fn copy_into<F: FiniteField, LA: MatrixLike<F>>(a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_COPY);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "copy_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, a.get(r, c));
        }
    }
}

/// Kernel `out <- -A`. Overwrites.
fn neg_into<F: FiniteField, LA: MatrixLike<F>>(a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_NEG);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "neg_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, -a.get(r, c));
        }
    }
}

/// Kernel `out <- α · A`. Overwrites.
fn scale_into<F: FiniteField, LA: MatrixLike<F>>(alpha: F, a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_SCALE);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "scale_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, alpha.clone() * a.get(r, c));
        }
    }
}

/// Kernel `out <- α · A + β · B`. Overwrites. Single-pass — the caller's
/// point of using this over two separate scales + sum is that only one
/// dispatch + one allocation is paid.
fn axpy_linear<F, LA, LB>(alpha: F, a: &LA, beta: F, b: &LB, out: &mut FieldMatrix<F>)
where
    F: FiniteField,
    LA: MatrixLike<F>,
    LB: MatrixLike<F>,
{
    bump(&KC_AXPY);
    assert_eq!(
        a.shape(),
        b.shape(),
        "axpy_linear: operand shape mismatch ({}×{} vs {}×{})",
        a.rows(),
        a.cols(),
        b.rows(),
        b.cols()
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "axpy_linear: output shape mismatch"
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            let v = alpha.clone() * a.get(r, c) + beta.clone() * b.get(r, c);
            out.set(r, c, v);
        }
    }
}

/// Kernel `out <- A · B` over generic `MatrixLike` operands. The concrete-
/// operand fast path routes through the T1 blocked gemm — see
/// [`gemm_concrete`].
fn gemm_matrixlike<F, LA, LB>(a: &LA, b: &LB, out: &mut FieldMatrix<F>)
where
    F: FiniteField,
    LA: MatrixLike<F>,
    LB: MatrixLike<F>,
{
    bump(&KC_GEMM);
    let (m, k1) = a.shape();
    let (k2, n) = b.shape();
    assert_eq!(
        k1, k2,
        "gemm_matrixlike: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_matrixlike: output shape mismatch"
    );
    if m == 0 || n == 0 || k1 == 0 {
        return;
    }
    let zero = a.get(0, 0).zero_like();
    for i in 0..m {
        for j in 0..n {
            let mut acc = zero.zero_like();
            for t in 0..k1 {
                acc += a.get(i, t) * b.get(t, j);
            }
            out.set(i, j, acc);
        }
    }
}

/// Concrete fast path: `out <- A · B` via the T1 blocked gemm. One
/// allocation (the transposed `B`) inside T1's `gemm`, then a single
/// row-by-row move into `out`. Counter is bumped once.
fn gemm_concrete<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>, out: &mut FieldMatrix<F>) {
    bump(&KC_GEMM);
    let (m, k1) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_concrete: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_concrete: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    // Delegate to T1's blocked gemm. This materialises a fresh FieldMatrix,
    // but the subsequent row move into `out` is O(m·n) clones — dominated
    // by the O(m·k·n) multiplies.
    let prod = crate::field::matrix::gemm(a, b);
    for r in 0..m {
        for c in 0..n {
            out.set(r, c, <FieldMatrix<F> as MatrixLike<F>>::get(&prod, r, c));
        }
    }
}

/// Kernel `out <- A · B + β · C`. Overwrites. See design §5.1 / §5.2.
///
/// The kernel walks the inner dimension once per output cell using the
/// same `dot_product_slices` chunked delayed-reduction primitive as T1's
/// blocked gemm; the β·C term is added after the dot-product reduction.
fn gemm_with_beta_concrete<F: FiniteField, LC: MatrixLike<F>>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_BETA);
    let (m, k) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (bk, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k, bk,
        "gemm_with_beta: inner dimensions must match ({} vs {})",
        k, bk
    );
    assert_eq!(
        c.shape(),
        (m, n),
        "gemm_with_beta: C shape must equal A·B shape ({}×{} vs {}×{})",
        c.rows(),
        c.cols(),
        m,
        n,
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_with_beta: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        // Zero inner dim: A·B is the zero matrix, so out <- β·C.
        for r in 0..m {
            for col in 0..n {
                out.set(r, col, beta.clone() * c.get(r, col));
            }
        }
        return;
    }
    // Source a zero witness from whichever factor has storage. Neither is
    // empty here (m, k, n > 0).
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let b_t = <FieldMatrix<F> as MatrixLike<F>>::transpose(b);
    for i in 0..m {
        let a_row = &a.as_data_slice()[i * k..(i + 1) * k];
        for j in 0..n {
            let b_col = &b_t.as_data_slice()[j * k..(j + 1) * k];
            let prod = dot_product_slices(a_row, b_col, &zero);
            out.set(i, j, prod + beta.clone() * c.get(i, j));
        }
    }
}

/// Kernel `out <- Aᵀ · B`. Overwrites. See design §5.4.
fn gemm_trans_a_concrete<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_TA);
    let (k1, m) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_trans_a: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_trans_a: output shape mismatch"
    );
    if m == 0 || n == 0 || k1 == 0 {
        return;
    }
    // Transpose A once (k×m → m×k) then the inner loop is a row·row dot
    // product — the same shape as the plain gemm kernel, reusing the same
    // delayed-reduction primitive.
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let a_t = <FieldMatrix<F> as MatrixLike<F>>::transpose(a);
    let b_t = <FieldMatrix<F> as MatrixLike<F>>::transpose(b);
    for i in 0..m {
        let a_row = &a_t.as_data_slice()[i * k1..(i + 1) * k1];
        for j in 0..n {
            let b_col = &b_t.as_data_slice()[j * k1..(j + 1) * k1];
            out.set(i, j, dot_product_slices(a_row, b_col, &zero));
        }
    }
}

/// Kernel `out <- α · Aᵀ · B + β · C`. Overwrites. Used by the §5.4
/// compositional extension and for `(α · a.t()) · &b + β · &c` patterns.
///
/// Both `alpha` and `beta` are explicit runtime scalars; callers that need
/// the β = 1 case pass a one-witness via `F::one_like()` / the usual
/// `get(0, 0).one_like()` pattern.
fn gemm_trans_a_with_beta_concrete<F: FiniteField, LC: MatrixLike<F>>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_TA_BETA);
    let (k1, m) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_trans_a_with_beta: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        c.shape(),
        (m, n),
        "gemm_trans_a_with_beta: C shape must equal Aᵀ·B shape"
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_trans_a_with_beta: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    if k1 == 0 {
        for r in 0..m {
            for col in 0..n {
                out.set(r, col, beta.clone() * c.get(r, col));
            }
        }
        return;
    }
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let a_t = <FieldMatrix<F> as MatrixLike<F>>::transpose(a);
    let b_t = <FieldMatrix<F> as MatrixLike<F>>::transpose(b);
    for i in 0..m {
        let a_row = &a_t.as_data_slice()[i * k1..(i + 1) * k1];
        for j in 0..n {
            let b_col = &b_t.as_data_slice()[j * k1..(j + 1) * k1];
            let prod = dot_product_slices(a_row, b_col, &zero);
            out.set(i, j, alpha.clone() * prod + beta.clone() * c.get(i, j));
        }
    }
}

// ─── Proxy types (design §3) ────────────────────────────────────────────────

/// Deferred matrix multiplication `A · B`.
///
/// *Lazy expression type.* Binding the result to a typed
/// [`FieldMatrix<F>`] forces eager evaluation and loses fusion
/// opportunities; stay in proxy form until the final `.into()`.
///
/// # Arguments
///
/// * `0` - Left operand.
/// * `1` - Right operand.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let r: FieldMatrix<Fp<7>> = (&a * &b).into();
/// assert_eq!(r, FieldMatrix::<Fp<7>>::identity(3));
/// ```
///
/// # Panics
///
/// The `Mul` impl panics at construction if `a.cols() != b.rows()`.
/// `evaluate_into` panics if `out.shape() != (a.rows(), b.cols())`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "Product is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Product<A, B>(pub A, pub B);

/// Deferred element-wise addition `A + B`.
///
/// *Lazy expression type.* Binding to a typed [`FieldMatrix<F>`] forces
/// evaluation.
///
/// # Arguments
///
/// * `0` - Left operand.
/// * `1` - Right operand.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(2);
/// let b = FieldMatrix::<Fp<7>>::identity(2);
/// let r: FieldMatrix<Fp<7>> = (&a + &b).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(2));
/// ```
///
/// # Panics
///
/// Construction panics if `a.shape() != b.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "Sum is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Sum<A, B>(pub A, pub B);

/// Deferred scalar-times-matrix `α · M`.
///
/// *Lazy expression type.* Built by `alpha * &a`, `&a * alpha`, or by
/// combining with a [`Product`].
///
/// # Arguments
///
/// * `0` - Scalar.
/// * `1` - Matrix-like operand.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(2);
/// let r: FieldMatrix<Fp<7>> = (&a * Fp::<7>::new(3)).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(3));
/// ```
///
/// # Panics
///
/// Evaluation asserts `out.shape() == self.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "Scale is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Scale<F: FiniteField, M>(pub F, pub M);

/// Deferred element-wise negation `-M`.
///
/// Named `NegProxy` rather than `Neg` to avoid colliding with
/// [`std::ops::Neg`]. Built by `-&a` or `-a`.
///
/// # Arguments
///
/// * `0` - The matrix being negated.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(2);
/// let r: FieldMatrix<Fp<7>> = (-&a).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(6));
/// ```
///
/// # Panics
///
/// Evaluation asserts `out.shape() == self.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "NegProxy is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct NegProxy<M>(pub M);

/// Canonical fusion: `A · B + C` (β = 1). See design §5.1.
///
/// Built by `Product<A, B> + &c` (and the commuted form). Evaluates in a
/// single `gemm_with_beta` kernel call.
///
/// # Arguments
///
/// * `0` - A [`Product`] subexpression.
/// * `1` - The addend `C`.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::field::expr::{kernel_counts, reset_kernel_counts};
/// use gf2_core::gfp::Fp;
///
/// reset_kernel_counts();
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let c = FieldMatrix::<Fp<7>>::identity(3);
/// let before = kernel_counts();
/// let _r: FieldMatrix<Fp<7>> = (&a * &b + &c).into();
/// let after = kernel_counts();
/// assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
/// ```
///
/// # Panics
///
/// Construction panics if the product shape differs from the addend shape.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "FusedProductPlus is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedProductPlus<P, C>(pub P, pub C);

/// Canonical fusion: `A · B + β · C`. See design §5.2.
///
/// Built by `Product<A, B> + Scale<F, &c>` (and commutations).
///
/// # Arguments
///
/// * `0` - A [`Product`] subexpression.
/// * `1` - A [`Scale`] wrapping the addend.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(2);
/// let b = FieldMatrix::<Fp<7>>::identity(2);
/// let c = FieldMatrix::<Fp<7>>::identity(2);
/// let beta = Fp::<7>::new(3);
/// let r: FieldMatrix<Fp<7>> = (&a * &b + &c * beta).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(4));
/// ```
///
/// # Panics
///
/// Construction panics if the product shape differs from the addend shape.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "FusedProductPlusScaled is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedProductPlusScaled<P, SS>(pub P, pub SS);

/// Canonical fusion: `α · A + β · B`. See design §5.3.
///
/// Built by `Scale<F, A> + Scale<F, B>`. The degenerate `Scale + &M` form
/// is wrapped to `β = 1` by the operator overload.
///
/// # Arguments
///
/// * `0` - A [`Scale`] subexpression.
/// * `1` - A [`Scale`] subexpression.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let r: FieldMatrix<Fp<7>> =
///     (Fp::<7>::new(2) * &a + Fp::<7>::new(3) * &b).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(5));
/// ```
///
/// # Panics
///
/// Construction panics on shape mismatch.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "FusedLinear is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedLinear<A, B>(pub A, pub B);

/// Canonical fusion: `Aᵀ · B`. See design §5.4.
///
/// Built by `a.t() * &b`. Evaluates in a single `gemm_trans_a` kernel call.
///
/// # Arguments
///
/// * `0` - The un-transposed left operand (the evaluator transposes it).
/// * `1` - The right operand.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let r: FieldMatrix<Fp<7>> = (a.t() * &b).into();
/// assert_eq!(r, FieldMatrix::<Fp<7>>::identity(3));
/// ```
///
/// # Panics
///
/// Construction panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "TransposedProduct is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct TransposedProduct<A, B>(pub A, pub B);

/// Canonical fusion: `α · Aᵀ · B`. See design §5.4 (compositional).
///
/// Built by `alpha * a.t() * &b` or `(alpha * a.t()) * &b`. Exists as a
/// distinct proxy (rather than `Scale<F, TransposedProduct<A, B>>`) so the
/// `Scale<F, X> + Scale<F, Y> → FusedLinear` add impl does not spuriously
/// match here — `FusedLinear` requires both sides to be `MatrixLike`, and
/// we deliberately contract `Aᵀ·B` only at `evaluate_into` time.
///
/// # Arguments
///
/// * `0` - The scalar `α`.
/// * `1` - The un-transposed left operand.
/// * `2` - The right operand.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let alpha = Fp::<7>::new(2);
/// let r: FieldMatrix<Fp<7>> = ((alpha * a.t()) * &b).into();
/// assert_eq!(r.get(0, 0), Fp::<7>::new(2));
/// ```
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "ScaledTransposedProduct is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct ScaledTransposedProduct<F: FiniteField, A, B>(pub F, pub A, pub B);

// ─── Proxy constructors (design §7.1 — shape checks) ───────────────────────

impl<A, B> Product<A, B> {
    /// Constructs the proxy after checking that inner dimensions match.
    ///
    /// # Arguments
    ///
    /// * `a` - Left operand.
    /// * `b` - Right operand.
    ///
    /// # Panics
    ///
    /// Panics if `a.cols() != b.rows()` with the standard
    /// `FieldMatrix::mul: inner dimensions must match` message.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::expr::Product;
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// let b = FieldMatrix::<Fp<7>>::zeros(3, 4);
    /// let _p = Product::new::<Fp<7>>(&a, &b);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.cols(),
            b.rows(),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            a.cols(),
            b.rows()
        );
        Product(a, b)
    }
}

impl<A, B> Sum<A, B> {
    /// Constructs the proxy after checking that shapes match.
    ///
    /// # Arguments
    ///
    /// * `a` - Left operand.
    /// * `b` - Right operand.
    ///
    /// # Panics
    ///
    /// Panics if `a.shape() != b.shape()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::expr::Sum;
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// let b = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// let _s = Sum::new::<Fp<7>>(&a, &b);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.shape(),
            b.shape(),
            "FieldMatrix::add: shape mismatch ({}×{} vs {}×{})",
            a.rows(),
            a.cols(),
            b.rows(),
            b.cols()
        );
        Sum(a, b)
    }
}

impl<A, B> TransposedProduct<A, B> {
    /// Constructs the `Aᵀ·B` proxy after checking that inner dims match.
    ///
    /// # Arguments
    ///
    /// * `a` - Left operand (un-transposed; evaluator handles transpose).
    /// * `b` - Right operand.
    ///
    /// # Panics
    ///
    /// Panics if `a.rows() != b.rows()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::expr::TransposedProduct;
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldMatrix::<Fp<7>>::zeros(3, 2);
    /// let b = FieldMatrix::<Fp<7>>::zeros(3, 4);
    /// let _tp = TransposedProduct::new::<Fp<7>>(&a, &b);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.rows(),
            b.rows(),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            a.rows(),
            b.rows()
        );
        TransposedProduct(a, b)
    }
}

// ─── MatrixLike impls for proxies (design §8) ───────────────────────────────
//
// Only the "Yes"-column proxies in §8.1 implement MatrixLike<F>. Products
// and their fused forms have O(k) `get` cost and intentionally stay outside
// the trait.

impl<F, A, B> MatrixLike<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0.get(r, c) + self.1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F, M> MatrixLike<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.1.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.1.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0.clone() * self.1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F, M> MatrixLike<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        -self.0.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F: FiniteField> MatrixLike<F> for Transposed<&FieldMatrix<F>> {
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        <FieldMatrix<F> as MatrixLike<F>>::cols(self.0)
    }
    #[inline]
    fn cols(&self) -> usize {
        <FieldMatrix<F> as MatrixLike<F>>::rows(self.0)
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        <FieldMatrix<F> as MatrixLike<F>>::get(self.0, c, r)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        self.0.clone()
    }
}

impl<F, A, B> MatrixLike<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0 .0.clone() * self.0 .1.get(r, c) + self.1 .0.clone() * self.1 .1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

/// Private helper: materialise a `MatrixLike<F>` proxy as an owned
/// [`FieldMatrix<F>`], used by proxy `MatrixLike::transpose` impls. Sources
/// a zero witness from the expression's own `(0, 0)` cell when the shape is
/// nonempty; for empty shapes it falls back to `F::zero_hint()`, panicking
/// only in the degenerate runtime-context-with-empty-shape case (which
/// matches the gemm/matvec behaviour T1 documented for runtime-context
/// fields — see `dev/active/ab791e27-design.md`).
fn materialise<F, E>(expr: &E) -> FieldMatrix<F>
where
    F: FiniteField,
    E: MatrixLike<F>,
{
    let (rows, cols) = expr.shape();
    if rows == 0 || cols == 0 {
        // Empty shape: try a zero hint (ConstField path) and fall back to
        // an owned empty matrix. Storage is empty regardless.
        let zero = match F::zero_hint() {
            Some(z) => z,
            None => {
                // Runtime-context field with empty shape: construct via
                // `FieldMatrix::from_raw_parts` with an empty FieldVec. This
                // is identical to what T1's `gemm` returns for m×0 · 0×n
                // pairs.
                return FieldMatrix::from_raw_parts(rows, cols, FieldVec::<F>::new());
            }
        };
        return FieldMatrix::from_raw_parts(rows, cols, FieldVec::zeros_from(0, &zero));
    }
    // Source a zero witness from the proxy itself.
    let zero = expr.get(0, 0).zero_like();
    let mut data = FieldVec::<F>::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            data.push(expr.get(r, c));
        }
    }
    let _ = zero; // `data` carries its own element witnesses; zero was for allocation branching only.
    FieldMatrix::from_raw_parts(rows, cols, data)
}

// ─── Evaluate<F> impls (design §5, §6) ──────────────────────────────────────
//
// NOTE (d48a3cfd/T2): neither `FieldMatrix<F>` nor `&FieldMatrix<F>` implements
// `Evaluate<F>`. The blanket `impl<F, E> From<E> for FieldMatrix<F>` below
// would otherwise overlap the reflexive `impl<T> From<T> for T` from core
// (E0119). See the module-header rationale and design §6.5 amendment.
//
// Users who want an owned copy write `a.clone()`; a lazy-friendly route is
// `(F::one() * &a).into()` which goes through `Scale<F, &FieldMatrix<F>>` →
// `Evaluate<F>`.

impl<F: FiniteField> Evaluate<F> for Transposed<&FieldMatrix<F>> {
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let t = <FieldMatrix<F> as MatrixLike<F>>::transpose(self.0);
        copy_into(&t, out);
    }
    fn shape(&self) -> (usize, usize) {
        (
            <FieldMatrix<F> as MatrixLike<F>>::cols(self.0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.0),
        )
    }
}

// Product evaluator — generic over MatrixLike operands with a concrete
// specialisation for the `&FieldMatrix<F>` × `&FieldMatrix<F>` case.

impl<F, A, B> Evaluate<F> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        match (A::as_concrete(&self.0), B::as_concrete(&self.1)) {
            (Some(a), Some(b)) => gemm_concrete(a, b, out),
            _ => gemm_matrixlike(&self.0, &self.1, out),
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0.rows(), self.1.cols())
    }
}

/// Private: lets a `MatrixLike` operand optionally expose itself as a
/// concrete `&FieldMatrix<F>` so the evaluator can route through the T1
/// blocked gemm. Every `MatrixLike<F>` has a default impl returning `None`;
/// `&FieldMatrix<F>` overrides to return `Some(self)`.
#[doc(hidden)]
pub trait ConcreteRef<F: FiniteField>: MatrixLike<F> {
    /// Returns `Some(self)` when `Self` is `&FieldMatrix<F>`, else `None`.
    fn as_concrete(&self) -> Option<&FieldMatrix<F>> {
        None
    }
}

impl<F: FiniteField> ConcreteRef<F> for &FieldMatrix<F> {
    fn as_concrete(&self) -> Option<&FieldMatrix<F>> {
        Some(self)
    }
}

impl<F, A, B> ConcreteRef<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
}

impl<F, M> ConcreteRef<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
}

impl<F, M> ConcreteRef<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
}

impl<F: FiniteField> ConcreteRef<F> for Transposed<&FieldMatrix<F>> {}

impl<F, A, B> ConcreteRef<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
}

impl<F, A, B> Evaluate<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        // Sum is axpy_linear with α = β = 1. We call out the one-like
        // witness via A's first cell (legal because both operands share a
        // shape; if it is empty, the output is empty too).
        assert_eq!(
            self.0.shape(),
            self.1.shape(),
            "Sum::evaluate_into: operand shape mismatch"
        );
        if self.0.rows() == 0 || self.0.cols() == 0 {
            return;
        }
        let one = self.0.get(0, 0).one_like();
        axpy_linear(one.clone(), &self.0, one, &self.1, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0.shape()
    }
}

impl<F, M> Evaluate<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        scale_into(self.0, &self.1, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.1.shape()
    }
}

impl<F, M> Evaluate<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        neg_into(&self.0, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0.shape()
    }
}

impl<F, A, B, C> Evaluate<F> for FusedProductPlus<Product<A, B>, C>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.rows(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlus::evaluate_into: output shape mismatch"
        );
        // Source β = 1 from whichever operand has storage.
        let one = if m > 0 && self.0 .0.cols() > 0 {
            self.0 .0.get(0, 0).one_like()
        } else if self.1.rows() > 0 && self.1.cols() > 0 {
            self.1.get(0, 0).one_like()
        } else {
            // Empty output: nothing to do.
            return;
        };
        let Product(a, b) = self.0;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_with_beta_concrete(ar, br, one, &self.1, out),
            _ => {
                // Generic MatrixLike path: `out <- A·B + C` as one kernel.
                bump(&KC_GEMM_BETA);
                let (mm, kk) = a.shape();
                let (_, nn) = b.shape();
                let zero = if mm > 0 && kk > 0 {
                    a.get(0, 0).zero_like()
                } else if self.1.rows() > 0 && self.1.cols() > 0 {
                    self.1.get(0, 0).zero_like()
                } else {
                    return;
                };
                for i in 0..mm {
                    for j in 0..nn {
                        let mut acc = zero.zero_like();
                        for t in 0..kk {
                            acc += a.get(i, t) * b.get(t, j);
                        }
                        acc += self.1.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.rows(), self.0 .1.cols())
    }
}

impl<F, A, B, C> Evaluate<F> for FusedProductPlusScaled<Product<A, B>, Scale<F, C>>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.rows(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlusScaled::evaluate_into: output shape mismatch"
        );
        let Product(a, b) = self.0;
        let Scale(beta, c) = self.1;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_with_beta_concrete(ar, br, beta, &c, out),
            _ => {
                bump(&KC_GEMM_BETA);
                let (mm, kk) = a.shape();
                let (_, nn) = b.shape();
                if mm == 0 || nn == 0 {
                    return;
                }
                let zero = if kk > 0 {
                    a.get(0, 0).zero_like()
                } else if c.rows() > 0 && c.cols() > 0 {
                    c.get(0, 0).zero_like()
                } else {
                    return;
                };
                for i in 0..mm {
                    for j in 0..nn {
                        let mut acc = zero.zero_like();
                        for t in 0..kk {
                            acc += a.get(i, t) * b.get(t, j);
                        }
                        acc += beta.clone() * c.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.rows(), self.0 .1.cols())
    }
}

impl<F, A, B> Evaluate<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let Scale(alpha, a) = self.0;
        let Scale(beta, b) = self.1;
        axpy_linear(alpha, &a, beta, &b, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0 .1.shape()
    }
}

impl<F, A, B> Evaluate<F> for TransposedProduct<A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0.cols(), self.1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "TransposedProduct::evaluate_into: output shape mismatch"
        );
        match (A::as_concrete(&self.0), B::as_concrete(&self.1)) {
            (Some(ar), Some(br)) => gemm_trans_a_concrete(ar, br, out),
            _ => {
                // Generic MatrixLike: Aᵀ·B element-wise.
                bump(&KC_GEMM_TA);
                if m == 0 || n == 0 {
                    return;
                }
                let k = self.0.rows();
                if k == 0 {
                    return;
                }
                let zero = self.0.get(0, 0).zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += self.0.get(t, i) * self.1.get(t, j);
                        }
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0.cols(), self.1.cols())
    }
}

// Compositional fusion: `Aᵀ·B + C` collapses to `gemm_trans_a_with_beta`.
impl<F, A, B, C> Evaluate<F> for FusedProductPlus<TransposedProduct<A, B>, C>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.cols(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlus<TransposedProduct, C>: output shape mismatch"
        );
        let one = if self.0 .0.rows() > 0 && m > 0 {
            self.0 .0.get(0, 0).one_like()
        } else if self.1.rows() > 0 && self.1.cols() > 0 {
            self.1.get(0, 0).one_like()
        } else {
            return;
        };
        let TransposedProduct(a, b) = self.0;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => {
                gemm_trans_a_with_beta_concrete(one.clone(), ar, br, one, &self.1, out)
            }
            _ => {
                bump(&KC_GEMM_TA_BETA);
                let k = a.rows();
                let zero = one.zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        acc += self.1.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.cols(), self.0 .1.cols())
    }
}

// Evaluator for the dedicated `α·Aᵀ·B` proxy (§5.4 compositional). Falls
// back to `TransposedProduct`'s scalar-less gemm_trans_a + post-scale when
// no addend is present; the full αAᵀ·B + βC fusion lives on the
// `FusedProductPlusScaled<ScaledTransposedProduct, Scale<F, C>>` impl below.
impl<F, A, B> Evaluate<F> for ScaledTransposedProduct<F, A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let ScaledTransposedProduct(alpha, a, b) = self;
        let (m, n) = (a.cols(), b.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "ScaledTransposedProduct::evaluate_into: output shape mismatch"
        );
        if m == 0 || n == 0 {
            return;
        }
        let k = a.rows();
        let zero = if k > 0 {
            a.get(0, 0).zero_like()
        } else {
            // m > 0 and n > 0 but k = 0 — output is α · 0 = 0. We need a
            // zero witness; borrow from b (which shares an element type).
            if b.rows() > 0 && b.cols() > 0 {
                b.get(0, 0).zero_like()
            } else {
                // All operands empty along k — nothing to write.
                return;
            }
        };
        // Reuse the alpha-parametric concrete kernel with a zero-shaped C
        // so we pay exactly one kernel call. We synthesize that C as an
        // on-stack adapter that always returns the zero element and
        // reports the (m, n) shape the kernel expects.
        struct ZeroC<'a, F: FiniteField> {
            m: usize,
            n: usize,
            zero: &'a F,
        }
        impl<'a, F: FiniteField> MatrixLike<F> for ZeroC<'a, F> {
            type Owned = FieldMatrix<F>;
            fn rows(&self) -> usize {
                self.m
            }
            fn cols(&self) -> usize {
                self.n
            }
            fn get(&self, _r: usize, _c: usize) -> F {
                self.zero.clone()
            }
            fn transpose(&self) -> FieldMatrix<F> {
                unimplemented!("ZeroC is a kernel-internal witness adapter")
            }
        }
        let c_witness = ZeroC { m, n, zero: &zero };
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => {
                gemm_trans_a_with_beta_concrete(alpha, ar, br, zero.clone(), &c_witness, out)
            }
            _ => {
                bump(&KC_GEMM_TA_BETA);
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        out.set(i, j, alpha.clone() * acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.1.cols(), self.2.cols())
    }
}

// Compositional fusion: `α·Aᵀ·B + β·C` collapses to `gemm_trans_a_with_beta`.
//
// Built by `(alpha * a.t()) * &b + beta * &c`. The operator chain produces
// `FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, C>>`
// via the Add overload on scaled transposed-product + scaled addend below.
impl<F, A, B, C> Evaluate<F>
    for FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, C>>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let ScaledTransposedProduct(alpha, a, b) = self.0;
        let Scale(beta, c) = self.1;
        let (m, n) = (a.cols(), b.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlusScaled<ScaledTransposedProduct, Scale<C>>: output shape mismatch"
        );
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_trans_a_with_beta_concrete(alpha, ar, br, beta, &c, out),
            _ => {
                bump(&KC_GEMM_TA_BETA);
                if m == 0 || n == 0 {
                    return;
                }
                let k = a.rows();
                if k == 0 {
                    for i in 0..m {
                        for j in 0..n {
                            out.set(i, j, beta.clone() * c.get(i, j));
                        }
                    }
                    return;
                }
                let zero = a.get(0, 0).zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        acc = alpha.clone() * acc + beta.clone() * c.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .1.cols(), self.0 .2.cols())
    }
}

// ─── From<E> for FieldMatrix<F> bridge (design §6.4) ────────────────────────
//
// NOTE (d48a3cfd/T2): even after dropping `Evaluate<F>` for bare
// `FieldMatrix<F>`, Rust still rejects a blanket `impl<F, E> From<E> for
// FieldMatrix<F> where E: Evaluate<F>` with E0119: "downstream crates may
// implement trait `Evaluate<F>` for type `FieldMatrix<F>`". We seal the
// bridge by routing it through a private module marker (`sealed::ProxyExpr`)
// and implementing that marker only for the in-crate proxy types. The
// reflexive `From<T> for T` continues to serve bare `FieldMatrix` → `FieldMatrix`
// without overlap.
mod sealed {
    /// Sealed marker for the `From<E> for FieldMatrix<F>` bridge. See
    /// module-header rationale "Why `FieldMatrix<F>` does not implement
    /// `Evaluate<F>`".
    pub trait ProxyExpr {}
}

impl<A, B> sealed::ProxyExpr for Product<A, B> {}
impl<A, B> sealed::ProxyExpr for Sum<A, B> {}
impl<F: FiniteField, M> sealed::ProxyExpr for Scale<F, M> {}
impl<M> sealed::ProxyExpr for NegProxy<M> {}
impl<F: FiniteField> sealed::ProxyExpr for Transposed<&FieldMatrix<F>> {}
impl<P, C> sealed::ProxyExpr for FusedProductPlus<P, C> {}
impl<P, SS> sealed::ProxyExpr for FusedProductPlusScaled<P, SS> {}
impl<A, B> sealed::ProxyExpr for FusedLinear<A, B> {}
impl<A, B> sealed::ProxyExpr for TransposedProduct<A, B> {}
impl<F: FiniteField, A, B> sealed::ProxyExpr for ScaledTransposedProduct<F, A, B> {}

impl<F, E> From<E> for FieldMatrix<F>
where
    F: ConstField,
    E: Evaluate<F> + sealed::ProxyExpr,
{
    fn from(expr: E) -> Self {
        let (rows, cols) = expr.shape();
        let mut out = FieldMatrix::<F>::zeros(rows, cols);
        expr.evaluate_into(&mut out);
        out
    }
}

// ─── FieldMatrix::eval sugar (design §9.2 item 5 / §12 item B) ──────────────

impl<F: ConstField> FieldMatrix<F> {
    /// Evaluates a proxy expression into a fresh owned matrix.
    ///
    /// This is sugar for `expr.into()`; it reads closer to Armadillo's
    /// `C = A*B + C` assignment form. See §9.2 item 5 and §12 item B of
    /// `expression_templates_design.md`.
    ///
    /// # Arguments
    ///
    /// * `expr` - Any proxy expression implementing [`Evaluate<F>`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let a = FieldMatrix::<Fp<7>>::identity(3);
    /// let b = FieldMatrix::<Fp<7>>::identity(3);
    /// let c = FieldMatrix::<Fp<7>>::identity(3);
    /// let r = FieldMatrix::<Fp<7>>::eval(&a * &b + &c);
    /// assert_eq!(r.get(0, 0), Fp::<7>::new(2));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `expr`'s shape is inconsistent internally (i.e. the
    /// operator overloads would already have caught this at construction).
    ///
    /// # Complexity
    ///
    /// One allocation plus one kernel call per canonical fusion.
    pub fn eval<E>(expr: E) -> FieldMatrix<F>
    where
        E: Evaluate<F> + sealed::ProxyExpr,
    {
        FieldMatrix::<F>::from(expr)
    }
}

// ─── Operator-overload surface (design §4) ──────────────────────────────────
//
// The dispatch tables in §4 enumerate every (owned, ref) combination. We
// provide proxy-returning impls here in place of the eager impls that T1
// kept in `field/matrix.rs`. The caller site is:
//
//     let r: FieldMatrix<F> = (&a * &b + &c).into();
//
// which Rust desugars to `FusedProductPlus::new(Product::new(&a, &b), &c)`
// via the Add impl for `Product + &M`, then `.into()` on the proxy.

// ---- Mul: matrix × matrix ----

impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for &'a FieldMatrix<F> {
    type Output = Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        Product::new::<F>(self, rhs)
    }
}

impl<'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(&self, rhs)
    }
}

impl<F: FiniteField> Mul<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(self, &rhs)
    }
}

impl<F: FiniteField> Mul<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(&self, &rhs)
    }
}

// Transposed × &M → TransposedProduct
impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Transposed<&'a FieldMatrix<F>> {
    type Output = TransposedProduct<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        TransposedProduct::new::<F>(self.0, rhs)
    }
}

// Scale<F, Transposed<&M>> × &M → ScaledTransposedProduct<F, &M, &M>
// (§5.4 compositional: lets `(alpha * a.t()) * &b + beta * &c` produce the
// αAᵀ·B + βC fusion without overlapping the generic
// `Scale<F, A> + Scale<F, B> → FusedLinear` add impl.)
impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Scale<F, Transposed<&'a FieldMatrix<F>>> {
    type Output = ScaledTransposedProduct<F, &'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        // Inner-dimension check mirroring TransposedProduct::new: Aᵀ·B
        // requires a.rows() == b.rows().
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.1 .0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(rhs),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.1 .0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(rhs)
        );
        ScaledTransposedProduct(self.0, self.1 .0, rhs)
    }
}

// Scale<F, &M> × &M → Scale<F, Product<&M, &M>> (design §4.1)
impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Scale<F, &'a FieldMatrix<F>> {
    type Output = Scale<F, Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        let p = Product::new::<F>(self.1, rhs);
        Scale(self.0, p)
    }
}

// &M × Scale<F, &M> → Scale<F, Product<&M, &M>> (design §4.1, commuted)
impl<'a, 'b, F: FiniteField> Mul<Scale<F, &'b FieldMatrix<F>>> for &'a FieldMatrix<F> {
    type Output = Scale<F, Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>>;
    fn mul(self, rhs: Scale<F, &'b FieldMatrix<F>>) -> Self::Output {
        let p = Product::new::<F>(self, rhs.1);
        Scale(rhs.0, p)
    }
}

// ---- Mul: matrix × scalar (right) ----
// Keeps the same generic bound that T1 used (`F: FiniteField`, covering
// runtime-context fields via right-scalar). Returns `Scale<F, &M>` rather
// than an eager `FieldMatrix<F>` so downstream `+` can fuse to
// `FusedProductPlusScaled` or `FusedLinear`.

impl<'a, F: FiniteField> Mul<F> for &'a FieldMatrix<F> {
    type Output = Scale<F, &'a FieldMatrix<F>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

impl<F: FiniteField> Mul<F> for FieldMatrix<F> {
    type Output = Scale<F, FieldMatrix<F>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

// ---- Mul: matrix × scalar-wrapped Product (scalar · Product) ----

impl<F: FiniteField, A, B> Mul<F> for Product<A, B> {
    type Output = Scale<F, Product<A, B>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

// ---- Left-scalar `F * &M` — per-ConstField macro (same pattern as T1) ----
// T1 stamps this out for every ConstField family because the orphan rule
// forbids a blanket `impl<F: ConstField> Mul<&M<F>> for F`. The T2 change
// is that these now return `Scale<F, &M>` — a proxy — rather than an eager
// `FieldMatrix<F>`.

/// Stamps out `F * &M` / `F * M` returning [`Scale`] proxies for one
/// concrete `ConstField` type. Mirrors the `impl_left_scalar_mul!` macro
/// in `field/matrix.rs` which T1 uses for the eager scalar multiplication
/// path.
macro_rules! impl_left_scalar_mul_proxy {
    ($field_ty:ty $(, $($generics:tt)+)?) => {
        impl<'a $(, $($generics)+)?> Mul<&'a FieldMatrix<$field_ty>> for $field_ty {
            type Output = Scale<$field_ty, &'a FieldMatrix<$field_ty>>;
            #[inline]
            fn mul(self, rhs: &'a FieldMatrix<$field_ty>) -> Self::Output {
                Scale(self, rhs)
            }
        }

        impl$(<$($generics)+>)? Mul<FieldMatrix<$field_ty>> for $field_ty {
            type Output = Scale<$field_ty, FieldMatrix<$field_ty>>;
            #[inline]
            fn mul(self, rhs: FieldMatrix<$field_ty>) -> Self::Output {
                Scale(self, rhs)
            }
        }

        // Left-scalar `F * Transposed<&M>` → `Scale<F, Transposed<&M>>`.
        // Needed so `alpha * a.t()` is a proxy that can participate in the
        // `αAᵀ·B + βC` fusion (§5.4 compositional).
        impl<'a $(, $($generics)+)?> Mul<Transposed<&'a FieldMatrix<$field_ty>>> for $field_ty {
            type Output = Scale<$field_ty, Transposed<&'a FieldMatrix<$field_ty>>>;
            #[inline]
            fn mul(self, rhs: Transposed<&'a FieldMatrix<$field_ty>>) -> Self::Output {
                Scale(self, rhs)
            }
        }
    };
}

impl_left_scalar_mul_proxy!(crate::gfp::Fp<P>, const P: u64);
impl_left_scalar_mul_proxy!(crate::gfp::specialized::GoldilocksFp);
impl_left_scalar_mul_proxy!(
    crate::gfpn::QuadraticExt<C>,
    C: crate::gfpn::ExtConfig
);
impl_left_scalar_mul_proxy!(
    crate::gfpn::CubicExt<C>,
    C: crate::gfpn::ExtConfig
);
impl_left_scalar_mul_proxy!(
    crate::gf2m::Gf2mWide<N, Cfg>,
    const N: usize,
    Cfg: crate::gf2m::Gf2mWideConfig<N> + Send + Sync + 'static
);

// ---- Add: Sum / FusedProductPlus / FusedProductPlusScaled / FusedLinear ----

// &M + &M → Sum
impl<'a, 'b, F: FiniteField> Add<&'b FieldMatrix<F>> for &'a FieldMatrix<F> {
    type Output = Sum<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn add(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        Sum::new::<F>(self, rhs)
    }
}

impl<F: FiniteField> Add<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self, rhs)
    }
}

impl<'b, F: FiniteField> Add<&'b FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self, rhs.clone())
    }
}

impl<F: FiniteField> Add<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self.clone(), rhs)
    }
}

fn elementwise_add_owned<F: FiniteField>(a: FieldMatrix<F>, b: FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(&a),
        <FieldMatrix<F> as MatrixLike<F>>::shape(&b),
        "FieldMatrix::add: shape mismatch"
    );
    let (rows, cols) = <FieldMatrix<F> as MatrixLike<F>>::shape(&a);
    if rows == 0 || cols == 0 {
        // Empty output: reuse one operand's shape. Its storage is empty.
        return a;
    }
    let mut data = FieldVec::<F>::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            data.push(
                <FieldMatrix<F> as MatrixLike<F>>::get(&a, r, c)
                    + <FieldMatrix<F> as MatrixLike<F>>::get(&b, r, c),
            );
        }
    }
    FieldMatrix::from_raw_parts(rows, cols, data)
}

// Product + &M → FusedProductPlus
impl<'c, F, A, B> Add<&'c FieldMatrix<F>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, &'c FieldMatrix<F>>;
    fn add(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::add: product·addend shape mismatch"
        );
        FusedProductPlus(self, rhs)
    }
}

// &M + Product → FusedProductPlus (commuted, design §4.2)
impl<'a, F, A, B> Add<Product<A, B>> for &'a FieldMatrix<F>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, &'a FieldMatrix<F>>;
    fn add(self, rhs: Product<A, B>) -> Self::Output {
        let m = rhs.0.rows();
        let n = rhs.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self),
            (m, n),
            "FieldMatrix::add: addend·product shape mismatch"
        );
        FusedProductPlus(rhs, self)
    }
}

// Product + Scale<F, &M> → FusedProductPlusScaled
impl<'c, F, A, B> Add<Scale<F, &'c FieldMatrix<F>>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlusScaled<Product<A, B>, Scale<F, &'c FieldMatrix<F>>>;
    fn add(self, rhs: Scale<F, &'c FieldMatrix<F>>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs.1),
            (m, n),
            "FieldMatrix::add: product·scaled-addend shape mismatch"
        );
        FusedProductPlusScaled(self, rhs)
    }
}

// Scale<F, &M> + Product → FusedProductPlusScaled (commuted, design §4.2)
impl<'a, F, A, B> Add<Product<A, B>> for Scale<F, &'a FieldMatrix<F>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlusScaled<Product<A, B>, Scale<F, &'a FieldMatrix<F>>>;
    fn add(self, rhs: Product<A, B>) -> Self::Output {
        let m = rhs.0.rows();
        let n = rhs.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self.1),
            (m, n),
            "FieldMatrix::add: scaled-addend·product shape mismatch"
        );
        FusedProductPlusScaled(rhs, self)
    }
}

// Scale<F, A> + Scale<F, B> → FusedLinear
impl<F, A, B> Add<Scale<F, B>> for Scale<F, A>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedLinear<Scale<F, A>, Scale<F, B>>;
    fn add(self, rhs: Scale<F, B>) -> Self::Output {
        assert_eq!(
            self.1.shape(),
            rhs.1.shape(),
            "FieldMatrix::add: shape mismatch between scaled operands"
        );
        FusedLinear(self, rhs)
    }
}

// TransposedProduct + &M → FusedProductPlus<TransposedProduct, &M>
impl<'c, F, A, B> Add<&'c FieldMatrix<F>> for TransposedProduct<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<TransposedProduct<A, B>, &'c FieldMatrix<F>>;
    fn add(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.cols();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::add: Aᵀ·B shape mismatch"
        );
        FusedProductPlus(self, rhs)
    }
}

// ScaledTransposedProduct<F, A, B> + Scale<F, &M>
//   → FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, &M>>
//
// §5.4 compositional fusion for `αAᵀ·B + βC`. Uses a dedicated proxy type
// (`ScaledTransposedProduct`) rather than `Scale<F, TransposedProduct<...>>`
// so the generic `Scale<F, A> + Scale<F, B> → FusedLinear` impl does not
// overlap (Rust conservatively assumes downstream crates may add a
// `MatrixLike<F>` impl for `TransposedProduct<A, B>`).
impl<'c, F, A, B> Add<Scale<F, &'c FieldMatrix<F>>> for ScaledTransposedProduct<F, A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output =
        FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, &'c FieldMatrix<F>>>;
    fn add(self, rhs: Scale<F, &'c FieldMatrix<F>>) -> Self::Output {
        let m = self.1.cols();
        let n = self.2.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs.1),
            (m, n),
            "FieldMatrix::add: scaled-Aᵀ·B · scaled-addend shape mismatch"
        );
        FusedProductPlusScaled(self, rhs)
    }
}

// Scale<F, &M> + ScaledTransposedProduct<F, A, B> (commuted, design §4.2)
impl<'a, F, A, B> Add<ScaledTransposedProduct<F, A, B>> for Scale<F, &'a FieldMatrix<F>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output =
        FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, &'a FieldMatrix<F>>>;
    fn add(self, rhs: ScaledTransposedProduct<F, A, B>) -> Self::Output {
        let m = rhs.1.cols();
        let n = rhs.2.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self.1),
            (m, n),
            "FieldMatrix::add: scaled-addend · scaled-Aᵀ·B shape mismatch"
        );
        FusedProductPlusScaled(rhs, self)
    }
}

// ---- Sub: rewrite A - B as A + (-B) via NegProxy ----

impl<F: FiniteField> Sub<&FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, rhs)
    }
}

impl<F: FiniteField> Sub<&FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(&self, rhs)
    }
}

impl<F: FiniteField> Sub<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, &rhs)
    }
}

impl<F: FiniteField> Sub<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_sub(&self, &rhs)
    }
}

fn elementwise_sub<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    let (ar, ac) = <FieldMatrix<F> as MatrixLike<F>>::shape(a);
    let (br, bc) = <FieldMatrix<F> as MatrixLike<F>>::shape(b);
    assert_eq!(
        (ar, ac),
        (br, bc),
        "FieldMatrix::sub: shape mismatch ({}×{} vs {}×{})",
        ar,
        ac,
        br,
        bc
    );
    if ar == 0 || ac == 0 {
        return a.clone();
    }
    let mut data = FieldVec::<F>::with_capacity(ar * ac);
    for r in 0..ar {
        for c in 0..ac {
            data.push(
                <FieldMatrix<F> as MatrixLike<F>>::get(a, r, c)
                    - <FieldMatrix<F> as MatrixLike<F>>::get(b, r, c),
            );
        }
    }
    FieldMatrix::from_raw_parts(ar, ac, data)
}

// Product - &M → FusedProductPlus<Product, NegProxy<&M>>
impl<'c, F, A, B> Sub<&'c FieldMatrix<F>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, NegProxy<&'c FieldMatrix<F>>>;
    fn sub(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::sub: product·addend shape mismatch"
        );
        FusedProductPlus(self, NegProxy(rhs))
    }
}

// ---- Neg: eager on &M / M, lazy on proxies (design §4.4) ----

impl<'a, F: FiniteField> Neg for &'a FieldMatrix<F> {
    type Output = NegProxy<&'a FieldMatrix<F>>;
    fn neg(self) -> Self::Output {
        NegProxy(self)
    }
}

impl<F: FiniteField> Neg for FieldMatrix<F> {
    type Output = NegProxy<FieldMatrix<F>>;
    fn neg(self) -> Self::Output {
        NegProxy(self)
    }
}

// -NegProxy(x) = x (normalisation per design §3.5)
impl<M> Neg for NegProxy<M> {
    type Output = M;
    fn neg(self) -> Self::Output {
        self.0
    }
}

// ─── Tests (d48a3cfd/T2 success criteria) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    // `#[serial]` keeps global KernelCounts deltas correct when `cargo test`
    // (in-process parallel runner) is used instead of `cargo nextest` (per
    // R1 finding F1). `cargo nextest` already isolates tests in subprocesses,
    // so the annotation is defensive — the goal is correctness under every
    // runner Cargo ships with.
    use serial_test::serial;

    const MERSENNE_31: u64 = 2_147_483_647;
    type M31 = Fp<MERSENNE_31>;

    // GF(2^8) with AES irreducible x^8 + x^4 + x^3 + x + 1.
    struct Gf2m8AesCfg;
    impl Gf2mWideConfig<1> for Gf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "Gf2m8AesCfg";
    }
    type Gf2m8 = Gf2mWide<1, Gf2m8AesCfg>;

    fn rand_m31(rows: usize, cols: usize, seed: u64) -> FieldMatrix<M31> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<M31>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, M31::new(rng.gen::<u64>() % MERSENNE_31));
            }
        }
        m
    }

    fn rand_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    // ─── Fusion trace-counter assertions (success criterion 2) ─────────
    //
    // Every test in this cluster reads process-wide `KernelCounts` atomics.
    // `#[serial]` serialises the cluster against itself so before/after
    // deltas are stable under `cargo test` (in-process parallel runner).
    // `cargo nextest` runs each test in a fresh subprocess and does not
    // strictly need this; the annotation is defensive (per R1 finding F1).

    #[test]
    #[serial]
    fn test_fusion_product_plus_one_gemm_with_beta_call() {
        // (&a * &b + &c).into() must dispatch exactly one `gemm_with_beta`
        // call and zero plain `gemm` / `axpy_linear` calls.
        let a = rand_m31(8, 6, 0x101);
        let b = rand_m31(6, 7, 0x102);
        let c = rand_m31(8, 7, 0x103);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + &c).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.axpy_linear - before.axpy_linear, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_product_plus_scaled_one_gemm_with_beta_call() {
        // (&a * &b + beta * &c).into() must dispatch one `gemm_with_beta`
        // (general β).
        let a = rand_m31(5, 4, 0x201);
        let b = rand_m31(4, 6, 0x202);
        let c = rand_m31(5, 6, 0x203);
        let beta = M31::new(42);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + beta * &c).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.axpy_linear - before.axpy_linear, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_product_plus_scaled_right_one_gemm_with_beta_call() {
        // The commuted form `&a * &b + &c * beta` must also fuse.
        let a = rand_m31(5, 4, 0x301);
        let b = rand_m31(4, 6, 0x302);
        let c = rand_m31(5, 6, 0x303);
        let beta = M31::new(17);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + &c * beta).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_linear_one_axpy_call() {
        // (alpha * &a + beta * &b).into() must dispatch one `axpy_linear`.
        let a = rand_m31(6, 9, 0x401);
        let b = rand_m31(6, 9, 0x402);
        let alpha = M31::new(3);
        let beta = M31::new(5);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (alpha * &a + beta * &b).into();
        let after = kernel_counts();
        assert_eq!(after.axpy_linear - before.axpy_linear, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_transposed_product_one_gemm_trans_a_call() {
        // (a.t() * &b).into() must dispatch one `gemm_trans_a`.
        let a = rand_m31(6, 5, 0x501); // k=6, m=5 → Aᵀ is 5×6
        let b = rand_m31(6, 7, 0x502); // Aᵀ·B ⇒ 5×7
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (a.t() * &b).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_trans_a - before.gemm_trans_a, 1);
        assert_eq!(after.gemm - before.gemm, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_alpha_transposed_product_plus_beta_c_one_call() {
        // `(alpha * a.t()) * &b + beta * &c` must dispatch exactly one
        // `gemm_trans_a_with_beta` call and zero other kernels. This is the
        // full `αAᵀ·B + βC` canonical fusion (issue §5.4 compositional,
        // R1 finding F2).
        let a = rand_m31(6, 5, 0x601); // a is 6×5 → a.t() is 5×6
        let b = rand_m31(6, 7, 0x602); // Aᵀ·B is 5×7
        let c = rand_m31(5, 7, 0x603);
        let alpha = M31::new(3);
        let beta = M31::new(5);
        reset_kernel_counts();
        let _r: FieldMatrix<M31> = ((alpha * a.t()) * &b + beta * &c).into();
        let kc = kernel_counts();
        assert_eq!(kc.gemm_trans_a_with_beta, 1);
        assert_eq!(kc.gemm_trans_a, 0);
        assert_eq!(kc.gemm_with_beta, 0);
        assert_eq!(kc.gemm, 0);
        assert_eq!(kc.axpy_linear, 0);
        assert_eq!(kc.scale_into, 0);
    }

    #[test]
    #[serial]
    fn test_fusion_alpha_transposed_product_plus_beta_c_commuted_one_call() {
        // Commuted form `beta * &c + (alpha * a.t()) * &b` must also fuse.
        let a = rand_m31(6, 5, 0x611);
        let b = rand_m31(6, 7, 0x612);
        let c = rand_m31(5, 7, 0x613);
        let alpha = M31::new(4);
        let beta = M31::new(9);
        reset_kernel_counts();
        let _r: FieldMatrix<M31> = (beta * &c + (alpha * a.t()) * &b).into();
        let kc = kernel_counts();
        assert_eq!(kc.gemm_trans_a_with_beta, 1);
        assert_eq!(kc.gemm_trans_a, 0);
        assert_eq!(kc.gemm_with_beta, 0);
        assert_eq!(kc.gemm, 0);
    }

    #[test]
    fn test_alpha_transposed_product_plus_beta_c_bit_exact() {
        // Bit-exact cross-check: the fused `αAᵀ·B + βC` must match a
        // materialised eager pipeline that computes each subexpression
        // separately. Counter-independent so no `#[serial]` needed.
        let a = rand_m31(6, 5, 0x621);
        let b = rand_m31(6, 7, 0x622);
        let c = rand_m31(5, 7, 0x623);
        let alpha = M31::new(3);
        let beta = M31::new(5);

        let fused: FieldMatrix<M31> = ((alpha * a.t()) * &b + beta * &c).into();

        // Eager expansion: Aᵀ·B → scale by α, C → scale by β, then sum.
        let at_b: FieldMatrix<M31> = (a.t() * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(5, 7);
        for i in 0..5 {
            for j in 0..7 {
                expected.set(i, j, alpha * at_b.get(i, j) + beta * c.get(i, j));
            }
        }
        assert_eq!(fused, expected);
    }

    // ─── Bit-exact fused vs eager (success criterion 3) ────────────────

    fn bit_exact_fused_vs_eager_m31(n: usize) {
        let a = rand_m31(n, n, 0x700 ^ n as u64);
        let b = rand_m31(n, n, 0x701 ^ n as u64);
        let c = rand_m31(n, n, 0x702 ^ n as u64);
        // Fused.
        let fused: FieldMatrix<M31> = (&a * &b + &c).into();
        // Eager: materialise the product first, then add.
        let t: FieldMatrix<M31> = (&a * &b).into();
        let eager: FieldMatrix<M31> = (&t + &c).into();
        assert_eq!(fused, eager, "fused != eager at n={}", n);
    }

    fn bit_exact_fused_vs_eager_gf2m8(n: usize) {
        let a = rand_gf2m8(n, n, 0x800 ^ n as u64);
        let b = rand_gf2m8(n, n, 0x801 ^ n as u64);
        let c = rand_gf2m8(n, n, 0x802 ^ n as u64);
        let fused: FieldMatrix<Gf2m8> = (&a * &b + &c).into();
        let t: FieldMatrix<Gf2m8> = (&a * &b).into();
        let eager: FieldMatrix<Gf2m8> = (&t + &c).into();
        assert_eq!(fused, eager, "fused != eager at n={}", n);
    }

    #[test]
    fn test_fused_bit_exact_m31_n7() {
        bit_exact_fused_vs_eager_m31(7);
    }

    #[test]
    fn test_fused_bit_exact_m31_n64() {
        bit_exact_fused_vs_eager_m31(64);
    }

    #[test]
    #[ignore = "slow: fused-vs-eager bit-exact at n=256 (Mersenne-31)"]
    fn test_fused_bit_exact_m31_n256() {
        bit_exact_fused_vs_eager_m31(256);
    }

    #[test]
    fn test_fused_bit_exact_gf2m8_n7() {
        bit_exact_fused_vs_eager_gf2m8(7);
    }

    #[test]
    fn test_fused_bit_exact_gf2m8_n64() {
        bit_exact_fused_vs_eager_gf2m8(64);
    }

    #[test]
    #[ignore = "slow: fused-vs-eager bit-exact at n=256 (GF(2^8))"]
    fn test_fused_bit_exact_gf2m8_n256() {
        bit_exact_fused_vs_eager_gf2m8(256);
    }

    // ─── Construction-time shape-mismatch panics (success criterion 4) ──

    #[test]
    #[should_panic(expected = "FieldMatrix::mul: inner dimensions must match")]
    fn test_product_construction_panics_on_dim_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(5, 6);
        let _p = &a * &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::add: shape mismatch")]
    fn test_sum_construction_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 3);
        let _s = &a + &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::mul: inner dimensions must match")]
    fn test_transposed_product_construction_panics_on_dim_mismatch() {
        // `a.t() * &b` requires a.rows() == b.rows() (since Aᵀ has
        // a.cols() rows and b is multiplied on the right — the inner dim
        // is a.rows() vs b.rows()).
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(5, 6);
        let _p = a.t() * &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::add")]
    fn test_fused_product_plus_construction_panics_on_shape_mismatch() {
        // Product is 3×5, addend is 3×6 → shape mismatch.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 6);
        let _f = &a * &b + &c;
    }

    // ─── Evaluation-time shape-mismatch panics (success criterion 4 / R1 F4) ──
    //
    // Each kernel primitive asserts `out.shape() == self.shape()`. These
    // tests preallocate a wrong-shape `out` and call `evaluate_into` on a
    // construction-legal proxy so the panic fires inside the kernel rather
    // than inside an operator overload.

    #[test]
    #[should_panic(expected = "copy_into: shape mismatch")]
    fn test_evaluate_into_copy_panics_on_shape_mismatch() {
        // Transposed<&M>::evaluate_into routes through `copy_into`.
        let a = FieldMatrix::<M31>::zeros(3, 4); // transpose shape: 4×3
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        a.t().evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "scale_into: shape mismatch")]
    fn test_evaluate_into_scale_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let s = M31::new(7) * &a;
        s.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "neg_into: shape mismatch")]
    fn test_evaluate_into_neg_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let n = -&a;
        n.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "axpy_linear: output shape mismatch")]
    fn test_evaluate_into_axpy_linear_panics_on_shape_mismatch() {
        // `Sum::evaluate_into` calls `axpy_linear`. Use a construction-legal
        // Sum (matching operand shapes) but a wrong-shape `out`.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let s = &a + &b;
        s.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "gemm_concrete: output shape mismatch")]
    fn test_evaluate_into_gemm_panics_on_shape_mismatch() {
        // Product<&M, &M>::evaluate_into routes through `gemm_concrete` for
        // concrete operands. Product is 3×5; pass a 4×4 `out`.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let p = &a * &b;
        p.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlus::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_gemm_with_beta_panics_on_shape_mismatch() {
        // FusedProductPlus::evaluate_into performs its own output-shape
        // assert before calling the concrete kernel.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 5);
        let mut out = FieldMatrix::<M31>::zeros(6, 6);
        let f = &a * &b + &c;
        f.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "TransposedProduct::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_gemm_trans_a_panics_on_shape_mismatch() {
        // TransposedProduct<&M, &M>::evaluate_into asserts out shape before
        // routing to `gemm_trans_a_concrete`.
        let a = FieldMatrix::<M31>::zeros(6, 5); // a.t() is 5×6
        let b = FieldMatrix::<M31>::zeros(6, 7); // Aᵀ·B is 5×7
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let tp = a.t() * &b;
        tp.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlus<TransposedProduct, C>: output shape mismatch")]
    fn test_evaluate_into_gemm_trans_a_with_beta_panics_on_shape_mismatch() {
        // FusedProductPlus<TransposedProduct<&M, &M>, &M>::evaluate_into
        // routes through `gemm_trans_a_with_beta_concrete`.
        let a = FieldMatrix::<M31>::zeros(6, 5);
        let b = FieldMatrix::<M31>::zeros(6, 7);
        let c = FieldMatrix::<M31>::zeros(5, 7);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let fused = a.t() * &b + &c;
        fused.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(
        expected = "FusedProductPlusScaled<ScaledTransposedProduct, Scale<C>>: output shape mismatch"
    )]
    fn test_evaluate_into_alpha_trans_a_with_beta_panics_on_shape_mismatch() {
        // αAᵀ·B + βC fusion — new in R1 rework.
        let a = FieldMatrix::<M31>::zeros(6, 5);
        let b = FieldMatrix::<M31>::zeros(6, 7);
        let c = FieldMatrix::<M31>::zeros(5, 7);
        let alpha = M31::new(3);
        let beta = M31::new(4);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let fused = (alpha * a.t()) * &b + beta * &c;
        fused.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlusScaled::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_product_plus_scaled_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 5);
        let beta = M31::new(2);
        let mut out = FieldMatrix::<M31>::zeros(6, 6);
        let f = &a * &b + beta * &c;
        f.evaluate_into(&mut out);
    }

    // ─── Allocation-count evidence for fused vs eager (R1 F3) ─────────────
    //
    // Direct assertion: the fused `(&a * &b + &c).into()` path allocates
    // exactly one owned `FieldMatrix<F>` via a single `gemm_with_beta`
    // kernel call, whereas the eager two-step pipeline allocates two —
    // one from the `&a * &b` `gemm` and one from the `&t + &c`
    // `axpy_linear`. We observe this by counting each kernel invocation
    // (each produces exactly one newly-allocated `FieldMatrix<F>`). Any
    // transpose/packing scratch done inside the T1 blocked gemm is
    // `FieldVec`-backed, not a `FieldMatrix`, and is not counted here.
    // See `benches/field_matrix_fusion_results.md` for the full
    // timing + allocation characterisation.

    #[test]
    #[serial]
    fn test_fused_path_allocates_fewer_matrices_than_eager() {
        let a = rand_m31(16, 16, 0x1101);
        let b = rand_m31(16, 16, 0x1102);
        let c = rand_m31(16, 16, 0x1103);

        // Fused path: exactly one owned matrix via one `gemm_with_beta`.
        reset_kernel_counts();
        let _fused: FieldMatrix<M31> = (&a * &b + &c).into();
        let fused_counts = kernel_counts();
        let fused_owned_matrices = fused_counts.gemm
            + fused_counts.gemm_with_beta
            + fused_counts.gemm_trans_a
            + fused_counts.gemm_trans_a_with_beta
            + fused_counts.axpy_linear
            + fused_counts.scale_into
            + fused_counts.neg_into
            + fused_counts.copy_into;

        // Eager path: two owned matrices — one `gemm` for the product, one
        // `axpy_linear` for the sum (the T1 blocked gemm that backs the
        // product also allocates a transposed B scratch internally, but
        // that is not a `FieldMatrix` and is not exposed to callers).
        reset_kernel_counts();
        let t: FieldMatrix<M31> = (&a * &b).into();
        let _eager: FieldMatrix<M31> = (&t + &c).into();
        let eager_counts = kernel_counts();
        let eager_owned_matrices = eager_counts.gemm
            + eager_counts.gemm_with_beta
            + eager_counts.gemm_trans_a
            + eager_counts.gemm_trans_a_with_beta
            + eager_counts.axpy_linear
            + eager_counts.scale_into
            + eager_counts.neg_into
            + eager_counts.copy_into;

        assert_eq!(
            fused_owned_matrices, 1,
            "fused path must produce exactly one owned FieldMatrix (one kernel call)"
        );
        assert!(
            eager_owned_matrices >= 2,
            "eager path must produce at least two owned FieldMatrices; got {}",
            eager_owned_matrices
        );
        assert!(
            fused_owned_matrices < eager_owned_matrices,
            "fused ({}) must allocate fewer owned matrices than eager ({})",
            fused_owned_matrices,
            eager_owned_matrices
        );
    }

    // ─── Scale + NegProxy ───────────────────────────────────────────────

    #[test]
    fn test_scale_into_matches_eager_scalar_mul() {
        let a = rand_m31(5, 7, 0x900);
        let alpha = M31::new(11);
        let lazy: FieldMatrix<M31> = (alpha * &a).into();
        // Eager reference via scalar Mul on each entry.
        let mut eager = FieldMatrix::<M31>::zeros(5, 7);
        for r in 0..5 {
            for c in 0..7 {
                eager.set(r, c, alpha * a.get(r, c));
            }
        }
        assert_eq!(lazy, eager);
    }

    #[test]
    fn test_neg_proxy_into_matches_eager_neg() {
        let a = rand_m31(4, 6, 0xA00);
        let lazy: FieldMatrix<M31> = (-&a).into();
        let mut eager = FieldMatrix::<M31>::zeros(4, 6);
        for r in 0..4 {
            for c in 0..6 {
                eager.set(r, c, -a.get(r, c));
            }
        }
        assert_eq!(lazy, eager);
    }

    #[test]
    fn test_double_negation_normalises() {
        // -NegProxy(x) = x per design §3.5. `-(-&a)` un-wraps the
        // `NegProxy<&FieldMatrix<F>>` back to the bare `&FieldMatrix<F>`,
        // so the result type is `&FieldMatrix<F>` rather than an owned
        // matrix. We clone to materialise and compare.
        let a = rand_m31(3, 3, 0xB00);
        let twice: &FieldMatrix<M31> = -(-&a);
        assert_eq!(twice.clone(), a);
    }

    // ─── MatrixLike trait surface (success criterion 1) ─────────────────

    #[test]
    fn test_sum_matrix_like_get_is_operand_sum() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xC00);
        let b = rand_m31(3, 4, 0xC01);
        let s = &a + &b;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(
                    <_ as MatrixLike<M31>>::get(&s, r, c),
                    a.get(r, c) + b.get(r, c)
                );
            }
        }
        assert_eq!(<_ as MatrixLike<M31>>::shape(&s), (3, 4));
    }

    #[test]
    fn test_scale_matrix_like_get_is_scalar_times_operand() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xD00);
        let alpha = M31::new(5);
        let s = alpha * &a;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(<_ as MatrixLike<M31>>::get(&s, r, c), alpha * a.get(r, c));
            }
        }
    }

    #[test]
    fn test_neg_proxy_matrix_like_get_is_negated_operand() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xE00);
        let n = -&a;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(<_ as MatrixLike<M31>>::get(&n, r, c), -a.get(r, c));
            }
        }
    }

    #[test]
    fn test_fused_linear_matrix_like_get_matches_alpha_a_plus_beta_b() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 5, 0xF00);
        let b = rand_m31(3, 5, 0xF01);
        let alpha = M31::new(2);
        let beta = M31::new(3);
        let fl = alpha * &a + beta * &b;
        for r in 0..3 {
            for c in 0..5 {
                assert_eq!(
                    <_ as MatrixLike<M31>>::get(&fl, r, c),
                    alpha * a.get(r, c) + beta * b.get(r, c)
                );
            }
        }
    }

    // ─── FieldMatrix::eval sugar ────────────────────────────────────────

    #[test]
    fn test_field_matrix_eval_sugar_equals_into() {
        let a = rand_m31(5, 5, 0x1234);
        let b = rand_m31(5, 5, 0x5678);
        let c = rand_m31(5, 5, 0x9ABC);
        let via_eval = FieldMatrix::<M31>::eval(&a * &b + &c);
        let via_into: FieldMatrix<M31> = (&a * &b + &c).into();
        assert_eq!(via_eval, via_into);
    }

    // ─── Subtraction proxy path ─────────────────────────────────────────

    #[test]
    fn test_product_minus_matrix_fuses_with_neg_proxy() {
        // `&a * &b - &c` should build a FusedProductPlus<Product, NegProxy<&M>>
        // and evaluate as A·B - C in one dispatch.
        let a = rand_m31(4, 3, 0x2001);
        let b = rand_m31(3, 5, 0x2002);
        let c = rand_m31(4, 5, 0x2003);
        let fused: FieldMatrix<M31> = (&a * &b - &c).into();
        let t: FieldMatrix<M31> = (&a * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(4, 5);
        for r in 0..4 {
            for cc in 0..5 {
                expected.set(r, cc, t.get(r, cc) - c.get(r, cc));
            }
        }
        assert_eq!(fused, expected);
    }

    // ─── Transposed proxy evaluation ────────────────────────────────────

    #[test]
    fn test_transposed_proxy_evaluates_to_transpose() {
        let a = rand_m31(3, 5, 0x3001);
        // A Transposed proxy wrapping &a materialises to a.transpose().
        let t = a.t();
        let t_mat: FieldMatrix<M31> = t.into();
        assert_eq!(t_mat, a.transpose());
    }
}
