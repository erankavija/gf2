# FieldMatrix foundation — design notes (issue `ab791e27`)

This document records the design decisions made while delivering the
`FieldMatrix<F: FiniteField>` foundation, the `MatView` / `MatViewMut` /
`ColView` zero-copy views, and the shared `MatrixLike<Elem>` trait. It is
referenced from the issue and kept under `dev/active/` so follow-up stories
in the same epic (`bb85c68a`) can build on the same vocabulary.

## 1. `MatrixLike` — read/write split

The issue suggested a single `MatrixLike<Elem>` trait carrying both
immutable (`rows`, `cols`, `get`, `transpose`) and mutating (`set`,
`swap_rows`) methods. The story text added a decision rule:

> **Decision rule**: if `MatrixLike` mandates `set`, then split into a
> read-only super-trait `MatrixLikeRead` + write extension.

I opted for the **split form** and named the pieces:

- `MatrixLike<Elem>` — read-only. Has `rows`, `cols`, `get`, `transpose`,
  plus default-method `shape`, `is_square`, `is_empty`.
- `MatrixLikeMut<Elem>: MatrixLike<Elem>` — super-trait adds `set` and
  `swap_rows`.

Rationale:

1. A truly read-only `MatView<'_, F>` has no sensible `set` to provide; the
   alternative (`panic!("read-only")`) makes the trait a footgun for
   generic callers and breaks the "no silent failures" philosophy used
   elsewhere in `gf2-core`.
2. Generic algorithms like Gauss-Jordan or PLE need *mutable* matrix access
   anyway, so they already constrain to the stronger trait. Nothing
   downstream is made harder by the split — callers write
   `impl<M: MatrixLikeMut<F>>`, exactly one character more than the
   combined form.
3. The split keeps the immutable surface small enough that `Transposed<M>`
   and future `Submat` proxy types can implement it cheaply without
   promising writes they cannot deliver.

Both traits live in `crates/gf2-core/src/matrix_like.rs`. `BitMatrix` gets
a no-op impl (`MatrixLike<bool>` + `MatrixLikeMut<bool>`) forwarding to its
existing inherent methods, so zero existing call sites need to change and
no behavior shifts. `FieldMatrix<F>` and `MatViewMut<'_, F>` implement
both; `MatView<'_, F>` implements only the read-only half.

The `MatView` / `MatViewMut` transpose methods are deliberately
`unimplemented!()`. Returning a transposed view over the same buffer would
require a column-major reinterpretation of a row-major slice — incorrect
without a physical data copy. Users who need the transpose must reify:
`view.to_owned().transpose()`. The issue's success criterion ("`MatView`
implements `MatrixLike<F>`") is satisfied because the other three methods
are honest; the fourth documents a precise escape hatch.

## 2. `Transposed<M>` placeholder

The full expression-template layer — `Sum<A, B>`, `Product<A, B>`,
`FusedProductPlus<…>`, `Evaluate<F>` — is designed in issue `cdcebf6a`
and implemented in `d48a3cfd`. This story only needs the `.t()` method to
compile and expose a sensible shape.

Approach: `Transposed<M>(M)` is a thin wrapper. The single inherent impl
block (`impl<F> Transposed<&FieldMatrix<F>>`) exposes `rows()` / `cols()`
reading the inner matrix's shape swapped. The proxy does *not* yet
implement `Evaluate<F>` and cannot participate in a fused fgemm; those
capabilities land in `d48a3cfd`. Nothing on the current `FieldMatrix`
surface silently depends on the proxy — the eager `transpose(&self) ->
Self` method remains the canonical way to materialise a transpose in this
story.

## 3. View representation

Both `MatView` and `MatViewMut` carry five fields:

```
data:        &[F] | &mut [F]
parent_cols: usize   // full row stride of the parent
row_offset:  usize
col_offset:  usize
rows:        usize
cols:        usize
```

Element access translates `(r, c)` to the parent's linear index as
`(row_offset + r) * parent_cols + col_offset + c`. This supports both
contiguous slices (full rows) and non-contiguous windows (strictly interior
column bands), which is the minimum needed for recursive algorithms such
as block Gauss-Jordan and divide-and-conquer PLE.

