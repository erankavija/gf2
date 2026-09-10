use gf2_core::BitVec;
use std::process::Command;

const WORDS: &[u64] = &[0, 1, 2, 7, 8, 9, 63, 64, 65, 256, 4096];
const OFFSETS: &[u32] = &[0, 1, 2, 3];
const SEEDS: &[u64] = &[0, 1, 0x0123_4567_89ab_cdef];
const ARMS: &[&str] = &[
    "production-dispatch",
    "nibble-lut",
    "scalar-popcnt",
    "compiler-count-ones",
];

fn splitmix_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn reference_words(words: u64, seed: u64, pattern: &str, offset: u32) -> Vec<u64> {
    let mut state = seed;
    let mut all = Vec::with_capacity(words as usize + 4);
    for _ in 0..(words + 4) {
        all.push(match pattern {
            "random" => splitmix_next(&mut state),
            "all_zero" => 0,
            "all_one" => u64::MAX,
            other => panic!("unknown pattern {other}"),
        });
    }
    all[offset as usize..offset as usize + words as usize].to_vec()
}

fn reference_count(words: &[u64]) -> u64 {
    words.iter().map(|word| word.count_ones() as u64).sum()
}

fn binary() -> String {
    std::env::var("CARGO_BIN_EXE_popcount-gf2-side")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_popcount_gf2_side"))
        .expect("cargo must provide the popcount-gf2-side binary")
}

fn check(arm: &str, args: &[String]) -> u64 {
    let output = Command::new(binary())
        .env("GF2_POPCOUNT_ARM", arm)
        .arg("--check")
        .args(args)
        .output()
        .expect("failed to run gf2-side --check");
    assert!(output.status.success(), "{arm}: {:?}", output);
    assert!(output.stderr.is_empty(), "{arm}: stderr was not empty");
    let stdout = String::from_utf8(output.stdout).expect("check output must be UTF-8");
    assert_eq!(stdout.lines().count(), 1, "{arm}: {stdout:?}");
    stdout.trim().parse().expect("check output must be a u64")
}

#[test]
fn popcount_arms_match_an_independent_reference() {
    for &words in WORDS {
        for &pattern in &["random", "all_zero", "all_one"] {
            for &seed in if pattern == "random" { SEEDS } else { &[0] } {
                for &offset in OFFSETS {
                    let expected = reference_count(&reference_words(words, seed, pattern, offset));
                    let args = vec![
                        "popcount".to_owned(),
                        words.to_string(),
                        seed.to_string(),
                        pattern.to_owned(),
                        offset.to_string(),
                    ];
                    for &arm in ARMS {
                        assert_eq!(check(arm, &args), expected, "arm={arm}, args={args:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn fused_arms_match_an_independent_reference() {
    let arms = [
        "and-popcnt-fused",
        "and-popcnt-scalar-control",
        "and-popcnt-two-pass-consumer",
    ];
    for &words in WORDS {
        for &pattern in &["random", "all_zero", "all_one"] {
            for &offset in OFFSETS {
                let lhs = reference_words(words, 0x1111_2222_3333_4444, pattern, offset);
                let rhs = reference_words(words, 0xaaaa_bbbb_cccc_dddd, pattern, offset);
                let expected: u64 = lhs
                    .iter()
                    .zip(&rhs)
                    .map(|(left, right)| (left & right).count_ones() as u64)
                    .sum();
                let args = vec![
                    "and_popcnt".to_owned(),
                    words.to_string(),
                    "1229801703532086340".to_owned(),
                    "12297848147757817309".to_owned(),
                    pattern.to_owned(),
                    offset.to_string(),
                ];
                for &arm in &arms {
                    assert_eq!(check(arm, &args), expected, "arm={arm}, args={args:?}");
                }
            }
        }
    }
}

/// `BitVec::count_ones` delegates to `crate::kernels::ops::popcount` at
/// `crates/gf2-core/src/bitvec.rs:601`; this is the exact tail-semantics path.
#[test]
fn bitvec_count_ones_ignores_logical_tail_padding() {
    for &bit_len in &[0, 1, 63, 64, 65, 127, 128, 4095, 4096] {
        let seed = 0xfeed_face_cafe_beef_u64 ^ bit_len as u64;
        let mut state = seed;
        let mut bitvec = BitVec::new();
        for index in 0..bit_len {
            bitvec.push_bit(false);
            if splitmix_next(&mut state) & 1 != 0 {
                bitvec.set(index, true);
            }
        }
        let mut independent_state = seed;
        let expected = (0..bit_len)
            .filter(|_| splitmix_next(&mut independent_state) & 1 != 0)
            .count();
        let direct = (0..bit_len).filter(|&index| bitvec.get(index)).count();
        assert_eq!(bitvec.count_ones(), expected);
        assert_eq!(bitvec.count_ones(), direct);
    }
}

/// The alignment cell's premise: `word_offset` shifts the fixture window by
/// exactly that many words past a 32-byte aligned base, so `0` is AVX2-vector
/// aligned and `1..=3` break the alignment by 8, 16 and 24 bytes. The external
/// arms print the identical lines, which `cross-check.sh` diffs.
#[test]
fn word_offset_shifts_the_fixture_off_a_vector_aligned_base() {
    let output = Command::new(binary())
        .env("GF2_POPCOUNT_ARM", "production-dispatch")
        .arg("--alignment")
        .output()
        .expect("failed to run gf2-side --alignment");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("alignment output must be UTF-8");
    let mut lines = 0;
    for line in stdout.lines() {
        let offset: usize = line
            .split_whitespace()
            .find_map(|field| field.strip_prefix("word_offset="))
            .expect("every line names its word_offset")
            .parse()
            .expect("word_offset must parse");
        let modulus: usize = line
            .split_whitespace()
            .find_map(|field| field.strip_prefix("address_mod_32="))
            .expect("every line names its address modulus")
            .parse()
            .expect("modulus must parse");
        assert_eq!(modulus, offset * 8, "{line}");
        lines += 1;
    }
    assert_eq!(lines, 16, "{stdout}");
}

/// The whole-consumer cell names its case `and_popcnt_two_pass`; every fused
/// arm must accept that op and return the same count it returns for the
/// kernel-isolated `and_popcnt` op on the same buffers.
#[test]
fn the_whole_consumer_op_returns_the_kernel_isolated_count() {
    for &words in WORDS {
        for &arm in &[
            "and-popcnt-fused",
            "and-popcnt-scalar-control",
            "and-popcnt-two-pass-consumer",
        ] {
            let tail = [
                words.to_string(),
                "401".to_owned(),
                "1401".to_owned(),
                "random".to_owned(),
                "0".to_owned(),
            ];
            let isolated: Vec<String> = std::iter::once("and_popcnt".to_owned())
                .chain(tail.iter().cloned())
                .collect();
            let consumer: Vec<String> = std::iter::once("and_popcnt_two_pass".to_owned())
                .chain(tail.iter().cloned())
                .collect();
            assert_eq!(check(arm, &isolated), check(arm, &consumer), "arm={arm}");
        }
    }
}
