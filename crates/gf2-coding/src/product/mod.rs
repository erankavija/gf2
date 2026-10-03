//! Product code construction and iterative block turbo decoder
//! (`@/citation/Pyndiah1998`).
//!
//! A component (n, k) code encodes the rows and then the columns of a k x k
//! information matrix, giving an (n^2, k^2) product code.  [`TurboDecoder`]
//! iterates between row and column SISO decoding with [`SoGrand`] or
//! [`BcjrDecoder`] components; [`ChasePyndiahDecoder`] uses Chase-Pyndiah
//! components.

pub mod chase_pyndiah;

pub use chase_pyndiah::{ChasePyndiahConfig, ChasePyndiahDecoder};

use crate::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use crate::bcjr::BcjrDecoder;
use crate::grand::{OneLineIntercept, OrbGrand, OrbGrandConfig, SisoResult, SoGrand};
use crate::llr::Llr;
use crate::traits::block::{
    BlockCode as CanonicalBlockCode, BlockEncoder as CanonicalBlockEncoder, ParityCheckMatrixAccess,
};
use crate::traits::BlockEncoder;
use crate::transform::Extended;
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::gf2m::Gf2mField;
use gf2_core::{BitMatrix, BitVec};

enum SisoEngine {
    SoGrand(SoGrand),
    Bcjr(BcjrDecoder),
    #[cfg(feature = "hip")]
    GpuBcjr(gf2_kernels_hip::GpuBcjrBatch),
}

/// Product-code component adapter for an extended canonical binary BCH code.
///
/// Product decoding needs a borrowed parity-check matrix, so this adapter
/// materializes the [`ParityCheckMatrixAccess`] result of [`Extended`] once.
#[derive(Debug, Clone)]
pub struct ExtendedBchComponent {
    code: Extended<BinaryBchCode>,
    parity_check: BitMatrix,
}

impl ExtendedBchComponent {
    fn from_parameters(degree: usize, primitive_polynomial: u64, designed_distance: u64) -> Self {
        let extension =
            BinaryPrimeExt::new(Gf2mField::new(degree, primitive_polynomial).with_tables())
                .expect("a valid binary BCH extension");
        let base = BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
            extension,
            designed_distance: DesignedDistance::try_from(designed_distance)
                .expect("a positive BCH designed distance"),
        })
        .expect("a valid binary BCH construction");

        let code = Extended::new(base).expect("an extended BCH code fits in memory");
        let parity_check =
            ParityCheckMatrixAccess::parity_check_matrix(&code).expect("BCH parity matrix");

        Self { code, parity_check }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn into_code_for_test(self) -> Extended<BinaryBchCode> {
        self.code
    }

    /// Creates the canonical eBCH(16,11) product component.
    pub fn ebch_16_11() -> Self {
        Self::from_parameters(4, 0b10011, 3)
    }

    /// Creates the canonical eBCH(16,7) product component.
    pub fn ebch_16_7() -> Self {
        Self::from_parameters(4, 0b10011, 5)
    }

    /// Creates the canonical eBCH(32,26) product component.
    pub fn ebch_32_26() -> Self {
        Self::from_parameters(5, 0b100101, 3)
    }

    /// Creates the canonical eBCH(64,57) product component.
    pub fn ebch_64_57() -> Self {
        Self::from_parameters(6, 0b1000011, 3)
    }
}

impl CanonicalBlockCode for ExtendedBchComponent {
    type Symbol = <Extended<BinaryBchCode> as CanonicalBlockCode>::Symbol;
    type Symbols = <Extended<BinaryBchCode> as CanonicalBlockCode>::Symbols;

    fn symbol_zero(&self) -> Self::Symbol {
        CanonicalBlockCode::symbol_zero(&self.code)
    }

    fn k(&self) -> usize {
        CanonicalBlockCode::k(&self.code)
    }

    fn n(&self) -> usize {
        CanonicalBlockCode::n(&self.code)
    }
}

impl CanonicalBlockEncoder for ExtendedBchComponent {
    fn encode_into(
        &self,
        message: &Self::Symbols,
        codeword: &mut Self::Symbols,
    ) -> Result<(), crate::CodeError> {
        CanonicalBlockEncoder::encode_into(&self.code, message, codeword)
    }
}

impl SisoEngine {
    fn decode_siso(&self, input: &[Llr]) -> SisoResult {
        match self {
            SisoEngine::SoGrand(s) => s.decode_siso(input),
            SisoEngine::Bcjr(b) => b.decode_siso(input),
            #[cfg(feature = "hip")]
            SisoEngine::GpuBcjr(gpu) => {
                let llrs: Vec<f32> = input.iter().map(|l| l.value()).collect();
                let (app, ext) = gpu.decode_batch(&[llrs]).expect("GPU decode failed");
                SisoResult {
                    app_llrs: app[0].iter().map(|&v| Llr::new(v)).collect(),
                    extrinsic_llrs: ext[0].iter().map(|&v| Llr::new(v)).collect(),
                    list_bler_prediction: 0.0,
                    query_count: 0,
                }
            }
        }
    }

    /// Batch-decode multiple inputs in one GPU launch. Falls back to serial
    /// decode_siso for non-GPU engines.
    fn decode_siso_batch(&self, inputs: &[Vec<f32>]) -> Vec<SisoResult> {
        match self {
            #[cfg(feature = "hip")]
            SisoEngine::GpuBcjr(gpu) => {
                let (app_batch, ext_batch) =
                    gpu.decode_batch(inputs).expect("GPU batch decode failed");
                app_batch
                    .into_iter()
                    .zip(ext_batch)
                    .map(|(app, ext)| SisoResult {
                        app_llrs: app.into_iter().map(Llr::new).collect(),
                        extrinsic_llrs: ext.into_iter().map(Llr::new).collect(),
                        list_bler_prediction: 0.0,
                        query_count: 0,
                    })
                    .collect()
            }
            _ => {
                // Serial fallback for CPU engines
                inputs
                    .iter()
                    .map(|llrs| {
                        let input: Vec<Llr> = llrs.iter().map(|&v| Llr::new(v)).collect();
                        self.decode_siso(&input)
                    })
                    .collect()
            }
        }
    }

