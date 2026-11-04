//! The committed coding owner envelopes read through the owner codec, and
//! the batch decoder's routes and results under the committed production
//! envelope, installed in a fresh process.

#![cfg(feature = "tuning-profile")]

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use std::collections::BTreeMap;
use std::path::Path;

use gf2_coding::ldpc::{
    BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout, LaneBackend,
    LdpcBatchDecoder, LdpcCode, QuasiCyclicLdpc,
};
use gf2_coding::test_support;
use gf2_coding::tuning::{CodingTuning, CodingTuningCodec, DecodeSelectors, QcRouteCase};
use gf2_core::tuning::{MeasurementProvenance, SectionCodec, TuningSection};
use gf2_kernels_simd::ldpc_lanes::LANES;
use std::num::NonZeroUsize;

/// The selectors of a section body by `family.field`.
fn fields(selectors: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    selectors
        .as_object()
        .expect("a selector body")
        .iter()
        .flat_map(|(family, fields)| {
            fields
                .as_object()
                .expect("a selector family")
                .iter()
                .map(move |(field, value)| (format!("{family}.{field}"), value.clone()))
        })
        .collect()
}

/// Every selector `section` resolves, whether or not its envelope states it.
fn resolved(section: &CodingTuning) -> BTreeMap<String, serde_json::Value> {
    let complete = CodingTuning::from_selectors(section.selectors().clone());
    let body = CodingTuningCodec::encode_body(&complete).expect("a complete section encodes");
    fields(&serde_json::to_value(body).expect("a canonical value is JSON"))
}

#[test]
fn a_selector_the_measured_owner_omits_keeps_its_conservative_value() {
    let prepared = fresh::measured_owner();
    let section = prepared
        .section::<CodingTuning>()
        .unwrap()
        .expect("the owner holds the coding section")
        .section;
    let document: serde_json::Value = serde_json::from_str(fresh::MEASURED_OWNER).unwrap();
    let stated = fields(&document["sections"][CodingTuning::ID.as_str()]["selectors"]);
    let conservative = resolved(&CodingTuning::CONSERVATIVE);

    let resolved = resolved(section);
    assert!(stated.keys().all(|field| resolved.contains_key(field)));
    for (field, value) in &resolved {
        match stated.get(field) {
            // The campaign states a selector only where its decision leaves
            // the conservative value.
            Some(stated) => {
                assert_eq!(value, stated, "{field}");
                assert_ne!(value, &conservative[field], "{field}");
            }
            None => assert_eq!(value, &conservative[field], "{field}"),
        }
    }
}

/// The decode family of [`fresh::MEASURED_OWNER`] and the decode selectors
/// its body states.
fn measured_decode() -> (DecodeSelectors, Vec<String>) {
    let prepared = fresh::measured_owner();
    let decode = prepared
        .section::<CodingTuning>()
        .unwrap()
        .expect("the owner holds the coding section")
        .section
        .decode()
        .clone();
    let document: serde_json::Value = serde_json::from_str(fresh::MEASURED_OWNER).unwrap();
    let stated = fields(&document["sections"][CodingTuning::ID.as_str()]["selectors"])
        .into_keys()
        .filter_map(|field| field.strip_prefix("decode.").map(str::to_owned))
        .collect();
    (decode, stated)
}

/// The route [`gf2_coding::ldpc::decide_batch_route`] documents for a
/// min-sum wave of `frames` frames on a code of `edges` edges.
fn documented_route(
    decode: &DecodeSelectors,
    backend: LaneBackend,
    frames: usize,
    edges: usize,
) -> BatchRoute {
    if frames.min(LANES) >= decode.lane_min_frames() && edges <= decode.lane_max_edges() {
        BatchRoute::Lane(backend)
    } else {
        BatchRoute::SingleFrame
    }
}

