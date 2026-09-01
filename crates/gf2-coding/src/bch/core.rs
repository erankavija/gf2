//! Core BCH code types and algorithms.
//!
//! # Mathematical Background
//!
//! This module is the binary (GF(2)) implementation; the field-generic
//! construction model lives in [`crate::bch::spec`]. In the binary primitive
//! narrow-sense case implemented here, the code is defined by its generator
//! polynomial g(x), which has consecutive powers of a primitive element α as
//! roots in the extension field GF(2^m):
//!
//! g(x) = LCM(m₁(x), m₂(x), ..., m₂ₜ(x))
//!
//! where mᵢ(x) is the minimal polynomial of αⁱ, and t is the error correction
//! capability.
//!
//! # Encoding
//!
//! Systematic encoding divides the message polynomial (shifted by n-k positions)
//! by the generator polynomial:
//!
//! c(x) = x^(n-k) · m(x) + remainder(x^(n-k) · m(x) / g(x))
//!
//! # Decoding
//!
//! 1. **Syndrome computation**: Evaluate received polynomial at α, α², ..., α^(2t)
//! 2. **Berlekamp-Massey**: Find error locator polynomial from syndromes
//! 3. **Chien search**: Find roots of error locator (error positions)
//! 4. **Correction**: Flip bits at error positions
//!
//! # Hardened decoding over the canonical model
//!
//! [`BinaryBchDecoder`] runs that same algebra against the canonical
//! [`BinaryBchCode`](crate::bch::spec::BinaryBchCode) and reports a
//! [`BchDecodeOutcome`] instead of message bits: it verifies every candidate
//! correction by recomputing the corrected word's syndrome, keeps its
//! workspace allocation outside the per-word path, and exposes the corrected
//! codeword, information word, and error positions only through the opt-in
//! [`BinaryBchDecoder::decode`] diagnostic path. It uses the construction
//! model's ascending-degree coordinate convention rather than the DVB-T2
//! systematic layout the types above carry.
//!
//! Under `--features hip`, `BinaryBchDecoder::correct_batch_gpu` runs the
//! syndrome evaluations of a whole batch on a HIP device and the locator
//! search on the CPU, reporting the same [`BchDecodeOutcome`] values, and
//! leaving the same corrected words, as the per-word CPU path.
//!
//! # Examples
//!
//! ```
//! use gf2_coding::bch::{BchCode, BchEncoder, BchDecoder};
//! use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
//! use gf2_core::gf2m::Gf2mField;
//! use gf2_core::BitVec;
//!
//! // Create BCH(15, 11, 1) code
//! let field = Gf2mField::new(4, 0b10011);
//! let code = BchCode::new(15, 11, 1, field);
//! let encoder = BchEncoder::new(code.clone());
//! let decoder = BchDecoder::new(code);
//!
//! // Encode message
//! let msg = BitVec::ones(11);
//! let cw = encoder.encode(&msg);
//!
//! // Inject single-bit error
//! let mut received = cw.clone();
//! received.set(5, !received.get(5));
//!
//! // Decode and correct
//! let decoded = decoder.decode(&received);
//! assert_eq!(decoded, msg);
//! ```

use crate::bch::error::BchError;
use crate::bch::spec::BinaryBchCode;
use crate::error::CodeError;
use crate::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::field::extension::FieldExtension;
use gf2_core::field::{FieldPoly, FiniteField, FiniteFieldExt};
use gf2_core::gf2m::{Gf2mElement, Gf2mElement_, Gf2mField, Gf2mPoly, UintExt};
use gf2_core::BitVec;

/// Code rates for DVB-T2 standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeRate {
    Rate1_2,
    Rate3_5,
    Rate2_3,
    Rate3_4,
    Rate4_5,
    Rate5_6,
}

/// BCH (Bose-Chaudhuri-Hocquenghem) code over GF(2^m).
///
/// A BCH code is defined by its generator polynomial g(x) which has
/// consecutive powers of a primitive element α as roots.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_coding::bch::BchCode;
///
/// let field = Gf2mField::new(4, 0b10011);
/// let code = BchCode::new(15, 11, 1, field);
/// assert_eq!(code.n(), 15);
/// assert_eq!(code.k(), 11);
/// assert_eq!(code.t(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct BchCode {
    n: usize,                 // Codeword length
    k: usize,                 // Message length
    t: usize,                 // Error correction capability
    field: Gf2mField,         // Extension field GF(2^m)
    generator: Gf2mPoly,      // Generator polynomial g(x)
    designed_distance: usize, // δ = 2t + 1
}

impl BchCode {
    /// Creates a new BCH code with t-error correction capability.
    ///
    /// The generator polynomial is the LCM of minimal polynomials of
    /// α, α^2, ..., α^(2t) where α is a primitive element.
    ///
    /// # Arguments
    ///
    /// * `n` - Codeword length (must divide 2^m - 1)
    /// * `k` - Message length
    /// * `t` - Error correction capability
    /// * `field` - Extension field GF(2^m)
    ///
    /// # Panics
    ///
    /// Panics if n > 2^m - 1 or if parameters are inconsistent.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_coding::bch::BchCode;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let code = BchCode::new(15, 11, 1, field);
    /// assert_eq!(code.designed_distance(), 3); // 2*1 + 1
    /// ```
    pub fn new(n: usize, k: usize, t: usize, field: Gf2mField) -> Self {
        assert!(n > k, "Codeword length must exceed message length");
        assert!((n as u64) < field.order(), "n must divide 2^m - 1");
        assert!(t > 0, "Error correction capability must be positive");

        // Ensure field has tables (needed for primitive element)
        let field = if field.has_tables() {
            field
        } else {
            field.with_tables()
        };

        let generator = Self::construct_generator(&field, t);
        let designed_distance = 2 * t + 1;

        Self {
            n,
            k,
            t,
            field,
            generator,
            designed_distance,
        }
    }

    /// Creates a BCH code from an explicit generator polynomial.
    ///
    /// This is useful when the generator polynomial is provided by a standard
    /// (e.g., DVB-T2) rather than computed from minimal polynomials.
    ///
    /// # Arguments
    ///
    /// * `n` - Codeword length
    /// * `k` - Message length  
    /// * `t` - Error correction capability
    /// * `field` - Extension field GF(2^m)
    /// * `generator` - Pre-computed generator polynomial
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::{Gf2mField, Gf2mPoly};
    /// use gf2_coding::bch::BchCode;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// // Generator for BCH(15, 11, 1): x^4 + x + 1
    /// let g = Gf2mPoly::new(vec![
    ///     field.one(), field.one(), field.zero(), field.zero(), field.one()
    /// ]);
    /// let code = BchCode::from_generator(15, 11, 1, field, g);
    /// ```
    pub fn from_generator(
        n: usize,
        k: usize,
        t: usize,
        field: Gf2mField,
        generator: Gf2mPoly,
    ) -> Self {
        assert!(n > k, "Codeword length must exceed message length");
        assert!((n as u64) < field.order(), "n must divide 2^m - 1");
        assert!(t > 0, "Error correction capability must be positive");

        let designed_distance = 2 * t + 1;

        Self {
            n,
            k,
            t,
            field,
            generator,
            designed_distance,
        }
    }

    /// Constructs generator polynomial from consecutive roots.
    ///
    /// g(x) = LCM(m_1(x), m_2(x), ..., m_{2t}(x))
    /// where m_i(x) is the minimal polynomial of α^i.
    fn construct_generator(field: &Gf2mField, t: usize) -> Gf2mPoly {
        let alpha = field
            .primitive_element()
            .expect("Field must have primitive element");

        // Start with m_1(x)
        let mut alpha_power = alpha.clone();
        let mut g = alpha_power.minimal_polynomial();

        // Accumulate LCM of minimal polynomials for α^2, ..., α^(2t)
        for _ in 2..=(2 * t) {
            alpha_power = &alpha_power * &alpha;
            let m_i = alpha_power.minimal_polynomial();
            g = Gf2mPoly::lcm(&g, &m_i);
        }

        g
    }

    /// Returns the codeword length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Returns the message length.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Returns the error correction capability.
    pub fn t(&self) -> usize {
        self.t
    }

    /// Returns the designed minimum distance (2t + 1).
    pub fn designed_distance(&self) -> usize {
        self.designed_distance
    }

    /// Returns a reference to the generator polynomial.
    pub fn generator(&self) -> &Gf2mPoly {
        &self.generator
    }

    /// Returns a reference to the field.
    pub fn field(&self) -> &Gf2mField {
        &self.field
    }

    /// Computes the generator matrix by encoding each basis vector.
    ///
    /// For message m_i = [0,...,0,1,0,...,0] (1 at position i),
    /// row i of G is the encoding of m_i.
    ///
    /// This is expensive for large codes and is cached after first computation.
    fn compute_generator_matrix(&self) -> gf2_core::BitMatrix {
        use gf2_core::BitVec;

        let mut g = gf2_core::BitMatrix::zeros(self.k, self.n);

        // Use the encoder to generate each row
        let encoder = BchEncoder::new(self.clone());

        for i in 0..self.k {
            // Create basis vector message
            let mut msg = BitVec::new();
            msg.resize(self.k, false);
            msg.set(i, true);

            // Encode using systematic encoding
            let codeword = encoder.encode(&msg);

            // Set row i of G
            for j in 0..self.n {
                g.set(i, j, codeword.get(j));
            }
        }

        g
    }
}

impl crate::traits::GeneratorMatrixAccess for BchCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    fn generator_matrix(&self) -> gf2_core::BitMatrix {
        // Deliberately uncached: matrix access materializes on request, and
        // caching is an explicit opt-in through the canonical
        // `bch::matrix::CachedMatrices` wrapper. Consumers snapshot at
        // construction.
        self.compute_generator_matrix()
    }

    fn is_systematic(&self) -> bool {
        true // BCH codes constructed here are always systematic
    }
}

/// Systematic encoder for BCH codes.
///
/// # Systematic Encoding Convention (legacy binary surface)
///
/// **This legacy encoder produces systematic codewords in [message | parity] format:**
/// - Bits 0..(k-1): Original message bits (unchanged)
/// - Bits k..(n-1): Computed parity bits
///
/// with bit position 0 holding the highest polynomial coefficient (the
/// DVB-T2 transmission order). The canonical construction model in
/// [`crate::bch::encode`] generalizes this: the internal convention is
/// coordinate `i` ↔ coefficient of `x^i`, and user layouts — including this
/// descending order — are explicit declared mappings.
///
/// # Encoding Algorithm
///
/// 1. Convert message to polynomial m(x) over GF(2)
/// 2. Compute parity p(x) = [x^r · m(x)] mod g(x), where r = n - k
/// 3. Construct codeword polynomial c(x) = x^r · m(x) + p(x)
/// 4. Convert to bitvec in systematic form: [message | parity]
///
/// # Polynomial-to-Bit Mapping
///
/// DVB-T2 convention (used throughout this codebase):
/// - Bit position 0 corresponds to highest polynomial coefficient
/// - Bit position k-1 corresponds to coefficient of x^0
///
/// This ensures that the systematic property holds in the bitvec representation.
#[derive(Clone, Debug)]
pub struct BchEncoder {
    code: BchCode,
}

impl BchEncoder {
    /// Creates a new BCH encoder.
    pub fn new(code: BchCode) -> Self {
        Self { code }
    }

    /// Encodes a batch of messages in parallel (when parallel feature enabled).
    ///
    /// This method processes multiple messages efficiently, either sequentially
    /// or in parallel depending on feature flags and batch size.
    ///
    /// # Arguments
    ///
    /// * `messages` - Slice of messages to encode (each must have length k)
    ///
    /// # Returns
    ///
    /// Vector of codewords (each of length n)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::{BchCode, BchEncoder};
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_core::BitVec;
    ///
    /// let field = Gf2mField::new(4, 0b10011);
    /// let code = BchCode::new(15, 11, 1, field);
    /// let encoder = BchEncoder::new(code);
    ///
    /// let messages: Vec<BitVec> = (0..10)
    ///     .map(|i| {
    ///         let mut msg = BitVec::with_capacity(11);
    ///         for j in 0..11 {
    ///             msg.push_bit((i + j) % 2 == 0);
    ///         }
    ///         msg
    ///     })
    ///     .collect();
    ///
    /// let codewords = encoder.encode_batch(&messages);
    /// assert_eq!(codewords.len(), 10);
    /// assert_eq!(codewords[0].len(), 15);
    /// ```
    pub fn encode_batch(&self, messages: &[BitVec]) -> Vec<BitVec> {
        // TODO: Use ComputeBackend for parallelization
        // For now, sequential implementation
        messages.iter().map(|msg| self.encode(msg)).collect()
    }
}

impl BlockEncoder for BchEncoder {
    fn k(&self) -> usize {
        self.code.k
    }

    fn n(&self) -> usize {
        self.code.n
    }

    /// Encodes a message using systematic BCH encoding.
    ///
    /// Algorithm:
    /// 1. Multiply message polynomial m(x) by x^r (shift left by r positions)
    /// 2. Divide by generator polynomial g(x) to get remainder p(x)
    /// 3. Codeword c(x) = x^r · m(x) + p(x)
    /// 4. Rearrange to systematic form: [m(x) | p(x)]
    ///
    /// In systematic form, the codeword consists of k message bits followed by r parity bits.
    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.code.k,
            "Message must have length k = {}",
            self.code.k
        );

        let r = self.code.n - self.code.k;

        // Convert message to polynomial m(x)
        // DVB-T2 convention: bit 0 is highest coefficient, bit k-1 is x^0
        let m_coeffs: Vec<_> = (0..self.code.k)
            .rev()
            .map(|i| {
                if message.get(i) {
                    self.code.field.one()
                } else {
                    self.code.field.zero()
                }
            })
            .collect();
        let m = Gf2mPoly::new(m_coeffs);

        // Multiply by x^r to shift message left: x^r · m(x).
        // `m` normalises to the zero polynomial when the input message
        // is all-zero; use `coeff_or_zero` so the shift still produces
        // a well-formed, all-zero shifted polynomial.
        let field_zero = self.code.field.zero();
        let mut m_shifted_coeffs = vec![field_zero.clone(); r];
        for i in 0..=m.degree().unwrap_or(0) {
            m_shifted_coeffs.push(m.coeff_or_zero(i, &field_zero));
        }
        let m_shifted = Gf2mPoly::new(m_shifted_coeffs);

        // Compute parity: p(x) = remainder of (x^r · m(x)) / g(x)
        let (_, parity) = m_shifted.div_rem(&self.code.generator);

        // Codeword: c(x) = x^r · m(x) + p(x) = m_shifted + parity
        // Since we're in GF(2), addition is XOR. The codeword polynomial
        // may normalise to zero (all-zero message produces all-zero
        // codeword); `coeff_or_zero` keeps the extraction total.
        let codeword_poly = &m_shifted + &parity;

        // Convert polynomial to systematic bitvec: [message | parity]
        // DVB-T2 convention: highest coefficient first
        let mut codeword = BitVec::new();

        // First k bits: message (from polynomial degrees k-1 down to 0 of m(x))
        // In codeword_poly, these are at degrees r+k-1 down to r
        for i in (r..self.code.n).rev() {
            codeword.push_bit(codeword_poly.coeff_or_zero(i, &field_zero).is_one());
        }

        // Last r bits: parity (from polynomial degrees r-1 down to 0 of p(x))
        for i in (0..r).rev() {
            codeword.push_bit(codeword_poly.coeff_or_zero(i, &field_zero).is_one());
        }

        codeword
    }
}

