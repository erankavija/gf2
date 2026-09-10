//! Shared generators, formatting, and correctness checks for the survey arms.

use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DesignedDistance};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::field::FiniteField;
use gf2_core::gf2m::Gf2mField;
use gf2_core::matrix::BitMatrix;
use std::io::{self, Write};
use std::time::Duration;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::timing::{execution_windows_configured, TimingSample, FIXTURE_BANKS};

/// Expands one seed into exactly `words` SplitMix64 outputs.
pub fn splitmix_words(words: usize, seed: u64) -> Vec<u64> {
    let mut mixer = SplitMix64::new(seed);
    (0..words).map(|_| mixer.next_u64()).collect()
}

/// Builds a row-major bit matrix from one SplitMix64 bit draw per matrix bit.
///
/// The draw order is row-major: the draw at flattened index `r * cols + c`
/// supplies bit `(r, c)`, and its low bit is the matrix value.
pub fn seeded_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut mixer = SplitMix64::new(seed);
    let mut matrix = BitMatrix::zeros(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            matrix.set(row, col, mixer.next_u64() & 1 == 1);
        }
    }
    matrix
}

/// Runs the canonical calibrated timing loop and normalizes its samples for
/// the child-v2 result shape.
pub fn timed_windows(
    windows: u32,
    window_target_ms: u32,
    mut body: impl FnMut(usize),
) -> io::Result<Vec<TimingSample>> {
    // `execution_windows_configured` rotates through this fixed bank count;
    // each arm has one deterministic workload, so the bank is only a protocol
    // index here rather than a second input-generation dimension.
    let banks = FIXTURE_BANKS;
    execution_windows_configured(
        0,
        u64::from(windows),
        Duration::from_millis(u64::from(window_target_ms)),
        &mut |bank| body(bank % banks),
        |_| Ok(()),
    )
}

/// Writes one packed word with its least-significant bit at the left edge.
pub fn write_word_bits(mut writer: impl Write, word: u64) -> io::Result<()> {
    for bit in 0..64 {
        writer.write_all(if word & (1u64 << bit) != 0 {
            b"1"
        } else {
            b"0"
        })?;
    }
    writer.write_all(b"\n")
}

/// Writes a `BitMatrix` one row per line, with bit 0 / column 0 first.
pub fn write_matrix_bits(mut writer: impl Write, matrix: &BitMatrix) -> io::Result<()> {
    for row in 0..matrix.rows() {
        for col in 0..matrix.cols() {
            writer.write_all(if matrix.get(row, col) { b"1" } else { b"0" })?;
        }
        writer.write_all(b"\n")?;
    }
    Ok(())
}

/// The four BCH rows required by the external M4RI generator parser.
#[derive(Clone, Copy, Debug)]
pub struct BchRow {
    /// External row name.
    pub name: &'static str,
    /// Extension degree.
    pub degree: usize,
    /// Primitive modulus in the same representation as the benchmark table.
    pub modulus: u64,
    /// Requested narrow-sense designed distance.
    pub designed_distance: u64,
}

/// The selected rows copied from `crates/gf2-coding/benches/bch_genmatrix.rs`
/// lines 46-79; `T2N-mother` is intentionally not part of this arm's dump.
pub const BCH_ROWS: &[BchRow] = &[
    BchRow {
        name: "B1",
        degree: 4,
        modulus: 0b1_0011,
        designed_distance: 7,
    },
    BchRow {
        name: "B2",
        degree: 7,
        modulus: 0b1000_0011,
        designed_distance: 21,
    },
    BchRow {
        name: "B3",
        degree: 8,
        modulus: 0b1_0001_1101,
        designed_distance: 9,
    },
    BchRow {
        name: "T2S-mother",
        degree: 14,
        modulus: 0b100_0000_0010_1011,
        designed_distance: 25,
    },
];

/// Builds one row with the exact `PrimitiveNarrowSense` construction used by
/// the source benchmark's `build()` function (lines 95-103).
pub fn build_bch(name: &str) -> Result<BinaryBchCode, String> {
    let row = BCH_ROWS
        .iter()
        .find(|row| row.name == name)
        .ok_or_else(|| format!("unknown BCH code {name:?}"))?;
    let extension = BinaryPrimeExt::new(Gf2mField::new(row.degree, row.modulus))
        .map_err(|error| format!("invalid extension for {name}: {error}"))?;
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(row.designed_distance)
            .map_err(|error| format!("invalid designed distance for {name}: {error}"))?,
    })
    .map_err(|error| format!("cannot construct {name}: {error}"))
}

/// Returns the generator polynomial as the parser's low-degree-first bit text.
pub fn generator_bits(code: &BinaryBchCode) -> String {
    let generator = code.generator();
    let degree = generator
        .degree()
        .expect("constructed BCH generator is nonzero");
    (0..=degree)
        .map(|index| {
            if generator
                .try_coeff(index)
                .is_some_and(|coefficient| coefficient.is_one())
            {
                '1'
            } else {
                '0'
            }
        })
        .collect()
}

/// Correctness checks shared by the standalone project tests.
#[cfg(test)]
mod tests {
    use super::*;
    use gf2_coding::test_support::bch_generator_matrix_by_encoding;
    use gf2_core::kernels::ops::xor_inplace;
    use gf2_kernels_simd::transpose;

    #[test]
    fn raw_transpose_matches_bit_matrix_transpose() {
        let input: [u64; 64] = splitmix_words(64, 0x0123_4567_89ab_cdef)
            .try_into()
            .expect("64 words");
        let mut raw_output = [0u64; 64];
        let fns = transpose::detect().expect("scalar transpose fallback exists");
        (fns.transpose_64x64)(&input, &mut raw_output);

        let mut matrix = BitMatrix::zeros(64, 64);
        for (row, word) in input.iter().copied().enumerate() {
            for col in 0..64 {
                matrix.set(row, col, word & (1u64 << col) != 0);
            }
        }
        let expected = matrix.transpose();
        for (row, word) in raw_output.iter().copied().enumerate() {
            assert_eq!(expected.row_words(row)[0], word);
        }
    }

    #[test]
    fn xor_inplace_matches_naive_bit_reference_at_boundaries() {
        for words in [1usize, 8, 16, 32, 63, 64, 65] {
            let src0 = splitmix_words(words, 0x1111_2222_3333_4444);
            let src1 = splitmix_words(words, 0x1111_2222_3333_4445);
            let mut actual = src0.clone();
            xor_inplace(&mut actual, &src1);

            let mut expected = vec![0u64; words];
            for word in 0..words {
                for bit in 0..64 {
                    let a = (src0[word] >> bit) & 1;
                    let b = (src1[word] >> bit) & 1;
                    expected[word] |= (a ^ b) << bit;
                }
            }
            assert_eq!(actual, expected, "words={words}");
        }
    }

    #[test]
    fn b1_encoded_generator_is_systematic() {
        let code = build_bch("B1").expect("B1 construction");
        let mut matrix = BitMatrix::zeros(code.k(), code.n());
        bch_generator_matrix_by_encoding(&code, &mut matrix).expect("generator matrix");
        for row in 0..code.k() {
            for col in 0..code.k() {
                assert_eq!(matrix.get(row, col), row == col, "row={row} col={col}");
            }
        }
    }
}