#[test]
fn the_installed_envelope_routes_each_side_of_each_stated_selector() {
    let (decode, stated) = measured_decode();
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::MeasuredOwnerRoutes).unwrap();
    assert_eq!(result["decode"], fresh::decode_report(&decode));
    assert_eq!(result["resolution"], "installed");

    let mut observed = BTreeMap::new();
    for report in result["routes"].as_array().expect("a route list") {
        let frames = report["frames"].as_u64().unwrap() as usize;
        let edges = report["edges"].as_u64().unwrap() as usize;
        for (key, backend) in [
            ("detected", LaneBackend::detected()),
            ("portable", LaneBackend::Portable),
        ] {
            let expected = documented_route(&decode, backend, frames, edges);
            assert_eq!(
                report[key],
                fresh::route_report(expected),
                "{key}: {frames} frames, {edges} edges"
            );
        }
        observed.insert((frames, edges), report["detected"].clone());
    }

    let route = |frames: usize, edges: usize| {
        observed
            .get(&(frames, edges))
            .unwrap_or_else(|| panic!("no route reported for {frames} frames, {edges} edges"))
    };
    let (min_frames, max_edges) = (decode.lane_min_frames(), decode.lane_max_edges());
    assert!(!stated.is_empty());
    for field in &stated {
        let (below, at) = match field.as_str() {
            "lane_min_frames" => (
                route(min_frames.checked_sub(1).expect("a size below"), max_edges),
                route(min_frames, max_edges),
            ),
            "lane_max_edges" => (
                route(
                    LANES,
                    max_edges.checked_add(1).expect("an edge count above"),
                ),
                route(LANES, max_edges),
            ),
            // The QC route's own test covers its cases.
            "granules_per_task" | "qc_route_cases" => continue,
            other => panic!("{other} is no decode selector"),
        };
        assert_eq!(
            *below,
            fresh::route_report(BatchRoute::SingleFrame),
            "{field}"
        );
        assert_eq!(
            *at,
            fresh::route_report(BatchRoute::Lane(LaneBackend::detected())),
            "{field}"
        );
    }
}

#[test]
fn a_decode_under_the_installed_envelope_equals_the_single_frame_decoder() {
    let (decode, _) = measured_decode();
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::MeasuredOwnerDecode).unwrap();
    assert_eq!(result["decode"], fresh::decode_report(&decode));
    assert_eq!(result["resolution"], "installed");

    let code = test_support::ldpc_batch_code();
    let edges = EdgeLayout::from_parity_check(code.parity_check_matrix()).edges();
    let expected: Vec<serde_json::Value> = (0..=LANES)
        .map(|frames| {
            fresh::route_report(documented_route(
                &decode,
                LaneBackend::detected(),
                frames,
                edges,
            ))
        })
        .collect();
    assert_eq!(
        result["wave_routes"],
        serde_json::Value::from(expected.clone())
    );
    // The suite's batch sizes reach both routes, so the contract held on each.
    for route in [
        BatchRoute::SingleFrame,
        BatchRoute::Lane(LaneBackend::detected()),
    ] {
        assert!(
            expected[1..].contains(&fresh::route_report(route)),
            "{route:?}"
        );
    }
}

