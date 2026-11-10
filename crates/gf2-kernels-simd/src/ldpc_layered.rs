//! Layered min-sum updates over saturating `i8` messages. A layer is a set of
//! checks sharing no variable; lane `i` of every buffer is the layer's check
//! `i`, and value `edge * stride + i` is that check's edge `edge`.

#![forbid(unsafe_code)]

/// Lanes of one AVX2 vector of `i8`; a layer's stride is a multiple of it.
pub const LAYER_LANES: usize = 32;

/// The largest magnitude of an a-posteriori value.
pub const LAYER_SATURATION: i8 = 127;

/// The magnitude scaling of a quantized min-sum check update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayeredRule {
    /// Normalized min-sum: magnitude `m` becomes `(m * eighths) >> 3`.
    Normalized {
        /// The normalization factor in eighths, in `1..=8`.
        eighths: u8,
    },
    /// Offset min-sum: magnitude `m` becomes `max(m - beta, 0)`.
    Offset {
        /// The offset in quanta, in `0..=127`.
        beta: u8,
    },
}

/// A validated check rule and message saturation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerParams {
    rule: LayeredRule,
    message_limit: u8,
}

impl LayerParams {
    /// Pairs a rule with the magnitude bound of check-to-variable messages.
    ///
    /// # Panics
    ///
    /// Panics if `eighths` is outside `1..=8`, `beta` exceeds 127, or
    /// `message_limit` is outside `1..=126`.
    #[must_use]
    pub fn new(rule: LayeredRule, message_limit: u8) -> Self {
        match rule {
            LayeredRule::Normalized { eighths } => {
                assert!((1..=8).contains(&eighths), "eighths must be in 1..=8");
            }
            LayeredRule::Offset { beta } => assert!(beta <= 127, "beta must be at most 127"),
        }
        assert!(
            (1..=126).contains(&message_limit),
            "message limit must be in 1..=126"
        );
        Self {
            rule,
            message_limit,
        }
    }

    /// The check rule.
    #[must_use]
    pub fn rule(&self) -> LayeredRule {
        self.rule
    }

    /// The magnitude bound of check-to-variable messages.
    #[must_use]
    pub fn message_limit(&self) -> u8 {
        self.message_limit
    }

    /// The magnitude bound of variable-to-check values, `127 - message_limit`,
    /// so that a value plus a message is an exact `i8` a-posteriori value.
    #[must_use]
    pub fn extrinsic_limit(&self) -> u8 {
        127 - self.message_limit
    }

    /// Bounds `app - message` by the extrinsic limit.
    #[inline]
    pub(crate) fn extrinsic(self, app: i8, message: i8) -> i8 {
        let limit = i16::from(self.extrinsic_limit());
        (i16::from(app) - i16::from(message)).clamp(-limit, limit) as i8
    }

    /// Scales and saturates an excluded minimum magnitude in `0..=127`.
    #[inline]
    pub(crate) fn scale(self, magnitude: u8) -> u8 {
        let scaled = match self.rule {
            LayeredRule::Normalized { eighths } => {
                ((u16::from(magnitude) * u16::from(eighths)) >> 3) as u8
            }
            LayeredRule::Offset { beta } => magnitude.saturating_sub(beta),
        };
        scaled.min(self.message_limit)
    }
}

type LayerFn = fn(LayerParams, &mut [i8], &mut [i8], usize, usize);

/// A safe layer kernel, with a portable implementation on every target.
#[derive(Clone, Copy)]
pub struct LayerKernel {
    pub(crate) layer_fn: LayerFn,
}

impl LayerKernel {
    /// Scalar kernel.
    #[must_use]
    pub const fn portable() -> Self {
        Self {
            layer_fn: update_layer_scalar,
        }
    }

