//! Prints the code parameters each project derives for every configuration.
//!
//! Run without features it reports gf2 alone; with `--features aff3ct,srsran`
//! it adds each comparator's own derivation, which is what decides whether a
//! configuration is a matched arm or a recorded non-equivalence.
//!
//! The output is one `nr-encode-parameters-v1` JSON document on stdout.

use serde::Serialize;
use survey_nr_encode::{gf2_code, Configuration, CONFIGURATIONS};

#[derive(Serialize)]
struct Gf2Parameters {
    base_graph: u8,
    lifting_factor: usize,
    target_k: usize,
    target_n: usize,
    full_k: usize,
    full_n: usize,
    num_shortened: usize,
    num_punctured_systematic: usize,
    num_punctured_parity: usize,
    kb: usize,
    kb_for_z_selection: usize,
    nb: usize,
    effective_rate: f64,
}

#[derive(Serialize)]
struct Row {
    configuration: String,
    redundancy_version: u8,
    gf2: Gf2Parameters,
    #[serde(skip_serializing_if = "Option::is_none")]
    aff3ct: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    srsran: Option<serde_json::Value>,
}

#[cfg(feature = "aff3ct")]
fn aff3ct_row(configuration: Configuration) -> Option<serde_json::Value> {
    use survey_nr_encode::aff3ct;
    match aff3ct::base_graph(configuration.target_k, configuration.target_n) {
        Some(derived) => Some(serde_json::json!({
            "accepted": true,
            "base_graph": derived.base_graph,
            "lifting_factor": derived.lifting,
            "index_list": derived.index_list,
            "k_ldpc": derived.k_ldpc,
            "n_ldpc": derived.n_ldpc,
        })),
        None => Some(serde_json::json!({ "accepted": false })),
    }
}

#[cfg(not(feature = "aff3ct"))]
fn aff3ct_row(_configuration: Configuration) -> Option<serde_json::Value> {
    None
}

#[cfg(feature = "srsran")]
fn srsran_row(configuration: Configuration) -> Option<serde_json::Value> {
    use survey_nr_encode::srsran;
    let code = gf2_code(configuration);
    let params = code.params();
    match srsran::dimensions(params.base_graph, params.lifting_factor) {
        Some(dims) => Some(serde_json::json!({
            "accepted": true,
            "base_graph": params.base_graph,
            "lifting_factor": params.lifting_factor,
            "k_ldpc": dims.k_ldpc,
            "n_short": dims.n_short,
            "n_full": dims.n_full,
            "supports_avx2": srsran::supports_avx2(),
        })),
        None => Some(serde_json::json!({ "accepted": false })),
    }
}

#[cfg(not(feature = "srsran"))]
fn srsran_row(_configuration: Configuration) -> Option<serde_json::Value> {
    None
}

fn main() {
    let rows: Vec<Row> = CONFIGURATIONS
        .iter()
        .copied()
        .map(|configuration| {
            let code = gf2_code(configuration);
            let params = code.params();
            Row {
                configuration: configuration.name.to_string(),
                redundancy_version: configuration.redundancy_version,
                gf2: Gf2Parameters {
                    base_graph: params.base_graph,
                    lifting_factor: params.lifting_factor,
                    target_k: params.target_k,
                    target_n: params.target_n,
                    full_k: params.full_k,
                    full_n: params.full_n,
                    num_shortened: params.num_shortened,
                    num_punctured_systematic: params.num_punctured_systematic,
                    num_punctured_parity: params.num_punctured_parity,
                    kb: params.kb,
                    kb_for_z_selection: gf2_coding::ldpc::nr_5g::kb_for_z_selection(
                        params.base_graph,
                        params.target_k,
                    ),
                    nb: params.nb,
                    effective_rate: params.effective_rate(),
                },
                aff3ct: aff3ct_row(configuration),
                srsran: srsran_row(configuration),
            }
        })
        .collect();
    let document = serde_json::json!({
        "schema": "nr-encode-parameters-v1",
        "rows": rows,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&document).expect("parameters serialize")
    );
}
