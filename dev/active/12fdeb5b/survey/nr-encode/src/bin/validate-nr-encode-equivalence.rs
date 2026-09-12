//! Untimed bit-exact equivalence gate for the 5G NR rate-matched encoder
//! comparison (jit:12fdeb5b).
//!
//! For every configuration the gate encodes the same messages through gf2 and
//! through each adapter and compares the rate-matched codewords bit for bit.
//! A configuration is a matched arm only when the comparator derives gf2's
//! code parameters and reproduces its codeword on every message; otherwise
//! the record names the parameter that differs and the arm is not timed.
//!
//! The message set is the all-zero word, the all-one word and seeded random
//! words, so an implementation that is correct only on the all-zero codeword
//! fails here.
//!
//! Usage: validate-nr-encode-equivalence <aff3ct-conf-root> <record.json>

use gf2_coding::traits::BlockEncoder;
use gf2_core::BitVec;
use serde::Serialize;
use std::process::ExitCode;
use survey_nr_encode::{
    aff3ct, configuration_named, gf2_code, identical_bits, seeded_messages, srsran, Configuration,
    CONFIGURATIONS,
};

/// Seed for the random messages; fixed so the record reproduces.
const MESSAGE_SEED: u64 = 20260912;
/// Random messages per configuration, beyond the two extreme words.
const RANDOM_MESSAGES: usize = 8;
/// The modulation order the comparison fixes. TS 38.212 Section 5.4.2.2's
/// interleaving is the identity at one bit per symbol, so the rate-matched
/// codeword is exactly the selected bit sequence, which is the operation gf2
/// implements.
const MODULATION_ORDER: u8 = 1;

#[derive(Serialize)]
struct Derivation {
    base_graph: i32,
    lifting_factor: i32,
    k_ldpc: i32,
    n_mother: i32,
}

#[derive(Serialize)]
struct ArmOutcome {
    arm: String,
    /// `matched`, `non-equivalent` or `unavailable`.
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    differing_parameter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    derivation: Option<Derivation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_backend: Option<String>,
    messages_compared: usize,
    /// Messages whose rate-matched codeword is bit-identical to gf2's.
    messages_identical: usize,
    /// Smallest number of agreeing bit positions over the compared messages.
    min_identical_positions: usize,
    codeword_bits: usize,
}

#[derive(Serialize)]
struct Row {
    configuration: String,
    base_graph: u8,
    target_k: usize,
    target_n: usize,
    lifting_factor: usize,
    filler_bits: usize,
    punctured_systematic_bits: usize,
    redundancy_version: u8,
    modulation_order: u8,
    arms: Vec<ArmOutcome>,
}

fn messages(configuration: Configuration) -> Vec<BitVec> {
    let mut set = vec![
        BitVec::zeros(configuration.target_k),
        BitVec::ones(configuration.target_k),
    ];
    set.extend(seeded_messages(
        MESSAGE_SEED,
        RANDOM_MESSAGES,
        configuration.target_k,
    ));
    set
}

