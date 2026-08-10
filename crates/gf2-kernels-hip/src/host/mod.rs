//! Host-side HIP infrastructure for the `gf2-sim` pipeline (design doc §6).
//!
//! This module stands up the host plumbing the GPU pipeline stages and the
//! Phase C executor build on:
//!
//! | Submodule | Contents |
//! |-----------|----------|
//! | [`streams`] | [`HipStream`] (RAII over `hipStream_t`) and [`HipStreamPool`] |
//! | [`events`] | [`HipEvent`] and [`HipEventSpan`] timing resources |
//! | [`alloc`]   | [`DeviceBuffer<T>`] and [`PinnedHostBuffer<T>`] |
//! | [`launch`]  | deterministic kernel-launch helpers (fixed grid/block) |
//! | [`arch`]    | [`GfxTarget`] enum + runtime [`GfxTarget::detect`] |
//!
//! All `unsafe` FFI is encapsulated behind safe wrappers here; each call site
//! carries a `// SAFETY:` comment, satisfying the kernel-crate isolation rule.
//!
//! [`HipStream`]: streams::HipStream
//! [`HipStreamPool`]: streams::HipStreamPool
//! [`HipEvent`]: events::HipEvent
//! [`HipEventSpan`]: events::HipEventSpan
//! [`DeviceBuffer<T>`]: alloc::DeviceBuffer
//! [`PinnedHostBuffer<T>`]: alloc::PinnedHostBuffer
//! [`GfxTarget`]: arch::GfxTarget
//! [`GfxTarget::detect`]: arch::GfxTarget::detect

pub mod alloc;
pub mod arch;
pub mod events;
pub mod launch;
pub mod streams;

pub use alloc::{device_mem_info, device_mem_info_for, DeviceBuffer, PinnedHostBuffer};
pub use arch::GfxTarget;
pub use events::{HipEvent, HipEventSpan};
pub use launch::{LaunchDims, MAX_BLOCK_THREADS};
pub use streams::{HipStream, HipStreamPool};
