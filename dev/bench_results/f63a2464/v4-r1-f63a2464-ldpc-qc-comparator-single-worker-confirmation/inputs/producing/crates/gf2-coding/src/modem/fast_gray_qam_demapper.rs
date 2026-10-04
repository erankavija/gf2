//! Fast Gray square-QAM soft demapper exploiting I/Q axis separability.
//!
//! [`FastGrayQamDemapper`] is the specialized [`BatchSoftDemapper`] for
//! the Gray-coded square-QAM presets (and BPSK as the degenerate
//! single-axis case). Because under AWGN with independent I/Q noise the
//! per-symbol 2D log-MAP factorizes into two 1D Gray-PAM demaps of size
//! `sqrt(M)` each, the hot-path cost drops from
//! `O(num_symbols * M * m)` for the arbitrary-constellation
//! [`super::ReferenceSoftDemapper`] to
//! `O(num_symbols * sqrt(M) * m)`.
//!
//! # Separability with complex channel gains
//!
//! The demapper accepts the same optional `(gain_i, gain_q)` pair as
//! every other backend. For a complex gain `h = h_i + j h_q`, the
//! squared distance to the (complex) scaled constellation point `h * p`
//! can be rewritten by multiplying through by `conj(h)`:
//!
//! ```text
//! |y - h p|^2 / N0 = |z - |h|^2 p|^2 / (N0 |h|^2)
//! ```
//!
//! where `z = conj(h) * y`. Expanding the right-hand side yields a sum
//! of I-only and Q-only squared terms, so the separable kernel applies
//! after a per-symbol pre-rotation by `conj(h)` and a rescaling of the
//! PAM levels by `|h|^2`. With `gain_i = gain_q = None` this reduces to
//! the identity transform and the kernel falls back to the plain AWGN
//! path.
//!
//! # Bit ordering and sign convention
//!
//! Bit ordering follows the framework-wide MSB-first intra-symbol
//! convention: index `k = 0` is the MSB of the [`super::LabelWord`],
//! and for Gray square-QAM the first `m/2` MSBs are the I-axis Gray-PAM
//! label (MSB = coarsest level) while the remaining `m/2` bits are the
//! Q-axis label. LLR sign follows [`crate::llr::Llr`]: **positive LLR =
//! bit 0 more likely**, exactly matching
//! [`super::ReferenceSoftDemapper`].

use crate::llr::Llr;

use super::{BatchSoftDemapper, DemapInput, DemapMethod, ModemScalar, ModemSpec, ModemView};

use gf2_kernels_simd::modem::{self as kernel_modem, GrayPamDistanceFnsF64};
use std::sync::OnceLock;

/// Returns the cached best-available Gray-PAM distance kernel bundle.
///
/// Dispatch cost is amortized: the first call runs CPU-feature
/// detection via `gf2_kernels_simd::modem::detect_f64`, and all
/// subsequent calls return the same `&'static` bundle through a
/// `OnceLock` read. The bundle always yields a working kernel (scalar
/// fallback when no SIMD backend matches, AVX2 on x86_64 hosts that
/// advertise it), so the demap hot path never needs a `None` branch
/// and no architecture-specific `cfg` gating is required here.
#[inline]
fn scalar_fns_f64_static() -> &'static GrayPamDistanceFnsF64 {
    static FNS: OnceLock<GrayPamDistanceFnsF64> = OnceLock::new();
    FNS.get_or_init(kernel_modem::scalar_fns_f64)
}

fn kernel_fns_f64() -> &'static GrayPamDistanceFnsF64 {
    static FNS: OnceLock<GrayPamDistanceFnsF64> = OnceLock::new();
    FNS.get_or_init(kernel_modem::detect_f64)
}

