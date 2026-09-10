//! Correctness evidence for the polynomial-multiplication baseline survey
//! (jit:c7113c5a).
//!
//! Every timed cell has an entry here, and this binary runs to completion
//! before any timing. It checks coefficients, bit order, input lengths and
//! complete outputs of the gf2 and gf2x long products against a canonical
//! bit-by-bit definition of multiplication in GF(2)[x] that shares no code with
//! either library, then checks that the two libraries agree word for word. The
//! separated field-reduction stages are checked against long division.
//!
//! The canonical reference is the definition itself: the product of `a` and `b`
//! is the XOR of `b` shifted left by every bit position set in `a`. It uses no
//! carry-less multiply instruction, no gf2 kernel and no gf2x routine, so an
//! agreement is evidence rather than a tautology.
//!
//! Exits 0 when every check passes and 1 otherwise, and writes a machine
//! readable summary on stdout.

use serde::Serialize;
use std::process::ExitCode;
use tuning_campaign_support::abtest::SplitMix64;

use gf2_core::gf2m::{Gf2mWide, Gf2mWideConfig};
use poly_baseline_arms::gf2_backend::{schoolbook, Gf2Backend};
use poly_baseline_arms::wide_field::{WideReducer, GF2_256_MODULUS, GF2_571_MODULUS};
use poly_baseline_arms::{Backend, Bank, Case, DOT_FIELD_DEGREE, DOT_FIELD_POLY};

#[path = "../gf2x_backend.rs"]
mod gf2x_backend;

use gf2x_backend::{assert_pinned_library, mul as gf2x_mul, Gf2xPool};

/// GF(2^256) with the modulus the wide-field reduction probe uses.
struct Gf256;
impl Gf2mWideConfig<4> for Gf256 {
    const M: usize = 256;
    const MODULUS: [u64; 4] = GF2_256_MODULUS;
}

/// GF(2^571) with the modulus the wide-field reduction probe uses.
struct Gf571;
impl Gf2mWideConfig<9> for Gf571 {
    const M: usize = 571;
    const MODULUS: [u64; 9] = GF2_571_MODULUS;
}

/// Word counts the arms are instantiated for and this validator covers.
///
/// Every operand length any cell measures appears here, so no size is timed
/// before it is checked. The canonical reference is quadratic in the word count
/// and the largest two lengths dominate the validator's runtime, so they take
/// fewer random trials than the small lengths.
const LENGTHS: &[usize] = &[
    1, 2, 3, 4, 5, 8, 9, 16, 63, 64, 65, 127, 128, 256, 1024, 2048,
];

/// Random trials per length, reduced for the two lengths whose canonical
/// reference costs a fraction of a second per trial.
fn trials_for(words: usize) -> u32 {
    if words >= 1024 {
        2
    } else {
        8
    }
}

/// Bit positions exercised as single-term polynomials, covering the 0, 1, 63,
/// 64 and 65 word-boundary cases the engineering contract names.
const BIT_POSITIONS: &[usize] = &[0, 1, 62, 63, 64, 65, 126, 127, 128, 191, 255];

/// Canonical definition of multiplication in GF(2)[x].
///
/// `out` receives `a.len() + b.len()` words. Bit `i` of word `j` is the
/// coefficient of `x^(64j + i)`, so shifting `b` left by a set bit position of
/// `a` and accumulating with XOR is the definition of the product.
fn canonical_mul(a: &[u64], b: &[u64], out: &mut [u64]) {
    out.fill(0);
    for bit in 0..a.len() * 64 {
        if (a[bit >> 6] >> (bit & 63)) & 1 == 0 {
            continue;
        }
        let word_shift = bit >> 6;
        let bit_shift = bit & 63;
        for (index, value) in b.iter().enumerate() {
            out[word_shift + index] ^= value << bit_shift;
            if bit_shift != 0 {
                out[word_shift + index + 1] ^= value >> (64 - bit_shift);
            }
        }
    }
}

/// One polynomial with a single set coefficient at `bit`.
fn monomial(words: usize, bit: usize) -> Vec<u64> {
    let mut value = vec![0u64; words];
    value[bit >> 6] = 1u64 << (bit & 63);
    value
}

#[derive(Default, Serialize)]
struct Check {
    name: String,
    cases: u64,
    failures: Vec<String>,
}

