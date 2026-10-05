//! Generic `BatchedBipedalLike<C>` framework: `C: BipedalLikeConfig` supplies
//! the lane types and the lane-level add/sub/mul/neg formulas of one prime,
//! which [`BatchedBipedalLike`] exposes. Every method is `#[inline(always)]` so
//! that it inlines into the `#[target_feature(enable = "avx2")]` batch entry
//! points in `crate::x86::bipedal_avx2`.

use core::marker::PhantomData;

use super::lanes::BipedalLogicalLanes;

/// Per-prime arithmetic recipe for the bipedal-like framework.
///
/// An impl supplies the lane shape and the lane-level add/sub/mul/neg
/// formula for one prime.
///
/// # Safety
///
/// All methods take and return values whose construction implied a hardware
/// feature precondition; callers must already have established that
/// precondition (typically by calling through a `#[target_feature]`-attributed
/// kernel entry point).
pub trait BipedalLikeConfig {
    /// Lane type carrying the magnitude bits.
    type MagLane: BipedalLogicalLanes;

    /// Lane type carrying the sign bits.
    type SgnLane: BipedalLogicalLanes;

    /// The prime characteristic this configuration encodes.
    const PRIME: u64;

    /// Number of `u64` words spanned by one `(MagLane, SgnLane)` pair: the
    /// iteration step over the `&[u64]` slice ABI.
    const U64_PER_LANE_PAIR: usize;

    /// Lane-wise sum `(m1, s1) + (m2, s2)`.
    ///
    /// For F_3: `t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`
    /// (`@/citation/Scheinerman2024` §2.2).
    ///
    /// # Safety
    ///
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    unsafe fn add_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-wise difference `(m1, s1) - (m2, s2)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    unsafe fn sub_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-wise product `(m1, s1) * (m2, s2)`.
    ///
    /// For F_3: `m_x = m1 & m2; s_x = s1 ^ s2` (`@/citation/Scheinerman2024` §2.2).
    ///
    /// # Safety
    ///
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    unsafe fn mul_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-wise negation of `(m, s)`.
    ///
    /// For F_3 the canonical invariant is `sgn & !mag == 0`, under which
    /// negation is `(mag, sgn ^ mag)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    unsafe fn neg_lane(m: Self::MagLane, s: Self::SgnLane) -> (Self::MagLane, Self::SgnLane);
}

/// Generic batched bipedal-like SIMD framework.
///
/// Zero-sized; its associated functions are the lane operations of the
/// per-prime recipe `C`.
///
/// # Safety
///
/// All `unsafe fn` callers must runtime-detect the hardware feature
/// underlying `C::MagLane` and `C::SgnLane` before invoking any method.
pub struct BatchedBipedalLike<C: BipedalLikeConfig> {
    _phantom: PhantomData<fn() -> C>,
}

impl<C> BatchedBipedalLike<C>
where
    C: BipedalLikeConfig,
{
    /// Lane-wise sum `(m1, s1) + (m2, s2)` in the field of `C`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    #[inline(always)]
    pub unsafe fn add(
        m1: C::MagLane,
        s1: C::SgnLane,
        m2: C::MagLane,
        s2: C::SgnLane,
    ) -> (C::MagLane, C::SgnLane) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::add_lane(m1, s1, m2, s2) }
    }

    /// Lane-wise difference `(m1, s1) - (m2, s2)` in the field of `C`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    #[inline(always)]
    pub unsafe fn sub(
        m1: C::MagLane,
        s1: C::SgnLane,
        m2: C::MagLane,
        s2: C::SgnLane,
    ) -> (C::MagLane, C::SgnLane) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::sub_lane(m1, s1, m2, s2) }
    }

    /// Lane-wise product `(m1, s1) * (m2, s2)` in the field of `C`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    #[inline(always)]
    pub unsafe fn mul(
        m1: C::MagLane,
        s1: C::SgnLane,
        m2: C::MagLane,
        s2: C::SgnLane,
    ) -> (C::MagLane, C::SgnLane) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::mul_lane(m1, s1, m2, s2) }
    }

    /// Lane-wise negation of `(m, s)` in the field of `C`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// Every argument is a value, so no pointer, length or aliasing condition
    /// applies.
    #[inline(always)]
    pub unsafe fn neg(m: C::MagLane, s: C::SgnLane) -> (C::MagLane, C::SgnLane) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::neg_lane(m, s) }
    }
}
