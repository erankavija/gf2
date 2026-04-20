# `gf2-core::field` polynomial and transform module overview

This document is the reader-facing guide to the polynomial and fast-transform
surface in `crates/gf2-core/src/field/`. It is **not** API reference — the
rustdoc on each module and item is the authoritative source — but a map
from "which algorithm do I need?" to the file, function, and threshold that
implements it.

The module surface was landed across story `bdf95060` ("batch polynomial
operations for extension fields"). The close-out task `224a7d9e` added this
document plus the consolidated bench harness and the complexity-reference
table in `field/poly.rs`'s module docstring.

## High-level map

| File (`crates/gf2-core/src/field/…`) | What lives here |
|--------------------------------------|-----------------|
| `mod.rs` | Module-tree declarations and the `pub use` re-exports that form the crate-root polynomial surface. |
| `traits.rs` | The `FiniteField`, `ConstField`, and `FiniteFieldExt` trait hierarchy that parameterises every polynomial routine. |
| `two_adic.rs` | The `TwoAdicField` trait — opt-in certification that a field carries primitive `2^k`-th roots of unity, enabling radix-2 NTT. |
| `batch_ops.rs` | Montgomery's batch-inversion trick: `batch_inverse`, `batch_inverse_in_place`, skip-zero variants, caller-scratch variant. Powers every barycentric-weight pass in `poly_interpolate.rs`. |
| `poly.rs` | `FieldPoly<F>` — the single-source-of-truth polynomial type. Construction, query, operator overloads, schoolbook/Karatsuba multiplication, Euclidean division, GCD, evaluation (single + batch), subproduct-tree construction, and NTT multiplication (`mul_ntt`) for `TwoAdicField`. |
| `poly_interpolate.rs` | Lagrange interpolation: `interpolate` (O(n²) barycentric), `interpolate_fast` (subproduct-tree, generic), `interpolate_fast_auto` (subproduct-tree, `TwoAdicField`-specialised middle step via `batch_evaluate_auto`), `interpolate_auto` (threshold-tuned dispatcher, generic), `interpolate_auto_two_adic` (threshold-tuned dispatcher, `TwoAdicField`), `formal_derivative`, plus the `InterpolationError` enum. |
| `ntt.rs` | Radix-2 decimation-in-time NTT primitive `ntt_inplace` over any `TwoAdicField`. The low-level kernel that `FieldPoly::mul_ntt` and `mul_fast` sit on top of. |
| `vec.rs` | `FieldVec<F>` dense vector, strided iterator helpers. Used by BCH / LDPC call sites but not central to the polynomial algorithms. |
| `axiom_tests.rs` | The `FiniteField` axiom test harness; invoked from each field implementation's test module and kept compiled under `cfg(test)` / `test-support`. |

## Algorithm → file → public API map

For each polynomial operation, the table gives the algorithm name,
implementing file and approximate line range (taken against commit
`3d98431`, i.e. before this document's own docstring expansion), and the
cost in terms of field operations. Lines shift over time — treat them as
approximate landmarks; rely on rustdoc (`cargo doc --no-deps -p gf2-core`)
for authoritative location.

| Operation | Algorithm | File : line range | Complexity |
|-----------|-----------|-------------------|------------|
| `FieldPoly::new` / `from_coeffs_trimmed` | Linear scan + trailing-zero trim | `poly.rs` : 240 – 280 | `O(n)` |
| `FieldPoly::zero_like` / `one_like` / `constant` | Direct construction | `poly.rs` : 319 – 420 | `O(1)` |
| `FieldPoly::monomial` | Fill + normalise | `poly.rs` : 422 – 455 | `O(degree)` |
| `FieldPoly::from_roots` | Left-fold of linear factors | `poly.rs` : 1040 – 1080 | `O(k²)` |
| `FieldPoly::product` | Balanced tree of multiplications | `poly.rs` : 1087 – 1160 | `O(k · M(k))` |
| `Add` / `Sub` / `Neg` / `AddAssign` / `SubAssign` | Elementwise | `poly.rs` : 1620 – 1740 | `O(max(n, m))` |
| `FieldPoly::mul_scalar` / `scale` | Elementwise scale | `poly.rs` : 770 – 860 | `O(n)` |
| `FieldPoly::mul` and `impl Mul` (owned / borrowed) | Schoolbook ⇄ Karatsuba dispatch | `poly.rs` : 733 – 768, 2035 – 2200 | schoolbook `O(n · m)`, Karatsuba `O(n^{log₂ 3})` |
| `FieldPoly::mul_ntt` | Radix-2 NTT convolution (requires `TwoAdicField`) | `poly.rs` : 2271 – 2370 | `O(N log N)` with `N = next_pow2(n + m − 1)` |
| Free function `mul_fast` | Tuned dispatcher (Karatsuba ⇄ NTT) | `poly.rs` : 2373 – 2400 | winning arm for each size |
| `FieldPoly::div_rem` | Schoolbook long division | `poly.rs` : 1364 – 1455 | `O(n · m)` |
| `FieldPoly::gcd` | Euclidean algorithm over `div_rem` | `poly.rs` : 1457 – 1550 | `O(n · m · log min(n, m))` |
| `FieldPoly::eval` | Horner | `poly.rs` : 857 – 900 | `O(n)` |
| `FieldPoly::eval_batch` | `k` Horner folds | `poly.rs` : 902 – 985 | `O(n · k)` |
| `FieldPoly::batch_evaluate` | Auto-dispatch: naive Horner ⇄ schoolbook-`div_rem` subproduct tree | `poly.rs` : 988 – 1040 | `O(n · k)` below `SUBPRODUCT_THRESHOLD`, `O(n · k + k² log k)` above |
| `FieldPoly::batch_evaluate_auto` (for `F: TwoAdicField`) | Auto-dispatch: naive Horner ⇄ `div_rem_auto`-backed subproduct tree | `poly.rs` : in the `impl<F: TwoAdicField>` block | `O(n · k)` below `SUBPRODUCT_THRESHOLD`, `O(M(n) · log k + k² log k)` above |
| Free fn `batch_evaluate_subproduct` | Unconditional subproduct tree (schoolbook `div_rem`) | `poly.rs` : 1920 – 2034 | `O(n · k + k² log k)` |
| Free fn `batch_evaluate_subproduct_auto` (for `F: TwoAdicField`) | Unconditional subproduct tree (`div_rem_auto`) | `poly.rs` : same block as `batch_evaluate_subproduct` | `O(M(n) · log k + k² log k)` above `DIV_REM_THRESHOLD` |
| Free fn `build_subproduct_tree` | Balanced pair-merge | `poly.rs` : 1826 – 1918 | `O(k · M(k))` polynomial multiplications |
| `FieldPoly::batch_mul` / `batch_mul_with_field` | Balanced binary merge | `poly.rs` : 1159 – 1305 | `O(K · M(K) · log k)` |
| `FieldPoly::batch_gcd` | Repeated Euclidean reductions | `poly.rs` : 1305 – 1364 | `O(k · n · m · log min(n, m))` |
| `interpolate` | Barycentric Lagrange + `batch_inverse` | `poly_interpolate.rs` : 348 – 495 | `O(n²)` |
| `interpolate_fast` | Subproduct-tree Lagrange (uses generic `batch_evaluate`) | `poly_interpolate.rs` : 495 – 680 | `O(n² log n)` generic |
| `interpolate_fast_auto` (for `F: TwoAdicField`) | Subproduct-tree Lagrange (uses `batch_evaluate_auto`) | `poly_interpolate.rs` : sibling of `interpolate_fast` | `O(n log² n)` above `SUBPRODUCT_THRESHOLD`; matches `interpolate_fast` below |
| `interpolate_auto` | Threshold-tuned dispatcher over `interpolate` + `interpolate_fast` | `poly_interpolate.rs` : 130 – 230 | picks the right asymptotic (generic substrate) |
| `interpolate_auto_two_adic` (for `F: TwoAdicField`) | Threshold-tuned dispatcher over `interpolate` + `interpolate_fast_auto` | `poly_interpolate.rs` : sibling of `interpolate_auto` | picks the right asymptotic; reaches `O(n log² n)` above `SUBPRODUCT_THRESHOLD` |
| `formal_derivative` | Elementwise `i · coeffs[i]` | `poly_interpolate.rs` : 233 – 347 | `O(n)` |
| `ntt_inplace` | Radix-2 DIT NTT | `ntt.rs` : 112 – 360 | `O(N log N)` field multiplications |
| `batch_inverse` / `batch_inverse_in_place` / skip-zero variants | Montgomery trick | `batch_ops.rs` : 70 – 840 | one `inv` + `3(N − 1)` multiplications |

The tuning constants live in `poly.rs` (`KARATSUBA_THRESHOLD`,
`NTT_THRESHOLD`, `DIV_REM_THRESHOLD`, `SUBPRODUCT_THRESHOLD`) and
`poly_interpolate.rs` (`INTERPOLATE_THRESHOLD`). See the **Threshold
summary** section below for the tuned values and the benchmark story
behind each. `cargo doc --no-deps -p gf2-core` produces the
cross-linked rustdoc tree; the module-level page of `field::poly`
holds the complexity-reference table and the most recent `cargo bench
-p gf2-core --bench field_poly -- --quick` snapshot.

## When to use which

- **Polynomial multiplication.** Reach for the `Mul` operator (or the
  `FieldPoly::mul` method) when the operands are under a thousand
  coefficients and the field is arbitrary: it dispatches to schoolbook or
  Karatsuba through `KARATSUBA_THRESHOLD = 32`. If your field implements
  `TwoAdicField` *and* the output length is large enough that the NTT's
  butterfly cost wins out, call the free function `mul_fast` — it applies
  the tuned `NTT_THRESHOLD = 128` and falls through to the Karatsuba path
  otherwise. For benchmarking the underlying NTT path unconditionally, call
  `FieldPoly::mul_ntt` directly.

- **Batch products / root polynomials.** For a slice of `k` polynomials,
  prefer `FieldPoly::batch_mul` over a hand-rolled left fold: the balanced
  binary merge tree is asymptotically `log k` cheaper and, in the current
  bench snapshot, 2.3× faster at `k = 128` degree-8 inputs on `Fp<65537>`.
  `batch_mul_with_field` exists for the empty-batch case where no operand
  is available to anchor `F`. `FieldPoly::from_roots` stays on a simple
  left fold because it is dominated by other call sites and `k` is
  typically small; callers with many roots should hand off to `batch_mul`
  themselves.

- **Multi-point evaluation.** `FieldPoly::eval` for a single point,
  `eval_batch` for `k` independent Horner folds, `batch_evaluate` for the
  generic auto-dispatched public entry point, and
  `FieldPoly::batch_evaluate_auto` for the `TwoAdicField`-specialised
  dispatcher. Both share `SUBPRODUCT_THRESHOLD = 4096` on `Fp<65537>`:
  below it they fall back to naive Horner; at or above it
  `batch_evaluate` uses the schoolbook-`div_rem` subproduct tree and
  `batch_evaluate_auto` uses the Newton-iteration-`div_rem_auto`
  subproduct tree (issue `046f95c1`). The crossover at `n = k = 4096`
  is narrow on `Fp<65537>` (subproduct_auto 54.29 ms vs naive
  60.89 ms, ≈ `0.89×`); the generic dispatcher at that cell lands
  at ~80 ms because it routes through the schoolbook subproduct
  arm — which is ≈ 1.32× slower than naive but the documented
  trade-off for sharing one threshold constant across both
  dispatchers. Callers on `TwoAdicField` should prefer
  `FieldPoly::batch_evaluate_auto` (the `subproduct_auto` arm is the
  one that actually wins at the crossover). Fields with
  significantly more expensive scalar arithmetic tip the balance
  earlier and can bypass the gate by calling
  `batch_evaluate_subproduct` (generic, schoolbook `div_rem`) or
  `batch_evaluate_subproduct_auto` (`TwoAdicField`, `div_rem_auto`)
  directly.

- **Interpolation.** `interpolate_auto` is the recommended generic entry
  point: it dispatches through `INTERPOLATE_THRESHOLD = 16`, sending small
  inputs to the quadratic Lagrange path and large inputs to the
  subproduct-tree variant. `interpolate_fast` already beats `interpolate`
  from `n = 4` upwards on `Fp<65537>`, so the threshold is a conservative
  margin for fields with expensive polynomial multiplication (where the
  intermediate merges in `build_subproduct_tree` may flip the balance).
  Callers on `TwoAdicField` fields should reach for
  `interpolate_auto_two_adic` instead: same threshold dispatch but the
  fast path routes through `interpolate_fast_auto`, whose middle step
  uses `FieldPoly::batch_evaluate_auto` and picks up the
  Newton-iteration `FieldPoly::div_rem_auto` primitive above
  `SUBPRODUCT_THRESHOLD` — unlocking the `O(n log² n)` asymptotic on
  large inputs (issue `046f95c1`). Callers who want a specific variant
  can call `interpolate`, `interpolate_fast`, or `interpolate_fast_auto`
  directly.

- **Field inversion in bulk.** Whenever you need to invert `N ≥ 8`
  elements, reach for `gf2_core::field::batch_ops::batch_inverse` — one
  field inversion plus `3(N − 1)` multiplications, ≈ 5× faster than a
  per-element `inv` loop on `Fp<65537>`. Both Lagrange implementations in
  `poly_interpolate.rs` already depend on this.

## Threshold summary

The fast-path dispatchers in this module all share the same tuning
pattern: a single `usize` constant, tuned from criterion benchmarks on
the Zen 3 reference host, that gates the crossover between a
constant-factor-cheap schoolbook path and an asymptotically-better
fast path.

| Constant | Location | Tuned value | Crossover story |
|----------|----------|------------:|-----------------|
| `KARATSUBA_THRESHOLD` | `poly.rs` | 32 | Karatsuba fires above this, schoolbook below. Tuned from early GF(2^14) measurements. |
| `NTT_THRESHOLD` | `poly.rs` | 128 | `mul_fast` dispatches to NTT at or above this, Karatsuba below. Tuned from `field_poly_mul_fp65537` (n = 128 ties, n = 256 NTT wins 2×). |
| `DIV_REM_THRESHOLD` | `poly.rs` | 2048 | `div_rem_auto` dispatches to `div_rem_fast` at or above this, schoolbook below. Tuned from `field_poly_div_rem_fp65537` (n = 2048 fast wins 1.7×). Landed in issue `ae0c7e1f`. |
| `SUBPRODUCT_THRESHOLD` | `poly.rs` | 4096 | `batch_evaluate` / `batch_evaluate_auto` dispatch to the subproduct tree at or above this, naive Horner below. Tuned from `field_poly_batch_evaluate_fp65537` — the only winning cell for `subproduct_auto` vs naive on `Fp<65537>` is `(n = 4096, k = 4096)` at `0.89×` (54.29 ms vs 60.89 ms on the latest rerun). The generic `batch_evaluate` dispatcher at that cell lands at ~80 ms because it routes through the schoolbook `subproduct` arm; `TwoAdicField` callers should reach for `batch_evaluate_auto`. Landed in issue `046f95c1` on top of the `ae0c7e1f` `div_rem_fast` substrate. |
| `INTERPOLATE_THRESHOLD` | `poly_interpolate.rs` | 16 | `interpolate_auto` dispatches to `interpolate_fast` at or above this, quadratic barycentric below. `fast` already wins from `n = 4` on `Fp<65537>` so the threshold is a conservative safety margin for fields with expensive polynomial multiplication. Re-verified under issue `046f95c1`; no retuning needed. |

## Known gaps and future work

- **`SUBPRODUCT_THRESHOLD = 4096` is narrow on `Fp<65537>`.** The
  crossover cell is `(n = 4096, k = 4096)` at `0.89×` of naive Horner
  (`subproduct_auto`: 54.29 ms vs 60.89 ms on the latest rerun), and
  every smaller cell still favours naive. The generic `batch_evaluate`
  dispatcher at that cell tracks the schoolbook `subproduct` arm at
  ~80 ms, a ~1.32× regression against naive — acceptable because
  `TwoAdicField`-eligible callers should use `batch_evaluate_auto`,
  which is the arm that actually wins there.
  Fields with substantially more expensive scalar arithmetic — large
  prime Montgomery, tower extensions — tip the balance earlier;
  callers on those fields who want to force the subproduct-tree path
  should call `batch_evaluate_subproduct` (generic) or
  `batch_evaluate_subproduct_auto` (`TwoAdicField`) directly. A
  future refinement could specialise `SUBPRODUCT_THRESHOLD` per-field
  (e.g. via an associated constant on a hypothetical `FieldCost`
  trait), but the single-constant tuning is sufficient for the
  benchmarked workload.

- **`mul_fast` is the only NTT entry point.** `FieldPoly::mul_ntt` is
  inherently specialised to `TwoAdicField`, and Rust coherence on
  MSRV-1.80 prevents a second `impl Mul` variant that picks the NTT path
  automatically. Call sites on `TwoAdicField` that want the fast product
  must therefore reach for `mul_fast` (or `mul_ntt`) explicitly — the
  `Mul` operator stays on the Karatsuba path. A future MSRV bump to a
  stabilised `specialization` feature would let the dispatcher fold into
  the operator itself; until then, the free function is the canonical
  entry point. The same coherence constraint is why
  `FieldPoly::batch_evaluate_auto` lives under
  `impl<F: TwoAdicField>` rather than overriding the generic
  `FieldPoly::batch_evaluate`.

- **`interpolate_fast` is generic over `F: FiniteField`; the
  `O(n log² n)` middle step is reached via `interpolate_fast_auto`.**
  The generic `interpolate_fast` body calls
  `FieldPoly::batch_evaluate` for step 3 (evaluating `M'` at the
  interpolation points), so on its own it keeps the schoolbook
  substrate. Issue `046f95c1` introduced the `TwoAdicField`-bounded
  sibling `interpolate_fast_auto` (SSOT-shared body with
  `interpolate_fast`, differing only in the injected middle-step
  primitive — it wires `FieldPoly::batch_evaluate_auto` in, which
  routes above-threshold cases through `FieldPoly::div_rem_auto` and
  unlocks the `O(n log² n)` asymptotic above `SUBPRODUCT_THRESHOLD`).
  `TwoAdicField` call-sites that want the threshold-tuned dispatch
  should use `interpolate_auto_two_adic`, the sibling dispatcher over
  `interpolate` + `interpolate_fast_auto`. Rust coherence on MSRV-1.80
  forbids a second `pub fn interpolate_auto` specialised on
  `TwoAdicField`, so the two dispatchers live under distinct names.

- **No cross-field polynomial helpers.** The `Gf2mPoly_<V>` type is a
  thin alias to `FieldPoly<Gf2mElement_<V>>`; there is *intentionally* no
  cross-type conversion helper between `FieldPoly<Gf2mElement>` and
  `FieldPoly<Gf2mElement_<V>>`. Callers that need to bridge the two
  construct the target polynomial via the field's `element(u64)` /
  `from_repr` primitives.

## Reproducing the bench snapshot

```bash
cargo bench -p gf2-core --bench field_poly -- --quick
```

All four benchmark groups (`field_poly_batch_evaluate_fp65537`,
`field_poly_batch_mul_fp65537`, `field_poly_mul_fp65537`, and
`field_poly_interpolate_fp65537`) run in a single invocation. The
harness is self-contained under `crates/gf2-core/benches/field_poly.rs`
and uses the workspace LCG (`gf2_core::rng::Lcg`) for deterministic
inputs. The tables in `field::poly`'s module docstring and the
per-module docstrings on `field::ntt` and `field::poly_interpolate` are
regenerated from the same run, so a snapshot from this command updates
every site consistently.
