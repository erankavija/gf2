# Rust-to-Lean extraction boundaries

The installation uses unmodified upstream Charon and Aeneas. The Aeneas
revision in `proofs/lakefile.lean` is authoritative; its `charon-pin` selects
Charon. `scripts/install-lean-tools.sh` reads both pins and Charon's Rust
nightly requirement. Lean and Mathlib follow the selected Aeneas backend.

## Duplicate trait fields

Aeneas gives identically named fields to bounds on `FiniteField::Self`,
`Characteristic`, and `Wide`. `scripts/fix-aeneas-dupes.py` disambiguates these
fields and their projections with `Characteristic` and `Wide` suffixes.
It does not replace function bodies or synthesize default methods.

## External models

`Gf2Core/FunsExternal.lean` is hand-maintained. It provides concrete definitions
for wrapping negation and checked U128 addition/add-assignment. The default
`FiniteField::theorem_4_operand_bound` returns `u128::MAX`, matching
`crates/gf2-core/src/field/traits.rs`. Making this default concrete lets
Aeneas's `impl_def` resolve the extension-field dictionaries without a
recursive reference through an opaque default.

Other external declarations are trusted axioms. They include formatting,
hashing, scalar queries, opaque trait defaults, and batch helpers outside the
proved scalar arithmetic. Aeneas's unsuccessful `Debug::fmt` translations
also appear as external axioms. The generated templates list the current
external signatures. Regeneration preserves the hand-maintained core file
and regenerates the algebra external file from its template.

## Algebra iterator dictionaries

Aeneas emits Rust iterator dictionary fields that its Lean `Iterator` model
does not contain. `scripts/fix-aeneas-gf2algebra.py` keeps the concrete `next`
implementations for `Map` and `Zip`; the remaining modeled fields use
upstream's defaults. These dictionaries introduce no additional axioms.

Packed7Vec arithmetic uses indexed loops, which Aeneas translates without
a mutable-iterator backward function. The postprocessor checks the expected
dictionary shapes and fails on drift.

The same pass retains the explicit allowlist of opaque `Fp` operator
wrappers reached through the `PackedField<Fp<P>>` bound. Their bodies are
outside the algebra extraction. The generated `FiniteField` dictionary and
usable Neg wrapper remain concrete. The packed-element proofs operate on
words and never project the opaque operator wrappers.

## Extraction scope

The two Charon invocations in `scripts/verify-lean.sh` define the authoritative
`--start-from` and `--opaque` sets. The core targets include scalar prime and
extension-field arithmetic and the free `gf2m::mul_raw` functions. Runtime
field containers, SIMD dispatch, and unrelated modules stay opaque.

`cfg(verify_lean)` preserves the extraction interface: `ExtConfig` exposes
`NON_RESIDUE()` as a method, and `Fp` uses scalar trait defaults instead of
SIMD hooks. Ordinary Rust builds retain their public associated constant and
runtime dispatch. These extraction-specific Rust branches remain in use.

The algebra arithmetic proofs target the inherent `add`, `sub`, `mul`, and
`neg` wrappers of the packed field elements. The Packed7 lookup tables are
opaque constants with source-faithful characterizations, cross-checked by
the exhaustive Rust LUT contract tests (@/issue/30e98ef1). Their initializers
are outside extraction. The bipedal proof covers `neg`, rather than the
original sketch's `div`, because the production `PackedField` interface
provides `neg` and has no division method (@/issue/f05ffbe1).

## Proof API

The proofs use Aeneas's `step` tactic and `spec_ok`/`bind_ok` laws for its
coinductive `Result`. `Gf2Core/Proofs/ResultSpec.lean` proves that a total
Rust specification yields a return value and an equality to `ok`; failure
and divergence contradict total correctness. Field models use that result
to recover pure operations. It introduces no axiom.

The [formal-verification guide](../docs/how-to/formal-verification.md)
describes installation, regeneration, and the strict hand-written-proof
build gate.
