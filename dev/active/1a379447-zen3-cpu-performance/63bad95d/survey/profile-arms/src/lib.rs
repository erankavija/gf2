//! Build-level arms of the profile families (jit:63bad95d).
//!
//! One executable measures a public gf2-core entry point on one cell's
//! operands. The plan names the build level an arm must be; the arm refuses a
//! request its own compile-time configuration does not match, installs the
//! measured owner on the profile levels, and reports the routes it observes.

use gf2_core::kernels::ops::{
    and_popcount, and_popcount_route, popcount, popcount_route, xor_inplace,
};
use gf2_core::kernels::select_backend_for_size;
use gf2_core::matrix::{matvec_route, transpose_route};
use gf2_core::tuning::{CoreTuning, CoreTuningCodec, ProfileRegistryBuilder};
use gf2_core::{BitMatrix, BitVec};
use serde::{Deserialize, Serialize};
use std::hint::black_box;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::protocol::{sha256_hex, BuildIdentity};

/// Names the build level the plan declares for an arm.
pub const BUILD_VAR: &str = "GF2_PROFILE_BUILD";
/// Names the measured core owner a profile arm installs, as an absolute path.
pub const OWNER_VAR: &str = "GF2_PROFILE_OWNER";
/// SHA-256 the plan pins for the file [`OWNER_VAR`] names.
pub const OWNER_SHA256_VAR: &str = "GF2_PROFILE_OWNER_SHA256";

/// Word count of the wide carry-less product a cell measures.
pub const CLMUL_WORDS: usize = 9;

/// Largest buffer length the compiled backend boundary is searched over.
const BOUNDARY_SEARCH_WORDS: usize = 64;

/// Resolves the build level the plan declares for an arm; a level no
/// profile arm serves is refused.
pub fn parse_build(name: &str) -> Result<BuildIdentity, String> {
    match serde_json::from_value(serde_json::Value::from(name)) {
        Ok(BuildIdentity::External) | Err(_) => Err(format!("unknown {BUILD_VAR} {name:?}")),
        Ok(build) => Ok(build),
    }
}

/// Whether `build` carries the profile.
pub fn profiled(build: BuildIdentity) -> bool {
    build != BuildIdentity::ConservativePortable
}

/// Refuses a level this executable's compile-time configuration is not.
pub fn check_executable(build: BuildIdentity) -> Result<(), String> {
    let baked = cfg!(gf2_tuning_baked);
    let static_avx2 = cfg!(target_feature = "avx2");
    if BuildIdentity::compiled(baked, static_avx2) == Some(build) {
        return Ok(());
    }
    Err(format!(
        "the plan declares {build:?}; this executable has baked={baked}, static-avx2={static_avx2}"
    ))
}

/// Installs the measured core owner the environment names and returns its
/// profile identity. Call before the first selector read of the process.
pub fn install_owner(path: &str, sha256: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    let observed = sha256_hex(&bytes);
    if observed != sha256 {
        return Err(format!(
            "{path} has SHA-256 {observed}, the plan pins {sha256}"
        ));
    }
    let text = String::from_utf8(bytes).map_err(|error| format!("{path}: {error}"))?;
    let registry = ProfileRegistryBuilder::new()
        .register::<CoreTuning, CoreTuningCodec>()
        .map_err(|error| format!("core codec: {error}"))?
        .build()
        .map_err(|error| format!("profile registry: {error}"))?;
    let prepared = registry
        .from_json(&text)
        .map_err(|error| format!("{path} does not reopen strictly: {error}"))?;
    let id = prepared.profile_id().as_str().to_owned();
    gf2_core::tuning::install(prepared)
        .map_err(|_| "a profile was resolved before the install".to_owned())?;
    Ok(id)
}

/// One cell's operation and operands.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Case {
    /// `kernels::ops::xor_inplace` over `words` words.
    XorInplace { words: usize, seed: u64 },
    /// `kernels::ops::popcount` over `words` words.
    Popcount { words: usize, seed: u64 },
    /// `kernels::ops::and_popcount` over `words` words.
    AndPopcount { words: usize, seed: u64 },
    /// `BitMatrix::matvec`, the output allocation and release included.
    Matvec {
        rows: usize,
        columns: usize,
        seed: u64,
    },
    /// `BitMatrix::transpose`, the output allocation and release included.
    Transpose {
        rows: usize,
        columns: usize,
        seed: u64,
    },
    /// `gf2m::wide::clmul_wide` over [`CLMUL_WORDS`]-word operands.
    ClmulWide { words: usize, seed: u64 },
}

fn draw(rng: &mut SplitMix64, count: usize) -> Vec<u64> {
    (0..count).map(|_| rng.next_u64()).collect()
}

fn random_matrix(rows: usize, columns: usize, rng: &mut SplitMix64) -> BitMatrix {
    let mut matrix = BitMatrix::zeros(rows, columns);
    let tail = columns % 64;
    for row in 0..rows {
        let words = matrix.row_words_mut(row);
        for word in words.iter_mut() {
            *word = rng.next_u64();
        }
        if tail != 0 {
            if let Some(last) = words.last_mut() {
                *last &= (1u64 << tail) - 1;
            }
        }
    }
    matrix
}

