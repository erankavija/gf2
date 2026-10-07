//! Arm executable for the transpose-lane receipts (jit:1d4fd63d).
//!
//! One executable serves every arm of every cell. `GF2_TRANSPOSE_LANE` selects
//! which 64×64 block kernel the arm runs, so the arms share one build, one
//! fixture generator, one warm pass and one timing loop and differ only in the
//! lane under measurement.
//!
//! # Arms
//!
//! * `production` — the kernel `gf2_kernels_simd::transpose::detect` publishes
//!   for this host, which is the block primitive
//!   [`gf2_core::matrix::BitMatrix::transpose`] and `gf2-coding`'s bit-sliced
//!   BCH encoding reach. This is the pinned pre-change implementation.
//! * a lane tag of `gf2_kernels_simd::transpose::TransposeLane` — that lane,
//!   named through the public `transpose::lane`. Every candidate reaches the
//!   consumers through the same abstraction the production kernel does:
//!   `BitMatrix::transpose_with_block_kernel` for the whole matrix and the
//!   `transpose_lane_block` field of `BchEncodeFns` for the bit-slicing.
//!
//! # Workloads
//!
//! [`Case::TransposeBlock`] and [`Case::TransposeBulk`] time the block kernel
//! alone over a resident run and over a rotating streaming run.
//! [`Case::BitsliceAbsorb`] and [`Case::BitsliceUnpack`] time the two halves
//! of the BCH bit-slice conversion: one lane group's whole message absorbed
//! through the transpose and the bit-sliced recurrence, and the reduced
//! register read back as packed per-frame parity.
//! [`Case::MatrixTranspose`] times the whole `BitMatrix::transpose` consumer,
//! its output allocation, tile assembly and tail mask included.
//! [`Case::BchEncodeBitslice`] times the whole bit-sliced BCH batch encode
//! through the production family entry point; it runs the dispatched lane
//! whatever the arm names, so the cell that uses it is an identity control
//! that pins the consumer this conversion sits inside.
//!
//! A code is named by its mother-field degree; the runner's case carries the
//! modulus and designed distance of that row, and the arm derives the
//! redundancy, the dimension and the block count from the constructed code
//! rather than from a declared size.

use gf2_coding::bch::encode::{BchEncodeWorkspace, EncodeFamily, SystematicLayout};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};
use gf2_kernels_simd::bch_encode::{
    self, BchEncodeFns, BITSLICE_LANES,
};
use gf2_kernels_simd::transpose::{self, Transpose64x64Fn, TransposeLane};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hint::black_box;
use std::io;
use std::time::{Duration, Instant};
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::host::CpuAffinity;
use tuning_campaign_support::timing::{execution_windows_configured, TimingSample, FIXTURE_BANKS};
use tuning_campaign_support::transport;

/// The block kernel an arm runs, resolved from `GF2_TRANSPOSE_LANE`.
struct Arm {
    kernel: Transpose64x64Fn,
    /// The tag the receipt's `selected_path` carries for this arm.
    tag: String,
}

impl Arm {
    fn from_env() -> Self {
        let requested = std::env::var("GF2_TRANSPOSE_LANE")
            .unwrap_or_else(|_| fail("GF2_TRANSPOSE_LANE names the lane this arm runs"));
        if requested == "production" {
            let fns = transpose::detect()
                .unwrap_or_else(|| fail("no transpose lane is available on this host"));
            return Arm {
                kernel: fns.transpose_64x64,
                tag: format!("production/{}", fns.name),
            };
        }
        let named = TransposeLane::from_name(&requested)
            .unwrap_or_else(|| fail(format!("{requested:?} names no transpose lane")));
        let kernel = transpose::lane(named).unwrap_or_else(|| {
            fail(format!(
                "this host cannot run the {} lane",
                named.name()
            ))
        });
        Arm {
            kernel,
            tag: format!("pinned/{}", named.name()),
        }
    }
}

