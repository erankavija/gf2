//! Safe host wrapper for the device batch BCH syndrome evaluator
//! (`hip/bch_syndrome.hip`).
//!
//! [`GpuBchSyndrome`] evaluates `S_i = r(β_i)` over GF(2^m) at each of `2t`
//! evaluation points.

use std::ptr;

use crate::host::DeviceBuffer;
use crate::{check_hip, ffi, HipError};

/// The GF(2^m) `exp` / `log` tables a [`GpuBchSyndrome`] uploads to the device.
///
/// # Invariants
///
/// * `exp.len() == order` (`= 2^m - 1`), `log.len() == 1 << m`.
/// * `order == (1 << m) - 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BchFieldTables {
    m: usize,
    order: u32,
    exp: Vec<u16>,
    log: Vec<u16>,
}

impl BchFieldTables {
    /// Builds the field-table bundle for GF(2^m) from its antilog table `exp`
    /// (`α^i`) and discrete-log table `log`.
    ///
    /// # Panics
    ///
    /// Panics if `exp.len() != 2^m - 1` or `log.len() != 2^m`.
    #[must_use]
    pub fn new(m: usize, exp: Vec<u16>, log: Vec<u16>) -> Self {
        let order = (1usize << m) - 1;
        assert_eq!(
            exp.len(),
            order,
            "BchFieldTables::new: exp.len() {} != 2^m - 1 ({order})",
            exp.len()
        );
        assert_eq!(
            log.len(),
            1usize << m,
            "BchFieldTables::new: log.len() {} != 2^m ({})",
            log.len(),
            1usize << m
        );
        Self {
            m,
            order: order as u32,
            exp,
            log,
        }
    }

    /// The extension degree `m`.
    #[must_use]
    pub fn m(&self) -> usize {
        self.m
    }

    /// The multiplicative-group order `2^m - 1` (the GF-multiply modulus).
    #[must_use]
    pub fn order(&self) -> u32 {
        self.order
    }

    /// The antilog (`exp`) table.
    #[must_use]
    pub fn exp(&self) -> &[u16] {
        &self.exp
    }

    /// The discrete-log (`log`) table.
    #[must_use]
    pub fn log(&self) -> &[u16] {
        &self.log
    }
}

/// A reusable device-side batch BCH syndrome evaluator.
///
/// The field tables and the `2t` evaluation points are uploaded once; the
/// per-batch input and output buffers are sized for `max_batch` frames at
/// construction.
pub struct GpuBchSyndrome {
    d_log: DeviceBuffer<u16>,
    d_exp: DeviceBuffer<u16>,
    d_points: DeviceBuffer<u16>,
    d_coeffs: DeviceBuffer<u64>,
    d_syndromes: DeviceBuffer<u16>,
    n: usize,
    two_t: usize,
    order: u32,
    words_per_frame: usize,
    max_batch: usize,
    device_id: i32,
}

