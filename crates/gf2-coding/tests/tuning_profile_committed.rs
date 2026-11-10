//! The committed coding owner envelopes read through the owner codec, and
//! the batch decoder's routes and results under the committed production
//! envelope, installed in a fresh process.

#![cfg(feature = "tuning-profile")]

#[path = "support/fresh_tuning_process.rs"]
mod fresh;

use std::collections::BTreeMap;
use std::path::Path;

use gf2_coding::ldpc::QcRouteBounds;
use gf2_coding::ldpc::{
    decide_batch_route, BatchRoute, DecodedWord, DecoderAlgorithm, DecoderConfig, EdgeLayout,
    LaneBackend, LaneCapability, LdpcBatchDecoder, LdpcCode, QcCapability, QuasiCyclicLdpc,
};
use gf2_coding::test_support;
use gf2_coding::tuning::{CodingTuning, CodingTuningCodec, DecodeSelectors};
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
/// min-sum wave of `frames` frames at one worker on a code of `edges` edges
/// without QC structure.
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
            // The QC route's own test covers its bounds.
            "granules_per_task" => continue,
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
    let backend = LaneBackend::detected();
    let qc = QcCapability::Present {
        lifting_size: code.quasi_cyclic().unwrap().expansion_factor(),
        backend,
    };
    let unstructured: Vec<serde_json::Value> = (0..=LANES)
        .map(|frames| fresh::route_report(documented_route(&decode, backend, frames, edges)))
        .collect();
    let structured: Vec<serde_json::Value> = (0..=LANES)
        .map(|frames| {
            fresh::route_report(decide_batch_route(
                &decode,
                DecoderConfig::default().algorithm(),
                LaneCapability::Present(backend),
                qc,
                frames,
                edges,
                NonZeroUsize::MIN,
            ))
        })
        .collect();
    assert_eq!(
        result["wave_routes"]["unstructured"],
        serde_json::Value::from(unstructured.clone())
    );
    assert_eq!(
        result["wave_routes"]["structured"],
        serde_json::Value::from(structured.clone())
    );
    // The suite's batch sizes reach each route, so the contract held on each.
    for (routes, route) in [
        (&unstructured, BatchRoute::SingleFrame),
        (&unstructured, BatchRoute::Lane(backend)),
        (&structured, BatchRoute::Qc(backend)),
    ] {
        assert!(
            routes[1..].contains(&fresh::route_report(route)),
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
        fresh::unstructured_batch_code(),
        gf2_coding::ldpc::LdpcCode::dvb_t2_normal(gf2_coding::CodeRate::Rate1_2)
            .without_quasi_cyclic(),
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

/// The committed envelope states no QC bounds, so its eligible lane route
/// claims full waves of NR codes up to the structural ceiling of the detected
/// QC backend, at every worker count of the route sweep, and every other wave
/// takes the QC route.
#[test]
fn the_committed_envelope_leaves_the_qc_route_to_its_structural_bounds() {
    let (selectors, stated) = measured_decode();
    assert!(!stated.iter().any(|field| field == "qc_route"));
    assert_eq!(selectors.qc_route(), None);
    let ceiling = QcRouteBounds::structural(LaneBackend::detected()).lane_claim_max_lifting_size();
    for base_graph in [1, 2] {
        // NR lifting sizes on both sides of the AVX2 and portable ceilings.
        for z in [2, 3, 4, 6, 24, 32, 384] {
            let decoder = LdpcBatchDecoder::with_selectors(
                LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(base_graph, z)),
                DecoderConfig::default(),
                1,
                DecodedWord::Codeword,
                selectors.clone(),
            );
            for frames in [1, LANES - 1, LANES] {
                for workers in [1, 6, 12, 24] {
                    let route = decoder
                        .batch_route_with_workers(frames, NonZeroUsize::new(workers).unwrap());
                    let lanes_claim = z <= ceiling && frames == LANES && workers == 1;
                    assert_eq!(
                        matches!(route, BatchRoute::Lane(_)),
                        lanes_claim,
                        "BG{base_graph} Z {z}, {frames} frames, {workers} workers: {route:?}"
                    );
                    assert_eq!(
                        matches!(route, BatchRoute::Qc(_)),
                        !lanes_claim,
                        "BG{base_graph} Z {z}, {frames} frames, {workers} workers: {route:?}"
                    );
                }
            }
        }
    }
}

/// The installed envelope routes each of [`fresh::QC_CALLS`] on NR BG1
/// Z=384, with its structure attached and without it, on the detected
/// backend and on the portable fallback, as
/// [`decide_batch_route`] documents for its decode family.
#[test]
fn the_installed_envelope_routes_each_qc_call_as_its_bounds_document() {
    let (decode, _) = measured_decode();
    let result = fresh::fresh_tuning_process(fresh::FreshProcessCase::MeasuredOwnerQc).unwrap();
    assert_eq!(result["decode"], fresh::decode_report(&decode));
    assert_eq!(result["resolution"], "installed");

    let code = LdpcCode::from_quasi_cyclic(&QuasiCyclicLdpc::nr_5g(1, 384));
    let edges = EdgeLayout::from_parity_check(code.parity_check_matrix()).edges();
    let reports = result["routes"].as_array().expect("a route list");
    assert_eq!(reports.len(), 2 * 2 * fresh::QC_CALLS.len());
    let mut qc_calls = 0;
    for report in reports {
        let structured = report["structured"].as_bool().unwrap();
        let portable = report["portable"].as_bool().unwrap();
        let workers = report["workers"].as_u64().unwrap() as usize;
        let frames = report["frames"].as_u64().unwrap() as usize;
        let backend = if portable {
            LaneBackend::Portable
        } else {
            LaneBackend::detected()
        };
        let qc = if structured {
            QcCapability::Present {
                lifting_size: 384,
                backend,
            }
        } else {
            QcCapability::Absent
        };
        let expected = decide_batch_route(
            &decode,
            DecoderAlgorithm::NormalizedMinSum(0.75),
            LaneCapability::Present(backend),
            qc,
            frames.min(LANES),
            edges,
            NonZeroUsize::new(workers).unwrap(),
        );
        qc_calls += usize::from(matches!(expected, BatchRoute::Qc(_)));
        assert_eq!(report["route"], fresh::route_report(expected), "{report}");
        if structured {
            // The child also held each decoded call to the single-frame
            // decoder's words and outcomes.
            assert_eq!(report["executed"], report["route"], "{report}");
        }
    }
    // The structural bounds put every structured Z=384 call on the QC route.
    assert_eq!(qc_calls, 2 * fresh::QC_CALLS.len());
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
