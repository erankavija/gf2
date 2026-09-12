//! Shared helpers for the DVB-T2 external-baseline survey.
//!
//! One MODCOD name selects both sides of the comparison: gf2's
//! [`DvbT2BitInterleaver`] configuration and the matching xdsopl `PCTITL`
//! template instantiation in `../xdsopl-shim/xdsopl_shim.cpp`. Keeping the
//! mapping here means the validator and both timed arms agree by construction.
//! The cache-state policy and the timing loop live here for the same reason:
//! both timed arms apply one [`CachePolicy`] through [`timed_windows`].

use gf2_coding::ldpc::dvb_t2::bit_interleaver::{DvbT2Modcod, DvbT2Modulation};
use gf2_coding::ldpc::dvb_t2::FrameSize;
use gf2_coding::CodeRate;
use gf2_core::BitVec;
use std::io;
use std::time::Duration;
use tuning_campaign_support::timing::{
    execution_windows_configured, TimingProgress, TimingSample, FIXTURE_BANKS,
};

/// Cache-state policy a timed arm applies, decoded from its request.
///
/// The arms implement the protocol's `warm` and `streaming` states. A `cold`
/// cell needs a frozen fixed call count that the arms' requests do not carry,
/// so `cold` is refused with every other state instead of being reported as
/// applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePolicy {
    /// One fixture bank, passed over once, untimed, before calibration.
    Warm,
    /// The working set rotates through the canonical fixture banks.
    Streaming,
}

impl CachePolicy {
    /// Decodes a request's declared cache state.
    pub fn from_request(cache_state: &str) -> io::Result<Self> {
        match cache_state {
            "warm" => Ok(Self::Warm),
            "streaming" => Ok(Self::Streaming),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cache state {other:?} is not implemented; the arms apply warm and streaming"
                ),
            )),
        }
    }

    /// The protocol's name for this state, reported as `cache_state_applied`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Streaming => "streaming",
        }
    }

    /// Fixture banks in the working set.
    pub fn banks(self) -> usize {
        match self {
            Self::Warm => 1,
            Self::Streaming => FIXTURE_BANKS,
        }
    }
}

/// Runs the canonical calibrated timing loop under `policy`.
///
/// `body(bank)` is the timed operation on fixture bank `bank`, always below
/// `policy.banks()`. A warm policy first runs `body(0)` once, untimed, over
/// its one-bank working set: the protocol defines `warm` as one untimed pass
/// over the working set before calibration. The streaming policy runs nothing
/// before calibration. `progress` receives the timing loop's progress reports.
pub fn timed_windows(
    policy: CachePolicy,
    windows: u32,
    window_target_ms: u32,
    mut body: impl FnMut(usize),
    progress: impl FnMut(TimingProgress) -> io::Result<()>,
) -> io::Result<Vec<TimingSample>> {
    if policy == CachePolicy::Warm {
        body(0);
    }
    let banks = policy.banks();
    let mut rotated = |bank: usize| body(bank % banks);
    execution_windows_configured(
        0,
        u64::from(windows),
        Duration::from_millis(u64::from(window_target_ms)),
        &mut rotated,
        progress,
    )
}

/// MODCOD names this survey measures, in the order the addenda declare them.
pub const MODCODS: [&str; 4] = [
    "qam16-r12-normal",
    "qam64-r12-normal",
    "qam16-r12-short",
    "qam64-r12-short",
];

/// FECFRAME length in bits for one MODCOD name.
pub fn frame_bits(modcod: &str) -> usize {
    match modcod {
        "qam16-r12-normal" | "qam64-r12-normal" => 64_800,
        "qam16-r12-short" | "qam64-r12-short" => 16_200,
        other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
    }
}

/// gf2-side MODCOD descriptor for one MODCOD name.
pub fn modcod_for_name(name: &str) -> DvbT2Modcod {
    let (frame_size, modulation) = match name {
        "qam16-r12-normal" => (FrameSize::Normal, DvbT2Modulation::Qam16),
        "qam64-r12-normal" => (FrameSize::Normal, DvbT2Modulation::Qam64),
        "qam16-r12-short" => (FrameSize::Short, DvbT2Modulation::Qam16),
        "qam64-r12-short" => (FrameSize::Short, DvbT2Modulation::Qam64),
        other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
    };
    DvbT2Modcod::new(frame_size, CodeRate::Rate1_2, modulation)
}