impl GpuBchSyndrome {
    /// Builds an evaluator on `device_id`, sized for up to `max_batch` frames
    /// per [`evaluate_batch`](Self::evaluate_batch).
    ///
    /// `eval_points` holds the `2t` syndrome evaluation points as u16 field
    /// values; `n` is the codeword length and `t` the error-correction
    /// capability.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] if any device allocation or upload fails (an OOM is
    /// [`HipError::OutOfMemory`]).
    ///
    /// # Panics
    ///
    /// Panics if `eval_points.len() != 2 * t`, `t == 0`, or `n == 0`.
    ///
    /// # Complexity
    ///
    /// O(`2^m + max_batch * (ceil(n/64) + 2t)`) device memory.
    pub fn new(
        tables: &BchFieldTables,
        eval_points: &[u16],
        n: usize,
        t: usize,
        max_batch: usize,
        device_id: i32,
    ) -> Result<Self, HipError> {
        assert!(n > 0, "GpuBchSyndrome::new: n must be > 0");
        assert!(t > 0, "GpuBchSyndrome::new: t must be > 0");
        let two_t = 2 * t;
        assert_eq!(
            eval_points.len(),
            two_t,
            "GpuBchSyndrome::new: eval_points.len() {} != 2t ({two_t})",
            eval_points.len()
        );

        let words_per_frame = n.div_ceil(64);

        let d_log = DeviceBuffer::<u16>::new(tables.log.len(), device_id)?;
        d_log.copy_from_host(&tables.log)?;
        let d_exp = DeviceBuffer::<u16>::new(tables.exp.len(), device_id)?;
        d_exp.copy_from_host(&tables.exp)?;
        let d_points = DeviceBuffer::<u16>::new(two_t, device_id)?;
        d_points.copy_from_host(eval_points)?;

        let d_coeffs = DeviceBuffer::<u64>::new(max_batch * words_per_frame, device_id)?;
        let d_syndromes = DeviceBuffer::<u16>::new(max_batch * two_t, device_id)?;

        Ok(Self {
            d_log,
            d_exp,
            d_points,
            d_coeffs,
            d_syndromes,
            n,
            two_t,
            order: tables.order,
            words_per_frame,
            max_batch,
            device_id,
        })
    }

    /// Codeword length `n` (coefficient count per frame).
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Number of syndromes per frame (`2t`).
    #[must_use]
    pub fn two_t(&self) -> usize {
        self.two_t
    }

    /// `ceil(n / 64)` — the u64 word count of each packed coefficient stream.
    #[must_use]
    pub fn words_per_frame(&self) -> usize {
        self.words_per_frame
    }

    /// Maximum frames per evaluate call.
    #[must_use]
    pub fn max_batch(&self) -> usize {
        self.max_batch
    }

    /// The device this evaluator's buffers are bound to.
    #[must_use]
    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    /// Evaluates the `2t` BCH syndromes for a batch of `batch` frames on the
    /// default stream.
    ///
    /// `coeff_streams` is `batch * words_per_frame` u64 words: frame `f`'s
    /// packed coefficient stream is `coeff_streams[f * wpf .. (f+1) * wpf]`,
    /// little-endian bit order with bit `i` the coefficient of `x^i`. Returns
    /// `batch * 2t` u16 syndromes, row-major per frame, one per evaluation
    /// point in upload order.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`] on device memcpy, kernel launch, or synchronization
    /// failure.
    ///
    /// # Panics
    ///
    /// Panics if `batch > max_batch` or
    /// `coeff_streams.len() != batch * words_per_frame`.
    ///
    /// # Complexity
    ///
    /// O(`batch * 2t * n`) device work.
    pub fn evaluate_batch(
        &mut self,
        coeff_streams: &[u64],
        batch: usize,
    ) -> Result<Vec<u16>, HipError> {
        assert!(
            batch <= self.max_batch,
            "evaluate_batch: batch {batch} > max_batch {}",
            self.max_batch
        );
        if batch == 0 {
            return Ok(Vec::new());
        }
        assert_eq!(
            coeff_streams.len(),
            batch * self.words_per_frame,
            "evaluate_batch: coeff_streams.len() {} != batch * words_per_frame ({})",
            coeff_streams.len(),
            batch * self.words_per_frame
        );

        self.d_coeffs.copy_from_host(coeff_streams)?;

        // SAFETY: all device pointers were allocated in `new` sized for
        // `max_batch` frames; `batch <= max_batch` and
        // `coeff_streams.len() == batch * words_per_frame` (both asserted). The
        // kernel reads the leading `batch * words_per_frame` coeff words, the
        // `2t` points, and the field tables, and writes the leading
        // `batch * 2t` syndrome elements. The null stream is the default stream.
        check_hip(
            unsafe {
                ffi::launch_bch_syndrome(
                    self.d_coeffs.as_ptr() as *const u64,
                    self.d_points.as_ptr() as *const u16,
                    self.d_log.as_ptr() as *const u16,
                    self.d_exp.as_ptr() as *const u16,
                    self.d_syndromes.as_mut_ptr() as *mut u16,
                    self.n as i32,
                    self.two_t as i32,
                    self.words_per_frame as i32,
                    self.order,
                    batch as i32,
                    ptr::null_mut(),
                )
            },
            "launch_bch_syndrome",
        )?;

        // SAFETY: hipDeviceSynchronize blocks until the default-stream launch
        // above completes; no preconditions.
        check_hip(
            unsafe { ffi::hip_device_synchronize() },
            "hipDeviceSynchronize",
        )?;

        let mut out = vec![0u16; batch * self.two_t];
        self.d_syndromes.copy_to_host(&mut out)?;
        Ok(out)
    }
}

