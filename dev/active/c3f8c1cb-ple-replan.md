# PLE Decomposition Replanning Artefact (c3f8c1cb)

**Trigger**: R1 review of commit `74867c3` failed on SSOT / allocation contract.
**Decision**: replanning round per user directive ("performance contract is non-negotiable").
**Date**: 2026-04-25.

## Why R1 failed

The R1 worker built PLE around two helpers:

```rust
fn clone_block<F: FiniteField>(...) -> FieldMatrix<F>;
fn clone_columns<F: FiniteField>(...) -> FieldMatrix<F>;
```

and the recursive driver `ple_recursive(a: &mut FieldMatrix<F>)` repeatedly:

1. `clone_block`s the off-diagonal sub-region into a freshly-owned `FieldMatrix<F>`.
2. Hands the clone to `gemm` / `trsm_lower`.
3. Copies the result back via another `set` loop.

Empirical budget at 4×4 over `Fp<MERSENNE_31>`: **35 `FieldMatrix::new` bumps** (counter pinned in a loose `[3, 120]` window). At 1024×1024 this would explode well into the thousands — fully out of line with the issue's `[hard]` "no intermediate allocation beyond the output" criterion and with the architectural pattern that 83b1ad8b R3..R5 settled on.

## What 83b1ad8b R3..R5 actually settled

After 5 cycles, the project's accepted pattern is:

1. **Recursion uses `MatViewMut<F>` over the input matrix.** No per-level cloning of submatrices. The `MatView` / `MatViewMut` API at `crates/gf2-core/src/field/matrix.rs` already supports `submat`, `submat_mut`, `as_view`, `reborrow`, `split_rows_mut` — use them.

