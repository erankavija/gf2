# bdf95060 — Batch polynomial operations (breakdown)

**Parent story**: bdf95060 — Implement batch polynomial operations for extension fields
**Target**: 4–8 task issues, each 1–3 days of implementation effort

## Scope summary

The story introduces a new generic module `crates/gf2-core/src/field/poly.rs` exposing `FieldPoly<F: FiniteField>` with the standard univariate toolkit: Horner evaluation, schoolbook/Karatsuba multiplication, division, GCD, batch evaluation (via subproduct tree), interpolation (Lagrange + fast), and NTT-based multiplication for NTT-friendly fields.

**What's already in the repo (must be reused or explicitly superseded)**:

- `Gf2mPoly` / `Gf2mPoly_<V>` in `crates/gf2-core/src/gf2m/field.rs` (≈lines 2103–3000+) already implements `new`, `zero`, `constant`, `degree`, `coeff`, `from_bitvec`, `monomial`, `x`, `from_roots`, `product`, `eval` (Horner), `eval_batch` (naive), `div_rem`, `gcd`, schoolbook mul, Karatsuba mul (threshold 32). This is the concrete reference for the generic version — shape, docs, and invariants all transfer.
- `gf2-coding/src/bch/core.rs` consumes `Gf2mPoly` (`generator`, `gcd`, `from_roots`, `minimal_polynomial`, etc.). BCH decoding does **not** currently use a subproduct tree for Chien search or a fast Lagrange interpolation for Forney, so the new module is a strict capability addition, not a rewrite trigger.
- `crates/gf2-core/src/field/batch_ops.rs` is the *style template*: module-level algorithm prose, in-place + allocating + with-scratch variants, zero-handling doc block, op-count instrumented tests, and proptest axioms. The new module should match this shape.
- `Fp<P>` in `gfp/specialized.rs` exposes `classify()`, `is_proth_prime()`, `is_goldilocks_prime()`, `PrimeShape::Proth { k, n }`. **There is no existing `two_adicity` / `primitive_root_of_unity` / `ntt_root` API** in the repo (Grep confirmed: no hits on `two_adicity`, `root_of_unity`, `NTT`, `ntt`). The NTT task therefore has to *build* that API on top of the existing Proth classifier — it cannot assume it.

**Spec assumptions vs. reality**:

- The spec says "`Gf2mPoly` operations can delegate to `FieldPoly<Gf2mElement>` (or remain specialized)". Delegation is a *breaking* rewrite of `Gf2mPoly` and a ripple through `bch/core.rs`. The breakdown **keeps `Gf2mPoly` as-is** and only adds a conversion helper, deferring any delegation/replacement to a later epic.
- The spec's "batch_gcd" is specified as pairwise reduction. This is genuinely 1-dim; one task covers it.
- NTT needs a primitive `2^k`-th root of unity. For the currently supported `Fp<P>` Proth primes (BabyBear `15·2^27 + 1`, etc.) this is a real, finite two-adicity. The API needs to surface it.

## Label conventions for this breakdown

All child tasks should carry:

- `type:task`
- `component:gf2-core`
- `epic:e095a100` (exists)
- `story:bdf95060` (**does not yet exist** — lead must create this story-label value before dispatching the first child; JIT lets a value be added on first use via `--labels story:bdf95060`, but the label will need a human-readable description)

The `gf2-coding` integration task additionally carries `component:gf2-coding`.

## Proposed task DAG

### Task 1 — Core `FieldPoly<F>` type, normalisation, and basic arithmetic

