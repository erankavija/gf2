//! Non-timed harness smoke of this family's arms (jit:4c1e441f).
//!
//! The driver is the shared one: `byte_field_arm_common::smoke` drives every arm
//! of every cell of a saved runner plan over the runner's own wire in the
//! `validation` position, one untimed dispatch each, and refuses an arm that
//! reports a timing window. This binary supplies the family's title and command
//! and pins the request mirror against the runner's own request declaration.
//!
//! Usage (from the worktree root): gf256-gemm-smoke PLAN...

use byte_field_arm_common::smoke;
use std::process::ExitCode;

fn main() -> ExitCode {
    smoke::main_with(
        "gf256-gemm-smoke",
        "GF(2^8) dense product arm smoke (jit:4c1e441f)",
        "dev/active/4c1e441f/survey/smoke-arms.sh",
        "gf256-gemm-smoke PLAN...",
    )
}

#[cfg(test)]
mod tests {
    use byte_field_arm_common::smoke::{accept, decode_guarded, top_level_keys};
    use byte_field_arm_common::{
        runner_request_fields, validation_request, ArmResult, Case, Window, ARM_RESULT_SCHEMA,
    };
    use serde_json::Value;
    use tuning_campaign_support::protocol::{CellDeclaration, FamilyAddendum, SHARED_SETTINGS};
    use tuning_campaign_support::transport;

    /// The frozen pilot family, so the mirror is pinned against a declaration
    /// the campaigns measure rather than a fixture written here.
    const PILOT_ADDENDUM: &str = include_str!("../../../../addendum-v4-dense-product-pilot.json");

    /// The family's whole-consumer declaration, the one whose case carries the
    /// consumer boundary this family's scratch and restore costs are paid at.
    fn whole_consumer_declaration() -> CellDeclaration {
        FamilyAddendum::decode(PILOT_ADDENDUM.as_bytes())
            .expect("the frozen addendum decodes")
            .cells
            .into_iter()
            .find(|cell| cell.conversion_costs_included)
            .expect("the family declares a whole-consumer cell")
    }

    /// The case `make-plan.py` projects for one declaration.
    fn case_for(declaration: &CellDeclaration) -> Value {
        serde_json::json!({
            "operation": "matmul",
            "bytes": 0,
            "n": declaration.workload.size["n"],
            "k": 0,
            "rows": 0,
            "poly": 285,
            "metric": "whole-consumer",
            "seed": declaration.workload.seed,
            "workers": declaration.workers.declared,
        })
    }

    /// Runner request fields no cell of this family carries, and why.
    const ABSENT_FIELDS: [(&str, &str); 2] = [
        (
            "cold_calls",
            "no cell is cold, and the arm refuses a cache state whose contract forbids the \
             untimed calls its allocation probe makes",
        ),
        (
            "decoder",
            "no cell declares a decoder, and the shared driver rejects a request that carries one",
        ),
    ];

    /// The request this smoke sends is the runner's request: it carries the
    /// runner's declared fields in the runner's declaration order, the arm's own
    /// guarded decoder accepts it, and its re-encoding is byte-identical, so a
    /// field added, dropped or reordered on either side fails here rather than
    /// inside a benchmark window.
    #[test]
    fn the_request_mirror_is_the_runners_request() {
        let declaration = whole_consumer_declaration();
        let request = validation_request(
            &declaration,
            "table-element",
            case_for(&declaration),
            vec![0],
            &SHARED_SETTINGS,
        );
        let encoded = transport::encode_case(&request).expect("the runner's encoder");

        let expected: Vec<&str> = runner_request_fields()
            .into_iter()
            .filter(|field| !ABSENT_FIELDS.iter().any(|(absent, _)| absent == field))
            .collect();
        assert_eq!(top_level_keys(&encoded), expected);
        for (field, reason) in ABSENT_FIELDS {
            assert!(
                runner_request_fields().contains(&field),
                "{field} is no longer a runner request field ({reason})"
            );
        }

        let decoded = decode_guarded(&encoded).expect("the arm's decoder");
        assert_eq!(
            transport::encode_case(&decoded).expect("re-encodes"),
            encoded
        );
        assert_eq!(decoded.cache_state, declaration.cache_state);
        assert_eq!(decoded.cold_calls, declaration.cold_calls);
        assert_eq!(decoded.windows, SHARED_SETTINGS.windows_per_execution);
        assert_eq!(decoded.window_target_ms, SHARED_SETTINGS.window_target_ms);
        assert_eq!(decoded.workers_declared, declaration.workers.declared);
    }

    /// A result carrying a timing window is refused: an arm that timed its
    /// dispatch was driven in a measuring position, and a smoke records none.
    #[test]
    fn a_reported_timing_window_fails_the_smoke() {
        let declaration = whole_consumer_declaration();
        let case: Case = serde_json::from_value(case_for(&declaration)).expect("the case decodes");
        let untimed = ArmResult {
            schema: ARM_RESULT_SCHEMA.to_owned(),
            windows: Vec::new(),
            cache_state_applied: declaration.cache_state,
            workers_observed: declaration.workers.declared,
            cpus_observed: vec![0],
            selected_path: Some("lane=gf256-product-table/table-builds=1".to_owned()),
            conversion: None,
            quality: None,
            calibrated: false,
        };
        let seen = accept(
            untimed.clone(),
            "fixture",
            &declaration,
            &case,
            "table-element",
        )
        .expect("an untimed result is accepted");
        assert_eq!(seen.cell_id, declaration.cell_id);
        assert_eq!(seen.operation, "matmul");

        let timed = ArmResult {
            windows: vec![Window {
                calls: 1,
                elapsed_ns: 1,
            }],
            ..untimed
        };
        let error = accept(timed, "fixture", &declaration, &case, "table-element")
            .expect_err("a timed result is refused");
        assert!(error.contains("timing windows"), "{error}");
    }
}