    fn is_gpu(&self) -> bool {
        #[cfg(feature = "hip")]
        if matches!(self, SisoEngine::GpuBcjr(_)) {
            return true;
        }
        false
    }

    /// Returns `true` if the SISO engine produces
    /// [`SisoResult::list_bler_prediction`] values: SOGRAND does, while BCJR
    /// and GPU-BCJR always return 0.0.
    #[allow(dead_code)]
    fn has_list_bler_prediction(&self) -> bool {
        matches!(self, SisoEngine::SoGrand(_))
    }
}

/// Trait abstracting a component code for use in product code constructions.
pub trait ProductComponent: BlockEncoder {
    /// Returns the codeword length of the component code.
    fn comp_n(&self) -> usize;

    /// Returns the message length of the component code.
    fn comp_k(&self) -> usize;

    /// Returns `true` if all codewords have even Hamming weight.
    ///
    /// This flag enables ORBGRAND's even-code optimization, which skips
    /// parity-mismatched noise patterns.
    fn comp_is_even(&self) -> bool;

    /// Returns the (n - k) x n parity-check matrix H.
    fn comp_parity_check(&self) -> &BitMatrix;
}

impl ProductComponent for ExtendedBchComponent {
    fn comp_n(&self) -> usize {
        CanonicalBlockCode::n(self)
    }

    fn comp_k(&self) -> usize {
        CanonicalBlockCode::k(self)
    }

    fn comp_is_even(&self) -> bool {
        true
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        &self.parity_check
    }
}

impl ProductComponent for crate::crc::CrcCode {
    fn comp_n(&self) -> usize {
        self.n()
    }

    fn comp_k(&self) -> usize {
        self.k()
    }

    fn comp_is_even(&self) -> bool {
        self.is_even()
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        self.parity_check()
    }
}

impl ProductComponent for crate::drm::DrmCode {
    fn comp_n(&self) -> usize {
        self.n()
    }

    fn comp_k(&self) -> usize {
        self.k()
    }

    fn comp_is_even(&self) -> bool {
        self.is_even()
    }

    fn comp_parity_check(&self) -> &BitMatrix {
        self.parity_check()
    }
}

/// A product code constructed from a component (n, k) linear block code.
///
/// The product code has parameters (n^2, k^2) and is formed by encoding rows
/// and columns of a k x k information matrix with the component code.
#[derive(Debug, Clone)]
pub struct ProductCode<C: ProductComponent> {
    component: C,
    comp_n: usize,
    comp_k: usize,
}

impl<C: ProductComponent> ProductCode<C> {
    /// Creates a product code from the given component code.
    pub fn new(component: C) -> Self {
        let comp_n = component.comp_n();
        let comp_k = component.comp_k();
        Self {
            component,
            comp_n,
            comp_k,
        }
    }

    /// Returns the product code codeword length (n^2).
    pub fn n(&self) -> usize {
        self.comp_n * self.comp_n
    }

    /// Returns the product code message length (k^2).
    pub fn k(&self) -> usize {
        self.comp_k * self.comp_k
    }

    /// Returns a reference to the component code.
    pub fn component(&self) -> &C {
        &self.component
    }