/// Algebraic decoder for BCH codes.
///
/// Decoding steps:
/// 1. Compute syndromes S_i = r(α^i) for i = 1..2t
/// 2. Find error locator polynomial Λ(x) using Berlekamp-Massey
/// 3. Find error positions using Chien search
/// 4. Correct errors and extract message
#[derive(Clone, Debug)]
pub struct BchDecoder {
    code: BchCode,
}

impl BchDecoder {
    /// Creates a new BCH decoder.
    pub fn new(code: BchCode) -> Self {
        Self { code }
    }

    /// Decodes a batch of received codewords in parallel (when parallel feature enabled).
    ///
    /// This method processes multiple codewords efficiently, either sequentially
    /// or in parallel depending on feature flags and batch size.
    ///
    /// # Arguments
    ///
    /// * `received` - Slice of received codewords to decode (each must have length n)
    ///
    /// # Returns
    ///
    /// Vector of decoded messages (each of length k)
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::{BchCode, BchEncoder, BchDecoder};
    /// use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_core::BitVec;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let code = BchCode::new(15, 11, 1, field);
    /// let encoder = BchEncoder::new(code.clone());
    /// let decoder = BchDecoder::new(code);
    ///
    /// // Encode messages
    /// let messages: Vec<BitVec> = (0..10)
    ///     .map(|i| {
    ///         let mut msg = BitVec::with_capacity(11);
    ///         for j in 0..11 {
    ///             msg.push_bit((i + j) % 2 == 0);
    ///         }
    ///         msg
    ///     })
    ///     .collect();
    ///
    /// let codewords: Vec<BitVec> = messages
    ///     .iter()
    ///     .map(|msg| encoder.encode(msg))
    ///     .collect();
    ///
    /// // Decode batch
    /// let decoded = decoder.decode_batch(&codewords);
    /// assert_eq!(decoded.len(), 10);
    /// ```
    pub fn decode_batch(&self, received: &[BitVec]) -> Vec<BitVec> {
        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            received.par_iter().map(|cw| self.decode(cw)).collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            received.iter().map(|cw| self.decode(cw)).collect()
        }
    }

    /// Computes syndrome sequence S_1, S_2, ..., S_{2t}.
    ///
    /// For received polynomial r(x), syndrome S_i = r(α^i)
    /// where α is a primitive element of the field.
    ///
    /// # Panics
    ///
    /// Panics if received vector has wrong length.
    pub fn compute_syndromes(&self, received: &BitVec) -> Vec<Gf2mElement> {
        assert_eq!(
            received.len(),
            self.code.n,
            "Received vector must have length n = {}",
            self.code.n
        );

        // Convert systematic bitvec [message | parity] to polynomial representation
        // DVB-T2 convention: bit 0 is highest coefficient
        // In polynomial form, we need: c(x) = x^r·m(x) + p(x)
        // where r = n - k, degrees 0..r-1 have parity and degrees r..n-1 have message
        let mut coeffs = Vec::new();

        // Parity polynomial p(x): degrees 0..r-1
        // DVB-T2 bits k..n (parity bits), highest coefficient first
        for i in (self.code.k..self.code.n).rev() {
            coeffs.push(if received.get(i) {
                self.code.field.one()
            } else {
                self.code.field.zero()
            });
        }

        // Message polynomial m(x) shifted by x^r: degrees r..n-1
        // DVB-T2 bits 0..k (message bits), highest coefficient first
        for i in (0..self.code.k).rev() {
            coeffs.push(if received.get(i) {
                self.code.field.one()
            } else {
                self.code.field.zero()
            });
        }

        // Convert to polynomial. `FieldPoly::eval_batch` is total on the
        // zero polynomial (returns `x.zero_like()` for each point), so
        // the all-zero received vector needs no special case here.
        let r_poly = Gf2mPoly::new(coeffs);

        // Compute evaluation points α, α^2, ..., α^(2t)
        let alpha = self
            .code
            .field
            .primitive_element()
            .expect("Field must have primitive element");

        let mut eval_points = Vec::with_capacity(2 * self.code.t);
        let mut alpha_power = alpha.clone();

        for _ in 0..(2 * self.code.t) {
            eval_points.push(alpha_power.clone());
            alpha_power = &alpha_power * &alpha;
        }

        // Batch evaluate r(x) at all syndrome points.
        r_poly.eval_batch(&eval_points)
    }

    /// Packs one received frame into the GPU coefficient stream in the
    /// design-doc §3.1 order (parity bits reversed, then message bits
    /// reversed), as a little-endian packed bit stream of `n` bits.
    ///
    /// This is the bit-packed equivalent of the `coeffs` vector
    /// [`compute_syndromes`](Self::compute_syndromes) builds: coefficient index
    /// `0` is `received[n-1]` (highest parity bit), index `n-1` is
    /// `received[0]`. The returned stream is `ceil(n/64)` u64 words.
    ///
    /// This is the single source of truth for the GPU coefficient-stream
    /// layout: both [`compute_syndromes_batch_gpu`](Self::compute_syndromes_batch_gpu)
    /// and the `gpu_bch_syndrome_throughput` benchmark's repack-phase timing call
    /// it, so the measured repack cost matches the real path exactly.
    ///
    /// # Arguments
    ///
    /// * `received` — one received codeword of length `n` (the code block length).
    ///
    /// # Panics
    ///
    /// Panics if `received.len()` is not the code length `n`.
    ///
    /// # Examples
    ///
    /// Requires a HIP/ROCm GPU build (`--features hip`), so this is `no_run`; the
    /// body is gated behind a hidden `cfg` so the doctest is a no-op on the
    /// default build.
    ///
    /// ```no_run
    /// # #[cfg(feature = "hip")]
    /// # fn demo() {
    /// use gf2_coding::bch::{BchCode, BchDecoder};
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_core::BitVec;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let code = BchCode::new(15, 11, 1, field);
    /// let decoder = BchDecoder::new(code);
    ///
    /// let received = BitVec::zeros(15);
    /// let stream = decoder.pack_coeff_stream(&received);
    /// assert_eq!(stream.len(), 15usize.div_ceil(64)); // ceil(n/64) u64 words
    /// # }
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` — one pass over the `n` received bits.
    #[cfg(feature = "hip")]
    pub fn pack_coeff_stream(&self, received: &BitVec) -> Vec<u64> {
        let n = self.code.n;
        let k = self.code.k;
        assert_eq!(received.len(), n, "received must have length n = {n}");
        let words_per_frame = n.div_ceil(64);
        let mut stream = vec![0u64; words_per_frame];
        let mut idx = 0usize;
        // Parity bits first: received[n-1], received[n-2], ..., received[k].
        for i in (k..n).rev() {
            if received.get(i) {
                stream[idx >> 6] |= 1u64 << (idx & 63);
            }
            idx += 1;
        }
        // Then message bits: received[k-1], ..., received[0].
        for i in (0..k).rev() {
            if received.get(i) {
                stream[idx >> 6] |= 1u64 << (idx & 63);
            }
            idx += 1;
        }
        stream
    }

    /// Computes the syndrome sequence `S_1, ..., S_{2t}` for a batch of
    /// received words on the GPU, bit-identical to the per-frame CPU
    /// [`compute_syndromes`](Self::compute_syndromes).
    ///
    /// Drives the [`GpuBchSyndrome`](gf2_kernels_hip::GpuBchSyndrome) evaluator:
    /// it extracts the `exp` / `log` tables and the `α^1..α^(2t)` evaluation
    /// points from `self.code.field`, reorders each `received` frame into the
    /// design-doc §3.1 packed coefficient stream, runs the device Horner
    /// kernel, and rehydrates the returned u16 field values into
    /// [`Gf2mElement`]s. The CPU `compute_syndromes` is the oracle — the GPU
    /// multiply IS the uploaded CPU table, so the result is exact (design doc
    /// §5, §8). Berlekamp-Massey and Chien search remain on the CPU.
    ///
    /// Only compiled under `--features hip`.
    ///
    /// # Arguments
    ///
    /// * `received` — the batch of received codewords (each of length `n`).
    ///
    /// # Returns
    ///
    /// One `Vec<Gf2mElement>` of length `2t` per frame, in the same order as
    /// `received`. An empty input yields an empty output.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`](gf2_kernels_hip::HipError) on device allocation,
    /// memcpy, kernel launch, or synchronization failure (an OOM is the
    /// distinguished [`HipError::OutOfMemory`](gf2_kernels_hip::HipError::OutOfMemory)).
    ///
    /// # Panics
    ///
    /// Panics if the field has no precomputed tables (the DVB-T2 fields always
    /// do), if it has no primitive element, or if any `received` frame has
    /// length != `n`.
    ///
    /// # Complexity
    ///
    /// O(`batch * 2t * n`) device work; host-side cost is the per-frame
    /// coefficient repack plus the H2D of `batch * ceil(n/64)` u64 words and the
    /// D2H of `batch * 2t` u16 syndromes.
    ///
    /// # Examples
    ///
    /// Requires a HIP/ROCm GPU at runtime, so this is `no_run`. The body is
    /// compiled only under `--features hip` (the method exists only there); the
    /// hidden `cfg` gate keeps the doctest a no-op on the default build.
    ///
    /// ```no_run
    /// # #[cfg(feature = "hip")]
    /// # fn demo() {
    /// use gf2_coding::bch::{BchCode, BchEncoder, BchDecoder};
    /// use gf2_coding::traits::BlockEncoder;
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_core::BitVec;
    ///
    /// let field = Gf2mField::new(4, 0b10011).with_tables();
    /// let code = BchCode::new(15, 11, 1, field);
    /// let encoder = BchEncoder::new(code.clone());
    /// let decoder = BchDecoder::new(code);
    ///
    /// // A small batch of valid codewords.
    /// let received: Vec<BitVec> = (0..4)
    ///     .map(|i| {
    ///         let mut msg = BitVec::with_capacity(11);
    ///         for j in 0..11 {
    ///             msg.push_bit((i + j) % 2 == 0);
    ///         }
    ///         encoder.encode(&msg)
    ///     })
    ///     .collect();
    ///
    /// // GPU syndromes are byte-identical to the per-frame CPU path.
    /// let syndromes = decoder.compute_syndromes_batch_gpu(&received).unwrap();
    /// assert_eq!(syndromes.len(), 4);
    /// assert_eq!(syndromes[0].len(), 2); // 2t = 2 for t = 1
    /// # }
    /// ```
    #[cfg(feature = "hip")]
    pub fn compute_syndromes_batch_gpu(
        &self,
        received: &[BitVec],
    ) -> Result<Vec<Vec<Gf2mElement>>, gf2_kernels_hip::HipError> {
        use gf2_kernels_hip::{BchFieldTables, GpuBchSyndrome};

        let field = &self.code.field;
        let n = self.code.n;
        let two_t = 2 * self.code.t;

        if received.is_empty() {
            return Ok(Vec::new());
        }
        for (i, frame) in received.iter().enumerate() {
            assert_eq!(
                frame.len(),
                n,
                "received frame {i} has length {}, expected n = {n}",
                frame.len()
            );
        }

        // Field tables (the exact CPU exp/log tables) and the m = field degree.
        let exp = field
            .exp_table()
            .expect("BCH field must have precomputed tables")
            .to_vec();
        let log = field
            .log_table()
            .expect("BCH field must have precomputed tables")
            .to_vec();
        let tables = BchFieldTables::new(field.degree(), exp, log);

        // Evaluation points α^1..α^(2t), built identically to compute_syndromes.
        let alpha = field
            .primitive_element()
            .expect("Field must have primitive element");
        let mut eval_points: Vec<u16> = Vec::with_capacity(two_t);
        let mut alpha_power = alpha.clone();
        for _ in 0..two_t {
            eval_points.push(alpha_power.value() as u16);
            alpha_power = &alpha_power * &alpha;
        }

        // Pack every frame's coefficient stream (design-doc §3.1 order).
        let words_per_frame = n.div_ceil(64);
        let batch = received.len();
        let mut coeff_streams: Vec<u64> = Vec::with_capacity(batch * words_per_frame);
        for frame in received {
            coeff_streams.extend_from_slice(&self.pack_coeff_stream(frame));
        }

        // One device evaluator sized for this batch; default-stream evaluate.
        let mut evaluator = GpuBchSyndrome::new(&tables, &eval_points, n, self.code.t, batch, 0)?;
        let flat = evaluator.evaluate_batch(&coeff_streams, batch)?;

        // Rehydrate u16 field values into Gf2mElements, row-major per frame.
        let mut out = Vec::with_capacity(batch);
        for f in 0..batch {
            let row = &flat[f * two_t..(f + 1) * two_t];
            out.push(row.iter().map(|&v| field.element(v as u64)).collect());
        }
        Ok(out)
    }
}

impl HardDecisionDecoder for BchDecoder {
    fn decode(&self, received: &BitVec) -> BitVec {
        assert_eq!(
            received.len(),
            self.code.n,
            "Received vector must have length n = {}",
            self.code.n
        );

        // Step 1: Compute syndromes
        let syndromes = self.compute_syndromes(received);

        // Check if all syndromes are zero (no errors)
        if syndromes.iter().all(|s| s.is_zero()) {
            return self.extract_message(received);
        }

        // Step 2: Find error locator polynomial (Berlekamp-Massey)
        let lambda = self.berlekamp_massey(&syndromes);

        // Step 3: Find error positions (Chien search)
        let error_positions = self.chien_search(&lambda);

        // Step 4: Correct errors
        // Error positions are in polynomial coefficient order (degrees 0..n-1)
        // Need to convert to systematic bit positions [message | parity]
        // DVB-T2 convention: highest coefficient first
        let mut corrected = received.clone();
        let r = self.code.n - self.code.k;
        for poly_pos in error_positions {
            // Convert polynomial degree to DVB-T2 bit position
            let sys_pos = if poly_pos < r {
                // Parity bit: polynomial degree 0..r-1 (reversed)
                // degree r-1 → bit k, degree 0 → bit n-1
                self.code.n - 1 - poly_pos
            } else {
                // Message bit: polynomial degree r..n-1 (reversed)
                // degree n-1 → bit 0, degree r → bit k-1
                self.code.n - 1 - poly_pos
            };
            corrected.set(sys_pos, !corrected.get(sys_pos));
        }

        // Step 5: Extract message bits
        self.extract_message(&corrected)
    }
}