    /// Updates one layer in place.
    ///
    /// On entry `apps` holds each edge's variable a-posteriori value and
    /// `messages` the check-to-variable message the layer last wrote. Per lane
    /// and edge the kernel forms `v = app - message` bounded by
    /// ±[`LayerParams::extrinsic_limit`], writes the min-sum message of the
    /// other edges' `v` and replaces `app` with the exact `v + message`, at
    /// most [`LAYER_SATURATION`] in magnitude. Bounding `v` rather than the
    /// sum keeps `app - message` exact at the next update of the same check.
    /// The sign of `v >= 0` is positive. The excluded minimum starts at 127 and
    /// is replaced only on a strictly smaller magnitude, then is scaled by the
    /// rule and bounded by the message limit. A degree-one layer writes zero
    /// messages. Takes O(degree * stride) time without allocation; every
    /// input value, `-128` included, is accepted.
    ///
    /// # Panics
    ///
    /// Panics before writing unless `stride` is a multiple of
    /// [`LAYER_LANES`] and both lengths equal `degree * stride`, or when that
    /// product overflows `usize`.
    pub fn update_layer(
        &self,
        params: LayerParams,
        apps: &mut [i8],
        messages: &mut [i8],
        degree: usize,
        stride: usize,
    ) {
        assert_eq!(stride % LAYER_LANES, 0, "layer stride must fill vectors");
        let length = degree.checked_mul(stride).expect("layer size overflow");
        assert_eq!(apps.len(), length, "layer a-posteriori length");
        assert_eq!(messages.len(), length, "layer message length");
        (self.layer_fn)(params, apps, messages, degree, stride);
    }
}

/// AVX2 kernel when supported by the host; `None` on other targets or hosts.
#[must_use]
pub fn detect() -> Option<LayerKernel> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::ldpc_layered::detect();
    }
    #[allow(unreachable_code)]
    None
}

fn update_layer_scalar(
    params: LayerParams,
    apps: &mut [i8],
    messages: &mut [i8],
    degree: usize,
    stride: usize,
) {
    update_lanes_scalar(params, apps, messages, degree, stride, 0);
}