    /// Encodes k^2 row-major message bits into the row-major n x n product
    /// codeword: each row of the k x k message matrix is encoded, then each
    /// column of the resulting k x n matrix.
    ///
    /// # Panics
    ///
    /// Panics if `message.len() != k^2`.
    ///
    /// # Complexity
    ///
    /// k row encodings plus n column encodings of the component code, and
    /// O(n^2) bit copies.
    pub fn encode_product(&self, message: &BitVec) -> BitVec {
        let k = self.comp_k;
        let n = self.comp_n;
        assert_eq!(
            message.len(),
            k * k,
            "Message length {} must equal k^2 = {}",
            message.len(),
            k * k
        );

        let mut row_encoded = BitMatrix::zeros(k, n);
        for i in 0..k {
            let mut row_msg = BitVec::with_capacity(k);
            for j in 0..k {
                row_msg.push_bit(message.get(i * k + j));
            }
            let row_cw = self.component.encode(&row_msg);
            for j in 0..n {
                row_encoded.set(i, j, row_cw.get(j));
            }
        }

        let mut codeword_matrix = BitMatrix::zeros(n, n);
        for j in 0..n {
            let mut col = BitVec::with_capacity(k);
            for i in 0..k {
                col.push_bit(row_encoded.get(i, j));
            }
            let col_cw = self.component.encode(&col);
            for i in 0..n {
                codeword_matrix.set(i, j, col_cw.get(i));
            }
        }

        let mut result = BitVec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                result.push_bit(codeword_matrix.get(i, j));
            }
        }
        result
    }

    /// Checks whether a given n x n bit matrix is a valid product codeword.
    ///
    /// Verifies that every row and every column has zero syndrome under the
    /// component code's parity-check matrix.
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    ///
    /// # Complexity
    ///
    /// O(n^2 * r) where r = n - k is the number of parity checks per component.
    pub fn is_valid_codeword(&self, matrix: &BitMatrix) -> bool {
        let n = self.comp_n;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let h = self.component.comp_parity_check();

        for i in 0..n {
            let mut row = BitVec::with_capacity(n);
            for j in 0..n {
                row.push_bit(matrix.get(i, j));
            }
            let syn = h.matvec(&row);
            if syn.count_ones() > 0 {
                return false;
            }
        }

        for j in 0..n {
            let mut col = BitVec::with_capacity(n);
            for i in 0..n {
                col.push_bit(matrix.get(i, j));
            }
            let syn = h.matvec(&col);
            if syn.count_ones() > 0 {
                return false;
            }
        }

        true
    }

    /// Converts a row-major flat codeword vector to an n x n matrix.
    ///
    /// # Panics
    ///
    /// Panics if `flat.len() != n^2`.
    pub fn flat_to_matrix(&self, flat: &BitVec) -> BitMatrix {
        let n = self.comp_n;
        assert_eq!(
            flat.len(),
            n * n,
            "Flat vector length {} must equal n^2 = {}",
            flat.len(),
            n * n
        );
        let mut matrix = BitMatrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                matrix.set(i, j, flat.get(i * n + j));
            }
        }
        matrix
    }

    /// Converts an n x n matrix to a flat codeword vector (row-major).
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    pub fn matrix_to_flat(&self, matrix: &BitMatrix) -> BitVec {
        let n = self.comp_n;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let mut result = BitVec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                result.push_bit(matrix.get(i, j));
            }
        }
        result
    }

    /// Extracts message bits from a valid product codeword matrix.
    ///
    /// For a systematic code, the message bits occupy the top-left k x k submatrix.
    ///
    /// # Panics
    ///
    /// Panics if `matrix` dimensions are not n x n.
    pub fn extract_message(&self, matrix: &BitMatrix) -> BitVec {
        let n = self.comp_n;
        let k = self.comp_k;
        assert_eq!(matrix.rows(), n);
        assert_eq!(matrix.cols(), n);
        let mut msg = BitVec::with_capacity(k * k);
        for i in 0..k {
            for j in 0..k {
                msg.push_bit(matrix.get(i, j));
            }
        }
        msg
    }
}

impl<C: ProductComponent> BlockEncoder for ProductCode<C> {
    fn k(&self) -> usize {
        self.comp_k * self.comp_k
    }

    fn n(&self) -> usize {
        self.comp_n * self.comp_n
    }

    fn encode(&self, message: &BitVec) -> BitVec {
        self.encode_product(message)
    }
}

/// Configuration for the iterative block turbo decoder.
#[derive(Debug, Clone)]
pub struct TurboDecoderConfig {
    /// Maximum number of row-column iteration pairs.
    pub max_iterations: usize,

    /// Extrinsic information scaling factor.
    ///
    /// The scaled extrinsic LLRs `alpha * L_E` are fed as a-priori
    /// information to the next decoder step.
    pub alpha: f32,

    /// ORBGRAND list size for the component SISO decoder.
    pub list_size: usize,

    /// Maximum ORBGRAND queries per component decode.
    pub max_queries: usize,

    /// Per-component list-BLER early-stop threshold for SOGRAND
    /// (`@/citation/Yuan2025`).
    ///
    /// Each SOGRAND component decode receives this value as
    /// [`crate::grand::OrbGrandConfig::list_bler_stop_threshold`].  The
    /// turbo loop itself terminates only when all rows and columns are
    /// valid codewords (`@/citation/Yuan2025` § V, step 1).
    ///
    /// This threshold is ignored in BCJR and GPU-BCJR modes.
    pub list_bler_threshold: Option<f64>,

    /// Final alpha value for iteration-dependent scaling schedule.
    ///
    /// When set, alpha moves linearly from `alpha` (initial) to
    /// `alpha_final` over `max_iterations`.  When `None`, a fixed `alpha`
    /// is used for all iterations.
    pub alpha_final: Option<f32>,

    /// Maximum absolute extrinsic LLR value `beta`.
    ///
    /// Bounds the extrinsic information to `[-beta, +beta]`. When `None`,
    /// extrinsic is unbounded.
    pub extrinsic_clamp: Option<f32>,

    /// Disable early termination on valid product codeword.
    ///
    /// When `true`, the decoder always runs all `max_iterations` regardless
    /// of whether a valid product codeword is found.
    pub no_early_termination: bool,

    /// Use Pyndiah-style extrinsic extraction.
    ///
    /// When `true`, the extrinsic is computed as `L_E = L_APP - L_Ch` (subtracting
    /// only the channel LLR, not the a-priori), the formula of
    /// `@/citation/Pyndiah1998`.
    ///
    /// When `false` (default), `L_E = L_APP - L_Ch - L_A`.
    pub pyndiah_extrinsic: bool,

    /// Use BCJR trellis decoder instead of SOGRAND for component SISO.
    ///
    /// When `true`, the turbo decoder uses a forward-backward (BCJR) algorithm
    /// on the code trellis for exact APP LLR computation. The `list_size` and
    /// `max_queries` fields are ignored in BCJR mode.
    pub use_bcjr: bool,

    /// Use GPU-accelerated batch BCJR via HIP/ROCm.
    ///
    /// When `true`, the turbo decoder batches all row (or column) SISO calls
    /// into a single GPU kernel launch. Requires the `hip` feature and an AMD
    /// GPU with ROCm. Takes precedence over `use_bcjr`.
    #[cfg(feature = "hip")]
    pub use_gpu_bcjr: bool,
}