impl BchDecoder {
    /// Extracts message bits from systematic codeword.
    ///
    /// For systematic encoding [message | parity], extracts the
    /// message portion (first k bits).
    fn extract_message(&self, codeword: &BitVec) -> BitVec {
        let mut message = BitVec::new();

        for i in 0..self.code.k {
            message.push_bit(codeword.get(i));
        }

        message
    }

    /// Berlekamp-Massey algorithm for finding error locator polynomial.
    ///
    /// Given syndrome sequence S = [S_1, S_2, ..., S_{2t}],
    /// finds minimal polynomial Λ(x) such that:
    ///   S_n + Λ_1·S_{n-1} + ... + Λ_L·S_{n-L} = 0
    /// for all n ≥ L, where L is the number of errors.
    ///
    /// The algorithm iteratively builds Λ(x) by detecting discrepancies
    /// and updating the polynomial to satisfy syndrome constraints.
    ///
    /// # Returns
    ///
    /// Error locator polynomial Λ(x) where roots indicate error positions.
    pub fn berlekamp_massey(&self, syndromes: &[Gf2mElement]) -> Gf2mPoly {
        let field = &self.code.field;

        // Initialize: Λ(x) = 1, B(x) = 1
        let mut lambda = Gf2mPoly::constant(field.one());
        let mut b = Gf2mPoly::constant(field.one());

        let mut l = 0; // Current error locator degree
        let mut m = 1; // Shift amount for B(x)

        for n in 0..syndromes.len() {
            // Compute discrepancy δ_n = S_n + Σ(Λ_i · S_{n-i})
            let mut delta = syndromes[n].clone();

            for i in 1..=l {
                if i <= lambda.degree().unwrap_or(0) && n >= i {
                    let lambda_i = lambda.coeff(i);
                    let s_n_minus_i = &syndromes[n - i];
                    delta = &delta + &(&lambda_i * s_n_minus_i);
                }
            }

            if delta.is_zero() {
                // No correction needed, just increment shift
                m += 1;
            } else {
                // Correction needed
                let t = lambda.clone();

                // Λ(x) ← Λ(x) - δ · x^m · B(x)
                // In GF(2), subtraction is XOR, same as addition
                let mut b_shifted_coeffs = vec![field.zero(); m];
                for i in 0..=b.degree().unwrap_or(0) {
                    b_shifted_coeffs.push(&delta * &b.coeff(i));
                }
                let b_term = Gf2mPoly::new(b_shifted_coeffs);

                lambda = &lambda + &b_term;

                if 2 * l <= n {
                    // Update auxiliary polynomial
                    l = n + 1 - l;

                    if let Some(delta_inv) = delta.inverse() {
                        let mut new_b_coeffs = Vec::new();
                        for i in 0..=t.degree().unwrap_or(0) {
                            new_b_coeffs.push(&t.coeff(i) * &delta_inv);
                        }
                        b = Gf2mPoly::new(new_b_coeffs);
                    } else {
                        // If delta has no inverse, keep B as is
                        b = t;
                    }
                    m = 1;
                } else {
                    m += 1;
                }
            }
        }

        lambda
    }

    /// Chien search for finding error positions.
    ///
    /// Evaluates error locator polynomial Λ(x) at all field elements
    /// to find positions where Λ(α^(-i)) = 0, which indicate errors at position i.
    ///
    /// Uses incremental evaluation for efficiency: if Λ(x) = Σ λ_j·x^j,
    /// then Λ(α^(-i-1)) is computed from Λ(α^(-i)) by multiplying each
    /// coefficient λ_j by α^(-j).
    ///
    /// # Returns
    ///
    /// Vector of error positions (indices where errors occurred).
    pub fn chien_search(&self, lambda: &Gf2mPoly) -> Vec<usize> {
        let field = &self.code.field;
        let alpha = field
            .primitive_element()
            .expect("Field must have primitive element");

        let degree = lambda.degree().unwrap_or(0);
        if degree == 0 {
            // Λ(x) = 1 means no errors
            return Vec::new();
        }

        let mut error_positions = Vec::new();

        // Precompute α^(-j) for each coefficient position j
        let alpha_inv = alpha
            .inverse()
            .expect("Primitive element must be invertible");

        let mut alpha_powers = vec![field.one()]; // α^0 = 1
        let mut alpha_power = alpha_inv.clone();
        for _ in 1..=degree {
            alpha_powers.push(alpha_power.clone());
            alpha_power = &alpha_power * &alpha_inv;
        }

        // Initialize coefficients for Λ(α^0) = Λ(1)
        let mut coeffs: Vec<Gf2mElement> = (0..=degree).map(|i| lambda.coeff(i)).collect();

        // Test each position i = 0, 1, ..., n-1
        for i in 0..self.code.n {
            // Evaluate Λ(α^(-i)) by summing all coefficients
            let mut eval = field.zero();
            for coeff in &coeffs {
                eval = &eval + coeff;
            }

            if eval.is_zero() {
                error_positions.push(i);
            }

            // Update coefficients for next iteration: coeffs[j] ← coeffs[j] · α^(-j)
            for (j, coeff) in coeffs.iter_mut().enumerate() {
                *coeff = &*coeff * &alpha_powers[j];
            }
        }

        error_positions
    }
}

// ---------------------------------------------------------------------------
// Hardened binary decoder over the canonical construction model
// ---------------------------------------------------------------------------

/// What one bounded-distance decode established about a received word.
///
/// The three variants are exhaustive and mutually exclusive. They describe
/// the *procedure's* result, never the channel: see
/// [`BinaryBchDecoder`] for the exact guarantee each one carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BchDecodeOutcome {
    /// The received word is a codeword, so no coordinate was changed.
    NoErrors,
    /// A candidate correction was found and verified: flipping these
    /// coordinates produces a codeword.
    Corrected {
        /// Number of flipped coordinates, at most the correction radius.
        count: usize,
    },
    /// The procedure found no verified correction.
    ///
    /// This is a statement about the bounded-distance procedure, not about
    /// the transmitted word: the received word may still be a corrupted
    /// codeword that lies beyond the correction radius.
    Uncorrectable,
}

impl BchDecodeOutcome {
    /// Returns the number of corrected coordinates, or `None` when no
    /// verified correction was found.
    pub fn corrected_count(self) -> Option<usize> {
        match self {
            Self::NoErrors => Some(0),
            Self::Corrected { count } => Some(count),
            Self::Uncorrectable => None,
        }
    }
}

/// The reusable scratch space one [`BinaryBchDecoder`] needs per word.
///
/// Every buffer is sized once, by [`BinaryBchDecoder::workspace`], from the
/// code's syndrome count and correction radius, and is only ever overwritten
/// in place afterwards; none is ever pushed to, resized, or replaced. Together
/// with two further facts that gives
/// [`correct_in_place`](BinaryBchDecoder::correct_in_place) its
/// no-heap-allocation contract: the decoder itself precomputes its evaluation
/// points and root powers at construction and holds no interior mutability,
/// and $\mathrm{GF}(2^m)$ element arithmetic produces values that share their
/// field by reference count rather than by allocating. The buffers here are
/// therefore the only heap a decode could want, and the caller owns their one
/// allocation.
///
/// A workspace belongs to the decoder that produced it. Every workspace
/// carries a fingerprint of the code it was built for, and the decoder
/// rejects a foreign workspace with a typed error even when its buffer
/// lengths happen to match.
#[derive(Clone, Debug)]
pub struct BchDecodeWorkspace<V: UintExt = u64> {
    /// Fingerprint of the producing decoder's code, checked on every decode.
    code_stamp: u64,
    /// Syndrome values, one per evaluation exponent of the decoder.
    syndromes: Vec<Gf2mElement_<V>>,
    /// Error-locator coefficients in ascending degree order.
    locator: Vec<Gf2mElement_<V>>,
    /// The Berlekamp-Massey auxiliary polynomial `B(x)`.
    previous: Vec<Gf2mElement_<V>>,
    /// A copy of the locator taken before each Berlekamp-Massey update.
    scratch: Vec<Gf2mElement_<V>>,
    /// Running locator-coefficient values for the Chien search.
    chien: Vec<Gf2mElement_<V>>,
    /// Ascending error positions of the last candidate correction.
    positions: Vec<usize>,
}

/// A bounded-distance decoder for a canonical binary BCH code.
///
/// The decoder borrows the constructed
/// [`BinaryBchCode`](crate::bch::spec::BinaryBchCode) and reads its length,
/// order-$n$ root, defining set, and witnessed distance bound; it derives
/// nothing the construction already fixed. Coordinate $i$ of a received word
/// is the coefficient of $x^i$, matching the construction model's convention.
///
/// # Guarantee
///
/// Write $t$ for [`correction_radius`](crate::bch::spec::BchCode::correction_radius),
/// the radius the code's witnessed bound implies.
///
/// - At most $t$ errors: the decode returns [`BchDecodeOutcome::NoErrors`] or
///   [`BchDecodeOutcome::Corrected`], and the corrected word is the
///   transmitted codeword. Bounded-distance decoding is exact inside the
///   radius because a codeword within distance $t$ of the received word is
///   unique.
/// - More than $t$ errors: the decode returns either
///   [`BchDecodeOutcome::Uncorrectable`], or
///   [`BchDecodeOutcome::Corrected`] naming a *different* codeword that
///   happens to lie within distance $t$ of the received word. That
///   miscorrection is inherent to bounded-distance decoding, and this decoder
///   cannot distinguish it from a genuine correction: both produce a verified
///   codeword. The reported outcome is always sound about the *word* it
///   produces and never a claim about the transmitted word.
/// - [`BchDecodeOutcome::Uncorrectable`] therefore means "this procedure
///   found no verified correction", not "the received word is beyond
///   repair".
///
/// Verification is what makes the `Corrected` claim checkable: a candidate is
/// accepted only after the corrected word's syndrome is recomputed and found
/// zero, which is equivalent to codeword membership (see
/// [`workspace`](Self::workspace) for why the evaluated exponents suffice).
/// No received word makes a decode panic.
///
/// # Paths
///
/// [`correct_in_place`](Self::correct_in_place) is the fast path: it corrects
/// the caller's packed storage and returns status and count, with no heap
/// allocation. [`decode`](Self::decode) is the opt-in diagnostic path: it may
/// allocate, and returns the corrected codeword, the information word, the
/// sorted error positions, and the count. Under `--features hip`,
/// `correct_batch_gpu` is the fast path over a whole batch, with the syndrome
/// evaluations on a device; it reports the same outcomes and leaves the same
/// corrected words.
///
/// # Examples
///
/// ```
/// use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
/// use gf2_coding::bch::{BchDecodeOutcome, BinaryBchDecoder};
/// use gf2_core::field::extension::BinaryPrimeExt;
/// use gf2_core::field::FiniteField;
/// use gf2_core::gf2m::Gf2mField;
/// use gf2_core::BitVec;
///
/// // BCH(15, 7) with a witnessed run of four consecutive roots, so t = 2.
/// let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())?;
/// let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
///     extension,
///     designed_distance: DesignedDistance::try_from(5)?,
/// })?;
/// assert_eq!(code.correction_radius(), 2);
///
/// // The generator polynomial is itself a codeword, and coordinate i carries
/// // the coefficient of x^i.
/// let mut word = BitVec::zeros(code.n());
/// for i in 0..=code.generator().degree().expect("a nonzero generator") {
///     word.set(i, code.generator().coeff(i).is_one());
/// }
///
/// let decoder = BinaryBchDecoder::new(&code);
/// let mut workspace = decoder.workspace();
///
/// // Two errors are inside the radius, so the fast path recovers the word.
/// word.set(0, !word.get(0));
/// word.set(9, !word.get(9));
/// let outcome = decoder.correct_in_place(&mut word, &mut workspace)?;
/// assert_eq!(outcome, BchDecodeOutcome::Corrected { count: 2 });
///
/// // The diagnostic path names the evidence; the repaired word is clean.
/// let report = decoder.decode(&word)?;
/// assert_eq!(report.outcome(), BchDecodeOutcome::NoErrors);
/// assert!(report.error_positions().is_empty());
/// # Ok::<(), gf2_coding::bch::error::BchError>(())
/// ```
#[derive(Clone, Debug)]
pub struct BinaryBchDecoder<'code, V: UintExt = u64> {
    code: &'code BinaryBchCode<V>,
    /// Evaluation points `β^e`, the witnessed consecutive run first.
    syndrome_points: Box<[Gf2mElement_<V>]>,
    /// How many leading syndrome points form the consecutive run.
    run_length: usize,
    /// The correction radius the code's witnessed bound implies.
    radius: usize,
    /// `β^{-j}` for `j` up to the radius, for the Chien update step.
    inverse_root_powers: Box<[Gf2mElement_<V>]>,
    zero: Gf2mElement_<V>,
    one: Gf2mElement_<V>,
}

/// Structured evidence from one [`BinaryBchDecoder::decode`].
///
/// The report carries the [`outcome`](Self::outcome) the fast path would have
/// returned, plus the words and coordinates behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BchDecodeReport {
    outcome: BchDecodeOutcome,
    recovered: Option<RecoveredWord>,
    error_positions: Vec<usize>,
}

/// The verified codeword a decode produced, with its information word.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecoveredWord {
    codeword: BitVec,
    message: BitVec,
}

impl BchDecodeReport {
    /// Returns the decode outcome.
    pub fn outcome(&self) -> BchDecodeOutcome {
        self.outcome
    }

    /// Returns the verified codeword, or `None` when the outcome is
    /// [`BchDecodeOutcome::Uncorrectable`].
    pub fn codeword(&self) -> Option<&BitVec> {
        self.recovered.as_ref().map(|word| &word.codeword)
    }

    /// Returns the information word `m` with `c = m * g`, or `None` when the
    /// outcome is [`BchDecodeOutcome::Uncorrectable`].
    ///
    /// The construction model defines a codeword as the coefficient vector of
    /// a multiple of the generator, so dividing by the generator is the
    /// inverse of that encoding. A systematic user layout is a separate
    /// coordinate map above this convention.
    pub fn message(&self) -> Option<&BitVec> {
        self.recovered.as_ref().map(|word| &word.message)
    }

