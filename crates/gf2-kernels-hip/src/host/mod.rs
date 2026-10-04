//! Host-side HIP infrastructure: [`HipStream`] and [`HipStreamPool`]
//! ([`streams`]), [`HipEvent`] and [`HipEventSpan`] timing resources
//! ([`events`]), [`DeviceBuffer<T>`] and [`PinnedHostBuffer<T>`] ([`alloc`]),
//! fixed launch geometry ([`launch`]), and [`GfxTarget`] detection ([`arch`]).
//! Every `unsafe` FFI call sits behind a safe wrapper with a `// SAFETY:`
//! comment.
//!
//! [`HipStream`]: streams::HipStream
//! [`HipStreamPool`]: streams::HipStreamPool
//! [`HipEvent`]: events::HipEvent
//! [`HipEventSpan`]: events::HipEventSpan
//! [`DeviceBuffer<T>`]: alloc::DeviceBuffer
//! [`PinnedHostBuffer<T>`]: alloc::PinnedHostBuffer
//! [`GfxTarget`]: arch::GfxTarget

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