fn random_vector(bits: usize, rng: &mut SplitMix64) -> BitVec {
    let mut words = draw(rng, bits.div_ceil(64));
    let tail = bits % 64;
    if tail != 0 {
        if let Some(last) = words.last_mut() {
            *last &= (1u64 << tail) - 1;
        }
    }
    BitVec::from_words(words, bits)
}

/// A cell's prepared operands and its measured call.
pub enum Workload {
    Xor {
        target: Vec<u64>,
        source: Vec<u64>,
    },
    Popcount {
        buffer: Vec<u64>,
    },
    AndPopcount {
        left: Vec<u64>,
        right: Vec<u64>,
    },
    Matvec {
        matrix: BitMatrix,
        vector: BitVec,
    },
    Transpose {
        matrix: BitMatrix,
    },
    Clmul {
        left: [u64; CLMUL_WORDS],
        right: [u64; CLMUL_WORDS],
    },
}

impl Workload {
    /// Builds the operands `case` declares.
    pub fn build(case: &Case) -> Result<Self, String> {
        let positive = |name: &str, value: usize| {
            if value == 0 {
                Err(format!("the case declares zero {name}"))
            } else {
                Ok(())
            }
        };
        match *case {
            Case::XorInplace { words, seed } => {
                positive("words", words)?;
                let mut rng = SplitMix64::new(seed);
                Ok(Self::Xor {
                    target: draw(&mut rng, words),
                    source: draw(&mut rng, words),
                })
            }
            Case::Popcount { words, seed } => {
                positive("words", words)?;
                Ok(Self::Popcount {
                    buffer: draw(&mut SplitMix64::new(seed), words),
                })
            }
            Case::AndPopcount { words, seed } => {
                positive("words", words)?;
                let mut rng = SplitMix64::new(seed);
                Ok(Self::AndPopcount {
                    left: draw(&mut rng, words),
                    right: draw(&mut rng, words),
                })
            }
            Case::Matvec {
                rows,
                columns,
                seed,
            } => {
                positive("rows", rows)?;
                positive("columns", columns)?;
                let mut rng = SplitMix64::new(seed);
                let matrix = random_matrix(rows, columns, &mut rng);
                Ok(Self::Matvec {
                    matrix,
                    vector: random_vector(columns, &mut rng),
                })
            }
            Case::Transpose {
                rows,
                columns,
                seed,
            } => {
                positive("rows", rows)?;
                positive("columns", columns)?;
                let mut rng = SplitMix64::new(seed);
                Ok(Self::Transpose {
                    matrix: random_matrix(rows, columns, &mut rng),
                })
            }
            Case::ClmulWide { words, seed } => {
                if words != CLMUL_WORDS {
                    return Err(format!(
                        "this arm compiles the {CLMUL_WORDS}-word product; the case declares {words}"
                    ));
                }
                let mut rng = SplitMix64::new(seed);
                let mut left = [0u64; CLMUL_WORDS];
                let mut right = [0u64; CLMUL_WORDS];
                left.iter_mut().for_each(|word| *word = rng.next_u64());
                right.iter_mut().for_each(|word| *word = rng.next_u64());
                Ok(Self::Clmul { left, right })
            }
        }
    }