    /// Returns the corrected coordinates in ascending order, empty when the
    /// decode changed nothing.
    pub fn error_positions(&self) -> &[usize] {
        &self.error_positions
    }

    /// Returns the number of corrected coordinates, or `None` when no
    /// verified correction was found.
    pub fn error_count(&self) -> Option<usize> {
        self.outcome.corrected_count()
    }
}

impl<'code, V: UintExt> BinaryBchDecoder<'code, V> {
    /// Builds a decoder for `code`.
    ///
    /// The evaluation points, their inverses, and the field constants are
    /// derived once here so that decoding a word touches no allocator.
    ///
    /// # Panics
    ///
    /// Panics if the code's order-$n$ root is not invertible, which a
    /// completed [`construct`](crate::bch::spec::BchCode::construct) rules
    /// out; reaching it means an internal invariant broke.
    ///
    /// # Complexity
    ///
    /// $O(n)$ modular exponent bookkeeping plus one exponentiation per
    /// evaluation point.
    pub fn new(code: &'code BinaryBchCode<V>) -> Self {
        let extension = code.extension();
        let root = code.root();
        let length = code.n();
        let radius = code.correction_radius();
        let bound = code.distance_bound();
        let run_length = bound.consecutive_root_count();
        let run_start = bound.first_root().map_or(0, |exponent| exponent.get());

        // The witnessed consecutive run comes first: Berlekamp-Massey reads
        // exactly those syndromes, in that order. The remaining exponents are
        // the defining-set members the run does not already imply. Over
        // GF(2), r(β^e) = 0 forces r(β^{2e}) = 0, so evaluating one exponent
        // per Frobenius orbit decides membership in the whole defining set,
        // and a zero syndrome is therefore equivalent to being a codeword.
        // A constructed length is positive, so the modulus below is nonzero.
        let modulus = u128::from(length as u64);
        let mut exponents: Vec<u64> = Vec::with_capacity(run_length + code.defining_set().len());
        let mut implied = vec![false; length];
        for offset in 0..run_length {
            let exponent = ((u128::from(run_start) + offset as u128) % modulus) as u64;
            exponents.push(exponent);
            mark_frobenius_orbit(&mut implied, exponent, modulus);
        }
        for exponent in code.defining_set() {
            let exponent = exponent.get();
            if !implied[exponent as usize] {
                exponents.push(exponent);
                mark_frobenius_orbit(&mut implied, exponent, modulus);
            }
        }

        let syndrome_points: Box<[Gf2mElement_<V>]> = exponents
            .iter()
            .map(|&exponent| root.pow(exponent))
            .collect();

        let inverse = root
            .inv()
            .expect("a constructed code's order-n root is nonzero and therefore invertible");
        let mut inverse_root_powers = Vec::with_capacity(radius + 1);
        let mut power = extension.ext_one();
        for _ in 0..=radius {
            inverse_root_powers.push(power.clone());
            power = &power * &inverse;
        }

        Self {
            code,
            syndrome_points,
            run_length,
            radius,
            inverse_root_powers: inverse_root_powers.into_boxed_slice(),
            zero: extension.ext_zero(),
            one: extension.ext_one(),
        }
    }

    /// Returns the code this decoder was built for.
    pub fn code(&self) -> &'code BinaryBchCode<V> {
        self.code
    }

    /// Returns the correction radius the code's witnessed bound implies.
    pub fn correction_radius(&self) -> usize {
        self.radius
    }

    /// Allocates the scratch space one decode needs.
    ///
    /// Build this once and pass the same value to every
    /// [`correct_in_place`](Self::correct_in_place) call: the buffers are
    /// sized here for the worst case and are only overwritten afterwards, so
    /// the per-word path performs no allocation.
    ///
    /// # Complexity
    ///
    /// One allocation per buffer, together $O(r + t)$ field elements for a
    /// witnessed run of length $r$ and radius $t$.
    pub fn workspace(&self) -> BchDecodeWorkspace<V> {
        BchDecodeWorkspace {
            code_stamp: self.code_stamp(),
            syndromes: vec![self.zero.clone(); self.syndrome_points.len()],
            locator: vec![self.zero.clone(); self.run_length + 1],
            previous: vec![self.zero.clone(); self.run_length + 1],
            scratch: vec![self.zero.clone(); self.run_length + 1],
            chien: vec![self.zero.clone(); self.radius + 1],
            positions: Vec::with_capacity(self.radius),
        }
    }

    /// Corrects `received` in place and reports the outcome.
    ///
    /// This is the fast path: it returns status and count, performs no heap
    /// allocation on any path (see [`BchDecodeWorkspace`]), and leaves
    /// `received` untouched unless it returns
    /// [`BchDecodeOutcome::Corrected`]. A candidate correction is applied,
    /// then verified by recomputing the corrected word's syndrome; a
    /// candidate that fails verification is rolled back and reported as
    /// [`BchDecodeOutcome::Uncorrectable`].
    ///
    /// See the [type documentation](Self) for what each outcome guarantees.
    ///
    /// # Errors
    ///
    /// [`BchError::WorkspaceMismatch`] for a workspace built by a decoder
    /// for a different code, and [`BchError::Decode`] wrapping
    /// [`CodeError::BufferLengthMismatch`](crate::error::CodeError::BufferLengthMismatch)
    /// when `received` is not `n` coordinates long. Both are rejected before
    /// any decoding, so no partial correction is observable.
    ///
    /// # Complexity
    ///
    /// $O(sn)$ field multiplications for $s$ syndrome evaluations, plus
    /// $O(tn)$ for the Chien search and $O(r^2)$ for Berlekamp-Massey.
    pub fn correct_in_place(
        &self,
        received: &mut BitVec,
        workspace: &mut BchDecodeWorkspace<V>,
    ) -> Result<BchDecodeOutcome, BchError> {
        self.validate(received.len(), workspace)?;

        self.evaluate_syndromes(received, &mut workspace.syndromes);
        if workspace.syndromes.iter().all(|value| value.is_zero()) {
            return Ok(BchDecodeOutcome::NoErrors);
        }
        Ok(self.correct_nonzero_syndrome(received, workspace))
    }

    /// Decodes `received` and returns the full evidence.
    ///
    /// This is the opt-in diagnostic path. It allocates its own workspace and
    /// working copy, so it neither borrows nor disturbs the caller's storage;
    /// use [`correct_in_place`](Self::correct_in_place) when only the status
    /// and count are wanted.
    ///
    /// # Errors
    ///
    /// [`BchError::Decode`] when `received` is not `n` coordinates long.
    ///
    /// # Complexity
    ///
    /// That of [`correct_in_place`](Self::correct_in_place), plus one
    /// polynomial division by the generator to recover the information word.
    pub fn decode(&self, received: &BitVec) -> Result<BchDecodeReport, BchError> {
        let mut workspace = self.workspace();
        let mut corrected = received.clone();
        let outcome = self.correct_in_place(&mut corrected, &mut workspace)?;

        if outcome == BchDecodeOutcome::Uncorrectable {
            return Ok(BchDecodeReport {
                outcome,
                recovered: None,
                error_positions: Vec::new(),
            });
        }

        let message = self.extract_message(&corrected);
        let error_positions = match outcome {
            BchDecodeOutcome::Corrected { .. } => workspace.positions.clone(),
            _ => Vec::new(),
        };
        Ok(BchDecodeReport {
            outcome,
            recovered: Some(RecoveredWord {
                codeword: corrected,
                message,
            }),
            error_positions,
        })
    }

    /// Fingerprints this decoder's code: length, radius, and the exact
    /// syndrome evaluation points, which encode the root, the witnessed run,
    /// and the field presentation together (FNV-1a).
    fn code_stamp(&self) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(PRIME);
        };
        mix(self.code.n() as u64);
        mix(self.run_length as u64);
        mix(self.radius as u64);
        // The field presentation itself: raw point values coincide across
        // presentations for low-degree points (x and x^2 reduce nowhere), so
        // the modulus and degree must enter the stamp.
        let field = self.zero.field();
        mix(field.degree() as u64);
        mix(field.primitive_polynomial().as_u64_truncated());
        for point in self.syndrome_points.iter() {
            mix(point.value().as_u64_truncated());
        }
        hash
    }

    /// Rejects a received word or workspace that does not match this code.
    fn validate(&self, length: usize, workspace: &BchDecodeWorkspace<V>) -> Result<(), BchError> {
        if workspace.code_stamp != self.code_stamp() {
            return Err(BchError::WorkspaceMismatch {
                expected_stamp: self.code_stamp(),
                actual_stamp: workspace.code_stamp,
            });
        }
        let mismatch = |expected: usize, actual: usize| {
            BchError::Decode(CodeError::BufferLengthMismatch { expected, actual })
        };
        if length != self.code.n() {
            return Err(mismatch(self.code.n(), length));
        }
        if workspace.syndromes.len() != self.syndrome_points.len() {
            return Err(mismatch(
                self.syndrome_points.len(),
                workspace.syndromes.len(),
            ));
        }
        if workspace.locator.len() != self.run_length + 1 {
            return Err(mismatch(self.run_length + 1, workspace.locator.len()));
        }
        if workspace.chien.len() != self.radius + 1 {
            return Err(mismatch(self.radius + 1, workspace.chien.len()));
        }
        Ok(())
    }

    /// Runs the locator search and verification for a nonzero syndrome.
    fn correct_nonzero_syndrome(
        &self,
        received: &mut BitVec,
        workspace: &mut BchDecodeWorkspace<V>,
    ) -> BchDecodeOutcome {
        if !self.locate_candidate(workspace) {
            return BchDecodeOutcome::Uncorrectable;
        }

        flip(received, &workspace.positions);
        self.evaluate_syndromes(received, &mut workspace.syndromes);
        if workspace.syndromes.iter().all(|value| value.is_zero()) {
            BchDecodeOutcome::Corrected {
                count: workspace.positions.len(),
            }
        } else {
            flip(received, &workspace.positions);
            BchDecodeOutcome::Uncorrectable
        }
    }

    /// Searches for a candidate error pattern behind a nonzero syndrome,
    /// leaving its ascending coordinates in `workspace.positions`.
    ///
    /// Returns `false` when the syndrome admits no well-defined candidate
    /// inside the radius, which is [`BchDecodeOutcome::Uncorrectable`] before
    /// any verification. A `true` result is a candidate, not yet a decision:
    /// only recomputing the corrected word's syndrome settles that.
    fn locate_candidate(&self, workspace: &mut BchDecodeWorkspace<V>) -> bool {
        let Some(degree) = self.berlekamp_massey(workspace) else {
            return false;
        };
        if degree == 0 || degree > self.radius {
            return false;
        }
        self.chien_search(workspace, degree)
    }

    /// Writes `word(β^e)` for every evaluation exponent into `out`.
    fn evaluate_syndromes(&self, word: &BitVec, out: &mut [Gf2mElement_<V>]) {
        let Some(top) = word.find_last_set() else {
            for slot in out.iter_mut() {
                *slot = self.zero.clone();
            }
            return;
        };
        for (slot, point) in out.iter_mut().zip(self.syndrome_points.iter()) {
            // Horner from the highest set coordinate downwards.
            let mut accumulator = self.zero.clone();
            for index in (0..=top).rev() {
                accumulator = &accumulator * point;
                if word.get(index) {
                    accumulator = &accumulator + &self.one;
                }
            }
            *slot = accumulator;
        }
    }

    /// Finds the error-locator polynomial of the run syndromes and returns its
    /// degree, or `None` when the recurrence outgrows the workspace.
    ///
    /// The locator is left in `workspace.locator` in ascending degree order,
    /// with a constant term of one.
    fn berlekamp_massey(&self, workspace: &mut BchDecodeWorkspace<V>) -> Option<usize> {
        let BchDecodeWorkspace {
            syndromes,
            locator,
            previous,
            scratch,
            ..
        } = workspace;
        let capacity = locator.len();

        for coefficient in locator.iter_mut().chain(previous.iter_mut()) {
            *coefficient = self.zero.clone();
        }
        locator[0] = self.one.clone();
        previous[0] = self.one.clone();
        let mut locator_len = 1usize;
        let mut previous_len = 1usize;
        let mut order = 0usize;
        let mut shift = 1usize;

        for index in 0..self.run_length {
            let mut discrepancy = syndromes[index].clone();
            for offset in 1..=order.min(locator_len.saturating_sub(1)).min(index) {
                discrepancy = &discrepancy + &(&locator[offset] * &syndromes[index - offset]);
            }
            if discrepancy.is_zero() {
                shift += 1;
                continue;
            }

            let updated_len = locator_len.max(previous_len + shift);
            if updated_len > capacity {
                // Unreachable for a genuine Berlekamp-Massey trace, where the
                // locator degree stays below the syndrome count; bailing out
                // keeps a corrupt trace from indexing out of bounds.
                return None;
            }
            scratch[..locator_len].clone_from_slice(&locator[..locator_len]);
            let scratch_len = locator_len;

            // Λ(x) ← Λ(x) + δ · x^shift · B(x); in GF(2^m) that is also the
            // subtraction the algorithm calls for.
            for (offset, coefficient) in previous[..previous_len].iter().enumerate() {
                let target = offset + shift;
                let updated = &locator[target] + &(&discrepancy * coefficient);
                locator[target] = updated;
            }
            locator_len = updated_len;

            if 2 * order <= index {
                order = index + 1 - order;
                let inverse = discrepancy.inv()?;
                for (slot, coefficient) in previous.iter_mut().zip(scratch[..scratch_len].iter()) {
                    *slot = coefficient * &inverse;
                }
                for slot in previous[scratch_len..].iter_mut() {
                    *slot = self.zero.clone();
                }
                previous_len = scratch_len;
                shift = 1;
            } else {
                shift += 1;
            }
        }

        while locator_len > 1 && locator[locator_len - 1].is_zero() {
            locator_len -= 1;
        }
        Some(locator_len - 1)
    }

    /// Collects the ascending coordinates where the locator vanishes.
    ///
    /// Returns `true` when the locator has exactly `degree` distinct roots
    /// among the code's coordinates, which is the condition for the candidate
    /// error pattern to be well defined.
    fn chien_search(&self, workspace: &mut BchDecodeWorkspace<V>, degree: usize) -> bool {
        let BchDecodeWorkspace {
            locator,
            chien,
            positions,
            ..
        } = workspace;
        positions.clear();
        chien[..=degree].clone_from_slice(&locator[..=degree]);

        for position in 0..self.code.n() {
            // Λ(β^{-position}) is the running sum of the scaled coefficients.
            let mut value = self.zero.clone();
            for coefficient in chien[..=degree].iter() {
                value = &value + coefficient;
            }
            if value.is_zero() {
                if positions.len() == degree {
                    return false;
                }
                positions.push(position);
            }
            for (coefficient, power) in chien[..=degree]
                .iter_mut()
                .zip(self.inverse_root_powers.iter())
            {
                *coefficient = &*coefficient * power;
            }
        }

        positions.len() == degree
    }

    /// Returns the information word `m` of the codeword `c`, with `c = m * g`.
    fn extract_message(&self, codeword: &BitVec) -> BitVec {
        let extension = self.code.extension();
        let zero = extension.base_zero();
        let one = extension.base_one();
        let coefficients = (0..self.code.n())
            .map(|index| if codeword.get(index) { one } else { zero })
            .collect();
        let (quotient, _) = FieldPoly::new(coefficients).div_rem(self.code.generator());

        let mut message = BitVec::zeros(self.code.k());
        for index in 0..self.code.k() {
            if quotient.coeff_or_zero(index, &zero).is_one() {
                message.set(index, true);
            }
        }
        message
    }
}

