//! Arm identities and the functions their timed calls invoke.
//!
//! Three families share this module. The population-count and fused
//! AND-population-count families time isolated kernels and the two
//! dispatchers over one buffer or one pair of buffers; the matrix-vector
//! family times a whole consumer, output allocation included.
//!
//! Two arms of each dispatched family spell the routes this issue compares:
//! the `legacy` arm reproduces the route `gf2_core` took before this issue,
//! word for word, and the `resolved` arm calls the public entry point as the
//! issue leaves it. Neither arm reimplements a kernel: both reach the same
//! `gf2_kernels_simd` bundle.

use crate::external;
use gf2_core::kernels::ops;
use gf2_core::kernels::Backend;
use gf2_core::{BitMatrix, BitVec};

/// A population-count arm resolved for this host.
///
/// # Safety
///
/// The slice must satisfy the arm's alignment precondition
/// ([`PopcountArm::accepts_misalignment`]), and the pointer must come from
/// [`PopcountArm::resolve`] on the host that calls it.
pub type PopcountFn = unsafe fn(&[u64]) -> u64;

/// A fused AND-then-population-count arm over two slices.
pub type AndPopcountFn = fn(&[u64], &[u64]) -> u64;

/// The word count at or above which `gf2_core` took its SIMD backend before
/// this issue, and still takes it: `bit_backend.simd_min_words` of the
/// conservative table.
const LEGACY_SIMD_MIN_WORDS: usize = 8;

/// The detected kernel bundle, behind an optimisation barrier.
///
/// A gf2-core consumer reaches these kernels through a function pointer loaded
/// from a process-wide `OnceLock`, which no caller can see through: every call
/// is indirect. `detect()` called here instead constructs the bundle in this
/// crate's own link-time-optimised unit, where the optimiser resolves each
/// field to its kernel and emits a direct call. The barrier restores the call
/// shape a consumer has, so an arm that spells a library route measures that
/// route rather than a devirtualised copy of it.
fn simd_fns() -> Option<gf2_kernels_simd::LogicalFns> {
    std::hint::black_box(gf2_kernels_simd::detect())
}

fn host_has_avx2() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

fn host_has_popcnt() -> bool {
    std::arch::is_x86_feature_detected!("popcnt")
}

/// Arms of the population-count family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopcountArm {
    /// The route `gf2_core::kernels::ops::popcount` took before this issue:
    /// the scalar backend below eight words, the AVX2 nibble-lookup kernel at
    /// or above it.
    LegacyDispatch,
    /// `gf2_core::kernels::ops::popcount` as this issue leaves it.
    ResolvedDispatch,
    /// The bundle's nibble-lookup kernel called directly, without a threshold.
    NibbleLut,
    /// The bundle's scalar `POPCNT` kernel called directly.
    ScalarPopcnt,
    /// The bundle's Harley-Seal carry-save kernel called directly.
    Csa,
    /// The portable `u64::count_ones` loop.
    CompilerCountOnes,
    /// libpopcnt v4.2 `popcnt()` with its own CPUID dispatch.
    Libpopcnt,
    /// Mula's `popcnt_AVX2_harley_seal` carry-save reference.
    MulaAvx2HarleySeal,
}

