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

## Opaque modules

The following modules are marked `--opaque` during Charon extraction because
they are outside the verification scope or cause extraction issues:

| Module | Reason |
|--------|--------|
| `gf2_core::field` | HRTB `for<'a>` bounds on `FiniteField` trait |
| `gf2_core::gf2m` | Runtime field parameters, `Vec<u64>` storage |
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

## Const generics work

`Fp<const P: u64>` extracts correctly — Charon handles const generics and
Aeneas translates `P` as a Lean4 parameter `(P : Std.U64)`. No monomorphization
wrappers were needed.

## Tool versions

| Tool | Version | Pin |
|------|---------|-----|
| Charon | v0.1.173 | local patched build, base `24a17b5e` + 3 fixes (see `dev/lean4-verification-pipeline.md`) |
| Aeneas | latest | git rev `c23de9324fbe4d3630fc532e5216a0568b9beb5c` |
| Lean4 | v4.28.0-rc1 | via `proofs/lean-toolchain` |
| Rust nightly | nightly-2026-02-07 | required by Charon for rustc internals |
