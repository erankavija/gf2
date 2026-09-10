//! Consumer workloads for the bit-storage production profile (jit:04b85d10).
//!
//! Each workload is one production public entry point exercised on packed
//! GF(2) data, and each *path* of a workload is one route production code
//! already offers to reach a kernel. A path is never a new implementation of
//! a kernel: the profile attributes cost across routes that exist, so the
//! ranked experiments it publishes rest on measured production behaviour.
//!
//! The three consumer families the profile covers are
//!
//! - **logical** — row and parity operations over packed rows, dense
//!   elimination and dense matrix-vector products;
//! - **count** — population counts and the fused reductions built on them,
//!   including the zero tests that consume only whether a count is zero;
//! - **layout** — bit-layout transforms: the 64x64 block transpose, the dense
//!   transpose built on it, and both binary BCH encoding entry points.
//!
//! [`prepare`] builds every buffer a cell reads before timing starts, and
//! [`Prepared::run`] is the body the fixed-window timing protocol calls. The
//! separation is what keeps allocation out of a `warm-reuse` cell and inside a
//! `fresh-alloc` one, matching the cache-state contract of
//! `dev/active/4e732b56/workload-selection.md` section 5.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use gf2_coding::bch::encode::{
    max_parallel_batch_workers, BchEncodeWorkspace, EncodeFamily, SystematicLayout,
};
use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_coding::bch::{BchCode, BchEncoder, CodeRate};
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::traits::BlockEncoder;
use gf2_core::alg::rref::rref;
use gf2_core::field::extension::{BinaryPrimeExt, FieldId};
use gf2_core::gf2m::Gf2mField;
use gf2_core::kernels::backend::select_backend_for_size;
use gf2_core::kernels::ops::{popcount, resolve_xor_inplace, xor_inplace, XorInplaceFn};
use gf2_core::kernels::scalar::ScalarBackend;
use gf2_core::kernels::Backend;
use gf2_core::matrix::{matvec_route, MatvecRoute};
use gf2_core::{BitMatrix, BitVec};
use gf2_kernels_simd::transpose::{self, Transpose64x64Fn};

/// The request body the runner forwards verbatim to both arms of a cell.
///
/// It carries the workload identity and its declared sizes, never the arm's
/// route: arms differ through their environment so that the two children of a
/// pair read byte-identical input.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Workload identity, equal to the addendum cell's `workload.identity`.
    pub workload: String,
    /// Declared sizes, equal to the addendum cell's `workload.size`.
    pub size: BTreeMap<String, u64>,
    /// Fixture seed, equal to the addendum cell's `workload.seed`.
    pub seed: u64,
}

impl Case {
    /// Reads one declared size.
    fn size(&self, key: &str) -> Result<usize, String> {
        self.size
            .get(key)
            .ok_or_else(|| format!("case for {} declares no size {key:?}", self.workload))
            .and_then(|value| {
                usize::try_from(*value).map_err(|_| format!("size {key:?} exceeds usize"))
            })
    }
}

/// The generator behind every seeded fixture this harness builds.
///
/// Fixtures come from `BitVec::random_seeded` and `BitMatrix::random_seeded`
/// in `gf2-core`, which seed `rand::rngs::StdRng` through
/// `SeedableRng::seed_from_u64` and fill whole words. The versions are the
/// ones the harness lockfile resolves; `tests/arm_paths.rs` reads the
/// lockfile and fails when it resolves anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FixtureRng {
    /// The calls that turn a case seed into fixture words.
    pub generator: &'static str,
    /// The algorithm those calls run at the resolved versions.
    pub algorithm: &'static str,
    /// Resolved `rand` version.
    pub rand: &'static str,
    /// Resolved `rand_chacha` version.
    pub rand_chacha: &'static str,
    /// Resolved `rand_core` version.
    pub rand_core: &'static str,
}

/// The fixture generator of this build.
pub const FIXTURE_RNG: FixtureRng = FixtureRng {
    generator: "gf2_core BitVec::random_seeded and BitMatrix::random_seeded: \
                rand::rngs::StdRng::seed_from_u64(seed), then Rng::fill over u64 words; \
                per-row, per-block and per-bank seeds are the case-seed offsets `prepare` \
                defines",
    algorithm: "ChaCha12 (rand 0.8 StdRng is rand_chacha::ChaCha12Rng); seed_from_u64 \
                expands the u64 seed to the 32-byte key with PCG32 (rand_core 0.6)",
    rand: "0.8.8",
    rand_chacha: "0.3.1",
    rand_core: "0.6.4",
};

