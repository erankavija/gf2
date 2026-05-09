//! Generic `BatchedBipedalLike<C, Mag, Sgn>` framework.
//!
//! The framework abstracts the bipedal-like `(mag, sgn)` arithmetic over
//! three type-parameters:
//!
//! 1. `C: BipedalLikeConfig` — the per-prime arithmetic recipe (paper §2.2
//!    formulas for F_3 today; F_5 D-bit-sliced and F_7 LUT-A in W4).
//! 2. `Mag: BipedalLogicalLanes` — the lane type carrying magnitude bits.
//! 3. `Sgn: BipedalLogicalLanes` — the lane type carrying sign bits.
//!
//! For F_3 both lane types are [`crate::bipedal::Avx2Lane`]. For F_5
//! D-bit-sliced (W4) the lane types may differ (three planes vs single
//! plane), and the per-prime config impl supplies the appropriate formula.
//!
//! The `BatchedBipedalLike::{add, sub, mul, neg}` methods delegate to the
//! config's `add_lane / sub_lane / mul_lane / neg_lane` recipes; this is
//! the single point of customisation when a new prime joins.
//!
//! ## Inlining contract
//!
//! All methods on this struct are `#[inline(always)]`. The `run_*_batch`
//! entry points (defined per-instantiation, see [`crate::bipedal::f3`])
//! carry `#[target_feature(enable = "avx2")]`. R4 §4.1 documents the
//! 12-34x regression that occurs without this discipline.

use core::marker::PhantomData;

use super::lanes::BipedalLogicalLanes;

/// Per-prime arithmetic recipe for the bipedal-like framework.
///
/// An impl supplies the lane-level add/sub/mul/neg formula for one
/// prime. The framework dispatches to these methods with whatever lane
/// type the kernel is instantiated over (typically [`crate::bipedal::Avx2Lane`]
/// today, future AVX-512 / AArch64 lanes once those backends land).
///
/// The methods are generic over the two lane types `Mag` and `Sgn` rather
/// than nailing them to a single type so a single config impl can drive
/// every backend.
///
/// # Safety
///
/// All methods take and return values whose construction implied a hardware
/// feature precondition; callers must already have established that
/// precondition (typically by calling through a `#[target_feature]`-attributed
/// kernel entry point).
pub trait BipedalLikeConfig {
    /// The prime characteristic this configuration encodes.
    ///
    /// Used only for documentation / debugging today; future codegen-time
    /// dispatch may use it to select between formula variants.
    const PRIME: u64;

    /// Lane-level add formula.
    ///
    /// For F_3: `t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`
    /// (Scheinerman 2024 §2.2).
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand `(mag, sgn)`.
    /// * `(m2, s2)` — second operand `(mag, sgn)`.
    ///
    /// Returns the lane-wise sum `(m_+, s_+)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` / `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops
    /// (six for F_3).
    unsafe fn add_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes;

    /// Lane-level sub formula.
    ///
    /// For F_3: `t = s1 ^ s2; u = m1 & t; m_- = u | (m1 ^ m2); s_- = u ^ (m2 ^ s2)`
    /// (Scheinerman 2024 §2.2).
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand `(mag, sgn)`.
    /// * `(m2, s2)` — second operand `(mag, sgn)`.
    ///
    /// Returns the lane-wise difference `(m_-, s_-)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` / `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn sub_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes;

    /// Lane-level mul formula.
    ///
    /// For F_3: `m_x = m1 & m2; s_x = s1 ^ s2` (Scheinerman 2024 §2.2).
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand `(mag, sgn)`.
    /// * `(m2, s2)` — second operand `(mag, sgn)`.
    ///
    /// Returns the lane-wise product `(m_x, s_x)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` / `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn mul_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes;

    /// Lane-level negation formula.
    ///
    /// For F_3 the canonical `(mag, sgn)` invariant is `sgn & !mag == 0`
    /// (a sgn bit is meaningful only in a non-zero magnitude lane). Under
    /// this invariant negation is `(mag, sgn ^ mag)` — flipping sign on
    /// every nonzero lane while leaving zero lanes invariant. (Equivalently,
    /// `sgn = 1 - sgn` on lanes where `mag = 1`.)
    ///
    /// # Arguments
    ///
    /// * `(m, s)` — operand `(mag, sgn)`.
    ///
    /// Returns the lane-wise negation `(m', s')`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` / `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn neg_lane<Mag, Sgn>(m: Mag, s: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes;
}

/// Generic batched bipedal-like SIMD framework.
///
/// `C` is the per-prime arithmetic recipe ([`BipedalLikeConfig`]).
/// `Mag` is the magnitude lane type, `Sgn` the sign lane type — both
/// implementing [`BipedalLogicalLanes`]. For the F_3 instantiation
/// [`crate::bipedal::Bipedal3x4`] both lane types are
/// [`crate::bipedal::Avx2Lane`] and `C = Config3`.
///
/// The struct is zero-sized — it carries only a `PhantomData` to mention
/// the type parameters. All operations are associated functions; callers
/// invoke them by spelling out the type, e.g.
/// `Bipedal3x4::run_add_batch(...)`.
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe implementing [`BipedalLikeConfig`].
/// * `Mag` — magnitude lane type implementing [`BipedalLogicalLanes`].
/// * `Sgn` — sign lane type implementing [`BipedalLogicalLanes`].
///
/// # Safety
///
/// All `unsafe fn` callers must runtime-detect the hardware feature
/// underlying `Mag` and `Sgn` before invoking any method.
pub struct BatchedBipedalLike<
    C: BipedalLikeConfig,
    Mag: BipedalLogicalLanes,
    Sgn: BipedalLogicalLanes,
> {
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (C, Mag, Sgn)>,
}

impl<C, Mag, Sgn> BatchedBipedalLike<C, Mag, Sgn>
where
    C: BipedalLikeConfig,
    Mag: BipedalLogicalLanes,
    Sgn: BipedalLogicalLanes,
{
    /// Lane-level add — delegates to `C::add_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::add_lane` which is itself constant-op-count.
    #[inline(always)]
    pub unsafe fn add(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::add_lane::<Mag, Sgn>(m1, s1, m2, s2) }
    }

    /// Lane-level sub — delegates to `C::sub_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::sub_lane`.
    #[inline(always)]
    pub unsafe fn sub(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::sub_lane::<Mag, Sgn>(m1, s1, m2, s2) }
    }

    /// Lane-level mul — delegates to `C::mul_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::mul_lane`.
    #[inline(always)]
    pub unsafe fn mul(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::mul_lane::<Mag, Sgn>(m1, s1, m2, s2) }
    }

    /// Lane-level neg — delegates to `C::neg_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m, s)` — operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::neg_lane`.
    #[inline(always)]
    pub unsafe fn neg(m: Mag, s: Sgn) -> (Mag, Sgn) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::neg_lane::<Mag, Sgn>(m, s) }
    }
}
