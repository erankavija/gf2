//! Safe wrapper over the C-ABI shim in `cpp/aff3ct_encode_shim.cpp`.
//!
//! AFF3CT derives the base graph, the lifting size and the mother dimensions
//! from `(K, N)` itself, so the adapter reports that derivation rather than
//! imposing gf2's. The validation record names the pairs where the two
//! derivations differ; those configurations are not timed as matched arms.

use gf2_core::BitVec;
use std::ffi::{c_void, CString};

/// The parameters AFF3CT's `tools::build_5G_base_graph(K, N)` derives.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BaseGraph {
    pub base_graph: i32,
    pub lifting: i32,
    pub index_list: i32,
    pub k_ldpc: i32,
    pub n_ldpc: i32,
}

unsafe extern "C" {
    fn gf2_aff3ct_nr_base_graph_for(k: i32, n: i32, out: *mut BaseGraph) -> i32;
    fn gf2_aff3ct_nr_comparator_new(k: i32, n: i32, conf_root: *const i8) -> *mut c_void;
    fn gf2_aff3ct_nr_comparator_free(handle: *mut c_void);
    fn gf2_aff3ct_nr_encode_rate_match(
        handle: *mut c_void,
        message: *const i32,
        mother: *mut i32,
        transmitted: *mut i32,
    ) -> i32;
}

fn dimension(value: usize) -> i32 {
    i32::try_from(value).expect("dimension fits in i32")
}

/// AFF3CT's derivation for `(k, n)`, or `None` when AFF3CT rejects it.
pub fn base_graph(k: usize, n: usize) -> Option<BaseGraph> {
    let mut out = BaseGraph::default();
    // SAFETY: `out` is a valid, exclusively borrowed `BaseGraph` whose layout
    // matches the shim's `gf2_aff3ct_nr_base_graph`.
    let status = unsafe { gf2_aff3ct_nr_base_graph_for(dimension(k), dimension(n), &mut out) };
    (status == 0).then_some(out)
}

/// One AFF3CT encoder and puncturer pair behind the shim.
pub struct Comparator {
    handle: *mut c_void,
    k: usize,
    n: usize,
    derived: BaseGraph,
}

impl Comparator {
    /// Constructs the modules AFF3CT's factories build for `(k, n)`.
    /// `conf_root` names the directory holding AFF3CT's `conf` tree.
    ///
    /// # Errors
    ///
    /// Returns a message when AFF3CT rejects the pair or cannot read its
    /// generator-matrix file.
    pub fn new(k: usize, n: usize, conf_root: &str) -> Result<Self, String> {
        let derived = base_graph(k, n).ok_or_else(|| format!("AFF3CT rejects K={k}, N={n}"))?;
        let root = CString::new(conf_root).map_err(|error| error.to_string())?;
        // SAFETY: the shim returns either null or an owned handle that `Drop`
        // releases exactly once; `root` outlives the call.
        let handle =
            unsafe { gf2_aff3ct_nr_comparator_new(dimension(k), dimension(n), root.as_ptr()) };
        if handle.is_null() {
            return Err(format!(
                "AFF3CT cannot construct its 5G encoder and puncturer for K={k}, N={n} \
                 with conf root {conf_root}"
            ));
        }
        Ok(Self {
            handle,
            k,
            n,
            derived,
        })
    }

    /// The parameters AFF3CT derived for this pair.
    pub fn derived(&self) -> BaseGraph {
        self.derived
    }

    /// Mother-codeword length AFF3CT's encoder writes.
    pub fn mother_len(&self) -> usize {
        usize::try_from(self.derived.n_ldpc).expect("positive N_LDPC")
    }

    /// Runs AFF3CT's encoder and puncturer on raw buffers, one value per bit.
    ///
    /// # Panics
    ///
    /// Panics on a length mismatch or when AFF3CT reports a failure.
    pub fn encode_rate_match(
        &self,
        message: &[i32],
        mother: &mut [i32],
        transmitted: &mut [i32],
    ) {
        assert_eq!(message.len(), self.k, "message length must equal K");
        assert_eq!(mother.len(), self.mother_len(), "mother length must equal N_LDPC");
        assert_eq!(transmitted.len(), self.n, "output length must equal N");
        // SAFETY: `handle` is live; the three buffers hold exactly the K,
        // N_LDPC and N values the shim's modules were constructed with.
        let status = unsafe {
            gf2_aff3ct_nr_encode_rate_match(
                self.handle,
                message.as_ptr(),
                mother.as_mut_ptr(),
                transmitted.as_mut_ptr(),
            )
        };
        assert_eq!(status, 0, "AFF3CT encode and puncture failed");
    }
}

impl Drop for Comparator {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `gf2_aff3ct_nr_comparator_new` and is
        // released only here.
        unsafe { gf2_aff3ct_nr_comparator_free(self.handle) };
    }
}

/// The whole-consumer AFF3CT arm: gf2's representation in and out.
pub struct Adapter {
    comparator: Comparator,
}

impl Adapter {
    pub fn new(comparator: Comparator) -> Self {
        Self { comparator }
    }

    pub fn comparator(&self) -> &Comparator {
        &self.comparator
    }

    /// Stage 1: converts gf2's message into AFF3CT's one-value-per-bit input
    /// and allocates its mother and transmitted buffers.
    pub fn unpack(&self, message: &BitVec) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        let input: Vec<i32> = (0..message.len())
            .map(|index| i32::from(message.get(index)))
            .collect();
        (
            input,
            vec![0_i32; self.comparator.mother_len()],
            vec![0_i32; self.comparator.n],
        )
    }

    /// Stage 3: converts AFF3CT's transmitted values into gf2's
    /// representation.
    pub fn pack(&self, transmitted: &[i32]) -> BitVec {
        let mut bits = BitVec::with_capacity(transmitted.len());
        for &value in transmitted {
            bits.push_bit(value != 0);
        }
        bits
    }

    /// The timed call: unpack, AFF3CT encode and puncture, pack.
    pub fn encode(&self, message: &BitVec) -> BitVec {
        let (input, mut mother, mut transmitted) = self.unpack(message);
        self.comparator
            .encode_rate_match(&input, &mut mother, &mut transmitted);
        self.pack(&transmitted)
    }
}
