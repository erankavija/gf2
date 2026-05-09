//! Generic `BatchedBipedalLike<MagLanes, SgnLanes>` framework.
//!
//! The framework abstracts the bipedal `(mag, sgn)` arithmetic over two
//! type-parameters that supply lane-width logical primitives. F_3
//! instantiates with both `MagLanes = SgnLanes = __m256i`. F_5 (D-bit-sliced,
//! 3 planes) and F_7 (LUT-A, 16-bit slot) would later instantiate the same
//! framework with their own lane types and their own primitive sets.
//!
//! ## A note on `#[target_feature]` and inlining
//!
//! Rust trait methods cannot carry `#[target_feature(enable = "avx2")]`
//! attributes. To still get clean inlining of the inner trait methods into
//! the outer kernel, we co-attribute the kernel entry points
//! (`run_*_batch`) with `#[target_feature(enable = "avx2")]` and ensure the
//! trait impl methods are `#[inline(always)]`. With both ends of the call
//! enabled for AVX2, rustc inlines through the trait calls and emits the
//! same AVX2 instruction stream as the per-prime kernel. Earlier versions
//! that only put `#[target_feature]` on the per-prime side measured a 12-34x
//! penalty against the generic framework — that was a benchmark artefact,
//! not a real cost. The current implementation closes the gap modulo a
//! small constant factor (see `dev/plans/r4_simd_batching_decision.md`).
//!
//! All `pub unsafe fn` here carry a top-of-function `// SAFETY:` comment.

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Lane-width logical primitives required by the generic bipedal-like
/// framework.
///
/// An impl supplies the lane-wise AND, XOR, and OR for one register width
/// plus the load and store. The framework calls these primitives in the
/// same order the per-prime kernel uses, but is generic over the underlying
/// lane width.
///
/// # Safety
///
/// All methods are `unsafe` because they assume the corresponding hardware
/// feature is present at runtime. Callers must runtime-detect the feature
/// before calling any method.
pub trait BipedalLogicalLanes: Copy {
    /// Load a lane from a `&[u64]` slice at the given word offset.
    ///
    /// # Safety
    ///
    /// `offset + Self::U64_PER_LANE <= src.len()` and the corresponding
    /// hardware feature is available.
    unsafe fn loadu(src: &[u64], offset: usize) -> Self;

    /// Store a lane to a `&mut [u64]` slice at the given word offset.
    ///
    /// # Safety
    ///
    /// `offset + Self::U64_PER_LANE <= dst.len()` and the corresponding
    /// hardware feature is available.
    unsafe fn storeu(dst: &mut [u64], offset: usize, v: Self);

    /// Lane-wise AND.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn and(a: Self, b: Self) -> Self;

    /// Lane-wise XOR.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn xor(a: Self, b: Self) -> Self;

    /// Lane-wise OR.
    ///
    /// # Safety
    ///
    /// Hardware feature must be available.
    unsafe fn or(a: Self, b: Self) -> Self;

    /// How many u64 words this lane spans.
    const U64_PER_LANE: usize;
}

/// AVX2 256-bit lane (4 u64) impl of `BipedalLogicalLanes`.
///
/// Used by the F_3 instantiation `Bipedal3x4` of [`BatchedBipedalLike`].
#[derive(Clone, Copy)]
pub struct Avx2Lane(pub __m256i);

impl BipedalLogicalLanes for Avx2Lane {
    const U64_PER_LANE: usize = 4;

    #[inline(always)]
    unsafe fn loadu(src: &[u64], offset: usize) -> Self {
        // SAFETY: caller-checked bounds + AVX2 availability.
        unsafe {
            Avx2Lane(_mm256_loadu_si256(
                src.as_ptr().add(offset) as *const __m256i
            ))
        }
    }

    #[inline(always)]
    unsafe fn storeu(dst: &mut [u64], offset: usize, v: Self) {
        // SAFETY: caller-checked bounds + AVX2 availability.
        unsafe {
            _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, v.0);
        }
    }

    #[inline(always)]
    unsafe fn and(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_and_si256(a.0, b.0)) }
    }

    #[inline(always)]
    unsafe fn xor(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_xor_si256(a.0, b.0)) }
    }

    #[inline(always)]
    unsafe fn or(a: Self, b: Self) -> Self {
        // SAFETY: AVX2 availability is the caller's precondition.
        unsafe { Avx2Lane(_mm256_or_si256(a.0, b.0)) }
    }
}