// ---------------------------------------------------------------------------
// GPU-assisted decoding over the canonical model
// ---------------------------------------------------------------------------

/// GPU-assisted batch decoding, compiled only under `--features hip`.
///
/// The device evaluates syndromes; the locator search, the verification
/// decision, and the correction stay on the CPU. Syndrome evaluation is the
/// part that scales with `n` per point, and the device multiply *is* the
/// uploaded CPU `exp`/`log` table, so a device syndrome equals the CPU
/// syndrome exactly — every outcome below is the one
/// [`correct_in_place`](BinaryBchDecoder::correct_in_place) reports for the
/// same word.
#[cfg(feature = "hip")]
impl<V: UintExt> BinaryBchDecoder<'_, V> {
    /// Evaluates every syndrome point of `frames` on the HIP device.
    ///
    /// Returns the values row-major, one row of `self.syndrome_points.len()`
    /// per frame in the decoder's own evaluation order, so a row holds exactly
    /// what [`evaluate_syndromes`](Self::evaluate_syndromes) writes for that
    /// word. Callers guarantee a nonempty point set.
    fn syndromes_on_device(
        &self,
        frames: &[&BitVec],
    ) -> Result<Vec<Gf2mElement_<V>>, gf2_kernels_hip::HipError> {
        use gf2_kernels_hip::{BchFieldTables, GpuBchSyndrome};

        let length = self.code.n();
        let points = self.syndrome_points.len();
        let words_per_frame = length.div_ceil(64);
        let field = self.code.extension().field();

        // The exact CPU tables, so the device multiply is the CPU multiply.
        let exp = field
            .exp_table()
            .expect("a BCH splitting field carries precomputed tables")
            .to_vec();
        let log = field
            .log_table()
            .expect("a BCH splitting field carries precomputed tables")
            .to_vec();
        let tables = BchFieldTables::new(field.degree(), exp, log);

        // The evaluator takes its points in pairs, the `2t` of a narrow-sense
        // code. A defining set whose orbit representatives make the count odd
        // is padded with a repeat of the last point, whose column is dropped
        // below; the repeat changes no other value.
        let mut device_points: Vec<u16> = self
            .syndrome_points
            .iter()
            .map(|point| point.value().as_u64_truncated() as u16)
            .collect();
        if points % 2 == 1 {
            device_points.push(device_points[points - 1]);
        }
        let stride = device_points.len();

        // Coordinate `i` is the coefficient of `x^i` in both the construction
        // model and the device coefficient stream, so a canonical word is
        // already packed: its words are the stream, with no host-side reorder.
        let mut streams: Vec<u64> = Vec::with_capacity(frames.len() * words_per_frame);
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(
                frame.len(),
                length,
                "frame {index} has length {}, expected n = {length}",
                frame.len()
            );
            streams.extend_from_slice(&frame.words()[..words_per_frame]);
        }

        let mut evaluator =
            GpuBchSyndrome::new(&tables, &device_points, length, stride / 2, frames.len(), 0)?;
        let evaluated = evaluator.evaluate_batch(&streams, frames.len())?;

        let mut out = Vec::with_capacity(frames.len() * points);
        for row in evaluated.chunks_exact(stride) {
            out.extend(
                row[..points]
                    .iter()
                    .map(|&value| field.element(V::from_u16(value))),
            );
        }
        Ok(out)
    }

    /// Evaluates the syndromes of a batch of received words on the GPU.
    ///
    /// Each returned row holds one value per evaluation point of this decoder,
    /// in its evaluation order: the witnessed consecutive run first, then one
    /// representative per remaining Frobenius orbit of the defining set. An
    /// all-zero row is therefore equivalent to the word being a codeword, the
    /// same equivalence the CPU path decides on.
    ///
    /// # Arguments
    ///
    /// * `received` — the batch of received words, each of `n` coordinates.
    ///
    /// # Returns
    ///
    /// One row per frame, in input order. An empty batch yields no rows, and a
    /// code with an empty defining set yields empty rows.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`](gf2_kernels_hip::HipError) on device allocation,
    /// transfer, launch, or synchronization failure; an exhausted device is the
    /// distinguished [`HipError::OutOfMemory`](gf2_kernels_hip::HipError::OutOfMemory).
    ///
    /// # Panics
    ///
    /// Panics if a frame is not `n` coordinates long, or if the splitting
    /// field carries no precomputed tables, which a constructed code rules out.
    ///
    /// # Complexity
    ///
    /// $O(bsn)$ device work for a batch of $b$ frames and $s$ evaluation
    /// points, plus the host transfer of $b \lceil n/64 \rceil$ words up and
    /// $bs$ values back. The field tables upload once per call.
    pub fn compute_syndromes_batch_gpu(
        &self,
        received: &[BitVec],
    ) -> Result<Vec<Vec<Gf2mElement_<V>>>, gf2_kernels_hip::HipError> {
        let points = self.syndrome_points.len();
        if received.is_empty() || points == 0 {
            return Ok(vec![Vec::new(); received.len()]);
        }
        let frames: Vec<&BitVec> = received.iter().collect();
        let evaluated = self.syndromes_on_device(&frames)?;
        Ok(evaluated.chunks_exact(points).map(<[_]>::to_vec).collect())
    }

    /// Corrects a batch of received words in place and reports one outcome per
    /// word.
    ///
    /// This is the batch counterpart of
    /// [`correct_in_place`](Self::correct_in_place) and carries its contract
    /// word for word, verification included: a candidate correction is applied,
    /// then checked by recomputing the corrected word's syndrome on the device,
    /// and a candidate that fails that check is rolled back and reported as
    /// [`BchDecodeOutcome::Uncorrectable`]. Every entry of `received` is left
    /// exactly as the per-word CPU path would leave it. See the
    /// [type documentation](Self) for what each outcome guarantees.
    ///
    /// Unlike the per-word path this one allocates: it owns one workspace for
    /// the whole batch, holds the coordinates of each applied candidate until
    /// verification, and stages the batch's coefficient streams.
    ///
    /// # Arguments
    ///
    /// * `received` — the batch of received words, each of `n` coordinates,
    ///   corrected in place.
    ///
    /// # Returns
    ///
    /// One outcome per frame, in input order.
    ///
    /// # Errors
    ///
    /// Returns [`HipError`](gf2_kernels_hip::HipError) for a device failure in
    /// either syndrome pass that
    /// [`is_recoverable`](gf2_kernels_hip::HipError::is_recoverable) rejects —
    /// a missing device, an unreadable kernel blob, or a raw driver status —
    /// with every entry of `received` holding exactly the word the caller
    /// passed. A recoverable failure, an exhausted device or an arch this
    /// build carries no kernel blob for, returns no error at all: the batch is
    /// restored and decoded on the CPU path, the safe fallback of
    /// `@/inv/accelerator-safe-fallback`, whose outcomes this path reproduces
    /// anyway. No path leaves an altered, unverified word behind, so a caller
    /// that retries decodes the words it started with.
    ///
    /// # Panics
    ///
    /// Panics under the same conditions as
    /// [`compute_syndromes_batch_gpu`](Self::compute_syndromes_batch_gpu).
    ///
    /// # Complexity
    ///
    /// The device syndrome work of
    /// [`compute_syndromes_batch_gpu`](Self::compute_syndromes_batch_gpu) over
    /// the batch and again over its candidates, plus the per-word host cost of
    /// [`correct_in_place`](Self::correct_in_place) without its two syndrome
    /// evaluations.
    ///
    /// # Examples
    ///
    /// Requires a HIP/ROCm device at run time, so this is `no_run`; the body is
    /// gated so the doctest is a no-op on a build without the feature.
    ///
    /// ```no_run
    /// # #[cfg(feature = "hip")]
    /// # fn demo() -> Result<(), gf2_coding::bch::error::BchError> {
    /// use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
    /// use gf2_coding::bch::{BchDecodeOutcome, BinaryBchDecoder};
    /// use gf2_core::field::extension::BinaryPrimeExt;
    /// use gf2_core::gf2m::Gf2mField;
    /// use gf2_core::BitVec;
    ///
    /// // BCH(15, 7), so the radius is two.
    /// let extension = BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())?;
    /// let code = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
    ///     extension,
    ///     designed_distance: DesignedDistance::try_from(5)?,
    /// })?;
    /// let decoder = BinaryBchDecoder::new(&code);
    ///
    /// // The all-zero word is a codeword; give the second frame one error.
    /// let mut batch = vec![BitVec::zeros(code.n()); 2];
    /// batch[1].set(3, true);
    ///
    /// let outcomes = decoder.correct_batch_gpu(&mut batch).expect("a working device");
    /// assert_eq!(outcomes[0], BchDecodeOutcome::NoErrors);
    /// assert_eq!(outcomes[1], BchDecodeOutcome::Corrected { count: 1 });
    /// assert_eq!(batch[1], BitVec::zeros(code.n()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn correct_batch_gpu(
        &self,
        received: &mut [BitVec],
    ) -> Result<Vec<BchDecodeOutcome>, gf2_kernels_hip::HipError> {
        let points = self.syndrome_points.len();
        let mut outcomes = vec![BchDecodeOutcome::NoErrors; received.len()];
        if received.is_empty() || points == 0 {
            return Ok(outcomes);
        }

        let frames: Vec<&BitVec> = received.iter().collect();
        let evaluated = self.syndromes_on_device(&frames);
        // The frames borrow the batch the recovery arm corrects in place.
        drop(frames);
        let evaluated = match evaluated {
            Ok(evaluated) => evaluated,
            // No candidate is applied yet, so there is nothing to restore.
            Err(error) => return self.recover_from_device_error(received, &[], error),
        };

        let candidates = self.apply_candidates(received, &evaluated, &mut outcomes);
        if candidates.is_empty() {
            return Ok(outcomes);
        }

        // Verification: the corrected words' syndromes, in one further device
        // batch, deciding exactly what the CPU path's second evaluation does.
        let corrected: Vec<&BitVec> = candidates
            .iter()
            .map(|&(index, _)| &received[index])
            .collect();
        let verification = self.syndromes_on_device(&corrected);
        // As above: the recovery arm restores the very words these borrow.
        drop(corrected);
        let verification = match verification {
            Ok(verification) => verification,
            Err(error) => return self.recover_from_device_error(received, &candidates, error),
        };
        for ((index, positions), row) in candidates.iter().zip(verification.chunks_exact(points)) {
            if row.iter().all(|value| value.is_zero()) {
                outcomes[*index] = BchDecodeOutcome::Corrected {
                    count: positions.len(),
                };
            } else {
                flip(&mut received[*index], positions);
                outcomes[*index] = BchDecodeOutcome::Uncorrectable;
            }
        }
        Ok(outcomes)
    }

    /// Applies the located candidate of every frame whose syndrome row is
    /// nonzero and returns the coordinates it applied, indexed by frame.
    ///
    /// Those coordinates are what verification decides on and what a failed
    /// device pass rolls back, so this is the whole record of the batch's
    /// applied-but-unverified state. A row admitting no candidate inside the
    /// radius is [`BchDecodeOutcome::Uncorrectable`] without any flip, exactly
    /// as [`correct_nonzero_syndrome`](Self::correct_nonzero_syndrome) decides
    /// it per word; an all-zero row keeps the caller's
    /// [`BchDecodeOutcome::NoErrors`].
    ///
    /// `evaluated` holds the batch's syndromes row-major, one row per frame in
    /// the layout [`syndromes_on_device`](Self::syndromes_on_device) returns.
    fn apply_candidates(
        &self,
        received: &mut [BitVec],
        evaluated: &[Gf2mElement_<V>],
        outcomes: &mut [BchDecodeOutcome],
    ) -> Vec<(usize, Vec<usize>)> {
        let mut workspace = self.workspace();
        let mut candidates: Vec<(usize, Vec<usize>)> = Vec::new();
        for (index, row) in evaluated
            .chunks_exact(self.syndrome_points.len())
            .enumerate()
        {
            if row.iter().all(|value| value.is_zero()) {
                continue;
            }
            workspace.syndromes.clone_from_slice(row);
            if !self.locate_candidate(&mut workspace) {
                outcomes[index] = BchDecodeOutcome::Uncorrectable;
                continue;
            }
            flip(&mut received[index], &workspace.positions);
            candidates.push((index, workspace.positions.clone()));
        }
        candidates
    }

    /// Answers a device failure raised partway through
    /// [`correct_batch_gpu`](Self::correct_batch_gpu).
    ///
    /// Undoes every applied candidate first — `flip` is an involution and
    /// `candidates` carries the coordinates — so `received` holds the caller's
    /// own words again before either arm decides anything. A recoverable
    /// failure then decodes that restored batch on the CPU, the tested safe
    /// fallback of `@/inv/accelerator-safe-fallback`, which reports the very
    /// outcomes the device path reproduces; a failure that stays explicit
    /// propagates, with the batch as the caller passed it.
    fn recover_from_device_error(
        &self,
        received: &mut [BitVec],
        candidates: &[(usize, Vec<usize>)],
        error: gf2_kernels_hip::HipError,
    ) -> Result<Vec<BchDecodeOutcome>, gf2_kernels_hip::HipError> {
        for (index, positions) in candidates {
            flip(&mut received[*index], positions);
        }
        if error.is_recoverable() {
            Ok(self.correct_batch_cpu(received))
        } else {
            Err(error)
        }
    }

    /// Corrects a whole batch on the per-word CPU path, over one workspace.
    ///
    /// # Panics
    ///
    /// Panics if a frame is not `n` coordinates long, which the device pass
    /// asserts before it can fail into this fallback.
    fn correct_batch_cpu(&self, received: &mut [BitVec]) -> Vec<BchDecodeOutcome> {
        let mut workspace = self.workspace();
        received
            .iter_mut()
            .map(|word| {
                self.correct_in_place(word, &mut workspace)
                    .expect("every frame of the batch is n coordinates long")
            })
            .collect()
    }
}

