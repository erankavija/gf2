# 6fb4abad — Multi-word GF(2^m) for m > 128 (breakdown)

**Parent story**: 6fb4abad — Support multi-word GF(2^m) for field degrees m > 128
**Target**: 4–8 task issues, each 1–3 days of implementation effort.

## Scope summary

The existing `Gf2mElement_<V: UintExt>` family (`crates/gf2-core/src/gf2m/field.rs:152–209`, `crates/gf2-core/src/gf2m/uint_ext.rs:31–94`) already covers `m ≤ 127` via `V = u128` (delivered by c488ed29). Its runtime `Arc<FieldParams_<V>>` design means:

- It does **not** implement `ConstField` — only `FiniteField` (see `crates/gf2-core/src/gf2m/field.rs:1253`). Confirmed by grepping all `impl ConstField` sites: only `Fp<P>`, `GoldilocksFp`, `QuadraticExt<C>`, `CubicExt<C>` today.
- `BarrettReducer` deliberately caps at `degree ≤ 63` (`crates/gf2-core/src/gf2m/barrett.rs:15–31`). It is not a drop-in for multi-word moduli.

This story introduces a **new** parallel type `Gf2mWide<N, Cfg>` (const-generic over the word count, generic over a config trait) living in `crates/gf2-core/src/gf2m/wide.rs`. Focal case is GF(2^256) (`N = 4`). The design must also admit `N = 8` (GF(2^512)) without any `N`-specific special-casing.

**In scope** for the child tasks below:

- The `Gf2mWide<const N: usize, Cfg: Gf2mWideConfig<N>>` type, `Copy`, stack-allocated.
- A trait-based parameter surface (`Gf2mWideConfig<N>`), mirroring `ExtConfig` in `crates/gf2-core/src/gfpn/ext_config.rs:57` — this is the lever that makes `Copy` and `ConstField` achievable.
- Schoolbook GF(2)-polynomial multiplication of two `[u64; N]` operands into a `[u64; 2*N]` raw product.
- A multi-word Barrett reducer `BarrettReducerWide<N>` that takes a `[u64; 2*N]` and returns `[u64; N]`, implemented in pure scalar Rust (no `unsafe`).
- `FiniteField` + `ConstField` trait impls, including `Neg`, `Sub`, `AddAssign`, div via `inv()`, and the wide accumulator (trivially `Wide = Self` for binary fields).
- At least one concrete `Gf2mWideConfig<4>` using a documented low-weight irreducible polynomial for GF(2^256).
- Full axiom-harness coverage via `test_const_field_axioms` (`crates/gf2-core/src/field/axiom_tests.rs:192`).

**Explicitly deferred** to follow-up stories (not to be done in this breakdown):

- Heap-backed `Gf2mBig` (Option B in the spec). Out of scope.
- A general primitive-polynomial catalogue for `m > 128`. The child tasks add exactly the polynomials they need (one per target `N`), not a sweep.
- `Gf2mWideConfig<8>` / `<16>`: design must support them but only a smoke test is required here. Full axiom coverage at `N = 8` is a stretch goal per Task 5.
- VPCLMULQDQ SIMD kernels in `gf2-kernels-simd`: covered as one opt-in task (Task 6). Not required for the correctness success criterion; required only for the "within 2× of handwritten SIMD" performance criterion — and only if the scalar schoolbook + Barrett path fails that bar.
- Karatsuba for `N = 4` is a *conditional* extension (Task 7) — added only if bench evidence shows schoolbook is the bottleneck.
- `Gf2mField_<V>` → `Gf2mWide` bridge APIs or inter-conversion: out of scope. The two types are independent.
- Extending `BarrettReducer` (the existing `u128`-based one) in place. That type stays as-is; the new `BarrettReducerWide<N>` is a sibling.

## Label conventions for this breakdown

All child tasks carry:

- `type:task`
- `component:gf2-core`
- `epic:e095a100`
- `story:6fb4abad` (**does not yet exist** under the `story` namespace — confirmed via `jit label values story`; lead must create it on first use)

Task 6 additionally carries `component:gf2-kernels-simd`.

