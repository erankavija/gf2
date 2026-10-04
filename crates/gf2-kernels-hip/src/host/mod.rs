//! Host-side HIP infrastructure: streams ([`streams`]), timing events
//! ([`events`]), device and pinned buffers ([`alloc`]), fixed launch geometry
//! ([`launch`]), and gfx target detection ([`arch`]). Every `unsafe` FFI call
//! sits behind a safe wrapper with a `// SAFETY:` comment.

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
