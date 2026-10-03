# Finite-field arithmetic

`gf2-core` implements each field family with a representation chosen for
when its parameters become known: at compile time through const generics
and zero-sized configuration types, or at run time through a shared field
handle. Every scalar element type implements `gf2_core::field::FiniteField`,
so polynomial, matrix and coding code is written once over that trait.

## Choosing a field type

| Field | Type | Parameters fixed | Applies when |
|---|---|---|---|
| $\mathrm{GF}(2)$, vectors and matrices | `BitVec`, `BitSlice`, `BitMatrix`, `SpBitMatrix` | n/a | Binary linear algebra and codes over $\mathrm{GF}(2)$ |
| $\mathrm{GF}(2)$, scalar | `gfp::Fp<2>` | compile time | A generic algorithm needs $\mathrm{GF}(2)$ as a `FiniteField` |
| $\mathrm{GF}(2^m)$, single word | `gf2m::Gf2mField_<V>`, `Gf2mElement_<V>` | run time | $m$ or the polynomial is a runtime value, with $m <$ `V::BITS` |
| $\mathrm{GF}(2^m)$, multi-word | `gf2m::Gf2mWide<N, Cfg>` | compile time | $m$ and the polynomial are constants, including $m > 127$ |
| $\mathrm{GF}(p)$ | `gfp::Fp<P>` | compile time | Prime $P$ with $1 < P \le 2^{63}$ |
| $\mathrm{GF}(p)$, Goldilocks | `gfp::specialized::GoldilocksFp` | compile time | $P = 2^{64} - 2^{32} + 1$, outside the `Fp<P>` bound |
| $\mathrm{GF}(p^m)$, towers | `gfpn::QuadraticExt<C>`, `gfpn::CubicExt<C>` | compile time | Degree 2 and 3 steps defined by a binomial $x^r - \beta$, nested to build $\mathrm{GF}(p^4)$, $\mathrm{GF}(p^6)$ and so on |
| $\mathrm{GF}(q^d)$, any degree | `gfpn::QuotientField`, `gfpn::QuotientElement` | run time | Base field and modulus are runtime values; the base may itself be an extension |
| $\mathrm{GF}(q^d)$, any degree | `gfpn::ConstQuotient<R, C>` | compile time | The same quotient with base, degree and modulus fixed by a type |

`gf2-algebra` adds packed elements, vectors and matrices over
$\mathbb{F}_3$, $\mathbb{F}_5$ and $\mathbb{F}_7$ for its permanent
algorithms, each implementing `PackedField<Fp<P>>`. `Bipedal3` holds 64
$\mathbb{F}_3$ lanes in two `u64` bit-planes, `Packed5` holds 64
$\mathbb{F}_5$ lanes in three bit-planes, and `Packed7` holds 16
$\mathbb{F}_7$ lanes in 4-bit slots of one `u64`, combined through lookup
tables.

## $\mathrm{GF}(2)$

Bit vectors and matrices pack $\mathrm{GF}(2)$ elements into `u64` words
under `@/inv/canonical-bit-indexing`: bit $i$ lives in word `i >> 6` at mask
`1u64 << (i & 63)`, and padding bits above the length are zero.

`Fp<2>` stores one bit and uses `^` and `&`. It is the base field of
`field::extension::BinaryPrimeExt`, which witnesses
$\mathrm{GF}(2) \subset \mathrm{GF}(2^m)$ for a runtime binary field.

## $\mathrm{GF}(2^m)$

Elements and defining polynomials are stored as integers whose bit $i$ is
the coefficient of $x^i$. The single-word type keeps its leading term
explicit at bit $m$, so $x^4 + x + 1$ is `0b10011`. The multi-word
configuration stores only the low $M$ bits and leaves the leading
coefficient implicit.

### Single-word runtime fields

