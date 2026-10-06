//! Operation-equivalence validation for every optimization arm (jit:5cbb6545).
//!
//! Without arguments it checks every arm against an independent byte-table
//! count over a matrix of sizes, word offsets, bit patterns and seeds, the
//! byte-length tails of the external kernels, gf2's bit-length tail semantics
//! through `BitVec::count_ones`, the fused AND arms, the matrix-vector
//! consumer arms against a bit-by-bit parity reference, and Mula's alignment
//! precondition, and prints the runtime selections it observes. With
//! `--plan <plan.json>` it checks every arm of every planned cell on the exact
//! case the runner will send. Any mismatch exits nonzero.

use count_optimization_survey::arms::{legacy_route, resolved_and_route, resolved_route};
use count_optimization_survey::external::{self, libpopcnt_capabilities};
use count_optimization_survey::{AndArm, Fixture, MatvecArm, Pattern, PopcountArm};
use gf2_core::{BitMatrix, BitVec};
use serde_json::Value;
use std::process::ExitCode;
use tuning_campaign_support::abtest::SplitMix64;

/// Word counts covering the empty buffer, every sub-vector width, the
/// carry-save block boundary at 64 words and the cache regimes the families
/// declare.
const SIZES: &[usize] = &[
    0, 1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 13, 15, 16, 17, 31, 32, 33, 59, 60, 61, 63, 64, 65, 66, 95,
    96, 97, 127, 128, 129, 191, 192, 193, 255, 256, 257, 1023, 1024, 1025, 4096, 16384,
];
const STREAMING_SIZE: usize = 1 << 20;
const SEEDS: &[u64] = &[0, 1, 0x0123_4567_89ab_cdef];
const BIT_LENGTHS: &[usize] = &[
    0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024, 1025, 4095,
    4096, 4097, 65535, 65536, 65537,
];
/// Matrix shapes whose strides bracket the routes the consumer can take.
const MATVEC_SHAPES: &[(usize, usize)] = &[(1, 64), (5, 448), (9, 512), (17, 4096), (3, 4161)];

/// Independent reference: a 256-entry table built bit by bit.
struct Reference([u8; 256]);

impl Reference {
    fn new() -> Self {
        let mut table = [0_u8; 256];
        for (byte, count) in table.iter_mut().enumerate() {
            *count = (0..8).filter(|bit| byte >> bit & 1 == 1).count() as u8;
        }
        Self(table)
    }

    fn bytes(&self, bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .map(|byte| u64::from(self.0[usize::from(*byte)]))
            .sum()
    }

    fn words(&self, words: &[u64]) -> u64 {
        words
            .iter()
            .map(|word| self.bytes(&word.to_le_bytes()))
            .sum()
    }
}

#[derive(Default)]
struct Tally {
    cases: usize,
    failures: Vec<String>,
}

impl Tally {
    fn check(&mut self, label: impl Fn() -> String, observed: u64, expected: u64) {
        self.cases += 1;
        if observed != expected {
            self.failures
                .push(format!("{}: observed {observed}, expected {expected}", label()));
        }
    }
}