impl PopcountArm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 8] = [
        Self::LegacyDispatch,
        Self::ResolvedDispatch,
        Self::NibbleLut,
        Self::ScalarPopcnt,
        Self::Csa,
        Self::CompilerCountOnes,
        Self::Libpopcnt,
        Self::MulaAvx2HarleySeal,
    ];

    /// The `GF2_COUNT_ARM` spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::LegacyDispatch => "legacy-dispatch",
            Self::ResolvedDispatch => "resolved-dispatch",
            Self::NibbleLut => "nibble-lut",
            Self::ScalarPopcnt => "scalar-popcnt",
            Self::Csa => "csa",
            Self::CompilerCountOnes => "compiler-count-ones",
            Self::Libpopcnt => "libpopcnt",
            Self::MulaAvx2HarleySeal => "mula-avx2-harley-seal",
        }
    }

    /// Parses the `GF2_COUNT_ARM` spelling.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.name() == name)
    }

    /// Resolves the function the timed calls invoke, failing when this host
    /// lacks an instruction the arm requires.
    pub fn resolve(self) -> Result<PopcountFn, String> {
        match self {
            Self::LegacyDispatch => Ok(legacy_dispatch),
            Self::ResolvedDispatch => Ok(resolved_dispatch),
            Self::NibbleLut => simd_fns()
                .map(|fns| fns.popcnt_fn as PopcountFn)
                .ok_or_else(|| "nibble-lut requires the detected bundle".to_owned()),
            Self::ScalarPopcnt if host_has_popcnt() => simd_fns()
                .map(|fns| fns.popcnt_scalar_fn as PopcountFn)
                .ok_or_else(|| "scalar-popcnt requires the detected bundle".to_owned()),
            Self::ScalarPopcnt => Err("scalar-popcnt requires POPCNT".to_owned()),
            Self::Csa => simd_fns()
                .map(|fns| fns.popcnt_csa_fn as PopcountFn)
                .ok_or_else(|| "csa requires the detected bundle".to_owned()),
            Self::CompilerCountOnes => Ok(compiler_count_ones),
            Self::Libpopcnt => Ok(libpopcnt),
            Self::MulaAvx2HarleySeal if host_has_avx2() && host_has_popcnt() => {
                Ok(mula_avx2_harley_seal)
            }
            Self::MulaAvx2HarleySeal => Err("mula-avx2-harley-seal requires AVX2".to_owned()),
        }
    }

    /// Whether the arm accepts a window starting `misalignment_bytes` past a
    /// 32-byte boundary. Mula's reference loads through `const __m256i*`.
    pub fn accepts_misalignment(self, misalignment_bytes: usize) -> bool {
        self != Self::MulaAvx2HarleySeal || misalignment_bytes == 0
    }

    /// The route this arm takes for a `words`-word buffer on this host.
    pub fn selected_path(self, words: usize) -> String {
        match self {
            Self::LegacyDispatch => format!("gf2-legacy-popcount:{}", legacy_route(words)),
            Self::ResolvedDispatch => format!("gf2-ops-popcount:{}", resolved_route(words)),
            Self::NibbleLut => "gf2-kernels-simd:avx2-popcnt".to_owned(),
            Self::ScalarPopcnt => "gf2-kernels-simd:popcnt-words".to_owned(),
            Self::Csa => "gf2-kernels-simd:avx2-popcnt-csa".to_owned(),
            Self::CompilerCountOnes => "portable-count-ones-loop".to_owned(),
            Self::Libpopcnt => format!(
                "libpopcnt-4.2-popcnt:cpuid={:#x}:bytes={}",
                external::libpopcnt_capabilities().flags,
                words * size_of::<u64>()
            ),
            Self::MulaAvx2HarleySeal => "mula-avx2-harley-seal".to_owned(),
        }
    }
}

/// The kernel `ops::popcount` reached for `words` words before this issue.
pub fn legacy_route(words: usize) -> &'static str {
    if words >= LEGACY_SIMD_MIN_WORDS && simd_fns().is_some() {
        "simd-nibble-lut"
    } else {
        "scalar"
    }
}

/// The kernel `ops::popcount` reaches for `words` words as this issue leaves
/// it, as gf2-core reports the boundary at run time.
pub fn resolved_route(words: usize) -> &'static str {
    ops::popcount_route(words).as_str()
}

/// The pre-change `ops::popcount`, word for word: the shared size threshold,
/// then either the scalar backend or the bundle's nibble-lookup kernel.
fn legacy_dispatch(buf: &[u64]) -> u64 {
    if buf.len() >= LEGACY_SIMD_MIN_WORDS {
        if let Some(fns) = gf2_core::kernels::simd::maybe_simd() {
            return fns.popcount(buf);
        }
    }
    gf2_core::kernels::scalar::SCALAR_BACKEND.popcount(buf)
}

fn resolved_dispatch(buf: &[u64]) -> u64 {
    ops::popcount(buf)
}

fn compiler_count_ones(buf: &[u64]) -> u64 {
    buf.iter().map(|word| u64::from(word.count_ones())).sum()
}

fn libpopcnt(buf: &[u64]) -> u64 {
    external::libpopcnt_words(buf)
}

/// # Safety
///
/// The window is 32-byte aligned, which `accepts_misalignment` establishes.
unsafe fn mula_avx2_harley_seal(buf: &[u64]) -> u64 {
    // SAFETY: the caller upholds Mula's alignment precondition.
    unsafe { external::mula_words(buf) }
}

/// Arms of the fused AND-population-count family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AndArm {
    /// The route a fused consumer took before this issue: the bundle's
    /// nibble-lookup `avx2_and_popcnt`, resolved once.
    LegacyFused,
    /// `gf2_core::kernels::ops::and_popcount` as this issue leaves it.
    ResolvedFused,
    /// The bundle's carry-save fused kernel called directly.
    CsaFused,
    /// A single-pass portable `(a & b).count_ones()` loop.
    ScalarControl,
    /// The public gf2-core route without a fused kernel: copy into a
    /// temporary, `and_inplace`, then `popcount`, all inside every call.
    TwoPass,
}