- **Type**: task
- **Depends on**: — (parent story 72a2118a already done; trait surface bfe0ba7b done)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/poly.rs` (new)
  - `crates/gf2-core/src/field/mod.rs` (re-exports)
- **Size estimate**: 1–1.5 days
- **Success criteria**:
  - `pub struct FieldPoly<F: FiniteField> { coeffs: Vec<F> }` exists, with ascending-degree storage.
  - Constructors: `new(coeffs)`, `zero_like(&F)`, `one_like(&F)`, `constant(F)`, `monomial(F, usize)`, `from_coeffs_trimmed(…)`.
  - Queries: `degree() -> Option<usize>` (None for zero), `is_zero()`, `coeff(i) -> F`, `leading_coeff()`, `len()`, `iter()`.
  - Normalisation: every constructor and mutating op trims trailing zero coefficients (the module's equivalent of `mask_tail` — critical invariant; document prominently).
  - Operator overloads: `Add`, `Sub`, `Neg`, `AddAssign`, `SubAssign` (both owned and `&`-ref RHS) for `FieldPoly<F>` matching the trait style already used for field elements.
  - Schoolbook `mul(&self, &Self) -> Self`. Karatsuba deferred to Task 2.
  - Scalar `mul_scalar(&self, &F)` and in-place `scale(&mut self, &F)`.
  - Pretty `Debug` impl showing non-zero terms in descending degree.
  - Doc comments on every public item with worked `# Examples` in both `Fp<7>` and `Gf2mElement` flavours (compiled by `cargo test --doc`).
  - Unit tests: constructors, normalisation (trailing zeros), zero polynomial, degree of constant, schoolbook over `Fp<7>` and `Gf2mElement`.
  - Proptests (≤64 cases per test to stay under the 60s budget): `(a+b)+c == a+(b+c)`, `a*(b+c) == a*b + a*c`, `deg(a*b) == deg(a)+deg(b)` when neither is zero.
- **Rationale**: Establishes the type, normalisation invariant, and the operator-overload boilerplate that every later task depends on. Deliberately excludes Karatsuba, evaluation, and division so this stays a tight 1–1.5 day unit.

### Task 2 — Division, GCD, Horner eval, and Karatsuba multiplication

- **Type**: task
- **Depends on**: Task 1
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**: `crates/gf2-core/src/field/poly.rs`
- **Size estimate**: 2 days
- **Success criteria**:
  - `div_rem(&self, &Self) -> (Self, Self)` with `deg(r) < deg(divisor)` invariant (panics on divisor zero).
  - Standard `Div`/`Rem` operator overloads built on top.
  - `gcd(a, b) -> Self` via Euclidean algorithm; returns a *monic* result (divide by leading coefficient). For binary fields the monic-ness is trivial; covered by a generic test using `Fp<7>`.
  - `evaluate(&self, point: &F) -> F` using Horner's method (empty polynomial = zero, matching `Gf2mPoly::eval`'s fixed behaviour — note this differs from current `Gf2mPoly::eval` which panics; align on the zero-returns-zero convention and document).
  - `mul(&self, &Self)` upgraded to Karatsuba with the same `KARATSUBA_THRESHOLD = 32` cut-off that `Gf2mPoly::mul_karatsuba` uses (keep the threshold as a named `const` for later tuning).
  - Proptests: `(a*b) / b == a` (for non-zero b), `gcd(a, b)` divides both a and b, `evaluate` vs naive sum-of-terms agreement.
  - Op-count microbench or assertion that Karatsuba drops below schoolbook at n ≥ 64 (can be a `#[test]` using a counting newtype analogous to `batch_ops::OpCount`, not a criterion bench).
- **Rationale**: Completes the single-pair polynomial algebra. Kept separate from Task 1 because Karatsuba + div_rem is genuinely the heaviest logic outside NTT and benefits from a focused review.

### Task 3 — Batch evaluation via subproduct tree

