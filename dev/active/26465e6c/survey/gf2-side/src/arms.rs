//! Arm identities and the functions their timed calls invoke.

use crate::external;
use gf2_core::kernels::Backend;

/// A population-count arm resolved for this host.
///
/// # Safety
///
/// The slice must satisfy the arm's alignment precondition
/// ([`PopcountArm::accepts_misalignment`]), and the pointer must come from
/// [`PopcountArm::resolve`] on the host that calls it.
pub type PopcountFn = unsafe fn(&[u64]) -> u64;

/// A fused AND-then-population-count arm over two equal-length slices.
pub type AndPopcountFn = fn(&[u64], &[u64]) -> u64;

/// Arms of the population-count family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopcountArm {
    /// gf2's production dispatcher, `gf2_core::kernels::ops::popcount`.
    ProductionDispatch,
    /// gf2's AVX2 nibble-lookup kernel called directly, without the
    /// dispatcher's size threshold.
    NibbleLut,
    /// A scalar loop forced onto the `POPCNT` instruction.
    ScalarPopcnt,
    /// The portable `u64::count_ones` loop, which gf2's scalar backend runs.
    CompilerCountOnes,
    /// libpopcnt v4.2 `popcnt()` with its own CPUID dispatch.
    Libpopcnt,
    /// Mula's `popcnt_AVX2_harley_seal` carry-save reference.
    MulaAvx2HarleySeal,
}

impl PopcountArm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::ProductionDispatch,
        Self::NibbleLut,
        Self::ScalarPopcnt,
        Self::CompilerCountOnes,
        Self::Libpopcnt,
        Self::MulaAvx2HarleySeal,
    ];

    /// The `GF2_POPCOUNT_ARM` spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::ProductionDispatch => "production-dispatch",
            Self::NibbleLut => "nibble-lut",
            Self::ScalarPopcnt => "scalar-popcnt",
            Self::CompilerCountOnes => "compiler-count-ones",
            Self::Libpopcnt => "libpopcnt",
            Self::MulaAvx2HarleySeal => "mula-avx2-harley-seal",
        }
    }

    /// Parses the `GF2_POPCOUNT_ARM` spelling.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.name() == name)
    }

    /// Resolves the function the timed calls invoke, failing when this host
    /// lacks an instruction the arm requires.
    pub fn resolve(self) -> Result<PopcountFn, String> {
        match self {
            Self::ProductionDispatch => Ok(production_dispatch),
            Self::NibbleLut => simd_fns()
                .map(|fns| fns.popcnt_fn as PopcountFn)
                .ok_or_else(|| "nibble-lut requires AVX2".to_owned()),
            Self::ScalarPopcnt if host_has_popcnt() => Ok(scalar_popcnt),
            Self::ScalarPopcnt => Err("scalar-popcnt requires POPCNT".to_owned()),
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
            Self::ProductionDispatch => format!("gf2-ops-popcount:{}", dispatch_route(words)),
            Self::NibbleLut => "gf2-kernels-simd:avx2-popcnt".to_owned(),
            Self::ScalarPopcnt => "popcnt-instruction-loop".to_owned(),
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

/// Arms of the fused AND-population-count family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AndArm {
    /// gf2's fused AVX2 kernel from the `gf2_kernels_simd::detect()` bundle,
    /// resolved once as `BitMatrix::matvec_simd` receives it.
    Fused,
    /// A single-pass portable `(a & b).count_ones()` loop.
    ScalarControl,
    /// The public gf2-core route: copy into a temporary, `and_inplace`, then
    /// `popcount`, all inside every call.
    TwoPass,
}

impl AndArm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 3] = [Self::Fused, Self::ScalarControl, Self::TwoPass];

    /// The `GF2_POPCOUNT_ARM` spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fused => "and-fused",
            Self::ScalarControl => "and-scalar-control",
            Self::TwoPass => "and-two-pass",
        }
    }

    /// Parses the `GF2_POPCOUNT_ARM` spelling.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arm| arm.name() == name)
    }

    /// Resolves the function the timed calls invoke.
    pub fn resolve(self) -> Result<AndPopcountFn, String> {
        match self {
            Self::Fused => simd_fns()
                .map(|fns| fns.and_popcnt_fn)
                .ok_or_else(|| "and-fused requires AVX2".to_owned()),
            Self::ScalarControl => Ok(and_scalar_control),
            Self::TwoPass => Ok(and_two_pass),
        }
    }

    /// The route this arm takes for `words`-word operands on this host.
    pub fn selected_path(self, words: usize) -> String {
        match self {
            Self::Fused => "gf2-kernels-simd:avx2-and-popcnt".to_owned(),
            Self::ScalarControl => "portable-and-count-ones-loop".to_owned(),
            Self::TwoPass => format!("gf2-ops-and-inplace+popcount:{}", dispatch_route(words)),
        }
    }
}

/// The backend gf2-core's size dispatch selects for `words` on this host.
pub fn dispatch_route(words: usize) -> String {
    match gf2_core::kernels::select_backend_for_size(words) {
        gf2_core::kernels::SelectedBackend::Simd => gf2_core::kernels::simd::maybe_simd()
            .map_or_else(
                || "scalar".to_owned(),
                |backend| format!("simd-{}", backend.name()),
            ),
        gf2_core::kernels::SelectedBackend::Scalar => "scalar".to_owned(),
    }
}

fn simd_fns() -> Option<gf2_kernels_simd::LogicalFns> {
    gf2_kernels_simd::detect()
}

fn host_has_popcnt() -> bool {
    std::arch::is_x86_feature_detected!("popcnt")
}

fn host_has_avx2() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

#[inline(never)]
fn production_dispatch(words: &[u64]) -> u64 {
    gf2_core::kernels::ops::popcount(words)
}

#[inline(never)]
fn compiler_count_ones(words: &[u64]) -> u64 {
    words.iter().map(|word| u64::from(word.count_ones())).sum()
}

#[target_feature(enable = "popcnt")]
unsafe fn popcnt_instruction_loop(words: &[u64]) -> u64 {
    words
        .iter()
        .map(|word| std::arch::x86_64::_popcnt64(*word as i64) as u64)
        .sum()
}

/// # Safety
///
/// The host supports `POPCNT`; [`PopcountArm::resolve`] checks it.
unsafe fn scalar_popcnt(words: &[u64]) -> u64 {
    // SAFETY: the caller upholds the POPCNT requirement.
    unsafe { popcnt_instruction_loop(words) }
}

fn libpopcnt(words: &[u64]) -> u64 {
    external::libpopcnt_words(words)
}

/// # Safety
///
/// `words` starts on a 32-byte boundary and the host supports AVX2 and
/// POPCNT; [`PopcountArm::resolve`] and the arm's alignment check ensure both.
unsafe fn mula_avx2_harley_seal(words: &[u64]) -> u64 {
    // SAFETY: the caller upholds alignment and ISA support.
    unsafe { external::mula_words(words) }
}

#[inline(never)]
fn and_scalar_control(lhs: &[u64], rhs: &[u64]) -> u64 {
    lhs.iter()
        .zip(rhs)
        .map(|(left, right)| u64::from((left & right).count_ones()))
        .sum()
}

#[inline(never)]
fn and_two_pass(lhs: &[u64], rhs: &[u64]) -> u64 {
    let mut temporary = lhs.to_vec();
    gf2_core::kernels::ops::and_inplace(&mut temporary, rhs);
    gf2_core::kernels::ops::popcount(&temporary)
}