#[test]
fn each_committed_owner_decodes_under_the_codec_harness_schema() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tuning-profiles");
    let mut owners = Vec::new();
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            owners.push((std::fs::read_to_string(&path).unwrap(), path));
        }
    }
    assert!(owners
        .iter()
        .any(|(text, _)| text.as_str() == fresh::MEASURED_OWNER));

    let registry = fresh::coding_registry();
    for (text, path) in &owners {
        let prepared = registry
            .from_json(text)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let section = prepared
            .section::<CodingTuning>()
            .unwrap()
            .unwrap_or_else(|| panic!("{} holds no coding section", path.display()));
        if let MeasurementProvenance::Calibrated { harness_schema, .. } = section.measurement {
            assert_eq!(
                harness_schema.as_str(),
                CodingTuningCodec::HARNESS_SCHEMA,
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn fresh_tuning_process_child() {
    let Some(case) = fresh::child_case().expect("fresh-process protocol is valid") else {
        return;
    };
    fresh::emit_result(fresh::execute_child(case));
}

#[test]
fn committed_lane_domain_rejects_multicore_loss_configurations() {
    let (selectors, _) = measured_decode();
    for code in [
        test_support::ldpc_batch_code(),
        gf2_coding::ldpc::LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2),
    ] {
        let decoder = LdpcBatchDecoder::with_selectors(
            code,
            DecoderConfig::default(),
            1,
            DecodedWord::Codeword,
            selectors.clone(),
        );
        assert!(matches!(decoder.batch_route(LANES), BatchRoute::Lane(_)));
        for workers in [2, 6, 12, 24] {
            assert_eq!(
                decoder.batch_route_with_workers(LANES, NonZeroUsize::new(workers).unwrap()),
                BatchRoute::SingleFrame
            );
        }
    }
}

#[test]
fn the_committed_envelope_states_the_qc_route_cases_of_qc_go_rule() {
    let (selectors, stated) = measured_decode();
    assert!(stated.iter().any(|field| field == "qc_route_cases"));
    let case = |workers, frames| QcRouteCase::try_new(1, 384, workers, frames).unwrap();
    assert_eq!(selectors.qc_route_cases(), [case(1, 1), case(12, 8)]);
}

/// The installed envelope takes the QC route on attached NR BG1 Z=384 at
/// exactly its cases, on the detected backend and on the portable fallback,
/// and every other call takes the route the envelope's selectors without the
/// QC cases give it.
#[test]
fn the_installed_envelope_takes_the_qc_route_on_its_cases_and_keeps_every_other_route() {
    let (decode, _) = measured_decode();
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::MeasuredOwnerQc).unwrap();
    assert_eq!(result["decode"], fresh::decode_report(&decode));
    assert_eq!(result["resolution"], "installed");

    let config = DecoderConfig::new(DecoderAlgorithm::NormalizedMinSum(0.75), true);
    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
    let existing = |portable: bool| {
        let decoder = LdpcBatchDecoder::with_selectors(
            code.clone(),
            config,
            2,
            DecodedWord::Codeword,
            decode.clone().with_qc_route_cases(Vec::new()),
        );
        if portable {
            decoder.with_portable_lanes()
        } else {
            decoder
        }
    };
    let existing = [existing(false), existing(true)];

    let reports = result["routes"].as_array().expect("a route list");
    assert_eq!(reports.len(), 2 * 2 * fresh::QC_CALLS.len());
    let mut qc_calls = 0;
    for report in reports {
        let structured = report["structured"].as_bool().unwrap();
        let portable = report["portable"].as_bool().unwrap();
        let workers = report["workers"].as_u64().unwrap() as usize;
        let frames = report["frames"].as_u64().unwrap() as usize;
        let selected = structured
            && decode
                .qc_route_cases()
                .iter()
                .any(|case| case.workers().get() == workers && case.frames().get() == frames);
        let expected = if selected {
            qc_calls += 1;
            BatchRoute::Qc(if portable {
                LaneBackend::Portable
            } else {
                LaneBackend::detected()
            })
        } else {
            existing[usize::from(portable)]
                .batch_route_with_workers(frames, NonZeroUsize::new(workers).unwrap())
        };
        assert_eq!(report["route"], fresh::route_report(expected), "{report}");
        if structured {
            // The child also held each decoded call to the single-frame
            // decoder's words and outcomes.
            assert_eq!(report["executed"], report["route"], "{report}");
        }
    }
    assert_eq!(qc_calls, 2 * decode.qc_route_cases().len());
}

#[test]
fn committed_worker_default_preserves_the_envelope_bytes() {
    let prepared = fresh::measured_owner();
    let assembly = prepared.verified_assembly().unwrap().provenance.clone();
    assert_eq!(
        fresh::coding_registry()
            .to_json(&prepared, &assembly)
            .unwrap(),
        fresh::MEASURED_OWNER
    );
}
