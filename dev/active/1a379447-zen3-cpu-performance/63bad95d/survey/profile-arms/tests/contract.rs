//! Contract of the profile arms: every declared operation agrees with its
//! portable reference at the word-boundary shapes, and a case round-trips the
//! canonical wire form.

use profile_build_arms::{check_executable, parse_build, Case, Workload, CLMUL_WORDS};

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for words in [1, 2, 3, 4, 5, 7, 8, 9, 63, 64, 65, 1024] {
        let seed = words as u64;
        cases.push(Case::XorInplace { words, seed });
        cases.push(Case::Popcount { words, seed });
        cases.push(Case::AndPopcount { words, seed });
    }
    for columns in [1, 63, 64, 65, 448, 512, 1535, 1536] {
        cases.push(Case::Matvec {
            rows: 65,
            columns,
            seed: columns as u64,
        });
        cases.push(Case::Transpose {
            rows: 130,
            columns,
            seed: columns as u64,
        });
    }
    cases.push(Case::ClmulWide {
        words: CLMUL_WORDS,
        seed: 9,
    });
    cases
}

#[test]
fn every_operation_matches_its_portable_reference() {
    for case in cases() {
        let mut workload = Workload::build(&case).unwrap();
        workload
            .verify()
            .unwrap_or_else(|error| panic!("{case:?}: {error}"));
        // A measured call leaves the operands a later call can use.
        workload.call();
        workload.call();
        if !matches!(case, Case::XorInplace { .. }) {
            workload
                .verify()
                .unwrap_or_else(|error| panic!("{case:?} after calls: {error}"));
        }
    }
}

#[test]
fn a_case_round_trips_the_canonical_wire_form() {
    for case in cases() {
        let text = serde_json::to_string(&case).unwrap();
        assert_eq!(serde_json::from_str::<Case>(&text).unwrap(), case);
    }
    assert!(serde_json::from_str::<Case>(r#"{"operation":"xor-inplace","words":4}"#).is_err());
    assert!(serde_json::from_str::<Case>(
        r#"{"operation":"xor-inplace","words":4,"seed":1,"x":0}"#
    )
    .is_err());
}

#[test]
fn degenerate_and_foreign_cases_are_refused() {
    assert!(Workload::build(&Case::Popcount { words: 0, seed: 0 }).is_err());
    assert!(Workload::build(&Case::Matvec {
        rows: 0,
        columns: 8,
        seed: 0
    })
    .is_err());
    assert!(Workload::build(&Case::ClmulWide {
        words: CLMUL_WORDS + 1,
        seed: 0
    })
    .is_err());
}

#[test]
fn a_level_no_profile_arm_serves_is_refused_at_the_boundary() {
    assert!(parse_build("external").is_err());
    assert!(parse_build("portable").is_err());
    let served = ["conservative-portable", "tuned-portable", "native"]
        .into_iter()
        .filter(|name| check_executable(parse_build(name).unwrap()).is_ok())
        .count();
    assert!(served <= 1);
}

/// The operands of the cases the collected confirmations record, by digest:
/// a change of the fixture stream or of the operand layout fails here.
#[test]
fn recorded_cases_generate_their_recorded_operands() {
    for (case, digest) in [
        (
            r#"{"operation":"xor-inplace","seed":630401,"words":4}"#,
            "2c732b6c227d93d28491d3dea7463ec09fcdf35880cc67f2a467214b3884ae19",
        ),
        (
            r#"{"operation":"and-popcount","seed":630701,"words":7}"#,
            "fe55aae7168684e7ef660045ba4c43ae210604b49396a0adfa1d02b35abc6297",
        ),
        (
            r#"{"operation":"xor-inplace","seed":630801,"words":8}"#,
            "2ee2b72f532d3d3d5301dcfc5816f467dc0250867e22d5f4eb4c2c55837a9cfd",
        ),
        (
            r#"{"operation":"xor-inplace","seed":630501,"words":5}"#,
            "97574e1a968956e94cf755ca805d7f892c046a23f79bb9b918e026e6a7983847",
        ),
        (
            r#"{"operation":"popcount","seed":630601,"words":6}"#,
            "7b22856c0f1a0e5336cb5ba3e9737b47c0bd8d087af16dd8cea71a49df9cd11b",
        ),
        (
            r#"{"operation":"xor-inplace","seed":630301,"words":3}"#,
            "5ecc9c561a029497f2916009c9cd088f732cc2bcd9481a2de929b450a3be3c77",
        ),
        (
            r#"{"operation":"matvec","rows":1024,"columns":1536,"seed":632411}"#,
            "041bfcbe6f1567c91644246cc3ea8e6b9baeb8db505a0182c49831a9a581f7cd",
        ),
        (
            r#"{"operation":"transpose","rows":8192,"columns":2048,"seed":638211}"#,
            "24fa0c47e92d2fbdc7789a4c9b484f57484f87ad25028ad2c902ec31d7132e76",
        ),
        (
            r#"{"operation":"popcount","seed":631011,"words":1024}"#,
            "8c2c0ac4f8c5a0acf77193529b6a2a9004ab307746e2f62943089a9cdd7fed84",
        ),
        (
            r#"{"operation":"and-popcount","seed":631021,"words":1024}"#,
            "75cde38e64be30d99f750b896f5b9167a8fd003e3c31c0e8a9479ec72488b471",
        ),
        (
            r#"{"operation":"xor-inplace","seed":630811,"words":8}"#,
            "71998996ef0ad6c78b965ae294d28a6fc6bffaddcb2ad5a77d4463a7e2b720d6",
        ),
        (
            r#"{"operation":"clmul-wide","seed":630911,"words":9}"#,
            "a04a9e93cd8fed15d75720dbcc6984c785b7472169c3acd806859eb52f60b41e",
        ),
    ] {
        let workload = Workload::build(&serde_json::from_str::<Case>(case).unwrap()).unwrap();
        assert_eq!(workload.operand_digest(), digest, "{case}");
    }
}