/// Fast soft demapper specialized for Gray square-QAM presets.
///
/// Accepts BPSK (order 2, `m = 1`) and the Gray-coded square-QAM presets
/// of orders 4, 16, 64, and 256 (`m = 2, 4, 6, 8`). The constructor
/// validates the passed [`ModemSpec`] against the locked preset bit-channel
/// layout and panics on any non-matching spec; use
/// [`super::ReferenceSoftDemapper`] for arbitrary constellations.
///
/// # Examples
///
/// ```
/// use gf2_coding::llr::Llr;
/// use gf2_coding::modem::{
///     BatchSoftDemapper, DemapInput, DemapMethod, FastGrayQamDemapper, ModemSpec,
/// };
///
/// let spec = ModemSpec::<f32>::bpsk();
/// let demapper = FastGrayQamDemapper::new(spec);
/// let rx_i = [0.8_f32];
/// let rx_q = [0.0_f32];
/// let noise_var = [0.5_f32];
/// let input = DemapInput::<f32> {
///     rx_i: &rx_i,
///     rx_q: &rx_q,
///     gain_i: None,
///     gain_q: None,
///     noise_var: &noise_var,
///     method: DemapMethod::ExactLogMap,
/// };
/// let mut out = [Llr::new(0.0); 1];
/// demapper.demap_llrs(input, &mut out);
/// assert!(out[0].value() > 0.0);
/// ```
pub struct FastGrayQamDemapper<S: ModemScalar> {
    spec: ModemSpec<S>,
    /// Total bits per symbol (`log2(order)`).
    m_total: u8,
    /// Half bits per symbol for QAM (`m_total / 2`); `0` for BPSK.
    m_half: u8,
    /// `true` if this is the BPSK (single-axis) preset.
    is_bpsk: bool,
    /// PAM squared-distance kernel pointer. Normally resolved to the
    /// runtime-best kernel (AVX2 when available, scalar otherwise) via
    /// [`gf2_kernels_simd::modem::detect_f64`]; callers that need to pin
    /// the scalar backend for benchmarking can construct the demapper via
    /// [`Self::new_with_scalar_kernel`].
    kernel_fns: &'static GrayPamDistanceFnsF64,
    /// Post-normalization Gray-PAM levels on each axis, indexed by the
    /// `m_half`-bit Gray label (MSB-first within the half-label). Length
    /// `1 << m_half` for QAM, or `2` for BPSK (indexed by the raw bit).
    pam_levels: Vec<f64>,
}

impl<S: ModemScalar> FastGrayQamDemapper<S> {
    /// Constructs a fast Gray-QAM demapper from a Gray-square-QAM preset.
    ///
    /// # Arguments
    ///
    /// * `spec` - A [`ModemSpec`] whose metadata **and** geometry match
    ///   the Gray-square-QAM layout. The canonical way to obtain one is
    ///   [`ModemSpec::bpsk`], [`ModemSpec::gray_square_qam`], or their
    ///   `*_with_scalar` variants. Custom [`super::ModemSpecBuilder`]
    ///   specs are accepted iff they pass every check listed under
    ///   Panics below. The constructor verifies both the metadata shape
    ///   and the axis-separable PAM geometry, so specs that spoof the
    ///   metadata but ship mismatched points are rejected at
    ///   construction — there is no silent "undefined LLR" path.
    ///
    /// # Panics
    ///
    /// Panics with a descriptive message if any of the following fails:
    ///
    /// - `bits_per_symbol` is not one of `1, 2, 4, 6, 8`.
    /// - `num_symbols != 2^bits_per_symbol`.
    /// - For BPSK, `bit_channels[0] != SingleAxisPam(0)`; for QAM, bit
    ///   channels do not follow `m/2` `IAxisPam` entries followed by
    ///   `m/2` `QAxisPam` entries in MSB-first order.
    /// - Capabilities do not advertise both `ExactLogMap` and `MaxLog`.
    /// - (QAM) Two symbols share an I-half-label but have different I
    ///   coordinates (I-axis factorisation failed), or the analogous
    ///   Q-half-label / Q-coordinate condition fails.
    /// - (QAM) Some I-half-label or Q-half-label is not populated by
    ///   any symbol.
    /// - (QAM) The per-label Q mapping disagrees with the I mapping:
    ///   the kernel reuses the I-derived level table for both axes, so
    ///   `q_levels[label] == pam_levels[label]` must hold for every
    ///   label value.
    /// - (BPSK) The two points are not stored in label order (label 0
    ///   at index 0, label 1 at index 1), or they do not share a common
    ///   Q coordinate (the BPSK kernel treats `label == index` and
    ///   drops Q as a common additive constant).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(64));
    /// assert_eq!(demapper.spec_ref().bits_per_symbol(), 6);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn new(spec: ModemSpec<S>) -> Self {
        Self::new_with_kernel(spec, kernel_fns_f64())
    }

    /// Builds the same fast demapper but pinned to the **scalar** PAM
    /// distance kernel, bypassing the runtime AVX2 dispatch.
    ///
    /// This is a benchmarking affordance, not a production construction
    /// path — the default [`Self::new`] constructor auto-selects the
    /// best-available kernel (AVX2 on x86_64 hosts that advertise it,
    /// scalar otherwise) and that remains the right choice for all
    /// production callers. Use this method only when a benchmark needs
    /// to measure the scalar full-demapper baseline in isolation from
    /// whatever kernel the host would otherwise detect (see
    /// `crates/gf2-coding/benches/cpu_dispatch_probe.rs`).
    ///
    /// # Arguments
    ///
    /// * `spec` — a validated Gray-square-QAM modem spec, as for
    ///   [`Self::new`].
    ///
    /// # Panics
    ///
    /// Same invariants as [`Self::new`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let spec = ModemSpec::<f32>::gray_square_qam(16);
    /// let _scalar = FastGrayQamDemapper::new_with_scalar_kernel(spec);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(M) in `order = M`.
    pub fn new_with_scalar_kernel(spec: ModemSpec<S>) -> Self {
        Self::new_with_kernel(spec, scalar_fns_f64_static())
    }

    fn new_with_kernel(spec: ModemSpec<S>, kernel_fns: &'static GrayPamDistanceFnsF64) -> Self {
        // SSOT validator: confirms layout, bit-channel semantics, and
        // that every (i_label, q_label) resolves to the canonical
        // Gray-PAM level table (ruling out permuted-level specs that
        // would silently produce wrong LLRs through the axis-separable
        // kernel).
        super::presets::assert_valid_gray_square_qam_spec(&spec.view());

        let m_total = spec.bits_per_symbol();
        let is_bpsk = m_total == 1;
        let m_half = if is_bpsk { 0u8 } else { m_total / 2 };

        // SSOT Gray-PAM level derivation. The validator above has
        // already asserted that the spec's points match this table, so
        // there is no divergence risk from recomputing it here.
        let pam_levels: Vec<f64> = super::presets::gray_pam_levels::<f64>(m_total);

        Self {
            spec,
            m_total,
            m_half,
            is_bpsk,
            kernel_fns,
            pam_levels,
        }
    }

    /// Returns the post-normalization Gray-PAM level table shared between
    /// the I and Q axes.
    ///
    /// The table is indexed by the raw Gray-PAM axis label (MSB-first
    /// within the `m/2`-bit half-label for QAM, or the single raw bit for
    /// BPSK) and has length `1 << (m / 2)` for QAM or exactly `2` for
    /// BPSK. The validator in [`Self::new`] guarantees both axes use the
    /// same level set, so GPU and alternate-backend adapters can reuse
    /// this one table rather than rederiving it from the
    /// [`super::ModemSpec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(16));
    /// assert_eq!(demapper.pam_levels().len(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn pam_levels(&self) -> &[f64] {
        &self.pam_levels
    }

    /// Returns a borrowed reference to the owned [`ModemSpec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::modem::{FastGrayQamDemapper, ModemSpec};
    ///
    /// let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(16));
    /// assert_eq!(demapper.spec_ref().bits_per_symbol(), 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1).
    #[inline]
    pub fn spec_ref(&self) -> &ModemSpec<S> {
        &self.spec
    }
}

