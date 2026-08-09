//! Uniform matrix sampling from reproducible, domain-separated ChaCha20 streams.
//!
//! An address becomes a 32-byte seed made from four little-endian `u64` words:
//! campaign root, field order, matrix dimension, and a final word with an
//! eight-bit purpose tag above a 56-bit stream index. Entries are then drawn by
//! exact byte rejection, so reducing an accepted byte modulo the field order
//! is unbiased. Matrix entry `k` is stored at row `k / n`, column `k % n`.

use std::error::Error;
use std::fmt;

use gf2_core::gfp::Fp;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

/// Exclusive upper bound for a valid [`StreamIndex`].
pub const STREAM_INDEX_LIMIT: u64 = 1 << 56;

/// The finite-field orders supported by the campaign sampler.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u64)]
pub enum FieldOrder {
    /// The field $\mathbb{F}_3$.
    F3 = 3,
    /// The field $\mathbb{F}_5$.
    F5 = 5,
    /// The field $\mathbb{F}_7$.
    F7 = 7,
}

impl FieldOrder {
    /// Returns the field order encoded in a sampler address.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }
}

/// A disjoint use of the campaign's matrix stream namespace.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum StreamPurpose {
    /// Draws reserved for sampler and estimator validation.
    Validation = 1,
    /// Draws reserved for timing fixtures and measurements.
    Timing = 2,
    /// Draws belonging to a published campaign cell.
    CampaignCell = 3,
    /// Draws reserved for rare-event estimation.
    RareEvent = 4,
}

impl StreamPurpose {
    /// Returns the eight-bit tag embedded in a sampler seed.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// A stream index that is representable in the low 56 seed bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StreamIndex(u64);

impl StreamIndex {
    /// Constructs an index in the low 56 bits of an address seed.
    ///
    /// # Errors
    ///
    /// Returns [`StreamIndexError::TooLarge`] when `value` is at least
    /// [`STREAM_INDEX_LIMIT`]. Rejecting that value keeps the purpose and index
    /// encodings disjoint by construction.
    pub const fn new(value: u64) -> Result<Self, StreamIndexError> {
        if value < STREAM_INDEX_LIMIT {
            Ok(Self(value))
        } else {
            Err(StreamIndexError::TooLarge { value })
        }
    }

    /// Returns the validated stream index.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The reason a raw stream index cannot be used in an address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamIndexError {
    /// The index would overlap the eight-bit purpose tag.
    TooLarge {
        /// The rejected raw index.
        value: u64,
    },
}

impl fmt::Display for StreamIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { value } => write!(
                formatter,
                "stream index {value} is outside the low 56-bit address range"
            ),
        }
    }
}

impl Error for StreamIndexError {}

/// The complete semantic address of one matrix stream.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MatrixAddress {
    campaign_root: u64,
    field_order: FieldOrder,
    dimension: usize,
    purpose: StreamPurpose,
    stream: StreamIndex,
}

impl MatrixAddress {
    /// Constructs the address of a stream of square matrices.
    ///
    /// `dimension` is part of the seed, not merely the output shape: matrices
    /// of different sizes are therefore domain-separated even when all other
    /// address components match. Construct `stream` with [`StreamIndex::new`]
    /// before calling this function.
    #[must_use]
    pub const fn new(
        campaign_root: u64,
        field_order: FieldOrder,
        dimension: usize,
        purpose: StreamPurpose,
        stream: StreamIndex,
    ) -> Self {
        Self {
            campaign_root,
            field_order,
            dimension,
            purpose,
            stream,
        }
    }

    /// Returns the campaign root component of this address.
    #[must_use]
    pub const fn campaign_root(self) -> u64 {
        self.campaign_root
    }

    /// Returns the field-order component of this address.
    #[must_use]
    pub const fn field_order(self) -> FieldOrder {
        self.field_order
    }

    /// Returns the square-matrix dimension encoded in this address.
    #[must_use]
    pub const fn dimension(self) -> usize {
        self.dimension
    }

    /// Returns the purpose component of this address.
    #[must_use]
    pub const fn purpose(self) -> StreamPurpose {
        self.purpose
    }

    /// Returns the validated stream-index component of this address.
    #[must_use]
    pub const fn stream(self) -> StreamIndex {
        self.stream
    }

