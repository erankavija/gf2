# Armadillo → `FieldMatrix` UX mapping

> Reference: [Armadillo C++ linear algebra library](https://arma.sourceforge.net/), specifically the `arma::Mat` / `arma::Col` public API.
>
> Scope: enumerate the Armadillo API patterns we commit to matching for `FieldMatrix<F>` and `FieldVec<F>`, adapted to idiomatic Rust. The user's epic-level directive: *"target the similar user ergonomics as Armadillo C++ library"*. The goal is not to clone Armadillo's C++; it is to deliver the same ease-of-use in Rust, expressed through operator overloads, views, free functions, and expression templates (proxy types).

## 1. Guiding principles

1. **Operator expressions read like math.** `let c = &a * &b + &c;` must work and should not allocate intermediates when the shapes permit fusion.
2. **No hidden copies.** Every expression tells the reader whether it allocates (owned result) or borrows (view), by using owned types vs. `&T`.
3. **Parity with `BitMatrix`.** Every method on `BitMatrix` that makes sense over a general field has the same name on `FieldMatrix<F>`. A `MatrixLike<Elem>` trait captures the shared surface; both types `impl` it.
4. **Free functions for readability, methods for chaining.** `solve(&a, &b)` reads like mathematics; `a.solve(&b)` chains after a construction. Both are provided.
5. **Panics are localized.** Shape mismatches panic with a clear message at construction/operator boundaries, never silently produce wrong results.

## 2. Side-by-side: Armadillo ↔ our API

| Armadillo C++ | `FieldMatrix<F>` (Rust) | Notes |
|---|---|---|
| `arma::mat A(rows, cols, fill::zeros)` | `FieldMatrix::<F>::zeros(rows, cols)` where `F: ConstField` | `ConstField` bound so we know what zero is |
| `arma::mat A(rows, cols, fill::none)` | `FieldMatrix::<F>::with_capacity(rows, cols)` | No-init for hot paths |
| `arma::eye(n, n)` | `FieldMatrix::<F>::identity(n)` | Same name as `BitMatrix::identity` |
| `arma::randu(n, m)` | `FieldMatrix::<F>::random(n, m, rng)` | Mirrors `BitMatrix::random` |
| `A(i, j)` | `a[(i, j)]` (read) / `a.set(i, j, v)` (write) | `Index<(usize,usize)>` for read; write needs `set` for reduction bookkeeping |
| `A.row(i)` | `a.row(i)` → `&[F]` view | Mirrors `BitMatrix::row_words` but at element granularity |
| `A.col(j)` | `a.col(j)` → `ColView<'_, F>` | Strided view; mirrors `BitMatrix::col_as_bitvec` (but zero-copy) |
| `A.submat(r0, c0, r1, c1)` | `a.submat(r0..=r1, c0..=c1)` | Immutable view |
| `A.submat(...) = X` | `a.submat_mut(r0..=r1, c0..=c1).assign(&x)` | Mutable view via `.assign()` |
| `A.t()` | `a.t()` | Owned transpose. `.t()` returns a lazy `Transpose<&Self>` proxy in Wave 1 design |
| `A.i()` / `inv(A)` | `a.inv()` / `inv(&a)` → `Option<FieldMatrix<F>>` | `None` iff singular |
| `A * B` | `&a * &b` (preferred) or `a * b` (moves) | All four owned/ref combos (as in `BitMatrix`) |
| `A + B`, `A - B`, `-A` | same operator set | All four combos for binary ops |
| `k * A` / `A * k` | `scalar * &a` / `&a * scalar` | Where `scalar: F` |
| `solve(A, B)` | `solve(&a, &b)` → `Option<FieldVec<F>>` or `Option<FieldMatrix<F>>` | Free function uses PLE under the hood |
| `det(A)` | `det(&a)` → `F` | Free function; `a.det()` also works |
| `rank(A)` | `rank(&a)` → `usize` | Reads from cached PLE if available |
| `trace(A)` | `trace(&a)` → `F` | |
| `A.diag()` | `a.diag()` → `FieldVec<F>` | Owned copy of the diagonal |
| `A.is_symmetric()` | `a.is_symmetric()` | |
| `cout << A` | `println!("{}", a)` via `Display` | Mirrors `BitMatrix::Display` styling |
| `A.save("f", raw_ascii)` / `A.load(...)` | `a.write_raw(path)` / `FieldMatrix::read_raw(path)` | Optional; behind `io` feature |

## 3. The `MatrixLike<Elem>` trait

Shared between `BitMatrix` (`Elem = bool`) and `FieldMatrix<F>` (`Elem = F`). Lives in `crates/gf2-core/src/matrix_like.rs`.

```rust
pub trait MatrixLike<Elem> {
    fn rows(&self) -> usize;
    fn cols(&self) -> usize;
    fn get(&self, row: usize, col: usize) -> Elem;
    fn set(&mut self, row: usize, col: usize, v: Elem);
    fn swap_rows(&mut self, r1: usize, r2: usize);
    fn transpose(&self) -> Self where Self: Sized;
    fn shape(&self) -> (usize, usize) { (self.rows(), self.cols()) }
    fn is_square(&self) -> bool { self.rows() == self.cols() }
    fn is_empty(&self) -> bool { self.rows() == 0 || self.cols() == 0 }
    // Default-method helpers go here too: trace, is_symmetric, etc.
}
```

`BitMatrix` gains a trivial impl of this trait in `matrix.rs`; `FieldMatrix<F>` implements it in `field/matrix.rs`. No behavior change for existing `BitMatrix` users.

## 4. Expression templates (proxy types) — Rust idiom

Armadillo uses C++ templates; Rust uses Op-trait implementations that return **proxy structs** deferring evaluation.

```rust
pub struct Sum<A, B>(pub A, pub B);
pub struct Product<A, B>(pub A, pub B);
pub struct Scale<F, M>(pub F, pub M);
pub struct Transposed<M>(pub M);

impl<'a, F: FiniteField> Mul<&'a FieldMatrix<F>> for &'a FieldMatrix<F> {
    type Output = Product<&'a FieldMatrix<F>, &'a FieldMatrix<F>>;
    fn mul(self, rhs: &'a FieldMatrix<F>) -> Self::Output { Product(self, rhs) }
}

impl<F, A, B> Add<C> for Product<A, B> where /* ... */ {
    type Output = FusedProductPlus<Product<A, B>, C>;
    fn add(self, c: C) -> Self::Output { FusedProductPlus(self, c) }
}

// Concrete evaluation at the assignment boundary:
impl<F: FiniteField, A, B> Evaluate<F> for Product<A, B> where /* ... */ {
    fn evaluate_into(self, out: &mut FieldMatrix<F>) { /* fgemm call */ }
}
impl<F, A, B, C> Evaluate<F> for FusedProductPlus<Product<A, B>, C> {
    /// out = A·B + C in one fgemm call (β = 1).
    fn evaluate_into(self, out: &mut FieldMatrix<F>) { /* fgemm with β=1 */ }
}

impl<F: FiniteField, E: Evaluate<F>> From<E> for FieldMatrix<F> {
    fn from(expr: E) -> Self { /* allocate, delegate to evaluate_into */ }
}
```

User code reads:
```rust
// A·B + C is one fgemm, not two passes + a temporary:
let result: FieldMatrix<F> = (&a * &b + &c).into();

// Without expression templates this would be:
//   let t = &a * &b;     // allocates; full fgemm
//   let result = t + &c; // allocates; full O(mn) add
```

**Scope & risk**: the proxy layer is non-trivial. Story **N1** (Wave 1, design-only) specifies which ops fuse and which fall back to eager eval; the scope is capped to the canonical fusions (fgemm with β, `A·B + αC`, `A + α·B`, transpose-apply). Anything beyond that compiles but evaluates eagerly.

## 5. Slicing model

| Armadillo | Ours (Rust) | Returns |
|---|---|---|
| `A(i, span::all)` | `a.row(i)` | `&[F]` (contiguous) |
| `A(span::all, j)` | `a.col(j)` | `ColView<'_, F>` (strided) |
| `A.submat(r0, c0, r1, c1)` | `a.submat(r0..=r1, c0..=c1)` | `MatView<'_, F>` (non-contiguous, strided) |
| `A.rows(a, b)` | `a.row_range(a..=b)` | `MatView<'_, F>` |
| `A.cols(a, b)` | `a.col_range(a..=b)` | `MatView<'_, F>` |
| In-place assign | `a.submat_mut(…).assign(&x)` | `()` |

Views borrow from `FieldMatrix` and **do not allocate**. They implement `MatrixLike<F>` so every algorithm written against the trait (e.g. `ple()`) can recurse into submatrices without copying.

## 6. Panic messages

`FieldMatrix` panics follow `BitMatrix`'s style verbatim, e.g.
```
assertion failed: `rows == other.rows` (3 != 5) at FieldMatrix::add
```

## 7. Checklist for reviewers

When reviewing a PR touching `FieldMatrix` surface:

- [ ] Every new method has a corresponding `BitMatrix` counterpart or a clear reason it doesn't apply.
- [ ] Every new operator implementation covers all four owned/ref combinations (unless the op cannot consume, in which case justify in the PR).
- [ ] Every method that allocates says so in the doc comment's `# Complexity` section.
- [ ] `MatrixLike<F>` is implemented if the type is matrix-shaped.
- [ ] Views do not allocate.
- [ ] Every public method has an `# Examples` block that compiles under `cargo test --doc`.
- [ ] Display output matches `BitMatrix`'s style (aligned columns, one row per line).
