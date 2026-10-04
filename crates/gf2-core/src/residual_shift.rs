//! Route selection for the residual branch of the public `BitVec` shifts.
//!
//! A shift by `k` bits is word-aligned when `k % 64` is zero and residual
//! otherwise. [`BitVec::shift_left`](crate::BitVec::shift_left) and
//! [`shift_right`](crate::BitVec::shift_right) hand their residual branch to
//! this module, which selects one of two routes for the funnel loop —
//! [`ResidualShiftRoute`] names them — and owns the boundary word and the
//! zero fill around it in either case.

/// Which implementation of the residual funnel loop a call takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ResidualShiftRoute {
    /// The portable funnel in this crate: safe scalar Rust, no processor
    /// feature, available in every build.
    ScalarFunnel,
    /// The `bmi2`-gated funnel kernel of `gf2-kernels-simd`.
    Bmi2Funnel,
}

impl ResidualShiftRoute {
    /// The tag receipts and benchmark records carry.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            ResidualShiftRoute::ScalarFunnel => "residual-scalar-funnel",
            ResidualShiftRoute::Bmi2Funnel => "residual-bmi2-funnel",
        }
    }
}

/// The selected route together with what it takes to run it.
#[derive(Copy, Clone)]
enum Funnel {
    Portable,
    #[cfg(feature = "simd")]
    Bmi2(&'static gf2_kernels_simd::shift_funnel::ShiftFunnelFns),
}

impl Funnel {
    fn route(self) -> ResidualShiftRoute {
        match self {
            Funnel::Portable => ResidualShiftRoute::ScalarFunnel,
            #[cfg(feature = "simd")]
            Funnel::Bmi2(_) => ResidualShiftRoute::Bmi2Funnel,
        }
    }
}

/// The one selection point: both directions and the public reporter resolve
/// the route here, so a report and an execution cannot disagree.
#[inline]
fn select() -> Funnel {
    #[cfg(feature = "simd")]
    if kernel_route_enabled() {
        if let Some(fns) = crate::simd::maybe_shift_funnel() {
            return Funnel::Bmi2(fns);
        }
    }
    Funnel::Portable
}

/// The route a residual shift takes on this host, in this build, right now.
///
/// [`ResidualShiftRoute::Bmi2Funnel`] needs all of: this crate's `simd` cargo
/// feature, which is not one of its defaults; the `bmi2` processor feature,
/// which `gf2_kernels_simd::shift_funnel::detect` tests once per process; and a
/// clear force switch, which only `test` and `test-support` builds can set.
/// Anything else is [`ResidualShiftRoute::ScalarFunnel`].
#[must_use]
pub fn residual_shift_route() -> ResidualShiftRoute {
    select().route()
}

/// Runs the residual left-shift branch over `data`.
///
/// Writes `data[word_shift ..]` as the left shift by
/// `64 * word_shift + bit_shift` bits of the original contents and zeroes
/// `data[.. word_shift]`. Bits shifted past the top word are lost; the caller
/// owns the tail mask above its own bit length.
///
/// # Panics
///
/// Panics unless `word_shift < data.len()` and `bit_shift` lies in `1..64`,
/// which is what the public shifts' `k < len_bits` split and their
/// word-aligned branch leave for this path.
pub(crate) fn shift_left(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    assert_residual_arguments(data.len(), word_shift, bit_shift);
    let funnel = select();
    record_route(funnel.route());
    match funnel {
        Funnel::Portable => portable_shift_left_funnel(data, word_shift, bit_shift),
        #[cfg(feature = "simd")]
        Funnel::Bmi2(fns) => (fns.shift_left_funnel)(data, word_shift, bit_shift),
    }
    // The lowest surviving word has no lower neighbour to funnel in.
    data[word_shift] = data[0] << bit_shift;
    data[..word_shift].fill(0);
}

/// Runs the residual right-shift branch over `data`.
///
/// Writes `data[.. data.len() - word_shift]` as the right shift by
/// `64 * word_shift + bit_shift` bits of the original contents and zeroes the
/// words above it.
///
/// # Panics
///
/// As [`shift_left`].
pub(crate) fn shift_right(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    assert_residual_arguments(data.len(), word_shift, bit_shift);
    let funnel = select();
    record_route(funnel.route());
    match funnel {
        Funnel::Portable => portable_shift_right_funnel(data, word_shift, bit_shift),
        #[cfg(feature = "simd")]
        Funnel::Bmi2(fns) => (fns.shift_right_funnel)(data, word_shift, bit_shift),
    }
    let last = data.len() - 1;
    // The topmost surviving word has no higher neighbour to funnel in.
    data[last - word_shift] = data[last] >> bit_shift;
    data[last + 1 - word_shift..].fill(0);
}

#[inline]
fn assert_residual_arguments(words: usize, word_shift: usize, bit_shift: u32) {
    assert!(
        word_shift < words,
        "residual shift: word_shift {word_shift} outside a {words}-word buffer"
    );
    assert!(
        (1..64).contains(&bit_shift),
        "residual shift: bit_shift {bit_shift} outside 1..64"
    );
}

/// The portable left funnel, and the fallback [`ResidualShiftRoute::ScalarFunnel`]
/// names. Descending, so an in-place rewrite reads no word it has written.
fn portable_shift_left_funnel(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    let inv = 64 - bit_shift;
    for i in (word_shift + 1..data.len()).rev() {
        data[i] = (data[i - word_shift] << bit_shift) | (data[i - word_shift - 1] >> inv);
    }
}

/// The portable right funnel. Ascending, for the same reason.
fn portable_shift_right_funnel(data: &mut [u64], word_shift: usize, bit_shift: u32) {
    let inv = 64 - bit_shift;
    for i in 0..data.len() - word_shift - 1 {
        data[i] = (data[i + word_shift] >> bit_shift) | (data[i + word_shift + 1] << inv);
    }
}

/// Records the route the most recent residual shift executed.
#[cfg(any(test, feature = "test-support"))]
#[inline]
fn record_route(route: ResidualShiftRoute) {
    LAST_ROUTE.with(|cell| cell.set(Some(route)));
}

#[cfg(not(any(test, feature = "test-support")))]
#[inline(always)]
fn record_route(_route: ResidualShiftRoute) {}

#[cfg(any(test, feature = "test-support"))]
std::thread_local! {
    /// Route of the most recent residual shift on this thread.
    static LAST_ROUTE: std::cell::Cell<Option<ResidualShiftRoute>> =
        const { std::cell::Cell::new(None) };
}

/// The route the most recent residual shift took on this thread, or `None`
/// where this thread has run none since the last
/// [`reset_last_residual_shift_route`].
///
/// The witness a suite reads to prove which route a public call took, rather
/// than assuming the route the build and host imply.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_residual_shift_route() -> Option<ResidualShiftRoute> {
    LAST_ROUTE.with(std::cell::Cell::get)
}

