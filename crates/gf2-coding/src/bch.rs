//! BCH (Bose-Chaudhuri-Hocquenghem) codes.
//!
//! BCH codes are a family of cyclic error-correcting codes that can correct
//! multiple random errors using algebraic decoding over extension fields GF(2^m).
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
//!
//! # DVB-T2 Support
//!
//! Factory methods provide standard DVB-T2 BCH codes:
//!
//! ```
//! use gf2_coding::bch::{BchCode, CodeRate};
//!
//! let code = BchCode::dvb_t2_normal(CodeRate::Rate1_2);
//! println!("DVB-T2 BCH: n={}, k={}, t={}", code.n(), code.k(), code.t());
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
    cached_generator: std::sync::Arc<std::sync::Mutex<Option<gf2_core::matrix::BitMatrix>>>,
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

    /// Creates DVB-T2 BCH code for normal frames (16200 bits).
    ///
    /// Uses GF(2^14) with primitive polynomial x^14 + x^5 + 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::{BchCode, CodeRate};
    ///
    /// let code = BchCode::dvb_t2_normal(CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 16200);
    /// assert_eq!(code.k(), 7200);
    /// assert_eq!(code.t(), 12);
    /// ```
    pub fn dvb_t2_normal(rate: CodeRate) -> Self {
        let field = Gf2mField::new(14, 0b100000000100001); // x^14 + x^5 + 1
        let (kbch, t) = Self::dvb_t2_normal_params(rate);

        Self::new(16200, kbch, t, field.with_tables())
    }

    /// Creates DVB-T2 BCH code for long frames (64800 bits).
    ///
    /// Uses GF(2^16) with primitive polynomial x^16 + x^5 + x^3 + x^2 + 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_coding::bch::{BchCode, CodeRate};
    ///
    /// let code = BchCode::dvb_t2_long(CodeRate::Rate1_2);
    /// assert_eq!(code.n(), 64800);
    /// assert_eq!(code.k(), 32400);
    /// assert_eq!(code.t(), 12);
    /// ```
    pub fn dvb_t2_long(rate: CodeRate) -> Self {
        let field = Gf2mField::new(16, 0b10000000000101101); // x^16 + x^5 + x^3 + x^2 + 1
        let (kbch, t) = Self::dvb_t2_long_params(rate);

        Self::new(64800, kbch, t, field.with_tables())
    }

    /// Returns (Kbch, t) for DVB-T2 normal frames.
    ///
    /// Parameters from ETSI EN 302 755 Table 6a.
    fn dvb_t2_normal_params(rate: CodeRate) -> (usize, usize) {
        match rate {
            CodeRate::Rate1_2 => (7200, 12),
            CodeRate::Rate3_5 => (9720, 12),
            CodeRate::Rate2_3 => (10800, 12),
            CodeRate::Rate3_4 => (11880, 12),
            CodeRate::Rate4_5 => (12600, 12),
            CodeRate::Rate5_6 => (13320, 12),
        }
    }

    /// Returns (Kbch, t) for DVB-T2 long frames.
    ///
    /// Parameters from ETSI EN 302 755 Table 6b.
    fn dvb_t2_long_params(rate: CodeRate) -> (usize, usize) {
        match rate {
            CodeRate::Rate1_2 => (32400, 12),
            CodeRate::Rate3_5 => (38880, 12),
            CodeRate::Rate2_3 => (43200, 12),
            CodeRate::Rate3_4 => (48600, 12),
            CodeRate::Rate4_5 => (51840, 12),
            CodeRate::Rate5_6 => (54000, 12),
        }
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
    fn compute_generator_matrix(&self) -> gf2_core::matrix::BitMatrix {
        use gf2_core::BitVec;

        let mut g = gf2_core::matrix::BitMatrix::zeros(self.k, self.n);

        for i in 0..self.k {
            // Create basis vector message
            let mut msg = BitVec::new();
            msg.resize(self.k, false);
            msg.set(i, true);

            // Encode using systematic encoding
            let codeword = self.encode_systematic(&msg);

            // Set row i of G
            for j in 0..self.n {
                g.set(i, j, codeword.get(j));
            }
        }

        g
    }

    /// Systematic encoding (extracted from BchEncoder for reuse).
    fn encode_systematic(&self, message: &BitVec) -> BitVec {
        use gf2_core::BitVec;

        let r = self.n - self.k;

        // Convert message to polynomial m(x)
        let m_coeffs: Vec<Gf2mElement> = (0..message.len())
            .map(|i| {
                if message.get(i) {
                    self.field.one()
                } else {
                    self.field.zero()
                }
            })
            .collect();
        let m = Gf2mPoly::new(m_coeffs);

        // Multiply by x^r to shift message left: x^r · m(x)
        let mut m_shifted_coeffs = vec![self.field.zero(); r];
        for i in 0..=m.degree().unwrap_or(0) {
            m_shifted_coeffs.push(m.coeff(i));
        }
        let m_shifted = Gf2mPoly::new(m_shifted_coeffs);

        // Compute parity: p(x) = remainder of (x^r · m(x)) / g(x)
        let (_, parity) = m_shifted.div_rem(&self.generator);

        // Codeword: c(x) = x^r · m(x) + p(x) = m_shifted + parity
        let codeword_poly = &m_shifted + &parity;

        // Convert polynomial to bitvec
        let mut codeword = BitVec::new();
        for i in 0..self.n {
            let coeff = codeword_poly.coeff(i);
            codeword.push_bit(coeff.is_one());
        }
        codeword
    }
}