pub(crate) fn update_lanes_scalar(
    params: LayerParams,
    apps: &mut [i8],
    messages: &mut [i8],
    degree: usize,
    stride: usize,
    start: usize,
) {
    for lane in start..stride {
        let mut min1 = 127u8;
        let mut min2 = 127u8;
        let mut arg1 = usize::MAX;
        let mut negative_parity = false;
        for edge in 0..degree {
            let at = edge * stride + lane;
            let value = params.extrinsic(apps[at], messages[at]);
            apps[at] = value;
            negative_parity ^= value < 0;
            let magnitude = value.unsigned_abs();
            if magnitude < min1 {
                min2 = min1;
                min1 = magnitude;
                arg1 = edge;
            } else if magnitude < min2 {
                min2 = magnitude;
            }
        }
        let (scaled1, scaled2) = if degree == 1 {
            (0, 0)
        } else {
            (params.scale(min1), params.scale(min2))
        };
        for edge in 0..degree {
            let at = edge * stride + lane;
            let value = apps[at];
            let magnitude = if edge == arg1 { scaled2 } else { scaled1 } as i8;
            let message = if negative_parity ^ (value < 0) {
                -magnitude
            } else {
                magnitude
            };
            messages[at] = message;
            apps[at] = value + message;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state >> 33
    }

    /// The definition: each message is the scaled minimum over the other
    /// edges, signed by the product of their signs.
    fn reference(
        params: LayerParams,
        apps: &mut [i8],
        messages: &mut [i8],
        degree: usize,
        stride: usize,
    ) {
        for lane in 0..stride {
            let v: Vec<i8> = (0..degree)
                .map(|e| {
                    let limit = 127 - i16::from(params.message_limit());
                    let v =
                        i16::from(apps[e * stride + lane]) - i16::from(messages[e * stride + lane]);
                    v.clamp(-limit, limit) as i8
                })
                .collect();
            for e in 0..degree {
                let others = (0..degree).filter(|&o| o != e);
                let magnitude = if degree == 1 {
                    0
                } else {
                    params.scale(
                        others
                            .clone()
                            .map(|o| v[o].unsigned_abs())
                            .min()
                            .unwrap()
                            .min(127),
                    )
                } as i8;
                let negative = others.filter(|&o| v[o] < 0).count() % 2 == 1;
                let message = if negative { -magnitude } else { magnitude };
                messages[e * stride + lane] = message;
                let app = i16::from(v[e]) + i16::from(message);
                assert!(app.abs() <= 127);
                apps[e * stride + lane] = app as i8;
            }
        }
    }

    fn rules() -> Vec<LayerParams> {
        vec![
            LayerParams::new(LayeredRule::Normalized { eighths: 8 }, 126),
            LayerParams::new(LayeredRule::Normalized { eighths: 6 }, 31),
            LayerParams::new(LayeredRule::Normalized { eighths: 1 }, 1),
            LayerParams::new(LayeredRule::Offset { beta: 0 }, 63),
            LayerParams::new(LayeredRule::Offset { beta: 2 }, 15),
            LayerParams::new(LayeredRule::Offset { beta: 127 }, 126),
        ]
    }

    fn draw(state: &mut u64, kind: u64) -> i8 {
        match kind % 4 {
            0 => (lcg(state) & 0xff) as u8 as i8,
            1 => [-128, -127, -126, -1, 0, 1, 126, 127][(lcg(state) % 8) as usize],
            2 => ((lcg(state) % 9) as i8) - 4,
            _ => ((lcg(state) % 63) as i8) - 31,
        }
    }

    fn kernels() -> Vec<LayerKernel> {
        let mut kernels = vec![LayerKernel::portable()];
        kernels.extend(detect());
        kernels
    }

    #[test]
    fn every_backend_equals_the_definition() {
        let mut state = 0x9e37_79b9_7f4a_7c15;
        for kernel in kernels() {
            for params in rules() {
                for degree in [0, 1, 2, 3, 7, 19, 255, 256, 300] {
                    for stride in [32, 64, 96] {
                        for kind in 0..4 {
                            let length = degree * stride;
                            let apps: Vec<i8> =
                                (0..length).map(|_| draw(&mut state, kind)).collect();
                            let messages: Vec<i8> =
                                (0..length).map(|_| draw(&mut state, kind + 1)).collect();
                            let (mut ra, mut rm) = (apps.clone(), messages.clone());
                            reference(params, &mut ra, &mut rm, degree, stride);
                            let (mut ka, mut km) = (apps, messages);
                            kernel.update_layer(params, &mut ka, &mut km, degree, stride);
                            assert_eq!((ka, km), (ra, rm), "{params:?} d={degree} s={stride}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_avx2_kernel_is_detected_on_avx2_hosts() {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert_eq!(
            detect().is_some(),
            std::arch::is_x86_feature_detected!("avx2")
        );
    }

    #[test]
    #[should_panic(expected = "layer stride must fill vectors")]
    fn a_partial_vector_stride_panics() {
        LayerKernel::portable().update_layer(
            LayerParams::new(LayeredRule::Offset { beta: 0 }, 63),
            &mut [0; 33],
            &mut [0; 33],
            1,
            33,
        );
    }

    #[test]
    #[should_panic(expected = "layer message length")]
    fn a_length_mismatch_panics() {
        LayerKernel::portable().update_layer(
            LayerParams::new(LayeredRule::Offset { beta: 0 }, 63),
            &mut [0; 64],
            &mut [0; 32],
            2,
            32,
        );
    }

    #[test]
    #[should_panic(expected = "message limit must be in 1..=126")]
    fn a_message_limit_without_extrinsic_room_panics() {
        let _ = LayerParams::new(LayeredRule::Offset { beta: 0 }, 127);
    }

    #[test]
    #[should_panic(expected = "eighths must be in 1..=8")]
    fn a_zero_normalization_panics() {
        let _ = LayerParams::new(LayeredRule::Normalized { eighths: 0 }, 63);
    }
}