/// Generic batched bipedal-like SIMD framework over two lane types.
///
/// `MagLanes` is the lane type carrying the magnitude bits, `SgnLanes` the
/// lane type carrying the sign bits. For F_3 both are `Avx2Lane`. For
/// future F_5 / F_7 encodings the lane types may differ in shape (e.g.,
/// F_5 D-bit-sliced uses three planes per word).
///
/// The framework's add/sub/mul/run_*_batch methods are written entirely in
/// terms of the `BipedalLogicalLanes` trait, with no direct intrinsic
/// references. This is what allows one body to serve every encoding.
///
/// # Type parameters
///
/// * `Mag` — magnitude lane type implementing `BipedalLogicalLanes`.
/// * `Sgn` — sign lane type implementing `BipedalLogicalLanes`.
///
/// # Safety
///
/// All `unsafe fn` callers must runtime-detect the hardware feature
/// underlying both `Mag` and `Sgn`.
pub struct BatchedBipedalLike<Mag: BipedalLogicalLanes, Sgn: BipedalLogicalLanes> {
    _phantom: core::marker::PhantomData<fn() -> (Mag, Sgn)>,
}

impl<Mag: BipedalLogicalLanes, Sgn: BipedalLogicalLanes> BatchedBipedalLike<Mag, Sgn> {
    /// Bipedal F_3-shape add over a `(Mag, Sgn)` lane pair.
    ///
    /// `t = m1 ^ s1 ^ s2; u = m2 & t; m_+ = u | (m1 ^ m2); s_+ = u ^ s1`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: six lane-level logical ops.
    #[inline(always)]
    pub unsafe fn add(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: hardware feature availability is the caller's precondition.
        // For the F_3 instantiation Mag = Sgn so the casts are no-ops; we
        // route through transmute to keep the framework body shape-uniform.
        unsafe {
            let s1_as_m: Mag = transmute_lane(s1);
            let s2_as_m: Mag = transmute_lane(s2);
            let t = Mag::xor(Mag::xor(m1, s1_as_m), s2_as_m);
            let u = Mag::and(m2, t);
            let m_plus = Mag::or(u, Mag::xor(m1, m2));
            let s_plus_m: Mag = Mag::xor(u, s1_as_m);
            let s_plus: Sgn = transmute_lane(s_plus_m);
            (m_plus, s_plus)
        }
    }

    /// Bipedal F_3-shape sub over a `(Mag, Sgn)` lane pair.
    ///
    /// `t = s1 ^ s2; u = m1 & t; m_- = u | (m1 ^ m2); s_- = u ^ (m2 ^ s2)`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: six lane-level logical ops.
    #[inline(always)]
    pub unsafe fn sub(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: hardware feature availability is the caller's precondition.
        unsafe {
            let s1_as_m: Mag = transmute_lane(s1);
            let s2_as_m: Mag = transmute_lane(s2);
            let m2_as_s: Sgn = transmute_lane(m2);
            let t_m = Mag::xor(s1_as_m, s2_as_m);
            let u = Mag::and(m1, t_m);
            let m_minus = Mag::or(u, Mag::xor(m1, m2));
            // s_- = u ^ (m2 ^ s2) — done in the Sgn type to mirror bipedal_avx2_sub.
            let m2_xor_s2: Sgn = Sgn::xor(m2_as_s, s2);
            let u_as_s: Sgn = transmute_lane(u);
            let s_minus = Sgn::xor(u_as_s, m2_xor_s2);
            (m_minus, s_minus)
        }
    }

    /// Bipedal F_3-shape mul over a `(Mag, Sgn)` lane pair.
    ///
    /// `m_x = m1 & m2; s_x = s1 ^ s2`.
    ///
    /// # Safety
    ///
    /// Hardware feature underlying `Mag` and `Sgn` must be available.
    ///
    /// # Complexity
    ///
    /// `O(1)`: two lane-level logical ops.
    #[inline(always)]
    pub unsafe fn mul(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn) {
        // SAFETY: hardware feature availability is the caller's precondition.
        unsafe {
            let m_x = Mag::and(m1, m2);
            let s_x = Sgn::xor(s1, s2);
            (m_x, s_x)
        }
    }
}

