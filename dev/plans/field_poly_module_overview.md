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
| `poly_interpolate.rs` | Lagrange interpolation: `interpolate` (O(n²) barycentric), `interpolate_fast` (subproduct-tree), `interpolate_auto` (threshold-tuned dispatcher), `formal_derivative`, plus the `InterpolationError` enum. |
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
| `FieldPoly::batch_evaluate` | Auto-dispatch: naive Horner ⇄ subproduct tree | `poly.rs` : 988 – 1040 | current `O(n · k)`; target `O(M(n) · log k)` pending fast `div_rem` |
| Free fn `batch_evaluate_subproduct` | Unconditional subproduct tree | `poly.rs` : 1920 – 2034 | current `O(n · k + k² log k)`; target `O(M(n) · log k)` |
| Free fn `build_subproduct_tree` | Balanced pair-merge | `poly.rs` : 1826 – 1918 | `O(k · M(k))` polynomial multiplications |
| `FieldPoly::batch_mul` / `batch_mul_with_field` | Balanced binary merge | `poly.rs` : 1159 – 1305 | `O(K · M(K) · log k)` |
| `FieldPoly::batch_gcd` | Repeated Euclidean reductions | `poly.rs` : 1305 – 1364 | `O(k · n · m · log min(n, m))` |
| `interpolate` | Barycentric Lagrange + `batch_inverse` | `poly_interpolate.rs` : 348 – 495 | `O(n²)` |
| `interpolate_fast` | Subproduct-tree Lagrange | `poly_interpolate.rs` : 495 – 680 | current `O(n² log n)`; target `O(n log² n)` |
| `interpolate_auto` | Threshold-tuned dispatcher | `poly_interpolate.rs` : 130 – 230 | picks the right asymptotic |
| `formal_derivative` | Elementwise `i · coeffs[i]` | `poly_interpolate.rs` : 233 – 347 | `O(n)` |
| `ntt_inplace` | Radix-2 DIT NTT | `ntt.rs` : 112 – 360 | `O(N log N)` field multiplications |
| `batch_inverse` / `batch_inverse_in_place` / skip-zero variants | Montgomery trick | `batch_ops.rs` : 70 – 840 | one `inv` + `3(N − 1)` multiplications |

The tuning constants live in `poly.rs` (`KARATSUBA_THRESHOLD`,
`NTT_THRESHOLD`, `SUBPRODUCT_THRESHOLD`) and `poly_interpolate.rs`
(`INTERPOLATE_THRESHOLD`). `cargo doc --no-deps -p gf2-core` produces the
cross-linked rustdoc tree; the module-level page of `field::poly` holds the
complexity-reference table and the most recent `cargo bench
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
  auto-dispatched public entry point. On today's schoolbook `div_rem`
  substrate the naive Horner path wins on `Fp<65537>` at every
  benchmarked size, so `SUBPRODUCT_THRESHOLD` is set to `usize::MAX` and
  `batch_evaluate` routes through `eval_batch`. Callers on fields with
  significantly more expensive scalar arithmetic can bypass the gate by
  calling `batch_evaluate_subproduct` directly.

- **Interpolation.** `interpolate_auto` is the recommended entry point:
  it dispatches through `INTERPOLATE_THRESHOLD = 16`, sending small inputs
  to the quadratic Lagrange path and large inputs to the subproduct-tree
  variant. `interpolate_fast` already beats `interpolate` from `n = 4`
  upwards on `Fp<65537>`, so the threshold is a conservative margin for
  fields with expensive polynomial multiplication (where the intermediate
  merges in `build_subproduct_tree` may flip the balance). Callers who
  want a specific variant can call `interpolate` or `interpolate_fast`
  directly.

- **Field inversion in bulk.** Whenever you need to invert `N ≥ 8`
  elements, reach for `gf2_core::field::batch_ops::batch_inverse` — one
  field inversion plus `3(N − 1)` multiplications, ≈ 5× faster than a
  per-element `inv` loop on `Fp<65537>`. Both Lagrange implementations in
  `poly_interpolate.rs` already depend on this.

## Known gaps and future work

- **`SUBPRODUCT_THRESHOLD = usize::MAX`.** The asymptotic benefit of the
  subproduct-tree path (`batch_evaluate_subproduct`) — and therefore the
  full `O(n log² n)` target for `interpolate_fast` — is unrealised until
  a fast `div_rem` primitive lands alongside the existing NTT. The tree
  currently depends on `FieldPoly::div_rem` for the reduction phase,
  which is schoolbook `O(n · m)` and eats the gain. While the threshold
  stays at `usize::MAX`, every call through the public
  `FieldPoly::batch_evaluate` dispatcher routes to the naive per-point
  Horner path, so the bench snapshot in `poly.rs` compares the internal
  fast-path helper (`batch_evaluate_subproduct`) against naive Horner
  rather than the dispatcher itself. The follow-up task to fix this is
  tracked in the `bdf95060` story plan and will replace `div_rem` with an
  NTT-backed Newton-iteration inverse; when it lands, this constant is
  lowered to the tuned crossover point and both fast paths activate
  without any further API churn.

- **No fast `div_rem` today.** The current schoolbook `FieldPoly::div_rem`
  is the substrate under `gcd`, `batch_gcd`, the subproduct-tree
  reduction, and the downward sweep in `interpolate_fast`. Every one of
  those routines has an `O(n log² n)` target that trips on this
  bottleneck. The successor task is the single largest remaining lever
  for throughput on this module.

- **`mul_fast` is the only NTT entry point.** `FieldPoly::mul_ntt` is
  inherently specialised to `TwoAdicField`, and Rust coherence on
  MSRV-1.80 prevents a second `impl Mul` variant that picks the NTT path
  automatically. Call sites on `TwoAdicField` that want the fast product
  must therefore reach for `mul_fast` (or `mul_ntt`) explicitly — the
  `Mul` operator stays on the Karatsuba path. A future MSRV bump to a
  stabilised `specialization` feature would let the dispatcher fold into
  the operator itself; until then, the free function is the canonical
  entry point.

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