/// Computes `out[j] = a[j] * b[j]` over GF(2^m) on the device with the
/// `gf_mul` of the syndrome kernel. The tables are uploaded on each call.
///
/// # Errors
///
/// Returns [`HipError`] on device allocation, memcpy, launch, or sync failure.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
pub fn gf_mul_device_batch(
    tables: &BchFieldTables,
    a: &[u16],
    b: &[u16],
    device_id: i32,
) -> Result<Vec<u16>, HipError> {
    assert_eq!(
        a.len(),
        b.len(),
        "gf_mul_device_batch: a.len() {} != b.len() {}",
        a.len(),
        b.len()
    );
    let count = a.len();
    if count == 0 {
        return Ok(Vec::new());
    }

    let d_log = DeviceBuffer::<u16>::new(tables.log.len(), device_id)?;
    d_log.copy_from_host(&tables.log)?;
    let d_exp = DeviceBuffer::<u16>::new(tables.exp.len(), device_id)?;
    d_exp.copy_from_host(&tables.exp)?;
    let d_a = DeviceBuffer::<u16>::new(count, device_id)?;
    d_a.copy_from_host(a)?;
    let d_b = DeviceBuffer::<u16>::new(count, device_id)?;
    d_b.copy_from_host(b)?;
    let d_out = DeviceBuffer::<u16>::new(count, device_id)?;

    // SAFETY: all five device buffers were just allocated (`d_a`/`d_b`/`d_out`
    // sized `count`, the tables sized to their slices). The kernel reads the
    // `count` operands and tables and writes `count` products. Null = default
    // stream.
    check_hip(
        unsafe {
            ffi::launch_gf_mul_test(
                d_a.as_ptr() as *const u16,
                d_b.as_ptr() as *const u16,
                d_log.as_ptr() as *const u16,
                d_exp.as_ptr() as *const u16,
                d_out.as_mut_ptr() as *mut u16,
                tables.order,
                count as i32,
                ptr::null_mut(),
            )
        },
        "launch_gf_mul_test",
    )?;
    // SAFETY: blocks until the default-stream launch completes; no preconditions.
    check_hip(
        unsafe { ffi::hip_device_synchronize() },
        "hipDeviceSynchronize",
    )?;

    let mut out = vec![0u16; count];
    d_out.copy_to_host(&mut out)?;
    Ok(out)
}

// `GpuBchSyndrome` is `Send` by auto-derive; this assertion keeps it so.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<GpuBchSyndrome>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_tables_accessors() {
        let exp: Vec<u16> = vec![1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9];
        let mut log = vec![0u16; 16];
        for (i, &e) in exp.iter().enumerate() {
            log[e as usize] = i as u16;
        }
        let tables = BchFieldTables::new(4, exp.clone(), log.clone());
        assert_eq!(tables.m(), 4);
        assert_eq!(tables.order(), 15);
        assert_eq!(tables.exp(), exp.as_slice());
        assert_eq!(tables.log(), log.as_slice());
    }

    #[test]
    #[should_panic(expected = "exp.len()")]
    fn test_field_tables_wrong_exp_len_panics() {
        let _ = BchFieldTables::new(4, vec![1u16; 14], vec![0u16; 16]);
    }
}
