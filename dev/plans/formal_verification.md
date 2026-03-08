# Formal Verification Strategy: Aeneas + Kani

## Rationale

We rely heavily on proptests for mathematical correctness, but these are sampling-based — they cannot prove absence of bugs. For the field arithmetic in `gfp/` and `gfpn/`, we want formal verification guarantees. After evaluating Lean4 (manual), Kani (bounded model checking), and Aeneas (Rust→Lean4 translation), the proposal is a two-track approach:

- **Aeneas** (Rust→Lean4 translation) for algebraic correctness proofs
- **Kani** (bounded model checking) for machine-arithmetic edge cases

### Why Aeneas

- Translates actual Rust code (not a hand-written model) to Lean4
- `gfp/` and `gfpn/` are fully in the supported subset: const generics, traits with associated types, safe sequential pure arithmetic, no unsafe/dyn/closures/async
- Mathlib integration: state theorems as "this Rust implementation satisfies Mathlib's `Field` typeclass"
- Precedent: Microsoft SymCrypt verification uses Aeneas for Montgomery arithmetic

### Why both Aeneas and Kani

| Concern | Aeneas | Kani |
|---------|--------|------|
| Algebraic correctness | Unbounded proofs for any P | Only bounded (small fields) |
| Algorithm equivalence | For all field sizes | Only small fields |
| Machine overflow | Abstract integers in Lean | Actual u64/u128 semantics |
| Bit manipulation | Not natural | Exhaustive for bounds |
| Unsafe SIMD | Not supported | Supported |

Aeneas gives us universal algebraic guarantees (field axioms hold for *any* prime P), while Kani verifies machine-level concerns (overflow, representation bounds) exhaustively for bounded inputs.

## Compatibility Assessment

| gf2-core feature | Aeneas/Charon support | Notes |
|---|---|---|
| `Fp<const P: u64>` | Supported | Charon `GenericParams` handles const generics |
| `ExtConfig` trait | Supported | Associated types lifted into type parameters |
| `QuadraticExt<C>`, `CubicExt<C>` | Supported | Generic structs, pure arithmetic |
| Montgomery internals | Supported | Safe u64/u128 arithmetic |
| `gf2-kernels-simd` (unsafe) | Not supported | Use Kani instead |
| `bitvec`/`BitMatrix` | Needs assessment | Array indexing needs bounds proofs |
| `rayon` parallel code | Not supported | Out of scope |

## Track 1: Aeneas/Lean4 Proofs

### Infrastructure (Task 1) — COMPLETED

1. ✅ Patched Charon (base `24a17b5e` + 3 HRTB/associated-type fixes) + Aeneas (rev `c23de93`) installed
2. ✅ `proofs/` directory with `lakefile.lean`, `lean-toolchain` (v4.28.0-rc1), Aeneas + Mathlib deps
3. ✅ Charon extraction: `charon cargo --preset aeneas` → `target/charon/gf2_core.llbc`
4. ✅ Aeneas translation: `aeneas -backend lean -split-files` → `proofs/Gf2Core/*.lean`
5. ✅ Translation succeeds for both `gfp/` and `gfpn/` modules
6. ✅ Workarounds documented in `proofs/WORKAROUNDS.md`
7. ✅ CI job: full Charon→Aeneas→`lake build` pipeline with patched Charon and drift check
8. ✅ `lake build` compiles all generated Lean code without errors

**Const generics**: `Fp<const P: u64>` extracts correctly — no monomorphization needed.

**HRTB (resolved)**: Three Charon patches fix HRTB-related extraction failures for `gfpn/`.
Patches exported to `patches/charon-hrtb-assoc-types.patch` and applied in CI.
See `dev/lean4-verification-pipeline.md` for details.

**Duplicate field name workaround**: Aeneas generates duplicate field names when traits
have bounds on multiple associated types. Fixed by `scripts/fix-aeneas-dupes.py` (includes
multi-line block detection for gfpn and projection path fixup).

### Fp<P> Field Proofs (Task 2)

- Prove `Fp<P>` satisfies Mathlib `CommRing` / `Field` typeclass (all field axioms for any prime P)
- Prove Montgomery roundtrip: `from_mont(to_mont(a)) = a` for arbitrary P
- Prove `redc` correctly computes `t * R^(-1) mod P`
- Proofs live in `proofs/GF2Core/Proofs/FpField.lean` and `MontgomeryRoundtrip.lean`

### Extension Field Proofs (Task 3)

- Prove `QuadraticExt<C>` satisfies `CommRing` / `Field`
- Prove Karatsuba multiplication equals schoolbook multiplication (likely solvable by `ring` tactic)
- Prove norm-based inversion correctness: `a * inv(a) = 1` for non-zero `a`
- Prove `CubicExt<C>` field axioms and Karatsuba-style 6-mul equivalence similarly
- Prove `order() = base_order^degree` for both extension types
- Proofs in `proofs/GF2Core/Proofs/KaratsubaEquiv.lean` and `ExtFieldAxioms.lean`

## Track 2: Kani Bounded Verification (Task 4)

- Add `kani` as feature-gated dev dependency
- Add `#[cfg(kani)]` proof harnesses in existing source files:
  - `gfp/mod.rs`: Montgomery representation bounds (intermediates fit in `u128`), `max_unreduced_additions` overflow safety
  - `gfp/montgomery.rs`: `redc` no-overflow, roundtrip for all values mod small primes
  - `gfpn/quadratic.rs`: Karatsuba == naive for all elements of GF(7^2)
  - `bitvec.rs`: Tail masking invariant for all lengths 0..128
- Add CI job: `cargo kani`

## File Layout

```
proofs/
├── lakefile.lean
├── lean-toolchain
├── GF2Core/
│   ├── Gfp.lean                # Auto-generated by Aeneas
│   ├── Gfpn.lean               # Auto-generated by Aeneas
│   └── Proofs/
│       ├── FpField.lean
│       ├── MontgomeryRoundtrip.lean
│       ├── KaratsubaEquiv.lean
│       └── ExtFieldAxioms.lean
crates/gf2-core/src/
├── gfp/mod.rs                  # #[cfg(kani)] blocks added
├── gfp/montgomery.rs           # #[cfg(kani)] blocks added
├── gfpn/quadratic.rs           # #[cfg(kani)] blocks added
└── bitvec.rs                   # #[cfg(kani)] blocks added
```

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Charon can't handle const generics | Test in Task 1; fall back to monomorphized wrappers |
| Aeneas generates unreadable Lean | Only write proofs against generated API |
| Mathlib version churn | Pin in `lean-toolchain` |
| u128 overflow semantics in Lean | Route to Kani track instead |
| CI time increase | Lean and Kani in separate parallel CI jobs |

## Success Criteria

- `lake build` compiles auto-generated Lean code and all proofs without errors
- `cargo kani` passes all proof harnesses
- CI runs both tracks in parallel, gating merges on both
