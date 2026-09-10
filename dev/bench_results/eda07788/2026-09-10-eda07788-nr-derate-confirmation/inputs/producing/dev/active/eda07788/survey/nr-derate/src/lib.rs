//! Shared pieces of the 5G NR LLR de-rate-matching comparison.
//!
//! The compared operation maps `target_n` channel LLRs onto the `full_n`
//! mother-code positions of a rate-matched 5G NR LDPC code: transmitted
//! positions receive their channel LLR, the `2*Z` punctured systematic
//! positions and untransmitted parity receive zero, and filler positions
//! receive a known-zero LLR. gf2 exposes it as
//! `Nr5gRateMatchedCode::prepare_llrs`; AFF3CT exposes it as the public
//! `Puncturer::depuncture` of `Puncturer_5G`.
//!
//! The consumer representation is gf2's: `&[Llr]` in, `Vec<Llr>` out. The
//! AFF3CT adapter converts into AFF3CT's `float` buffers and back. AFF3CT
//! writes only the selected positions, the punctured prefix and the fillers,
//! so the adapter hands it a zeroed output buffer; it writes +infinity into
//! fillers, so the adapter overwrites them with the filler value gf2 writes.
//! Both steps run inside the adapter's call and are charged to that arm.

use gf2_coding::ldpc::nr_5g::Nr5gRateMatchedCode;
use gf2_coding::ldpc::QuasiCyclicLdpc;
use gf2_coding::Llr;
use tuning_campaign_support::abtest::SplitMix64;

/// One de-rate-matching configuration: gf2's base graph and target lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub name: &'static str,
    pub base_graph: u8,
    pub target_n: usize,
    pub target_k: usize,
}

const fn configuration(
    name: &'static str,
    base_graph: u8,
    target_n: usize,
    target_k: usize,
) -> Configuration {
    Configuration {
        name,
        base_graph,
        target_n,
        target_k,
    }
}

/// Every configuration the equivalence gate checks, in report order.
///
/// The first six are the rate-matching survey's configurations. The rest add
/// a rate-3/4 BG1 code, a BG2 message length inside `560 < K <= 640`, and
/// filler-free codes (`K` equal to the base-graph systematic width times `Z`)
/// for both base graphs, which exist only where the lifting `Kb` equals that
/// width.
pub const CONFIGURATIONS: [Configuration; 12] = [
    configuration("bg2-n256-k121", 2, 256, 121),
    configuration("bg2-n512-k200", 2, 512, 200),
    configuration("bg2-n1024-k400", 2, 1024, 400),
    configuration("bg1-n1024-k512", 1, 1024, 512),
    configuration("bg1-n2048-k1024", 1, 2048, 1024),
    configuration("bg1-n4096-k2048", 1, 4096, 2048),
    configuration("bg1-n1200-k900", 1, 1200, 900),
    configuration("bg2-n1200-k600", 2, 1200, 600),
    configuration("bg2-n1440-k720", 2, 1440, 720),
    configuration("bg1-n400-k308", 1, 400, 308),
    configuration("bg1-n1320-k1056", 1, 1320, 1056),
    configuration("bg1-n2560-k2048", 1, 2560, 2048),
];

/// Looks up a configuration by name.
///
/// # Panics
///
/// Panics when `name` is not in [`CONFIGURATIONS`].
pub fn configuration_named(name: &str) -> Configuration {
    CONFIGURATIONS
        .iter()
        .copied()
        .find(|candidate| candidate.name == name)
        .unwrap_or_else(|| panic!("unknown NR de-rate-matching configuration {name:?}"))
}

/// gf2's rate-matched code for one configuration.
pub fn gf2_code(configuration: Configuration) -> Nr5gRateMatchedCode {
    QuasiCyclicLdpc::nr_5g_rate_matched(
        configuration.base_graph,
        configuration.target_n,
        configuration.target_k,
    )
}

/// Expands one seed into `banks` channel-LLR vectors of `len` finite values
/// uniformly spread over `[-16, 16)`.
pub fn seeded_llr_banks(seed: u64, banks: usize, len: usize) -> Vec<Vec<Llr>> {
    let mut mixer = SplitMix64::new(seed);
    (0..banks)
        .map(|_| {
            (0..len)
                .map(|_| {
                    let unit = (mixer.next_u64() >> 40) as f32 / (1_u64 << 24) as f32;
                    Llr::new(unit * 32.0 - 16.0)
                })
                .collect()
        })
        .collect()
}