- **Type**: task
- **Depends on**: Task 2
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/poly.rs` (extension)
  - `crates/gf2-core/benches/field_poly.rs` (new)
- **Size estimate**: 2 days
- **Success criteria**:
  - `batch_evaluate(&self, points: &[F]) -> Vec<F>` implemented via subproduct tree:
    1. Leaves `M_i = x − point_i` (so `len = 2`).
    2. Internal nodes = products of their children (reuse `FieldPoly::mul`).
    3. Top-down reduction of `self` modulo each internal subproduct (reuse `div_rem`).
    4. Leaf values = evaluations.
  - Fallback to per-point Horner when `points.len() < 16` or `self.degree() < 16` (document threshold as a named `const`; the bench must be what tunes it).
  - Agreement proptest: `batch_evaluate(points) == points.map(|p| evaluate(p))`.
  - `cargo bench -p gf2-core --bench field_poly` shows subproduct-tree path beats k individual Horner evaluations for k ≥ 16, n ≥ 16 on `Fp<65537>` (matches the story's stated success criterion verbatim). Results committed to the module docstring (mirroring how `batch_ops.rs` documents its benchmark table).
  - Unit tests: k=1, k=2, duplicate points, points containing zero, polynomial of degree 0.
- **Rationale**: The single most valuable batch primitive (Chien search, Reed-Solomon decoding, FRI commitments). Separated from interpolation so the reviewer can focus on the tree-construction / modular-reduction correctness.

### Task 4 — Lagrange interpolation (quadratic + fast variant via tree)

- **Type**: task
- **Depends on**: Task 3
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**: `crates/gf2-core/src/field/poly.rs`
- **Size estimate**: 1.5–2 days
- **Success criteria**:
  - `interpolate(points: &[(F, F)]) -> FieldPoly<F>` using standard Lagrange (O(n²)); returns the zero polynomial for an empty input.
  - `interpolate_fast(points)` using the subproduct-tree-based algorithm (Geddes/von zur Gathen Ch. 10): build M(x) = Π(x − x_i), compute M'(x), evaluate M' at all x_i via `batch_evaluate`, then do one downward pass; O(n log² n).
  - Both routines must reuse `batch_ops::batch_inverse` for the `1 / Π_{j≠i}(x_i − x_j)` weights — this is the concrete motivation for the Montgomery-trick dependency and must be called out in the PR description.
  - Duplicate-x detection: if any two `x_i` coincide, return a clear error (`Result<FieldPoly<F>, InterpolationError>`) rather than panicking.
  - Round-trip proptest: `interpolate(points).evaluate(x_i) == y_i` for all i (mentioned verbatim in story spec).
  - Agreement proptest: `interpolate_fast` agrees with `interpolate` for n up to 32.
  - Threshold constant selecting naive vs fast, tuned by a bench case.
- **Rationale**: The natural dual of batch evaluation; both are needed to claim the story's "polynomial-level computations" remit is covered. One task because both variants share `batch_inverse` plumbing.

### Task 5 — Batch product tree + batch GCD

- **Type**: task
- **Depends on**: Task 2 (Task 3 would be nice but is not required)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**: `crates/gf2-core/src/field/poly.rs`
- **Size estimate**: 1 day
- **Success criteria**:
  - `batch_mul(polys: &[Self]) -> Self` using a balanced binary product tree (fold-by-pairs). Benchmark and show O(n log n · log k) behaviour versus left-fold schoolbook when polys are balanced.
  - `batch_gcd(polys: &[Self]) -> Self` via pairwise reduction — spec calls this out explicitly as "pairwise reduction" so the naive sequential `fold(gcd)` is acceptable; document the choice.
  - Proptest: `batch_mul(polys) == polys.iter().fold(one, |a,b| a*b)`.
  - Proptest: `batch_gcd([a*d, b*d, c*d])` divides `d`.
  - Empty-slice behaviour: `batch_mul([]) = FieldPoly::one_like(some_element)` returns the multiplicative identity on a caller-provided `&F` (unlike the batch-inverse empty case which returns `[]`, an empty product needs a field sample — add a `batch_mul_with_field(&F, &[Self])` variant).
- **Rationale**: Small but standalone. Kept off Task 3's critical path because the product-tree logic here is simpler (no division) and can ship independently.

### Task 6 — NTT-friendly field API (`TwoAdicField` trait + roots of unity)

- **Type**: task
- **Depends on**: — (independent of Tasks 1–5; can run in parallel with Task 1)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/mod.rs` (re-export)
  - `crates/gf2-core/src/field/two_adic.rs` (new)
  - `crates/gf2-core/src/gfp/specialized.rs` (impl for Proth `Fp<P>`)
- **Size estimate**: 1.5 days
- **Success criteria**:
  - New trait in `field/two_adic.rs`:
    ```rust
    pub trait TwoAdicField: FiniteField {
        /// The largest k such that 2^k divides |F^*|.
        const TWO_ADICITY: u32;
        /// A fixed generator of the 2^TWO_ADICITY-th roots of unity.
        fn two_adic_generator() -> Self;
        /// The 2^k-th root of unity for k ≤ TWO_ADICITY; panics otherwise.
        fn two_adic_root_of_unity(k: u32) -> Self {
            assert!(k <= Self::TWO_ADICITY);
            Self::two_adic_generator().pow(1u64 << (Self::TWO_ADICITY - k))
        }
    }
    ```
  - Impl for every supported Proth `Fp<P>` using `classify()` at const-context (const-generic or `const fn` helper in `specialized.rs`). `TWO_ADICITY` = `PrimeShape::Proth { n, .. }.n`.
  - A small look-up table of known primitive generators per prime (at minimum `Fp<65537>`, BabyBear, KoalaBear) — these are standard constants; cite the reference in comments.
  - Unit tests per supported prime: `g = two_adic_generator()` satisfies `g^(2^TWO_ADICITY) == 1` and `g^(2^(TWO_ADICITY-1)) != 1` (primitive check).
  - **Explicitly not impl for `Gf2mElement`**: the multiplicative group has odd order `2^m − 1`, so `TWO_ADICITY = 0`. Document this in the module prose; the NTT path in Task 7 is prime-field-only.