impl Check {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }

    fn record(&mut self, passed: bool, detail: impl FnOnce() -> String) {
        self.cases += 1;
        if !passed && self.failures.len() < 16 {
            self.failures.push(detail());
        } else if !passed {
            self.failures.push("(further failures elided)".to_owned());
        }
    }

    fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    issue: &'static str,
    gf2_selected_paths: Vec<String>,
    gf2x_selected_path: String,
    checks: Vec<Check>,
    passed: bool,
}

/// Random operands of `words` words from a fixed stream.
fn operands(mixer: &mut SplitMix64, words: usize) -> (Vec<u64>, Vec<u64>) {
    (
        (0..words).map(|_| mixer.next_u64()).collect(),
        (0..words).map(|_| mixer.next_u64()).collect(),
    )
}

fn check_bit_order(pool: &mut Gf2xPool) -> Check {
    let mut check = Check::new("bit-order-and-coefficients");
    for &left_bit in BIT_POSITIONS {
        for &right_bit in BIT_POSITIONS {
            let words = (left_bit.max(right_bit) / 64) + 1;
            let Some(words) = LENGTHS.iter().copied().find(|length| *length >= words) else {
                continue;
            };
            let a = monomial(words, left_bit);
            let b = monomial(words, right_bit);
            let mut expected = vec![0u64; 2 * words];
            expected[(left_bit + right_bit) >> 6] = 1u64 << ((left_bit + right_bit) & 63);

            let mut gf2 = vec![0u64; 2 * words];
            schoolbook(words, &a, &b, &mut gf2);
            check.record(gf2 == expected, || {
                format!("gf2 x^{left_bit} * x^{right_bit} over {words} words")
            });

            let mut gf2x = vec![0u64; 2 * words];
            gf2x_mul(pool, &a, &b, &mut gf2x);
            check.record(gf2x == expected, || {
                format!("gf2x x^{left_bit} * x^{right_bit} over {words} words")
            });
        }
    }
    check
}

fn check_lengths(pool: &mut Gf2xPool) -> Check {
    let mut check = Check::new("lengths-and-complete-outputs");
    let mut mixer = SplitMix64::new(0x5CA1_AB1E_0C71_13C5);
    for &words in LENGTHS {
        let trials = trials_for(words);
        for trial in 0..trials {
            let (mut a, mut b) = operands(&mut mixer, words);
            // The final trial clears the top word of each operand so the top
            // product words are genuinely zero and a truncated write is caught.
            if trial == trials - 1 {
                a[words - 1] = 0;
                b[words - 1] = 0;
            }
            let mut expected = vec![0u64; 2 * words];
            canonical_mul(&a, &b, &mut expected);

            let mut gf2 = vec![0u64; 2 * words];
            schoolbook(words, &a, &b, &mut gf2);
            check.record(gf2 == expected, || {
                format!("gf2 schoolbook differs from the canonical product at {words} words, trial {trial}")
            });

            let mut gf2x = vec![0u64; 2 * words];
            gf2x_mul(pool, &a, &b, &mut gf2x);
            check.record(gf2x == expected, || {
                format!("gf2x differs from the canonical product at {words} words, trial {trial}")
            });
        }
    }
    check
}

fn check_dispatched_paths(pool: &mut Gf2xPool) -> (Check, Vec<String>) {
    let mut check = Check::new("dispatched-wide-kernels");
    let mut paths = Vec::new();
    let mut mixer = SplitMix64::new(0xD15E_A5E0_C711_3C5A);
    for &words in &[4usize, 9] {
        let case = Case::PolyMul {
            words,
            inner: 1,
            seed: 7,
        };
        let mut backend = Gf2Backend::create(&case);
        paths.push(backend.selected_path());
        for trial in 0..64u32 {
            let (a, b) = operands(&mut mixer, words);
            let mut expected = vec![0u64; 2 * words];
            canonical_mul(&a, &b, &mut expected);

            let mut bank = Bank {
                a: a.clone(),
                b: b.clone(),
                out: vec![0u64; 2 * words],
            };
            backend.run(&case, &mut bank);
            check.record(bank.out == expected, || {
                format!("dispatched gf2 kernel differs from the canonical product at {words} words, trial {trial}")
            });

            let mut gf2x = vec![0u64; 2 * words];
            gf2x_mul(pool, &a, &b, &mut gf2x);
            check.record(gf2x == bank.out, || {
                format!(
                    "gf2x and the dispatched gf2 kernel disagree at {words} words, trial {trial}"
                )
            });
        }
    }
    (check, paths)
}