fn patterns() -> [(Pattern, &'static str); 3] {
    [
        (Pattern::Random, "random"),
        (Pattern::AllZero, "all_zero"),
        (Pattern::AllOne, "all_one"),
    ]
}

/// Every population-count arm over the size, offset, pattern and seed matrix.
/// Returns the number of windows an arm refused for misalignment.
fn word_matrix(reference: &Reference, tally: &mut Tally) -> Result<usize, String> {
    let mut refused = 0;
    for &words in SIZES {
        for offset in 0..4 {
            for (pattern, pattern_name) in patterns() {
                for &seed in SEEDS {
                    let fixture = Fixture::new(words, seed, pattern, offset)?;
                    let expected = reference.words(fixture.words());
                    for arm in PopcountArm::ALL {
                        if !arm.accepts_misalignment(fixture.misalignment_bytes()) {
                            refused += 1;
                            continue;
                        }
                        let op = arm.resolve()?;
                        // SAFETY: the alignment precondition was checked above
                        // and the arm resolved on this host.
                        let observed = unsafe { op(fixture.words()) };
                        tally.check(
                            || {
                                format!(
                                    "{} words={words} offset={offset} {pattern_name} seed={seed}",
                                    arm.name()
                                )
                            },
                            observed,
                            expected,
                        );
                    }
                }
            }
        }
    }
    Ok(refused)
}

/// The external kernels over byte lengths shorter than one vector, where a
/// word-granular entry point cannot express the tail.
fn byte_tails(reference: &Reference, tally: &mut Tally) -> Result<(), String> {
    let fixture = Fixture::new(16, 7, Pattern::Random, 0)?;
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            fixture.words().as_ptr().cast::<u8>(),
            fixture.words().len() * 8,
        )
    };
    for length in 0..bytes.len() {
        let expected = reference.bytes(&bytes[..length]);
        tally.check(
            || format!("libpopcnt bytes={length}"),
            external::libpopcnt_bytes(&bytes[..length]),
            expected,
        );
        // SAFETY: the fixture's base is vector-aligned and the host was
        // checked for AVX2 before this binary resolved any Mula arm.
        let mula = unsafe { external::mula_bytes(&bytes[..length]) };
        tally.check(|| format!("mula bytes={length}"), mula, expected);
    }
    Ok(())
}

/// gf2's bit-length tail semantics: `BitVec::from_words` requires the padding
/// beyond the declared length to be zero, so a count over whole words equals
/// the count over canonical bit indices.
fn bit_lengths(reference: &Reference, tally: &mut Tally) -> Result<(), String> {
    for &bits in BIT_LENGTHS {
        let mut generator = SplitMix64::new(0xfeed);
        let mut words: Vec<u64> = (0..bits.div_ceil(64)).map(|_| generator.next_u64()).collect();
        if bits % 64 != 0 {
            if let Some(last) = words.last_mut() {
                *last &= (1_u64 << (bits % 64)) - 1;
            }
        }
        let vector = BitVec::from_words(words, bits);
        let expected = reference.words(vector.words());
        tally.check(
            || format!("BitVec::count_ones bits={bits}"),
            vector.count_ones() as u64,
            expected,
        );
        let set_expected = (0..bits).filter(|index| vector.get(*index)).count() as u64;
        tally.check(
            || format!("canonical bit indexing bits={bits}"),
            set_expected,
            expected,
        );
    }
    Ok(())
}

/// Every fused arm over the same matrix, including unequal operand lengths.
fn and_matrix(tally: &mut Tally) -> Result<(), String> {
    for &words in SIZES {
        for offset in 0..4 {
            for (pattern, pattern_name) in patterns() {
                let lhs = Fixture::new(words, 11, pattern, offset)?;
                let rhs = Fixture::new(words, 29, pattern, offset)?;
                let expected: u64 = lhs
                    .words()
                    .iter()
                    .zip(rhs.words())
                    .map(|(left, right)| u64::from((left & right).count_ones()))
                    .sum();
                for arm in AndArm::ALL {
                    let op = arm.resolve()?;
                    tally.check(
                        || format!("{} words={words} offset={offset} {pattern_name}", arm.name()),
                        op(lhs.words(), rhs.words()),
                        expected,
                    );
                }
            }
        }
    }
    for (long, short) in [(129_usize, 0_usize), (129, 1), (129, 65), (1024, 193)] {
        let long_words = vec![u64::MAX; long];
        let short_words = vec![u64::MAX; short];
        for arm in AndArm::ALL {
            let op = arm.resolve()?;
            tally.check(
                || format!("{} shorter-operand {short} of {long}", arm.name()),
                op(&long_words[..short], &short_words),
                (short * 64) as u64,
            );
        }
    }
    Ok(())
}