/// Setup and conversion costs an arm reports beside its timing windows.
///
/// The five fields are the ones the receipt schema carries. A phase a
/// workload does not have reports zero, which is the honest statement that
/// the route performs no such work; it is never a stand-in for a phase that
/// exists and was not measured. A measured phase below one nanosecond per
/// call also reports zero, because [`per_call_ns`] rounds down.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Conversion {
    /// One-shot preparation the timed body does not repeat: code
    /// construction, table build, matrix materialization.
    pub setup_ns: u64,
    /// Caller representation to the route's packed input, per timed call. A
    /// route that packs inside its timed call reports here the caller-side
    /// preparation it borrows instead, and says so where it sets the field.
    pub pack_ns: u64,
    /// Route output back to the caller representation, per timed call.
    pub unpack_ns: u64,
    /// Filling a batch's input buffers, per timed call.
    pub batch_fill_ns: u64,
    /// Route selection the timed body performs, per timed call.
    pub dispatch_ns: u64,
}

/// Times `reps` repetitions of `body` and returns the nanoseconds one
/// repetition took.
///
/// Phases short enough to sit under the clock's resolution are measured in
/// bulk so their reported figure is a per-call cost rather than a rounding
/// artefact.
fn per_call_ns(reps: u32, mut body: impl FnMut()) -> u64 {
    let reps = reps.max(1);
    let start = Instant::now();
    for _ in 0..reps {
        body();
    }
    let elapsed = start.elapsed().as_nanos();
    u64::try_from(elapsed / u128::from(reps)).unwrap_or(u64::MAX)
}

/// The route an arm takes through production code.
///
/// Every variant names a route production code already selects or already
/// exposes as a public entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmPath {
    /// The route the production dispatcher selects with no override.
    Current,
    /// The size-dispatched kernel entry point, re-selected per call.
    OpsDispatched,
    /// The same kernel with `resolve_xor_inplace` hoisted out of the loop.
    OpsResolved,
    /// The scalar backend, called directly.
    ScalarBackend,
    /// The detected SIMD backend, called directly with detection hoisted.
    SimdBackend,
    /// A zero test spelled as a full population count.
    CountOnes,
    /// The same zero test spelled as `find_first_one`.
    FindFirstOne,
    /// The bit-serial reference encoding family.
    FamilyPolyRemainderScalar,
    /// The table-driven remainder family.
    FamilyTableRemainder,
    /// The bit-sliced interleaved family.
    FamilyBitsliceInterleaved,
    /// The carry-less-multiply fold family.
    FamilyClmulFold,
    /// The caller-buffer batch entry point `encode_batch_into`, paired
    /// against the allocating `encode_batch` on the same batch.
    CallerBuffer,
    /// The portable 64x64 block transpose.
    TransposeScalar,
    /// The detected 64x64 block transpose lane.
    TransposeDetected,
}

impl ArmPath {
    /// Decodes the path identity an arm reads from its environment.
    ///
    /// # Errors
    ///
    /// Returns the unrecognised spelling so a mis-declared plan fails before
    /// it produces samples.
    pub fn parse(value: &str) -> Result<Self, String> {
        Ok(match value {
            "current" => Self::Current,
            "ops-dispatched" => Self::OpsDispatched,
            "ops-resolved" => Self::OpsResolved,
            "scalar-backend" => Self::ScalarBackend,
            "simd-backend" => Self::SimdBackend,
            "count-ones" => Self::CountOnes,
            "find-first-one" => Self::FindFirstOne,
            "family-poly-remainder-scalar" => Self::FamilyPolyRemainderScalar,
            "family-table-remainder" => Self::FamilyTableRemainder,
            "family-bitslice-interleaved" => Self::FamilyBitsliceInterleaved,
            "family-clmul-fold" => Self::FamilyClmulFold,
            "caller-buffer" => Self::CallerBuffer,
            "transpose-scalar" => Self::TransposeScalar,
            "transpose-detected" => Self::TransposeDetected,
            other => return Err(format!("unknown arm path {other:?}")),
        })
    }

    /// The encoding family this path pins, where it pins one.
    fn encode_family(self) -> Option<EncodeFamily> {
        Some(match self {
            Self::FamilyPolyRemainderScalar => EncodeFamily::PolyRemainderScalar,
            Self::FamilyTableRemainder => EncodeFamily::TableRemainder,
            Self::FamilyBitsliceInterleaved => EncodeFamily::BitsliceInterleaved,
            Self::FamilyClmulFold => EncodeFamily::ClmulFold,
            _ => return None,
        })
    }
}