/// Expands one seed into canonical little-endian packed fixture banks.
///
/// The tail bits above `bits` are zeroed so every bank satisfies the canonical
/// zero-tail-padding invariant of a `BitVec` of that length.
pub fn seeded_word_banks(seed: u64, banks: usize, bits: usize) -> Vec<Vec<u64>> {
    let words = bits.div_ceil(64);
    let mut mixer = tuning_campaign_support::abtest::SplitMix64::new(seed);
    (0..banks)
        .map(|_| {
            let mut bank: Vec<u64> = (0..words).map(|_| mixer.next_u64()).collect();
            let tail_bits = bits % 64;
            if tail_bits != 0 {
                bank[words - 1] &= (1_u64 << tail_bits) - 1;
            }
            bank
        })
        .collect()
}

/// Unpacks canonical little-endian words into one 0/1 `i32` per bit.
pub fn unpack_words(words: &[u64], bits: usize) -> Vec<i32> {
    assert!(words.len() >= bits.div_ceil(64));
    (0..bits)
        .map(|index| ((words[index >> 6] >> (index & 63)) & 1) as i32)
        .collect()
}

/// Packs 0/1 values using the canonical little-endian bit numbering.
pub fn pack_bits(bits: &[i32]) -> Vec<u64> {
    let mut words = vec![0_u64; bits.len().div_ceil(64)];
    for (index, &bit) in bits.iter().enumerate() {
        assert!(bit == 0 || bit == 1, "non-bit value {bit} at index {index}");
        if bit == 1 {
            words[index >> 6] |= 1_u64 << (index & 63);
        }
    }
    words
}

/// Builds a `BitVec` from canonical packed words without changing bit order.
pub fn bitvec_from_words(words: &[u64], bits: usize) -> BitVec {
    BitVec::from_words(words.to_vec(), bits)
}

/// Calls the selected xdsopl `PCTITL` forward operation.
pub fn xdsopl_forward(modcod: &str, input: &[i32], output: &mut [i32]) {
    let bits = frame_bits(modcod);
    assert_eq!(input.len(), bits);
    assert_eq!(output.len(), bits);
    unsafe {
        match modcod {
            "qam16-r12-normal" => {
                xdsopl_pctitl_qam16_r12_normal_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam64-r12-normal" => {
                xdsopl_pctitl_qam64_r12_normal_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam16-r12-short" => {
                xdsopl_pctitl_qam16_r12_short_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            "qam64-r12-short" => {
                xdsopl_pctitl_qam64_r12_short_fwd(input.as_ptr(), output.as_mut_ptr())
            }
            other => panic!("unrecognized DVB-T2 MODCOD: {other}"),
        }
    }
}

unsafe extern "C" {
    fn xdsopl_pctitl_qam16_r12_normal_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam64_r12_normal_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam16_r12_short_fwd(input: *const i32, output: *mut i32);
    fn xdsopl_pctitl_qam64_r12_short_fwd(input: *const i32, output: *mut i32);
}

#[cfg(test)]
mod tests {
    use super::{timed_windows, CachePolicy};
    use tuning_campaign_support::timing::FIXTURE_BANKS;

    #[test]
    fn warm_request_runs_one_untimed_pass_before_calibration() {
        // Zero windows makes the timing loop reject the protocol before its
        // first call, so every body call observed here precedes calibration.
        let policy = CachePolicy::from_request("warm").expect("warm is implemented");
        let mut calls = Vec::new();
        assert!(timed_windows(policy, 0, 100, |bank| calls.push(bank), |_| Ok(())).is_err());
        assert_eq!(calls, vec![0]);
    }

    #[test]
    fn streaming_request_rotates_banks_without_a_pass() {
        let policy = CachePolicy::from_request("streaming").expect("streaming is implemented");
        assert_eq!(policy.banks(), FIXTURE_BANKS);
        let mut calls = 0;
        assert!(timed_windows(policy, 0, 100, |_| calls += 1, |_| Ok(())).is_err());
        assert_eq!(calls, 0);
    }

    #[test]
    fn unimplemented_cache_states_are_refused() {
        for state in ["cold", "Warm", ""] {
            assert!(CachePolicy::from_request(state).is_err(), "{state:?}");
        }
    }

    #[test]
    fn applied_names_round_trip() {
        for policy in [CachePolicy::Warm, CachePolicy::Streaming] {
            assert_eq!(CachePolicy::from_request(policy.name()).unwrap(), policy);
        }
        assert_eq!(CachePolicy::Warm.banks(), 1);
    }
}