## Proposed task DAG

### Task 1 — `Gf2mWideConfig<N>` trait + `Gf2mWide<N, Cfg>` type shell

- **Type**: task
- **Depends on**: — (parent deps c488ed29 + 2248b17d already done)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**:
  - `crates/gf2-core/src/gf2m/wide.rs` (new)
  - `crates/gf2-core/src/gf2m/wide_config.rs` (new; config trait)
  - `crates/gf2-core/src/gf2m/mod.rs` (add `pub mod wide;` + re-exports)
- **Size estimate**: 1 day
- **Success criteria**:
  - Public `trait Gf2mWideConfig<const N: usize>: 'static` with:
    - `const M: usize` — extension degree (must satisfy `64*(N-1) < M <= 64*N`).
    - `const MODULUS: [u64; N]` — the *low-order* `M` bits of the irreducible polynomial (bit `M` implicit = 1, matching the existing `Gf2mField_::new` convention in `crates/gf2-core/src/gf2m/field.rs:249`).
    - `const MODULUS_HIGH_BIT_WORD: usize` / `MODULUS_HIGH_BIT_MASK: u64` — cached `(M>>6, 1u64 << (M & 63))` for fast reduction (or made derivable via a default `fn`).
    - Optional `const NAME: &'static str` for `Debug`.
  - `pub struct Gf2mWide<const N: usize, Cfg: Gf2mWideConfig<N>> { words: [u64; N], _marker: PhantomData<Cfg> }` with `Copy`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Debug`.
  - Constructors: `pub const fn from_words(words: [u64; N]) -> Self` (debug-asserts the high-bit tail is masked), `new(words: [u64; N])` (masks tail), `zero()`, `one()`, `from_u64(u64)`.
  - Accessors: `words() -> &[u64; N]`, `bit(usize) -> bool`, `is_zero(&self) -> bool`, `is_one(&self) -> bool`.
  - `Add`, `Sub`, `Neg`, `AddAssign` (owned + `&`-ref RHS, all five variants matching the existing `Gf2mElement_<V>` boilerplate around `crates/gf2-core/src/gf2m/field.rs:1014–1249`) using plain word-wise XOR. `Neg` is identity (char 2). `Sub == Add`.
  - A `mask_tail_in_place(&mut [u64; N])` private helper that zeroes bits ≥ `M` in the top word; document it as the multi-word analogue of the project-wide tail-masking invariant (CLAUDE.md "Tail masking").
  - A minimal test `Gf2m256TestConfig` referencing a documented Seroussi pentanomial for `M = 256` (pick one irreducible polynomial from *HPL-98-135* Table 1 and cite the exact row in the docstring — e.g. `x^256 + x^10 + x^5 + x^2 + 1`; the implementer verifies irreducibility locally with `Gf2mField::verify_irreducible` if callable, otherwise via a fresh `cargo test` that factors the polynomial in a sage cross-check noted in the PR body).
  - Unit tests (no proptest yet): addition commutativity, `zero() + a == a`, `a - a == zero()`, tail masking zeroes bits above `M`.
- **Rationale**: Gets the type, the config trait, and the cheap (XOR-only) operator scaffolding landed. Multiplication, reduction, and the full axiom run come next. Matching the `ExtConfig`/`QuadraticExt` pattern keeps the mental model consistent with `gfpn`.

### Task 2 — Multi-word schoolbook carry-less multiplication

- **Type**: task
- **Depends on**: Task 1
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**:
  - `crates/gf2-core/src/gf2m/wide.rs` (add `clmul_wide` helper + `wide_mul_raw`)
  - Unit tests colocated in the same file under `#[cfg(test)]`.