- **Rationale**: This is the missing plumbing the spec quietly assumed. The NTT task cannot land without it. Keeping it separate lets the trait design get proper review.

### Task 7 — NTT-based polynomial multiplication

- **Type**: task
- **Depends on**: Task 2 (for `FieldPoly` arithmetic), Task 6 (for `TwoAdicField`)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/poly.rs` (extend `mul` to dispatch)
  - `crates/gf2-core/src/field/ntt.rs` (new — pure NTT kernels)
  - `crates/gf2-core/benches/field_poly.rs` (extend)
- **Size estimate**: 2–3 days
- **Success criteria**:
  - `ntt_inplace<F: TwoAdicField>(&mut [F], inverse: bool)` — standard radix-2 decimation-in-time FFT, bit-reversal permutation, twiddles via `two_adic_root_of_unity`. Length must be a power of two and ≤ `2^F::TWO_ADICITY`.
  - `FieldPoly::<F>::mul_ntt(&self, other: &Self) -> Self` that pads to the next power of two, applies NTT, pointwise-multiplies, applies inverse NTT, scales by `n⁻¹` (reuses `batch_inverse`'s `F::inv`), and trims.
  - Dispatch in `FieldPoly::mul` based on *both* (a) a `TwoAdicField` impl (probed via a marker trait or a separate `mul_fast` method to avoid a coherence clash) and (b) a size threshold tuned by bench (expect n ≳ 128 for `Fp<65537>`).
  - Proptest: for `Fp<65537>`, `mul_ntt(a, b) == mul_karatsuba(a, b)` for random polys of degree up to 64.
  - Roundtrip proptest: `inverse_ntt(ntt(x)) == x`.
  - Bench entry showing NTT beating Karatsuba at some crossover point, documented in the module docstring.
  - `Gf2mElement` callers fall through to Karatsuba (NTT dispatch is a no-op for non-`TwoAdicField`).
- **Rationale**: The spec's headline "O(n log n) via NTT" deliverable. Kept separate because it's the largest algorithmic chunk and naturally belongs in its own file. The 2–3 day estimate is conservative — bit-reversal and twiddle management are classic subtle bugs.

### Task 8 — Integration docs + bench consolidation

- **Type**: task
- **Depends on**: Tasks 3, 4, 5, 7
- **Labels**: `type:task`, `component:gf2-core`, `component:gf2-coding`, `epic:e095a100`, `story:bdf95060`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/poly.rs` (module-level overview)
  - `crates/gf2-core/benches/field_poly.rs` (consolidated harness)
  - `crates/gf2-coding/src/bch/core.rs` (only a small `from_generic` helper; no rewrite)
  - `dev/plans/field_poly_module_overview.md` (new, short — overview table linking each new API to the algorithmic reference)
- **Size estimate**: 1 day
- **Success criteria**:
  - Top-of-module docstring in `field/poly.rs` enumerates all new operations with complexity, mirroring the table in `batch_ops.rs`.
  - Bench table is regenerated and pasted into the docstring for `batch_evaluate`, `interpolate_fast`, `mul_ntt`.
  - A narrow integration helper `pub(crate) fn gf2m_poly_to_field_poly(&Gf2mPoly) -> FieldPoly<Gf2mElement>` (and its inverse) lives in `gf2-core` and has a proptest-level roundtrip test. **No bch rewrite**; that stays out of scope.
  - `cargo test --workspace --all-features --release` ≤ 60 s (CLAUDE.md rule) — verify, and if exceeded, move large proptests to `proptest-cases < 64` or push them behind `#[ignore]` long-test flag.
  - All three gates (`code-review`, `doc-review`, `cargo-ci`) pass on the full epic.
- **Rationale**: Catches all the end-of-story housekeeping without starting a full bch refactor (which is correctly out of scope for this story).

## Dependency DAG