/// One measured workload, forwarded verbatim by the runner to both arms.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "kebab-case")]
enum Case {
    /// `blocks` 64×64 bit-block transposes over one resident run.
    TransposeBlock { blocks: usize, seed: u64 },
    /// The same over a run large enough that the banks rotate.
    TransposeBulk { blocks: usize, seed: u64 },
    /// One lane group's message absorbed through the bit-slice conversion.
    BitsliceAbsorb {
        degree: usize,
        modulus: u64,
        designed_distance: u64,
        seed: u64,
    },
    /// One lane group's reduced register read back as packed parity.
    BitsliceUnpack {
        degree: usize,
        modulus: u64,
        designed_distance: u64,
        seed: u64,
    },
    /// The whole `BitMatrix::transpose` consumer.
    MatrixTranspose {
        rows: usize,
        cols: usize,
        seed: u64,
    },
    /// The whole bit-sliced BCH batch encode.
    BchEncodeBitslice {
        degree: usize,
        modulus: u64,
        designed_distance: u64,
        batch: usize,
        seed: u64,
    },
}

/// Request the runner writes on this child's stdin.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    cell_id: String,
    arm: String,
    role: String,
    pair: u32,
    case: Value,
    cache_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cold_calls: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decoder: Option<Value>,
    windows: u32,
    window_target_ms: u32,
    cpus: Vec<u32>,
    workers_declared: u32,
}

#[derive(Serialize)]
struct Window {
    calls: u64,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct Conversion {
    setup_ns: u64,
    pack_ns: u64,
    unpack_ns: u64,
    batch_fill_ns: u64,
    dispatch_ns: u64,
}

/// The one canonical result line this child writes.
#[derive(Serialize)]
struct ArmResult {
    schema: String,
    windows: Vec<Window>,
    cache_state_applied: String,
    workers_observed: u32,
    cpus_observed: Vec<u32>,
    selected_path: Option<String>,
    conversion: Option<Conversion>,
    quality: Option<Value>,
    calibrated: bool,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("transpose-lane-arm: {message}");
    std::process::exit(2);
}

/// Mean nanoseconds of `repeats` runs of `body`, rounded up.
///
/// Every probe runs before the timed windows, so no probe's cost enters a
/// measured window.
fn per_call_ns(repeats: u64, mut body: impl FnMut()) -> u64 {
    let start = Instant::now();
    for _ in 0..repeats.max(1) {
        body();
    }
    (start.elapsed().as_nanos() as u64).div_ceil(repeats.max(1))
}

/// Repetitions the dispatch probe averages over: one capability lookup costs
/// a few nanoseconds, at or below the resolution of a single `Instant`
/// interval.
const DISPATCH_PROBE_REPEATS: u64 = 4096;

/// A run of `blocks` pseudorandom 64-word bit blocks.
fn blocks(count: usize, seed: u64) -> Vec<[u64; 64]> {
    let mut mixer = SplitMix64::new(seed);
    (0..count)
        .map(|_| {
            let mut block = [0u64; 64];
            for word in block.iter_mut() {
                *word = mixer.next_u64();
            }
            block
        })
        .collect()
}

/// The binary BCH code of one declared mother-field row.
fn binary_bch(degree: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
    let field = Gf2mField::new(degree, modulus);
    let extension = BinaryPrimeExt::new(field).unwrap_or_else(|error| {
        fail(format!("the declared primitive polynomial is rejected: {error:?}"))
    });
    let distance = DesignedDistance::try_from(designed_distance).unwrap_or_else(|error| {
        fail(format!("the declared designed distance is rejected: {error:?}"))
    });
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: distance,
    })
    .unwrap_or_else(|error| fail(format!("the declared BCH construction is rejected: {error:?}")))
}

/// The bit-slice bundle this arm runs: the dispatched bundle with the arm's
/// lane substituted for its transpose primitive.
fn bitslice_bundle(arm: &Arm) -> BchEncodeFns {
    let base = bch_encode::detect().unwrap_or_else(bch_encode::scalar);
    BchEncodeFns {
        transpose_lane_block: arm.kernel,
        ..base
    }
}