/// Computes the 1D Gray-PAM LLR for bit `bit_idx` (MSB-first within the
/// `m_bits`-wide half-label) given per-level squared distances `d`.
///
/// `d.len() == 1 << m_bits`, indexed by the raw Gray label. `method`
/// selects exact log-MAP or max-log. The subset-reduction math itself
/// lives in [`super::demapper::subset_log_map_llr`] — this is a thin
/// wrapper that supplies the "label = index" mapping used on a PAM
/// axis.
#[inline]
fn pam_axis_llr(d: &[f64], m_bits: u8, bit_idx: u8, method: DemapMethod) -> f64 {
    super::demapper::subset_log_map_llr(d, |j| j as u16, d.len(), m_bits, bit_idx, method)
}

impl<S: ModemScalar> BatchSoftDemapper<S> for FastGrayQamDemapper<S> {
    fn spec(&self) -> ModemView<'_, S> {
        self.spec.view()
    }

    /// Batched, allocation-minimal Gray-QAM soft demap.
    ///
    /// The hot path is organized as three contiguous passes over the
    /// symbol batch:
    ///
    /// 1. **Pre-rotation**: walk `num_symbols` once to compute
    ///    `z_i[s], z_q[s], g[s], inv_n0_eq[s]` into stack-friendly
    ///    scratch `Vec<f64>` buffers (sized once, not per-symbol).
    /// 2. **Distance kernel**: call the runtime-selected Gray-PAM
    ///    squared-distance kernel once per axis, filling a
    ///    `num_symbols * axis_len` contiguous distance slab. This is
    ///    the SIMD plug-in point — the default dispatch is AVX2 on
    ///    x86_64, scalar otherwise. For BPSK the Q axis is skipped
    ///    (its only contribution is a common additive constant).
    /// 3. **LLR reduction**: walk the distance slab with the shared
    ///    subset-reduction helper (`subset_log_map_llr` in the
    ///    crate-private `super::demapper` module) to produce the final
    ///    LLRs in the canonical symbol-major, MSB-first layout.
    ///
    /// The zero-gain guard is expressed by writing `inv_n0_eq[s] = 0`
    /// into the pre-rotation scratch; the kernel contract
    /// (see [`gf2_kernels_simd::modem`]) emits a zero distance slab
    /// for that symbol, and the LLR reduction naturally yields zero
    /// on balanced Gray presets.
    ///
    /// # Complexity
    ///
    /// `O(num_symbols * sqrt(M) * m)` where `M = 2^m` is the
    /// constellation order; the distance kernel and the reduction are
    /// both linear in the per-symbol work.
    fn demap_llrs(&self, input: DemapInput<'_, S>, out_llrs: &mut [Llr]) {
        let m = self.m_total as usize;
        let view = self.spec.view();
        let num_symbols = super::demapper::validate_demap_input(
            "FastGrayQamDemapper::demap_llrs",
            &view,
            &input,
            out_llrs.len(),
        );

        if num_symbols == 0 {
            return;
        }

        let axis_len = if self.is_bpsk {
            2
        } else {
            1usize << self.m_half
        };

        // Pass 1: build contiguous per-symbol scratch for the kernel.
        // All four buffers are allocated exactly once per call; the
        // per-symbol inner loop does no allocation and writes through
        // contiguous indices so both SIMD and scalar backends see an
        // allocation-free hot path.
        let mut z_i: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut z_q: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut g_scratch: Vec<f64> = Vec::with_capacity(num_symbols);
        let mut inv_n0_eq: Vec<f64> = Vec::with_capacity(num_symbols);
        for k in 0..num_symbols {
            let y_i = input.rx_i[k].to_f64();
            let y_q = input.rx_q[k].to_f64();
            let (h_i, h_q) = match (input.gain_i, input.gain_q) {
                (Some(gi), Some(gq)) => (gi[k].to_f64(), gq[k].to_f64()),
                _ => (1.0_f64, 0.0_f64),
            };
            let n0 = input.noise_var[k].to_f64();
            assert!(
                n0 > 0.0 && n0.is_finite(),
                "FastGrayQamDemapper::demap_llrs: noise_var[{k}] = {n0} must be positive and finite"
            );

            // Pre-rotate by conj(h) so the kernel runs on axis-separable
            // data: |y - h p|^2 / n0 == |z - g p|^2 / (n0 * g) where
            // z = conj(h) * y and g = |h|^2.
            let g = h_i * h_i + h_q * h_q;
            z_i.push(h_i * y_i + h_q * y_q);
            z_q.push(h_i * y_q - h_q * y_i);
            g_scratch.push(g);

            // Zero (or vanishingly small) channel gain: every
            // constellation point has identical squared distance from
            // y, so the log-MAP posterior degenerates to a label-count
            // ratio. For our balanced presets every bit channel
            // carries an equal count of 0- and 1-labels, so the LLR
            // is exactly zero. Feeding `inv_n0_eq = 0` to the distance
            // kernel makes it emit a zero distance slab for this
            // symbol (contract defined in `gf2_kernels_simd::modem`);
            // the subsequent subset-reduction then produces zero LLRs
            // without touching NaN. Matches the reference demapper's
            // behaviour at h = 0 on these presets.
            let n0_eq = n0 * g;
            let inv = if n0_eq <= f64::EPSILON * n0 {
                0.0
            } else {
                1.0 / n0_eq
            };
            inv_n0_eq.push(inv);
        }

        // Pass 2: distance-kernel dispatch. I axis always, Q axis only
        // for QAM. Levels are the same on both axes (enforced in
        // `FastGrayQamDemapper::new`), so we reuse the single level
        // table for both kernel calls.
        let mut d_i: Vec<f64> = vec![0.0; num_symbols * axis_len];
        let mut d_q: Vec<f64> = if self.is_bpsk {
            Vec::new()
        } else {
            vec![0.0; num_symbols * axis_len]
        };

        run_pam_distance_kernel(
            self.kernel_fns,
            &z_i,
            &g_scratch,
            &inv_n0_eq,
            &self.pam_levels,
            &mut d_i,
        );
        if !self.is_bpsk {
            run_pam_distance_kernel(
                self.kernel_fns,
                &z_q,
                &g_scratch,
                &inv_n0_eq,
                &self.pam_levels,
                &mut d_q,
            );
        }

        // Pass 3: LLR reduction over each per-symbol distance slab.
        if self.is_bpsk {
            for k in 0..num_symbols {
                let slab = &d_i[k * axis_len..(k + 1) * axis_len];
                let llr = pam_axis_llr(slab, 1, 0, input.method);
                out_llrs[k * m] = Llr::new(llr as f32);
            }
        } else {
            let m_half = self.m_half;
            for k in 0..num_symbols {
                let i_slab = &d_i[k * axis_len..(k + 1) * axis_len];
                let q_slab = &d_q[k * axis_len..(k + 1) * axis_len];
                // First m_half bits are I-axis, remainder are Q-axis.
                for b in 0..m_half {
                    let llr = pam_axis_llr(i_slab, m_half, b, input.method);
                    out_llrs[k * m + b as usize] = Llr::new(llr as f32);
                }
                for b in 0..m_half {
                    let llr = pam_axis_llr(q_slab, m_half, b, input.method);
                    out_llrs[k * m + (m_half + b) as usize] = Llr::new(llr as f32);
                }
            }
        }
    }
}