2. **All matrix multiplication routes through the shared kernels** in `field/matrix.rs`:
   - `gemm_into_view(&A, &B, MatViewMut<F>)` for `out := A · B`.
   - `gemm_axpy_into_view(α, &A, &B, β, &C, MatViewMut<F>)` for `out := α A·B + β C`. Supports `out == c` aliasing per cell.
   - `gemm_axpy_into_view_diag(diag_a, α, &A, diag_b, &B, β, &C, MatViewMut<F>)` if implicit-unit-diagonal operands enter the picture (probably not for PLE, since L is non-unit on the leading rank-`r` block — but available if needed).

   Each gemm call intrinsically allocates one `b.transpose()` scratch (T1's blocked structure). That cost is **inherent and accepted**; under `#[cfg(test)]` the `FIELDMATRIX_NEW_COUNT` thread-local counter records 2 bumps per gemm call (`MatView::to_owned` + `FieldMatrix::transpose`). Production heap impact: one `FieldVec<F>` per call.

3. **Triangular calls** (`trsm_lower`, `trsm_upper`, `trtri_*`, `trtrm`) take `MatView<F>` / `MatViewMut<F>` and allocate per their published budget at `crates/gf2-core/src/field/triangular.rs:108-167`. PLE consumes them through views, never through clones.

4. **Final assembly is the only place that allocates `FieldMatrix<F>` instances**:
   - One owned `L: FieldMatrix<F>` of shape `m × rank`.
   - One owned `E: FieldMatrix<F>` of shape `rank × n`.
   - The `Permutation` is a `Vec<usize>`, no matrix.

5. **Allocation regression test** (`#[serial_test::serial]`) pins exact counts at three sizes. The empirical budget is whatever the architecturally-correct implementation actually does — usually:
   - One initial clone of the input (so we don't destroy the caller's `&self`).
   - Per recursive level: 1 gemm B-transpose + 1 trsm B-transpose (in the rank-deficient branch the trsm of `A3` against `L1[0..r1, 0..r1]` and the gemm of `L1[r1..m, 0..r1] · A3` into `A4`).
   - At the leaves: 0.
   - Final `L`/`E` assembly: 2.

   For `n = 4` over `Fp<MERSENNE_31>`, expect ~5 bumps total. For `n = 64`, expect ~5 + 2 × log₂(64/1) bumps from the recursive halving of the column dimension. For `n = 1024`: ~5 + 2 × 10 = ~25, NOT 35-at-`n=4`-with-cloning.

## Concrete contract for the rework

The R2 worker MUST hit these targets. The reviewer will reject anything looser.

### API surface (unchanged from issue description)

```rust
pub struct Permutation { ... }                   // Vec<usize>-backed

impl<F: FiniteField> FieldMatrix<F> {
    pub fn ple(&self) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize);
    pub fn row_echelon(&self) -> (FieldMatrix<F>, FieldMatrix<F>);
    pub fn rref(&self) -> (FieldMatrix<F>, FieldMatrix<F>);
    pub fn rank(&self) -> usize;
    pub fn nullspace(&self) -> Vec<FieldVec<F>>;
    pub fn lu(&self) -> Option<(Permutation, FieldMatrix<F>, FieldMatrix<F>)>;
}
```

### Internal driver

```rust
/// In-place PLE on the supplied MatViewMut. Records pivot row swaps in
/// `perm` (caller-managed). Writes the L-factor's column-by-column
/// values directly into the lower part of `a`'s storage; the upper
/// triangular E factor lives in the upper part. This is the
/// "compact storage" trick from Dumas-Pernet §2.2 (alg 2.5 closing
/// note): the algorithm overwrites `a` so that after recursion
/// `a[i, j]` holds either L[i, j] (i > pivot row of column j) or
/// E[i, j] (i ≤ pivot row of column j). The rank is returned; the
/// caller separates the two factors at the end.
fn ple_in_place<F: FiniteField>(
    mut a: MatViewMut<'_, F>,
    perm: &mut [usize],
) -> usize { /* returns rank */ ... }
```

The public `FieldMatrix::ple()` wrapper:

1. Clones `self` once (the input is `&self` so we cannot destroy it). 1 alloc.
2. Builds `perm: Vec<usize> = (0..m).collect()`. 0 matrix allocs.
3. Calls `ple_in_place(working.submat_mut(.., ..), &mut perm)`. → returns `rank`.
4. Splits `working` into `L` (m × rank, lower triangle of the pivoted columns) and `E` (rank × n, upper triangle). 2 allocs.
5. Returns `(Permutation::from_inverse(perm), L, E, rank)`.

Total: **3 owned `FieldMatrix<F>` allocations**, plus whatever gemm/trsm intrinsically allocate during the recursion. For `n = 4`, expected count: 3 + 2 × (one gemm + one trsm) = ~7. For `n = 64`: 3 + 2 × log₂(64) × (~1.5) = ~33. For `n = 1024`: 3 + 2 × log₂(1024) × (~1.5) ≈ 33.

These are guideline numbers; the actual values get pinned in a regression test.

### Algorithm shape (Dumas-Pernet §2.2 alg 2.5)

```
ple_in_place(a: MatViewMut<m × n>, perm: &mut [m]) -> rank:
    if n == 1:
        # Find first non-zero entry, swap to row 0, normalize column
        for i in 0..m:
            if a[i, 0] != 0:
                if i != 0:
                    a.swap_rows(0, i)
                    perm.swap(0, i)
                let pivot = a[0, 0]
                let pivot_inv = pivot.inv()
                for k in 1..m:
                    a[k, 0] = a[k, 0] * pivot_inv  # write L into the column below
                # a[0, 0] stays as the pivot value E[0, 0]
                return 1
        return 0  # whole column is zero

    let h = n / 2
    let (a_left, a_right) = a.split_cols_mut(h)  # zero-copy
    let r1 = ple_in_place(a_left.reborrow(), perm)

    # Apply the row permutations recorded in `perm[0..r1]` to a_right.
    # No alloc — uses MatViewMut::swap_rows.
    apply_perm_to_right(&perm[0..r1], a_right.reborrow())  # actually trickier; see below

    # Split a_right into A3 (top r1 rows) and A4 (bottom m-r1 rows)
    let (a3, a4) = a_right.split_rows_mut(r1)

    # A3 ← trsm_lower(L1, A3) where L1 = a_left[0..r1, 0..r1]
    # L1 has unit diagonal (the algorithm normalises so), so use trsm_lower with
    # implicit unit diagonal — or trsm with explicit diagonal if the convention
    # is different.
    let l1_top = a_left.submat(0..r1, 0..r1)  # MatView
    trsm_lower(l1_top, a3.reborrow())  # in-place; allocates 1 gemm B-transpose

    # A4 ← A4 − L1_bot · A3 where L1_bot = a_left[r1..m, 0..r1]
    let l1_bot = a_left.submat(r1..m, 0..r1)  # MatView
    let a3_view = a3.as_view()  # read-only borrow
    gemm_axpy_into_view(F::neg_one(), &l1_bot, &a3_view, F::one(), &a4_view, a4.reborrow())
    # 1 gemm B-transpose

    let r2 = ple_in_place(a4.reborrow(), &mut perm[r1..])

    return r1 + r2
```

A few points the reviewer will check:

- **No `clone_block` / `clone_columns` / `to_owned` calls in the recursive driver.** Only `submat`, `submat_mut`, `split_cols_mut`, `split_rows_mut`, `reborrow`, `as_view`.
- The `apply_perm_to_right` step has to use `MatViewMut::swap_rows` since `a_left` and `a_right` are disjoint borrows — we can swap rows of `a_right` without touching `a_left`.
- The `trsm_lower(L1, A3)` call expects A3 to be `MatViewMut`. Either the existing `trsm_lower` already takes views (it should; it was 5 cycles of work to ensure that) or we need a thin shim. Verify.
- The `gemm_axpy_into_view` aliasing rule (out == c per-cell aliased) must be respected. The R3/R4/R5 work in 83b1ad8b documented exactly this; reuse.

### Derived operations

- `row_echelon`: PLE then return `(L^{-1}_top, E)` where `L^{-1}_top` is the inverse of `L`'s top `r × r` block (use `trtri_lower`). One additional `r × r` allocation for the inverted block.
- `rref`: `row_echelon` then zero above the leading 1s using one more `gemm`-into-view pass.
- `rank`: just the 4th return value of `ple()`.
- `nullspace`: read non-pivot columns of E, build basis vectors. Each basis vector is one `FieldVec<F>` allocation. Total: `(n - rank)` `FieldVec` allocs, no `FieldMatrix` allocs.
- `lu`: one PLE call. Returns `Some` iff rank == min(m, n). When `Some`, repackage the existing `L` and `E` as `(P, L, U)` (no extra allocations beyond what PLE already did).

### Allocation regression test

Add `#[serial_test::serial]` tests pinning the counter for each derived op:

```rust
#[test]
#[serial]
fn test_ple_allocation_budget_n64_fp_m31() {
    let a = random_fp::<MERSENNE_31>(64, 64, 0xC3F8);
    reset_fieldmatrix_new_count();
    let _result = a.ple();
    let allocs = fieldmatrix_new_count();
    assert_eq!(
        allocs, EXPECTED,
        "PLE at m=n=64 should allocate exactly EXPECTED FieldMatrix::new, got {allocs}"
    );
}
```

Where EXPECTED is whatever the corrected implementation actually does. Run, observe, write the number into the assertion. Same pattern at `n = 256` and `n = 1024`. Cross-link to triangular's test for the alloc-counting precedent.

### Doc contract

Module rustdoc opens with:

> # Allocation budget
>
> The PLE recursion runs in place on a single working clone of the input
> matrix; submatrices are passed as `MatView` / `MatViewMut`. Each
> recursive level pays for two intrinsic gemm-kernel B-transposes (one
> from `trsm_lower` on the rank-deficient branch, one from
> `gemm_axpy_into_view` for the Schur complement update). Total
> `FieldMatrix<F>` allocations:
>
> - `ple(m × n)`: 3 (input clone + final L + final E) + ~2 × log₂(min(m, n)).
> - `row_echelon(m × n)`: PLE + 1 (the inverted L-top block).
> - `rref(m × n)`: row_echelon + 1 (the back-subst gemm intermediate, scratch).
> - `lu(m × n)`: PLE + 0 (just repackages PLE's outputs).
> - `nullspace(m × n)`: PLE + (n − rank) `FieldVec` allocations.
>
> Exact counts are pinned in `tests::test_*_allocation_budget`.

Followed by the algorithm sketches, panic semantics, and complexity per the standard public-API doc contract.

## Testing checklist (must all pass)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
- `cargo nextest run --workspace --all-features --release --profile ci`: all pass.
- `cargo test -p gf2-core --lib --release`: all pass — INCLUDING the alloc-budget tests under default `cargo test` parallelism (use `#[serial]`).
- `cargo test -p gf2-core --doc --release`: all pass.
- `cargo bench -p gf2-core --bench ple --features rand -- --test`: all bench cases compile and run.

## What NOT to do (lessons from 83b1ad8b's 5 cycles)

1. **Do not write `clone_block` / `clone_columns` / `submatrix-to-owned` helpers** in the recursive driver. The `MatView` API is sufficient.
2. **Do not implement matrix multiplication anywhere outside the shared `gemm*` kernels.** Bespoke per-cell loops are SSOT violations.
3. **Do not over-allocate then "pin" the count in a loose window.** The reviewer reads strict integer asserts and rejects loose ranges.
4. **Do not propose your own scope amendments**. If a `[hard]` criterion is empirically unmeetable, escalate to the lead with measured evidence; the lead escalates to the user.
5. **Do not skip `#[serial_test::serial]` on counter-reading tests.** Even with the thread-local counter (per 83b1ad8b R5), `#[serial]` is hygiene that future global state changes won't break.
6. **Do not split the work into pre-emptive multiple commits.** One feature commit + one test/bench commit max. The reviewer weighs commit hygiene.
7. **Do not modify the `Permutation` API surface from the issue description.** Compact `Vec<usize>` representation only.

## Out-of-scope checks

- Allocation behaviour for `Fp<65521>` and `Gf2mWide<16>`: tested but not pinned. Only `Fp<MERSENNE_31>` and `Gf2mWide<8>` get pinned counts.
- SIMD optimisation of any PLE component: out of scope. Inherit SIMD from gemm.
- Threshold-based base-case dispatch (a la `WINOGRAD_THRESHOLD`): out of scope; the existing `n == 1` base is the algorithm.
- Block-recursive variants of row_echelon and rref beyond what alg 2.6/2.7 specify: out of scope.

## Closing note

The 83b1ad8b cycle taught us that the matrix-primitives surface in this project has a specific "shape" the reviewer wants: views all the way down, gemm kernels for all matrix multiplication, allocation costs documented honestly. The R1 PLE worker missed this shape and chose a clone-heavy implementation that would have looked fine in isolation but doesn't fit. The R2 dispatch must enforce the shape upfront so we don't repeat the multi-cycle drift.