impl AndArm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 5] = [
        Self::LegacyFused,
        Self::ResolvedFused,
        Self::CsaFused,
        Self::ScalarControl,
        Self::TwoPass,
    ];

    /// The `GF2_COUNT_ARM` spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::LegacyFused => "and-legacy-fused",
            Self::ResolvedFused => "and-resolved-fused",
            Self::CsaFused => "and-csa-fused",
            Self::ScalarControl => "and-scalar-control",
            Self::TwoPass => "and-two-pass",
        }
    }

    /// Parses the `GF2_COUNT_ARM` spelling.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.name() == name)
    }

    /// Resolves the function the timed calls invoke.
    pub fn resolve(self) -> Result<AndPopcountFn, String> {
        match self {
            Self::LegacyFused => simd_fns()
                .map(|fns| fns.and_popcnt_fn)
                .ok_or_else(|| "and-legacy-fused requires the detected bundle".to_owned()),
            Self::ResolvedFused => Ok(resolved_and_popcount),
            Self::CsaFused => simd_fns()
                .map(|fns| fns.and_popcnt_csa_fn)
                .ok_or_else(|| "and-csa-fused requires the detected bundle".to_owned()),
            Self::ScalarControl => Ok(and_scalar_control),
            Self::TwoPass => Ok(and_two_pass),
        }
    }

    /// The backend selection one call of this arm performs, for the
    /// conversion-cost probe of a whole-consumer cell.
    pub fn dispatch(self, words: usize) -> usize {
        match self {
            Self::TwoPass => {
                std::hint::black_box(ops::resolve_popcount(words));
                words
            }
            _ => words,
        }
    }

    /// The route this arm takes for a `words`-word pair on this host.
    pub fn selected_path(self, words: usize) -> String {
        match self {
            Self::LegacyFused => "gf2-kernels-simd:avx2-and-popcnt".to_owned(),
            Self::ResolvedFused => format!("gf2-ops-and-popcount:{}", resolved_and_route(words)),
            Self::CsaFused => "gf2-kernels-simd:avx2-and-popcnt-csa".to_owned(),
            Self::ScalarControl => "portable-and-count-ones-loop".to_owned(),
            Self::TwoPass => "gf2-core-copy-and-inplace-popcount".to_owned(),
        }
    }
}

/// The kernel `ops::and_popcount` reaches for `words` words, as gf2-core
/// reports the boundary at run time.
pub fn resolved_and_route(words: usize) -> &'static str {
    ops::and_popcount_route(words).as_str()
}

fn resolved_and_popcount(lhs: &[u64], rhs: &[u64]) -> u64 {
    ops::and_popcount(lhs, rhs)
}

fn and_scalar_control(lhs: &[u64], rhs: &[u64]) -> u64 {
    lhs.iter()
        .zip(rhs)
        .map(|(left, right)| u64::from((left & right).count_ones()))
        .sum()
}

/// The whole route a gf2-core consumer has without a fused kernel: a
/// temporary copy, an in-place AND and a separate count, all inside the call.
fn and_two_pass(lhs: &[u64], rhs: &[u64]) -> u64 {
    let mut scratch = lhs.to_vec();
    ops::and_inplace(&mut scratch, rhs);
    ops::popcount(&scratch)
}

/// Arms of the matrix-vector consumer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatvecArm {
    /// The row loop `BitMatrix::matvec` ran before this issue: the bundle's
    /// nibble-lookup fused kernel per row, into a freshly allocated output.
    Legacy,
    /// `BitMatrix::matvec` as this issue leaves it.
    Resolved,
}

impl MatvecArm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 2] = [Self::Legacy, Self::Resolved];

    /// The `GF2_COUNT_ARM` spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Legacy => "matvec-legacy",
            Self::Resolved => "matvec-resolved",
        }
    }

    /// Parses the `GF2_COUNT_ARM` spelling.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.name() == name)
    }

    /// Runs one whole product, output allocation included.
    pub fn run(self, matrix: &BitMatrix, x: &BitVec) -> Result<BitVec, String> {
        match self {
            Self::Legacy => {
                let fns = simd_fns().ok_or("matvec-legacy requires the detected bundle")?;
                let x_words = x.words();
                let stride = x_words.len();
                let mut y = BitVec::with_capacity(matrix.rows());
                for row in 0..matrix.rows() {
                    let words = matrix.row_words(row);
                    y.push_bit((fns.and_popcnt_fn)(&words[..stride], x_words) & 1 == 1);
                }
                Ok(y)
            }
            Self::Resolved => Ok(matrix.matvec(x)),
        }
    }

    /// The route this arm takes for a `stride_words`-word row.
    pub fn selected_path(self, stride_words: usize) -> String {
        match self {
            Self::Legacy => "gf2-matvec-legacy:avx2-and-popcnt".to_owned(),
            Self::Resolved => format!(
                "gf2-matvec:{}",
                match gf2_core::matrix::matvec_route(stride_words) {
                    gf2_core::matrix::MatvecRoute::Simd => resolved_and_route(stride_words),
                    gf2_core::matrix::MatvecRoute::Scalar => "scalar",
                }
            ),
        }
    }
}