/// Runs the Gray-PAM squared-distance kernel on a single axis.
///
/// Dispatches unconditionally to the runtime-selected
/// `gf2-kernels-simd` bundle (AVX2 on x86_64 hosts that advertise the
/// feature, scalar everywhere else). The output layout is
/// symbol-major: `out[s * pam_levels.len() + l]` is the squared
/// distance between the pre-rotated sample `z[s]` and the `l`-th
/// Gray-PAM level. The zero-gain contract
/// (`inv_n0_eq[s] == 0 ⇒ distance slab all zero`) is enforced inside
/// the kernel by every backend.
#[inline]
fn run_pam_distance_kernel(
    fns: &GrayPamDistanceFnsF64,
    z: &[f64],
    g: &[f64],
    inv_n0_eq: &[f64],
    pam_levels: &[f64],
    out: &mut [f64],
) {
    (fns.pam_sq_distances_fn)(z, g, inv_n0_eq, pam_levels, out);
}

#[cfg(test)]
mod tests {
    use super::super::{
        BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ModemSpecBuilder, Normalization,
        ReferenceSoftDemapper, SymbolPoint,
    };
    use super::FastGrayQamDemapper;
    use crate::llr::Llr;
    use crate::modem::LabelWord;

    /// All supported preset orders.
    const PRESET_ORDERS: [usize; 5] = [2, 4, 16, 64, 256];

