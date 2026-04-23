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

The `MatrixLike` trait carries an associated type `Owned: MatrixLike<Elem>`
so that `transpose(&self) -> Self::Owned` stays total. Concrete matrices
(`BitMatrix`, `FieldMatrix<F>`) set `Owned = Self` and return an in-kind
transpose. Zero-copy views (`MatView<'_, F>`, `MatViewMut<'_, F>`) set
`Owned = FieldMatrix<F>` and materialise a fresh owned matrix, because a
row-major borrowed slice cannot be reinterpreted in place as a column-major
one without physical data motion. Each view also exposes an inherent
`to_owned(&self) -> FieldMatrix<F>` so callers can reify explicitly; the
trait `transpose` composes `to_owned` with `FieldMatrix::transpose`.

This replaces the earlier `unimplemented!()` sketch — a public trait
method must not panic for any valid receiver, so the `Owned` type was
introduced to encode the "views return owned" semantics in the type
system rather than at runtime.

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
of those optimisations are explicitly deferred to issue `d48a3cfd`.

The eager path is bounded on `F: FiniteField` (not `ConstField`) so that
runtime-context fields such as `Gf2mElement` participate in matrix
multiplication directly. The zero-element comparison used for the
sparsity early-exit is sourced via `FiniteField::zero_like()` on an
interior element, and the output matrix is initialised with
`FieldVec::zeros_from(.., &zero)` so no `F::zero()` constant is required.
The same transformation applies to `matvec` and `matvec_transpose`.

## 6. `to_sparse` signature ships here, implementation stub

The issue contract lists `to_sparse` as part of the public surface of
`FieldMatrix<F>`. The full sparse representation — CSR over `F` entries,
arithmetic, conversion helpers — is owned by story `8a90882e`, but the
*signature* must exist in this story so callers can compile against the
final shape.

Implementation: add a dedicated module `crates/gf2-core/src/field/sparse_matrix.rs`
that defines a minimal `SparseFieldMatrix<F: FiniteField>` backed by a
`Vec<(usize, usize, F)>` triplet list, plus a crate-private
`from_dense_stub` constructor. `FieldMatrix::to_sparse` scans the dense
matrix and emits one triplet per non-zero entry (O(rows · cols)). The
stub compiles cleanly and behaves correctly; the earlier worry that the
signature "won't compile" was mistaken — a standalone stub type is a
valid target as long as the owning story (`8a90882e`) replaces it
wholesale. The stub module carries an `8a90882e` reference comment so
the follow-up story finds it on the first search.

This intentionally leaves no `todo!()` or `unimplemented!()` behind:
the `to_sparse` body is fully functional. What `8a90882e` replaces is the
internal representation and the additional operator surface, not the
`FieldMatrix::to_sparse` method itself.

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
- **`transpose` on views**: views now reify an owned `FieldMatrix<F>`
  via `MatrixLike::Owned` and the inherent `to_owned` path. If `cdcebf6a`
  later introduces a dedicated column-major view type, the trait method
  can be specialised without source-breaking callers because the return
  type is still `Self::Owned`.
- **Scalar `Mul<F>` for non-`Fp` fields**: the `F * &M` direction is only
  implemented for `Fp<P>` because a blanket `impl<F> Mul<&M<F>> for F`
  triggers orphan-rule trouble. `Gf2mElement` (which is not `Copy`) gets
  `&M * F` exclusively for now — enough for all current call sites.
