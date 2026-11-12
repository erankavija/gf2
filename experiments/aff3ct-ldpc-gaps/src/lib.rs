//! LDPC comparison arms and shared campaign driver.

#![deny(unsafe_code)]

pub mod arm;
pub mod batch;
pub mod bler_sweep;
pub mod bundle;
pub mod comparison_code;
pub mod driver;
pub mod fast;
pub mod pool;
pub mod route_sweep;
pub mod tuned;
pub mod workload;

#[cfg(test)]
mod reference;
