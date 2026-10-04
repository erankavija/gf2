//! GF(p^n) extension-field arithmetic in two forms: compile-time tower
//! constructions ([`QuadraticExt`], [`CubicExt`]), each defined by an
//! irreducible polynomial specified via [`ExtConfig`], and polynomial
//! quotient fields of arbitrary degree in [`quotient`], whose modulus is
//! certificate-validated at runtime or fixed by the type.

pub mod batch;
mod cubic;
mod ext_config;
mod quadratic;
pub mod quotient;

pub use batch::{BatchExtField, SimdKaratsubaHook};
pub use cubic::{CubicExt, CubicExtWide};
pub use ext_config::ExtConfig;
pub use quadratic::{QuadraticExt, QuadraticExtWide};
pub use quotient::{
    ConstQuotient, ConstQuotientConfig, ConstQuotientExt, QuotientElement, QuotientExt,
    QuotientField,
};
