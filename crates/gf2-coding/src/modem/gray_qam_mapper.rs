//! Optimized scalar Gray-QAM bit-to-symbol mapper.
//!
//! [`GrayQamMapper`] is the Gray-square-QAM fast path behind the
//! [`BatchMapper`] trait. It caches a post-normalization Gray-PAM-label →
//! level lookup table derived from the preset [`ModemSpec`] once at
//! construction, then maps each symbol by splitting its MSB-first label
//! into independent I/Q half-labels and indexing the table twice.
//!
//! Covers BPSK (order 2) and Gray square-QAM for orders 4, 16, 64, and 256,
//! matching the DVB-T2 EN 302 755 Table 14 layout locked by the
//! [`ModemSpec::gray_square_qam`](super::ModemSpec::gray_square_qam) preset.
//!
//! Future SIMD kernels replace the inner loop without touching the public
//! surface (task `c5cee991`).

use super::{BatchMapper, DefaultScalar, ModemScalar, ModemSpec, ModemView};

/// Scalar Gray-QAM bit-to-symbol mapper.
///
/// Constructed from a [`ModemSpec::gray_square_qam`](super::ModemSpec::gray_square_qam)
/// preset. Produces the same I/Q coordinates as the preset's per-label
/// [`ModemView::point`] lookup, while avoiding per-symbol label-to-index
/// scans by splitting labels into independent I/Q half-labels and indexing
/// a cached PAM lookup table.
///
/// # Examples
///
/// ```
/// use gf2_coding::modem::{BatchMapper, GrayQamMapper};
///
/// let mapper = GrayQamMapper::from_preset_order(16);
/// let bits = [false, false, false, false]; // label = 0
/// let mut i = [0.0_f32; 1];
/// let mut q = [0.0_f32; 1];
/// mapper.map_bits(&bits, &mut i, &mut q);
/// let expected = mapper.spec().point(0);
/// assert!((i[0] - expected.i).abs() < 1e-6);
/// assert!((q[0] - expected.q).abs() < 1e-6);
/// ```
///
/// # Complexity
///
/// Construction is O(M) in `order = M`. Mapping is O(num_symbols) with a
/// small per-symbol constant (two table indexings + a label assemble).
#[derive(Debug, Clone)]
pub struct GrayQamMapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Cached Gray-PAM-label → post-normalization level lookup.
    ///
    /// Length is `1 << m_half` for QAM (`m_half >= 1`). For BPSK the table
    /// is `[+1*scale, -1*scale]` (`scale == 1`) indexed by the raw bit.
    pam_levels: Vec<S>,
    /// Total bits per symbol (`log2(order)`).
    m_total: u8,
    /// Half bits per symbol for QAM (`m_total / 2`); `0` for BPSK.
    m_half: u8,
    /// Low-bit mask `(1 << m_half) - 1`; unused for BPSK.
    mask_half: u16,
    /// `true` iff this mapper was built from the BPSK preset.
    is_bpsk: bool,
}

impl GrayQamMapper<DefaultScalar> {
    /// Constructs the default-scalar (`f32`) mapper for a supported order.
    ///
    /// # Arguments
    ///
    /// * `order` - Constellation order; must be one of `2, 4, 16, 64, 256`.
    ///   `order = 2` selects BPSK.
    ///
    /// # Panics
    ///
    /// Panics with `"gray_square_qam: order must be one of 2, 4, 16, 64,
    /// 256 (got {order})"` via [`ModemSpec::gray_square_qam_with_scalar`]
    /// if `order` is unsupported.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BatchMapper, GrayQamMapper};
    ///
    /// let mapper = GrayQamMapper::from_preset_order(4);
    /// assert_eq!(mapper.spec().bits_per_symbol(), 2);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn from_preset_order(order: usize) -> Self {
        Self::from_preset_order_with_scalar(order)
    }
}

impl<S: ModemScalar> GrayQamMapper<S> {
    /// Scalar-generic companion of [`GrayQamMapper::from_preset_order`].
    ///
    /// Useful for `f64` research workflows.
    ///
    /// # Arguments
    ///
    /// * `order` - Constellation order; must be one of `2, 4, 16, 64, 256`.
    ///
    /// # Panics
    ///
    /// Panics via [`ModemSpec::gray_square_qam_with_scalar`] if `order` is
    /// unsupported.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{BatchMapper, GrayQamMapper};
    ///
    /// let mapper: GrayQamMapper<f64> =
    ///     GrayQamMapper::<f64>::from_preset_order_with_scalar(64);
    /// assert_eq!(mapper.spec().bits_per_symbol(), 6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn from_preset_order_with_scalar(order: usize) -> Self {
        let spec = ModemSpec::<S>::gray_square_qam_with_scalar(order);
        let m_total = spec.bits_per_symbol();
        let is_bpsk = m_total == 1;
        let (m_half, mask_half, table_len) = if is_bpsk {
            (0u8, 0u16, 2usize)
        } else {
            let m_half = m_total / 2;
            (m_half, (1u16 << m_half) - 1, 1usize << m_half)
        };

