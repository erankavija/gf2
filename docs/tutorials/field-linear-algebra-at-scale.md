# Finite-field linear algebra at scale

This tutorial runs dense linear algebra over GF(2^31 − 1) and its quadratic
extension with `gf2-core`'s `FieldMatrix`: multiplication, PLE decomposition,
solving, inversion and the characteristic polynomial. Every result is checked
against an independent identity, so a run that exits zero is a verified
computation. The code is the example program
[`field_linear_algebra.rs`](../../crates/gf2-core/examples/field_linear_algebra.rs);
[`field_linear_algebra_example.rs`](../../crates/gf2-core/tests/field_linear_algebra_example.rs)
runs it at small dimensions in the fast test tier.

## Run it

```text
./scripts/cargo-budget.sh cargo run --release -p gf2-core --example field_linear_algebra -- \
    --n 1024 --charpoly-n 512 --seed 2
```

`--n` sizes the product, PLE, solve and inverse operands (default 512),
`--charpoly-n` the characteristic-polynomial operand (default 256), and
`--seed` the `StdRng` stream (default 1). The program prints one line per
field and exits non-zero naming the first identity that fails:

```text
GF(2^31 - 1): n = 1024, rank 1024, charpoly degree 512: all identities hold in <elapsed>
GF((2^31 - 1)^2): n = 1024, rank 1024, charpoly degree 512: all identities hold in <elapsed>
```

Each line ends with the host's wall-clock time for that field, an
unbenchmarked observation; performance claims follow
`@/inv/benchmark-backed-performance`.

## Choose the field representation

`FieldMatrix<F>` and its algorithms are generic over `FiniteField`; the
constructors `zeros`, `identity` and `random` additionally require
`ConstField`, a `Copy` carrier whose field is fixed by its type. The
representation is therefore a type choice:

- `Fp<P>` is GF(P) for a const prime `P ≤ 2^63`. Mersenne `2^n − 1`
  (`n ≥ 31`) and Proth `k·2^n + 1` (`n ≥ 24`) primes are stored canonically
  with specialised reduction; other primes are stored in Montgomery form.
  `GoldilocksFp` covers `2^64 − 2^32 + 1`.
- `QuadraticExt<C>` and `CubicExt<C>` build GF(p^2) and GF(p^3) from an
  `ExtConfig` that names the base field and a non-residue β; they nest into
  towers.
- `ConstQuotient` fixes an arbitrary-degree polynomial quotient in the type;
  `QuotientElement` and `Gf2mElement` carry a runtime modulus and run the
  same generic algorithms without the `ConstField` constructors.

The example declares a Mersenne prime field and its quadratic extension by
u² = −1, which is irreducible because p ≡ 3 (mod 4):

```rust
type Mersenne31 = Fp<2_147_483_647>;

/// GF(p^2) = GF(p)[u]/(u^2 + 1); -1 is a non-residue because p = 3 mod 4.
struct MinusOne;
impl ExtConfig for MinusOne {
    type BaseField = Mersenne31;
    const NON_RESIDUE: Mersenne31 = Mersenne31::new(2_147_483_646);
}
type Mersenne31Sq = QuadraticExt<MinusOne>;
```

One generic function `check_field<F: ConstField>` then serves both fields;
only the element sampler differs:

```rust
check_field::<Mersenne31>("GF(2^31 - 1)", params, || rng.gen())?;
check_field::<Mersenne31Sq>("GF((2^31 - 1)^2)", params, || {
    Mersenne31Sq::new(rng.gen(), rng.gen())
})?;
```

## Build large matrices

Rows are `FieldVec<F>` values and `FieldMatrix::from_rows` assembles them
into row-major storage:

```rust
let mut vector = |len: usize| (0..len).map(|_| sample()).collect::<FieldVec<F>>();
let mut matrix = |n: usize| FieldMatrix::from_rows((0..n).map(|_| vector(n)).collect());
```

## Multiply

`gemm` is cache-blocked and accumulates each dot product in the field's
`Wide` type, reducing once per `F::max_unreduced_additions()` terms;
extension towers inherit the bound of their prime base. A Freivalds probe
verifies the product in O(n²): a wrong product survives a uniform probe `x`
with probability at most 1/|F|.

```rust
let ab = gemm(&a, &b);
ensure(
    ab.matvec(&x) == a.matvec(&b.matvec(&x)),
    "(A B) x = A (B x)",
)?;
```

## Decompose, solve and invert

`ple` returns a row permutation `P`, a unit lower-trapezoidal `L`, a
row-echelon `E` and the rank, with `P · L · E = A` for any shape and rank.
`solve` and `inv` are built on it: both return `None` exactly when the square
input is rank-deficient, so the check branches on the result.

```rust
let (p, l, e, rank) = a.ple();
ensure(p.apply(&gemm(&l, &e)) == a, "P L E = A")?;

let rhs = a.matvec(&x);
match a.inv() {
    Some(a_inv) => {
        ensure(rank == n, "invertible A has full rank")?;
        ensure(gemm(&a, &a_inv) == FieldMatrix::identity(n), "A A^-1 = I")?;
        let y = a.solve(&rhs).ok_or(format!("{name}: solve failed"))?;
        ensure(a.matvec(&y) == rhs, "A y - b = 0")?;
    }
    None => ensure(rank < n, "singular A is rank-deficient")?,
}
```

For rank-deficient systems, compose `row_echelon` and `nullspace` from the
same PLE module.

## Characteristic polynomial

`charpoly` returns det(xI − C) as a `FieldPoly<F>`. Three coefficients have
closed forms that `trace` and `det` check independently: the polynomial is
monic of degree k, its x^(k−1) coefficient is −tr(C), and its constant term
is (−1)^k det(C).

```rust
let k = cp.rows();
let chi = cp.charpoly();
let sign = if k % 2 == 0 { F::one() } else { -F::one() };
ensure(
    chi.degree() == Some(k) && chi.coeff(k).is_one(),
    "charpoly is monic of degree k",
)?;
ensure(
    k == 0 || chi.coeff(k - 1) == -cp.trace(),
    "charpoly trace coefficient",
)?;
ensure(
    chi.coeff(0) == sign * cp.det(),
    "charpoly determinant coefficient",
)?;
```

## Adapt it

Replace `Mersenne31` with another `Fp<P>` or `ExtConfig` declaration and
supply a matching sampler; `check_field` is unchanged. API contracts, panics
and complexity for each operation are in the `gf2_core::field::matrix`
rustdoc
(`./scripts/cargo-budget.sh cargo doc -p gf2-core --no-deps`).