/// A prepared cell: every buffer allocated, every route resolved, ready for
/// the timed windows.
pub struct Prepared {
    /// The route the arm observed itself taking, reported in the receipt.
    pub selected_path: String,
    /// Setup and conversion costs, present for whole-consumer workloads.
    pub conversion: Option<Conversion>,
    /// Worker count the arm observed for itself at run time.
    pub workers_observed: u32,
    /// Generator of the seeded fixture, absent where the fixture is
    /// deterministic and the seed is unused.
    pub fixture_rng: Option<FixtureRng>,
    state: State,
    sink: u64,
}

impl Prepared {
    /// Runs one timed call against fixture bank `bank`.
    pub fn run(&mut self, bank: usize) {
        self.sink ^= self.state.run(bank);
    }

    /// Consumes the accumulated result so no timed call is dead code.
    pub fn sink(&self) -> u64 {
        self.sink
    }
}

/// Rows of one logical bank: a packed matrix stored as independent rows.
struct RowBank {
    rows: Vec<Vec<u64>>,
}

impl RowBank {
    fn new(row_count: usize, words: usize, seed: u64) -> Self {
        let rows = (0..row_count)
            .map(|row| {
                let vector = BitVec::random_seeded(words * 64, seed.wrapping_add(row as u64 + 1));
                vector.words().to_vec()
            })
            .collect();
        Self { rows }
    }

    /// XORs each even row into its odd neighbour with `xor`.
    ///
    /// The pairs are disjoint and the operation is its own inverse, so a bank
    /// alternates between two bit patterns however many timed calls run and
    /// the working set never degenerates to zero.
    fn xor_pairs(&mut self, xor: impl Fn(&mut [u64], &[u64])) -> u64 {
        let pairs = self.rows.len() / 2;
        for pair in 0..pairs {
            let (left, right) = self.rows.split_at_mut(pair * 2 + 1);
            xor(black_box(&mut left[pair * 2]), black_box(&right[0]));
        }
        self.rows[0][0]
    }
}

/// The prepared state of one workload.
enum State {
    /// Packed row XOR over independent rows.
    RowXor { banks: Vec<RowBank>, xor: XorRoute },
    /// Population count over a packed buffer.
    Popcount {
        banks: Vec<Vec<u64>>,
        route: CountRoute,
    },
    /// Zero test over a packed buffer.
    ZeroTest { banks: Vec<BitVec>, path: ArmPath },
    /// Dense reduced row echelon form of a packed matrix.
    DenseRref { banks: Vec<BitMatrix> },
    /// Dense matrix-vector product over GF(2).
    DenseMatvec { banks: Vec<(BitMatrix, BitVec)> },
    /// Dense transpose of a packed matrix.
    DenseTranspose { banks: Vec<BitMatrix> },
    /// The 64x64 block transpose primitive.
    Transpose64 {
        banks: Vec<Vec<[u64; 64]>>,
        output: Vec<[u64; 64]>,
        kernel: Transpose64x64Fn,
    },
    /// Sparse parity-check syndrome of a codeword.
    LdpcSyndrome { code: LdpcCode, banks: Vec<BitVec> },
    /// The whole zero-syndrome codeword check.
    LdpcCodewordCheck {
        code: LdpcCode,
        banks: Vec<BitVec>,
        path: ArmPath,
    },
    /// Batch BCH encoding through the workspace entry point.
    BchBatchWorkspace {
        code: Box<BinaryBchCode>,
        messages: Vec<BitVec>,
        workspace: Box<BchEncodeWorkspace<u64>>,
        codewords: Vec<BitVec>,
        family: Option<EncodeFamily>,
    },
    /// Batch BCH encoding through the allocating entry point.
    BchBatchAllocating {
        code: Box<BinaryBchCode>,
        messages: Vec<BitVec>,
    },
    /// Batch BCH encoding across workers.
    BchBatchParallel {
        code: Box<BinaryBchCode>,
        messages: Vec<BitVec>,
        workspaces: Vec<BchEncodeWorkspace<u64>>,
        codewords: Vec<BitVec>,
    },
    /// The DVB-T2 field-polynomial encoding entry point.
    DvbBchEncode {
        encoder: Box<BchEncoder>,
        messages: Vec<BitVec>,
    },
    /// The per-message field-identity check of a batch encode.
    FieldIdHint { calls: usize },
}