/// The positions and the single value gf2's `prepare_llrs` writes into filler
/// positions, observed from an all-zero channel: zero channel LLRs, the
/// punctured prefix and untransmitted parity are all zero, so every nonzero
/// output position is a filler.
///
/// # Panics
///
/// Panics when the nonzero outputs do not share one value.
pub fn observed_fillers(code: &Nr5gRateMatchedCode) -> (Vec<usize>, Option<Llr>) {
    let zeros = vec![Llr::zero(); code.params().target_n];
    let full = code.prepare_llrs(&zeros);
    let positions: Vec<usize> = full
        .iter()
        .enumerate()
        .filter(|(_, llr)| llr.value() != 0.0)
        .map(|(position, _)| position)
        .collect();
    let value = positions.first().map(|&position| full[position]);
    assert!(
        positions
            .iter()
            .all(|&position| Some(full[position]) == value),
        "gf2 writes more than one filler value"
    );
    (positions, value)
}

/// Bitwise equality of two LLR vectors, counting equal positions.
pub fn identical_positions(left: &[Llr], right: &[Llr]) -> usize {
    left.iter()
        .zip(right)
        .filter(|(a, b)| a.value().to_bits() == b.value().to_bits())
        .count()
}

#[cfg(feature = "aff3ct")]
pub mod aff3ct {
    //! Safe wrapper over the C-ABI shim in `cpp/aff3ct_depuncture_shim.cpp`.

    use gf2_coding::Llr;
    use std::ffi::c_void;