    /// Derives the exact 32-byte ChaCha20 seed for this address.
    ///
    /// The seed has four little-endian `u64` words: campaign root, field order,
    /// dimension, then `(purpose.tag() << 56) | stream`. This is `O(1)` and
    /// allocates no memory.
    #[must_use]
    pub fn seed(self) -> [u8; 32] {
        let final_word = (u64::from(self.purpose.tag()) << 56) | self.stream.get();
        let words = [
            self.campaign_root,
            self.field_order.as_u64(),
            self.dimension as u64,
            final_word,
        ];
        let mut seed = [0; 32];
        for (offset, word) in words.into_iter().enumerate() {
            let start = offset * 8;
            seed[start..start + 8].copy_from_slice(&word.to_le_bytes());
        }
        seed
    }

    fn matrix_len(self) -> usize {
        self.dimension.checked_mul(self.dimension).expect(
            "matrix dimension squared exceeds usize; no caller-supplied buffer can represent it",
        )
    }
}

/// The reason a sampler could not open an address.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplerError {
    /// The sampler's compile-time field order differs from its address.
    FieldOrderMismatch {
        /// The field order requested by the sampler type.
        sampler_order: u64,
        /// The field order encoded in the address.
        address_order: FieldOrder,
    },
}

impl fmt::Display for SamplerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldOrderMismatch {
                sampler_order,
                address_order,
            } => write!(
                formatter,
                "MatrixSampler<F_{sampler_order}> cannot open an F_{} address",
                address_order.as_u64()
            ),
        }
    }
}

impl Error for SamplerError {}

/// Bytes retained from one ChaCha20 refill. A fixed inline buffer avoids a
/// heap allocation for every matrix draw.
const RNG_BUFFER_BYTES: usize = 64;

/// A uniform matrix sampler over `Fp<Q>` from one domain-separated stream.
///
/// # Workflow
///
/// ```
/// use gf2_core::gfp::Fp;
/// use gf2_stats::sampler::{
///     FieldOrder, MatrixAddress, MatrixSampler, StreamIndex, StreamPurpose,
/// };
///
/// let address = MatrixAddress::new(
///     0xB488_F02C,
///     FieldOrder::F5,
///     2,
///     StreamPurpose::CampaignCell,
///     StreamIndex::new(7).expect("the stream index fits in 56 bits"),
/// );
/// let mut sampler = MatrixSampler::<5>::new(address).expect("field orders match");
/// let mut entries = [Fp::<5>::new(0); 4];
/// sampler.fill_next_matrix(&mut entries);
/// ```
pub struct MatrixSampler<const Q: u64> {
    address: MatrixAddress,
    rng: ChaCha20Rng,
    bytes: [u8; RNG_BUFFER_BYTES],
    cursor: usize,
}

impl<const Q: u64> MatrixSampler<Q> {
    /// Opens the stream named by `address`.
    ///
    /// # Errors
    ///
    /// Returns [`SamplerError::FieldOrderMismatch`] unless `Q` equals the
    /// address's field order. This prevents a sampler from consuming a stream
    /// reserved for one field while producing entries for another.
    pub fn new(address: MatrixAddress) -> Result<Self, SamplerError> {
        if Q != address.field_order().as_u64() {
            return Err(SamplerError::FieldOrderMismatch {
                sampler_order: Q,
                address_order: address.field_order(),
            });
        }

        Ok(Self {
            address,
            rng: ChaCha20Rng::from_seed(address.seed()),
            bytes: [0; RNG_BUFFER_BYTES],
            cursor: RNG_BUFFER_BYTES,
        })
    }

    /// Returns the address used to seed this sampler.
    #[must_use]
    pub const fn address(&self) -> MatrixAddress {
        self.address
    }

    /// Draws one exactly uniform element of `Fp<Q>` by byte rejection.
    ///
    /// The accepted byte range is the greatest multiple of `Q` below 256, so
    /// each residue appears equally often before reduction. For the supported
    /// campaign fields the expected cost is under 1.02 raw bytes per element.
    #[must_use]
    pub fn next_entry(&mut self) -> Fp<Q> {
        let bound = 256 - 256 % Q;
        loop {
            let byte = self.next_raw_byte();
            if u64::from(byte) < bound {
                return Fp::new(u64::from(byte) % Q);
            }
        }
    }

    /// Fills `out` with the next matrix in row-major entry order.
    ///
    /// Draw `k` is stored as $A[k / n][k \bmod n]$, where `n` is the dimension
    /// encoded in [`MatrixAddress`]. The caller supplies and owns `out`, so the
    /// operation performs no per-matrix heap allocation.
    ///
    /// # Panics
    ///
    /// Panics when `out.len()` is not exactly `n * n`, or when `n * n` cannot
    /// fit in `usize`.
    ///
    /// # Complexity
    ///
    /// $O(n^2)$ expected time and $O(1)$ additional memory.
    pub fn fill_next_matrix(&mut self, out: &mut [Fp<Q>]) {
        let expected_len = self.address.matrix_len();
        assert_eq!(
            out.len(),
            expected_len,
            "matrix buffer length must equal the square of the address dimension"
        );
        for entry in out {
            *entry = self.next_entry();
        }
    }