impl crate::traits::GeneratorMatrixAccess for BchCode {
    fn k(&self) -> usize {
        self.k
    }

    fn n(&self) -> usize {
        self.n
    }

    fn generator_matrix(&self) -> gf2_core::matrix::BitMatrix {
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
/// Encoding algorithm:
/// 1. Treat message m(x) as polynomial over GF(2)
/// 2. Compute parity p(x) = remainder of x^r · m(x) divided by g(x)
/// 3. Codeword c(x) = x^r · m(x) + p(x) where r = n - k
///
/// In systematic form: [parity bits | message bits]
#[derive(Clone, Debug)]
pub struct BchEncoder {
    code: BchCode,
}

impl BchEncoder {
    /// Creates a new BCH encoder.
    pub fn new(code: BchCode) -> Self {
        Self { code }
    }

    /// Converts BitVec to polynomial over GF(2^m).
    ///
    /// Each bit becomes a coefficient (0 or 1) in the field.
    fn bitvec_to_poly(&self, bits: &BitVec) -> Gf2mPoly {
        let coeffs: Vec<Gf2mElement> = (0..bits.len())
            .map(|i| {
                if bits.get(i) {
                    self.code.field.one()
                } else {
                    self.code.field.zero()
                }
            })
            .collect();

        Gf2mPoly::new(coeffs)
    }

    /// Converts polynomial to BitVec.
    ///
    /// Only works for polynomials with binary coefficients (0 or 1).
    fn poly_to_bitvec(&self, poly: &Gf2mPoly, len: usize) -> BitVec {
        let mut bits = BitVec::new();

        for i in 0..len {
            let coeff = poly.coeff(i);
            bits.push_bit(coeff.is_one());
        }

        bits
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
    /// 3. Codeword c(x) = x^r · m(x) + p(x) = [p(x) | m(x)]
    ///
    /// In systematic form, the codeword consists of r parity bits followed by k message bits.
    fn encode(&self, message: &BitVec) -> BitVec {
        assert_eq!(
            message.len(),
            self.code.k,
            "Message must have length k = {}",
            self.code.k
        );

        let r = self.code.n - self.code.k;

        // Convert message to polynomial m(x)
        let m = self.bitvec_to_poly(message);

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

        self.poly_to_bitvec(&codeword_poly, self.code.n)
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

        // Convert received bits to polynomial
        let r = self.bitvec_to_poly(received);

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
        r.eval_batch(&eval_points)
    }

    /// Converts BitVec to polynomial over GF(2^m).
    fn bitvec_to_poly(&self, bits: &BitVec) -> Gf2mPoly {
        let coeffs: Vec<Gf2mElement> = (0..bits.len())
            .map(|i| {
                if bits.get(i) {
                    self.code.field.one()
                } else {
                    self.code.field.zero()
                }
            })
            .collect();

        Gf2mPoly::new(coeffs)
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
        let mut corrected = received.clone();
        for pos in error_positions {
            corrected.set(pos, !corrected.get(pos));
        }

        // Step 5: Extract message bits
        self.extract_message(&corrected)
    }
}

impl BchDecoder {
    /// Extracts message bits from systematic codeword.
    ///
    /// For systematic encoding [parity | message], extracts the
    /// message portion.
    fn extract_message(&self, codeword: &BitVec) -> BitVec {
        let r = self.code.n - self.code.k;
        let mut message = BitVec::new();

        for i in r..self.code.n {
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
    use gf2_core::matrix::BitMatrix;

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
        // (divisible by generator polynomial)
        for i in 0..code.k() {
            let mut row = BitVec::new();
            for j in 0..code.n() {
                row.push_bit(g.get(i, j));
            }

            // Convert to polynomial and check divisibility
            let row_coeffs: Vec<Gf2mElement> = (0..row.len())
                .map(|idx| {
                    if row.get(idx) {
                        code.field().one()
                    } else {
                        code.field().zero()
                    }
                })
                .collect();
            let row_poly = Gf2mPoly::new(row_coeffs);
            let (_, remainder) = row_poly.div_rem(code.generator());
            assert!(remainder.is_zero(), "Row {} of G must be valid codeword", i);
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
        let mut codeword2 = BitVec::new();
        for j in 0..code.n() {
            codeword2.push_bit(codeword2_matrix.get(0, j));
        }

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
}
