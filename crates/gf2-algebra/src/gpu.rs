//! HIP/ROCm host-side dispatcher for the GPU permanent kernels.
//!
//! Will host the thin Rust handles that drive
//! `gf2-kernels-hip::permanent::*` device kernels (gfx1030+) per the
//! epic design §11. Compiled only when the `hip` Cargo feature is
//! enabled (off by default; requires hipcc + ROCm and a gfx1030-class
//! GPU on the host).
//!
//! # Status
//!
//! W1-T1 skeleton — empty placeholder. Body lands in W5.