impl Default for TurboDecoderConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            alpha: 0.5,
            list_size: 4,
            max_queries: 1_000_000,
            list_bler_threshold: None,
            alpha_final: None,
            extrinsic_clamp: None,
            no_early_termination: false,
            pyndiah_extrinsic: false,
            use_bcjr: false,
            #[cfg(feature = "hip")]
            use_gpu_bcjr: false,
        }
    }
}

/// Result of a turbo decoding operation.
#[derive(Debug, Clone)]
pub struct TurboDecoderResult {
    /// The decoded message bits (length k^2).
    pub decoded_bits: BitVec,

    /// Number of row-column iteration pairs performed.
    pub iterations: usize,

    /// Whether the decoder converged to a valid product codeword.
    pub converged: bool,

    /// Total number of ORBGRAND queries across all component decodes.
    pub total_queries: usize,

    /// Average number of ORBGRAND queries per information bit:
    /// `total_queries / k^2` for component message length k.
    pub queries_per_bit: f64,
}

impl From<TurboDecoderResult> for crate::traits::DecoderResult {
    fn from(t: TurboDecoderResult) -> Self {
        crate::traits::DecoderResult {
            decoded_bits: t.decoded_bits,
            iterations: t.iterations,
            converged: t.converged,
            syndrome_check_passed: t.converged,
            queries: Some(t.total_queries),
        }
    }
}

/// Iterative block turbo decoder using SOGRAND or BCJR as the component SISO decoder.
///
/// The turbo decoder alternates between row-wise and column-wise SISO decoding
/// of `L_Ch + L_A`, feeding `L_A = alpha * L_E` to the next step.  It
/// terminates when the hard-decision matrix forms a valid product codeword;
/// [`TurboDecoderConfig::list_bler_threshold`] acts only inside each component
/// ORBGRAND decode.
pub struct TurboDecoder<C: ProductComponent> {
    component: C,
    config: TurboDecoderConfig,
    siso: SisoEngine,
    product_code: ProductCode<C>,
}

impl<C: ProductComponent + Clone> TurboDecoder<C> {
    /// Creates a turbo decoder for the given component code, with the SISO
    /// engine (SOGRAND, BCJR, or GPU-BCJR) selected by `config`.
    pub fn new(component: C, config: TurboDecoderConfig) -> Self {
        #[cfg(feature = "hip")]
        let use_gpu = config.use_gpu_bcjr;
        #[cfg(not(feature = "hip"))]
        let use_gpu = false;

        let siso = if use_gpu {
            #[cfg(feature = "hip")]
            {
                let h = component.comp_parity_check();
                let h_cols = gf2_kernels_hip::extract_h_cols(h);
                let n = component.comp_n();
                let k = component.comp_k();
                SisoEngine::GpuBcjr(
                    gf2_kernels_hip::GpuBcjrBatch::new(&h_cols, n, k, n)
                        .expect("Failed to initialize GPU BCJR"),
                )
            }
            #[cfg(not(feature = "hip"))]
            unreachable!()
        } else if config.use_bcjr {
            SisoEngine::Bcjr(BcjrDecoder::new(component.comp_parity_check()))
        } else {
            let h = component.comp_parity_check().clone();
            let orb_config = OrbGrandConfig {
                list_size: config.list_size,
                max_queries: config.max_queries,
                even_code: component.comp_is_even(),
                systematic: true,
                list_bler_stop_threshold: config.list_bler_threshold,
                one_line_intercept: OneLineIntercept::Auto,
            };
            let orbgrand = OrbGrand::new(h, orb_config);
            SisoEngine::SoGrand(SoGrand::new(orbgrand))
        };
        let product_code = ProductCode::new(component.clone());
        Self {
            component,
            config,
            siso,
            product_code,
        }
    }