/// Flips every listed coordinate of `word`.
fn flip(word: &mut BitVec, positions: &[usize]) {
    for &position in positions {
        word.set(position, !word.get(position));
    }
}

/// Marks the Frobenius orbit `e, 2e, 4e, ...` of `exponent` modulo `modulus`.
fn mark_frobenius_orbit(marked: &mut [bool], exponent: u64, modulus: u128) {
    let mut current = u128::from(exponent) % modulus;
    while !marked[current as usize] {
        marked[current as usize] = true;
        current = (current * 2) % modulus;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bch_code_creation() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        assert_eq!(code.n(), 15);
        assert_eq!(code.k(), 11);
        assert_eq!(code.t(), 1);
    }

    #[test]
    fn test_generator_polynomial_exists() {
        let field = Gf2mField::new(4, 0b10011);
        let code = BchCode::new(15, 11, 1, field);

        // Generator should be non-zero and have reasonable degree
        assert!(code.generator().degree().is_some());
    }
}

#[cfg(test)]
mod generator_matrix_access_tests {
    use super::*;
    use crate::traits::{BlockEncoder, GeneratorMatrixAccess};
    use gf2_core::BitMatrix;

    #[test]
    fn test_bch_generator_matrix_dimensions() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let g = code.generator_matrix();
        assert_eq!(g.rows(), 11);
        assert_eq!(g.cols(), 15);
    }

    #[test]
    fn test_bch_generator_parity_orthogonality() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let g = code.generator_matrix();

        // For BCH, verify each row of G is a valid codeword
        // Note: rows are in [message | parity] format with DVB-T2 bit ordering
        for i in 0..code.k() {
            let row = g.row_as_bitvec(i);

            // For a systematic code, the row should be the encoding of basis vector e_i
            // Just verify it has the right length and systematic property
            assert_eq!(row.len(), code.n());

            // For systematic codes, row i should have bit i set in the message portion
            assert!(
                row.get(i),
                "Row {} should have bit {} set (systematic property)",
                i,
                i
            );
        }
    }

    #[test]
    fn test_bch_is_systematic() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        assert!(code.is_systematic());
    }

    #[test]
    fn test_bch_encoding_via_generator_matches_polynomial() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 7, 2, field);
        let encoder = BchEncoder::new(code.clone());

        // Test message
        let mut msg = BitVec::new();
        msg.resize(7, false);
        msg.set(0, true);
        msg.set(3, true);
        msg.set(6, true);

        // Encode via polynomial (existing path)
        let codeword1 = encoder.encode(&msg);

        // Encode via generator matrix
        let g = code.generator_matrix();
        let mut msg_matrix = BitMatrix::zeros(1, code.k());
        for i in 0..code.k() {
            msg_matrix.set(0, i, msg.get(i));
        }
        let codeword2_matrix = &msg_matrix * &g;
        let codeword2 = codeword2_matrix.row_as_bitvec(0);

        assert_eq!(codeword1, codeword2);
    }

    #[test]
    fn test_bch_generator_matrix_cached() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);

        let g1 = code.generator_matrix();
        let g2 = code.generator_matrix();

        assert_eq!(g1, g2);
        // Second call should be faster (cached)
    }

    #[test]
    fn test_bch_small_code() {
        // BCH(7,4,1) - smaller test
        let field = Gf2mField::new(3, 0b1011).with_tables();
        let code = BchCode::new(7, 4, 1, field);
        let g = code.generator_matrix();

        assert_eq!(g.rows(), 4);
        assert_eq!(g.cols(), 7);
        assert!(code.is_systematic());
    }

    #[test]
    fn test_bch_systematic_encoding_preserves_message() {
        // Test that systematic encoding produces [message | parity] format
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());

        // Test with all-ones message
        let msg = BitVec::ones(11);
        let codeword = encoder.encode(&msg);

        // Verify first k bits = original message
        for i in 0..11 {
            assert_eq!(
                codeword.get(i),
                msg.get(i),
                "Systematic property violated at bit {}: codeword should start with message",
                i
            );
        }

        // Test with a specific pattern
        let mut msg2 = BitVec::zeros(11);
        msg2.set(0, true);
        msg2.set(5, true);
        msg2.set(10, true);
        let codeword2 = encoder.encode(&msg2);

        for i in 0..11 {
            assert_eq!(
                codeword2.get(i),
                msg2.get(i),
                "Systematic property violated at bit {}: codeword should start with message",
                i
            );
        }
    }
}

#[cfg(test)]
mod batch_api_tests {
    use super::*;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_decode_batch_empty() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let decoder = BchDecoder::new(code);

        let received: Vec<BitVec> = vec![];
        let decoded = decoder.decode_batch(&received);

        assert_eq!(decoded.len(), 0);
    }

    #[test]
    fn test_decode_batch_single() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Encode single message
        let msg = BitVec::ones(11);
        let codeword = encoder.encode(&msg);

        // Decode batch of one
        let received = vec![codeword];
        let decoded = decoder.decode_batch(&received);

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], msg);
    }

    #[test]
    fn test_decode_batch_multiple_no_errors() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Create multiple messages
        let messages: Vec<BitVec> = (0..10)
            .map(|i| {
                let mut msg = BitVec::with_capacity(11);
                for j in 0..11 {
                    msg.push_bit((i + j) % 2 == 0);
                }
                msg
            })
            .collect();

        // Encode all messages
        let codewords: Vec<BitVec> = messages.iter().map(|msg| encoder.encode(msg)).collect();

        // Decode batch (no errors)
        let decoded = decoder.decode_batch(&codewords);

        assert_eq!(decoded.len(), 10);
        for (i, dec) in decoded.iter().enumerate() {
            assert_eq!(dec, &messages[i], "Message {} not decoded correctly", i);
        }
    }

    #[test]
    fn test_decode_batch_with_single_error() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 11, 1, field);
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        // Create messages
        let messages: Vec<BitVec> = (0..5)
            .map(|i| {
                let mut msg = BitVec::zeros(11);
                msg.set(i * 2 % 11, true);
                msg
            })
            .collect();

        // Encode and introduce single error in each
        let received: Vec<BitVec> = messages
            .iter()
            .map(|msg| {
                let mut cw = encoder.encode(msg);
                // Introduce error at position 5
                cw.set(5, !cw.get(5));
                cw
            })
            .collect();

        // Decode batch (should correct all single errors)
        let decoded = decoder.decode_batch(&received);

        assert_eq!(decoded.len(), 5);
        for (i, dec) in decoded.iter().enumerate() {
            assert_eq!(dec, &messages[i], "Message {} not corrected properly", i);
        }
    }

    #[test]
    fn test_decode_batch_mixed_errors() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 7, 2, field); // t=2 error correction
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let messages: Vec<BitVec> = vec![BitVec::zeros(7), BitVec::ones(7), {
            let mut m = BitVec::zeros(7);
            m.set(0, true);
            m.set(3, true);
            m
        }];

        let mut received: Vec<BitVec> = vec![];

        // Message 0: no errors
        received.push(encoder.encode(&messages[0]));

        // Message 1: single error
        let mut cw1 = encoder.encode(&messages[1]);
        cw1.set(3, !cw1.get(3));
        received.push(cw1);

        // Message 2: two errors
        let mut cw2 = encoder.encode(&messages[2]);
        cw2.set(5, !cw2.get(5));
        cw2.set(10, !cw2.get(10));
        received.push(cw2);

        // Decode batch
        let decoded = decoder.decode_batch(&received);

        assert_eq!(decoded.len(), 3);
        for (i, dec) in decoded.iter().enumerate() {
            assert_eq!(dec, &messages[i], "Message {} not decoded correctly", i);
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::traits::{BlockEncoder, HardDecisionDecoder};
    use proptest::prelude::*;

    #[derive(Clone, Copy, Debug)]
    enum BchParameterSet {
        Bch7_4,
        Bch15_11,
        Bch15_7,
        Bch31_26,
        Bch63_57,
    }

    impl BchParameterSet {
        fn code(self) -> BchCode {
            match self {
                Self::Bch7_4 => BchCode::new(7, 4, 1, Gf2mField::new(3, 0b1011).with_tables()),
                Self::Bch15_11 => BchCode::new(15, 11, 1, Gf2mField::new(4, 0b10011).with_tables()),
                Self::Bch15_7 => BchCode::new(15, 7, 2, Gf2mField::new(4, 0b10011).with_tables()),
                Self::Bch31_26 => {
                    BchCode::new(31, 26, 1, Gf2mField::new(5, 0b100101).with_tables())
                }
                Self::Bch63_57 => {
                    BchCode::new(63, 57, 1, Gf2mField::new(6, 0b1000011).with_tables())
                }
            }
        }
    }

    fn bch_message_cases() -> impl Strategy<Value = (BchParameterSet, Vec<bool>)> {
        prop_oneof![
            (
                Just(BchParameterSet::Bch7_4),
                prop::collection::vec(any::<bool>(), 4)
            ),
            (
                Just(BchParameterSet::Bch15_11),
                prop::collection::vec(any::<bool>(), 11)
            ),
            (
                Just(BchParameterSet::Bch15_7),
                prop::collection::vec(any::<bool>(), 7)
            ),
            (
                Just(BchParameterSet::Bch31_26),
                prop::collection::vec(any::<bool>(), 26)
            ),
            (
                Just(BchParameterSet::Bch63_57),
                prop::collection::vec(any::<bool>(), 57)
            ),
        ]
    }

    fn bch_recovery_cases() -> impl Strategy<Value = (BchParameterSet, Vec<bool>, Vec<usize>)> {
        prop_oneof![
            (
                Just(BchParameterSet::Bch7_4),
                prop::collection::vec(any::<bool>(), 4),
                prop::collection::vec(0usize..7, 1)
            ),
            (
                Just(BchParameterSet::Bch15_11),
                prop::collection::vec(any::<bool>(), 11),
                prop::collection::vec(0usize..15, 1)
            ),
            (
                Just(BchParameterSet::Bch15_7),
                prop::collection::vec(any::<bool>(), 7),
                prop::collection::vec(0usize..15, 2)
            ),
            (
                Just(BchParameterSet::Bch31_26),
                prop::collection::vec(any::<bool>(), 26),
                prop::collection::vec(0usize..31, 1)
            ),
            (
                Just(BchParameterSet::Bch63_57),
                prop::collection::vec(any::<bool>(), 57),
                prop::collection::vec(0usize..63, 1)
            ),
        ]
    }

    fn distinct_error_positions(seeds: &[usize], n: usize, count: usize) -> Vec<usize> {
        let mut positions = Vec::with_capacity(count);

        for &seed in seeds {
            let position = seed % n;
            if !positions.contains(&position) {
                positions.push(position);
            }
        }

        for position in 0..n {
            if positions.len() == count {
                break;
            }
            if !positions.contains(&position) {
                positions.push(position);
            }
        }

        positions
    }

    proptest! {
        #[test]
        fn prop_bch_encode_decode_roundtrip_within_correction_capability(
            (parameter_set, msg_bits, error_seeds) in bch_recovery_cases()
        ) {
            let code = parameter_set.code();
            let encoder = BchEncoder::new(code.clone());
            let decoder = BchDecoder::new(code.clone());

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = encoder.encode(&msg);
            let error_positions = distinct_error_positions(&error_seeds, code.n(), code.t());

            // Testing every prefix makes the correction boundary explicit:
            // each generated case includes 0, 1, ..., t distinct errors.
            for error_count in 0..=code.t() {
                let mut received = codeword.clone();
                for &error_position in error_positions.iter().take(error_count) {
                    received.set(error_position, !received.get(error_position));
                }

                let decoded = decoder.decode(&received);
                prop_assert_eq!(
                    &decoded,
                    &msg,
                    "failed for {:?} with {} errors",
                    parameter_set,
                    error_count
                );
            }
        }

        #[test]
        fn prop_bch_valid_codeword_has_zero_syndrome(
            (parameter_set, msg_bits) in bch_message_cases()
        ) {
            let code = parameter_set.code();
            let encoder = BchEncoder::new(code.clone());
            let decoder = BchDecoder::new(code);

            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }

            let codeword = encoder.encode(&msg);
            let syndrome = decoder.compute_syndromes(&codeword);

            prop_assert!(
                syndrome.iter().all(|value| value.is_zero()),
                "valid {:?} codeword must have zero syndrome",
                parameter_set
            );
        }
    }
}

#[cfg(test)]
mod canonical_decoder_tests {
    use super::*;
    use crate::bch::spec::{BchSpec, DesignedDistance, RootExponent};
    use gf2_core::field::extension::BinaryPrimeExt;
    use gf2_core::gfp::Fp;
    use proptest::prelude::*;
    use std::sync::OnceLock;

    /// Representative binary narrow-sense points as
    /// `(m, primitive polynomial, designed distance)`, covering radii one
    /// through three and four block lengths.
    const POINTS: &[(usize, u64, u64)] = &[
        (3, 0b1011, 3),    // BCH(7, 4), t = 1
        (4, 0b10011, 3),   // BCH(15, 11), t = 1
        (4, 0b10011, 5),   // BCH(15, 7), t = 2
        (4, 0b10011, 7),   // BCH(15, 5), t = 3
        (5, 0b100101, 7),  // BCH(31, 21), t = 3
        (6, 0b1000011, 7), // BCH(63, 45), t = 3
    ];

