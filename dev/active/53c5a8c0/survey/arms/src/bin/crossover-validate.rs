//! Correctness validation for every arm path of the crossover study
//! (jit:53c5a8c0).
//!
//! Every measured path is checked against a canonical product that shares no
//! code with either library: the XOR of `b` shifted by each set bit of `a`,
//! computed on `u128` limbs here, and long division by the field modulus where
//! a reduced result is compared. The report names every executable and library
//! the validation used, by content digest, so a receipt's arms can be joined to
//! a validated build.
//!
//! Coverage: single-term operands at the word-boundary bit positions, random
//! operands at every measured length, the two crossover paths element by
//! element, the whole-consumer dot product and element-wise product, the
//! public long product at the repository's 0/1/63/64/65 word boundaries and at
//! every measured width, the whole-consumer wide-field product on both sides,
//! and the gf2x composition against the same oracle.
//!
//! Usage: crossover-validate <report.json>

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

use clmul_crossover_arms::gf2_backend::{long_product_into, wide_lane, Gf2Backend};
use clmul_crossover_arms::stages::StageFixture;
use clmul_crossover_arms::wide_field::WideReducer;
use clmul_crossover_arms::{
    banks, field_element_word, Backend, Bank, Case, FIELD_DEGREE, FIELD_POLY,
};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf2mElement, Gf2mField};
use gf2x_backend::Gf2xBackend;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Word counts the long-product checks cover.
const LENGTHS: [usize; 9] = [1, 2, 4, 9, 16, 32, 63, 64, 65];
/// Vector lengths the crossover checks cover, including the batch-kernel tail
/// boundaries.
const COUNTS: [usize; 9] = [1, 3, 4, 5, 8, 32, 63, 65, 1024];

/// The canonical unreduced product of two word-array polynomials.
///
/// Shifts `b` by every set bit of `a` and XORs; it calls neither library.
fn oracle(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; a.len() + b.len()];
    for (word_index, word) in a.iter().enumerate() {
        for bit in 0..64 {
            if (word >> bit) & 1 == 0 {
                continue;
            }
            let shift = 64 * word_index + bit;
            let (word_shift, bit_shift) = (shift / 64, shift % 64);
            for (index, value) in b.iter().enumerate() {
                out[word_shift + index] ^= value << bit_shift;
                if bit_shift != 0 {
                    out[word_shift + index + 1] ^= value >> (64 - bit_shift);
                }
            }
        }
    }
    out
}

/// The canonical GF(2^m) product of two single-word field elements, by the
/// bit-by-bit shift-and-reduce definition.
fn oracle_field(a: u64, b: u64, m: usize, poly: u64) -> u64 {
    let mut result = 0u64;
    let mut temp = a;
    for bit in 0..m {
        if (b >> bit) & 1 == 1 {
            result ^= temp;
        }
        let overflow = (temp >> (m - 1)) & 1 == 1;
        temp <<= 1;
        if overflow {
            temp ^= poly;
        }
    }
    result & ((1u64 << m) - 1)
}

/// Long division of an unreduced product by the wide modulus, leading term
/// implicit at bit `degree`.
fn oracle_reduce(product: &[u64], modulus: &[u64], degree: usize) -> Vec<u64> {
    let words = degree.div_ceil(64);
    let mut value = product.to_vec();
    let mut bit = 64 * value.len() - 1;
    loop {
        let set = (value[bit / 64] >> (bit % 64)) & 1 == 1;
        if set && bit >= degree {
            let shift = bit - degree;
            // XOR in modulus << shift, including its implicit leading term.
            for (index, word) in modulus.iter().enumerate().take(words) {
                let target = shift + 64 * index;
                let (word_shift, bit_shift) = (target / 64, target % 64);
                if word_shift < value.len() {
                    value[word_shift] ^= word << bit_shift;
                }
                if bit_shift != 0 && word_shift + 1 < value.len() {
                    value[word_shift + 1] ^= word >> (64 - bit_shift);
                }
            }
            value[bit / 64] ^= 1u64 << (bit % 64);
        }
        if bit == 0 {
            break;
        }
        bit -= 1;
    }
    value.truncate(words);
    value
}

fn sha256_file(path: &str) -> String {
    match std::fs::read(path) {
        Ok(bytes) => format!("{:x}", Sha256::digest(bytes)),
        Err(error) => format!("(unreadable: {error})"),
    }
}

struct Check {
    name: &'static str,
    cases: usize,
    failures: Vec<String>,
}