        // Derive the PAM table from the preset's points. The preset
        // guarantees consistency across all symbols that share an
        // i-label / q-label; we debug-assert it here as a safety net.
        let mut cached: Vec<Option<S>> = vec![None; table_len];
        let view = spec.view();
        if is_bpsk {
            // Preset guarantees point(0) = (+1, 0), point(1) = (-1, 0).
            cached[0] = Some(view.point(0).i);
            cached[1] = Some(view.point(1).i);
        } else {
            for (idx, label) in view.labels().iter().enumerate() {
                let i_label = ((label.bits >> m_half) & mask_half) as usize;
                let q_label = (label.bits & mask_half) as usize;
                let p = view.point(idx);
                match cached[i_label] {
                    None => cached[i_label] = Some(p.i),
                    Some(existing) => debug_assert!(
                        same_scalar::<S>(existing, p.i),
                        "GrayQamMapper: inconsistent I level for i_label {i_label}"
                    ),
                }
                match cached[q_label] {
                    None => cached[q_label] = Some(p.q),
                    Some(existing) => debug_assert!(
                        same_scalar::<S>(existing, p.q),
                        "GrayQamMapper: inconsistent Q level for q_label {q_label}"
                    ),
                }
            }
        }
        let pam_levels: Vec<S> = cached
            .into_iter()
            .map(|v| v.expect("GrayQamMapper: preset missing PAM level entry"))
            .collect();

        Self {
            spec,
            pam_levels,
            m_total,
            m_half,
            mask_half,
            is_bpsk,
        }
    }
}

/// Helper used only inside `debug_assert!`: bit-identical compare through
/// the lossless `to_f64` widening. Avoids depending on `PartialEq` beyond
/// what `ModemScalar` provides (it only requires `PartialOrd`).
#[inline]
fn same_scalar<S: ModemScalar>(a: S, b: S) -> bool {
    a.to_f64().to_bits() == b.to_f64().to_bits()
}

