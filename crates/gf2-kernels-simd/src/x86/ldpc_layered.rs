//! AVX2 layer updates; the 32 `i8` lanes of a vector are 32 checks of a layer.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::ldpc_layered::{
    update_layer_scalar, LayerKernel, LayerParams, LayeredRule, LAYER_LANES,
};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

pub(crate) fn detect() -> Option<LayerKernel> {
    std::arch::is_x86_feature_detected!("avx2").then_some(LayerKernel {
        layer_fn: update_layer_avx2,
    })
}

fn update_layer_avx2(
    params: LayerParams,
    apps: &mut [i8],
    messages: &mut [i8],
    degree: usize,
    stride: usize,
    width: usize,
) -> bool {
    // SAFETY: detect publishes this entry only on AVX2 hosts. LayerKernel's
    // public method checks the stride, both lengths and product overflow.
    unsafe { update_layer_avx2_unchecked(params, apps, messages, degree, stride, width) }
}

/// Scales and saturates 32 magnitudes in `0..=127`, as `LayerParams::scale`.
#[target_feature(enable = "avx2")]
fn scale(params: LayerParams, magnitude: __m256i) -> __m256i {
    let scaled = match params.rule() {
        LayeredRule::Normalized { eighths } => {
            let zero = _mm256_setzero_si256();
            let factor = _mm256_set1_epi16(i16::from(eighths));
            let low = _mm256_srli_epi16::<3>(_mm256_mullo_epi16(
                _mm256_unpacklo_epi8(magnitude, zero),
                factor,
            ));
            let high = _mm256_srli_epi16::<3>(_mm256_mullo_epi16(
                _mm256_unpackhi_epi8(magnitude, zero),
                factor,
            ));
            // Unpack and pack act within each 128-bit half in matching order.
            _mm256_packus_epi16(low, high)
        }
        LayeredRule::Offset { beta } => _mm256_subs_epu8(magnitude, _mm256_set1_epi8(beta as i8)),
    };
    _mm256_min_epu8(scaled, _mm256_set1_epi8(params.message_limit() as i8))
}

/// # Safety
/// The host must support AVX2, `stride` must be a multiple of 32 and both
/// slices must contain `degree * stride` values.
#[target_feature(enable = "avx2")]
unsafe fn update_layer_avx2_unchecked(
    params: LayerParams,
    apps: &mut [i8],
    messages: &mut [i8],
    degree: usize,
    stride: usize,
    width: usize,
) -> bool {
    // Edge indices are i8 lanes and 0xff marks no minimum yet.
    if degree > 255 {
        return update_layer_scalar(params, apps, messages, degree, stride, width);
    }
    let zero = _mm256_setzero_si256();
    let one = _mm256_set1_epi8(1);
    let limit = params.extrinsic_limit() as i8;
    let (low, high) = (_mm256_set1_epi8(-limit), _mm256_set1_epi8(limit));
    let mut unsettled = 0u32;
    // The entry sign bits of each edge of one lane group; degree <= 255.
    let mut signs = [0u32; 255];
    for lane in (0..stride).step_by(LAYER_LANES) {
        let mut min1 = _mm256_set1_epi8(127);
        let mut min2 = _mm256_set1_epi8(127);
        let mut arg1 = _mm256_set1_epi8(-1);
        let mut parity = zero;
        let mut entry_parity = 0u32;
        let mut index = zero;
        for (edge, entry) in signs[..degree].iter_mut().enumerate() {
            let at = edge * stride + lane;
            // SAFETY: at + 32 <= degree * stride, the length of both slices;
            // unaligned accesses are permitted and the slices cannot alias.
            let (value, entry_signs) = unsafe {
                let app = _mm256_loadu_si256(apps.as_ptr().add(at).cast());
                let message = _mm256_loadu_si256(messages.as_ptr().add(at).cast());
                // Saturation at ±128 lies outside the extrinsic bound.
                let value =
                    _mm256_min_epi8(_mm256_max_epi8(_mm256_subs_epi8(app, message), low), high);
                _mm256_storeu_si256(apps.as_mut_ptr().add(at).cast(), value);
                (value, _mm256_movemask_epi8(app) as u32)
            };
            *entry = entry_signs;
            entry_parity ^= entry_signs;
            parity = _mm256_xor_si256(parity, _mm256_cmpgt_epi8(zero, value));
            let magnitude = _mm256_abs_epi8(value);
            let below1 = _mm256_cmpgt_epi8(min1, magnitude);
            let below2 = _mm256_cmpgt_epi8(min2, magnitude);
            min2 = _mm256_blendv_epi8(_mm256_blendv_epi8(min2, magnitude, below2), min1, below1);
            min1 = _mm256_blendv_epi8(min1, magnitude, below1);
            arg1 = _mm256_blendv_epi8(arg1, index, below1);
            index = _mm256_add_epi8(index, one);
        }
        let (scaled1, scaled2) = if degree == 1 {
            (zero, zero)
        } else {
            (scale(params, min1), scale(params, min2))
        };
        // Lane bits: odd entry parity, then any change of sign.
        let mut flags = entry_parity;
        index = zero;
        for (edge, &entry) in signs[..degree].iter().enumerate() {
            let at = edge * stride + lane;
            // SAFETY: as in the first pass.
            unsafe {
                let value = _mm256_loadu_si256(apps.as_ptr().add(at).cast());
                let negative = _mm256_xor_si256(parity, _mm256_cmpgt_epi8(zero, value));
                let at_min = _mm256_cmpeq_epi8(arg1, index);
                index = _mm256_add_epi8(index, one);
                let magnitude = _mm256_blendv_epi8(scaled1, scaled2, at_min);
                let message =
                    _mm256_blendv_epi8(magnitude, _mm256_sub_epi8(zero, magnitude), negative);
                _mm256_storeu_si256(messages.as_mut_ptr().add(at).cast(), message);
                let app = _mm256_add_epi8(value, message);
                _mm256_storeu_si256(apps.as_mut_ptr().add(at).cast(), app);
                flags |= entry ^ _mm256_movemask_epi8(app) as u32;
            }
        }
        let real = width.saturating_sub(lane);
        let mask = if real >= LAYER_LANES {
            u32::MAX
        } else {
            (1u32 << real) - 1
        };
        unsettled |= flags & mask;
    }
    unsettled == 0
}