    /// Decodes a received product codeword from `n^2` row-major channel LLRs,
    /// where a positive LLR favours bit 0.
    ///
    /// # Panics
    ///
    /// Panics if `channel_llrs.len() != n^2`.
    /// Panics if any LLR has a NaN magnitude and at least one iteration runs.
    ///
    /// # Complexity
    ///
    /// O(I * n * S) where I is the number of iteration pairs, n is the
    /// component code length, and S is the per-component SISO cost.
    pub fn decode(&self, channel_llrs: &[Llr]) -> TurboDecoderResult {
        let n = self.component.comp_n();
        let k = self.component.comp_k();
        let n_sq = n * n;
        assert_eq!(
            channel_llrs.len(),
            n_sq,
            "Channel LLR length {} must equal n^2 = {}",
            channel_llrs.len(),
            n_sq
        );

        let l_ch: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| channel_llrs[i * n + j].value()).collect())
            .collect();

        let mut l_a: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
        let mut total_queries: usize = 0;

        for iteration in 0..self.config.max_iterations {
            let mut l_app_row: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            if self.siso.is_gpu() {
                let row_inputs: Vec<Vec<f32>> = (0..n)
                    .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect())
                    .collect();
                let results = self.siso.decode_siso_batch(&row_inputs);
                for (i, siso_result) in results.into_iter().enumerate() {
                    total_queries += siso_result.query_count;
                    for (j, app_llr) in siso_result.app_llrs.iter().enumerate() {
                        l_app_row[i][j] = app_llr.value();
                    }
                }
            } else {
                for i in 0..n {
                    let input: Vec<Llr> =
                        (0..n).map(|j| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                    let siso_result = self.siso.decode_siso(&input);
                    total_queries += siso_result.query_count;
                    for (j, app_llr) in siso_result.app_llrs.iter().enumerate() {
                        l_app_row[i][j] = app_llr.value();
                    }
                }
            }

            let mut l_e: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            for i in 0..n {
                for j in 0..n {
                    let mut ext = if self.config.pyndiah_extrinsic {
                        l_app_row[i][j] - l_ch[i][j]
                    } else {
                        l_app_row[i][j] - l_a[i][j] - l_ch[i][j]
                    };
                    if let Some(beta) = self.config.extrinsic_clamp {
                        ext = ext.clamp(-beta, beta);
                    }
                    l_e[i][j] = ext;
                }
            }

            if !self.config.no_early_termination && self.check_early_termination(&l_app_row) {
                let decoded = self.extract_decoded_message(&l_app_row);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries,
                    queries_per_bit: total_queries as f64 / (k * k) as f64,
                };
            }

            let alpha = if let Some(a_final) = self.config.alpha_final {
                let t = iteration as f32 / (self.config.max_iterations - 1).max(1) as f32;
                self.config.alpha + t * (a_final - self.config.alpha)
            } else {
                self.config.alpha
            };
            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha * l_e[i][j];
                }
            }

            let mut l_app_col: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
            if self.siso.is_gpu() {
                let col_inputs: Vec<Vec<f32>> = (0..n)
                    .map(|j| (0..n).map(|i| l_ch[i][j] + l_a[i][j]).collect())
                    .collect();
                let results = self.siso.decode_siso_batch(&col_inputs);
                for (j, siso_result) in results.into_iter().enumerate() {
                    total_queries += siso_result.query_count;
                    for (i, app_llr) in siso_result.app_llrs.iter().enumerate() {
                        l_app_col[i][j] = app_llr.value();
                    }
                }
            } else {
                for j in 0..n {
                    let input: Vec<Llr> =
                        (0..n).map(|i| Llr::new(l_ch[i][j] + l_a[i][j])).collect();
                    let siso_result = self.siso.decode_siso(&input);
                    total_queries += siso_result.query_count;
                    for (i, app_llr) in siso_result.app_llrs.iter().enumerate() {
                        l_app_col[i][j] = app_llr.value();
                    }
                }
            }

            for i in 0..n {
                for j in 0..n {
                    let mut ext = if self.config.pyndiah_extrinsic {
                        l_app_col[i][j] - l_ch[i][j]
                    } else {
                        l_app_col[i][j] - l_a[i][j] - l_ch[i][j]
                    };
                    if let Some(beta) = self.config.extrinsic_clamp {
                        ext = ext.clamp(-beta, beta);
                    }
                    l_e[i][j] = ext;
                }
            }

            if !self.config.no_early_termination && self.check_early_termination(&l_app_col) {
                let decoded = self.extract_decoded_message(&l_app_col);
                return TurboDecoderResult {
                    decoded_bits: decoded,
                    iterations: iteration + 1,
                    converged: true,
                    total_queries,
                    queries_per_bit: total_queries as f64 / (k * k) as f64,
                };
            }

            for i in 0..n {
                for j in 0..n {
                    l_a[i][j] = alpha * l_e[i][j];
                }
            }
        }

        let final_llrs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..n).map(|j| l_ch[i][j] + l_a[i][j]).collect())
            .collect();
        let decoded = self.extract_decoded_message(&final_llrs);

        TurboDecoderResult {
            decoded_bits: decoded,
            iterations: self.config.max_iterations,
            converged: false,
            total_queries,
            queries_per_bit: total_queries as f64 / (k * k) as f64,
        }
    }

    /// Checks if the hard decision on the given LLR matrix forms a valid product codeword.
    fn check_early_termination(&self, llr_matrix: &[Vec<f32>]) -> bool {
        let n = self.component.comp_n();
        let mut matrix = BitMatrix::zeros(n, n);
        for (i, row) in llr_matrix.iter().enumerate().take(n) {
            for (j, &val) in row.iter().enumerate().take(n) {
                if val < 0.0 {
                    matrix.set(i, j, true);
                }
            }
        }
        self.product_code.is_valid_codeword(&matrix)
    }

    /// Extracts k^2 decoded message bits from the hard decision on an LLR matrix.
    ///
    /// For a systematic code, the message bits are in the top-left k x k submatrix.
    fn extract_decoded_message(&self, llr_matrix: &[Vec<f32>]) -> BitVec {
        let k = self.component.comp_k();
        let mut msg = BitVec::with_capacity(k * k);
        for row in llr_matrix.iter().take(k) {
            for &val in row.iter().take(k) {
                msg.push_bit(val < 0.0);
            }
        }
        msg
    }

    /// Returns a reference to the underlying SOGRAND decoder.
    ///
    /// # Panics
    ///
    /// Panics unless the SISO engine is SOGRAND.
    pub fn sogrand(&self) -> &SoGrand {
        match &self.siso {
            SisoEngine::SoGrand(s) => s,
            _ => panic!("decoder not configured with SOGRAND"),
        }
    }

    /// Returns a reference to the underlying BCJR decoder.
    ///
    /// # Panics
    ///
    /// Panics unless the SISO engine is the CPU BCJR decoder.
    pub fn bcjr(&self) -> &BcjrDecoder {
        match &self.siso {
            SisoEngine::Bcjr(b) => b,
            _ => panic!("decoder not configured with BCJR"),
        }
    }

    /// Returns the turbo decoder configuration.
    pub fn config(&self) -> &TurboDecoderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crc::CrcCode;
    use crate::product::ExtendedBchComponent;
    use crate::traits::BlockEncoder;

    /// Whether some `size` columns of `columns`, starting at `from`, sum with
    /// `partial` to zero.
    fn has_zero_sum(columns: &[u64], from: usize, size: usize, partial: u64) -> bool {
        if size == 0 {
            return partial == 0;
        }
        (from..columns.len())
            .any(|index| has_zero_sum(columns, index + 1, size - 1, partial ^ columns[index]))
    }

    /// The minimum distance of the code `parity_check` defines, searched up
    /// to `limit`: the fewest columns summing to zero.
    fn minimum_distance(parity_check: &BitMatrix, limit: usize) -> Option<usize> {
        assert!(parity_check.rows() <= 64, "a column fits one word");
        let columns: Vec<u64> = (0..parity_check.cols())
            .map(|column| {
                (0..parity_check.rows())
                    .filter(|&row| parity_check.get(row, column))
                    .fold(0u64, |mask, row| mask | (1 << row))
            })
            .collect();
        (1..=limit).find(|&size| has_zero_sum(&columns, 0, size, 0))
    }

    #[test]
    fn every_extended_bch_component_has_its_designed_distance_plus_one() {
        for (name, component, distance) in [
            ("eBCH(16,11)", ExtendedBchComponent::ebch_16_11(), 4),
            ("eBCH(16,7)", ExtendedBchComponent::ebch_16_7(), 6),
            ("eBCH(32,26)", ExtendedBchComponent::ebch_32_26(), 4),
            ("eBCH(64,57)", ExtendedBchComponent::ebch_64_57(), 4),
        ] {
            assert_eq!(
                minimum_distance(component.comp_parity_check(), distance),
                Some(distance),
                "{name}"
            );
        }
    }

    #[test]
    fn test_product_code_parameters_16_11() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 256);
        assert_eq!(product.k(), 121);
        assert_eq!(product.component().comp_n(), 16);
        assert_eq!(product.component().comp_k(), 11);
    }

    #[test]
    fn test_product_code_parameters_16_7() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 256);
        assert_eq!(product.k(), 49);
    }

    #[test]
    fn test_product_code_parameters_crc_25_15() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        assert_eq!(product.n(), 625);
        assert_eq!(product.k(), 225);
        assert_eq!(product.component().comp_n(), 25);
        assert_eq!(product.component().comp_k(), 15);
    }

    #[test]
    fn test_product_code_block_encoder_trait() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let encoder: &dyn BlockEncoder = &product;
        assert_eq!(encoder.k(), 121);
        assert_eq!(encoder.n(), 256);
    }

    #[test]
    fn test_encode_all_zeros() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_produces_valid_codeword() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(5, true);
        msg.set(10, true);

        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());

        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "Encoded product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_systematic_message_recovery() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);

        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(3) {
            msg.set(i, true);
        }

        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(
            recovered, msg,
            "Extracted message must match original for systematic code"
        );
    }

    #[test]
    fn test_encode_all_ones_message() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::ones(product.k());
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "All-ones message should produce a valid product codeword"
        );
    }

    #[test]
    #[should_panic(expected = "Message length")]
    fn test_encode_wrong_message_length_panics() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(100);
        product.encode(&msg);
    }

    #[test]
    fn test_encode_ebch_16_7_all_zeros() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_ebch_16_7_produces_valid_codeword() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(3, true);
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "eBCH(16,7) product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_ebch_16_7_systematic_roundtrip() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(2) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(recovered, msg, "eBCH(16,7) systematic roundtrip must match");
    }

    #[test]
    fn test_encode_crc_25_15_all_zeros() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let msg = BitVec::zeros(product.k());
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), product.n());
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_encode_crc_25_15_produces_valid_codeword() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(7, true);
        msg.set(14, true);
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        assert!(
            product.is_valid_codeword(&matrix),
            "CRC(25,15) product codeword must be valid"
        );
    }

    #[test]
    fn test_encode_crc_25_15_systematic_roundtrip() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component);
        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(3) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);
        let matrix = product.flat_to_matrix(&cw);
        let recovered = product.extract_message(&matrix);
        assert_eq!(recovered, msg, "CRC(25,15) systematic roundtrip must match");
    }

    #[test]
    fn test_is_valid_codeword_all_zeros() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let matrix = BitMatrix::zeros(16, 16);
        assert!(product.is_valid_codeword(&matrix));
    }

    #[test]
    fn test_is_valid_codeword_invalid() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let mut matrix = BitMatrix::zeros(16, 16);
        matrix.set(0, 0, true); // single bit flip invalidates both row 0 and col 0
        assert!(!product.is_valid_codeword(&matrix));
    }

    #[test]
    fn test_flat_to_matrix_roundtrip() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);

        let mut flat = BitVec::zeros(256);
        flat.set(0, true);
        flat.set(17, true); // row 1, col 1
        flat.set(255, true);

        let matrix = product.flat_to_matrix(&flat);
        assert!(matrix.get(0, 0));
        assert!(matrix.get(1, 1));
        assert!(matrix.get(15, 15));

        let recovered = product.matrix_to_flat(&matrix);
        assert_eq!(recovered, flat);
    }

    #[test]
    #[should_panic(expected = "Flat vector length")]
    fn test_flat_to_matrix_wrong_length_panics() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component);
        let flat = BitVec::zeros(100);
        product.flat_to_matrix(&flat);
    }

    #[test]
    fn test_turbo_config_default() {
        let config = TurboDecoderConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!((config.alpha - 0.5).abs() < 1e-10);
        assert_eq!(config.list_size, 4);
        assert_eq!(config.max_queries, 1_000_000);
        assert!(config.list_bler_threshold.is_none());
        assert!(!config.use_bcjr);
    }

    #[test]
    fn test_turbo_config_list_bler_threshold() {
        let config = TurboDecoderConfig {
            list_bler_threshold: Some(1e-6),
            ..TurboDecoderConfig::default()
        };
        assert_eq!(config.list_bler_threshold, Some(1e-6));
    }

    #[test]
    fn test_turbo_decoder_construction() {
        let component = ExtendedBchComponent::ebch_16_11();
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);
        assert_eq!(decoder.sogrand().n(), 16);
        assert_eq!(decoder.config().max_iterations, 5);
    }

    #[test]
    fn test_turbo_decoder_bcjr_construction() {
        let component = ExtendedBchComponent::ebch_16_11();
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);
        assert_eq!(decoder.bcjr().n(), 16);
        assert_eq!(decoder.bcjr().k(), 11);
    }

    #[test]
    fn test_decode_bcjr_all_zeros_high_snr() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all zeros"
        );
        // BCJR reports 0 queries (trellis-based, not query-based)
        assert_eq!(result.total_queries, 0);
    }

    #[test]
    fn test_decode_bcjr_nonzero_message_high_snr() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(1, true);
        msg.set(10, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR should converge for high-SNR nonzero message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "BCJR decoded message should match input"
        );
    }

    #[test]
    fn test_decode_bcjr_drm_32_21_high_snr() {
        use crate::drm::DrmCode;

        let component = DrmCode::drm_32_21();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 10,
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "BCJR+dRM should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "dRM decoded message should be all zeros"
        );
    }

    #[test]
    fn test_decode_all_zeros_high_snr() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged, "Should converge for high-SNR all-zeros");
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(
            result.decoded_bits.count_ones(),
            0,
            "Decoded message should be all zeros"
        );
        assert!(result.total_queries > 0, "Should have performed queries");
        assert!(
            result.queries_per_bit > 0.0,
            "queries_per_bit should be positive"
        );
    }

    #[test]
    fn test_decode_nonzero_message_high_snr() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(1, true);
        msg.set(10, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "Should converge for high-SNR encoded message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "Decoded message must match original"
        );
    }

    #[test]
    fn test_decode_tracks_iteration_count() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 3,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.iterations >= 1);
        assert!(result.iterations <= 3);
    }

    #[test]
    fn test_decode_early_termination() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());

        let config = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged);
        assert!(
            result.iterations < 20,
            "Should terminate early for strong signal, used {} iterations",
            result.iterations
        );
    }

    #[test]
    #[should_panic(expected = "Channel LLR length")]
    fn test_decode_wrong_llr_length_panics() {
        let component = ExtendedBchComponent::ebch_16_11();
        let config = TurboDecoderConfig::default();
        let decoder = TurboDecoder::new(component, config);
        let llrs: Vec<Llr> = vec![Llr::new(1.0); 100];
        decoder.decode(&llrs);
    }

    #[test]
    fn test_decode_queries_increase_with_noise() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 3,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs_high: Vec<Llr> = vec![Llr::new(10.0); product.n()];
        let result_high = decoder.decode(&llrs_high);

        let llrs_low: Vec<Llr> = vec![Llr::new(2.0); product.n()];
        let result_low = decoder.decode(&llrs_low);

        assert!(
            result_low.total_queries >= result_high.total_queries,
            "Lower SNR should require at least as many queries: high={}, low={}",
            result_high.total_queries,
            result_low.total_queries,
        );
    }

    #[test]
    fn test_queries_per_bit_computed_correctly() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 3,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        let expected = result.total_queries as f64 / product.k() as f64;
        assert!(
            (result.queries_per_bit - expected).abs() < 1e-10,
            "queries_per_bit should equal total_queries / k^2: got {}, expected {}",
            result.queries_per_bit,
            expected
        );
    }

    #[test]
    fn test_list_bler_threshold_early_termination() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());

        let config_no_thresh = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            list_bler_threshold: None,
            ..TurboDecoderConfig::default()
        };
        let decoder_no_thresh = TurboDecoder::new(component.clone(), config_no_thresh);

        let config_with_thresh = TurboDecoderConfig {
            max_iterations: 20,
            list_size: 2,
            max_queries: 10_000,
            list_bler_threshold: Some(0.5),
            ..TurboDecoderConfig::default()
        };
        let decoder_with_thresh = TurboDecoder::new(component, config_with_thresh);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];

        let result_no = decoder_no_thresh.decode(&llrs);
        let result_with = decoder_with_thresh.decode(&llrs);

        assert!(result_no.converged);
        assert!(result_with.converged);

        assert!(
            result_with.iterations <= result_no.iterations,
            "BLER threshold should enable equal or earlier termination: \
             with={}, without={}",
            result_with.iterations,
            result_no.iterations,
        );
    }

    #[test]
    fn test_bcjr_ignores_list_bler_threshold() {
        // BCJR always returns list_bler_prediction=0.0. A threshold check
        // must be skipped in BCJR mode, otherwise any threshold > 0 would
        // trigger immediate early termination after the first half-iteration.
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());

        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            list_bler_threshold: Some(0.5),
            use_bcjr: true,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(3.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(result.converged, "BCJR should converge at moderate SNR");
    }

    #[test]
    fn test_decode_ebch_16_7_all_zeros_high_snr() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "eBCH(16,7) should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.queries_per_bit > 0.0);
    }

    #[test]
    fn test_decode_ebch_16_7_nonzero_message() {
        let component = ExtendedBchComponent::ebch_16_7();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(3, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "eBCH(16,7) should converge for high-SNR message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "eBCH(16,7) decoded must match original"
        );
    }

    #[test]
    fn test_decode_crc_25_15_all_zeros_high_snr() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let llrs: Vec<Llr> = vec![Llr::new(5.0); product.n()];
        let result = decoder.decode(&llrs);

        assert!(
            result.converged,
            "CRC(25,15) should converge for high-SNR all-zeros"
        );
        assert_eq!(result.decoded_bits.len(), product.k());
        assert_eq!(result.decoded_bits.count_ones(), 0);
        assert!(result.queries_per_bit > 0.0);
    }

    #[test]
    fn test_decode_crc_25_15_nonzero_message() {
        let component = CrcCode::crc_25_15();
        let product = ProductCode::new(component.clone());
        let config = TurboDecoderConfig {
            max_iterations: 5,
            list_size: 2,
            max_queries: 10_000,
            ..TurboDecoderConfig::default()
        };
        let decoder = TurboDecoder::new(component, config);

        let mut msg = BitVec::zeros(product.k());
        msg.set(0, true);
        msg.set(7, true);
        msg.set(14, true);
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                if cw.get(i) {
                    Llr::new(-5.0)
                } else {
                    Llr::new(5.0)
                }
            })
            .collect();

        let result = decoder.decode(&llrs);
        assert!(
            result.converged,
            "CRC(25,15) should converge for high-SNR message"
        );
        assert_eq!(
            result.decoded_bits, msg,
            "CRC(25,15) decoded must match original"
        );
    }

    #[test]
    fn test_ber_improves_over_iterations() {
        let component = ExtendedBchComponent::ebch_16_11();
        let product = ProductCode::new(component.clone());

        let mut msg = BitVec::zeros(product.k());
        for i in (0..product.k()).step_by(5) {
            msg.set(i, true);
        }
        let cw = product.encode(&msg);

        let llrs: Vec<Llr> = (0..cw.len())
            .map(|i| {
                let base = if cw.get(i) { -2.0_f32 } else { 2.0 };
                // Add systematic perturbation (not random, for reproducibility)
                let perturbation = if (i * 7 + 3) % 11 < 3 { -0.5 } else { 0.0 };
                Llr::new(base + perturbation)
            })
            .collect();

        let mut prev_ber = f64::MAX;
        for max_iter in 1..=5 {
            let config = TurboDecoderConfig {
                max_iterations: max_iter,
                list_size: 2,
                max_queries: 10_000,
                ..TurboDecoderConfig::default()
            };
            let decoder = TurboDecoder::new(component.clone(), config);
            let result = decoder.decode(&llrs);

            let mut bit_errors = 0;
            for i in 0..product.k() {
                if result.decoded_bits.get(i) != msg.get(i) {
                    bit_errors += 1;
                }
            }
            let ber = bit_errors as f64 / product.k() as f64;

            assert!(
                ber <= prev_ber + 1e-10,
                "BER should not increase with more iterations: \
                 iter={}, BER={}, prev_BER={}",
                max_iter,
                ber,
                prev_ber
            );
            prev_ber = ber;
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::crc::CrcCode;
    use crate::product::ExtendedBchComponent;
    use crate::traits::BlockEncoder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_encode_produces_valid_codeword_and_roundtrips(
            msg_bits in prop::collection::vec(any::<bool>(), 121)
        ) {
            let component = ExtendedBchComponent::ebch_16_11();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "Encoded codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "Extracted message must match original");
        }

        #[test]
        fn prop_ebch_16_7_encode_roundtrip(
            msg_bits in prop::collection::vec(any::<bool>(), 49)
        ) {
            let component = ExtendedBchComponent::ebch_16_7();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "eBCH(16,7) codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "eBCH(16,7) message must roundtrip");
        }

        #[test]
        fn prop_crc_25_15_encode_roundtrip(
            msg_bits in prop::collection::vec(any::<bool>(), 225)
        ) {
            let component = CrcCode::crc_25_15();
            let product = ProductCode::new(component);
            let mut msg = BitVec::new();
            for bit in msg_bits {
                msg.push_bit(bit);
            }
            let cw = product.encode(&msg);
            let matrix = product.flat_to_matrix(&cw);
            prop_assert!(product.is_valid_codeword(&matrix), "CRC(25,15) codeword must be valid");
            let extracted = product.extract_message(&matrix);
            prop_assert_eq!(extracted, msg, "CRC(25,15) message must roundtrip");
        }
    }
}