fn srsran_arm(configuration: Configuration, expected: &[BitVec]) -> ArmOutcome {
    let code = gf2_code(configuration);
    let params = code.params();
    let Some(dims) = srsran::dimensions(params.base_graph, params.lifting_factor) else {
        return ArmOutcome {
            arm: "srsran".into(),
            status: "unavailable".into(),
            differing_parameter: Some("lifting_factor".into()),
            detail: Some(format!(
                "srsRAN carries no graph for BG{} Z={}",
                params.base_graph, params.lifting_factor
            )),
            derivation: None,
            selected_backend: None,
            messages_compared: 0,
            messages_identical: 0,
            min_identical_positions: 0,
            codeword_bits: params.target_n,
        };
    };
    let derivation = Derivation {
        base_graph: i32::from(params.base_graph),
        lifting_factor: i32::try_from(params.lifting_factor).expect("Z fits in i32"),
        k_ldpc: dims.k_ldpc,
        n_mother: dims.n_full,
    };
    let comparator = match srsran::Comparator::new(None) {
        Ok(comparator) => comparator,
        Err(error) => {
            return ArmOutcome {
                arm: "srsran".into(),
                status: "unavailable".into(),
                differing_parameter: None,
                detail: Some(error),
                derivation: Some(derivation),
                selected_backend: None,
                messages_compared: 0,
                messages_identical: 0,
                min_identical_positions: 0,
                codeword_bits: params.target_n,
            };
        }
    };
    let backend = comparator.backend().name().to_string();
    let adapter = srsran::Adapter::new(
        comparator,
        params,
        configuration.redundancy_version,
        MODULATION_ORDER,
    );
    let mut identical = 0;
    let mut minimum = params.target_n;
    for (message, reference) in messages(configuration).iter().zip(expected) {
        let produced = adapter.encode(message);
        let agreeing = identical_bits(&produced, reference);
        minimum = minimum.min(agreeing);
        if agreeing == params.target_n {
            identical += 1;
        }
    }
    let compared = expected.len();
    let mother_matches = usize::try_from(dims.k_ldpc).expect("positive K_LDPC") == params.full_k
        && usize::try_from(dims.n_full).expect("positive N") == params.full_n;
    let (status, differing, detail) = if identical == compared {
        ("matched".to_string(), None, None)
    } else if configuration.redundancy_version != 0 {
        (
            "non-equivalent".to_string(),
            Some("redundancy_version".to_string()),
            Some(format!(
                "srsRAN selects from the redundancy-version {} starting offset; gf2's rate-matched \
                 encoder has no redundancy-version parameter and always starts at 2*Z",
                configuration.redundancy_version
            )),
        )
    } else if !mother_matches {
        (
            "non-equivalent".to_string(),
            Some("mother_dimensions".to_string()),
            Some(format!(
                "srsRAN's mother code is K_LDPC={} N={}; gf2's is K_LDPC={} N={}",
                dims.k_ldpc, dims.n_full, params.full_k, params.full_n
            )),
        )
    } else {
        (
            "non-equivalent".to_string(),
            Some("codeword".to_string()),
            Some(format!(
                "{identical} of {compared} messages agree; the closest disagreeing codeword shares \
                 {minimum} of {} bit positions",
                params.target_n
            )),
        )
    };
    ArmOutcome {
        arm: "srsran".into(),
        status,
        differing_parameter: differing,
        detail,
        derivation: Some(derivation),
        selected_backend: Some(backend),
        messages_compared: compared,
        messages_identical: identical,
        min_identical_positions: minimum,
        codeword_bits: params.target_n,
    }
}