    /// SHA-256 of the operand words in declaration order, little-endian.
    pub fn operand_digest(&self) -> String {
        let mut bytes = Vec::new();
        let mut push = |words: &[u64]| {
            for word in words {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
        };
        let rows = |matrix: &BitMatrix, push: &mut dyn FnMut(&[u64])| {
            for row in 0..matrix.rows() {
                push(matrix.row_words(row));
            }
        };
        match self {
            Self::Xor { target, source } => {
                push(target);
                push(source);
            }
            Self::Popcount { buffer } => push(buffer),
            Self::AndPopcount { left, right } => {
                push(left);
                push(right);
            }
            Self::Matvec { matrix, vector } => {
                rows(matrix, &mut push);
                push(vector.words());
            }
            Self::Transpose { matrix } => rows(matrix, &mut push),
            Self::Clmul { left, right } => {
                push(left);
                push(right);
            }
        }
        sha256_hex(&bytes)
    }

    /// One measured call; the returned word keeps the result observable.
    #[inline]
    pub fn call(&mut self) -> u64 {
        match self {
            Self::Xor { target, source } => {
                xor_inplace(
                    black_box(target.as_mut_slice()),
                    black_box(source.as_slice()),
                );
                target[0]
            }
            Self::Popcount { buffer } => popcount(black_box(buffer.as_slice())),
            Self::AndPopcount { left, right } => {
                and_popcount(black_box(left.as_slice()), black_box(right.as_slice()))
            }
            Self::Matvec { matrix, vector } => {
                let product = black_box(&*matrix).matvec(black_box(&*vector));
                black_box(&product).words()[0]
            }
            Self::Transpose { matrix } => {
                let transposed = black_box(&*matrix).transpose();
                black_box(&transposed).row_words(0)[0]
            }
            Self::Clmul { left, right } => {
                let product: [u64; 2 * CLMUL_WORDS] =
                    gf2_core::gf2m::wide::clmul_wide(black_box(&*left), black_box(&*right));
                product[0] ^ product[2 * CLMUL_WORDS - 1]
            }
        }
    }

    /// Checks one call against a portable reference computed here.
    pub fn verify(&mut self) -> Result<(), String> {
        let mismatch = |what: &str| Err(format!("{what} differs from the portable reference"));
        match self {
            Self::Xor { target, source } => {
                let expected: Vec<u64> = target
                    .iter()
                    .zip(source.iter())
                    .map(|(a, b)| a ^ b)
                    .collect();
                xor_inplace(target, source);
                if *target != expected {
                    return mismatch("xor_inplace");
                }
            }
            Self::Popcount { buffer } => {
                let expected: u64 = buffer.iter().map(|word| u64::from(word.count_ones())).sum();
                if popcount(buffer) != expected {
                    return mismatch("popcount");
                }
            }
            Self::AndPopcount { left, right } => {
                let expected: u64 = left
                    .iter()
                    .zip(right.iter())
                    .map(|(a, b)| u64::from((a & b).count_ones()))
                    .sum();
                if and_popcount(left, right) != expected {
                    return mismatch("and_popcount");
                }
            }
            Self::Matvec { matrix, vector } => {
                let product = matrix.matvec(vector);
                for row in 0..matrix.rows() {
                    let parity = matrix
                        .row_words(row)
                        .iter()
                        .zip(vector.words())
                        .fold(0u32, |acc, (a, b)| acc ^ (a & b).count_ones())
                        & 1;
                    if product.get(row) != (parity == 1) {
                        return mismatch("matvec");
                    }
                }
            }
            Self::Transpose { matrix } => {
                let transposed = matrix.transpose();
                if (transposed.rows(), transposed.cols()) != (matrix.cols(), matrix.rows()) {
                    return mismatch("transpose shape");
                }
                let mut rng = SplitMix64::new(matrix.rows() as u64 ^ matrix.cols() as u64);
                let mut probes: Vec<(usize, usize)> = (0..4096)
                    .map(|_| {
                        (
                            rng.next_u64() as usize % matrix.rows(),
                            rng.next_u64() as usize % matrix.cols(),
                        )
                    })
                    .collect();
                probes.extend([(0, 0), (matrix.rows() - 1, matrix.cols() - 1)]);
                if probes
                    .iter()
                    .any(|&(row, col)| matrix.get(row, col) != transposed.get(col, row))
                {
                    return mismatch("transpose");
                }
            }
            Self::Clmul { left, right } => {
                let product: [u64; 2 * CLMUL_WORDS] = gf2_core::gf2m::wide::clmul_wide(left, right);
                let mut expected = [0u64; 2 * CLMUL_WORDS];
                for bit in 0..64 * CLMUL_WORDS {
                    if (left[bit / 64] >> (bit % 64)) & 1 == 0 {
                        continue;
                    }
                    for (index, word) in right.iter().enumerate() {
                        let at = bit / 64 + index;
                        expected[at] ^= word << (bit % 64);
                        if bit % 64 != 0 {
                            expected[at + 1] ^= word >> (64 - bit % 64);
                        }
                    }
                }
                if product != expected {
                    return mismatch("clmul_wide");
                }
            }
        }
        Ok(())
    }

    /// The entry point and the route the library reports for these operands.
    pub fn observed_route(&self) -> String {
        match self {
            Self::Xor { target, .. } => format!(
                "gf2-core/kernels::ops::xor_inplace/w={}/backend={}",
                target.len(),
                select_backend_for_size(target.len()).name()
            ),
            Self::Popcount { buffer } => format!(
                "gf2-core/kernels::ops::popcount/w={}/route={}",
                buffer.len(),
                popcount_route(buffer.len()).as_str()
            ),
            Self::AndPopcount { left, .. } => format!(
                "gf2-core/kernels::ops::and_popcount/w={}/route={}",
                left.len(),
                and_popcount_route(left.len()).as_str()
            ),
            Self::Matvec { matrix, .. } => format!(
                "gf2-core/BitMatrix::matvec+release/rows={}/cols={}/stride={}w/route={:?}",
                matrix.rows(),
                matrix.cols(),
                matrix.stride_words(),
                matvec_route(matrix.stride_words())
            ),
            Self::Transpose { matrix } => format!(
                "gf2-core/BitMatrix::transpose+release/rows={}/cols={}/route={:?}",
                matrix.rows(),
                matrix.cols(),
                transpose_route(matrix.rows().div_ceil(64), matrix.cols().div_ceil(64))
            ),
            Self::Clmul { .. } => format!("gf2-core/gf2m::wide::clmul_wide/w={CLMUL_WORDS}"),
        }
    }
}

/// Smallest buffer length the compiled bit-backend selector sends to SIMD,
/// searched up to a fixed bound.
pub fn compiled_simd_boundary() -> Option<usize> {
    (1..=BOUNDARY_SEARCH_WORDS).find(|&words| select_backend_for_size(words).name() == "simd")
}
