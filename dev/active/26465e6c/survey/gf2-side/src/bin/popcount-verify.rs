//! Operation-equivalence validation for every survey arm (jit:26465e6c).
//!
//! Without arguments it checks every arm against an independent byte-table
//! count over a matrix of sizes, word offsets, bit patterns and seeds, the
//! byte-length tails of the external kernels, gf2's bit-length tail semantics
//! through `BitVec::count_ones`, the fused AND arms, and Mula's alignment
//! precondition, and prints the runtime selections it observes. With
//! `--plan <plan.json>` it checks both arms of every planned cell on the exact
//! case the runner will send. Any mismatch exits nonzero.

use gf2_core::BitVec;
use popcount_survey::external::{self, libpopcnt_capabilities};
use popcount_survey::{AndArm, Fixture, Pattern, PopcountArm, VECTOR_BYTES};
use serde_json::Value;
use std::process::ExitCode;
use tuning_campaign_support::abtest::SplitMix64;

const SIZES: &[usize] = &[
    0, 1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 13, 15, 16, 17, 31, 32, 33, 59, 60, 61, 63, 64, 65, 95, 96,
    97, 127, 128, 129, 255, 256, 257, 1023, 1024, 1025, 4096, 16384,
];
const STREAMING_SIZE: usize = 1 << 20;
const SEEDS: &[u64] = &[0, 1, 0x0123_4567_89ab_cdef];
const BIT_LENGTHS: &[usize] = &[
    0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 255, 256, 257, 511, 512, 513, 1023, 1024, 1025, 4095,
    4096, 4097, 65535, 65536, 65537,
];

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
    fn check(&mut self, what: impl FnOnce() -> String, got: u64, expected: u64) {
        self.cases += 1;
        if got != expected && self.failures.len() < 20 {
            self.failures
                .push(format!("{}: got {got}, expected {expected}", what()));
        }
    }
}

fn patterns() -> Vec<(Pattern, u64)> {
    let mut cases: Vec<_> = SEEDS.iter().map(|seed| (Pattern::Random, *seed)).collect();
    cases.push((Pattern::AllZero, 0));
    cases.push((Pattern::AllOne, 0));
    cases
}

/// Calls a resolved arm after checking its alignment precondition; `None`
/// when the arm does not accept this window.
fn call(arm: PopcountArm, words: &[u64]) -> Result<Option<u64>, String> {
    let op = arm.resolve()?;
    if !arm.accepts_misalignment(words.as_ptr() as usize % VECTOR_BYTES) {
        return Ok(None);
    }
    // SAFETY: the alignment precondition was checked and `op` resolved here.
    Ok(Some(unsafe { op(words) }))
}

fn word_matrix(reference: &Reference, tally: &mut Tally) -> Result<usize, String> {
    let mut refused = 0;
    let mut sizes = SIZES.to_vec();
    sizes.push(STREAMING_SIZE);
    for &len in &sizes {
        for (pattern, seed) in patterns() {
            for offset in 0..4 {
                if len == STREAMING_SIZE && offset != 0 {
                    continue;
                }
                let fixture = Fixture::new(len, seed, pattern, offset)?;
                let expected = reference.words(fixture.words());
                for arm in PopcountArm::ALL {
                    match call(arm, fixture.words())? {
                        Some(got) => tally.check(
                            || {
                                format!(
                                    "{} len={len} {pattern:?} seed={seed} offset={offset}",
                                    arm.name()
                                )
                            },
                            got,
                            expected,
                        ),
                        None => refused += 1,
                    }
                }
            }
        }
    }
    Ok(refused)
}

/// External byte-length entry points, every tail length 0 to 31 bytes.
fn byte_tails(reference: &Reference, tally: &mut Tally) -> Result<(), String> {
    PopcountArm::MulaAvx2HarleySeal.resolve()?;
    for &len in &[0_usize, 1, 4, 12, 16, 64, 128, 257] {
        let fixture = Fixture::new(len + 4, 0x5eed ^ len as u64, Pattern::Random, 0)?;
        let bytes = word_bytes(fixture.words());
        for tail in 0..32 {
            let count = len * 8 + tail;
            let expected = reference.bytes(&bytes[..count]);
            tally.check(
                || format!("libpopcnt bytes={count}"),
                external::libpopcnt_bytes(&bytes[..count]),
                expected,
            );
            // SAFETY: offset-0 fixtures start on a 32-byte boundary, and the
            // Mula arm resolved above, so AVX2 and POPCNT are present.
            let got = unsafe { external::mula_bytes(&bytes[..count]) };
            tally.check(|| format!("mula bytes={count}"), got, expected);
        }
    }
    Ok(())
}