    fn method_seed(m: DemapMethod) -> u64 {
        match m {
            DemapMethod::ExactLogMap => 0xA1,
            DemapMethod::MaxLog => 0xB2,
        }
    }

    fn spec_for_order_f32(order: usize) -> ModemSpec<f32> {
        if order == 2 {
            ModemSpec::<f32>::bpsk()
        } else {
            ModemSpec::<f32>::gray_square_qam(order)
        }
    }

    fn spec_for_order_f64(order: usize) -> ModemSpec<f64> {
        if order == 2 {
            ModemSpec::<f64>::bpsk_with_scalar()
        } else {
            ModemSpec::<f64>::gray_square_qam_with_scalar(order)
        }
    }

    // Deterministic LCG for test inputs is shared with the rest of the
    // modem test surface via `crate::modem::test_oracle::Lcg`.
    use crate::modem::test_oracle::Lcg;

    fn assert_close_f32(a: &[Llr], b: &[Llr], tol: f32, ctx: &str) {
        assert_eq!(a.len(), b.len(), "{ctx}: length mismatch");
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            let dx = (x.value() - y.value()).abs();
            assert!(
                dx <= tol,
                "{ctx}: mismatch at {i}: fast={}, ref={}, |d|={dx}",
                x.value(),
                y.value()
            );
        }
    }

    #[test]
    fn test_fast_matches_reference_awgn_f32() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f32(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xDEADBEEF ^ (order as u64) ^ method_seed(method));
                let batch = 64usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push((rng.next_unit_f64() * 2.0) as f32);
                    rx_q.push((rng.next_unit_f64() * 2.0) as f32);
                    nv.push(rng.next_positive_f64(0.05, 2.0) as f32);
                }

                let input = DemapInput::<f32> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-3,
                    &format!("f32 AWGN order={order} method={method:?}"),
                );
            }
        }
    }

    #[test]
    fn test_fast_matches_reference_awgn_f64() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f64(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xC0FFEE ^ (order as u64) ^ method_seed(method));
                let batch = 64usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push(rng.next_unit_f64() * 2.0);
                    rx_q.push(rng.next_unit_f64() * 2.0);
                    nv.push(rng.next_positive_f64(0.05, 2.0));
                }

                let input = DemapInput::<f64> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: None,
                    gain_q: None,
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                // Llr is f32-backed so tolerance is still f32-sized when
                // comparing values; drive it tighter than the f32 test.
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-4,
                    &format!("f64 AWGN order={order} method={method:?}"),
                );
            }
        }
    }

    #[test]
    fn test_fast_matches_reference_with_complex_gain_f64() {
        // Exercise the conj(h) pre-rotation path with non-trivial fading.
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        for &order in &PRESET_ORDERS {
            for method in methods {
                let spec = spec_for_order_f64(order);
                let m = spec.bits_per_symbol() as usize;
                let fast = FastGrayQamDemapper::new(spec.clone());
                let reference = ReferenceSoftDemapper::new(spec);

                let mut rng = Lcg::new(0xFADED ^ (order as u64) ^ method_seed(method));
                let batch = 48usize;
                let mut rx_i = Vec::with_capacity(batch);
                let mut rx_q = Vec::with_capacity(batch);
                let mut gi = Vec::with_capacity(batch);
                let mut gq = Vec::with_capacity(batch);
                let mut nv = Vec::with_capacity(batch);
                for _ in 0..batch {
                    rx_i.push(rng.next_unit_f64() * 2.0);
                    rx_q.push(rng.next_unit_f64() * 2.0);
                    // Avoid near-zero |h| that would blow up the
                    // equalized noise variance.
                    let hi = if rng.next_unit_f64() > 0.0 { 0.6 } else { -0.6 }
                        + 0.3 * rng.next_unit_f64();
                    let hq = 0.2 * rng.next_unit_f64();
                    gi.push(hi);
                    gq.push(hq);
                    nv.push(rng.next_positive_f64(0.05, 1.0));
                }

                let input = DemapInput::<f64> {
                    rx_i: &rx_i,
                    rx_q: &rx_q,
                    gain_i: Some(&gi),
                    gain_q: Some(&gq),
                    noise_var: &nv,
                    method,
                };
                let mut out_fast = vec![Llr::new(0.0); batch * m];
                let mut out_ref = vec![Llr::new(0.0); batch * m];
                fast.demap_llrs(input, &mut out_fast);
                reference.demap_llrs(input, &mut out_ref);
                assert_close_f32(
                    &out_fast,
                    &out_ref,
                    1e-3,
                    &format!("f64 fading order={order} method={method:?}"),
                );
            }
        }
    }

    /// Sweep batch sizes that cross the AVX2 8-lane boundary to prove
    /// the batched kernel dispatch is size-agnostic: every preset must
    /// match the reference demapper for sizes 0, 1, 7, 8, 9, 15, 16,
    /// 17, 31, 32 on both methods. The zero-length case exercises the
    /// empty-batch short-circuit added alongside the kernel refactor.
    #[test]
    fn test_batch_size_sweep_crosses_avx2_boundary() {
        let methods = [DemapMethod::ExactLogMap, DemapMethod::MaxLog];
        let sizes = [0usize, 1, 7, 8, 9, 15, 16, 17, 31, 32];
        for &order in &PRESET_ORDERS {
            for method in methods {
                for &n in &sizes {
                    let spec = spec_for_order_f32(order);
                    let m = spec.bits_per_symbol() as usize;
                    let fast = FastGrayQamDemapper::new(spec.clone());
                    let reference = ReferenceSoftDemapper::new(spec);

                    let mut rng =
                        Lcg::new(0xB47C_415E ^ (order as u64) ^ method_seed(method) ^ (n as u64));
                    let mut rx_i = Vec::with_capacity(n);
                    let mut rx_q = Vec::with_capacity(n);
                    let mut nv = Vec::with_capacity(n);
                    for _ in 0..n {
                        rx_i.push((rng.next_unit_f64() * 2.0) as f32);
                        rx_q.push((rng.next_unit_f64() * 2.0) as f32);
                        nv.push(rng.next_positive_f64(0.05, 2.0) as f32);
                    }
                    let input = DemapInput::<f32> {
                        rx_i: &rx_i,
                        rx_q: &rx_q,
                        gain_i: None,
                        gain_q: None,
                        noise_var: &nv,
                        method,
                    };
                    let mut out_fast = vec![Llr::new(0.0); n * m];
                    let mut out_ref = vec![Llr::new(0.0); n * m];
                    fast.demap_llrs(input, &mut out_fast);
                    reference.demap_llrs(input, &mut out_ref);
                    assert_close_f32(
                        &out_fast,
                        &out_ref,
                        1e-3,
                        &format!("batch sweep order={order} n={n} method={method:?}"),
                    );
                }
            }
        }
    }

    #[test]
    fn test_zero_channel_gain_emits_zero_llrs() {
        // When h = 0 every constellation point has identical squared
        // distance from y, so the log-MAP LLR collapses to the
        // 0-bit/1-bit label count ratio. For balanced presets that
        // ratio is exactly 1, i.e. LLR = 0. The fast path must produce
        // finite zeros rather than NaN from dividing by |h|^2 == 0.
        for &order in &PRESET_ORDERS {
            let spec = spec_for_order_f64(order);
            let m = spec.bits_per_symbol() as usize;
            let fast = FastGrayQamDemapper::new(spec);
            let rx_i = [0.4_f64];
            let rx_q = [-0.3_f64];
            let gi = [0.0_f64];
            let gq = [0.0_f64];
            let nv = [0.5_f64];
            let input = DemapInput::<f64> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: Some(&gi),
                gain_q: Some(&gq),
                noise_var: &nv,
                method: DemapMethod::ExactLogMap,
            };
            let mut out = vec![Llr::new(0.0); m];
            fast.demap_llrs(input, &mut out);
            for (b, llr) in out.iter().enumerate() {
                assert!(
                    llr.value().is_finite(),
                    "order={order} bit={b}: LLR {} not finite at zero channel gain",
                    llr.value()
                );
                assert!(
                    llr.value().abs() < 1e-6,
                    "order={order} bit={b}: expected ~0 LLR at zero gain, got {}",
                    llr.value()
                );
            }
        }
    }

    #[test]
    fn test_bpsk_closed_form_high_snr() {
        // BPSK sanity: L = 4*y / N0 at unit gain. Smoke-tests the
        // single-axis branch beyond the cross-check against the
        // reference.
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::bpsk());
        let rx_i = [0.8_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 1];
        demapper.demap_llrs(input, &mut out);
        let expected = 4.0 * 0.8 / 0.5;
        assert!(
            (out[0].value() - expected).abs() < 1e-4,
            "BPSK fast-path closed form mismatch: got {}, want {expected}",
            out[0].value()
        );
    }

    #[test]
    #[should_panic(expected = "rx_i.len()")]
    fn test_length_mismatch_rx_q_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32, 0.0];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32, 0.5];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 4];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "out_llrs.len()")]
    fn test_length_mismatch_out_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: None,
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 3];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "gain_i and gain_q must be both Some or both None")]
    fn test_half_specified_gain_panics() {
        let demapper = FastGrayQamDemapper::new(ModemSpec::<f32>::gray_square_qam(4));
        let rx_i = [0.0_f32];
        let rx_q = [0.0_f32];
        let nv = [0.5_f32];
        let gi = [1.0_f32];
        let input = DemapInput::<f32> {
            rx_i: &rx_i,
            rx_q: &rx_q,
            gain_i: Some(&gi),
            gain_q: None,
            noise_var: &nv,
            method: DemapMethod::ExactLogMap,
        };
        let mut out = [Llr::new(0.0); 2];
        demapper.demap_llrs(input, &mut out);
    }

    #[test]
    #[should_panic(expected = "bits_per_symbol 3 is not one of")]
    fn test_unsupported_bits_per_symbol_panics() {
        // Build a custom 8-point (m=3) spec via the public builder: it is
        // valid as a research constellation but is not a Gray square-QAM
        // preset, so the fast path must reject it.
        let points: Vec<SymbolPoint<f32>> = (0..8)
            .map(|k| {
                let theta = (k as f32) * std::f32::consts::PI / 4.0;
                SymbolPoint::new(theta.cos(), theta.sin())
            })
            .collect();
        let labels: Vec<LabelWord> = (0u16..8).map(|b| LabelWord::new(b, 3)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(3)
            .points(points)
            .labels(labels)
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "BPSK spec must store label 0 at index 0")]
    fn test_permuted_bpsk_label_order_rejected() {
        // Custom BPSK spec with labels stored in reversed order:
        // index 0 => label 1, index 1 => label 0. The fast kernel
        // assumes label == index, so must reject this at construction.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points = vec![SymbolPoint::new(1.0, 0.0), SymbolPoint::new(-1.0, 0.0)];
        let labels = vec![LabelWord::new(1, 1), LabelWord::new(0, 1)];
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(points)
            .labels(labels)
            .bit_channels(vec![BitChannelSemantics::SingleAxisPam(0)])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "common Q coordinate")]
    fn test_bpsk_non_common_q_coordinate_rejected() {
        // Custom BPSK spec where the two points have different Q
        // coordinates. The fast kernel drops Q as a common additive
        // constant, so a non-common Q would yield wrong LLRs — reject.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points = vec![SymbolPoint::new(1.0, 0.3), SymbolPoint::new(-1.0, -0.3)];
        let labels = vec![LabelWord::new(0, 1), LabelWord::new(1, 1)];
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(1)
            .points(points)
            .labels(labels)
            .bit_channels(vec![BitChannelSemantics::SingleAxisPam(0)])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(1.09))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "expected canonical Gray-PAM level")]
    fn test_permuted_q_label_mapping_rejected() {
        // A 4-QAM spec with valid I/Q factorisation, matching level
        // sets ({+1, -1} on both axes), but a Q-label permutation that
        // disagrees with the I mapping: Q-half-label 0 maps to -1 while
        // the I mapping puts label 0 at +1. The fast kernel reuses the
        // I-derived level table for both axes, so this must be
        // rejected at construction.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            // Label 00 (I-half=0, Q-half=0): I=+1, but Q = -1 instead of +1.
            SymbolPoint::new(1.0, -1.0),
            // Label 01 (I-half=0, Q-half=1): I=+1, Q=+1.
            SymbolPoint::new(1.0, 1.0),
            // Label 10 (I-half=1, Q-half=0): I=-1, Q=-1.
            SymbolPoint::new(-1.0, -1.0),
            // Label 11 (I-half=1, Q-half=1): I=-1, Q=+1.
            SymbolPoint::new(-1.0, 1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(2.0))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "expected canonical Gray-PAM level")]
    fn test_spoofed_qaxispam_metadata_with_non_preset_geometry_rejected() {
        // Symmetric of the I-axis case: valid IAxisPam/QAxisPam metadata
        // with I coordinates that pass factorisation but Q coordinates
        // that break it. Two symbols sharing Q-half-label 0b1 (raw
        // labels 0b01 and 0b11) must be rejected when they disagree on
        // their Q coordinate.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 1.0),   // label 00
            SymbolPoint::new(1.0, -1.0),  // label 01 (Q-half = 1, Q = -1)
            SymbolPoint::new(-1.0, 1.0),  // label 10
            SymbolPoint::new(-1.0, -2.0), // label 11 (Q-half = 1, Q = -2)  <- mismatch
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(1.875))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "expected canonical Gray-PAM level")]
    fn test_spoofed_iaxispam_metadata_with_non_preset_geometry_rejected() {
        // Build a custom 4-point spec with IAxisPam/QAxisPam metadata
        // (so validate_preset_layout accepts it) but asymmetric geometry
        // that breaks the I/Q factorisation the fast path relies on.
        use crate::modem::{BitChannelSemantics, ModemCapabilities};
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 1.0),
            SymbolPoint::new(1.0, -1.0),
            // Two symbols that share I-half-label 0b1 but have different
            // I coordinates break the factorisation and must be caught.
            SymbolPoint::new(-1.0, 1.0),
            SymbolPoint::new(-2.0, -1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::IAxisPam(0),
                BitChannelSemantics::QAxisPam(0),
            ])
            .capabilities(ModemCapabilities {
                supports_exact_log_map: true,
                supports_max_log: true,
                analysis: &[],
            })
            .normalization(Normalization::ExplicitEs(2.5))
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }

    #[test]
    #[should_panic(expected = "for Gray square-QAM preset")]
    fn test_non_preset_layout_panics() {
        // 4-point spec with m=2 but non-Gray-QAM bit-channel layout is
        // rejected. Use the Opaque semantics.
        use crate::modem::BitChannelSemantics;
        let points: Vec<SymbolPoint<f32>> = vec![
            SymbolPoint::new(1.0, 0.0),
            SymbolPoint::new(0.0, 1.0),
            SymbolPoint::new(-1.0, 0.0),
            SymbolPoint::new(0.0, -1.0),
        ];
        let labels: Vec<LabelWord> = (0u16..4).map(|b| LabelWord::new(b, 2)).collect();
        let spec = ModemSpecBuilder::<f32>::new()
            .bits_per_symbol(2)
            .points(points)
            .labels(labels)
            .bit_channels(vec![
                BitChannelSemantics::Opaque(0),
                BitChannelSemantics::Opaque(1),
            ])
            .normalization(Normalization::UnitAverageSymbolEnergy)
            .build();
        let _ = FastGrayQamDemapper::new(spec);
    }
}