`Gf2mField_<V>` is generic over the sealed `UintExt` storage trait (`u8`
through `u128`); `Gf2mField` is the `u64` alias. A field holds its
parameters behind an `Arc`, and every element carries a clone of that
handle. Arithmetic asserts that both operands share one handle, so elements
of two separately constructed fields do not combine even when $m$ and the
polynomial are equal: clone one `Gf2mField` and create every element from
it.

`Gf2mField::gf256()` uses `0b100011101`
($x^8 + x^4 + x^3 + x^2 + 1$) and `Gf2mField::gf65536()` uses
`0b10001000000001011` ($x^{16} + x^{12} + x^3 + x + 1$).

### Multi-word compile-time fields

`Gf2mWide<N, Cfg>` stores $N$ little-endian `u64` words and is `Copy`. The
zero-sized `Gf2mWideConfig<N>` implementor fixes $M$ and the modulus as
associated constants, with $64(N - 1) < M \le 64N$, so elements carry no
runtime parameters. The implementor guarantees irreducibility; the type
does not check it.

### Multiplication strategy

A single-word product selects its path in this order:

1. Log/antilog tables, when `with_tables()` built them. Tables exist only for
   $m \le 16$.
2. With the `simd` feature, `u64` storage and an x86 host reporting
   PCLMULQDQ and SSE4.1: a carry-less product followed by Barrett reduction.
3. Portable shift-and-reduce multiplication.

Batch and region entry points dispatch separately:

- `gf2m::batch::{batch_mul, batch_square}` serve $m \in \{8, 16, 32\}$,
  with a vector kernel on hosts reporting AVX2, VPCLMULQDQ and SSE4.1 and a
  scalar Barrett path otherwise; both produce identical results.
- $\mathrm{GF}(2^8)$ AXPY and classical GEMM over a single-word
  representation (`u64`-backed `Gf2mElement` or `Gf2mWide<1, Cfg>`) read a
  process-wide $256 \times 256$ byte product table, built once per
  reduction polynomial. No feature or CPU capability takes part.
- `Gf2mWide` products all pass through one carry-less dispatch whose
  predicate is stated in the rustdoc of
  [`gf2m/wide.rs`](../../crates/gf2-core/src/gf2m/wide.rs); widths without
  a kernel use the portable `clmul_wide_slice_portable`.

The suites
[`gf256_table_conformance.rs`](../../crates/gf2-core/tests/gf256_table_conformance.rs)
and
[`clmul_wide_conformance.rs`](../../crates/gf2-core/tests/clmul_wide_conformance.rs)
hold each dispatched path to the portable result.

## $\mathrm{GF}(p)$

`Fp<P>` exposes canonical values in $[0, P)$ while choosing its storage
form at compile time from `gfp::specialized::classify(P)`:

- Canonical storage for Mersenne primes $2^n - 1$ with $n \ge 31$ and Proth
  primes $k \cdot 2^n + 1$ with $n \ge 24$, reduced by shape-specific
  reducers.
- Montgomery form $aR \bmod P$, $R = 2^{64}$, for every other odd prime;
  `new` and `value` convert in and out.
- Bitwise storage for $P = 2$.

Primality of $P$ is the caller's obligation. Element-wise vector operations
on `field::FieldVec` consult `gfp::SimdVecOps`, which routes eligible primes
to AVX2 kernels under the `simd` feature and returns to the scalar loop
otherwise. Primes that implement `field::TwoAdicField` additionally support
the radix-2 NTT behind `FieldPoly::mul_ntt`.

## $\mathrm{GF}(p^m)$

`QuadraticExt<C>` represents $c_0 + c_1 u$ with $u^2 = \beta$, and
`CubicExt<C>` represents $c_0 + c_1 v + c_2 v^2$ with $v^3 = \beta$, where
the `ExtConfig` implementor `C` names the base field and the non-residue
$\beta$. Towers nest these types. The quadratic product uses the
three-multiplication Karatsuba form. $\beta$ is declared, not decided, so
`field::extension::ConstExt` records a `Declared` certificate basis; a
caller that proves $x^r - \beta$ irreducible with `field::prove_irreducible`
can supply the stronger certificate.