impl<S: ModemScalar> BatchMapper<S> for GrayQamMapper<S> {
    #[inline]
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    fn map_bits(&self, bits: &[bool], out_i: &mut [S], out_q: &mut [S]) {
        let m = self.m_total as usize;
        let num_symbols = super::bit_pack::check_batch_lengths(
            "GrayQamMapper::map_bits",
            self.m_total,
            bits.len(),
            out_i.len(),
            out_q.len(),
        );

        if self.is_bpsk {
            for k in 0..num_symbols {
                let b = bits[k] as usize;
                out_i[k] = self.pam_levels[b];
                out_q[k] = S::zero();
            }
            return;
        }

        let m_half = self.m_half;
        let mask_half = self.mask_half;
        for k in 0..num_symbols {
            let base = k * m;
            let label = super::bit_pack::pack_label_msb_first(&bits[base..base + m]);
            let i_label = ((label >> m_half) & mask_half) as usize;
            let q_label = (label & mask_half) as usize;
            out_i[k] = self.pam_levels[i_label];
            out_q[k] = self.pam_levels[q_label];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::bit_pack::unpack_label_msb_first as bits_msb_first;
    use super::*;
    use crate::modem::{BatchMapper, ModemSpec};
    use proptest::prelude::*;

    fn all_orders() -> [usize; 5] {
        [2, 4, 16, 64, 256]
    }

    #[test]
    fn test_map_bits_roundtrip_vs_preset_all_orders() {
        for order in all_orders() {
            let mapper = GrayQamMapper::from_preset_order(order);
            let spec = ModemSpec::gray_square_qam(order);
            let view = spec.view();
            let m = view.bits_per_symbol();
            for label in 0u16..(order as u16) {
                let bits = bits_msb_first(label, m);
                let mut oi = [0.0_f32; 1];
                let mut oq = [0.0_f32; 1];
                mapper.map_bits(&bits, &mut oi, &mut oq);
                let idx = view
                    .labels()
                    .iter()
                    .position(|l| l.bits == label)
                    .expect("label present");
                let expected = view.point(idx);
                assert!(
                    (oi[0] - expected.i).abs() < 1e-6,
                    "order {order} label {label} I: got {} want {}",
                    oi[0],
                    expected.i
                );
                assert!(
                    (oq[0] - expected.q).abs() < 1e-6,
                    "order {order} label {label} Q: got {} want {}",
                    oq[0],
                    expected.q
                );
            }
        }
    }

    #[test]
    fn test_map_bits_qpsk_matches_legacy_delta() {
        let mapper = GrayQamMapper::from_preset_order(4);
        let delta = (0.5_f64).sqrt() as f32;
        // (bits_MSB_first, expected_I, expected_Q)
        let cases = [
            ([false, false], delta, delta),
            ([false, true], delta, -delta),
            ([true, false], -delta, delta),
            ([true, true], -delta, -delta),
        ];
        for (bits, want_i, want_q) in cases {
            let mut oi = [0.0_f32; 1];
            let mut oq = [0.0_f32; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            assert!(
                (oi[0] - want_i).abs() < 1e-6,
                "QPSK I for {bits:?}: got {} want {want_i}",
                oi[0]
            );
            assert!(
                (oq[0] - want_q).abs() < 1e-6,
                "QPSK Q for {bits:?}: got {} want {want_q}",
                oq[0]
            );
        }
    }

    #[test]
    fn test_map_bits_bpsk_single_axis() {
        let mapper = GrayQamMapper::from_preset_order(2);
        let bits = [false, true];
        let mut oi = [0.0_f32; 2];
        let mut oq = [0.0_f32; 2];
        mapper.map_bits(&bits, &mut oi, &mut oq);
        assert!((oi[0] - 1.0).abs() < 1e-6);
        assert!((oi[1] + 1.0).abs() < 1e-6);
        assert_eq!(oq[0], 0.0);
        assert_eq!(oq[1], 0.0);
    }

    #[test]
    #[should_panic(expected = "order must be one of 2, 4, 16, 64, 256")]
    fn test_map_bits_invalid_order_panics() {
        let _ = GrayQamMapper::<f32>::from_preset_order(8);
    }

    #[test]
    #[should_panic(expected = "is not a multiple of bits_per_symbol")]
    fn test_map_bits_bits_length_mismatch_panics() {
        let mapper = GrayQamMapper::from_preset_order(16);
        let bits = vec![false; 5]; // not multiple of 4
        let mut oi = vec![0.0_f32; 1];
        let mut oq = vec![0.0_f32; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);
    }

    #[test]
    #[should_panic(expected = "out_i length")]
    fn test_map_bits_out_i_length_mismatch_panics() {
        let mapper = GrayQamMapper::from_preset_order(16);
        let bits = vec![false; 8]; // 2 symbols
        let mut oi = vec![0.0_f32; 1];
        let mut oq = vec![0.0_f32; 2];
        mapper.map_bits(&bits, &mut oi, &mut oq);
    }

    #[test]
    #[should_panic(expected = "out_q length")]
    fn test_map_bits_out_q_length_mismatch_panics() {
        let mapper = GrayQamMapper::from_preset_order(16);
        let bits = vec![false; 8]; // 2 symbols
        let mut oi = vec![0.0_f32; 2];
        let mut oq = vec![0.0_f32; 1];
        mapper.map_bits(&bits, &mut oi, &mut oq);
    }

    #[test]
    fn test_map_bits_f64_scalar_generic() {
        let mapper: GrayQamMapper<f64> = GrayQamMapper::<f64>::from_preset_order_with_scalar(64);
        let spec: ModemSpec<f64> = ModemSpec::<f64>::gray_square_qam_with_scalar(64);
        let view = spec.view();
        let m = view.bits_per_symbol();
        for label in 0u16..64u16 {
            let bits = bits_msb_first(label, m);
            let mut oi = [0.0_f64; 1];
            let mut oq = [0.0_f64; 1];
            mapper.map_bits(&bits, &mut oi, &mut oq);
            let idx = view
                .labels()
                .iter()
                .position(|l| l.bits == label)
                .expect("label present");
            let expected = view.point(idx);
            assert!((oi[0] - expected.i).abs() < 1e-12);
            assert!((oq[0] - expected.q).abs() < 1e-12);
        }
    }

    proptest! {
        #[test]
        fn test_map_bits_random_matches_preset(
            order_idx in 0usize..5,
            num_symbols in 0usize..64,
            seed in any::<u64>(),
        ) {
            let order = all_orders()[order_idx];
            let mapper = GrayQamMapper::from_preset_order(order);
            let spec = ModemSpec::gray_square_qam(order);
            let view = spec.view();
            let m = view.bits_per_symbol() as usize;

            // Deterministic pseudo-random bit generator seeded by `seed`,
            // routed through the shared SSOT modem test LCG.
            let mut rng = super::super::test_oracle::Lcg::new(seed | 1);
            let mut bits = Vec::with_capacity(num_symbols * m);
            for _ in 0..(num_symbols * m) {
                bits.push((rng.next_u64() >> 63) & 1 == 1);
            }
            let mut oi = vec![0.0_f32; num_symbols];
            let mut oq = vec![0.0_f32; num_symbols];
            mapper.map_bits(&bits, &mut oi, &mut oq);

            for k in 0..num_symbols {
                let label = super::super::bit_pack::pack_label_msb_first(
                    &bits[k * m..(k + 1) * m],
                );
                let idx = view
                    .labels()
                    .iter()
                    .position(|l| l.bits == label)
                    .expect("label present");
                let expected = view.point(idx);
                prop_assert!((oi[k] - expected.i).abs() < 1e-6);
                prop_assert!((oq[k] - expected.q).abs() < 1e-6);
            }
        }
    }
}
