//! LDPC comparison arms and shared campaign driver.

#![deny(unsafe_code)]

pub mod arm;
pub mod batch;
pub mod bundle;
pub mod driver;
pub mod pool;
pub mod tuned;
pub mod workload;

#[cfg(test)]
mod reference;