/// The consumer arms against a bit-by-bit parity reference.
fn matvec_shapes(tally: &mut Tally) -> Result<(), String> {
    for &(rows, cols) in MATVEC_SHAPES {
        let mut generator = SplitMix64::new(0x5cbb_6545);
        let mut matrix = BitMatrix::zeros(rows, cols);
        for row in 0..rows {
            for word in matrix.row_words_mut(row).iter_mut() {
                *word = generator.next_u64();
            }
        }
        let mut x = BitVec::zeros(cols);
        for index in 0..cols {
            x.set(index, generator.next_u64() & 1 == 1);
        }
        let expected: Vec<bool> = (0..rows)
            .map(|row| {
                (0..cols).filter(|col| matrix.get(row, *col) && x.get(*col)).count() % 2 == 1
            })
            .collect();
        for arm in MatvecArm::ALL {
            let y = arm.run(&matrix, &x)?;
            let observed = (0..rows).filter(|row| y.get(*row)).count() as u64;
            let matches = (0..rows).filter(|row| y.get(*row) == expected[*row]).count() as u64;
            tally.check(
                || format!("{} rows={rows} cols={cols} agreeing rows", arm.name()),
                matches,
                rows as u64,
            );
            tally.check(
                || format!("{} rows={rows} cols={cols} set rows", arm.name()),
                observed,
                expected.iter().filter(|bit| **bit).count() as u64,
            );
        }
    }
    Ok(())
}

fn case_str<'a>(case: &'a Value, key: &str) -> Result<&'a str, String> {
    case.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("case lacks string {key}"))
}

fn case_usize(case: &Value, key: &str) -> Result<usize, String> {
    case.get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| format!("case lacks integer {key}"))
}

fn case_u64(case: &Value, key: &str) -> Result<u64, String> {
    case.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("case lacks integer {key}"))
}

/// Both arms of every planned cell, on the exact case the runner sends.
fn plan_cells(path: &str, reference: &Reference) -> Result<Tally, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let plan: Value = serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?;
    let cells = plan
        .get("cells")
        .and_then(Value::as_array)
        .ok_or("plan lacks cells")?;
    let mut tally = Tally::default();
    for cell in cells {
        let id = case_str(cell, "cell_id")?.to_owned();
        let case = cell.get("case").ok_or("cell lacks case")?;
        let names = [case_str(cell, "baseline_arm")?, case_str(cell, "candidate_arm")?];
        match case.get("op").and_then(Value::as_str) {
            Some("popcount") => {
                let words = case_usize(case, "words")?;
                let pattern = Pattern::parse(case_str(case, "pattern")?)?;
                let offset = case_usize(case, "word_offset")?;
                let fixture = Fixture::new(words, case_u64(case, "seed")?, pattern, offset)?;
                let expected = reference.words(fixture.words());
                for name in names {
                    let arm = PopcountArm::parse(name)
                        .ok_or_else(|| format!("cell {id}: unknown count arm {name}"))?;
                    if !arm.accepts_misalignment(fixture.misalignment_bytes()) {
                        return Err(format!("cell {id}: {name} rejects this window"));
                    }
                    let op = arm.resolve()?;
                    // SAFETY: alignment checked above; arm resolved on this host.
                    let observed = unsafe { op(fixture.words()) };
                    tally.check(|| format!("cell {id} {name}"), observed, expected);
                }
            }
            Some("and_popcnt") => {
                let words = case_usize(case, "words")?;
                let pattern = Pattern::parse(case_str(case, "pattern")?)?;
                let offset = case_usize(case, "word_offset")?;
                let lhs = Fixture::new(words, case_u64(case, "seed_lhs")?, pattern, offset)?;
                let rhs = Fixture::new(words, case_u64(case, "seed_rhs")?, pattern, offset)?;
                let expected: u64 = lhs
                    .words()
                    .iter()
                    .zip(rhs.words())
                    .map(|(left, right)| u64::from((left & right).count_ones()))
                    .sum();
                for name in names {
                    let op = AndArm::parse(name)
                        .ok_or_else(|| format!("cell {id}: unknown AND arm {name}"))?
                        .resolve()?;
                    tally.check(
                        || format!("cell {id} {name}"),
                        op(lhs.words(), rhs.words()),
                        expected,
                    );
                }
            }
            Some("matvec") => {
                let rows = case_usize(case, "rows")?;
                let cols = case_usize(case, "cols")?;
                let seed = case_u64(case, "seed")?;
                let mut generator = SplitMix64::new(seed);
                let mut matrix = BitMatrix::zeros(rows, cols);
                for row in 0..rows {
                    for word in matrix.row_words_mut(row).iter_mut() {
                        *word = generator.next_u64();
                    }
                }
                let mut x = BitVec::zeros(cols);
                for index in 0..cols {
                    x.set(index, generator.next_u64() & 1 == 1);
                }
                let expected = (0..rows)
                    .filter(|row| {
                        (0..cols).filter(|col| matrix.get(*row, *col) && x.get(*col)).count() % 2
                            == 1
                    })
                    .count() as u64;
                for name in names {
                    let arm = MatvecArm::parse(name)
                        .ok_or_else(|| format!("cell {id}: unknown matvec arm {name}"))?;
                    let y = arm.run(&matrix, &x)?;
                    tally.check(
                        || format!("cell {id} {name}"),
                        (0..rows).filter(|row| y.get(*row)).count() as u64,
                        expected,
                    );
                }
            }
            other => return Err(format!("cell {id}: unknown op {other:?}")),
        }
    }
    Ok(tally)
}