    /// Parameters AFF3CT's `tools::build_5G_base_graph(K, N)` derives.
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
        fn gf2_aff3ct_nr_depuncturer_new(k: i32, n: i32) -> *mut c_void;
        fn gf2_aff3ct_nr_depuncturer_free(handle: *mut c_void);
        fn gf2_aff3ct_nr_depuncture(
            handle: *mut c_void,
            channel: *const f32,
            full: *mut f32,
        ) -> i32;
    }

    fn dimension(value: usize) -> i32 {
        i32::try_from(value).expect("dimension fits in i32")
    }

    /// AFF3CT's derivation for `(k, n)`, or `None` when AFF3CT rejects it.
    pub fn base_graph(k: usize, n: usize) -> Option<BaseGraph> {
        let mut out = BaseGraph::default();
        // SAFETY: `out` is a valid, exclusively borrowed `BaseGraph` whose
        // layout matches the shim's `gf2_aff3ct_nr_base_graph`.
        let status = unsafe { gf2_aff3ct_nr_base_graph_for(dimension(k), dimension(n), &mut out) };
        (status == 0).then_some(out)
    }

    /// One AFF3CT `Puncturer_5G<int32_t, float>` behind the shim.
    pub struct Depuncturer {
        handle: *mut c_void,
        k: usize,
        n: usize,
        derived: BaseGraph,
    }

    impl Depuncturer {
        /// Constructs the depuncturer AFF3CT's codec would build for `(k, n)`.
        pub fn new(k: usize, n: usize) -> Result<Self, String> {
            let derived = base_graph(k, n).ok_or_else(|| format!("AFF3CT rejects K={k}, N={n}"))?;
            // SAFETY: the shim returns either null or an owned handle that
            // `Drop` releases exactly once.
            let handle = unsafe { gf2_aff3ct_nr_depuncturer_new(dimension(k), dimension(n)) };
            if handle.is_null() {
                return Err(format!(
                    "AFF3CT cannot construct Puncturer_5G for K={k}, N={n}"
                ));
            }
            Ok(Self {
                handle,
                k,
                n,
                derived,
            })
        }

        /// The parameters AFF3CT derived for this depuncturer.
        pub fn derived(&self) -> BaseGraph {
            self.derived
        }

        /// Mother-codeword length of the output buffer.
        pub fn full_len(&self) -> usize {
            usize::try_from(self.derived.n_ldpc).expect("positive N_LDPC")
        }

        /// Runs AFF3CT's public depuncture on raw buffers. Positions AFF3CT
        /// does not write keep their previous contents.
        ///
        /// # Panics
        ///
        /// Panics on a length mismatch or when AFF3CT reports a failure.
        pub fn depuncture(&self, channel: &[f32], full: &mut [f32]) {
            assert_eq!(channel.len(), self.n, "channel length must equal N");
            assert_eq!(
                full.len(),
                self.full_len(),
                "output length must equal N_LDPC"
            );
            // SAFETY: `handle` is live; `channel` holds N readable floats and
            // `full` holds N_LDPC writable floats, the socket sizes the shim's
            // Puncturer_5G was constructed with.
            let status = unsafe {
                gf2_aff3ct_nr_depuncture(self.handle, channel.as_ptr(), full.as_mut_ptr())
            };
            assert_eq!(status, 0, "AFF3CT depuncture failed");
        }

        /// The filler range `[K, K_LDPC)` AFF3CT writes +infinity into.
        pub fn filler_range(&self) -> std::ops::Range<usize> {
            self.k..usize::try_from(self.derived.k_ldpc).expect("positive K_LDPC")
        }
    }

    impl Drop for Depuncturer {
        fn drop(&mut self) {
            // SAFETY: `handle` came from `gf2_aff3ct_nr_depuncturer_new` and
            // is released only here.
            unsafe { gf2_aff3ct_nr_depuncturer_free(self.handle) };
        }
    }

    /// The whole-consumer AFF3CT arm: gf2's representation in and out.
    pub struct Adapter {
        depuncturer: Depuncturer,
        filler: Llr,
    }

    impl Adapter {
        /// Wraps a depuncturer; `filler` is the value gf2 writes into fillers.
        pub fn new(depuncturer: Depuncturer, filler: Llr) -> Self {
            Self {
                depuncturer,
                filler,
            }
        }

        pub fn depuncturer(&self) -> &Depuncturer {
            &self.depuncturer
        }

        /// Stage 1: converts the channel LLRs to AFF3CT's `float` input and
        /// allocates the zeroed output buffer AFF3CT's contract needs.
        pub fn unpack(&self, channel: &[Llr]) -> (Vec<f32>, Vec<f32>) {
            let input = channel.iter().map(|llr| llr.value()).collect();
            (input, vec![0.0_f32; self.depuncturer.full_len()])
        }

        /// Stage 3: converts AFF3CT's output to gf2's representation and
        /// replaces AFF3CT's +infinity fillers with gf2's filler value.
        pub fn pack(&self, full: &[f32]) -> Vec<Llr> {
            let mut out: Vec<Llr> = full.iter().map(|&value| Llr::new(value)).collect();
            for position in self.depuncturer.filler_range() {
                out[position] = self.filler;
            }
            out
        }

        /// The timed call: unpack, AFF3CT depuncture, pack.
        pub fn prepare_llrs(&self, channel: &[Llr]) -> Vec<Llr> {
            let (input, mut full) = self.unpack(channel);
            self.depuncturer.depuncture(&input, &mut full);
            self.pack(&full)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_banks_are_deterministic_finite_and_bounded() {
        let first = seeded_llr_banks(7, 2, 65);
        assert_eq!(first, seeded_llr_banks(7, 2, 65));
        assert_ne!(first[0], first[1], "banks draw distinct values");
        assert!(first.iter().flatten().all(|llr| {
            let value = llr.value();
            value.is_finite() && (-16.0..16.0).contains(&value)
        }));
        assert_ne!(
            first,
            seeded_llr_banks(8, 2, 65),
            "the seed selects the stream"
        );
    }

    #[test]
    fn fillers_are_the_shortened_systematic_positions() {
        let code = gf2_code(configuration_named("bg2-n256-k121"));
        let params = code.params().clone();
        let (positions, value) = observed_fillers(&code);
        assert_eq!(positions.len(), params.full_k - params.target_k);
        assert!(
            value.is_some_and(|llr| llr.value() > 0.0),
            "fillers are known zeros"
        );
    }

    #[test]
    fn filler_free_codes_write_no_filler() {
        let code = gf2_code(configuration_named("bg1-n400-k308"));
        assert_eq!(code.params().full_k, code.params().target_k);
        assert_eq!(observed_fillers(&code), (Vec::new(), None));
    }

    #[test]
    fn configuration_names_are_unique() {
        for (index, configuration) in CONFIGURATIONS.iter().enumerate() {
            assert_eq!(configuration_named(configuration.name), *configuration);
            assert!(CONFIGURATIONS[..index]
                .iter()
                .all(|earlier| earlier.name != configuration.name));
        }
    }
}
