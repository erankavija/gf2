# Aeneas/Charon Workarounds

Known issues and workarounds for the Rust→Lean4 translation pipeline.

## Duplicate field names in FiniteField struct

**Issue**: Aeneas generates duplicate field names when a trait has supertype
bounds on multiple associated types. The `FiniteField` trait requires `Clone`,
`Eq`, etc. on `Self`, `Self::Characteristic`, and `Self::Wide`, producing three
fields all named `corecloneCloneInst`, etc.

**Fix**: `scripts/fix-aeneas-dupes.py` renames the duplicates with
`Characteristic` and `Wide` suffixes (e.g., `corecloneCloneCharacteristicInst`).
This runs automatically as Step 3 of `scripts/verify-lean.sh`.

## FunsExternal.lean hand-edited definitions

`FunsExternal.lean` replaces Aeneas axioms with concrete definitions for
`wrapping_neg`, `overflowing_sub`, and U128 `add`/`add_assign`. The
`verify-lean.sh` script only seeds from the template on first run; it never
overwrites the hand-edited file.

## Opaque modules

The following modules are marked `--opaque` during Charon extraction because
they are outside the verification scope or cause extraction issues:

| Module | Reason |
|--------|--------|
| `gf2_core::field` | HRTB `for<'a>` bounds on `FiniteField` trait |
| `gf2_core::gf2m::field` | Runtime field parameters, `Arc<FieldParams>`, `Vec<u64>` storage |
| `gf2_core::gf2m::generation` | Uses `gf2m::field` types |
| `gf2_core::gf2m::uint_ext` | Sealed trait, out of scope |
| `gf2_core::gf2m::thread_safety_tests` | Test module |
| ~~`gf2_core::gfpn`~~ | Now transparent (was opaque before Charon HRTB patches) |
| `gf2_core::bitvec` | Out of scope (bit manipulation, not field arithmetic) |
| `gf2_core::bitslice` | Out of scope |
| `gf2_core::matrix` | Out of scope |
| `gf2_core::sparse` | Out of scope |
| `gf2_core::alg` | Out of scope |
| `gf2_core::compute` | Rayon parallelism, not supported by Aeneas |
| `gf2_core::kernels` | SIMD dispatch, not supported by Aeneas |
| `gf2_core::primitive_polys` | Static data, not needed |
| `gf2_core::io` | Serde, not supported by Aeneas |
| `gf2_core::macros` | Proc macros, not relevant |

The `field::traits::FiniteField` and `ConstField` trait *declarations* are still
extracted (needed for the `Fp` impl), but their bodies are opaque.

## gfpn/ extraction and verification

The `gfpn/` module (`QuadraticExt`, `CubicExt`) is now fully extracted and
verified. This required three patches to our local Charon build (HRTB erase,
SelfClause/Local unification, implied clause constraint propagation) and the
post-processing workarounds described above. See
`dev/lean4-verification-pipeline.md` for full details.

Charon emits 13 benign "Type error after transformations" warnings about
mismatched generic arg counts for `CubicExt`/`QuadraticExt` (expected 4, got 7).
These are harmless — Aeneas handles them correctly via Lean4 implicit argument
inference.

## ExtConfig associated const extraction

Charon 0.1.173/0.1.174 rejects `ExtConfig::NON_RESIDUE` during trait
declaration checking because the associated const's type is the associated type
`Self::BaseField`; the diagnostic is "Found incorrect clause var" followed by a
Charon stack overflow. During `scripts/verify-lean.sh` only, the script passes
`--cfg=verify_lean`, and `ExtConfig` exposes the same β accessor as a trait
method (`NON_RESIDUE()`) rather than that associated const. The uppercase method
name is deliberate: it preserves the generated Lean trait-field name used by the
normal associated const, minimizing extraction-only proof drift. Normal Rust
builds keep the public associated const API.

The same Charon version can overflow after the const workaround when starting
from the whole crate. `verify-lean.sh` therefore starts extraction from the
proof-relevant modules (`gfp`, `gfpn`, and `gf2m::mul_raw`) instead of from
`crate`. The `gfpn::batch`
module remains opaque because it is a vectorized batching layer over the scalar
quadratic/cubic arithmetic and pulls iterator models that this Aeneas pin does
not provide. This keeps the intended production scalar field arithmetic
transparent while avoiding unrelated public items.

If Aeneas emits an opaque `Gf2mElement_` external signature that refers to the
sealed `UintExt` trait, `verify-lean.sh` adds only that opaque trait axiom to
`TypesExternal.lean` and removes any duplicate generated `UintExt` declaration
from `Types.lean`. This is outside the `gfp/` and `gfpn/` proof target.

## FiniteField default method projection extraction

Aeneas 1180be60 can generate `FiniteField` implementation records that project
default trait methods from impl-specific constants it never emitted. This showed
up after adding delayed product-sum hooks as missing
`mul_product_sum_wide`/`reduce_product_sum_wide` definitions for
`GoldilocksFp`, `QuadraticExt`, and `CubicExt`.

