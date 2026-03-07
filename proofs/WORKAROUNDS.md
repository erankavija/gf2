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
| `gf2_core::gfpn` | Depends on `ExtConfig` which inherits `FiniteField` HRTBs |
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

## gfpn/ not yet extracted

The `gfpn/` module (`QuadraticExt`, `CubicExt`) is currently opaque because
`ExtConfig` inherits `ConstField` → `FiniteField` HRTB bounds, causing Charon
type errors. This will be addressed in a follow-up task once upstream Charon
improves HRTB handling or we restructure the trait hierarchy.

## Const generics work

`Fp<const P: u64>` extracts correctly — Charon handles const generics and
Aeneas translates `P` as a Lean4 parameter `(P : Std.U64)`. No monomorphization
wrappers were needed.

## Tool versions

| Tool | Version | Pin |
|------|---------|-----|
| Charon | v0.1.173 | git rev `1a659e67b982e18da872dea26d4a7b3764dfe0c3` |
| Aeneas | latest | git rev `c23de9324fbe4d3630fc532e5216a0568b9beb5c` |
| Lean4 | v4.28.0-rc1 | via `proofs/lean-toolchain` |
| Rust nightly | nightly-2026-02-07 | required by Charon for rustc internals |