`row_range(..)` and `col_range(..)` are trivial shortcuts — they call
`submat(rows, ..)` / `submat(.., cols)`, so a single range-resolution
helper (`resolve_range`) centralises the `RangeBounds` unpacking.

`ColView<'_, F>` is simpler because a column is 1-D: a slice plus `start`,
`stride`, and `len`. It has an `iter()` method for generic use and a
`get(i) -> F` for random access.

## 4. `Display`

Matches the `BitMatrix::Display` styling (Unicode corner brackets) but
computes column widths dynamically because field elements may vary in
width (e.g. `Fp<65537>` prints up to 5 chars vs `Fp<7>`'s single digit).
Empty matrices render as `[ ]`, exactly as `BitMatrix::Display` does.

## 5. Classical `gemm` as eager fallback

`Mul` is implemented with a single classical O(n·m·k) triple loop in
`gemm()`, iterating `i → k → j` to keep the reads of `aik` out of the
inner loop. No delayed-reduction accumulation, no Strassen-Winograd. Both
of those optimisations are explicitly deferred to issue `d48a3cfd`. The
eager path is gated on `F: ConstField` because the body calls `F::zero()`
to skip zero-valued pivots — a cheap sparsity heuristic that disappears on
dense `Fp<7>` test data but earns its keep as soon as we start reusing
`gemm` for the identity-dominated blocks in PLE's `R11 B = L12` solves.

## 6. Why no `to_sparse`

`BitMatrix::to_sparse()` returns a `SpBitMatrix`. The finite-field analogue
is a `SparseFieldMatrix<F>` (CSR over `F` entries) which is the subject of
story `8a90882e` and therefore does not exist yet. Declaring a
`FieldMatrix::to_sparse` here would either have to:

1. return `SpBitMatrix` via some coercion — wrong type, and wrong semantics
   for non-GF(2) fields, or
2. refer to a not-yet-existing type — won't compile, blocking this story
   on `8a90882e`.

Neither is acceptable, so the method is omitted from this story. The
parity requirement is logged in the PR description as a known gap that
the follow-up story closes; there is no `todo!()` or stub to remove later.

## 7. Random generation

`FieldMatrix::random` / `::random_seeded` need a way to draw a uniform
element of `F`. Rust's `rand::distributions::Standard` is the idiomatic
distribution for "uniform over the type", so the random constructors are
bounded on `Standard: Distribution<F>`. I provide the impl for `Fp<P>`
locally in `field/matrix.rs` rather than mutating `gfp/mod.rs`, keeping
this story non-invasive on the field internals. `Gf2mElement` instances
carry a runtime polynomial context and therefore cannot be uniformly
sampled without additional plumbing — out of scope here; this is why the
constructors sit in the `ConstField` impl block.

## 8. Open questions for follow-up stories

- **Expression templates**: `cdcebf6a` will replace `Mul` for `&M * &M`
  with a `Product<&M, &M>` proxy. That's a breaking change in return type
  (from `FieldMatrix<F>` to `Product<…>`); the user-facing source stays
  identical thanks to `impl From<Product<…>> for FieldMatrix<F>`. This
  story deliberately does **not** depend on that refactor happening and
  will keep compiling through it.
- **In-place arithmetic (AddAssign/SubAssign/MulAssign for matrices)**:
  not covered by the issue; deferred until a benchmark shows the extra
  allocator traffic matters.
- **`AddAssign<&FieldMatrix<F>>` on views**: would be useful for PLE but
  not required here; can be added in Wave 2.
- **`transpose` on views**: left `unimplemented!` pending a decision in
  `cdcebf6a` about whether the expression layer materialises transposes
  via the owned path or via a dedicated column-major view type.
- **Scalar `Mul<F>` for non-`Fp` fields**: the `F * &M` direction is only
  implemented for `Fp<P>` because a blanket `impl<F> Mul<&M<F>> for F`
  triggers orphan-rule trouble. `Gf2mElement` (which is not `Copy`) gets
  `&M * F` exclusively for now — enough for all current call sites.