/// Clears the witness, so the next observation sees only routes taken after
/// this call.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_residual_shift_route() {
    LAST_ROUTE.with(|cell| cell.set(None));
}

#[cfg(any(test, feature = "test-support"))]
static FORCE_SCALAR_RESIDUAL_SHIFT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Holds every residual shift on [`ResidualShiftRoute::ScalarFunnel`], or
/// releases it back to [`residual_shift_route`], and reports the previous
/// setting.
///
/// This keeps the fallback reachable under test and lets one build drive either
/// route. The switch is process-wide, so a caller serialises its whole
/// toggle-execute-observe-restore section, as
/// `crates/gf2-core/tests/residual_shift_routes.rs` does. Without the `simd`
/// cargo feature there is no other route and the setting changes nothing.
#[cfg(any(test, feature = "test-support"))]
pub fn force_scalar_residual_shift(forced: bool) -> bool {
    FORCE_SCALAR_RESIDUAL_SHIFT.swap(forced, std::sync::atomic::Ordering::Relaxed)
}

/// Whether [`select`] is free to take the kernel route.
#[cfg(all(feature = "simd", any(test, feature = "test-support")))]
#[inline]
fn kernel_route_enabled() -> bool {
    !FORCE_SCALAR_RESIDUAL_SHIFT.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(all(feature = "simd", not(any(test, feature = "test-support"))))]
#[inline(always)]
fn kernel_route_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The left branch as written, over a whole buffer, so the module's own
    /// cases do not depend on `BitVec`.
    fn reference_left(data: &[u64], word_shift: usize, bit_shift: u32) -> Vec<u64> {
        let mut out = data.to_vec();
        portable_shift_left_funnel(&mut out, word_shift, bit_shift);
        out[word_shift] = data[0] << bit_shift;
        out[..word_shift].fill(0);
        out
    }

    fn reference_right(data: &[u64], word_shift: usize, bit_shift: u32) -> Vec<u64> {
        let mut out = data.to_vec();
        portable_shift_right_funnel(&mut out, word_shift, bit_shift);
        let last = data.len() - 1;
        out[last - word_shift] = data[last] >> bit_shift;
        out[last + 1 - word_shift..].fill(0);
        out
    }

    #[test]
    fn the_selected_route_agrees_with_the_portable_funnel() {
        type Branch = fn(&mut [u64], usize, u32);
        type Reference = fn(&[u64], usize, u32) -> Vec<u64>;
        let branches: [(&str, Branch, Reference); 2] = [
            ("left", shift_left, reference_left),
            ("right", shift_right, reference_right),
        ];
        let source: Vec<u64> = (0..9u64)
            .map(|i| 0x0123_4567_89AB_CDEF ^ (i * 0x9E37))
            .collect();
        for word_shift in 0..source.len() {
            for bit_shift in [1u32, 7, 32, 63] {
                for (direction, branch, reference) in branches {
                    let expected = reference(&source, word_shift, bit_shift);
                    let mut actual = source.clone();
                    branch(&mut actual, word_shift, bit_shift);
                    assert_eq!(
                        actual, expected,
                        "{direction}: word_shift {word_shift} bit_shift {bit_shift}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_witness_reports_the_route_the_reporter_names() {
        reset_last_residual_shift_route();
        assert_eq!(last_residual_shift_route(), None);
        let reported = residual_shift_route();
        shift_left(&mut [1u64, 2, 3], 1, 5);
        assert_eq!(last_residual_shift_route(), Some(reported));
    }

    #[test]
    #[should_panic(expected = "bit_shift")]
    fn a_word_aligned_offset_is_not_this_module_s_to_run() {
        shift_left(&mut [0u64; 2], 0, 0);
    }
}
