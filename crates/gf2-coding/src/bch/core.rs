//! Core BCH code types and algorithms.
//!
//! # Mathematical Background
//!
//! A BCH code is defined by its generator polynomial g(x), which has consecutive
//! powers of a primitive element α as roots in the extension field GF(2^m):
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

use crate::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::gf2m::{Gf2mElement, Gf2mField, Gf2mPoly};
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
    #[allow(clippy::type_complexity)]
    cached_generator: std::sync::Arc<std::sync::Mutex<Option<gf2_core::BitMatrix>>>,
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
        assert!(n < field.order(), "n must divide 2^m - 1");
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
            cached_generator: std::sync::Arc::new(std::sync::Mutex::new(None)),
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
        assert!(n < field.order(), "n must divide 2^m - 1");
        assert!(t > 0, "Error correction capability must be positive");

        let designed_distance = 2 * t + 1;

        Self {
            n,
            k,
            t,
            field,
            generator,
            designed_distance,
            cached_generator: std::sync::Arc::new(std::sync::Mutex::new(None)),
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
            g = Self::lcm_poly(&g, &m_i);
        }

        g
    }

    /// Computes LCM of two polynomials: lcm(a, b) = a*b / gcd(a, b)
    fn lcm_poly(a: &Gf2mPoly, b: &Gf2mPoly) -> Gf2mPoly {
        let gcd = Gf2mPoly::gcd(a, b);
        let product = a * b;
        let (quotient, remainder) = product.div_rem(&gcd);

        assert!(remainder.is_zero(), "Division must be exact for LCM");
        quotient
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
        let mut cache = self.cached_generator.lock().unwrap();
        if let Some(ref g) = *cache {
            g.clone()
        } else {
            let g = self.compute_generator_matrix();
            *cache = Some(g.clone());
            g
        }
    }

    fn is_systematic(&self) -> bool {
        true // BCH codes constructed here are always systematic
    }
}

/// Systematic encoder for BCH codes.
///
/// # Systematic Encoding Convention
///
/// **All BCH codes in this codebase use systematic encoding in [message | parity] format:**
/// - Bits 0..(k-1): Original message bits (unchanged)
/// - Bits k..(n-1): Computed parity bits
///
/// This matches the DVB-T2 standard and common convention for systematic codes.
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

        // Multiply by x^r to shift message left: x^r · m(x)
        let mut m_shifted_coeffs = vec![self.code.field.zero(); r];
        for i in 0..=m.degree().unwrap_or(0) {
            m_shifted_coeffs.push(m.coeff(i));
        }
        let m_shifted = Gf2mPoly::new(m_shifted_coeffs);

        // Compute parity: p(x) = remainder of (x^r · m(x)) / g(x)
        let (_, parity) = m_shifted.div_rem(&self.code.generator);

        // Codeword: c(x) = x^r · m(x) + p(x) = m_shifted + parity
        // Since we're in GF(2), addition is XOR
        let codeword_poly = &m_shifted + &parity;

        // Convert polynomial to systematic bitvec: [message | parity]
        // DVB-T2 convention: highest coefficient first
        let mut codeword = BitVec::new();

        // First k bits: message (from polynomial degrees k-1 down to 0 of m(x))
        // In codeword_poly, these are at degrees r+k-1 down to r
        for i in (r..self.code.n).rev() {
            codeword.push_bit(codeword_poly.coeff(i).is_one());
        }

        // Last r bits: parity (from polynomial degrees r-1 down to 0 of p(x))
        for i in (0..r).rev() {
            codeword.push_bit(codeword_poly.coeff(i).is_one());
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
        // TODO: Parallel implementation requires Gf2mField to use Arc instead of Rc
        // Currently sequential due to Rc<FieldParams> in Gf2mField not being Send+Sync
        // See: https://github.com/rust-lang/rust/issues/...
        received.iter().map(|cw| self.decode(cw)).collect()
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

        // Convert to polynomial
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

        // Batch evaluate r(x) at all syndrome points
        r_poly.eval_batch(&eval_points)
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
    use crate::traits::{BlockEncoder, HardDecisionDecoder};

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
            assert_eq!(
                dec, &messages[i],
                "Message {} not corrected properly",
                i
            );
        }
    }

    #[test]
    fn test_decode_batch_mixed_errors() {
        let field = Gf2mField::new(4, 0b10011).with_tables();
        let code = BchCode::new(15, 7, 2, field); // t=2 error correction
        let encoder = BchEncoder::new(code.clone());
        let decoder = BchDecoder::new(code);

        let messages: Vec<BitVec> = vec![
            BitVec::zeros(7),
            BitVec::ones(7),
            {
                let mut m = BitVec::zeros(7);
                m.set(0, true);
                m.set(3, true);
                m
            },
        ];

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