fn check_clmul_batch(pool: &mut Gf2xPool) -> (Check, String) {
    let mut check = Check::new("independent-64x64-products");
    let count = 1024usize;
    let case = Case::ClmulBatch {
        count,
        inner: 1,
        seed: 11,
    };
    let mut backend = Gf2Backend::create(&case);
    let path = backend.selected_path();
    let mut mixer = SplitMix64::new(0xBA7C_4321_0C71_13C5);
    for trial in 0..4u32 {
        let (a, b) = operands(&mut mixer, count);
        let mut bank = Bank {
            a: a.clone(),
            b: b.clone(),
            out: vec![0u64; 2 * count],
        };
        backend.run(&case, &mut bank);
        for index in 0..count {
            let mut expected = [0u64; 2];
            canonical_mul(&a[index..index + 1], &b[index..index + 1], &mut expected);
            check.record(bank.out[2 * index..2 * index + 2] == expected, || {
                format!(
                    "gf2 batch product {index} differs from the canonical product, trial {trial}"
                )
            });
            let mut gf2x = [0u64; 2];
            gf2x_mul(pool, &a[index..index + 1], &b[index..index + 1], &mut gf2x);
            check.record(gf2x == expected, || {
                format!("gf2x product {index} differs from the canonical product, trial {trial}")
            });
        }
    }
    (check, path)
}

/// Canonical reduction of an unreduced product modulo `x^m + low(x)`: long
/// division clears every set coefficient at or above `m`, from the top down, by
/// XOR-ing in the modulus shifted to it. Returns the low `ceil(m / 64)` words.
fn canonical_reduce(product: &[u64], modulus_low: &[u64], m: usize) -> Vec<u64> {
    let low_bits: Vec<usize> = (0..m)
        .filter(|bit| (modulus_low[bit >> 6] >> (bit & 63)) & 1 == 1)
        .collect();
    let mut remainder = product.to_vec();
    for bit in (m..remainder.len() * 64).rev() {
        if (remainder[bit >> 6] >> (bit & 63)) & 1 == 0 {
            continue;
        }
        remainder[bit >> 6] ^= 1u64 << (bit & 63);
        for low in &low_bits {
            let target = bit - m + low;
            remainder[target >> 6] ^= 1u64 << (target & 63);
        }
    }
    remainder.truncate(m.div_ceil(64));
    remainder
}

/// The field product `Gf2mWide::mul_ref` returns for `words`-word elements.
fn gf2_field_product(words: usize, a: &[u64], b: &[u64]) -> Vec<u64> {
    match words {
        4 => {
            let a = Gf2mWide::<4, Gf256>::from_words(a.try_into().expect("4-word element"));
            let b = Gf2mWide::<4, Gf256>::from_words(b.try_into().expect("4-word element"));
            a.mul_ref(&b).words().to_vec()
        }
        9 => {
            let a = Gf2mWide::<9, Gf571>::from_words(a.try_into().expect("9-word element"));
            let b = Gf2mWide::<9, Gf571>::from_words(b.try_into().expect("9-word element"));
            a.mul_ref(&b).words().to_vec()
        }
        other => panic!("no wide field of {other} words"),
    }
}