```
            ┌──────────┐
            │ Task 1   │  core type + basic arith
            └────┬─────┘
                 │
            ┌────▼─────┐
            │ Task 2   │  div_rem, gcd, Horner, Karatsuba
            └──┬───┬───┘
               │   │
         ┌─────┘   └──────────────────┐
         │                            │
    ┌────▼─────┐                 ┌────▼──────┐
    │ Task 3   │                 │ Task 5    │  batch_mul, batch_gcd
    │ subproduct-tree eval       │           │
    └────┬─────┘                 └────┬──────┘
         │                            │
    ┌────▼─────┐                      │
    │ Task 4   │  interpolate         │
    └────┬─────┘                      │
         │                            │
         │    ┌──────────┐            │
         │    │ Task 6   │  TwoAdicField trait
         │    └────┬─────┘            │
         │         │                  │
         │    ┌────▼─────┐            │
         │    │ Task 7   │  NTT mul   │
         │    └────┬─────┘            │
         │         │                  │
         └─────────┴───────┬──────────┘
                           │
                      ┌────▼─────┐
                      │ Task 8   │  docs + bench + small integration
                      └──────────┘
```

Parallel opportunities: **Task 1 and Task 6 can start simultaneously** (no shared files). Tasks 3/4/5 can run in parallel after Task 2 lands. Task 7 unblocks only once both 2 and 6 are in.

## Out of scope / deferred

- **Rewriting `Gf2mPoly` to delegate to `FieldPoly<Gf2mElement>`.** The spec suggests this is possible but also notes "remain specialized for performance". Performing the rewrite requires re-verifying every BCH/DVB-T2 test vector and is a multi-day ripple. Defer to a dedicated follow-up story once `FieldPoly` has stabilised.
- **Rewriting `gf2-coding::bch` to use generic polynomial operations.** Same reasoning — touches the standards-compliance test surface.
- **SIMD acceleration of polynomial operations.** The `simd` feature in `gf2-core` is driven by `gf2-kernels-simd`; SIMD polynomial multiplication (e.g., PCLMULQDQ for GF(2^m) polys, or AVX-512 IFMA for prime-field NTT) is a separate optimisation epic.
- **Parallel (rayon) polynomial operations.** The `parallel` feature exists but every task above uses sequential kernels. `par_batch_evaluate` etc. are natural follow-ups.
- **Multi-word / `u128`-backed fields in the NTT path.** The `TwoAdicField` impl covers `Fp<P>` with `P ≤ 2^63`; wider Proth primes (Goldilocks's 2-adicity is 32 but its magnitude > `Fp`'s bound) are deferred until after 6fb4abad lands multi-word support.
- **Matrix-based batch multipoint evaluation (BabyStepGiantStep / transposed Vandermonde).** Subproduct tree is already O(n log² n); the deeper variants are research territory out of scope for the 1–2 week story.
- **FFT over GF(2^m) (additive FFT / Cantor basis).** Non-trivial and only marginally useful for the current BCH workloads. Kept for a future story.

## Risks and open questions

1. **Story-label creation.** `story:bdf95060` does not yet exist as a label value (confirmed via `jit label values story`). The lead should add it before dispatching Task 1 so the child issues wire correctly into the graph.
2. **Empty-polynomial convention mismatch.** `Gf2mPoly::eval` currently panics on an empty polynomial; the story implies zero-returns-zero. Task 2 must pick one convention and *document* it prominently — I recommend "zero polynomial evaluates to zero" because it composes better with the subproduct tree (a leaf `x − x_i` minus a `x − x_i` is zero but we still want to evaluate the quotient). This is worth a 5-minute lead-level call before Task 2 starts.
3. **NTT dispatch mechanics.** Rust's coherence rules make "use NTT if `F: TwoAdicField`, else Karatsuba" awkward. Task 7 may end up exposing `mul_fast` as a separate method and having `mul` pick Karatsuba unconditionally, or gating via a separate `FastMul` marker trait. Both are acceptable; flag early.
4. **Test budget.** Tasks 3, 4, and 7 each add heavy proptests. Cap proptest cases at 32–64 per test in the generic bodies; keep deeper cases behind `--ignored` long-run tests. If the 60-second workspace limit is breached, Task 8 must cut back.
5. **Threshold tuning without committing bench artefacts.** Subproduct-tree, fast Lagrange, and NTT each need a size threshold. Bake sensible defaults (16 / 32 / 128) and document that tuning is follow-up work; don't block the story on bench-driven tuning of every constant.
6. **`Fp<P>`'s Proth detection is const but the prime is a const-generic.** `TWO_ADICITY` in Task 6 needs to be derivable from `P` at compile time. If `const fn` in trait-const position hits a nightly-only path on current MSRV 1.80, fall back to a `const` on a helper `struct ProthConst<const P: u64>` and document the workaround.