#[cfg(test)]
mod property_tests {
    //! Property-based coverage matching the sibling
    //! [`super::super::ref_demapper`] proptest surface: random inputs
    //! must not produce NaN and the fast path must agree with the
    //! reference demapper within tight tolerance across all preset
    //! orders and both demap methods.
    use super::super::{
        BatchSoftDemapper, DemapInput, DemapMethod, ModemSpec, ReferenceSoftDemapper,
    };
    use super::FastGrayQamDemapper;
    use crate::llr::Llr;
    use proptest::prelude::*;

    const PRESET_ORDERS: &[usize] = &[2, 4, 16, 64, 256];

    fn spec_for_order(order: usize) -> ModemSpec<f64> {
        if order == 2 {
            ModemSpec::<f64>::bpsk_with_scalar()
        } else {
            ModemSpec::<f64>::gray_square_qam_with_scalar(order)
        }
    }

    proptest! {
        #[test]
        fn prop_fast_matches_reference_on_random_awgn(
            order_idx in 0usize..PRESET_ORDERS.len(),
            method_max_log in any::<bool>(),
            n_sym in 1usize..24usize,
            y_seed in any::<u64>(),
            nv_base in 0.05f64..2.0f64,
        ) {
            let order = PRESET_ORDERS[order_idx];
            let method = if method_max_log {
                DemapMethod::MaxLog
            } else {
                DemapMethod::ExactLogMap
            };
            let spec = spec_for_order(order);
            let m = spec.bits_per_symbol() as usize;
            let fast = FastGrayQamDemapper::new(spec.clone());
            let reference = ReferenceSoftDemapper::new(spec);

            // Deterministic pseudo-random samples routed through the
            // shared SSOT modem test LCG.
            let mut rng = super::super::test_oracle::Lcg::new(y_seed | 1);
            let rx_i: Vec<f64> = (0..n_sym).map(|_| rng.next_unit_f64() * 1.5).collect();
            let rx_q: Vec<f64> = (0..n_sym).map(|_| rng.next_unit_f64() * 1.5).collect();
            let nv: Vec<f64> = (0..n_sym).map(|i| nv_base + (i as f64) * 0.01).collect();

            let input = DemapInput::<f64> {
                rx_i: &rx_i,
                rx_q: &rx_q,
                gain_i: None,
                gain_q: None,
                noise_var: &nv,
                method,
            };
            let mut out_fast = vec![Llr::new(0.0); n_sym * m];
            let mut out_ref = vec![Llr::new(0.0); n_sym * m];
            fast.demap_llrs(input, &mut out_fast);
            reference.demap_llrs(input, &mut out_ref);
            for (f, r) in out_fast.iter().zip(out_ref.iter()) {
                prop_assert!(f.value().is_finite());
                prop_assert!((f.value() - r.value()).abs() < 1e-2);
            }
        }
    }
}