- **Size estimate**: 1–1.5 days
- **Success criteria**:
  - `fn clmul_wide<const N: usize>(a: &[u64; N], b: &[u64; N]) -> [u64; 2*N]` — GF(2) polynomial multiplication of two multi-word operands into a double-width raw product. Pure safe Rust; inner word-word clmul uses the scalar `clmul(u64, u64) -> u128` already in `crates/gf2-core/src/gf2m/barrett.rs:59` (or a local copy to keep module boundaries clean). No `unsafe`. No SIMD.
  - Const-generic over `N`; compiles for any `N` (no special-casing `N=4`).
  - Uses the standard `O(N^2)` schoolbook shape: for each `(i, j)` the 128-bit sub-product `a[i] * b[j]` is accumulated (XOR) into `out[i+j]` (low 64) and `out[i+j+1]` (high 64). Documented complexity and op count in the doc comment.
  - **Rust stable does not support `[u64; 2*N]` return with `const N: usize` directly** — confirm this against the MSRV Rust 1.80. If blocked, use one of:
    1. A `const M: usize` second parameter equal to `2*N`, enforced at call-sites via a `const _: () = assert!(M == 2*N);`.
    2. A `Gf2mDoubleWide<N>` wrapper type with `[u64; 16]` storage for `N=4` plus a `cap` field, or a const-generic alias crate like `typenum`.
    3. Return via an out-parameter `&mut [u64; 2*N]`.
    The task picks (1) or (3) — no new dependencies. Document the choice in the module-level prose.
  - Unit tests using known small vectors: `(1) * (1) = 1`, `(x) * (x) = x^2`, `(x + 1) * (x + 1) = x^2 + 1`, and `all-ones * all-ones = 0x555...5` (carry-less square).
  - A cross-check test for `N = 2` (i.e., GF(2^128)-sized operands, 128-bit each) where the result is compared bit-for-bit against a `u128`-based reference implementation built on `barrett::clmul`. This is the correctness anchor; no dedicated GF(2^128) field needs to be instantiated.
  - Proptest (≤128 cases): `clmul_wide(a, b) == clmul_wide(b, a)` (commutativity of GF(2) polynomial mul) for random `[u64; 4]` inputs.
- **Rationale**: Pure carry-less multiplication is the algorithmic core and is testable without any reduction logic. Separating it from the Barrett reducer task makes the correctness of each component independently reviewable.

### Task 3 — `BarrettReducerWide<N>` scalar reducer

- **Type**: task
- **Depends on**: Task 1 (needs `Gf2mWideConfig`), Task 2 (not strictly — Task 3 can start in parallel if the developer fakes the raw product via schoolbook locally, but the DAG keeps it sequential to avoid merge churn).
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**:
  - `crates/gf2-core/src/gf2m/barrett.rs` (add `BarrettReducerWide<const N: usize>` alongside the existing `BarrettReducer` — do not modify the existing type).
