# Non-Residue Specification Pattern for Tower Extensions

> Design plan for jit issue 0203bebd
> Parent story: 3f4b946c (Tower extension field architecture for GF(p^n))

## Decision

Use a **config trait** (arkworks-style) to specify the irreducible polynomial at each
tower level. This is zero-cost, handles nested towers naturally, and is the industry
standard (arkworks, blstrs, halo2).

Rejected alternatives:
- **Const generic** (`QuadraticExt<F, const BETA: u64>`): Dead end for nested towers
  where the non-residue is itself an extension field element.
- **Runtime** (store non-residue in each element): Overhead per element, breaks `ConstField`.

## Design

### Core trait

```rust
/// Configuration specifying the irreducible polynomial for a field extension.
///
/// For `QuadraticExt`: defines β such that u² = β (a quadratic non-residue in F).
/// For `CubicExt`: defines β such that v³ = β (a cubic non-residue in F).
pub trait ExtConfig: 'static {
    /// The base field being extended.
    type BaseField: ConstField;

    /// The non-residue β defining the extension polynomial.
    ///
    /// For quadratic extensions: the irreducible polynomial is x² - β.
    /// For cubic extensions: the irreducible polynomial is x³ - β.
    const NON_RESIDUE: Self::BaseField;

    /// Multiply a base field element by the non-residue β.
    ///
    /// Default implementation uses generic multiplication, but specific configs
    /// can override for efficiency (e.g., when β = -1, this is just negation).
    #[inline]
    fn mul_by_non_residue(x: Self::BaseField) -> Self::BaseField {
        x * Self::NON_RESIDUE
    }
}
```

### Extension types

```rust
/// Quadratic extension: elements are c0 + c1·u where u² = β.
///
/// Uses Karatsuba multiplication (3 base-field muls instead of 4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct QuadraticExt<C: ExtConfig> {
    pub(crate) c0: C::BaseField,
    pub(crate) c1: C::BaseField,
}

/// Cubic extension: elements are c0 + c1·v + c2·v² where v³ = β.
///
/// Uses Toom-Cook multiplication (5 base-field muls instead of 9).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CubicExt<C: ExtConfig> {
    pub(crate) c0: C::BaseField,
    pub(crate) c1: C::BaseField,
    pub(crate) c2: C::BaseField,
}
```

### Key design points

1. **Config is a type, not a value.** `QuadraticExt<C>` is parameterized by the config
   type `C`, not by the base field. The base field is `C::BaseField`. This means two
   different extensions of the same base field are distinct types.

2. **`NON_RESIDUE` is a const.** This requires `ConstField` on the base field, which
   `Fp<P>` already implements. For nested towers, `QuadraticExt<C>` must also implement
   `ConstField` so it can serve as a base field for the next level.

3. **`mul_by_non_residue` override.** Many useful non-residues have cheap multiplication:
   - β = -1: just negation (GF(p²) with x² + 1)
   - β = small constant: shift-and-add
   - β from a lower tower level: exploit structure
   This hook lets configs specialize without touching the extension type.

4. **`'static` bound on `ExtConfig`.** Configs are zero-sized marker types with no
   runtime state. The `'static` bound ensures they can be used in const contexts.

### Nested tower example (BLS12-381 style)

```rust
// Level 0: base prime field
// type Fq = Fp<BLS12_381_P>;  // (tracked in separate issue)

// Level 1: GF(p²) via x² + 1
struct Fq2Config;
impl ExtConfig for Fq2Config {
    type BaseField = Fp<7>;  // small prime for illustration
    const NON_RESIDUE: Fp<7> = Fp::<7>::new(7 - 1);  // β = -1
    #[inline]
    fn mul_by_non_residue(x: Fp<7>) -> Fp<7> { -x }  // fast path
}
type Fq2 = QuadraticExt<Fq2Config>;

// Level 2: GF(p⁶) via v³ = (u + 1)
struct Fq6Config;
impl ExtConfig for Fq6Config {
    type BaseField = Fq2;  // base is the quadratic extension
    const NON_RESIDUE: Fq2 = Fq2 { c0: Fp::<7>::ONE, c1: Fp::<7>::ONE };
}
type Fq6 = CubicExt<Fq6Config>;

// Level 3: GF(p¹²) via w² = v
struct Fq12Config;
impl ExtConfig for Fq12Config {
    type BaseField = Fq6;
    const NON_RESIDUE: Fq6 = /* v as element of Fq6 */;
}
type Fq12 = QuadraticExt<Fq12Config>;
```

### FiniteField and ConstField implementation

`QuadraticExt<C>` implements `FiniteField` with:
- `type Characteristic = <C::BaseField as FiniteField>::Characteristic`
- `type Wide = (C::BaseField::Wide, C::BaseField::Wide)` (or a dedicated struct)
- `extension_degree() = 2 * C::BaseField::extension_degree()`
- `inv()` via norm-based inversion: `a⁻¹ = conjugate(a) / norm(a)`

`QuadraticExt<C>` implements `ConstField` (since `C::BaseField: ConstField + Copy`):
- `zero() = QuadraticExt { c0: zero, c1: zero }`
- `one() = QuadraticExt { c0: one, c1: zero }`
- `order() = base_order²`

Same pattern for `CubicExt<C>`.

### Const initialization challenge

`const NON_RESIDUE: Self::BaseField` requires that `BaseField` values can be constructed
in const context. For `Fp<P>`, this means `Fp::new()` must work in const — currently it
does since Montgomery conversion uses `const fn` helpers. For nested towers,
`QuadraticExt` construction must also be const-compatible, which it is since it's just
struct initialization of `Copy` fields.

If const construction proves problematic for complex nested types, fallback to:
```rust
fn non_residue() -> Self::BaseField;  // fn instead of const
```
This still compiles to a constant with `#[inline]` since the implementation returns
a literal, but avoids const-evaluation limitations.

## File layout

```
crates/gf2-core/src/gfpn/
├── mod.rs           — module root, re-exports
├── ext_config.rs    — ExtConfig trait
├── quadratic.rs     — QuadraticExt<C> + FiniteField/ConstField impls
└── cubic.rs         — CubicExt<C> + FiniteField/ConstField impls
```

## Implementation order

1. **This issue (0203bebd)**: `ext_config.rs` with `ExtConfig` trait only. Unit tests
   verifying a concrete config compiles and `mul_by_non_residue` works.
2. **8d2863e6**: `QuadraticExt<C>` with Karatsuba mul, `FiniteField` + `ConstField` impls.
3. **3f3d6c23**: `CubicExt<C>` with Toom-Cook mul.
4. **d11b769a**: Wide accumulator integration.
5. **86b3dc7d**: Nested tower verification.
6. **2ce2a757**: Cross-verification proptests.
