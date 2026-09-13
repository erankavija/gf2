//! Safe wrapper over the C-ABI shim in `cpp/srsran_encode_shim.cpp`.
//!
//! srsRAN takes the base graph and the lifting size from its caller, as gf2
//! does, so the adapter drives it with gf2's own parameters instead of a
//! derivation of its own.

use crate::{from_msb_first_bytes, to_msb_first_bytes};
use gf2_coding::ldpc::nr_5g::NrRateMatchParams;
use gf2_core::BitVec;
use std::ffi::c_void;

/// The dimensions srsRAN derives from a base graph and a lifting size.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dimensions {
    /// Systematic bits the encoder reads, including fillers.
    pub k_ldpc: i32,
    /// Codeblock bits the encoder writes, excluding the `2 * Z` prefix.
    pub n_short: i32,
    /// Mother-codeword bits including the `2 * Z` prefix.
    pub n_full: i32,
}

unsafe extern "C" {
    fn gf2_srsran_nr_supports_avx2() -> i32;
    fn gf2_srsran_nr_dims_for(bg: i32, z: i32, out: *mut Dimensions) -> i32;
    fn gf2_srsran_nr_comparator_new(backend: i32) -> *mut c_void;
    fn gf2_srsran_nr_comparator_free(handle: *mut c_void);
    fn gf2_srsran_nr_backend(handle: *mut c_void) -> i32;
    fn gf2_srsran_nr_encode_rate_match(
        handle: *mut c_void,
        bg: i32,
        z: i32,
        k: i32,
        n: i32,
        rv: i32,
        qm: i32,
        in_bytes: *const u8,
        out_bytes: *mut u8,
    ) -> i32;
    fn gf2_srsran_nr_encode_only(
        handle: *mut c_void,
        bg: i32,
        z: i32,
        k: i32,
        in_bytes: *const u8,
        out_bits: *mut u8,
    ) -> i32;
}

fn dimension(value: usize) -> i32 {
    i32::try_from(value).expect("dimension fits in i32")
}

/// Which software encoder backend srsRAN selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Generic,
    Avx2,
}

impl Backend {
    /// The name srsRAN's `ldpc_encoder_factory_sw` uses for this backend.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Generic => "generic",
            Backend::Avx2 => "avx2",
        }
    }

    fn code(self) -> i32 {
        match self {
            Backend::Generic => 0,
            Backend::Avx2 => 1,
        }
    }
}

/// True when this host's CPU reports AVX2, which is the backend srsRAN's own
/// `ldpc_encoder_factory_sw("auto")` then selects.
pub fn supports_avx2() -> bool {
    // SAFETY: the shim reads a CPU feature bit and takes no arguments.
    unsafe { gf2_srsran_nr_supports_avx2() == 1 }
}

/// srsRAN's dimensions for `(base_graph, z)`, or `None` when srsRAN carries
/// no graph for the pair.
pub fn dimensions(base_graph: u8, z: usize) -> Option<Dimensions> {
    let mut out = Dimensions::default();
    // SAFETY: `out` is a valid, exclusively borrowed `Dimensions` whose
    // layout matches the shim's `gf2_srsran_nr_dims`.
    let status =
        unsafe { gf2_srsran_nr_dims_for(i32::from(base_graph), dimension(z), &mut out) };
    (status == 0).then_some(out)
}

/// One srsRAN encoder and rate matcher pair behind the shim.
pub struct Comparator {
    handle: *mut c_void,
    backend: Backend,
}

impl Comparator {
    /// Constructs the modules srsRAN's factories build. `backend` of `None`
    /// reproduces the `"auto"` selection.
    pub fn new(backend: Option<Backend>) -> Result<Self, String> {
        let requested = backend.map_or(-1, Backend::code);
        // SAFETY: the shim returns either null or an owned handle that `Drop`
        // releases exactly once.
        let handle = unsafe { gf2_srsran_nr_comparator_new(requested) };
        if handle.is_null() {
            return Err(format!("srsRAN cannot serve backend {backend:?}"));
        }
        // SAFETY: `handle` is a live handle from the call above.
        let selected = unsafe { gf2_srsran_nr_backend(handle) };
        Ok(Self {
            handle,
            backend: if selected == 1 {
                Backend::Avx2
            } else {
                Backend::Generic
            },
        })
    }

