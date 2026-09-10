//! Makes the pinned selector non-regression harness's cell-grid, parser,
//! pooling, and comparison tests runnable under `cargo test`. A
//! `harness = false` benchmark is otherwise executed as a benchmark program
//! rather than compiled with its `#[cfg(test)]` module.

#![allow(dead_code)] // The included benchmark contains its production entry point too.

#[path = "../benches/selector_non_regression.rs"]
mod selector_non_regression;
