//! Runs the canonical core producer's retained-threshold and extent contracts:
//! manifests, strict child evidence, scalar witnesses, joint decisions, and
//! owner artifact reopening. The benchmark has an explicit CLI entry point;
//! this adapter gives its internal contract suite a libtest entry point.

#![allow(dead_code)] // The included benchmark contains its production entry point too.

#[path = "../benches/tuning_calibration.rs"]
mod tuning_calibration;