fn word_bytes(words: &[u64]) -> &[u8] {
    // SAFETY: `u64` has no padding and every byte pattern is a valid `u8`.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}

/// gf2's zero-padded bit vectors: `BitVec::count_ones`, every arm over the
/// storage words, and the byte-length external entries over `ceil(n / 8)`
/// bytes all equal the count of set bits below the bit length.
fn bit_lengths(reference: &Reference, tally: &mut Tally) -> Result<(), String> {
    PopcountArm::MulaAvx2HarleySeal.resolve()?;
    for &bits in BIT_LENGTHS {
        let mut generator = SplitMix64::new(0xb17 ^ bits as u64);
        let mut vector = BitVec::zeros(bits);
        let mut expected = 0_u64;
        for index in 0..bits {
            let bit = generator.next_u64() & 1 == 1;
            vector.set(index, bit);
            expected += u64::from(bit);
        }
        tally.check(
            || format!("BitVec::count_ones bits={bits}"),
            vector.count_ones() as u64,
            expected,
        );
        tally.check(
            || format!("reference over storage bits={bits}"),
            reference.words(vector.words()),
            expected,
        );
        let aligned = Fixture::from_slice(vector.words(), 0)?;
        for arm in PopcountArm::ALL {
            let got = call(arm, aligned.words())?.expect("offset 0 is accepted by every arm");
            tally.check(|| format!("{} bits={bits}", arm.name()), got, expected);
        }
        let bytes = &word_bytes(aligned.words())[..bits.div_ceil(8)];
        tally.check(
            || format!("libpopcnt ceil-bytes bits={bits}"),
            external::libpopcnt_bytes(bytes),
            expected,
        );
        // SAFETY: as in `byte_tails`.
        let got = unsafe { external::mula_bytes(bytes) };
        tally.check(|| format!("mula ceil-bytes bits={bits}"), got, expected);
    }
    Ok(())
}

fn and_matrix(tally: &mut Tally) -> Result<(), String> {
    let mut sizes = SIZES.to_vec();
    sizes.push(STREAMING_SIZE / 2);
    let arms = AndArm::ALL
        .into_iter()
        .map(|arm| arm.resolve().map(|op| (arm, op)))
        .collect::<Result<Vec<_>, _>>()?;
    for &len in &sizes {
        for (pattern, seed) in patterns() {
            for offset in 0..4 {
                if len == STREAMING_SIZE / 2 && offset != 0 {
                    continue;
                }
                let lhs = Fixture::new(len, seed, pattern, offset)?;
                let rhs = Fixture::new(len, seed.wrapping_add(1000), pattern, offset)?;
                let expected: u64 = lhs
                    .words()
                    .iter()
                    .zip(rhs.words())
                    .map(|(left, right)| {
                        (0..64).filter(|bit| (left & right) >> bit & 1 == 1).count() as u64
                    })
                    .sum();
                for (arm, op) in &arms {
                    tally.check(
                        || {
                            format!(
                                "{} len={len} {pattern:?} seed={seed} offset={offset}",
                                arm.name()
                            )
                        },
                        op(lhs.words(), rhs.words()),
                        expected,
                    );
                }
            }
        }
    }
    Ok(())
}

fn case_usize(case: &Value, key: &str) -> Result<usize, String> {
    case[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("case lacks {key}"))
}

fn case_u64(case: &Value, key: &str) -> Result<u64, String> {
    case[key]
        .as_u64()
        .ok_or_else(|| format!("case lacks {key}"))
}

/// Both arms of every planned cell on the exact planned case.
fn plan_cells(path: &str, reference: &Reference) -> Result<Tally, String> {
    let plan: Value = serde_json::from_slice(
        &std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?,
    )
    .map_err(|error| format!("plan does not decode: {error}"))?;
    let arm_name = |name: &str| -> Result<String, String> {
        plan["arms"][name]["environment"]["GF2_POPCOUNT_ARM"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("plan arm {name} lacks GF2_POPCOUNT_ARM"))
    };
    let mut tally = Tally::default();
    for cell in plan["cells"].as_array().ok_or("plan lacks cells")? {
        let id = cell["cell_id"].as_str().unwrap_or_default().to_owned();
        let case = &cell["case"];
        let pattern = Pattern::parse(case["pattern"].as_str().ok_or("case lacks pattern")?)?;
        let words = case_usize(case, "words")?;
        let offset = case_usize(case, "word_offset")?;
        let names = [
            arm_name(cell["baseline_arm"].as_str().unwrap_or_default())?,
            arm_name(cell["candidate_arm"].as_str().unwrap_or_default())?,
        ];
        match case["op"].as_str() {
            Some("popcount") => {
                let fixture = Fixture::new(words, case_u64(case, "seed")?, pattern, offset)?;
                let expected = reference.words(fixture.words());
                for name in &names {
                    let arm = PopcountArm::parse(name).ok_or("unknown popcount arm")?;
                    let got = call(arm, fixture.words())?
                        .ok_or_else(|| format!("cell {id}: {name} refuses its planned window"))?;
                    tally.check(|| format!("cell {id} {name}"), got, expected);
                }
            }
            Some("and_popcnt") => {
                let lhs = Fixture::new(words, case_u64(case, "seed_lhs")?, pattern, offset)?;
                let rhs = Fixture::new(words, case_u64(case, "seed_rhs")?, pattern, offset)?;
                let expected: u64 = lhs
                    .words()
                    .iter()
                    .zip(rhs.words())
                    .map(|(left, right)| reference.words(&[left & right]))
                    .sum();
                for name in &names {
                    let op = AndArm::parse(name).ok_or("unknown AND arm")?.resolve()?;
                    tally.check(
                        || format!("cell {id} {name}"),
                        op(lhs.words(), rhs.words()),
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
    for words in [
        1_usize,
        4,
        7,
        8,
        12,
        60,
        64,
        128,
        256,
        16384,
        STREAMING_SIZE,
    ] {
        println!(
            "gf2 dispatch words={words}: {}",
            popcount_survey::arms::dispatch_route(words)
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
            println!("popcount arms refused {refused} misaligned windows (Mula at offsets 1-3)");
            Ok([
                report("popcount arms over word windows", &words),
                report("external byte-length tails", &tails),
                report("bit-length tail semantics", &bits),
                report("fused AND arms", &fused),
            ]
            .iter()
            .all(|passed| *passed))
        })()
    } else {
        Err("usage: popcount-verify [--plan <plan.json>]".to_owned())
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("popcount-verify: {error}");
            ExitCode::from(2)
        }
    }
}
