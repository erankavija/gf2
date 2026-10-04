//! Standalone Charon/Aeneas probe for JIT issue 34d85cb9.
//!
//! Three copies of the same square-and-multiply loop taken from
//! `gf2_core::field::traits::FiniteFieldExt::pow`, differing only in how the
//! element type is abstracted:
//!
//! * `pow_concrete` — no genericity at all (`u64`).
//! * `pow_simple`   — generic over a trait with no associated types.
//! * `pow_assoc`    — generic over a trait with two associated types, i.e. the
//!   shape of `FiniteField { type Characteristic; type Wide; }`.
//!
//! Extracted with `charon rustc` so the probe is independent of the workspace.

pub trait Simple: Clone {
    fn mul_ref(&self, other: &Self) -> Self;
    fn one_like(&self) -> Self;
}

pub trait WithAssoc: Clone {
    type Ch;
    type Wide;
    fn mul_ref(&self, other: &Self) -> Self;
    fn one_like(&self) -> Self;
}

pub fn pow_concrete(x: u64, exp: u64) -> u64 {
    if exp == 0 {
        return 1;
    }
    let mut result = 1u64;
    let mut base = x;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(base);
        }
        e >>= 1;
        if e > 0 {
            base = base.wrapping_mul(base);
        }
    }
    result
}

pub fn pow_simple<F: Simple>(x: &F, exp: u64) -> F {
    if exp == 0 {
        return x.one_like();
    }
    let mut result = x.one_like();
    let mut base = x.clone();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.mul_ref(&base);
        }
        e >>= 1;
        if e > 0 {
            base = base.mul_ref(&base);
        }
    }
    result
}

pub fn pow_assoc<F: WithAssoc>(x: &F, exp: u64) -> F {
    if exp == 0 {
        return x.one_like();
    }
    let mut result = x.one_like();
    let mut base = x.clone();
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.mul_ref(&base);
        }
        e >>= 1;
        if e > 0 {
            base = base.mul_ref(&base);
        }
    }
    result
}

/// Same loop again, this time as a *provided (default) method* on an
/// extension trait whose supertrait carries the two associated types — the
/// exact shape of `FiniteFieldExt: FiniteField` in `gf2-core`.
pub trait WithAssocExt: WithAssoc {
    fn pow_default(&self, exp: u64) -> Self {
        if exp == 0 {
            return self.one_like();
        }
        let mut result = self.one_like();
        let mut base = self.clone();
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul_ref(&base);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul_ref(&base);
            }
        }
        result
    }
}

impl<T: WithAssoc> WithAssocExt for T {}

/// Third variant: the associated types now appear in the trait's own method
/// signatures and carry bounds, as `FiniteField::{Characteristic, Wide}` do.
pub trait WithAssoc2: Clone {
    type Ch: Clone;
    type Wide: Clone;
    fn ch(&self) -> Self::Ch;
    fn to_wide(&self) -> Self::Wide;
    fn reduce_wide(w: &Self::Wide) -> Self;
    fn mul_ref(&self, other: &Self) -> Self;
    fn one_like(&self) -> Self;
}

pub trait WithAssoc2Ext: WithAssoc2 {
    fn pow_default(&self, exp: u64) -> Self {
        if exp == 0 {
            return self.one_like();
        }
        let mut result = self.one_like();
        let mut base = self.clone();
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul_ref(&base);
            }
            e >>= 1;
            if e > 0 {
                base = base.mul_ref(&base);
            }
        }
        result
    }
}

impl<T: WithAssoc2> WithAssoc2Ext for T {}
