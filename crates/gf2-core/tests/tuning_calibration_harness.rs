//! Makes the host-calibration harness's grid, selection-rule, profile-building
//! and provenance-formatting tests runnable under `cargo test`. A
//! `harness = false` benchmark is otherwise executed as a benchmark program
//! rather than compiled with its `#[cfg(test)]` module.

#![allow(dead_code)] // The included benchmark contains its production entry point too.

#[path = "../benches/tuning_calibration.rs"]
mod tuning_calibration;