/// The wide-field consumer split into its unreduced product and its
/// reduction: `Gf2mWide::mul_ref`, the dispatched gf2 kernel followed by the
/// probe's reducer, and gf2x followed by the same reducer must all equal the
/// canonical product reduced by long division. This is the decomposition the
/// 4-word and 9-word cells rely on when they time only the unreduced stage and
/// report the reduction separately.
fn check_wide_field_composition(pool: &mut Gf2xPool) -> Check {
    let mut check = Check::new("wide-field-composition");
    let mut mixer = SplitMix64::new(0x71DE_F1E1_D0C7_113C);
    for &words in &[4usize, 9] {
        let reducer = WideReducer::for_words(words).expect("a wide field of this width");
        let m = reducer.degree();
        let case = Case::PolyMul {
            words,
            inner: 1,
            seed: 17,
        };
        let mut backend = Gf2Backend::create(&case);
        let top = monomial(words, m - 1);
        let mut one = vec![0u64; words];
        one[0] = 1;
        let ones = reducer.element(&vec![u64::MAX; words]);
        let mut operand_pairs = vec![
            (top.clone(), top.clone()),
            (one, top.clone()),
            (ones.clone(), ones),
        ];
        for _ in 0..64 {
            let (a, b) = operands(&mut mixer, words);
            operand_pairs.push((reducer.element(&a), reducer.element(&b)));
        }
        for (trial, (a, b)) in operand_pairs.into_iter().enumerate() {
            let mut product = vec![0u64; 2 * words];
            canonical_mul(&a, &b, &mut product);
            let expected = canonical_reduce(&product, reducer.modulus(), m);

            let field = gf2_field_product(words, &a, &b);
            check.record(field == expected, || {
                format!("Gf2mWide::mul_ref differs from the canonical field product at {words} words, trial {trial}")
            });

            let mut bank = Bank {
                a: a.clone(),
                b: b.clone(),
                out: vec![0u64; 2 * words],
            };
            backend.run(&case, &mut bank);
            check.record(reducer.reduce(&bank.out) == expected, || {
                format!("dispatched gf2 product plus the probe reducer differs at {words} words, trial {trial}")
            });

            let mut gf2x = vec![0u64; 2 * words];
            gf2x_mul(pool, &a, &b, &mut gf2x);
            check.record(reducer.reduce(&gf2x) == expected, || {
                format!("gf2x product plus the probe reducer differs at {words} words, trial {trial}")
            });
        }
    }
    check
}

fn check_dot_product(pool: &mut Gf2xPool) -> Check {
    let mut check = Check::new("whole-consumer-dot-product");
    let count = 1024usize;
    let case = Case::Gf2mDot {
        count,
        inner: 1,
        seed: 13,
    };
    let mask = (1u64 << DOT_FIELD_DEGREE) - 1;
    let mut mixer = SplitMix64::new(0xD07_0C71_13C5_0001);
    for trial in 0..4u32 {
        let (a, b) = operands(&mut mixer, count);
        let mut gf2_bank = Bank {
            a: a.clone(),
            b: b.clone(),
            out: vec![0u64; 1],
        };
        let mut gf2 = Gf2Backend::create(&case);
        gf2.run(&case, &mut gf2_bank);

        let mut gf2x_bank = Bank {
            a: a.clone(),
            b: b.clone(),
            out: vec![0u64; 1],
        };
        let mut gf2x = gf2x_backend::Gf2xBackend::create(&case);
        gf2x.run(&case, &mut gf2x_bank);

        // Canonical arithmetic: accumulate the definition-level products, then
        // reduce by long division rather than by the Barrett path either arm
        // uses.
        let mut accumulator = [0u64; 2];
        for index in 0..count {
            let mut product = [0u64; 2];
            canonical_mul(&[a[index] & mask], &[b[index] & mask], &mut product);
            accumulator[0] ^= product[0];
            accumulator[1] ^= product[1];
        }
        let mut remainder = u128::from(accumulator[0]) | (u128::from(accumulator[1]) << 64);
        let modulus = u128::from(DOT_FIELD_POLY);
        for bit in (DOT_FIELD_DEGREE as u32..128).rev() {
            if (remainder >> bit) & 1 == 1 {
                remainder ^= modulus << (bit - DOT_FIELD_DEGREE as u32);
            }
        }
        let expected = remainder as u64;
        check.record(gf2_bank.out[0] == expected, || {
            format!("gf2 simd_dot_product differs from the canonical dot product, trial {trial}")
        });
        check.record(gf2x_bank.out[0] == expected, || {
            format!("the composed gf2x dot product differs from the canonical dot product, trial {trial}")
        });
        let _ = pool;
    }
    check
}

fn main() -> ExitCode {
    assert_pinned_library();
    let mut pool = Gf2xPool::default();
    let mut checks = vec![check_bit_order(&mut pool), check_lengths(&mut pool)];
    let (dispatched, mut paths) = check_dispatched_paths(&mut pool);
    checks.push(dispatched);
    let (batch, batch_path) = check_clmul_batch(&mut pool);
    checks.push(batch);
    paths.push(batch_path);
    checks.push(check_wide_field_composition(&mut pool));
    checks.push(check_dot_product(&mut pool));

    let passed = checks.iter().all(Check::passed);
    let report = Report {
        schema: "poly-baseline-validation-v2",
        issue: "c7113c5a",
        gf2_selected_paths: paths,
        gf2x_selected_path: gf2x_backend::selected_path(),
        checks,
        passed,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("the report serialises")
    );
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