impl Check {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            cases: 0,
            failures: Vec::new(),
        }
    }

    fn assert(&mut self, condition: bool, detail: impl FnOnce() -> String) {
        self.cases += 1;
        if !condition {
            self.failures.push(detail());
        }
    }
}

fn crossover_bank(case: &Case) -> Bank {
    banks(case, 0, 1).pop().expect("one bank")
}

fn run_path(case: &Case, path: &str, bank: &mut Bank) -> Gf2Backend {
    std::env::set_var("GF2_CROSSOVER_PATH", path);
    let mut backend = Gf2Backend::create(case);
    backend.run(case, bank);
    backend
}

fn main() {
    let report_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: crossover-validate <report.json>");
        std::process::exit(2);
    });
    let mut checks: Vec<Check> = Vec::new();

    // 1. Raw carry-less batch: both crossover paths against the oracle.
    let mut check = Check::new("raw-batch-paths-match-the-oracle");
    for &count in &COUNTS {
        let case = Case::RawBatch {
            count,
            inner: 1,
            seed: 9001 + count as u64,
        };
        for path in ["element", "batch"] {
            let mut bank = crossover_bank(&case);
            let backend = run_path(&case, path, &mut bank);
            let _ = backend;
            // The backend keeps the products; re-derive them through the
            // extraction the conversion probe performs.
            let mut backend = {
                std::env::set_var("GF2_CROSSOVER_PATH", path);
                Gf2Backend::create(&case)
            };
            backend.run(&case, &mut bank);
            let conversion = backend.conversion(&case, &mut bank);
            let _ = conversion;
            for index in 0..count {
                let expected = oracle(&bank.a[index..index + 1], &bank.b[index..index + 1]);
                let got = [bank.out[2 * index], bank.out[2 * index + 1]];
                check.assert(got[0] == expected[0] && got[1] == expected[1], || {
                    format!("{path} raw batch count={count} index={index}")
                });
            }
        }
    }
    checks.push(check);

    // 2. Whole-consumer dot product: both paths against the oracle.
    let mut check = Check::new("dot-product-paths-match-the-oracle");
    let field = Gf2mField::new(FIELD_DEGREE, FIELD_POLY);
    for &count in &COUNTS {
        let case = Case::FieldDot {
            count,
            inner: 1,
            seed: 9100 + count as u64,
        };
        let mut expected = 0u64;
        {
            let bank = crossover_bank(&case);
            for index in 0..count {
                expected ^= oracle_field(
                    field_element_word(bank.a[index]),
                    field_element_word(bank.b[index]),
                    FIELD_DEGREE,
                    FIELD_POLY,
                );
            }
        }
        for path in ["element", "batch"] {
            let mut bank = crossover_bank(&case);
            run_path(&case, path, &mut bank);
            check.assert(bank.out[0] == expected, || {
                format!("{path} dot count={count}: {} != {expected}", bank.out[0])
            });
        }
    }
    checks.push(check);

    // 3. Whole-consumer element-wise product: both paths against the oracle.
    let mut check = Check::new("batch-mul-paths-match-the-oracle");
    for &count in &COUNTS {
        let case = Case::FieldBatchMul {
            count,
            inner: 1,
            seed: 9200 + count as u64,
        };
        for path in ["element", "batch"] {
            let mut bank = crossover_bank(&case);
            run_path(&case, path, &mut bank);
            for index in 0..count {
                let expected = oracle_field(
                    field_element_word(bank.a[index]),
                    field_element_word(bank.b[index]),
                    FIELD_DEGREE,
                    FIELD_POLY,
                );
                check.assert(bank.out[index] == expected, || {
                    format!("{path} batch-mul count={count} index={index}")
                });
            }
        }
    }
    checks.push(check);

    // 4. Long product: the gf2 public path and gf2x against the oracle,
    //    including single-term operands at the word boundaries.
    let mut check = Check::new("long-products-match-the-oracle");
    for &words in &LENGTHS {
        let case = Case::PolyMul {
            words,
            inner: 1,
            seed: 9300 + words as u64,
        };
        let mut gf2_bank = crossover_bank(&case);
        long_product_into(words, &gf2_bank.a.clone(), &gf2_bank.b.clone(), &mut gf2_bank.out);
        let expected = oracle(&gf2_bank.a, &gf2_bank.b);
        check.assert(gf2_bank.out == expected, || {
            format!("gf2 long product words={words}")
        });

        let mut gf2x_bank = crossover_bank(&case);
        let mut gf2x = Gf2xBackend::create(&case);
        gf2x.run(&case, &mut gf2x_bank);
        check.assert(gf2x_bank.out == expected, || {
            format!("gf2x long product words={words}")
        });

        // Single-term operands: x^(64j) times x^(64j), at every word boundary.
        for boundary in [0usize, 1, 63] {
            let mut a = vec![0u64; words];
            let mut b = vec![0u64; words];
            a[0] = 1u64 << boundary;
            b[words - 1] = 1u64 << boundary;
            let mut out = vec![0u64; 2 * words];
            long_product_into(words, &a, &b, &mut out);
            check.assert(out == oracle(&a, &b), || {
                format!("gf2 single term words={words} bit={boundary}")
            });
        }
    }
    checks.push(check);

    // 5. Whole-consumer wide field product on both sides against the oracle.
    let mut check = Check::new("wide-field-products-match-the-oracle");
    for &words in &[4usize, 9] {
        let case = Case::WideFieldMul {
            words,
            inner: 1,
            seed: 9400 + words as u64,
        };
        let reducer = WideReducer::for_words(words).expect("a configured wide field");
        let bank = crossover_bank(&case);
        let masked_a = reducer.element(&bank.a);
        let masked_b = reducer.element(&bank.b);
        let mut modulus = vec![0u64; words];
        modulus[0] = 0x425;
        let expected = oracle_reduce(&oracle(&masked_a, &masked_b), &modulus, reducer.degree());

        let mut gf2_bank = crossover_bank(&case);
        let mut gf2 = Gf2Backend::create(&case);
        gf2.run(&case, &mut gf2_bank);
        check.assert(gf2_bank.out[..words] == expected[..], || {
            format!("gf2 wide field words={words}")
        });

        let mut gf2x_bank = crossover_bank(&case);
        let mut gf2x = Gf2xBackend::create(&case);
        gf2x.run(&case, &mut gf2x_bank);
        check.assert(gf2x_bank.out[..words] == expected[..], || {
            format!("gf2x wide field words={words}")
        });
    }
    checks.push(check);

    // 6. The stage decomposition composes to the consumer's own result.
    let mut check = Check::new("stage-decomposition-composes-to-the-consumer");
    for &count in &[8usize, 64, 1024] {
        let case = Case::FieldDot {
            count,
            inner: 1,
            seed: 9500 + count as u64,
        };
        let bank = crossover_bank(&case);
        let fixture = StageFixture::new(count, &bank);
        check.assert(fixture.reconstructed_value() == fixture.consumer_value(), || {
            format!("stage reconstruction count={count}")
        });
        check.assert(fixture.accumulator() >> FIELD_DEGREE != 0, || {
            format!("stage accumulator already reduced at count={count}")
        });
        let vectors: Vec<Gf2mElement> = (0..count)
            .map(|index| field.element(field_element_word(bank.a[index])))
            .collect();
        check.assert(FieldVec::from(vectors).len() == count, || {
            format!("packed vector length count={count}")
        });
    }
    checks.push(check);

    let passed = checks.iter().all(|check| check.failures.is_empty());
    let mut executables = BTreeMap::new();
    for name in [
        "crossover-arm",
        "poly-arm",
        "gf2x-poly-arm",
        "stage-diagnostic",
        "crossover-validate",
    ] {
        let path = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        executables.insert(name.to_owned(), json!({"path": path, "sha256": sha256_file(&path)}));
    }
    let batch_lane = {
        std::env::set_var("GF2_CROSSOVER_PATH", "batch");
        let case = Case::RawBatch {
            count: 8,
            inner: 1,
            seed: 1,
        };
        Gf2Backend::create(&case).selected().to_owned()
    };
    let report = json!({
        "schema": "clmul-crossover-validation-v1",
        "issue": "53c5a8c0",
        "passed": passed,
        "checks": checks.iter().map(|check| json!({
            "name": check.name,
            "cases": check.cases,
            "failures": check.failures,
        })).collect::<Vec<_>>(),
        "executables": executables,
        "gf2x": {
            "library": gf2x_backend::loaded_library(),
            "sha256": gf2x_backend::loaded_library_sha256(),
            "selected_path": gf2x_backend::selected_path(),
        },
        "gf2_selected_paths": {
            "wide_4w": wide_lane(4),
            "wide_9w": wide_lane(9),
            "clmul_batch": batch_lane,
        },
    });
    std::fs::write(
        &report_path,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .expect("the validation report is writable");
    println!("{report_path}: passed={passed}");
    if !passed {
        std::process::exit(1);
    }
}