/// The measured state of one cell, with the fixture banks it rotates through.
enum State {
    Transpose {
        banks: Vec<Vec<[u64; 64]>>,
        output: Vec<[u64; 64]>,
        kernel: Transpose64x64Fn,
    },
    BitsliceAbsorb {
        fns: BchEncodeFns,
        redundancy: usize,
        scratch: Vec<u64>,
        banks: Vec<Vec<[u64; BITSLICE_LANES]>>,
    },
    BitsliceUnpack {
        fns: BchEncodeFns,
        redundancy: usize,
        scratch: Vec<u64>,
    },
    MatrixTranspose {
        banks: Vec<BitMatrix>,
        kernel: Transpose64x64Fn,
    },
    BchEncodeBitslice {
        code: Box<BinaryBchCode>,
        messages: Vec<BitVec>,
        workspace: Box<BchEncodeWorkspace<u64>>,
        codewords: Vec<BitVec>,
    },
}

impl State {
    /// One measured call. The returned word is consumed by `black_box` so no
    /// call is optimized away.
    fn run(&mut self, bank: usize) -> u64 {
        match self {
            State::Transpose {
                banks,
                output,
                kernel,
            } => {
                let input = &banks[bank % banks.len()];
                for (block, slot) in input.iter().zip(output.iter_mut()) {
                    kernel(black_box(block), slot);
                }
                output[0][0]
            }
            State::BitsliceAbsorb {
                fns,
                redundancy,
                scratch,
                banks,
            } => {
                let windows = &banks[bank % banks.len()];
                let parts = bch_encode::bitslice_split(scratch, *redundancy);
                let (register, masks) = (parts.register, parts.masks);
                for window in windows.iter() {
                    fns.absorb_block(register, masks, black_box(window), BITSLICE_LANES);
                }
                register[1]
            }
            State::BitsliceUnpack {
                fns,
                redundancy,
                scratch,
            } => {
                let words = redundancy.div_ceil(64);
                let (masks, rest) = scratch.split_at_mut(*redundancy);
                let (register, parity) = rest.split_at_mut(*redundancy + 1);
                let _ = masks;
                fns.unpack_parity(black_box(register), *redundancy, parity);
                parity[words - 1]
            }
            State::MatrixTranspose { banks, kernel } => {
                let matrix = black_box(&banks[bank % banks.len()]);
                matrix.transpose_with_block_kernel(*kernel).row_words(0)[0]
            }
            State::BchEncodeBitslice {
                code,
                messages,
                workspace,
                codewords,
            } => {
                code.encode_batch_family_into(
                    EncodeFamily::BitsliceInterleaved,
                    black_box(messages),
                    SystematicLayout::default(),
                    workspace,
                    codewords,
                )
                .unwrap_or_else(|error| fail(format!("the batch did not encode: {error:?}")));
                codewords[0].words()[0]
            }
        }
    }
}

/// How many independent working sets the arm allocates.
fn bank_count(cache_state: &str) -> usize {
    if cache_state == "streaming" {
        FIXTURE_BANKS
    } else {
        1
    }
}