fn aff3ct_arm(configuration: Configuration, expected: &[BitVec], conf_root: &str) -> ArmOutcome {
    let code = gf2_code(configuration);
    let params = code.params();
    let unavailable = |status: &str, differing: Option<&str>, detail: String| ArmOutcome {
        arm: "aff3ct".into(),
        status: status.into(),
        differing_parameter: differing.map(str::to_string),
        detail: Some(detail),
        derivation: None,
        selected_backend: None,
        messages_compared: 0,
        messages_identical: 0,
        min_identical_positions: 0,
        codeword_bits: params.target_n,
    };
    if configuration.redundancy_version != 0 {
        return unavailable(
            "unavailable",
            Some("redundancy_version"),
            format!(
                "AFF3CT's Puncturer_5G selects from position 2*Z with no redundancy-version \
                 offset, so it cannot serve redundancy version {}",
                configuration.redundancy_version
            ),
        );
    }
    let Some(derived) = aff3ct::base_graph(configuration.target_k, configuration.target_n) else {
        return unavailable(
            "unavailable",
            Some("base_graph"),
            format!(
                "AFF3CT rejects K={} N={}",
                configuration.target_k, configuration.target_n
            ),
        );
    };
    let derivation = Derivation {
        base_graph: derived.base_graph,
        lifting_factor: derived.lifting,
        k_ldpc: derived.k_ldpc,
        n_mother: derived.n_ldpc,
    };
    let matches_gf2 = usize::try_from(derived.base_graph).expect("positive base graph")
        == usize::from(params.base_graph)
        && usize::try_from(derived.lifting).expect("positive Z") == params.lifting_factor
        && usize::try_from(derived.k_ldpc).expect("positive K_LDPC") == params.full_k
        && usize::try_from(derived.n_ldpc).expect("positive N") == params.full_n;
    if !matches_gf2 {
        let differing = if usize::try_from(derived.base_graph).expect("positive base graph")
            != usize::from(params.base_graph)
        {
            "base_graph"
        } else {
            "lifting_factor"
        };
        return ArmOutcome {
            arm: "aff3ct".into(),
            status: "non-equivalent".into(),
            differing_parameter: Some(differing.into()),
            detail: Some(format!(
                "AFF3CT derives BG{} Z={} K_LDPC={} N={} from (K={}, N={}); gf2 uses BG{} Z={} \
                 K_LDPC={} N={}",
                derived.base_graph,
                derived.lifting,
                derived.k_ldpc,
                derived.n_ldpc,
                configuration.target_k,
                configuration.target_n,
                params.base_graph,
                params.lifting_factor,
                params.full_k,
                params.full_n
            )),
            derivation: Some(derivation),
            selected_backend: None,
            messages_compared: 0,
            messages_identical: 0,
            min_identical_positions: 0,
            codeword_bits: params.target_n,
        };
    }
    let comparator =
        match aff3ct::Comparator::new(configuration.target_k, configuration.target_n, conf_root) {
            Ok(comparator) => comparator,
            Err(error) => return unavailable("unavailable", None, error),
        };
    let adapter = aff3ct::Adapter::new(comparator);
    let mut identical = 0;
    let mut minimum = params.target_n;
    for (message, reference) in messages(configuration).iter().zip(expected) {
        let produced = adapter.encode(message);
        let agreeing = identical_bits(&produced, reference);
        minimum = minimum.min(agreeing);
        if agreeing == params.target_n {
            identical += 1;
        }
    }
    let compared = expected.len();
    let (status, differing, detail) = if identical == compared {
        ("matched".to_string(), None, None)
    } else {
        (
            "non-equivalent".to_string(),
            Some("codeword".to_string()),
            Some(format!(
                "{identical} of {compared} messages agree; the closest disagreeing codeword shares \
                 {minimum} of {} bit positions",
                params.target_n
            )),
        )
    };
    ArmOutcome {
        arm: "aff3ct".into(),
        status,
        differing_parameter: differing,
        detail,
        derivation: Some(derivation),
        selected_backend: Some("scalar".into()),
        messages_compared: compared,
        messages_identical: identical,
        min_identical_positions: minimum,
        codeword_bits: params.target_n,
    }
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(conf_root) = arguments.next() else {
        eprintln!("usage: validate-nr-encode-equivalence <aff3ct-conf-root> <record.json>");
        return ExitCode::from(2);
    };
    let Some(record_path) = arguments.next() else {
        eprintln!("usage: validate-nr-encode-equivalence <aff3ct-conf-root> <record.json>");
        return ExitCode::from(2);
    };

    let mut rows = Vec::new();
    for configuration in CONFIGURATIONS {
        let configuration = configuration_named(configuration.name);
        let code = gf2_code(configuration);
        let params = code.params().clone();
        let expected: Vec<BitVec> = messages(configuration)
            .iter()
            .map(|message| code.encode(message))
            .collect();
        for codeword in &expected {
            assert_eq!(
                codeword.len(),
                params.target_n,
                "gf2 emits target_n bits for {}",
                configuration.name
            );
        }
        rows.push(Row {
            configuration: configuration.name.to_string(),
            base_graph: params.base_graph,
            target_k: params.target_k,
            target_n: params.target_n,
            lifting_factor: params.lifting_factor,
            filler_bits: params.num_shortened,
            punctured_systematic_bits: params.num_punctured_systematic,
            redundancy_version: configuration.redundancy_version,
            modulation_order: MODULATION_ORDER,
            arms: vec![
                srsran_arm(configuration, &expected),
                aff3ct_arm(configuration, &expected, &conf_root),
            ],
        });
    }

    let matched: usize = rows
        .iter()
        .flat_map(|row| &row.arms)
        .filter(|arm| arm.status == "matched")
        .count();
    let document = serde_json::json!({
        "schema": "nr-encode-validation-v1",
        "message_seed": MESSAGE_SEED,
        "messages_per_configuration": RANDOM_MESSAGES + 2,
        "modulation_order": MODULATION_ORDER,
        "rows": rows,
    });
    std::fs::write(
        &record_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).expect("record serializes")
        ),
    )
    .unwrap_or_else(|error| panic!("cannot write {record_path}: {error}"));

    println!("configuration          arm     status          detail");
    for row in &rows {
        for arm in &row.arms {
            println!(
                "{:22} {:7} {:15} {}/{} messages identical{}",
                row.configuration,
                arm.arm,
                arm.status,
                arm.messages_identical,
                arm.messages_compared,
                arm.detail
                    .as_ref()
                    .map(|detail| format!("; {detail}"))
                    .unwrap_or_default()
            );
        }
    }
    println!("matched arms: {matched}");
    ExitCode::SUCCESS
}
