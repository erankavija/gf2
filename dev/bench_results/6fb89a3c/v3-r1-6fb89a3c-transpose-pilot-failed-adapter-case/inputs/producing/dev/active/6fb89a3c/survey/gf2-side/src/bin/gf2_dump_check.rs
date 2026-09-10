//! Correctness-only dumps for cross-language comparison.

use gf2_coding::test_support::bch_generator_matrix_by_encoding;
use gf2_core::kernels::ops::xor_inplace;
use gf2_core::BitMatrix;
use gf2_kernels_simd::transpose;
use std::io::{self, BufWriter, Write};
use survey_gf2_side_6fb89a3c::{
    build_bch, seeded_matrix, splitmix_words, write_matrix_bits, write_word_bits,
};

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("gf2_dump_check: {message}");
    std::process::exit(2);
}

fn parse<T: std::str::FromStr>(value: Option<String>, name: &str) -> T
where
    T::Err: std::fmt::Display,
{
    value
        .unwrap_or_else(|| fail(format!("missing {name}")))
        .parse()
        .unwrap_or_else(|error| fail(format!("invalid {name}: {error}")))
}

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    let mode = args.next().unwrap_or_else(|| fail("missing dump mode"));
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    match mode.as_str() {
        "transpose64" => {
            let seed = parse(args.next(), "seed");
            let input: [u64; 64] = splitmix_words(64, seed).try_into().expect("64 words");
            let fns = transpose::detect().unwrap_or(transpose::TransposeFns {
                transpose_64x64: transpose::transpose_64x64_scalar,
                name: "scalar-bit-twiddle",
            });
            let mut output = [0u64; 64];
            (fns.transpose_64x64)(&input, &mut output);
            for word in output {
                write_word_bits(&mut writer, word).unwrap_or_else(|error| fail(error));
            }
        }
        "transpose-tiled" => {
            let rows: usize = parse(args.next(), "rows");
            let cols: usize = parse(args.next(), "cols");
            let seed = parse(args.next(), "seed");
            // The dump contract names the output shape: print `rows` lines
            // of `cols` bits.  Build the inverse-shaped input so the generic
            // transpose produces exactly that declared output shape.
            let output = seeded_matrix(cols, rows, seed).transpose();
            write_matrix_bits(&mut writer, &output).unwrap_or_else(|error| fail(error));
        }
        "logical-xor" => {
            let words: usize = parse(args.next(), "words");
            let seed = parse(args.next(), "seed");
            let src0 = splitmix_words(words, seed);
            let src1 = splitmix_words(words, seed.wrapping_add(1));
            let mut output = src0;
            xor_inplace(&mut output, &src1);
            for word in output {
                for bit in 0..64 {
                    writer
                        .write_all(if word & (1u64 << bit) != 0 {
                            b"1"
                        } else {
                            b"0"
                        })
                        .unwrap_or_else(|error| fail(error));
                }
            }
            writer.write_all(b"\n").unwrap_or_else(|error| fail(error));
        }
        "bch-genmatrix" => {
            let name = args.next().unwrap_or_else(|| fail("missing code name"));
            let code = build_bch(&name).unwrap_or_else(|error| fail(error));
            let mut output = BitMatrix::zeros(code.k(), code.n());
            bch_generator_matrix_by_encoding(&code, &mut output)
                .unwrap_or_else(|error| fail(error));
            write_matrix_bits(&mut writer, &output).unwrap_or_else(|error| fail(error));
        }
        _ => fail("usage: transpose64|transpose-tiled|logical-xor|bch-genmatrix ..."),
    }
    writer.flush().unwrap_or_else(|error| fail(error));
}