`QuotientField` represents $B[x]/(f)$ for any base $B$ implementing
`FieldIdentity` and any monic $f$. Elements store $\deg f$ coefficients in
ascending order. `QuotientField::new` decides irreducibility of $f$;
`from_certificate` reuses a held certificate; and
`from_certificate_unchecked` trusts it. `ConstQuotient<R, C>` presents the
same field with the modulus as an associated constant, and both forms share
one `FieldId`, so `convert_element` between them is the identity on
canonical coordinates.

For the batched case, `gfpn::BatchExtField` stores many extension elements
in a structure-of-arrays layout, so each extension product reduces to
base-field vector operations.

## Defining polynomials

Arithmetic in $\mathrm{GF}(2^m)$ is correct for any irreducible defining
polynomial. Primitivity, where $x$ itself generates the multiplicative
group, matters when an application identifies $\alpha$ with $x$, as BCH and
Reed–Solomon root sets do. `with_tables()` searches for a generator starting
at $x$ and builds its tables from the first one found;
`primitive_element()` returns it. Standards-defined codes use the
polynomial their standard fixes; `gf2_coding::bch::dvb_t2::DvbBchParams`
carries the DVB-T2 BCH field polynomials of `@/citation/Etsi2015`.

### Validation

`Gf2mField_::new` takes irreducibility on trust. Each check below is opt-in:

- `field::extension::BinaryPrimeExt::new` decides irreducibility with
  `field::prove_irreducible` and fails with a factor witness for a reducible
  polynomial.
- `Gf2mField::verify_primitive` tests irreducibility (Rabin's criterion)
  and then checks $x^{(2^m - 1)/q} \ne 1$ for each prime factor $q$ of
  $2^m - 1$.
- `Gf2mField::new_verified` compares the polynomial with the database entry
  for $m$ and warns on stderr when they differ.

### Polynomial database

`primitive_polys::PrimitivePolynomialDatabase` returns polynomials in the
single-word encoding. Its guarantee depends on the accessor and degree, and
the module rustdoc of
[`primitive_polys.rs`](../../crates/gf2-core/src/primitive_polys.rs) states
the covered degrees, each entry's source and the test that verifies it:

- `standard(m)` entries are primitive; its degree-32 entry is the Conway
  polynomial.
- `standard_u128(m)` extends the catalogue into `u128` degrees with entries
  verified irreducible only. A caller that needs primitivity there verifies
  it independently.
- `verify(m, poly)` returns `Matches`, `Conflict` or `Unknown` against
  `standard(m)`, and `trinomials(m)` lists catalogued primitive trinomials.

`gf2m::generation::PrimitiveGenerator` searches for primitive polynomials by
exhaustive, trinomial or pentanomial strategy.

### Deterministic modulus selection

`field::select_modulus(base, degree)` returns a monic irreducible modulus
over any base field, so selecting over an extension yields a relative
presentation. It takes the registry's Conway entry, then its other verified
entry, and otherwise enumerates candidates in a fixed rank order (constant
coefficient fastest) and returns the first that `prove_irreducible` accepts.
The order is part of the `FieldId` contract: equal base and degree always
select the same presentation. `field::modulus_select::SelectExtension`
builds a validated `QuotientField` or `BinaryPrimeExt` from the selection.

## Shared contracts

`FiniteField::Wide` is an unreduced accumulator type, and
`FiniteField::max_unreduced_additions` bounds how many products it absorbs
before `reduce_wide`. `Fp<P>` accumulates in `u128`; binary fields
accumulate in `Self`, because XOR never overflows. Extension towers inherit
their base field's bound. Dot products and matrix kernels defer reduction up
to that bound.

Each field family runs the shared law suite in
`gf2_core::field::axiom_tests` (feature `test-support`) under
`@/inv/finite-field-laws`, and every accelerated path matches its portable
result under `@/inv/backend-behavioral-equivalence`.
