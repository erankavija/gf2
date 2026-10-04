//! Mirrors the `FiniteField` shape the four tuning constants live on: a trait
//! with a defaulted associated constant, a second constant defaulting to the
//! first, and one impl overriding the second.

/// Mirrors `FiniteField`.
pub trait Selector {
    /// Mirrors `WINOGRAD_THRESHOLD` / `TRI_BASE_THRESHOLD` / `PLE_BASE_COLS`.
    const THRESHOLD: usize = 128;
    /// Mirrors `PLE_PANEL_COLS`, whose default names the previous constant.
    const PANEL: usize = Self::THRESHOLD;
    /// A law-level method, so the trait carries something the driver needs.
    fn value(&self) -> u64;
}

/// Mirrors `Fp<P>`: the one carrier that overrides the panel constant.
pub struct Elem(pub u64);

impl Selector for Elem {
    const PANEL: usize = 256;
    fn value(&self) -> u64 {
        self.0
    }
}

/// Mirrors the `#[doc(hidden)]` accelerator-hook family on `FiniteField`: a
/// defaulted method that no reachable code calls and no extracted impl
/// overrides. Probe M4 contrasts its fate with the associated constants'.
pub trait Hooked {
    /// Mirrors `has_simd_ple_panel_base`.
    fn has_panel_kernel() -> bool {
        false
    }
    /// Mirrors an associated constant nothing reads.
    const UNREAD: usize = 7;
    /// The one item the driver calls.
    fn value(&self) -> u64;
}

impl Hooked for Elem {
    fn value(&self) -> u64 {
        self.0
    }
}
