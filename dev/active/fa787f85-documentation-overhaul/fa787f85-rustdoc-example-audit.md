# Rustdoc Example Audit — Pre-Removal Baseline

> **Diátaxis Type:** Reference

**Issue:** fa787f85
**Type:** epic
**Criterion:** REQ-07
**Date:** 2026-08-07

REQ-07 requires that Rustdoc examples be audited across the workspace, that
tautological examples be removed, and that before-and-after example counts and
documentation-test timing be recorded. This document is the **before** record:
the census, the per-example classification, and the doctest timing measured
against the tree at the date above.

These counts are audit evidence for one criterion, not permanent product facts.
They belong here and in the removal pass's completion record, not in
adopter-facing documentation.

## Census

Measured by `fa787f85-example-census.py` over `crates/**/*.rs`, excluding
`target/`.

| Measure | Count |
| --- | ---: |
| Rust source files | 523 |
| Public declarations | 3,175 |
| Rustdoc lines | 71,875 |
| Non-blank code lines | 174,518 |
| Rustdoc-to-code ratio | 41% |
| `# Example` / `# Examples` headings | 1,404 |
| Files carrying at least one heading | 175 |
| Rustdoc code fences | 1,660 |

Fence disposition across all 1,660 rustdoc code blocks:

| Kind | Count | Compiled | Executed |
| --- | ---: | --- | --- |
| bare ` ``` `, `rust`, `should_panic` | 1,290 | yes | yes |
| `no_run` | 122 | yes | no |
| `text`, `bash`, `ignore`, `json`, `toml`, `rust,ignore` | 248 | no | no |

Both heading spellings occur. The singular `# Example` is used throughout
`crates/gf2-core/src/gf2m/field.rs` and elsewhere; a census matching only the
plural form undercounts.

## Classification

Every one of the 1,404 example blocks was classified against the repository's
documentation standard as either `OBVIOUS` — a straightforward accessor,
constant, constructor, predicate, or direct field mapping; a restatement of the
signature; or one of several near-identical per-method examples on a type — or
`EARNED`. Per-example verdicts with file and line citations are in
`fa787f85-rustdoc-example-verdicts.tsv` alongside this document.

| Crate | Examples | Obvious | Earned | Obvious % |
| --- | ---: | ---: | ---: | ---: |
| gf2-algebra | 181 | 106 | 75 | 59% |
| gf2-coding | 428 | 211 | 217 | 49% |
| gf2-core | 543 | 248 | 295 | 46% |
| gf2-sim | 135 | 45 | 90 | 33% |
| gf2-kernels-hip | 81 | 27 | 54 | 33% |
| gf2-kernels-simd | 36 | 11 | 25 | 31% |
| **Total** | **1,404** | **648** | **756** | **46%** |

No block was left unclassified and none was left uninspected.

The obvious set carries 4,117 lines inside its code fences, against 8,059 for
the earned set. Its fences are 610 executed doctests plus 34 `no_run`; four
obvious headings carry no fence at all.

Densest files, which are where a removal pass recovers the most per file
touched:

| File | Obvious / total |
| --- | --- |
| `crates/gf2-algebra/src/packed/mod.rs` | 20/22 |
| `crates/gf2-coding/src/crc.rs` | 9/10 |
| `crates/gf2-core/src/field/vec.rs` | 21/26 |
| `crates/gf2-core/src/gf2m/wide.rs` | 33/41 |
| `crates/gf2-algebra/src/packed/packed7.rs` | 37/47 |
| `crates/gf2-algebra/src/packed/packed5.rs` | 28/37 |
| `crates/gf2-core/src/bitvec.rs` | 32/43 |
| `crates/gf2-core/src/field/matrix.rs` | 44/68 |
| `crates/gf2-core/src/field/sparse_matrix.rs` | 15/32 |
| `crates/gf2-coding/src/gldpc/mod.rs` | 17/27 |

A representative obvious block, `SparseFieldMatrix::shape` at
`crates/gf2-core/src/field/sparse_matrix.rs:316`, constructs a 3×5 zero matrix
and asserts that `shape()` returns `(3, 5)`. The same shape recurs for `len()`,
`cols()`, and `labels()` across most public types.

## Documentation-test timing

`cargo test --workspace --all-features --doc`, warm target directory:

| Crate | Doctests | Ignored | Execution |
| --- | ---: | ---: | ---: |
| gf2-core | 559 | 2 | 13.93 s |
| gf2-coding | 434 | 7 | 15.27 s |
| gf2-sim | 137 | 0 | 14.07 s |
| gf2-algebra | 181 | 0 | 4.82 s |
| gf2-kernels-simd | 32 | 0 | 0.17 s |
| **Total** | **1,343** | **9** | **48.26 s** |

Wall time for the invocation was 49 s. `gf2-kernels-hip` sits outside the
default Cargo workspace, so its 81 example blocks compile in no doctest run
reachable from the workspace root.

Of the executed doctests that sit under an example heading, 610 of 1,257 belong
to obvious blocks — 49%, or roughly 23 s of the 48.26 s. For scale, the
repository's ordinary fast test tier has a sixty-second budget for the whole
suite.

## Method

Classification ran as eight independent `gpt-5.6-luna` reviewers at high
reasoning effort, each read-only, over partitions balanced to about 176 headings
each. Every reviewer received the documentation standard verbatim, the same
category definitions, and its own file list, and returned one verdict line per
heading citing `path:line`.

Three integrity checks were applied to the returned verdicts:

1. Emitted verdict count per partition compared against the partition's census
   count. Seven of eight matched exactly.
2. Every cited line number re-derived independently and diffed against the
   census heading set, per file. This found two off-by-one citations in
   `crates/gf2-coding/src/bcjr/mod.rs` — same items, adjacent line — and seven
   headings in `crates/gf2-core/src/gf2m/field.rs` that one reviewer skipped
   because it searched only for the plural heading spelling. That reviewer
   reported the shortfall rather than silently closing it. A ninth targeted pass
   classified the seven.
3. Five verdicts were read back against source by hand. All five held.

## Limits of this baseline

The audit covers example blocks introduced by an example heading. Rustdoc prose
outside those blocks — the bulk of the 71,875 documentation lines — was not
assessed for triviality, so this document says nothing about how much obvious
prose exists.

The obvious/earned boundary is soft in the earned direction. `FieldPoly::eval_batch`
at `crates/gf2-core/src/field/poly.rs:1160` was classified earned for showing
ordered multi-point evaluation, though its body is a plain map over `eval`.
Judgments of that shape resolve toward earned, so 46% is a floor on the
tautological share rather than a midpoint.

One verdict row, the `gf2-coding` crate-level module example, carries no line
citation and is therefore absent from the fence-disposition figures while still
counted in the classification totals.

## What the after record must state

For REQ-07 to close, the removal pass records the same census and the same
timing run against the post-removal tree: heading count, fence disposition,
per-crate example counts, doctest count, and doctest execution time.
`fa787f85-example-census.py` produces the mechanical half of that on demand, so
the two ends of the comparison come from one method.