/// Prepares one cell's buffers, resolves its route and records its probes.
fn prepare(case: &Case, arm: &Arm, cache_state: &str) -> (State, String, Conversion) {
    let banks = bank_count(cache_state);
    let dispatch_ns = per_call_ns(DISPATCH_PROBE_REPEATS, || {
        black_box(transpose::detect());
    });
    let mut conversion = Conversion {
        setup_ns: 0,
        pack_ns: 0,
        unpack_ns: 0,
        batch_fill_ns: 0,
        dispatch_ns,
    };

    let state = match *case {
        Case::TransposeBlock { blocks: count, seed } | Case::TransposeBulk { blocks: count, seed } => {
            conversion.setup_ns = per_call_ns(1, || {
                black_box(blocks(count, seed));
            });
            State::Transpose {
                banks: (0..banks)
                    .map(|bank| blocks(count, seed.wrapping_add(bank as u64 * 0x9E37)))
                    .collect(),
                output: vec![[0u64; 64]; count],
                kernel: arm.kernel,
            }
        }
        Case::BitsliceAbsorb {
            degree,
            modulus,
            designed_distance,
            seed,
        } => {
            conversion.setup_ns = per_call_ns(1, || {
                black_box(binary_bch(degree, modulus, designed_distance));
            });
            let code = binary_bch(degree, modulus, designed_distance);
            let redundancy = code.n() - code.k();
            // One lane group absorbs the whole message of its frames, one
            // 64-degree block per step, so the block count is the code's.
            let count = code.k().div_ceil(BITSLICE_LANES);
            let workspace = code.encode_workspace();
            let mut scratch = vec![0u64; bch_encode::bitslice_scratch_words(redundancy)];
            {
                let parts = bch_encode::bitslice_split(&mut scratch, redundancy);
                bch_encode::bitslice_masks(&workspace.registers().low, parts.masks);
            }
            conversion.batch_fill_ns = per_call_ns(1, || {
                black_box(lane_windows(count, seed));
            });
            State::BitsliceAbsorb {
                fns: bitslice_bundle(arm),
                redundancy,
                scratch,
                banks: (0..banks)
                    .map(|bank| lane_windows(count, seed.wrapping_add(bank as u64 * 0x9E37)))
                    .collect(),
            }
        }
        Case::BitsliceUnpack {
            degree,
            modulus,
            designed_distance,
            seed,
        } => {
            conversion.setup_ns = per_call_ns(1, || {
                black_box(binary_bch(degree, modulus, designed_distance));
            });
            let code = binary_bch(degree, modulus, designed_distance);
            let redundancy = code.n() - code.k();
            let workspace = code.encode_workspace();
            let fns = bitslice_bundle(arm);
            let mut scratch = vec![0u64; bch_encode::bitslice_scratch_words(redundancy)];
            {
                let parts = bch_encode::bitslice_split(&mut scratch, redundancy);
                bch_encode::bitslice_masks(&workspace.registers().low, parts.masks);
            }
            // A reduced register, so the timed unpack reads the words a real
            // lane group leaves rather than zeros.
            {
                let parts = bch_encode::bitslice_split(&mut scratch, redundancy);
                let (register, masks) = (parts.register, parts.masks);
                for window in lane_windows(code.k().div_ceil(BITSLICE_LANES), seed) {
                    fns.absorb_block(register, masks, &window, BITSLICE_LANES);
                }
            }
            State::BitsliceUnpack {
                fns,
                redundancy,
                scratch,
            }
        }
        Case::MatrixTranspose { rows, cols, seed } => {
            conversion.setup_ns = per_call_ns(1, || {
                black_box(BitMatrix::random_seeded(rows, cols, seed));
            });
            State::MatrixTranspose {
                banks: (0..banks)
                    .map(|bank| {
                        BitMatrix::random_seeded(rows, cols, seed.wrapping_add(bank as u64 * 0x9E37))
                    })
                    .collect(),
                kernel: arm.kernel,
            }
        }
        Case::BchEncodeBitslice {
            degree,
            modulus,
            designed_distance,
            batch,
            seed,
        } => {
            conversion.setup_ns = per_call_ns(1, || {
                black_box(binary_bch(degree, modulus, designed_distance));
            });
            let code = Box::new(binary_bch(degree, modulus, designed_distance));
            if !code.encode_family_available(
                EncodeFamily::BitsliceInterleaved,
                SystematicLayout::default(),
            ) {
                fail("this code does not implement the bit-sliced encoding family");
            }
            let dimension = code.k();
            let messages: Vec<BitVec> = (0..batch)
                .map(|index| BitVec::random_seeded(dimension, seed.wrapping_add(index as u64 + 1)))
                .collect();
            conversion.batch_fill_ns = per_call_ns(4, || {
                let regenerated: Vec<BitVec> = (0..batch)
                    .map(|index| {
                        BitVec::random_seeded(dimension, seed.wrapping_add(index as u64 + 1))
                    })
                    .collect();
                black_box(regenerated);
            });
            // The one caller-side preparation the route takes: the workspace
            // the family entry point borrows.
            conversion.pack_ns = per_call_ns(16, || {
                black_box(code.encode_workspace());
            });
            let codewords = vec![BitVec::zeros(code.n()); batch];
            State::BchEncodeBitslice {
                workspace: Box::new(code.encode_workspace()),
                codewords,
                code,
                messages,
            }
        }
    };

    let tag = match *case {
        // The whole-consumer BCH cell runs the production family entry point,
        // which resolves its own block kernel: it reports the dispatched lane
        // whatever the arm names, which is what makes the cell an identity
        // control rather than a lane comparison.
        Case::BchEncodeBitslice { .. } => format!(
            "consumer/{}",
            transpose::detect()
                .map(|fns| fns.name)
                .unwrap_or("no-transpose-lane")
        ),
        _ => arm.tag.clone(),
    };
    (state, tag, conversion)
}