/// The resolved XOR route of a [`State::RowXor`] arm.
enum XorRoute {
    /// `kernels::ops::xor_inplace`, which re-selects a backend per call.
    Dispatched,
    /// The function pointer `resolve_xor_inplace` returns, resolved once.
    Resolved(XorInplaceFn),
    /// The scalar backend, called directly.
    Scalar,
    /// The detected SIMD backend, called directly.
    Simd(&'static gf2_core::kernels::simd::SimdBackend),
}

/// The resolved population-count route of a [`State::Popcount`] arm.
enum CountRoute {
    /// `kernels::ops::popcount`, which re-selects a backend per call.
    Dispatched,
    /// The scalar backend, called directly.
    Scalar,
    /// The detected SIMD backend, called directly.
    Simd(&'static gf2_core::kernels::simd::SimdBackend),
}

impl State {
    fn run(&mut self, bank: usize) -> u64 {
        match self {
            State::RowXor { banks, xor } => {
                let index = bank % banks.len();
                match xor {
                    XorRoute::Dispatched => banks[index].xor_pairs(xor_inplace),
                    XorRoute::Resolved(function) => {
                        let function = *function;
                        banks[index].xor_pairs(function)
                    }
                    XorRoute::Scalar => {
                        banks[index].xor_pairs(|dst, src| ScalarBackend.xor(dst, src))
                    }
                    XorRoute::Simd(backend) => {
                        let backend = *backend;
                        banks[index].xor_pairs(|dst, src| backend.xor(dst, src))
                    }
                }
            }
            State::Popcount { banks, route } => {
                let buffer = black_box(&banks[bank % banks.len()]);
                match route {
                    CountRoute::Dispatched => popcount(buffer),
                    CountRoute::Scalar => ScalarBackend.popcount(buffer),
                    CountRoute::Simd(backend) => backend.popcount(buffer),
                }
            }
            State::ZeroTest { banks, path } => {
                let vector = black_box(&banks[bank % banks.len()]);
                let zero = match path {
                    ArmPath::FindFirstOne => vector.find_first_one().is_none(),
                    _ => vector.count_ones() == 0,
                };
                u64::from(zero)
            }
            State::DenseRref { banks } => {
                let matrix = black_box(&banks[bank % banks.len()]);
                rref(matrix, false).rank as u64
            }
            State::DenseMatvec { banks } => {
                let (matrix, x) = black_box(&banks[bank % banks.len()]);
                matrix.matvec(x).words()[0]
            }
            State::DenseTranspose { banks } => {
                let matrix = black_box(&banks[bank % banks.len()]);
                matrix.transpose().row_words(0)[0]
            }
            State::Transpose64 {
                banks,
                output,
                kernel,
            } => {
                let input = &banks[bank % banks.len()];
                for (block, out) in input.iter().zip(output.iter_mut()) {
                    kernel(black_box(block), out);
                }
                output[0][0]
            }
            State::LdpcSyndrome { code, banks } => {
                let codeword = black_box(&banks[bank % banks.len()]);
                code.syndrome(codeword).words()[0]
            }
            State::LdpcCodewordCheck { code, banks, path } => {
                let codeword = black_box(&banks[bank % banks.len()]);
                let valid = match path {
                    ArmPath::FindFirstOne => code.syndrome(codeword).find_first_one().is_none(),
                    _ => code.is_valid_codeword(codeword),
                };
                u64::from(valid)
            }
            State::BchBatchWorkspace {
                code,
                messages,
                workspace,
                codewords,
                family,
            } => {
                match family {
                    Some(family) => code
                        .encode_batch_family_into(
                            *family,
                            black_box(messages),
                            SystematicLayout::default(),
                            workspace,
                            codewords,
                        )
                        .expect("an available family encodes a validated batch"),
                    None => code
                        .encode_batch_into(
                            black_box(messages),
                            SystematicLayout::default(),
                            workspace,
                            codewords,
                        )
                        .expect("the selected family encodes a validated batch"),
                }
                codewords[0].words()[0]
            }
            State::BchBatchAllocating { code, messages } => {
                let codewords = code
                    .encode_batch(black_box(messages), SystematicLayout::default())
                    .expect("the selected family encodes a validated batch");
                codewords[0].words()[0]
            }
            State::BchBatchParallel {
                code,
                messages,
                workspaces,
                codewords,
            } => {
                code.encode_batch_parallel_into(
                    black_box(messages),
                    SystematicLayout::default(),
                    workspaces,
                    codewords,
                )
                .expect("the selected family encodes a validated batch");
                codewords[0].words()[0]
            }
            State::FieldIdHint { calls } => {
                let mut sink = 0u64;
                for _ in 0..*calls {
                    sink ^= u64::from(FieldId::prime(black_box(2)).is_ok());
                }
                sink
            }
            State::DvbBchEncode { encoder, messages } => {
                let mut sink = 0u64;
                for message in black_box(messages.iter()) {
                    sink ^= encoder.encode(message).words()[0];
                }
                sink
            }
        }
    }
}

/// Builds the binary BCH code of the named mother field and designed
/// distance.
///
/// The two large rows are the mother codes of the DVB-T2 short and normal
/// frames of `dev/active/4e732b56/workload-selection.md` section 2: same
/// mother field, same primitive polynomial and same designed distance, hence
/// the same generator polynomial and the same redundancy. `BinaryBchCode`
/// constructs cyclic codes at the primitive length, so the shortened DVB-T2
/// lengths are reached through [`BchCode`] instead and are measured by the
/// `dvb-bch-encode` workload.
fn binary_bch(
    degree: usize,
    modulus: u64,
    designed_distance: u64,
) -> Result<BinaryBchCode, String> {
    let field = Gf2mField::new(degree, modulus);
    let extension = BinaryPrimeExt::new(field)
        .map_err(|error| format!("the declared primitive polynomial is rejected: {error:?}"))?;
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(designed_distance)
            .map_err(|error| format!("the declared designed distance is rejected: {error:?}"))?,
    })
    .map_err(|error| format!("the declared BCH construction is rejected: {error:?}"))
}