The production Rust impls for those three field families now explicitly forward
the hooks to the same canonical wide path used by the trait defaults:
`mul_product_sum_wide` calls `mul_to_wide`, and `reduce_product_sum_wide` calls
`reduce_wide`. This is intentionally narrow: `gfp/` and scalar `gfpn/`
arithmetic remain transparent to Charon/Aeneas, and the specialized
storage-domain product-sum override for generic `Fp<P>` remains unchanged.

## gf2m/ selective extraction

The `gf2m` module was originally fully opaque due to `Arc<FieldParams>` and
`Option<Vec<u16>>` in `Gf2mField_<V>`. To verify `mul_raw` (schoolbook GF(2^m)
multiplication), a monomorphized u64 free function `gf2m_mul_raw` was extracted
into `gf2m/mul_raw.rs`.

**Key**: Charon's `--opaque gf2_core::gf2m` prevents exploring the module
entirely, so `--include gf2_core::gf2m::mul_raw` within it has no effect.
The solution is to make individual submodules opaque (`gf2m::field`,
`gf2m::generation`, `gf2m::uint_ext`, `gf2m::thread_safety_tests`) while
leaving `gf2m::mul_raw` transparent.

The extracted loop uses Aeneas's `loop` combinator with `(result, temp, i)`
state. The Rust `while i < m` loop (replacing `for i in 0..m` which has a
runtime bound) extracts cleanly.

## Const generics work

`Fp<const P: u64>` extracts correctly — Charon handles const generics and
Aeneas translates `P` as a Lean4 parameter `(P : Std.U64)`. No monomorphization
wrappers were needed.

## gf2-algebra bipedal F_3 extraction (D2 / JIT f05ffbe1)

The `Gf2Algebra/` Lean library is a second Aeneas extraction covering only the
bipedal F_3 packed arithmetic at `gf2_algebra::packed::bipedal3::Bipedal3`,
needed for the D2 V1 correctness proof
(`dev/plans/d2_lean_bipedal3_sketch.md`). It is verified in lock-step with the
existing `Gf2Core/` extraction via the same `scripts/verify-lean.sh`. The
Bipedal3 V1 proofs (`proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean`)
target the four inherent wrappers
`Bipedal3::{add,sub,mul,neg}_inherent` defined in
`crates/gf2-algebra/src/packed/bipedal3.rs`. Each wrapper is a single tail-call
into the corresponding `PackedField<Fp<3>>` trait method on `Bipedal3`; the
arithmetic formula lives in the trait impl. Targeting the inherent wrappers
gives a stable, non-dispatch-indirected proof target (D2 sketch §5, Option A).

`scripts/fix-aeneas-gf2algebra.py` rewrites the transitively-extracted
`gf2_core::gfp::Fp` trait-impl wrappers as `axiom`s. These impls are pulled in
by Charon because `PackedField<Fp<3>>` has `Fp<3> : FiniteField` as a parent
bound, but they are never elaborated at runtime by the bipedal3 ops (which are
pure bitwise on `Std.U64`). Without this rewrite, Aeneas produces unresolvable
references in two ways:

1. The `FiniteField` impl on `Fp<P>` uses `*.default` field values that refer
   recursively to the impl itself
   (`WINOGRAD_THRESHOLD.default (… P)`), surfacing as
   `impl_def: could not resolve recursive fields`.
2. The per-trait `add/sub/mul/…` Fp impls reference body-defs
   (`gf2_core.gfp.Fp.Insts.CoreOpsArithAddFpFp.add`) that are opaque in our
   narrow extraction, surfacing as `Unknown constant`.

Both are eliminated by axiomatising the impl wrappers — the bipedal3 proofs
never project them.

The gf2-algebra `FunsExternal.lean` is always regenerated from the
auto-generated template (no hand-edits are needed: bipedal3 uses only `&&&`,
`|||`, `^^^` on `Std.U64`, never any wrapping arithmetic or U128 ops).

### V1 Proof divergence: `neg` in place of `div`

The D2 sketch §1 states the V1 contract over `{add, sub, mul, div}`. The
production code at `crates/gf2-algebra/src/packed/bipedal3.rs` exposes
`{add, sub, mul, neg}` — the `PackedField` trait surface
(`packed/mod.rs:185–224`) has no `div` method, so there is no production
function to verify against for `div`. Per the verification-work convention
(sketch supersedes the JIT description, but production code supersedes both
when the sketch names a function that does not exist), V1 proves `neg`
instead. The substitution is benign: the sketch §3.4 already factors `div` as
"the easy op, dispatched by the same `decide` truth table" — `neg` plays the
same role (one truth table, no Result-monad branching). The four `*_correct`
theorems plus the headline `bipedal3_correct_vs_canonical_F3` corollary cover
the same four ops as the production trait surface.

## Tool versions

| Tool | Version | Pin |
|------|---------|-----|
| Charon | v0.1.x | local patched build, base `419f53b6` + 3 fixes (see `dev/plans/charon-aeneas-upstream-sync.md`) |
| Aeneas | latest | git rev `1180be60` |
| Lean4 | v4.28.0-rc1 | via `proofs/lean-toolchain` |
| Rust nightly | nightly-2026-02-07 | required by Charon for rustc internals |
