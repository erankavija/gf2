//! Makes the raw-receipt harness's parser and schedule tests runnable under
//! `cargo test`. A `harness = false` benchmark is otherwise executed as a
//! benchmark program rather than compiled with its `#[cfg(test)]` module.

#![allow(dead_code)] // The included benchmark contains its production entry point too.

#[path = "../benches/batched_f3_permanent.rs"]
mod batched_f3_permanent;
