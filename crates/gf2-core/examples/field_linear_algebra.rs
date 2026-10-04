//! Dense linear algebra with `FieldMatrix` over GF(2^31 - 1) and its quadratic
//! extension: seeded random matrices are multiplied, decomposed through PLE,
//! solved, inverted and reduced to a characteristic polynomial, and every
//! result is checked against an independent identity; a mismatch exits non-zero.

use std::time::Instant;

use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::{ConstField, FieldVec};
use gf2_core::gfp::Fp;
use gf2_core::gfpn::{ExtConfig, QuadraticExt};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

type Mersenne31 = Fp<2_147_483_647>;

/// GF(p^2) = GF(p)[u]/(u^2 + 1); -1 is a non-residue because p = 3 mod 4.
struct MinusOne;
impl ExtConfig for MinusOne {
    type BaseField = Mersenne31;
    const NON_RESIDUE: Mersenne31 = Mersenne31::new(2_147_483_646);
}
type Mersenne31Sq = QuadraticExt<MinusOne>;

/// Command-line parameters: `--n` sizes the product, PLE, solve and inverse
/// operands; `--charpoly-n` sizes the characteristic-polynomial operand.
pub struct Params {
    pub n: usize,
    pub charpoly_n: usize,
    pub seed: u64,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Params> {
    let mut params = Params {
        n: 512,
        charpoly_n: 256,
        seed: 1,
    };
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--n" => params.n = value.parse()?,
            "--charpoly-n" => params.charpoly_n = value.parse()?,
            "--seed" => params.seed = value.parse()?,
            _ => return Err(format!("unknown flag {flag}").into()),
        }
    }
    Ok(params)
}

pub fn run(params: &Params) -> Result<()> {
    let mut rng = StdRng::seed_from_u64(params.seed);
    check_field::<Mersenne31>("GF(2^31 - 1)", params, || rng.gen())?;
    check_field::<Mersenne31Sq>("GF((2^31 - 1)^2)", params, || {
        Mersenne31Sq::new(rng.gen(), rng.gen())
    })?;
    Ok(())
}

fn check_field<F: ConstField>(
    name: &str,
    params: &Params,
    mut sample: impl FnMut() -> F,
) -> Result<()> {
    let ensure = |holds: bool, identity: &str| -> Result<()> {
        if holds {
            Ok(())
        } else {
            Err(format!("{name}: {identity} does not hold").into())
        }
    };
    let mut vector = |len: usize| (0..len).map(|_| sample()).collect::<FieldVec<F>>();
    let mut matrix = |n: usize| FieldMatrix::from_rows((0..n).map(|_| vector(n)).collect());
    let n = params.n;
    let start = Instant::now();
    let a = matrix(n);
    let b = matrix(n);
    let cp = matrix(params.charpoly_n);
    let x = vector(n);

    // Freivalds: a wrong product survives a random probe with probability at
    // most 1/|F|.
    let ab = gemm(&a, &b);
    ensure(
        ab.matvec(&x) == a.matvec(&b.matvec(&x)),
        "(A B) x = A (B x)",
    )?;

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

    // det(xI - C) has x^(k-1) coefficient -tr(C) and constant (-1)^k det(C).
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

    println!(
        "{name}: n = {n}, rank {rank}, charpoly degree {k}: all identities hold in {:.2?}",
        start.elapsed()
    );
    Ok(())
}

fn main() -> Result<()> {
    run(&parse_args(std::env::args().skip(1))?)
}
