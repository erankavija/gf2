//! Generic `BatchedBipedalLike<C>` framework.
//!
//! The framework abstracts the bipedal-like `(mag, sgn)` arithmetic over a
//! single type parameter `C: BipedalLikeConfig`. The configuration trait
//! exposes the per-prime arithmetic plug-in points as **associated types**
//! ([`BipedalLikeConfig::MagLane`] and [`BipedalLikeConfig::SgnLane`]) and
//! **associated `const`s** ([`BipedalLikeConfig::PRIME`] and
//! [`BipedalLikeConfig::U64_PER_LANE_PAIR`]); the lane-level formulas
//! ([`BipedalLikeConfig::add_lane`] etc.) operate on those associated types.
//!
//! For F_3 ([`crate::bipedal::Config3`]) both lane types are
//! [`crate::bipedal::Avx2Lane`]. F_5 and F_7 do **not** plug into this
//! framework — their R1 Candidate D 3-plane and R2 Candidate A LUT
//! encodings respectively do not fit the 2-stream `(MagLane, SgnLane)`
//! shape, so they ship via dedicated AVX2 batch entry points in
//! [`crate::x86::bipedal_avx2_packed5`] and
//! [`crate::x86::bipedal_avx2_packed7`] (see JIT issue `1f769232`'s
//! `## Amendment 2026-05-14`).
//!
//! The `BatchedBipedalLike::{add, sub, mul, neg}` methods delegate to the
//! config's `add_lane / sub_lane / mul_lane / neg_lane` recipes; this is
//! the single point of customisation when a new prime joins via the
//! framework path.
//!
//! ## Inlining contract
//!
//! All methods on this struct are `#[inline(always)]`. The `run_*_batch`
//! entry points (defined per-instantiation, see
//! [`crate::x86::bipedal_avx2`] for the F_3 generic monomorphisations)
//! carry `#[target_feature(enable = "avx2")]`. R4 §4.1 documents the
//! 12-34x regression that occurs without this discipline.

use core::marker::PhantomData;

use super::lanes::BipedalLogicalLanes;

/// Per-prime arithmetic recipe for the bipedal-like framework.
///
/// An impl supplies the lane shape and the lane-level add/sub/mul/neg
/// formula for one prime. The framework dispatches to the impl's methods
/// over the impl's chosen lane types.
///
/// The plug-in points the issue's success criterion 1 requires are split
/// across associated types (`MagLane`, `SgnLane`) and associated `const`s
/// (`PRIME`, `U64_PER_LANE_PAIR`); the methods (`add_lane`, ...) are the
/// implementation that uses those types and consts. This split is what
/// lets F_5 D-bit-sliced (W4) pick a different `MagLane` from `SgnLane`
/// without duplicating the framework body.
///
/// # Safety
///
/// All methods take and return values whose construction implied a hardware
/// feature precondition; callers must already have established that
/// precondition (typically by calling through a `#[target_feature]`-attributed
/// kernel entry point).
pub trait BipedalLikeConfig {
    /// Lane type carrying the magnitude bits.
    ///
    /// For F_3 today this is [`crate::bipedal::Avx2Lane`] (one 256-bit
    /// AVX2 vector covering 4 × `u64`). Future F_5 D-bit-sliced (W4) is
    /// expected to use a wider or differently-shaped lane covering three
    /// magnitude planes; the per-prime config selects the shape.
    type MagLane: BipedalLogicalLanes;

    /// Lane type carrying the sign bits.
    ///
    /// For F_3 today this is [`crate::bipedal::Avx2Lane`] (same as
    /// [`Self::MagLane`]). For an F_5 D-bit-sliced encoding the sign plane
    /// may be a single u64 register while the magnitude is a 3-plane wide
    /// lane — picking different `MagLane` and `SgnLane` lets the framework
    /// describe both shapes with one trait.
    type SgnLane: BipedalLogicalLanes;

    /// The prime characteristic this configuration encodes.
    ///
    /// Used for documentation and diagnostics today; future codegen-time
    /// dispatch may select between formula variants on this value.
    const PRIME: u64;

    /// Number of `u64` words spanned by one `(MagLane, SgnLane)` pair —
    /// the framework's iteration step over the `&[u64]` slice ABI.
    ///
    /// For F_3 today: `4` (one 256-bit `Avx2Lane` covers 4 × `u64`). For
    /// shape-uniform encodings this equals `<MagLane as
    /// BipedalLogicalLanes>::U64_PER_LANE`. For shape-non-uniform encodings
    /// (e.g. future F_5 D-bit-sliced where magnitude and sign use different
    /// lane shapes) this records the framework's per-iteration u64 step
    /// directly and may differ from either lane's individual stride.
    const U64_PER_LANE_PAIR: usize;

    /// Lane-level add formula, operating on the impl's [`Self::MagLane`] /
    /// [`Self::SgnLane`].
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
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops
    /// (six for F_3).
    unsafe fn add_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-level sub formula, operating on the impl's [`Self::MagLane`] /
    /// [`Self::SgnLane`].
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
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn sub_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-level mul formula, operating on the impl's [`Self::MagLane`] /
    /// [`Self::SgnLane`].
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
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn mul_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane);

    /// Lane-level negation formula, operating on the impl's
    /// [`Self::MagLane`] / [`Self::SgnLane`].
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
    /// Hardware feature underlying [`Self::MagLane`] / [`Self::SgnLane`]
    /// must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: a small constant number of lane-level logical ops.
    unsafe fn neg_lane(m: Self::MagLane, s: Self::SgnLane) -> (Self::MagLane, Self::SgnLane);
}

/// Generic batched bipedal-like SIMD framework.
///
/// `C` is the per-prime arithmetic recipe ([`BipedalLikeConfig`]). The
/// magnitude and sign lane shapes come from `C::MagLane` / `C::SgnLane`,
/// not from extra type parameters — adding a new prime is a single new
/// [`BipedalLikeConfig`] impl.
///
/// The struct is zero-sized — it carries only a `PhantomData` to mention
/// the type parameter. All operations are associated functions; callers
/// invoke them by spelling out the type, e.g.
/// `Bipedal3x4::add(...)` (where `Bipedal3x4 = BatchedBipedalLike<Config3>`).
///
/// # Type parameters
///
/// * `C` — per-prime arithmetic recipe implementing [`BipedalLikeConfig`].
///   The lane shape is determined by `C::MagLane` and `C::SgnLane`.
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
    /// Lane-level add — delegates to `C::add_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::add_lane` which is itself constant-op-count.
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

    /// Lane-level sub — delegates to `C::sub_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::sub_lane`.
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

    /// Lane-level mul — delegates to `C::mul_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m1, s1)` — first operand.
    /// * `(m2, s2)` — second operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::mul_lane`.
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

    /// Lane-level neg — delegates to `C::neg_lane`.
    ///
    /// # Arguments
    ///
    /// * `(m, s)` — operand.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `C::MagLane` and `C::SgnLane` must be
    /// available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: dispatched to `C::neg_lane`.
    #[inline(always)]
    pub unsafe fn neg(m: C::MagLane, s: C::SgnLane) -> (C::MagLane, C::SgnLane) {
        // SAFETY: forwarded precondition — hardware feature available.
        unsafe { C::neg_lane(m, s) }
    }
}