    fn next_raw_byte(&mut self) -> u8 {
        if self.cursor == self.bytes.len() {
            self.rng.fill_bytes(&mut self.bytes);
            self.cursor = 0;
        }
        let byte = self.bytes[self.cursor];
        self.cursor += 1;
        byte
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn address(
        root: u64,
        field_order: FieldOrder,
        dimension: usize,
        purpose: StreamPurpose,
        stream: u64,
    ) -> MatrixAddress {
        MatrixAddress::new(
            root,
            field_order,
            dimension,
            purpose,
            StreamIndex::new(stream).expect("test stream index fits"),
        )
    }

    #[test]
    fn seed_is_four_little_endian_words_with_purpose_above_stream() {
        let address = address(
            0x0123_4567_89AB_CDEF,
            FieldOrder::F7,
            9,
            StreamPurpose::Timing,
            0x0123_4567_89AB,
        );

        assert_eq!(
            address.seed(),
            [
                0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 7, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0,
                0, 0, 0, 0, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 0, 2,
            ]
        );
    }

    #[test]
    fn stream_index_is_limited_to_its_low_56_bits() {
        assert_eq!(
            StreamIndex::new(STREAM_INDEX_LIMIT),
            Err(StreamIndexError::TooLarge {
                value: STREAM_INDEX_LIMIT,
            })
        );
        assert_eq!(
            StreamIndex::new(STREAM_INDEX_LIMIT - 1)
                .expect("largest valid stream index")
                .get(),
            STREAM_INDEX_LIMIT - 1
        );
    }

    #[test]
    fn every_address_component_changes_the_seed_and_raw_stream() {
        let base = address(7, FieldOrder::F3, 2, StreamPurpose::CampaignCell, 0);
        let variants = [
            base,
            address(8, FieldOrder::F3, 2, StreamPurpose::CampaignCell, 0),
            address(7, FieldOrder::F5, 2, StreamPurpose::CampaignCell, 0),
            address(7, FieldOrder::F3, 3, StreamPurpose::CampaignCell, 0),
            address(7, FieldOrder::F3, 2, StreamPurpose::Validation, 0),
            address(7, FieldOrder::F3, 2, StreamPurpose::CampaignCell, 1),
        ];
        let seeds: BTreeSet<_> = variants.iter().map(|address| address.seed()).collect();
        assert_eq!(seeds.len(), variants.len());

        let raw_streams: BTreeSet<_> = variants
            .iter()
            .map(|address| {
                let mut rng = ChaCha20Rng::from_seed(address.seed());
                let mut bytes = [0; 32];
                rng.fill_bytes(&mut bytes);
                bytes
            })
            .collect();
        assert_ne!(
            raw_streams.len(),
            1,
            "address-separated seeds must diverge as raw ChaCha20 streams"
        );
        assert_eq!(raw_streams.len(), variants.len());
    }

    #[test]
    fn identical_addresses_reproduce_the_same_matrix() {
        let address = address(
            0xB488_F02C,
            FieldOrder::F5,
            3,
            StreamPurpose::CampaignCell,
            17,
        );
        let mut first = MatrixSampler::<5>::new(address).expect("field orders match");
        let mut second = MatrixSampler::<5>::new(address).expect("field orders match");
        let mut first_matrix = [Fp::<5>::new(0); 9];
        let mut second_matrix = [Fp::<5>::new(0); 9];

        first.fill_next_matrix(&mut first_matrix);
        second.fill_next_matrix(&mut second_matrix);

        assert_eq!(first_matrix, second_matrix);
    }

    #[test]
    fn fill_uses_row_major_draw_order_without_reallocating_the_callers_buffer() {
        let address = address(11, FieldOrder::F7, 2, StreamPurpose::Validation, 4);
        let mut entry_sampler = MatrixSampler::<7>::new(address).expect("field orders match");
        let expected: [Fp<7>; 4] = std::array::from_fn(|_| entry_sampler.next_entry());
        let mut matrix_sampler = MatrixSampler::<7>::new(address).expect("field orders match");
        let mut matrix = vec![Fp::<7>::new(0); 4];
        let pointer = matrix.as_ptr();
        let capacity = matrix.capacity();

        matrix_sampler.fill_next_matrix(&mut matrix);

        assert_eq!(
            matrix.as_ptr(),
            pointer,
            "filling a caller buffer must not reallocate it"
        );
        assert_eq!(
            matrix.capacity(),
            capacity,
            "filling a caller buffer must not grow it"
        );
        for (index, entry) in matrix.iter().enumerate() {
            assert_eq!(
                *entry,
                expected[index],
                "draw {index} must occupy A[{}][{}]",
                index / 2,
                index % 2
            );
        }
    }

    #[test]
    #[should_panic(expected = "matrix buffer length")]
    fn fill_rejects_a_buffer_with_the_wrong_shape() {
        let address = address(11, FieldOrder::F3, 2, StreamPurpose::Validation, 4);
        let mut sampler = MatrixSampler::<3>::new(address).expect("field orders match");
        let mut matrix = [Fp::<3>::new(0); 3];
        sampler.fill_next_matrix(&mut matrix);
    }

    #[test]
    fn entries_are_uniform_within_six_binomial_standard_errors() {
        fn check<const Q: u64>(field_order: FieldOrder) {
            const DRAWS: usize = 300_000;
            let address = address(0xFEED, field_order, 1, StreamPurpose::Validation, 0);
            let mut sampler = MatrixSampler::<Q>::new(address).expect("field orders match");
            let mut counts = [0usize; 7];
            for _ in 0..DRAWS {
                counts[sampler.next_entry().value() as usize] += 1;
            }

            let expected = DRAWS as f64 / Q as f64;
            let tolerance = 6.0 * (expected * (1.0 - 1.0 / Q as f64)).sqrt();
            for (value, &count) in counts.iter().take(Q as usize).enumerate() {
                let deviation = (count as f64 - expected).abs();
                assert!(
                    deviation < tolerance,
                    "q={Q}, residue={value}: count={count}, deviation={deviation}, tolerance={tolerance}"
                );
            }
        }

        check::<3>(FieldOrder::F3);
        check::<5>(FieldOrder::F5);
        check::<7>(FieldOrder::F7);
    }

    #[derive(Debug, Eq, PartialEq)]
    struct GoldenVector {
        seed: [u8; 32],
        matrix: [u64; 4],
    }

    #[test]
    fn golden_vectors_pin_every_field_and_purpose() {
        let cases = [
            address(
                0x0123_4567_89AB_CDEF,
                FieldOrder::F3,
                2,
                StreamPurpose::Validation,
                1,
            ),
            address(
                0x0F1E_2D3C_4B5A_6978,
                FieldOrder::F5,
                2,
                StreamPurpose::Timing,
                0x1234,
            ),
            address(
                0xA5A5_A5A5_A5A5_A5A5,
                FieldOrder::F7,
                2,
                StreamPurpose::CampaignCell,
                42,
            ),
            address(
                0xDEAD_BEEF_0123_4567,
                FieldOrder::F3,
                2,
                StreamPurpose::RareEvent,
                0x00AB_CDEF,
            ),
        ];

        let actual = [
            snapshot::<3>(cases[0]),
            snapshot::<5>(cases[1]),
            snapshot::<7>(cases[2]),
            snapshot::<3>(cases[3]),
        ];
        let expected = [
            GoldenVector {
                seed: [
                    0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 3, 0, 0, 0, 0, 0, 0, 0, 2, 0,
                    0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1,
                ],
                matrix: [2, 0, 0, 1],
            },
            GoldenVector {
                seed: [
                    0x78, 0x69, 0x5A, 0x4B, 0x3C, 0x2D, 0x1E, 0x0F, 5, 0, 0, 0, 0, 0, 0, 0, 2, 0,
                    0, 0, 0, 0, 0, 0, 0x34, 0x12, 0, 0, 0, 0, 0, 2,
                ],
                matrix: [0, 1, 0, 1],
            },
            GoldenVector {
                seed: [
                    0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 7, 0, 0, 0, 0, 0, 0, 0, 2, 0,
                    0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 3,
                ],
                matrix: [1, 0, 6, 2],
            },
            GoldenVector {
                seed: [
                    0x67, 0x45, 0x23, 0x01, 0xEF, 0xBE, 0xAD, 0xDE, 3, 0, 0, 0, 0, 0, 0, 0, 2, 0,
                    0, 0, 0, 0, 0, 0, 0xEF, 0xCD, 0xAB, 0, 0, 0, 0, 4,
                ],
                matrix: [0, 2, 0, 0],
            },
        ];

        assert_eq!(actual, expected);
    }

    fn snapshot<const Q: u64>(address: MatrixAddress) -> GoldenVector {
        let mut sampler = MatrixSampler::<Q>::new(address).expect("field orders match");
        let mut matrix = [Fp::<Q>::new(0); 4];
        sampler.fill_next_matrix(&mut matrix);
        GoldenVector {
            seed: address.seed(),
            matrix: matrix.map(Fp::value),
        }
    }
}