- **Size estimate**: 2 days
- **Success criteria**:
  - `pub struct BarrettReducerWide<const N: usize> { mu: [u64; N+1], modulus: [u64; N], m: u32 }` (same signed-`+1` caveat as Task 2 for the `mu` length). Precomputed in a `const fn new([u64; N], u32)` where possible, otherwise a plain `fn new`.
  - `fn reduce(&self, product: &[u64; 2*N]) -> [u64; N]` implementing the standard Barrett reduction over GF(2) polynomials: `q1 = product >> (m-1)`, `q2 = q1 * mu`, `q3 = q2 >> (m+1)`, `r = product ^ (q3 * modulus)`, final mask to `m` bits. All shifts/multiplies are over GF(2) polynomials and call into a `clmul_wide`-class helper (or a narrower variant for the ≤ `N+1` word intermediates).
  - Correctness cross-check test: for `N = 1` and `m = 63`, `BarrettReducerWide::<1>::new([modulus_lo], 63).reduce(&[lo, hi])` must equal the existing `BarrettReducer` output on the same operands (reuse the existing reducer's tests as oracle data).
  - Correctness cross-check for `N = 2, m = 127`: reduction result must match a reference implementation that does naive `O(m)` shift-and-XOR reduction (write a tiny `reference_reduce_wide<N>` in the test module). Random-input proptest, ≤100 cases.
  - Correctness test for `N = 4, m = 256`: reference-vs-Barrett agreement on 100 random products. Use the module-level `CASES_PER_AXIOM` budget as a ceiling and keep individual tests under 5s each (the 60s total-suite budget is the hard constraint; see CLAUDE.md "Performance rules").
  - Doc comment explicitly calls out that this is the **multi-word generalisation deferred** by `barrett.rs:21–31` and links back to that note; also remove / amend that existing comment's "deliberately deferred" wording once the new reducer lands (change is scoped to barrett.rs).
- **Rationale**: Reduction is the hardest correctness hot-spot and benefits from a dedicated review + property-test budget. A scalar-only implementation means Task 6 (SIMD) becomes purely a performance optimisation over an already-correct baseline.

### Task 4 — `Gf2mWide` multiplication, inversion, `FiniteField` + `ConstField` impls

- **Type**: task
- **Depends on**: Task 2, Task 3
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**: `crates/gf2-core/src/gf2m/wide.rs`
- **Size estimate**: 1.5 days
- **Success criteria**:
  - `impl<const N: usize, Cfg: Gf2mWideConfig<N>> Mul for Gf2mWide<N, Cfg>` composing Task 2's `clmul_wide` + Task 3's `BarrettReducerWide<N>`. Also `Mul<&Self>`, matching the existing five-variant boilerplate (`crates/gf2-core/src/gf2m/field.rs:1215`).
  - The `BarrettReducerWide<N>` instance is materialised at `impl` time from the `Cfg` constants. If runtime construction is needed because of `const fn` limits, it can be lazily built via `OnceLock<BarrettReducerWide<N>>` keyed on `TypeId::of::<Cfg>()`; fast path still const.
  - `fn inverse(&self) -> Option<Self>` via Fermat's little theorem: `self.pow(2^M - 2)` (since `GF(2^M)*` has order `2^M - 1`). Exponent is a 256-bit number for `N = 4` — use a bit-by-bit square-and-multiply loop over `self.words()` rather than materialising the exponent as an integer. Document the alternative (extended Euclidean over polynomials) and note why Fermat is chosen (simpler, still O(M) multiplications, acceptable for the "correctness first" story scope).
  - `impl FiniteField for Gf2mWide<N, Cfg>`:
    - `Characteristic = u64`, returns 2.
    - `Wide = Self`, `to_wide == clone`, `reduce_wide == identity`, `max_unreduced_additions() = usize::MAX` (mirrors the existing `Gf2mElement_` impl at `crates/gf2-core/src/gf2m/field.rs:1253`).
    - `extension_degree()` returns `Cfg::M`.
    - `zero_like`, `one_like`, `inv`, `is_zero`, `is_one` all trivial.
  - `impl ConstField for Gf2mWide<N, Cfg>` (the new capability over `Gf2mElement_`):
    - `zero()`, `one()`, `order() -> u128` returning `1u128 << Cfg::M` for `M ≤ 127`; for `M ≥ 128` **panics** with an explicit "order exceeds u128" message. Document this limitation in the trait doc comment and note that the `order()` return type is a `FiniteField`-trait constraint (`crates/gf2-core/src/field/traits.rs:272`). Flag this as a design question to raise with the project lead if an extended-precision `FieldOrder` return type is wanted; for now, the panic is acceptable because no current caller invokes `order()` on fields larger than `u128`.
    - **Open question / risk**: `test_const_field_axioms` (`crates/gf2-core/src/field/axiom_tests.rs:206`) calls `F::order()` and asserts `p^m == expected_order`. For `M = 256` this assertion will overflow `u128::pow`. Task 5 must gate `Gf2mWide<4>` through `test_field_axioms` (the non-const variant) rather than `test_const_field_axioms`, OR the axiom harness must be widened. The simpler path — use `test_field_axioms` and add a *separate* unit test covering `zero()`/`one()`/`order()` with an explicit `#[should_panic]` for the order-of-large-M case — is recommended. Document both options in the PR.
  - `Display` + pretty `Debug`: print `GF(2^M):0x…` with the hex rendering of `words` in little-endian-limb big-endian-byte order; one line each.
  - Doc examples in every public item (tested by `cargo test --doc`), using `Gf2m256TestConfig` from Task 1.
- **Rationale**: This is the story's structural payload: the first `ConstField`-implementing GF(2^m) type in the crate. The `order()` tension with `u128` is the single design-level decision and is worth one explicit bullet above.

### Task 5 — Axiom-harness coverage for `Gf2mWide<4>`

- **Type**: task
- **Depends on**: Task 4
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**:
  - `crates/gf2-core/src/field/axiom_tests.rs` (add `gf2m_wide_strategy` + `#[test] fn test_axioms_gf2m_wide_256`).
- **Size estimate**: 0.5 days
- **Success criteria**:
  - Public `fn gf2m_wide_strategy<const N: usize, Cfg: Gf2mWideConfig<N>>() -> BoxedStrategy<Gf2mWide<N, Cfg>>` mirroring `gf2m_u128_strategy` (`crates/gf2-core/src/field/axiom_tests.rs:50`). Generates 2N random `u64`s, packs them as `[u64; N]`, calls `Gf2mWide::new` (which tail-masks to `Cfg::M`).
  - `#[test] fn test_axioms_gf2m_wide_256()` calls `test_field_axioms(gf2m_wide_strategy::<4, Gf2m256TestConfig>(), 2)` (not `test_const_field_axioms` — see Task 4 open question).
  - Also add a narrow `#[test] fn test_const_field_zero_one_gf2m_wide_256()` asserting `Gf2mWide::<4, Gf2m256TestConfig>::zero().is_zero()` and `one().is_one()`, skipping the `order()` assertion.
  - The axiom test must complete within 5 seconds wall-clock in release mode. If it exceeds that budget, the fix is to drop `CASES_PER_AXIOM` locally: pass an explicit `ProptestConfig::with_cases(100)` via a new overload (or marker constant) rather than reusing the module's `1000` default. Document the choice; 100 cases per axiom × ~18 axioms × ~hundred-microsecond per GF(2^256) mul is ≈ 200 ms of wall clock, comfortably inside the budget.
  - A separate **ignored** stress test `#[test] #[ignore] fn test_axioms_gf2m_wide_256_stress()` running at the full 1000 cases for manual invocation (`cargo test --release -- --ignored`).
  - `test_const_field_axioms` itself is **not** modified — Task 4 documents the `order()` limitation, and the test added here uses the simpler `test_field_axioms` entry point.
  - Stretch / documented-in-PR-body only (no new blocking criterion): add a disabled `test_axioms_gf2m_wide_512` behind `#[cfg(feature = "slow-tests")]` or `#[ignore]` to smoke-test `N = 8` without adding ≈ 4× more wall-clock to the default run.
- **Rationale**: Closes the story's first success criterion ("`Gf2mWide<4>` passes the full field axiom test harness") with a tight, focused task. The 60 s full-suite budget (CLAUDE.md) is the hard constraint governing proptest case counts here.

### Task 6 — VPCLMULQDQ SIMD kernel for GF(2^256) (opt-in, performance)

- **Type**: task
- **Depends on**: Task 4, Task 5 (need a correct scalar baseline to bench against and to diff behaviour during development)
- **Labels**: `type:task`, `component:gf2-core`, `component:gf2-kernels-simd`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**:
  - `crates/gf2-kernels-simd/src/gf2m_wide.rs` (new) — `unsafe` VPCLMULQDQ-based schoolbook for `N = 4`, plus scalar PCLMULQDQ fallback for hosts without VPCLMULQDQ.
  - `crates/gf2-kernels-simd/src/lib.rs` (public re-export).
  - `crates/gf2-core/src/gf2m/wide.rs` — add feature-gated fast-path dispatch to `simd_gf2m_wide::clmul_wide_256` inside the `Mul` impl, following the `OnceLock`+`detect()` pattern already used at `crates/gf2-core/src/lib.rs` for `simd::maybe_simd()`.
  - A new criterion benchmark `crates/gf2-core/benches/gf2m_wide_mul.rs` comparing scalar vs SIMD.
- **Size estimate**: 2–2.5 days
- **Success criteria**:
  - `pub type ClmulWide256Fn = unsafe fn(&[u64; 4], &[u64; 4], &mut [u64; 8])` detecting at runtime and using VPCLMULQDQ (256-bit lane). `avx512vl + vpclmulqdq` path lands first (the repo's existing `clmul_batch_vpclmul` at `crates/gf2-kernels-simd/src/x86/clmul.rs:130` is the reference shape). Scalar PCLMULQDQ fallback covers Zen 2 / Ivy Bridge.
  - CPU feature detection + dispatch live in `gf2-kernels-simd::gf2m_wide::detect() -> Option<ClmulWide256Fn>`, parallel to the existing `gf2m::detect()` in `crates/gf2-kernels-simd/src/gf2m.rs:78`.
  - All `unsafe` lives in `gf2-kernels-simd`; `gf2-core` stays under `#![deny(unsafe_code)]` (CLAUDE.md "Unsafe isolation" invariant).
  - A `#[test]` in `wide.rs` asserts scalar and SIMD paths agree on 100 random inputs (when SIMD is detected; `#[ignore]`'d otherwise).
  - Criterion bench reports scalar-vs-SIMD throughput. PR body must record the measured ratio. Success = scalar path within 2× of the VPCLMULQDQ path (the story's performance target); if scalar is *already* within 2×, this task's SIMD work is marked as optional speedup, not required for story completion.
  - If no PCLMULQDQ is present the `detect()` returns `None` and the scalar path is used unconditionally.
- **Rationale**: Sole SIMD work in this story; strictly additive. Only becomes required if the scalar baseline fails the "within 2×" bar from the story's success criteria. Keeping it as a separate, late task means story completion can be claimed with scalar-only if the bench shows the scalar path is already competitive.

### Task 7 — Conditional: Karatsuba for `N = 4` (performance only)

- **Type**: task
- **Depends on**: Task 6 (needs bench data)
- **Labels**: `type:task`, `component:gf2-core`, `epic:e095a100`, `story:6fb4abad`
- **Files (planned scope)**: `crates/gf2-core/src/gf2m/wide.rs`
- **Size estimate**: 1 day (only if opened)
- **Success criteria**:
  - **Entry condition**: Task 6's benchmark shows that the schoolbook path (SIMD or scalar) is not the limiting factor — i.e., reduction is not dominant and mul is the bottleneck. If schoolbook is already within 2× of hand-written SIMD, this task is **closed without implementation**.
  - If opened: one-level Karatsuba for `N = 4` (3 half-size products instead of 4), implemented as pure safe Rust over `[u64; 2]` halves. Cross-checked for bit-exact equality with `clmul_wide::<4>` on ≥ 10 000 proptest cases.
  - Selection between schoolbook and Karatsuba hidden behind a `const fn` that picks one at compile time per `N`; no runtime switching.
  - Benchmark delta vs Task 6 baseline recorded in the PR body.
- **Rationale**: Explicitly conditional and last. Listed here so the project lead can defer it cleanly if bench evidence says schoolbook is enough.

## Dependency DAG

```
     Task 1 (Gf2mWide type + config trait)
        |
        +--> Task 2 (schoolbook clmul_wide)
        |         |
        +--> Task 3 (BarrettReducerWide)
                  |
                  +--> Task 4 (Mul/Inv, FiniteField + ConstField impls)
                             |
                             +--> Task 5 (axiom-harness test for GF(2^256))
                             |
                             +--> Task 6 (SIMD kernel) [performance]
                                      |
                                      +--> Task 7 (Karatsuba) [conditional]
```

Tasks 2 and 3 can run in parallel after Task 1 if two implementers are available; Task 3 can mock its raw-product input with a local naive schoolbook to de-risk scheduling. Tasks 5 and 6 can run in parallel after Task 4 (correctness test vs SIMD kernel are independent work streams).

## Out of scope / deferred

- **Heap-backed `Gf2mBig`** (Option B). The story recommends this as a follow-up; no task here.
- **General catalogue of irreducible polynomials for `m > 128`.** Tasks 1 and 4 ship exactly one concrete config (`Gf2m256TestConfig`). A systematic sweep is a separate follow-up story whose first task should also reconsider whether `verify_irreducible` (currently only exposed for `u64` storage in `crates/gf2-core/src/gf2m/field.rs`) should be widened to multi-word operands.
- **Extending `verify_primitive`/`verify_irreducible` to `Gf2mWide`.** Ties into the polynomial-catalogue work above; not blocking this story.
- **`Gf2mWideConfig<8>` / `<16>`** filled-in coverage. The architecture supports them; one smoke test covers `N = 8`, but a production-grade `N = 8` field (axiom-harness coverage, polynomial choice, benches) is left to a follow-up.
- **LFSR / primitive-element operations** for multi-word fields. These require primitivity (not just irreducibility) and the polynomial database does not currently guarantee it for `m > 63`.
- **`ConstField::order()` for `M > 127`.** Task 4 panics on this path; a proper fix requires changing the `FiniteField::order()` return type signature (currently `u128` in `crates/gf2-core/src/field/traits.rs:272`) and is a cross-cutting change best handled in its own story.
- **Any change to `Gf2mElement_<V>` or the existing `BarrettReducer`.** The new type is additive; no ripples into `gf2-coding` (BCH/Reed-Solomon continue to use `Gf2mElement_<u64>`).

## Risks and open questions

1. **`ConstField::order()` at `M = 256` overflows `u128`.** See Task 4 open question. Mitigation: use `test_field_axioms` (not `test_const_field_axioms`) in Task 5 and add a focused `#[should_panic]`-or-`None`-returning test. Full fix requires API evolution, out of scope.
2. **Rust stable const-generic arithmetic `[u64; 2*N]`.** MSRV is 1.80 (CLAUDE.md); `generic_const_exprs` is not stable. Task 2 enumerates three workarounds; Task 3 uses the same trick for `[u64; N+1]`. If this turns out more painful than expected, an alternative is **N-specific specialisation via macro + a `trait DoubleWide { type Output; }`**, which is stable but uglier. The developer should pick and document the choice at the start of Task 2 so Task 3 can inherit.
3. **Polynomial-correctness due diligence.** The chosen GF(2^256) polynomial must actually be irreducible. Seroussi's HPL-98-135 is explicitly a table of *irreducibles*, so citing a specific row (with degree and weight) is sufficient; absent that, running `Gf2mField::verify_irreducible` (once widened, or via an ad-hoc proof-of-irreducibility test) is a blocker. Mitigation: pick a documented entry in Task 1 from a named source; add a one-off `#[test]` that factors the polynomial over GF(2) by trial division up to degree `M/2 = 128` — this is expensive (~2 s at `M = 256` with naive trial) but runs once. Wrap in `#[ignore]` if it threatens the 60 s budget.
4. **Test budget.** Full-suite budget is 60 s (CLAUDE.md). Each axiom run at 1000 cases with GF(2^256) mul at ~1 μs = ~20 ms per axiom, ~360 ms per field; comfortable. Scale sub-linearly if the mul is slower pre-SIMD. Stress tests at `N = 8` belong behind `#[ignore]`.
5. **SIMD kernel correctness on non-Zen 3 hosts.** CI may not have VPCLMULQDQ. Mitigation follows the existing pattern at `crates/gf2-kernels-simd/src/x86/clmul.rs:98–120`: runtime detection, scalar fallback, feature-gated fast path. The equivalence test in Task 6 self-skips when no SIMD is detected.
6. **`BarrettReducerWide::new` as `const fn`.** The constants are computable statically, but `[u64; N+1]` + `const fn` + const loops may strain stable Rust 1.80. If `const fn` is blocked, fall back to a `OnceLock<BarrettReducerWide<N>>` inside the `Mul` impl (keyed per-`Cfg` via `TypeId`). Only trivial runtime cost on first use.
7. **Polynomial-catalog drift**. `primitive_polys.rs` (`crates/gf2-core/src/primitive_polys.rs`) has an established style: enum, `standard`, `standard_u128`, per-range irreducibility notes. When the polynomial catalogue eventually expands beyond the single focal case, the new `standard_wide<N>` accessor must carry the same strength-of-guarantee contract (irreducible-only for most `m > 128`, primitivity **not** asserted without a widened `verify_primitive`).