#[cfg(test)]
mod additional_component_tests {
    use super::*;
    use crate::drm::DrmCode;
    use crate::product::ExtendedBchComponent;
    use crate::traits::BlockEncoder;

    #[test]
    fn test_product_code_parameters_ebch_32_26() {
        let comp = ExtendedBchComponent::ebch_32_26();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 32 * 32);
        assert_eq!(product.k(), 26 * 26);
    }

    #[test]
    fn test_encode_ebch_32_26_all_zeros() {
        let comp = ExtendedBchComponent::ebch_32_26();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(26 * 26);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 32 * 32);
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_product_code_parameters_ebch_64_57() {
        let comp = ExtendedBchComponent::ebch_64_57();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 64 * 64);
        assert_eq!(product.k(), 57 * 57);
    }

    #[test]
    fn test_encode_ebch_64_57_all_zeros() {
        let comp = ExtendedBchComponent::ebch_64_57();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(57 * 57);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 64 * 64);
        assert_eq!(cw.count_ones(), 0);
    }

    #[test]
    fn test_product_code_parameters_drm_32_21() {
        let comp = DrmCode::drm_32_21();
        let product = ProductCode::new(comp);
        assert_eq!(product.n(), 32 * 32);
        assert_eq!(product.k(), 21 * 21);
    }

    #[test]
    fn test_encode_drm_32_21_all_zeros() {
        let comp = DrmCode::drm_32_21();
        let product = ProductCode::new(comp);
        let msg = BitVec::zeros(21 * 21);
        let cw = product.encode(&msg);
        assert_eq!(cw.len(), 32 * 32);
        assert_eq!(cw.count_ones(), 0);
    }
}