    /// The backend this handle selected.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Runs srsRAN's encoder and rate matcher on packed buffers.
    ///
    /// `input` holds `k_ldpc` bits and `output` receives `n` bits, both
    /// packed most-significant-bit first.
    ///
    /// # Errors
    ///
    /// Returns the shim's status code when srsRAN's assertions reject the
    /// request: 1 an unknown graph, 2 an out-of-range redundancy version or
    /// modulation order, 3 an unserviceable length.
    ///
    /// # Panics
    ///
    /// Panics when a buffer is too short for the declared lengths.
    pub fn encode_rate_match(
        &self,
        base_graph: u8,
        z: usize,
        k: usize,
        n: usize,
        redundancy_version: u8,
        modulation_order: u8,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), i32> {
        let dims = dimensions(base_graph, z).ok_or(1)?;
        let k_ldpc = usize::try_from(dims.k_ldpc).expect("positive K_LDPC");
        assert!(
            input.len() >= k_ldpc.div_ceil(8),
            "input holds {} bytes, K_LDPC needs {}",
            input.len(),
            k_ldpc.div_ceil(8)
        );
        assert!(
            output.len() >= n.div_ceil(8),
            "output holds {} bytes, N needs {}",
            output.len(),
            n.div_ceil(8)
        );
        // SAFETY: `handle` is live; `input` holds at least the K_LDPC bits
        // and `output` at least the N bits the shim reads and writes.
        let status = unsafe {
            gf2_srsran_nr_encode_rate_match(
                self.handle,
                i32::from(base_graph),
                dimension(z),
                dimension(k),
                dimension(n),
                i32::from(redundancy_version),
                i32::from(modulation_order),
                input.as_ptr(),
                output.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }

    /// Runs srsRAN's encoder alone and returns the `n_short` codeblock bits
    /// one per byte, which is how its rate matcher reads them.
    ///
    /// # Errors
    ///
    /// Returns the shim's status code.
    ///
    /// # Panics
    ///
    /// Panics when srsRAN carries no graph for `(base_graph, z)`.
    pub fn encode_only(
        &self,
        base_graph: u8,
        z: usize,
        k: usize,
        input: &[u8],
    ) -> Result<Vec<u8>, i32> {
        let dims = dimensions(base_graph, z).ok_or(1)?;
        let n_short = usize::try_from(dims.n_short).expect("positive codeblock length");
        let mut out = vec![0_u8; n_short];
        // SAFETY: `handle` is live; `input` holds the K_LDPC bits the shim
        // reads and `out` the `n_short` bytes it writes.
        let status = unsafe {
            gf2_srsran_nr_encode_only(
                self.handle,
                i32::from(base_graph),
                dimension(z),
                dimension(k),
                input.as_ptr(),
                out.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(out)
        } else {
            Err(status)
        }
    }
}

impl Drop for Comparator {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `gf2_srsran_nr_comparator_new` and is
        // released only here.
        unsafe { gf2_srsran_nr_comparator_free(self.handle) };
    }
}

/// The whole-consumer srsRAN arm: gf2's representation in and out.
pub struct Adapter {
    comparator: Comparator,
    base_graph: u8,
    z: usize,
    target_k: usize,
    target_n: usize,
    k_ldpc: usize,
    redundancy_version: u8,
    modulation_order: u8,
}

impl Adapter {
    /// Wires srsRAN to gf2's own code parameters.
    ///
    /// # Panics
    ///
    /// Panics when srsRAN carries no graph for gf2's `(base graph, Z)`.
    pub fn new(
        comparator: Comparator,
        params: &NrRateMatchParams,
        redundancy_version: u8,
        modulation_order: u8,
    ) -> Self {
        let dims = dimensions(params.base_graph, params.lifting_factor)
            .expect("srsRAN carries a graph for gf2's base graph and lifting size");
        Self {
            comparator,
            base_graph: params.base_graph,
            z: params.lifting_factor,
            target_k: params.target_k,
            target_n: params.target_n,
            k_ldpc: usize::try_from(dims.k_ldpc).expect("positive K_LDPC"),
            redundancy_version,
            modulation_order,
        }
    }

    pub fn comparator(&self) -> &Comparator {
        &self.comparator
    }

    /// srsRAN's systematic length, which equals gf2's `full_k` when the two
    /// projects agree on the mother code.
    pub fn k_ldpc(&self) -> usize {
        self.k_ldpc
    }

    /// Stage 1: converts gf2's message into srsRAN's packed systematic
    /// buffer, zero filling the filler positions, and allocates the packed
    /// output.
    pub fn unpack(&self, message: &BitVec) -> (Vec<u8>, Vec<u8>) {
        (
            to_msb_first_bytes(message, self.k_ldpc),
            vec![0_u8; self.target_n.div_ceil(8)],
        )
    }

    /// Stage 3: converts srsRAN's packed output into gf2's representation.
    pub fn pack(&self, output: &[u8]) -> BitVec {
        from_msb_first_bytes(output, self.target_n)
    }

    /// The timed call: unpack, srsRAN encode and rate match, pack.
    ///
    /// # Panics
    ///
    /// Panics when srsRAN rejects the request, which the untimed validation
    /// gate rules out before any measurement.
    pub fn encode(&self, message: &BitVec) -> BitVec {
        let (input, mut output) = self.unpack(message);
        self.comparator
            .encode_rate_match(
                self.base_graph,
                self.z,
                self.target_k,
                self.target_n,
                self.redundancy_version,
                self.modulation_order,
                &input,
                &mut output,
            )
            .expect("srsRAN accepts the validated request");
        self.pack(&output)
    }
}