/// `count` lane-group windows of pseudorandom message degrees.
fn lane_windows(count: usize, seed: u64) -> Vec<[u64; BITSLICE_LANES]> {
    let mut mixer = SplitMix64::new(seed ^ 0x5bd1_e995_0000_0001);
    (0..count)
        .map(|_| {
            let mut window = [0u64; BITSLICE_LANES];
            for word in window.iter_mut() {
                *word = mixer.next_u64();
            }
            window
        })
        .collect()
}

fn main() {
    let sentinel = std::env::var(transport::FRESH_CASE_VAR).ok();
    let request: Request =
        match transport::read_guarded_case(sentinel.as_deref(), io::stdin().lock()) {
            Ok(request) => request,
            Err(error) => fail(error),
        };
    let case: Case = match serde_json::from_value(request.case.clone()) {
        Ok(case) => case,
        Err(error) => fail(format!("case does not decode: {error}")),
    };
    if request.cold_calls.is_some() {
        fail("this family declares no fixed-call cold cell");
    }
    if request.decoder.is_some() {
        fail("this family declares no decoder cell");
    }
    if request.workers_declared != 1 {
        fail(format!(
            "this family declares one worker per cell, got {}",
            request.workers_declared
        ));
    }
    if !matches!(request.cache_state.as_str(), "warm" | "streaming") {
        fail(format!(
            "this family declares warm and streaming cells only, got {:?}",
            request.cache_state
        ));
    }

    let arm = Arm::from_env();
    let (mut state, selected_path, conversion) = prepare(&case, &arm, &request.cache_state);

    // The declared cache policy, applied before calibration: one untimed pass
    // of the timed body over every bank of the working set.
    let banks = bank_count(&request.cache_state);
    for bank in 0..banks {
        black_box(state.run(bank));
    }

    let mut body = |index: usize| {
        black_box(state.run(index));
    };
    let samples: Vec<TimingSample> = match execution_windows_configured(
        0,
        u64::from(request.windows),
        Duration::from_millis(u64::from(request.window_target_ms)),
        &mut body,
        |_| Ok(()),
    ) {
        Ok(samples) => samples,
        Err(error) => fail(format!("timing failed: {error}")),
    };

    let cpus_observed = CpuAffinity::observe()
        .map(|affinity| affinity.cpus().to_vec())
        .unwrap_or_default();
    let result = ArmResult {
        schema: "zen3-benchmark-arm-result-v1".into(),
        windows: samples
            .iter()
            .map(|sample| Window {
                calls: sample.calls,
                elapsed_ns: sample.elapsed_ns,
            })
            .collect(),
        cache_state_applied: request.cache_state.clone(),
        workers_observed: 1,
        cpus_observed,
        selected_path: Some(selected_path),
        conversion: Some(conversion),
        quality: None,
        calibrated: true,
    };
    if let Err(error) = transport::write_result_line(io::stdout().lock(), &result) {
        fail(error);
    }
}