    fn narrow_sense(m: usize, modulus: u64, designed_distance: u64) -> BinaryBchCode {
        BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension: BinaryPrimeExt::new(Gf2mField::new(m, modulus).with_tables())
                .expect("a primitive modulus"),
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("a positive designed distance"),
        })
        .expect("a valid primitive narrow-sense spec")
    }

    #[test]
    fn a_foreign_workspace_is_rejected_even_with_matching_buffer_lengths() {
        // Two distinct primitive presentations of the same (n, delta) give
        // decoders with identical buffer geometry; the stamp must still tell
        // them apart.
        // Designed distance 3 (t = 1) is the reviewer's sharpest case: the
        // syndrome points x, x^2 have identical raw values in every degree-4
        // presentation, so only the modulus separates the stamps.
        let code_a = narrow_sense(4, 0b10011, 3);
        let code_b = narrow_sense(4, 0b11001, 3);
        let decoder_a = BinaryBchDecoder::new(&code_a);
        let decoder_b = BinaryBchDecoder::new(&code_b);
        let mut foreign = decoder_b.workspace();
        let mut received = BitVec::zeros(15);
        let result = decoder_a.correct_in_place(&mut received, &mut foreign);
        assert!(matches!(
            result,
            Err(BchError::WorkspaceMismatch { expected_stamp, actual_stamp })
                if expected_stamp != actual_stamp
        ));
        let mut own = decoder_a.workspace();
        assert!(decoder_a.correct_in_place(&mut received, &mut own).is_ok());
    }

    /// The parameter points, constructed once for the whole suite.
    fn codes() -> &'static [BinaryBchCode] {
        static CODES: OnceLock<Vec<BinaryBchCode>> = OnceLock::new();
        CODES.get_or_init(|| {
            POINTS
                .iter()
                .map(|&(m, modulus, designed_distance)| narrow_sense(m, modulus, designed_distance))
                .collect()
        })
    }

    /// Reads a word as the polynomial whose degree-`i` coefficient is
    /// coordinate `i`, the construction model's convention.
    fn as_polynomial(code: &BinaryBchCode, word: &BitVec) -> FieldPoly<Fp<2>> {
        let extension = code.extension();
        let zero = extension.base_zero();
        let one = extension.base_one();
        FieldPoly::new(
            (0..word.len())
                .map(|index| if word.get(index) { one } else { zero })
                .collect(),
        )
    }

    /// Encodes `message` as the coefficient vector of `m(x) * g(x)`.
    ///
    /// This is the encoding the construction model defines, and the inverse
    /// of the information word [`BchDecodeReport::message`] reports.
    fn encode(code: &BinaryBchCode, message: &[bool]) -> BitVec {
        assert_eq!(message.len(), code.k(), "a message has k coordinates");
        let word = bits_to_word(message);
        let product = as_polynomial(code, &word).mul(code.generator());
        let zero = code.extension().base_zero();

        let mut codeword = BitVec::zeros(code.n());
        for index in 0..code.n() {
            if product.coeff_or_zero(index, &zero).is_one() {
                codeword.set(index, true);
            }
        }
        codeword
    }

    fn bits_to_word(bits: &[bool]) -> BitVec {
        let mut word = BitVec::zeros(bits.len());
        for (index, &bit) in bits.iter().enumerate() {
            word.set(index, bit);
        }
        word
    }

    fn is_codeword(code: &BinaryBchCode, word: &BitVec) -> bool {
        as_polynomial(code, word)
            .div_rem(code.generator())
            .1
            .is_zero()
    }

    fn distance(left: &BitVec, right: &BitVec) -> usize {
        (0..left.len())
            .filter(|&index| left.get(index) != right.get(index))
            .count()
    }

    /// Turns arbitrary seeds into `count` distinct coordinates below `length`,
    /// filling from the low coordinates when the seeds collide.
    fn distinct_positions(seeds: &[usize], length: usize, count: usize) -> Vec<usize> {
        let mut positions = Vec::with_capacity(count);
        for &seed in seeds {
            if positions.len() == count {
                break;
            }
            let position = seed % length;
            if !positions.contains(&position) {
                positions.push(position);
            }
        }
        for position in 0..length {
            if positions.len() == count {
                break;
            }
            if !positions.contains(&position) {
                positions.push(position);
            }
        }
        positions
    }

    fn sorted(positions: &[usize]) -> Vec<usize> {
        let mut sorted = positions.to_vec();
        sorted.sort_unstable();
        sorted
    }

    // -- REQ-01/REQ-02: the error-free outcome and the two paths -----------

    #[test]
    fn a_codeword_decodes_to_its_information_word() {
        for code in codes() {
            let decoder = BinaryBchDecoder::new(code);
            let mut workspace = decoder.workspace();
            let message: Vec<bool> = (0..code.k()).map(|index| index % 3 == 0).collect();
            let codeword = encode(code, &message);
            assert!(is_codeword(code, &codeword), "the test encoder is faithful");

            let mut storage = codeword.clone();
            assert_eq!(
                decoder
                    .correct_in_place(&mut storage, &mut workspace)
                    .expect("the received word has length n"),
                BchDecodeOutcome::NoErrors
            );
            assert_eq!(storage, codeword, "an error-free word is left untouched");

            let report = decoder.decode(&codeword).expect("the word has length n");
            assert_eq!(report.outcome(), BchDecodeOutcome::NoErrors);
            assert_eq!(report.codeword(), Some(&codeword));
            assert_eq!(report.message(), Some(&bits_to_word(&message)));
            assert!(report.error_positions().is_empty());
            assert_eq!(report.error_count(), Some(0));
        }
    }

    // -- REQ-03: every error count at and beyond the radius ----------------

    fn error_cases() -> impl Strategy<Value = (usize, Vec<bool>, Vec<usize>)> {
        (
            0..POINTS.len(),
            prop::collection::vec(any::<bool>(), 45),
            prop::collection::vec(any::<usize>(), 6),
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Every error count from zero to the radius is corrected, and the
        /// diagnostic path names exactly the injected coordinates.
        #[test]
        fn prop_errors_within_the_radius_are_corrected_at_their_exact_positions(
            (index, bits, seeds) in error_cases(),
        ) {
            let code = &codes()[index];
            let decoder = BinaryBchDecoder::new(code);
            let mut workspace = decoder.workspace();

            let message = &bits[..code.k()];
            let codeword = encode(code, message);
            let injected =
                distinct_positions(&seeds, code.n(), code.correction_radius());

            for error_count in 0..=code.correction_radius() {
                let mut received = codeword.clone();
                flip(&mut received, &injected[..error_count]);
                prop_assert_eq!(distance(&received, &codeword), error_count);

                let expected = if error_count == 0 {
                    BchDecodeOutcome::NoErrors
                } else {
                    BchDecodeOutcome::Corrected { count: error_count }
                };

                let expected_positions = sorted(&injected[..error_count]);
                let report = decoder.decode(&received).expect("the word has length n");
                prop_assert_eq!(report.outcome(), expected);
                prop_assert_eq!(report.codeword(), Some(&codeword));
                prop_assert_eq!(report.message(), Some(&bits_to_word(message)));
                prop_assert_eq!(report.error_positions(), expected_positions.as_slice());

                let mut storage = received.clone();
                let outcome = decoder
                    .correct_in_place(&mut storage, &mut workspace)
                    .expect("the received word has length n");
                prop_assert_eq!(outcome, expected);
                prop_assert_eq!(&storage, &codeword);
            }
        }

        /// One error past the radius leaves exactly two possibilities: no
        /// verified correction, or a correction to a codeword other than the
        /// transmitted one. Nothing the decoder reports is ever a false claim
        /// about the word it produces.
        #[test]
        fn prop_one_error_beyond_the_radius_is_uncorrectable_or_a_verified_codeword(
            (index, bits, seeds) in error_cases(),
        ) {
            let code = &codes()[index];
            let decoder = BinaryBchDecoder::new(code);
            let mut workspace = decoder.workspace();

            let codeword = encode(code, &bits[..code.k()]);
            let injected =
                distinct_positions(&seeds, code.n(), code.correction_radius() + 1);
            let mut received = codeword.clone();
            flip(&mut received, &injected);
            let original = received.clone();
            prop_assert_eq!(
                distance(&received, &codeword),
                code.correction_radius() + 1
            );

            let outcome = decoder
                .correct_in_place(&mut received, &mut workspace)
                .expect("the received word has length n");
            match outcome {
                BchDecodeOutcome::NoErrors => {
                    prop_assert!(false, "t + 1 errors cannot leave a codeword")
                }
                BchDecodeOutcome::Uncorrectable => prop_assert_eq!(&received, &original),
                BchDecodeOutcome::Corrected { count } => {
                    prop_assert!(count <= code.correction_radius());
                    prop_assert!(is_codeword(code, &received));
                    prop_assert_eq!(distance(&original, &received), count);
                    prop_assert_ne!(&received, &codeword);
                }
            }
        }

        /// An arbitrary word never panics, and each outcome means what the
        /// contract says: `NoErrors` exactly for codewords, `Corrected` only
        /// for a verified codeword inside the radius, `Uncorrectable` only
        /// with the caller's storage left intact.
        #[test]
        fn prop_arbitrary_words_decode_soundly(
            (index, bits) in (0..POINTS.len(), prop::collection::vec(any::<bool>(), 63)),
        ) {
            let code = &codes()[index];
            let decoder = BinaryBchDecoder::new(code);
            let mut workspace = decoder.workspace();

            let received = bits_to_word(&bits[..code.n()]);
            let mut storage = received.clone();
            let outcome = decoder
                .correct_in_place(&mut storage, &mut workspace)
                .expect("the received word has length n");

            match outcome {
                BchDecodeOutcome::NoErrors => {
                    prop_assert!(is_codeword(code, &received));
                    prop_assert_eq!(&storage, &received);
                }
                BchDecodeOutcome::Uncorrectable => {
                    prop_assert!(!is_codeword(code, &received));
                    prop_assert_eq!(&storage, &received);
                }
                BchDecodeOutcome::Corrected { count } => {
                    prop_assert!(count >= 1 && count <= code.correction_radius());
                    prop_assert!(is_codeword(code, &storage));
                    prop_assert_eq!(distance(&received, &storage), count);
                }
            }
        }
    }

    /// One error beyond the radius leaves exactly two possibilities, and both
    /// occur for BCH(15, 7): the procedure reports no verified correction, or
    /// it miscorrects to a different codeword within the radius.
    #[test]
    fn beyond_the_radius_the_outcome_is_uncorrectable_or_a_verified_miscorrection() {
        let code = narrow_sense(4, 0b10011, 5);
        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();
        let length = code.n();
        let radius = code.correction_radius();
        let zero_codeword = BitVec::zeros(length);

        let mut uncorrectable = 0usize;
        let mut miscorrected = 0usize;
        for first in 0..length {
            for second in (first + 1)..length {
                for third in (second + 1)..length {
                    let mut received = zero_codeword.clone();
                    flip(&mut received, &[first, second, third]);
                    let original = received.clone();

                    let outcome = decoder
                        .correct_in_place(&mut received, &mut workspace)
                        .expect("the received word has length n");
                    match outcome {
                        BchDecodeOutcome::NoErrors => {
                            panic!("a weight-three word is not a codeword of BCH(15, 7)")
                        }
                        BchDecodeOutcome::Uncorrectable => {
                            assert_eq!(received, original, "a rejected candidate is rolled back");
                            uncorrectable += 1;
                        }
                        BchDecodeOutcome::Corrected { count } => {
                            assert!(count <= radius, "a correction stays inside the radius");
                            assert_eq!(distance(&original, &received), count);
                            assert!(
                                is_codeword(&code, &received),
                                "a reported correction is always a codeword"
                            );
                            assert_ne!(
                                received, zero_codeword,
                                "three errors cannot resolve to the transmitted word"
                            );
                            miscorrected += 1;
                        }
                    }
                }
            }
        }

        assert!(uncorrectable > 0, "the uncorrectable arm is exercised");
        assert!(miscorrected > 0, "the miscorrection arm is exercised");
    }

    /// A word whose whole coset has weight above the radius has no codeword
    /// within the radius at all, so no bounded-distance procedure can produce
    /// a verified correction for it.
    #[test]
    fn a_coset_leader_beyond_the_radius_is_uncorrectable() {
        let code = narrow_sense(4, 0b10011, 5);
        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();
        let length = code.n();
        let radius = code.correction_radius();

        let codewords: Vec<BitVec> = (0..1u32 << code.k())
            .map(|value| {
                let bits: Vec<bool> = (0..code.k()).map(|index| value >> index & 1 == 1).collect();
                encode(&code, &bits)
            })
            .collect();

        let mut leader = None;
        'search: for first in 0..length {
            for second in (first + 1)..length {
                for third in (second + 1)..length {
                    let mut candidate = BitVec::zeros(length);
                    flip(&mut candidate, &[first, second, third]);
                    if codewords
                        .iter()
                        .all(|codeword| distance(&candidate, codeword) > radius)
                    {
                        leader = Some(candidate);
                        break 'search;
                    }
                }
            }
        }
        let received = leader.expect("BCH(15, 7) has covering radius three, so such a word exists");

        let mut storage = received.clone();
        assert_eq!(
            decoder
                .correct_in_place(&mut storage, &mut workspace)
                .expect("the received word has length n"),
            BchDecodeOutcome::Uncorrectable
        );
        assert_eq!(storage, received, "the caller's storage is left intact");

        let report = decoder.decode(&received).expect("the word has length n");
        assert_eq!(report.outcome(), BchDecodeOutcome::Uncorrectable);
        assert_eq!(report.codeword(), None);
        assert_eq!(report.message(), None);
        assert_eq!(report.error_count(), None);
        assert!(report.error_positions().is_empty());
    }

    // -- REQ-02: the fast path reuses one workspace ------------------------

    /// The lengths and capacities of every workspace buffer after a mixed run
    /// of decodes. The decoder sizes each buffer once and only overwrites it
    /// afterwards, so an unchanged snapshot witnesses that no buffer grew and
    /// therefore that the per-word path reallocated nothing.
    fn buffer_shape(workspace: &BchDecodeWorkspace) -> Vec<(usize, usize)> {
        vec![
            (workspace.syndromes.len(), workspace.syndromes.capacity()),
            (workspace.locator.len(), workspace.locator.capacity()),
            (workspace.previous.len(), workspace.previous.capacity()),
            (workspace.scratch.len(), workspace.scratch.capacity()),
            (workspace.chien.len(), workspace.chien.capacity()),
            (0, workspace.positions.capacity()),
        ]
    }

    #[test]
    fn repeated_decodes_reuse_one_workspace() {
        let code = narrow_sense(4, 0b10011, 5);
        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();
        let shape = buffer_shape(&workspace);

        let message: Vec<bool> = (0..code.k()).map(|index| index % 2 == 0).collect();
        let codeword = encode(&code, &message);

        for round in 0..64 {
            for error_count in 0..=code.correction_radius() + 1 {
                let mut received = codeword.clone();
                let positions: Vec<usize> = (0..error_count)
                    .map(|offset| (round + 3 * offset) % code.n())
                    .collect();
                let unique: Vec<usize> = sorted(&positions);
                if unique.windows(2).any(|pair| pair[0] == pair[1]) {
                    continue;
                }
                flip(&mut received, &positions);

                let outcome = decoder
                    .correct_in_place(&mut received, &mut workspace)
                    .expect("the received word has length n");
                assert!(matches!(
                    outcome.corrected_count(),
                    None | Some(0..=2) // at most the radius
                ));
                assert_eq!(
                    buffer_shape(&workspace),
                    shape,
                    "no workspace buffer grew during decoding"
                );
            }
        }
    }

    // -- Input validation --------------------------------------------------

    #[test]
    fn a_mismatched_buffer_is_rejected_before_decoding() {
        let code = narrow_sense(4, 0b10011, 5);
        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();

        let mut short = BitVec::zeros(code.n() - 1);
        assert_eq!(
            decoder
                .correct_in_place(&mut short, &mut workspace)
                .expect_err("a received word has n coordinates"),
            BchError::Decode(CodeError::BufferLengthMismatch {
                expected: code.n(),
                actual: code.n() - 1,
            })
        );
        assert!(decoder.decode(&short).is_err());

        // A workspace sized for a different code is a caller mistake the
        // decoder reports instead of indexing past a buffer.
        let other = narrow_sense(4, 0b10011, 7);
        let mut foreign = BinaryBchDecoder::new(&other).workspace();
        let mut word = BitVec::zeros(code.n());
        assert!(decoder.correct_in_place(&mut word, &mut foreign).is_err());
    }

    // -- Boundary and non-narrow-sense constructions -----------------------

    #[test]
    fn the_full_space_code_reports_every_word_as_a_codeword() {
        let code = narrow_sense(4, 0b10011, 1);
        assert_eq!(code.correction_radius(), 0);
        assert_eq!(code.k(), code.n());

        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();
        for value in 0..1u32 << 8 {
            let bits: Vec<bool> = (0..code.n()).map(|index| value >> index & 1 == 1).collect();
            let received = bits_to_word(&bits);
            let mut storage = received.clone();
            assert_eq!(
                decoder
                    .correct_in_place(&mut storage, &mut workspace)
                    .expect("the received word has length n"),
                BchDecodeOutcome::NoErrors
            );
            assert_eq!(storage, received);
        }

        let ones = BitVec::ones(code.n());
        let report = decoder.decode(&ones).expect("the word has length n");
        assert_eq!(report.message(), Some(&ones), "the generator is one");
    }

    /// The decoder reads the witnessed run rather than assuming the
    /// narrow-sense start: seeds `{2, 3, 4, 5}` close to a run of six that
    /// starts at exponent one, so the radius is three.
    #[test]
    fn a_first_root_code_corrects_within_its_witnessed_radius() {
        let code = BinaryBchCode::construct(BchSpec::PrimitiveFirstRoot {
            extension: BinaryPrimeExt::new(Gf2mField::new(4, 0b10011).with_tables())
                .expect("a primitive modulus"),
            first_root: RootExponent::from(2),
            designed_distance: DesignedDistance::try_from(5).expect("positive"),
        })
        .expect("a valid primitive first-root spec");
        assert_eq!(code.correction_radius(), 3);
        assert_eq!(code.k(), 5);

        let decoder = BinaryBchDecoder::new(&code);
        let mut workspace = decoder.workspace();
        let message = [true, false, true, true, false];
        let codeword = encode(&code, &message);

        for positions in [&[3usize][..], &[0, 11][..], &[2, 7, 13][..]] {
            let mut received = codeword.clone();
            flip(&mut received, positions);

            let report = decoder.decode(&received).expect("the word has length n");
            assert_eq!(
                report.outcome(),
                BchDecodeOutcome::Corrected {
                    count: positions.len()
                }
            );
            assert_eq!(report.error_positions(), sorted(positions).as_slice());
            assert_eq!(report.message(), Some(&bits_to_word(&message)));

            let mut storage = received;
            assert_eq!(
                decoder
                    .correct_in_place(&mut storage, &mut workspace)
                    .expect("the received word has length n"),
                BchDecodeOutcome::Corrected {
                    count: positions.len()
                }
            );
            assert_eq!(storage, codeword);
        }
    }

    // -- GPU-assisted decoding over the same model -------------------------

    /// The device path, held against the CPU path it reproduces.
    ///
    /// Each test self-gates on a usable device and skips without one, and each
    /// runs in about a tenth of a second on the small parameter points, so
    /// they stay in the fast tier rather than behind an ignore. The committed
    /// receipt of issue `c3cc5226` records a run on a named HIP host.
    #[cfg(feature = "hip")]
    mod gpu {
        use super::*;

        use gf2_kernels_hip::HipError;

        fn device_present() -> bool {
            gf2_kernels_hip::host::device_mem_info().is_ok()
        }

        /// Every weight-three word of a length-`length` code.
        fn weight_three_words(length: usize) -> Vec<BitVec> {
            let mut frames = Vec::new();
            for first in 0..length {
                for second in (first + 1)..length {
                    for third in (second + 1)..length {
                        let mut word = BitVec::zeros(length);
                        flip(&mut word, &[first, second, third]);
                        frames.push(word);
                    }
                }
            }
            frames
        }

        /// A mixed batch over four messages: the codeword itself, then every
        /// error count from one up to one past the radius.
        fn population(code: &BinaryBchCode) -> Vec<BitVec> {
            let mut frames = Vec::new();
            for round in 0..4 {
                let message: Vec<bool> = (0..code.k())
                    .map(|index| (index + round) % 3 == 0)
                    .collect();
                let codeword = encode(code, &message);
                for errors in 0..=code.correction_radius() + 1 {
                    let seeds: Vec<usize> = (0..errors)
                        .map(|offset| 5 * offset + 3 * round + errors)
                        .collect();
                    let mut word = codeword.clone();
                    flip(&mut word, &distinct_positions(&seeds, code.n(), errors));
                    frames.push(word);
                }
            }
            frames
        }

        /// The outcomes and corrected words of the per-word CPU fast path.
        fn cpu_reference(
            decoder: &BinaryBchDecoder<'_>,
            frames: &[BitVec],
        ) -> (Vec<BchDecodeOutcome>, Vec<BitVec>) {
            let mut workspace = decoder.workspace();
            let mut words = frames.to_vec();
            let outcomes = words
                .iter_mut()
                .map(|word| {
                    decoder
                        .correct_in_place(word, &mut workspace)
                        .expect("the received word has length n")
                })
                .collect();
            (outcomes, words)
        }

        #[test]
        fn device_syndromes_equal_the_cpu_evaluator() {
            if !device_present() {
                eprintln!("skipping device_syndromes_equal_the_cpu_evaluator: no usable GPU");
                return;
            }
            for code in codes() {
                let decoder = BinaryBchDecoder::new(code);
                let frames = population(code);
                let device = decoder
                    .compute_syndromes_batch_gpu(&frames)
                    .expect("a working device");
                assert_eq!(device.len(), frames.len());

                let mut host = vec![decoder.zero.clone(); decoder.syndrome_points.len()];
                for (index, frame) in frames.iter().enumerate() {
                    decoder.evaluate_syndromes(frame, &mut host);
                    let evaluated: Vec<u64> =
                        device[index].iter().map(|value| value.value()).collect();
                    let expected: Vec<u64> = host.iter().map(|value| value.value()).collect();
                    assert_eq!(
                        evaluated,
                        expected,
                        "BCH({}, {}) frame {index}: device syndromes differ from the CPU",
                        code.n(),
                        code.k()
                    );
                }
            }
        }

        #[test]
        fn gpu_assisted_correction_reports_the_cpu_outcomes() {
            if !device_present() {
                eprintln!(
                    "skipping gpu_assisted_correction_reports_the_cpu_outcomes: no usable GPU"
                );
                return;
            }
            let mut corrections = 0usize;
            for code in codes() {
                let decoder = BinaryBchDecoder::new(code);
                let frames = population(code);
                let (expected, expected_words) = cpu_reference(&decoder, &frames);

                let mut words = frames.clone();
                let outcomes = decoder
                    .correct_batch_gpu(&mut words)
                    .expect("a working device");
                assert_eq!(
                    outcomes,
                    expected,
                    "BCH({}, {}): device outcomes differ from the CPU",
                    code.n(),
                    code.k()
                );
                assert_eq!(
                    words,
                    expected_words,
                    "BCH({}, {}): device corrections differ from the CPU",
                    code.n(),
                    code.k()
                );
                corrections += outcomes
                    .iter()
                    .filter(|outcome| matches!(outcome, BchDecodeOutcome::Corrected { .. }))
                    .count();
            }
            assert!(corrections > 0, "the population exercises corrections");
        }

        /// Every weight-three word of BCH(15, 7) lies one past the radius, and
        /// both arms occur there: a candidate that verifies as a different
        /// codeword, and one that fails verification. The device path must
        /// report each exactly as the CPU path does, rolling back the words it
        /// rejects.
        #[test]
        fn a_candidate_failing_verification_is_uncorrectable_on_both_paths() {
            if !device_present() {
                eprintln!(
                    "skipping a_candidate_failing_verification_is_uncorrectable_on_both_paths: no usable GPU"
                );
                return;
            }
            let code = narrow_sense(4, 0b10011, 5);
            let decoder = BinaryBchDecoder::new(&code);
            let frames = weight_three_words(code.n());

            let (expected, expected_words) = cpu_reference(&decoder, &frames);
            let mut words = frames.clone();
            let outcomes = decoder
                .correct_batch_gpu(&mut words)
                .expect("a working device");
            assert_eq!(outcomes, expected);
            assert_eq!(words, expected_words);

            let mut rejected = 0usize;
            let mut miscorrected = 0usize;
            for (index, outcome) in outcomes.iter().enumerate() {
                match outcome {
                    BchDecodeOutcome::NoErrors => {
                        panic!("a weight-three word is not a codeword of BCH(15, 7)")
                    }
                    BchDecodeOutcome::Uncorrectable => {
                        assert_eq!(
                            words[index], frames[index],
                            "a rejected candidate is rolled back"
                        );
                        rejected += 1;
                    }
                    BchDecodeOutcome::Corrected { count } => {
                        assert!(*count <= code.correction_radius());
                        assert!(is_codeword(&code, &words[index]));
                        miscorrected += 1;
                    }
                }
            }
            assert!(
                rejected > 0,
                "the device path rejects unverified candidates"
            );
            assert!(miscorrected > 0, "the device path accepts verified ones");
        }

        // -- The device-failure paths ------------------------------------
        //
        // These drive the host bookkeeping of `correct_batch_gpu` — the
        // candidates it applies before verification, and the recovery it runs
        // when a syndrome pass fails — with a synthesized `HipError` standing
        // for the failure. They need no device, and so carry no skip.

        /// A batch's syndromes in the row-major layout the device pass returns,
        /// evaluated on the CPU: the device multiply is the CPU multiply, so
        /// the bookkeeping sees the same rows either way.
        fn host_syndromes(decoder: &BinaryBchDecoder<'_>, frames: &[BitVec]) -> Vec<Gf2mElement> {
            let mut row = vec![decoder.zero.clone(); decoder.syndrome_points.len()];
            let mut evaluated = Vec::with_capacity(frames.len() * row.len());
            for frame in frames {
                decoder.evaluate_syndromes(frame, &mut row);
                evaluated.extend(row.iter().cloned());
            }
            evaluated
        }

        /// The caller's frames, the batch holding the applied candidates, and
        /// the coordinates applied to it.
        type AppliedBatch = (Vec<BitVec>, Vec<BitVec>, Vec<(usize, Vec<usize>)>);

        /// A batch of weight-three words with every candidate applied, as the
        /// verification pass finds it.
        fn applied_batch(decoder: &BinaryBchDecoder<'_>) -> AppliedBatch {
            let frames = weight_three_words(decoder.code.n());
            let mut words = frames.clone();
            let evaluated = host_syndromes(decoder, &words);
            let mut outcomes = vec![BchDecodeOutcome::NoErrors; words.len()];
            let candidates = decoder.apply_candidates(&mut words, &evaluated, &mut outcomes);
            assert!(
                !candidates.is_empty(),
                "a weight-three word of BCH(15, 7) carries a candidate"
            );
            assert_ne!(words, frames, "the candidates are applied");
            (frames, words, candidates)
        }

        /// A device failure that stays explicit propagates with the caller's
        /// batch restored: the applied candidates are rolled back first, so a
        /// retry decodes the original words rather than altered, unverified
        /// ones.
        #[test]
        fn a_fatal_verification_failure_restores_the_batch() {
            let code = narrow_sense(4, 0b10011, 5);
            let decoder = BinaryBchDecoder::new(&code);
            let (frames, mut words, candidates) = applied_batch(&decoder);

            let result = decoder.recover_from_device_error(
                &mut words,
                &candidates,
                HipError::Hip {
                    code: 700,
                    context: "hipStreamSynchronize",
                },
            );

            assert!(
                matches!(result, Err(HipError::Hip { code: 700, .. })),
                "a raw driver status stays explicit"
            );
            assert_eq!(words, frames, "the batch is the caller's own again");
        }

        /// A recoverable device failure selects the CPU path instead of
        /// erroring: the batch is restored and decoded there, so the caller
        /// sees exactly the outcomes and words of the pure CPU path.
        #[test]
        fn a_recoverable_verification_failure_falls_back_to_the_cpu() {
            let code = narrow_sense(4, 0b10011, 5);
            let decoder = BinaryBchDecoder::new(&code);
            let (frames, mut words, candidates) = applied_batch(&decoder);
            let (expected, expected_words) = cpu_reference(&decoder, &frames);

            let outcomes = decoder
                .recover_from_device_error(
                    &mut words,
                    &candidates,
                    HipError::OutOfMemory {
                        device_id: 0,
                        bytes_requested: 1 << 40,
                    },
                )
                .expect("an exhausted device falls back to the CPU path");

            assert_eq!(outcomes, expected);
            assert_eq!(words, expected_words);
            assert!(
                outcomes
                    .iter()
                    .any(|outcome| matches!(outcome, BchDecodeOutcome::Corrected { .. }))
                    && outcomes.contains(&BchDecodeOutcome::Uncorrectable),
                "the fallback covers both verification arms"
            );
        }

        /// The first syndrome pass fails before any candidate is applied.
        /// Neither arm alters the batch on its way: the fatal one returns it
        /// untouched, and the recoverable one decodes the caller's own words.
        #[test]
        fn a_first_pass_failure_never_alters_the_batch() {
            let code = narrow_sense(4, 0b10011, 5);
            let decoder = BinaryBchDecoder::new(&code);
            let frames = weight_three_words(code.n());
            let (expected, expected_words) = cpu_reference(&decoder, &frames);

            let mut words = frames.clone();
            let result = decoder.recover_from_device_error(&mut words, &[], HipError::NoDevice);
            assert!(matches!(result, Err(HipError::NoDevice)));
            assert_eq!(words, frames);

            let outcomes = decoder
                .recover_from_device_error(
                    &mut words,
                    &[],
                    HipError::UnsupportedArch {
                        gcn_arch_name: "gfx900".to_owned(),
                    },
                )
                .expect("an arch without a kernel blob falls back to the CPU path");
            assert_eq!(outcomes, expected);
            assert_eq!(words, expected_words);
        }
    }
}
