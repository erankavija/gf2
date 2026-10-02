# Rustdoc Example Census — Post-Removal

> **Diátaxis Type:** Reference

**Issue:** a0a29512
**Baseline:** `fa787f85-rustdoc-example-audit.md`

Post-removal figures for the tautological-example pass, measured with
`fa787f85-example-census.py` and the baseline's documentation-test command.

## Census

| Measure | Count |
| --- | ---: |
| `# Example` / `# Examples` headings | 801 |
| Rustdoc code fences | 1,085 |
| Executed (bare, `rust`, `should_panic`) | 737 |
| Compiled, not executed (`no_run`) | 82 |
| Never compiled | 266 |

| Crate | Example headings |
| --- | ---: |
| gf2-algebra | 73 |
| gf2-coding | 227 |
| gf2-core | 328 |
| gf2-kernels-hip | 53 |
| gf2-kernels-simd | 27 |
| gf2-sim | 91 |
| gf2-stats | 2 |

## Documentation-test timing

`cargo test --workspace --all-features --doc` through
`scripts/cargo-budget.sh --test`, warm target directory, rustc 1.97.0, debug
profile, on a 24-thread host at load average 50–60. The host was contended, so
these times are an upper bound and not comparable with the baseline as a speedup.

| Crate | Doctests | Ignored | Execution |
| --- | ---: | ---: | ---: |
| gf2-core | 348 | 3 | 14.83 s |
| gf2-coding | 234 | 7 | 13.57 s |
| gf2-sim | 94 | 0 | 16.02 s |
| gf2-algebra | 73 | 0 | 7.97 s |
| gf2-kernels-simd | 26 | 0 | 0.16 s |
| gf2-stats | 3 | 0 | 0.19 s |
| **Total** | **778** | **10** | **52.74 s** |

`gf2-kernels-hip` is outside the default Cargo workspace, so no workspace
doctest run reaches it; its examples run through
`cargo test --doc --manifest-path crates/gf2-kernels-hip/Cargo.toml`
(40 passed, 12 ignored).

## Disposition of the audit's tautological set

The audit's line citations predate later edits, so each verdict was re-located
by block content. Every tautological block still present in the tree is
removed; none is kept. Thirteen tautological verdicts name examples that no
longer exist: `BchCode::new`, the ten `ExtendedBchCode` items of the removed
`bch/extended.rs`, and the per-method `GeneratorMatrixAccess::generator_matrix`
and `is_systematic` examples.

The orphaned `FieldMatrix::from_rows` documentation fragment in
`crates/gf2-core/src/field/matrix.rs`, which carried one tautological example
and was attached to `as_data_slice`, is removed whole; the method's own
documentation remains.

Blocks the audit classed as contract-clarifying, removed as accessor,
constant, constructor, predicate or operator restatements:

- `crates/gf2-algebra/src/packed/bipedal3.rs`: `Bipedal3Vec`, both `Debug`
  impls, `add`, `sub`, `neg`, `mul`, `lane`
- `crates/gf2-coding/src/fading.rs`: `Complex::norm`, `Complex::norm_sq`
- `crates/gf2-coding/src/gldpc/mod.rs`: `QcGldpcCode::systematic_positions`
- `crates/gf2-coding/src/product/mod.rs`: `TurboDecoder::sogrand`
- `crates/gf2-coding/src/traits.rs`: bit-only `GeneratorMatrixAccess`
- `crates/gf2-core/src/field/poly.rs`: `FieldPoly::one_like`
- `crates/gf2-kernels-hip/src/host/arch.rs`: `GfxTarget::has_compiled_blob`
- `crates/gf2-kernels-hip/src/host/streams.rs`: `HipStreamPool::get`
- `crates/gf2-kernels-simd/src/bipedal/bipedal3.rs`: `Config3`
- `crates/gf2-sim/src/executor/failure.rs`: `default_dump_dir`

Blocks added after the audit, classified under the same policy and removed:

- `crates/gf2-algebra/src/permanent/rank.rs`: `PermanentalRank`,
  `PermanentalRank::is_deficient`
- `crates/gf2-coding/src/bch/dvb_t2/mod.rs`: `dvb_t2_bch_code`
- `crates/gf2-coding/src/ldpc/core.rs`: `LdpcDecoder::edge_layout`
- `crates/gf2-coding/src/traits.rs`: bit-only `BlockEncoder`
- `crates/gf2-core/src/field/axiom_tests.rs`: `quadratic_strategy`,
  `cubic_strategy`, `test_field_identity_laws`, `test_extension_laws`
- `crates/gf2-core/src/field/extension.rs`: `CosetPartition`,
  `CosetPartition::cosets`, `CosetPartition::defining_set`, `BinaryPrimeExt`
- `crates/gf2-core/src/gf2m/field.rs`: `Gf2mElement_::field`
- `crates/gf2-core/src/gfpn/quotient.rs`: `QuotientField`, `QuotientExt`,
  `QuotientField::modulus`, `QuotientField::indeterminate`,
  `QuotientField::order`, `QuotientElement`, `QuotientElement::coefficients`,
  `QuotientElement::field`
- `crates/gf2-core/src/kernels/ops.rs`: `resolve_popcount`,
  `resolve_and_popcount`
- `crates/gf2-kernels-simd/src/transpose.rs`: `TransposeLane::from_name`
- `crates/gf2-stats/src/intervals.rs`: `wilson_interval`,
  `clopper_pearson_interval`