fn report(group: &str, tally: &Tally) -> bool {
    if tally.failures.is_empty() {
        println!("{group}: PASS {} cases", tally.cases);
        true
    } else {
        println!("{group}: FAIL {} cases", tally.cases);
        for failure in &tally.failures {
            println!("  {failure}");
        }
        false
    }
}

fn observations() {
    let caps = libpopcnt_capabilities();
    println!(
        "libpopcnt get_cpuid(): flags={:#x} popcnt={} avx2={} avx512_vpopcntdq={}",
        caps.flags, caps.popcnt, caps.avx2, caps.avx512_vpopcntdq
    );
    println!(
        "host: avx2={} popcnt={} avx512vpopcntdq={}",
        std::arch::is_x86_feature_detected!("avx2"),
        std::arch::is_x86_feature_detected!("popcnt"),
        std::arch::is_x86_feature_detected!("avx512vpopcntdq"),
    );
    for words in [1_usize, 4, 7, 8, 16, 32, 63, 64, 65, 128, 256, 1024, 16384, STREAMING_SIZE] {
        println!(
            "gf2 route words={words}: legacy={} resolved={} and_resolved={}",
            legacy_route(words),
            resolved_route(words),
            resolved_and_route(words)
        );
    }
    for offset in 0..4 {
        let fixture = Fixture::new(64, 1, Pattern::Random, offset).expect("fixture");
        let accepted: Vec<&str> = PopcountArm::ALL
            .into_iter()
            .filter(|arm| arm.accepts_misalignment(fixture.misalignment_bytes()))
            .map(PopcountArm::name)
            .collect();
        println!(
            "word_offset={offset}: {} bytes past a vector boundary; arms accepting it: {}",
            fixture.misalignment_bytes(),
            accepted.join(" ")
        );
    }
}

fn main() -> ExitCode {
    let reference = Reference::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if let [flag, path] = args.as_slice() {
        if flag != "--plan" {
            Err(format!("unknown argument {flag}"))
        } else {
            plan_cells(path, &reference).map(|tally| report(&format!("plan {path}"), &tally))
        }
    } else if args.is_empty() {
        observations();
        (|| -> Result<bool, String> {
            let mut words = Tally::default();
            let refused = word_matrix(&reference, &mut words)?;
            let mut tails = Tally::default();
            byte_tails(&reference, &mut tails)?;
            let mut bits = Tally::default();
            bit_lengths(&reference, &mut bits)?;
            let mut fused = Tally::default();
            and_matrix(&mut fused)?;
            let mut consumers = Tally::default();
            matvec_shapes(&mut consumers)?;
            println!("count arms refused {refused} misaligned windows (Mula at offsets 1-3)");
            Ok([
                report("count arms over word windows", &words),
                report("external byte-length tails", &tails),
                report("bit-length tail semantics", &bits),
                report("fused AND arms", &fused),
                report("matrix-vector consumer arms", &consumers),
            ]
            .iter()
            .all(|passed| *passed))
        })()
    } else {
        Err("usage: count-verify [--plan <plan.json>]".to_owned())
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("count-verify: {error}");
            ExitCode::FAILURE
        }
    }
}