// AVX2-specific batch entry points for the F_3 instantiation. We attach
// `#[target_feature(enable = "avx2")]` here so rustc inlines the trait
// method bodies (which use AVX2 intrinsics) into a function-level context
// that has the feature enabled. Without these the generic kernel measures
// 12-34x slower than per-prime, purely as an inlining-budget artefact.
impl BatchedBipedalLike<Avx2Lane, Avx2Lane> {
    /// Apply [`BatchedBipedalLike::add`] across two slices of `(mag, sgn)` u64 words.
    ///
    /// # Safety
    ///
    /// AVX2 must be available; all six slices share length divisible by 4.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use simd_batching_bench::generic::Bipedal3x4;
    /// if is_x86_feature_detected!("avx2") {
    ///     let v = vec![0u64; 4];
    ///     let mut out_m = vec![0u64; 4];
    ///     let mut out_s = vec![0u64; 4];
    ///     // SAFETY: AVX2 verified, slices are length 4.
    ///     unsafe { Bipedal3x4::run_add_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n / 4)`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_add_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        let n = mag1.len();
        debug_assert_eq!(n % 4, 0);
        // SAFETY: AVX2 + bounds are caller's preconditions.
        unsafe {
            let mut i = 0;
            while i < n {
                let v_m1 = Avx2Lane::loadu(mag1, i);
                let v_s1 = Avx2Lane::loadu(sgn1, i);
                let v_m2 = Avx2Lane::loadu(mag2, i);
                let v_s2 = Avx2Lane::loadu(sgn2, i);
                let (m, s) = Self::add(v_m1, v_s1, v_m2, v_s2);
                Avx2Lane::storeu(out_mag, i, m);
                Avx2Lane::storeu(out_sgn, i, s);
                i += 4;
            }
        }
    }

    /// Apply [`BatchedBipedalLike::sub`] across two slices of `(mag, sgn)` u64 words.
    ///
    /// # Safety
    ///
    /// AVX2 must be available; all six slices share length divisible by 4.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use simd_batching_bench::generic::Bipedal3x4;
    /// if is_x86_feature_detected!("avx2") {
    ///     let v = vec![0u64; 4];
    ///     let mut out_m = vec![0u64; 4];
    ///     let mut out_s = vec![0u64; 4];
    ///     // SAFETY: AVX2 verified, slices are length 4.
    ///     unsafe { Bipedal3x4::run_sub_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n / 4)`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_sub_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        let n = mag1.len();
        debug_assert_eq!(n % 4, 0);
        // SAFETY: AVX2 + bounds are caller's preconditions.
        unsafe {
            let mut i = 0;
            while i < n {
                let v_m1 = Avx2Lane::loadu(mag1, i);
                let v_s1 = Avx2Lane::loadu(sgn1, i);
                let v_m2 = Avx2Lane::loadu(mag2, i);
                let v_s2 = Avx2Lane::loadu(sgn2, i);
                let (m, s) = Self::sub(v_m1, v_s1, v_m2, v_s2);
                Avx2Lane::storeu(out_mag, i, m);
                Avx2Lane::storeu(out_sgn, i, s);
                i += 4;
            }
        }
    }

    /// Apply [`BatchedBipedalLike::mul`] across two slices of `(mag, sgn)` u64 words.
    ///
    /// # Safety
    ///
    /// AVX2 must be available; all six slices share length divisible by 4.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use simd_batching_bench::generic::Bipedal3x4;
    /// if is_x86_feature_detected!("avx2") {
    ///     let v = vec![0u64; 4];
    ///     let mut out_m = vec![0u64; 4];
    ///     let mut out_s = vec![0u64; 4];
    ///     // SAFETY: AVX2 verified, slices are length 4.
    ///     unsafe { Bipedal3x4::run_mul_batch(&v, &v, &v, &v, &mut out_m, &mut out_s); }
    /// }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n / 4)`.
    #[inline]
    #[target_feature(enable = "avx2")]
    pub unsafe fn run_mul_batch(
        mag1: &[u64],
        sgn1: &[u64],
        mag2: &[u64],
        sgn2: &[u64],
        out_mag: &mut [u64],
        out_sgn: &mut [u64],
    ) {
        let n = mag1.len();
        debug_assert_eq!(n % 4, 0);
        // SAFETY: AVX2 + bounds are caller's preconditions.
        unsafe {
            let mut i = 0;
            while i < n {
                let v_m1 = Avx2Lane::loadu(mag1, i);
                let v_s1 = Avx2Lane::loadu(sgn1, i);
                let v_m2 = Avx2Lane::loadu(mag2, i);
                let v_s2 = Avx2Lane::loadu(sgn2, i);
                let (m, s) = Self::mul(v_m1, v_s1, v_m2, v_s2);
                Avx2Lane::storeu(out_mag, i, m);
                Avx2Lane::storeu(out_sgn, i, s);
                i += 4;
            }
        }
    }
}

/// Concrete F_3 instantiation: 256-lane batched AVX2.
///
/// `LANES = 256` (4 u64 × 64 bits per u64). Both magnitude and sign use
/// the AVX2 256-bit lane.
pub type Bipedal3x4 = BatchedBipedalLike<Avx2Lane, Avx2Lane>;

/// Reinterpret one lane type as another of the same byte size.
///
/// For the F_3 instantiation `Mag = Sgn = Avx2Lane`, so this is a no-op.
/// In a future F_5 framework where `Mag != Sgn` the framework would
/// supply a domain-specific conversion instead; this helper only exists
/// to keep the F_3 code path generic-looking.
///
/// # Safety
///
/// `From` and `To` must have the same memory layout. For the only call
/// site (`Mag = Sgn = Avx2Lane`) this is trivially true.
#[inline(always)]
unsafe fn transmute_lane<From: Copy, To: Copy>(x: From) -> To {
    debug_assert_eq!(core::mem::size_of::<From>(), core::mem::size_of::<To>());
    debug_assert_eq!(core::mem::align_of::<From>(), core::mem::align_of::<To>());
    // SAFETY: caller asserts From and To have the same layout. This is the
    // case for the only instantiated call site (Mag = Sgn = Avx2Lane), so
    // the transmute is a true no-op.
    unsafe { core::mem::transmute_copy::<From, To>(&x) }
}