/// The mother field of one declared BCH row.
///
/// The rows are those of `dev/active/4e732b56/workload-selection.md`
/// section 2; `degree` selects the row.
fn bch_row(degree: usize) -> Result<(u64, u64), String> {
    Ok(match degree {
        8 => (0b1_0001_1101, 9),
        14 => (0b100_0000_0010_1011, 25),
        16 => (0b1_0000_0000_0010_1101, 25),
        other => {
            return Err(format!(
                "no declared BCH row of mother-field degree {other}"
            ))
        }
    })
}

/// Number of fixture banks a cache state reads.
///
/// A streaming cell rotates through the protocol's eight banks so successive
/// calls do not reuse cache-resident data; every other cell reads one.
#[must_use]
pub fn bank_count(cache_state: &str, banks: usize) -> usize {
    if cache_state == "streaming" {
        banks
    } else {
        1
    }
}

/// Prepares one cell's buffers and resolves its route.
///
/// `cache_state` is the declared state of the cell and `banks` the protocol's
/// fixture-bank count; together they fix how many independent working sets
/// the arm allocates.
///
/// # Errors
///
/// Returns a message naming the workload, the size, or the path that this
/// build cannot serve, so a mis-declared cell fails before it emits samples.
pub fn prepare(
    case: &Case,
    path: ArmPath,
    cache_state: &str,
    banks: usize,
) -> Result<Prepared, String> {
    let bank_count = bank_count(cache_state, banks);
    let simd = gf2_core::kernels::simd::maybe_simd();
    let mut conversion = None;
    let mut workers_observed = 1u32;
    let mut fixture_rng = Some(FIXTURE_RNG);

    let (state, selected_path) = match case.workload.as_str() {
        "row-xor" => {
            let words = case.size("words")?;
            let rows = case.size("rows")?;
            let banks = (0..bank_count)
                .map(|bank| RowBank::new(rows, words, case.seed.wrapping_add(bank as u64 * 0x9E37)))
                .collect();
            let (route, name) = match path {
                ArmPath::OpsDispatched | ArmPath::Current => (
                    XorRoute::Dispatched,
                    format!("ops-dispatched/{}", select_backend_for_size(words).name()),
                ),
                ArmPath::OpsResolved => (
                    XorRoute::Resolved(resolve_xor_inplace(words)),
                    format!("ops-resolved/{}", select_backend_for_size(words).name()),
                ),
                ArmPath::ScalarBackend => (XorRoute::Scalar, "scalar-backend".to_owned()),
                ArmPath::SimdBackend => {
                    let backend = simd.ok_or("this host publishes no SIMD backend")?;
                    (
                        XorRoute::Simd(backend),
                        format!("simd-backend/{}", backend.name()),
                    )
                }
                other => return Err(format!("row-xor does not serve {other:?}")),
            };
            (State::RowXor { banks, xor: route }, name)
        }
        "popcount" => {
            let words = case.size("words")?;
            let banks = (0..bank_count)
                .map(|bank| {
                    BitVec::random_seeded(words * 64, case.seed.wrapping_add(bank as u64 * 0x9E37))
                        .words()
                        .to_vec()
                })
                .collect();
            let (route, name) = match path {
                ArmPath::OpsDispatched | ArmPath::Current => (
                    CountRoute::Dispatched,
                    format!("ops-dispatched/{}", select_backend_for_size(words).name()),
                ),
                ArmPath::ScalarBackend => (CountRoute::Scalar, "scalar-backend".to_owned()),
                ArmPath::SimdBackend => {
                    let backend = simd.ok_or("this host publishes no SIMD backend")?;
                    (
                        CountRoute::Simd(backend),
                        format!("simd-backend/{}", backend.name()),
                    )
                }
                other => return Err(format!("popcount does not serve {other:?}")),
            };
            (State::Popcount { banks, route }, name)
        }
        "zero-test" => {
            let words = case.size("words")?;
            let set_bit = case.size("set_bit")?;
            // An all-zero buffer with at most one set bit: nothing is drawn.
            fixture_rng = None;
            let banks = (0..bank_count)
                .map(|_| {
                    let mut vector = BitVec::zeros(words * 64);
                    if set_bit < words * 64 {
                        vector.set(set_bit, true);
                    }
                    vector
                })
                .collect();
            let name = match path {
                ArmPath::FindFirstOne => {
                    if simd.is_some() {
                        "find-first-one/simd".to_owned()
                    } else {
                        "find-first-one/scalar".to_owned()
                    }
                }
                ArmPath::CountOnes | ArmPath::Current => {
                    format!("count-ones/{}", select_backend_for_size(words).name())
                }
                other => return Err(format!("zero-test does not serve {other:?}")),
            };
            (State::ZeroTest { banks, path }, name)
        }
        "dense-rref" => {
            let rows = case.size("rows")?;
            let cols = case.size("cols")?;
            let banks: Vec<BitMatrix> = (0..bank_count)
                .map(|bank| {
                    BitMatrix::random_seeded(
                        rows,
                        cols,
                        case.seed.wrapping_add(bank as u64 * 0x9E37),
                    )
                })
                .collect();
            let setup_ns = per_call_ns(1, || {
                black_box(BitMatrix::random_seeded(rows, cols, case.seed));
            });
            let stride = banks[0].stride_words();
            conversion = Some(Conversion {
                setup_ns,
                pack_ns: 0,
                unpack_ns: 0,
                batch_fill_ns: 0,
                // `rref` resolves its XOR kernel once per call through this
                // function.
                dispatch_ns: per_call_ns(4096, || {
                    black_box(resolve_xor_inplace(black_box(stride)));
                }),
            });
            (
                State::DenseRref { banks },
                "current/blocked-m4ri".to_owned(),
            )
        }
        "dense-matvec" => {
            let rows = case.size("rows")?;
            let cols = case.size("cols")?;
            let banks: Vec<(BitMatrix, BitVec)> = (0..bank_count)
                .map(|bank| {
                    let seed = case.seed.wrapping_add(bank as u64 * 0x9E37);
                    (
                        BitMatrix::random_seeded(rows, cols, seed),
                        BitVec::random_seeded(cols, seed ^ 0x5DEE),
                    )
                })
                .collect();
            let stride = banks[0].0.stride_words();
            let setup_ns = per_call_ns(1, || {
                black_box(BitMatrix::random_seeded(rows, cols, case.seed));
            });
            conversion = Some(Conversion {
                setup_ns,
                pack_ns: 0,
                // The route allocates and bit-appends its output vector.
                unpack_ns: per_call_ns(256, || {
                    let mut y = BitVec::with_capacity(rows);
                    for _ in 0..rows {
                        y.push_bit(true);
                    }
                    black_box(y);
                }),
                batch_fill_ns: 0,
                // The stride route `matvec` selects per call; the kernel-table
                // read that follows it is crate-private and is not timed.
                dispatch_ns: per_call_ns(4096, || {
                    black_box(matvec_route(black_box(stride)));
                }),
            });
            let route = match matvec_route(stride) {
                MatvecRoute::Simd if simd.is_some() => "current/simd-and-popcnt",
                MatvecRoute::Simd => "current/scalar-row-parity",
                MatvecRoute::Scalar => "current/scalar-row-parity",
            };
            (State::DenseMatvec { banks }, route.to_owned())
        }
        "dense-transpose" => {
            let rows = case.size("rows")?;
            let cols = case.size("cols")?;
            let banks: Vec<BitMatrix> = (0..bank_count)
                .map(|bank| {
                    BitMatrix::random_seeded(
                        rows,
                        cols,
                        case.seed.wrapping_add(bank as u64 * 0x9E37),
                    )
                })
                .collect();
            let setup_ns = per_call_ns(1, || {
                black_box(BitMatrix::random_seeded(rows, cols, case.seed));
            });
            let route = gf2_core::matrix::transpose_route(rows.div_ceil(64), cols.div_ceil(64));
            conversion = Some(Conversion {
                setup_ns,
                pack_ns: 0,
                unpack_ns: 0,
                batch_fill_ns: 0,
                // The outer-loop route `transpose` selects per call; its
                // block-kernel lookup is crate-private and is not timed.
                dispatch_ns: per_call_ns(4096, || {
                    black_box(gf2_core::matrix::transpose_route(
                        black_box(rows.div_ceil(64)),
                        black_box(cols.div_ceil(64)),
                    ));
                }),
            });
            let name = match route.macro_tile_blocks() {
                Some(blocks) => format!("current/macro-tiled-{blocks}"),
                None => "current/simple".to_owned(),
            };
            (State::DenseTranspose { banks }, name)
        }
        "transpose-64x64" => {
            let blocks = case.size("blocks")?;
            let banks: Vec<Vec<[u64; 64]>> = (0..bank_count)
                .map(|bank| {
                    let seed = case.seed.wrapping_add(bank as u64 * 0x9E37);
                    (0..blocks)
                        .map(|block| {
                            let vector =
                                BitVec::random_seeded(64 * 64, seed.wrapping_add(block as u64 + 1));
                            let mut square = [0u64; 64];
                            square.copy_from_slice(vector.words());
                            square
                        })
                        .collect()
                })
                .collect();
            let (kernel, name): (Transpose64x64Fn, String) = match path {
                ArmPath::TransposeScalar => (
                    transpose::transpose_64x64_scalar,
                    "transpose-scalar/portable".to_owned(),
                ),
                ArmPath::TransposeDetected | ArmPath::Current => {
                    let fns = transpose::detect()
                        .ok_or("this host publishes no block-transpose bundle")?;
                    (
                        fns.transpose_64x64,
                        format!("transpose-detected/{}", fns.name),
                    )
                }
                other => return Err(format!("transpose-64x64 does not serve {other:?}")),
            };
            (
                State::Transpose64 {
                    banks,
                    output: vec![[0u64; 64]; blocks],
                    kernel,
                },
                name,
            )
        }
        "ldpc-syndrome" | "ldpc-codeword-check" => {
            let n = case.size("n")?;
            let code = match n {
                16200 => LdpcCode::dvb_t2_short(CodeRate::Rate1_2),
                64800 => LdpcCode::dvb_t2_normal(CodeRate::Rate1_2),
                other => return Err(format!("no declared LDPC row of length {other}")),
            };
            let banks: Vec<BitVec> = (0..bank_count)
                .map(|bank| {
                    BitVec::random_seeded(code.n(), case.seed.wrapping_add(bank as u64 * 0x9E37))
                })
                .collect();
            let checks = code.m();
            conversion = Some(Conversion {
                // The construction of the code this case measures.
                setup_ns: per_call_ns(1, || {
                    black_box(if n == 16200 {
                        LdpcCode::dvb_t2_short(CodeRate::Rate1_2)
                    } else {
                        LdpcCode::dvb_t2_normal(CodeRate::Rate1_2)
                    });
                }),
                pack_ns: 0,
                // The syndrome route allocates its output with one bit of
                // capacity per check and appends one bit per check.
                unpack_ns: per_call_ns(64, || {
                    let mut y = BitVec::with_capacity(checks);
                    for _ in 0..checks {
                        y.push_bit(true);
                    }
                    black_box(y);
                }),
                batch_fill_ns: 0,
                dispatch_ns: 0,
            });
            if case.workload == "ldpc-syndrome" {
                (
                    State::LdpcSyndrome { code, banks },
                    "current/csr-bit-at-a-time-matvec".to_owned(),
                )
            } else {
                let name = match path {
                    ArmPath::FindFirstOne => "syndrome-then-find-first-one",
                    _ => "syndrome-then-count-ones",
                };
                (
                    State::LdpcCodewordCheck { code, banks, path },
                    name.to_owned(),
                )
            }
        }
        "bch-encode-batch" | "bch-encode-batch-alloc" | "bch-encode-batch-parallel" => {
            let degree = case.size("degree")?;
            let batch = case.size("batch")?;
            let (modulus, designed_distance) = bch_row(degree)?;
            let setup_ns = per_call_ns(1, || {
                black_box(binary_bch(degree, modulus, designed_distance).is_ok());
            });
            let code = Box::new(binary_bch(degree, modulus, designed_distance)?);
            let layout = SystematicLayout::default();
            let messages: Vec<BitVec> = (0..batch)
                .map(|index| {
                    BitVec::random_seeded(code.k(), case.seed.wrapping_add(index as u64 + 1))
                })
                .collect();
            // Regenerating the batch: allocation plus the fixture generator's
            // fill, which a consumer replaces with a copy of its own data.
            let batch_fill_ns = per_call_ns(4, || {
                let filled: Vec<BitVec> = (0..batch)
                    .map(|index| {
                        BitVec::random_seeded(code.k(), case.seed.wrapping_add(index as u64 + 1))
                    })
                    .collect();
                black_box(filled);
            });
            let selected = code.selected_encode_family(layout, batch);
            let dispatch_ns = per_call_ns(1024, || {
                black_box(code.selected_encode_family(layout, black_box(batch)));
            });
            // The families pack messages inside the timed call, so this field
            // carries the one caller-side preparation the route takes instead:
            // building the workspace `encode_batch_into` borrows.
            let pack_ns = per_call_ns(16, || {
                black_box(code.encode_workspace());
            });
            conversion = Some(Conversion {
                setup_ns,
                pack_ns,
                unpack_ns: 0,
                batch_fill_ns,
                dispatch_ns,
            });
            match case.workload.as_str() {
                "bch-encode-batch" => {
                    let family = path.encode_family();
                    if let Some(family) = family {
                        if !code.encode_family_available(family, layout) {
                            return Err(format!("this code does not implement {family:?}"));
                        }
                    }
                    let name = match family {
                        Some(family) => format!("family-pinned/{family:?}"),
                        None => format!("current/{selected:?}"),
                    };
                    (
                        State::BchBatchWorkspace {
                            workspace: Box::new(code.encode_workspace()),
                            codewords: vec![BitVec::zeros(code.n()); batch],
                            code,
                            messages,
                            family,
                        },
                        name,
                    )
                }
                "bch-encode-batch-alloc" => match path {
                    // The caller-buffer arm of the allocation cell: the same
                    // selected family through `encode_batch_into`, with the
                    // workspace and the output prepared here rather than
                    // allocated inside the timed call.
                    ArmPath::CallerBuffer => (
                        State::BchBatchWorkspace {
                            workspace: Box::new(code.encode_workspace()),
                            codewords: vec![BitVec::zeros(code.n()); batch],
                            code,
                            messages,
                            family: None,
                        },
                        format!("caller-buffer/{selected:?}"),
                    ),
                    ArmPath::Current => (
                        State::BchBatchAllocating { code, messages },
                        format!("current-allocating/{selected:?}"),
                    ),
                    other => {
                        return Err(format!("bch-encode-batch-alloc does not serve {other:?}"))
                    }
                },
                _ => {
                    let declared = case.size("workers")?;
                    let workers = NonZeroUsize::new(declared)
                        .ok_or("a parallel cell declares at least one worker")?;
                    workers_observed = u32::try_from(max_parallel_batch_workers().get())
                        .map_err(|_| "the observed worker count exceeds u32")?;
                    (
                        State::BchBatchParallel {
                            workspaces: code.encode_workspaces(workers),
                            codewords: vec![BitVec::zeros(code.n()); batch],
                            code,
                            messages,
                        },
                        format!("current-parallel/{selected:?}"),
                    )
                }
            }
        }
        "dvb-bch-encode" => {
            let n = case.size("n")?;
            let batch = case.size("batch")?;
            let frame = match n {
                7200 => gf2_coding::bch::dvb_t2::FrameSize::Short,
                32400 => gf2_coding::bch::dvb_t2::FrameSize::Normal,
                other => return Err(format!("no declared DVB-T2 BCH row of length {other}")),
            };
            let setup_ns = per_call_ns(1, || {
                black_box(BchCode::dvb_t2(frame, CodeRate::Rate1_2));
            });
            let code = BchCode::dvb_t2(frame, CodeRate::Rate1_2);
            let k = code.k();
            let encoder = Box::new(BchEncoder::new(code));
            let messages: Vec<BitVec> = (0..batch)
                .map(|index| BitVec::random_seeded(k, case.seed.wrapping_add(index as u64 + 1)))
                .collect();
            // Regenerating the batch, as in the packed BCH workloads.
            let batch_fill_ns = per_call_ns(4, || {
                let filled: Vec<BitVec> = (0..batch)
                    .map(|index| BitVec::random_seeded(k, case.seed.wrapping_add(index as u64 + 1)))
                    .collect();
                black_box(filled);
            });
            conversion = Some(Conversion {
                setup_ns,
                // The route rebuilds the message as k field elements per call.
                pack_ns: 0,
                unpack_ns: 0,
                batch_fill_ns,
                dispatch_ns: 0,
            });
            (
                State::DvbBchEncode { encoder, messages },
                "current/field-polynomial-div-rem".to_owned(),
            )
        }
        "field-id-hint" => {
            let calls = case.size("calls")?;
            // The body checks a constant field identity and reads no buffer.
            fixture_rng = None;
            (
                State::FieldIdHint { calls },
                "current/arc-allocating-field-identity".to_owned(),
            )
        }
        other => return Err(format!("unknown workload {other:?}")),
    };

    Ok(Prepared {
        selected_path,
        conversion,
        workers_observed,
        fixture_rng,
        state,
        sink: 0,
    })
}
