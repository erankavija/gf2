//! Behavioral contract of the dense-parity measurement harness.
//!
//! The survey harness is its own workspace, so the repository CI contract
//! never reaches it: these tests and the launcher's `build` subcommand are the
//! only thing that runs it.

use dense_parity_harness::campaign::{self, PlanInputs, M4RI_ARM};
use dense_parity_harness::cells::{
    cells, family_cells, Cache, MatvecShape, Question, ALL_WORDS, ANCHOR_WORDS, BOUNDARY_BITS,
    CAMPAIGN_SEED, MATVEC_ROWS, M4RI_SHAPES, SIMD_LANE_MIN_WORDS, STREAMING_BANKS,
    STREAMING_BANK_BYTES, UNAVAILABLE_ROWS,
};
use dense_parity_harness::external::{self, LibraryIdentity};
use dense_parity_harness::fixture::{KernelBanks, MatvecBanks};
use dense_parity_harness::inputs;
use dense_parity_harness::oracle;
use dense_parity_harness::routes::{
    run_windows, verify_lane, verify_shape, OutputSink, Route, WindowPlan, MAX_RETAINED_OUTPUTS,
    RETAINED_BUDGET_BYTES, RETAINED_OUTPUT_BYTES, RETAINED_PEAK_BYTES,
};
use dense_parity_harness::wire::{Case, Request};
use gf2_core::BitVec;
use std::collections::BTreeSet;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::protocol::{
    CellRole, FamilyAddendum, MetricKind, ReceiptLabel, SHARED_SETTINGS,
};
use tuning_campaign_support::transport;

const ISSUE: &str = "e1f9a78f";
const FROZEN: &str = "2026-09-16T16:16:04Z";

fn plan_inputs<'a>(
    scalar: Option<&'a str>,
    m4ri: Option<&'a str>,
    max_cells: Option<u32>,
    pilot_pairs: Option<u32>,
) -> PlanInputs<'a> {
    PlanInputs {
        campaign_id: "e1f9a78f-contract-plan",
        campaign_seed: 1,
        label: ReceiptLabel::Pilot,
        addendum_path: "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaign.json",
        producing_manifest: MANIFEST,
        lock_path: "/tmp/gf2-contract.lock",
        gf2_executable: "/nonexistent/dense-arm",
        candidate_executable: None,
        scalar_executable: scalar,
        m4ri_executable: m4ri,
        max_cells_per_session: max_cells,
        pilot_pairs,
    }
}

#[test]
fn the_cell_table_reproduces_the_frozen_matrices() {
    let table = cells();
    let counts: Vec<usize> = Question::ALL
        .iter()
        .map(|question| family_cells(*question).len())
        .collect();
    // Isolated: 5 anchors, 2 neighbours, 5 streaming. Allocated: 5 anchors,
    // 2 neighbours, 5 tail1, 2 cold, 5 streaming, 5 scalar-reference. M4RI:
    // 6 qualified shapes plus 2 retained-state cells.
    assert_eq!(counts, vec![12, 24, 8]);
    assert_eq!(table.len(), counts.iter().sum::<usize>());

    let ids: Vec<&str> = table.iter().map(|cell| cell.cell_id.as_str()).collect();
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    assert_eq!(ids[0], "and-popcnt-8w-warm");
    assert_eq!(ids[5], "and-popcnt-7w-warm");
    assert_eq!(ids[7], "and-popcnt-8w-streaming");
    assert_eq!(ids[12], "matvec-r1024-8w-warm");
    assert_eq!(ids[19], "matvec-r1024-8w-tail1-warm");
    assert_eq!(ids[24], "matvec-r1024-8w-cold");
    assert_eq!(ids[26], "matvec-r1024-8w-streaming");
    assert_eq!(ids[31], "matvec-r1024-8w-scalar-reference-warm");
    assert_eq!(ids[36], "m4ri-gap-65x512-warm");
    assert_eq!(ids[42], "m4ri-gap-65x512-retained-warm");

    // Anchors are the confirmatory family of each question and are warm.
    let anchors: Vec<usize> = Question::ALL
        .iter()
        .map(|question| family_cells(*question).iter().filter(|cell| cell.anchor).count())
        .collect();
    assert_eq!(anchors, vec![5, 5, 2]);
    assert!(table.iter().all(|cell| !cell.anchor || cell.cache == Cache::Warm));

    // The scalar-reference rows are exploratory cells of the allocated family.
    let reference: Vec<&str> = table
        .iter()
        .filter(|cell| cell.scalar_reference)
        .map(|cell| cell.cell_id.as_str())
        .collect();
    assert_eq!(reference.len(), ANCHOR_WORDS.len());
    assert!(reference.iter().all(|id| id.contains("scalar-reference")));
}

#[test]
fn the_unavailable_comparator_rows_take_no_ordinal() {
    let table = cells();
    let declared: BTreeSet<&str> = table.iter().map(|cell| cell.cell_id.as_str()).collect();
    for row in UNAVAILABLE_ROWS {
        assert!(!declared.contains(row.row_id), "{}", row.row_id);
        assert!(!row.reason.trim().is_empty());
    }
    let strides: BTreeSet<usize> = UNAVAILABLE_ROWS.iter().map(|row| row.stride_words).collect();
    assert_eq!(strides, BTreeSet::from([9, 63, 65]));
}

#[test]
fn seeds_follow_one_campaign_splitmix_stream() {
    let table = cells();
    let mut mixer = SplitMix64::new(CAMPAIGN_SEED);
    assert_eq!(table[0].seed, CAMPAIGN_SEED);
    for cell in table.iter().skip(1) {
        assert_eq!(cell.seed, mixer.next_u64(), "ordinal {}", cell.ordinal);
    }
    let distinct: BTreeSet<u64> = table.iter().map(|cell| cell.seed).collect();
    assert_eq!(distinct.len(), table.len());
    for (ordinal, cell) in table.iter().enumerate() {
        assert_eq!(cell.ordinal, ordinal);
    }
}

#[test]
fn cell_generation_is_deterministic() {
    assert_eq!(cells(), cells());
}

#[test]
fn every_family_transcribes_into_a_valid_version_four_addendum() {
    for question in Question::ALL {
        let addendum = campaign::addendum(question, ISSUE, FROZEN);
        addendum
            .validate()
            .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
        assert_eq!(addendum.family.id, question.family_id());
        assert_eq!(
            addendum.family_wise.ledger_path.as_deref(),
            Some(question.ledger_path())
        );
        assert_eq!(addendum.family_wise.alpha, SHARED_SETTINGS.family_alpha);
        // A pilot transcription resolves no measurement resolution, so no cell
        // of it can be confirmatory.
        assert!(addendum.effect.measurement_resolution.is_none());
        assert!(addendum.effect.resolution_evidence.is_none());
        assert!(addendum.cells.iter().all(|cell| cell.role == CellRole::Exploratory));
        assert_eq!(addendum.cells.len(), family_cells(question).len());

        let encoded = serde_json::to_vec(&addendum).expect("encodes");
        assert_eq!(FamilyAddendum::decode(&encoded).expect("decodes"), addendum);
    }
}

#[test]
fn the_frozen_margins_reach_each_family_unchanged() {
    let isolated = campaign::addendum(Question::IsolatedFusedParity, ISSUE, FROZEN);
    assert_eq!(isolated.effect.worthwhile_speedup, Some(1.10));
    assert_eq!(isolated.effect.equivalence_margin, Some(1.03));
    assert_eq!(isolated.effect.material_gap_threshold, None);
    assert_eq!(isolated.complexity_budget.max_added_source_lines, Some(96));

    let allocated = campaign::addendum(Question::AllocatedMatvec, ISSUE, FROZEN);
    assert_eq!(allocated.effect.worthwhile_speedup, Some(1.08));
    assert_eq!(allocated.effect.equivalence_margin, Some(1.05));
    assert_eq!(allocated.effect.material_gap_threshold, None);

    let comparator = campaign::addendum(Question::MatvecVsM4ri, ISSUE, FROZEN);
    assert_eq!(comparator.effect.worthwhile_speedup, None);
    assert_eq!(comparator.effect.equivalence_margin, Some(1.05));
    assert_eq!(comparator.effect.material_gap_threshold, Some(1.10));
    assert_eq!(comparator.complexity_budget.max_new_unsafe_kernels, 0);
    assert_eq!(comparator.complexity_budget.max_added_source_lines, Some(0));
}

#[test]
fn a_changed_margin_or_cell_no_longer_matches_the_transcription() {
    let derived = campaign::addendum(Question::AllocatedMatvec, ISSUE, FROZEN);

    let mut loosened = derived.clone();
    loosened.effect.worthwhile_speedup = Some(1.01);
    assert_ne!(loosened, derived);

    let mut promoted = derived.clone();
    promoted.cells[0].role = CellRole::Confirmatory;
    assert_ne!(promoted, derived);

    let mut dropped = derived.clone();
    dropped.cells.pop();
    assert_ne!(dropped, derived);

    let mut reseeded = derived.clone();
    reseeded.cells[0].workload.seed ^= 1;
    assert_ne!(reseeded, derived);
}

#[test]
fn the_request_mirror_accepts_exactly_the_runner_request() {
    // These are the exact bytes `benchmark-ab-runner` writes: its own struct
    // field order, with the case object as the plan's JSON map. The runner
    // omits `cold_calls` and `decoder` for a cell that declares neither, and a
    // mirror that spells them `null` rejects the request.
    let warm = concat!(
        r#"{"schema":"zen3-benchmark-arm-request-v1","cell_id":"matvec-r1024-8w-warm","#,
        r#""arm":"matvec-a","role":"baseline","pair":0,"#,
        r#""case":{"question":"allocated-matvec","seed":7,"shape":"full","words":8},"#,
        r#""cache_state":"warm","windows":5,"window_target_ms":100,"cpus":[0],"#,
        r#""workers_declared":1}"#
    );
    let decoded: Request = transport::decode_case(warm).expect("the mirror decodes");
    assert_eq!(decoded.cell_id, "matvec-r1024-8w-warm");
    assert!(decoded.cold_calls.is_none());
    assert!(decoded.decoder.is_none());

    let with_nulls = warm.replace(
        r#""cache_state":"warm""#,
        r#""cache_state":"warm","cold_calls":null,"decoder":null"#,
    );
    assert!(
        transport::decode_case::<Request>(&with_nulls).is_err(),
        "a null-spelled optional is not the runner's canonical request"
    );

    let cold = concat!(
        r#"{"schema":"zen3-benchmark-arm-request-v1","cell_id":"matvec-r1024-8w-cold","#,
        r#""arm":"matvec-a","role":"baseline","pair":0,"#,
        r#""case":{"question":"allocated-matvec","seed":7,"shape":"full","words":8},"#,
        r#""cache_state":"cold","cold_calls":1,"windows":5,"window_target_ms":100,"#,
        r#""cpus":[0],"workers_declared":1}"#
    );
    let decoded: Request = transport::decode_case(cold).expect("the mirror decodes");
    assert_eq!(decoded.cold_calls, Some(1));

    let comparator = concat!(
        r#"{"schema":"zen3-benchmark-arm-request-v1","cell_id":"m4ri-gap-65x512-warm","#,
        r#""arm":"m4ri-mzd-mul","role":"candidate","pair":0,"#,
        r#""case":{"cols":512,"question":"matvec-vs-m4ri","retained":false,"rows":65,"seed":7},"#,
        r#""cache_state":"warm","windows":5,"window_target_ms":100,"cpus":[0],"#,
        r#""workers_declared":1}"#
    );
    let decoded: Request = transport::decode_case(comparator).expect("the mirror decodes");
    let case: Case = serde_json::from_value(decoded.case).expect("the case decodes");
    assert_eq!(case.seed(), 7);
}

#[test]
fn every_frozen_case_round_trips_and_names_its_workload() {
    for cell in cells() {
        let case = Case::of(&cell);
        let value = serde_json::to_value(&case).expect("encodes");
        let back: Case = serde_json::from_value(value).expect("decodes");
        assert_eq!(back, case);
        assert_eq!(case.seed(), cell.seed);
        assert_eq!(case.workload().expect("frozen workload"), cell.workload);
    }
}

#[test]
fn an_unfrozen_case_is_refused() {
    let unqualified_shape = Case::MatvecVsM4ri { rows: 65, cols: 576, retained: false, seed: 1 };
    assert!(unqualified_shape.workload().is_err());

    let unfrozen_shape = Case::AllocatedMatvec { words: 8, shape: "tail17".to_owned(), seed: 1 };
    assert!(unfrozen_shape.workload().is_err());
}

#[test]
fn isolated_operands_begin_on_the_declared_boundary() {
    for words in ALL_WORDS {
        let banks = KernelBanks::build(words, Cache::Warm, 1);
        assert_eq!(banks.addresses_mod_64(0, 0), (0, 0), "{words}");
        let (row, vector) = banks.operands(0, 0);
        assert_eq!(row.len(), words);
        assert_eq!(vector.len(), words);
    }
}

#[test]
fn a_tail1_fixture_carries_canonical_zero_tail_padding() {
    for words in ALL_WORDS {
        let columns = MatvecShape::Tail1.columns(words);
        let banks = MatvecBanks::build(65, columns, Cache::Warm, 3);
        let item = banks.item(0, 0);
        let tail = columns % 64;
        assert_ne!(tail, 0);
        for row in 0..item.matrix.rows() {
            let row_words = item.matrix.row_words(row);
            assert_eq!(row_words[row_words.len() - 1] >> tail, 0, "{words} row {row}");
        }
        let vector_words = item.vector.words();
        assert_eq!(vector_words[vector_words.len() - 1] >> tail, 0, "{words}");
    }
}

#[test]
fn a_streaming_working_set_fills_every_declared_bank() {
    let kernel = KernelBanks::build(8, Cache::Streaming, 1);
    assert!(kernel.items() > 1);
    assert!(kernel.working_set_bytes() >= STREAMING_BANKS * STREAMING_BANK_BYTES);
    let allocated = MatvecBanks::allocated(8, MatvecShape::Full, Cache::Streaming, 1);
    assert!(allocated.working_set_bytes() >= STREAMING_BANKS * STREAMING_BANK_BYTES);
}

#[test]
fn warm_runs_one_untimed_pass_and_streaming_runs_none() {
    // A zero-window plan is the non-timed arrangement pass: it stops before
    // the timing protocol, so every body call observed here is the cache
    // policy's own untimed pass and the execution reports no sample.
    let mut calls = Vec::new();
    let samples = run_windows(
        WindowPlan {
            cache: Cache::Warm,
            cold_calls: None,
            windows: 0,
            window_target_ms: 100,
            banks: 1,
            items: 1,
        },
        &mut |bank, item| calls.push((bank, item)),
        |_| Ok(()),
    )
    .expect("the arrangement pass succeeds");
    assert!(samples.is_empty());
    assert_eq!(calls, vec![(0, 0)]);

    let mut streaming = 0;
    let samples = run_windows(
        WindowPlan {
            cache: Cache::Streaming,
            cold_calls: None,
            windows: 0,
            window_target_ms: 100,
            banks: STREAMING_BANKS,
            items: 4,
        },
        &mut |_, _| streaming += 1,
        |_| Ok(()),
    )
    .expect("the arrangement pass succeeds");
    assert!(samples.is_empty());
    assert_eq!(streaming, 0);
}

#[test]
fn a_zero_window_arrangement_pass_collects_no_timing_sample() {
    for (cache, cold_calls) in [
        (Cache::Warm, None),
        (Cache::Streaming, None),
        (Cache::Cold, Cache::Cold.cold_calls()),
    ] {
        let samples = run_windows(
            WindowPlan {
                cache,
                cold_calls,
                windows: 0,
                window_target_ms: 0,
                banks: 1,
                items: 1,
            },
            &mut |_, _| {},
            |_| panic!("a zero-window plan reports no timing progress"),
        )
        .expect("the arrangement pass succeeds");
        assert!(samples.is_empty(), "{}", cache.id());
    }

    // A zero-window plan still refuses a cache state paired with the wrong
    // frozen call count, so the smoke cannot arrange an undeclared cell.
    assert!(run_windows(
        WindowPlan {
            cache: Cache::Warm,
            cold_calls: Some(1),
            windows: 0,
            window_target_ms: 0,
            banks: 1,
            items: 1,
        },
        &mut |_, _| {},
        |_| Ok(()),
    )
    .is_err());
}

#[test]
fn a_cache_state_and_its_frozen_call_count_must_agree() {
    for (cache, calls) in [
        (Cache::Warm, Some(1)),
        (Cache::Streaming, Some(1)),
        (Cache::Cold, None),
    ] {
        let plan = WindowPlan {
            cache,
            cold_calls: calls,
            windows: 5,
            window_target_ms: 1,
            banks: 1,
            items: 1,
        };
        assert!(run_windows(plan, &mut |_, _| {}, |_| Ok(())).is_err());
    }
    assert_eq!(Cache::Cold.cold_calls(), Some(1));
    assert_eq!(Cache::Warm.cold_calls(), None);
    assert!(Cache::from_request("tepid").is_err());
}

#[test]
fn a_window_at_the_retention_bound_releases_no_output_until_it_closes() {
    let calls = MAX_RETAINED_OUTPUTS as u64;
    let mut sink = OutputSink::default();
    sink.admit("matvec-r1024-8w-warm", calls).expect("the bound is admissible");
    for call in 0..calls {
        sink.keep(BitVec::zeros(MATVEC_ROWS));
        assert_eq!(sink.held(), call as usize + 1);
    }
    sink.release_window(calls).expect("the window retained every output");
    assert_eq!(sink.held(), 0);
}

#[test]
fn a_window_over_the_retention_bound_is_refused_before_it_opens() {
    let over = MAX_RETAINED_OUTPUTS as u64 + 1;
    let mut sink = OutputSink::default();
    let refusal = sink
        .admit("matvec-r1024-8w-warm", over)
        .expect_err("a count above the bound is refused");
    for fragment in ["matvec-r1024-8w-warm", &over.to_string(), &MAX_RETAINED_OUTPUTS.to_string()] {
        assert!(refusal.contains(fragment), "{refusal}");
    }
    assert_eq!(sink.held(), 0);

    // The refusal precedes the window: the count of a fixed-call cell is known
    // before `run_windows`, and a calibrated count reaches the sink from the
    // post-calibration callback, whose error stops the execution with no sample.
    let mut kept = 0_usize;
    let outcome = run_windows(
        WindowPlan {
            cache: Cache::Warm,
            cold_calls: None,
            windows: 5,
            window_target_ms: 1,
            banks: 1,
            items: 1,
        },
        &mut |_, _| kept += 1,
        |progress| match progress {
            tuning_campaign_support::timing::TimingProgress::CalibrationComplete { .. } => {
                Err(std::io::Error::other(refusal.clone()))
            }
            tuning_campaign_support::timing::TimingProgress::WindowComplete(_) => {
                panic!("a refused execution opens no window")
            }
        },
    );
    assert!(outcome.is_err());
    assert!(kept > 0);
}

#[test]
fn the_retention_bound_is_derived_from_the_largest_declared_retaining_cell() {
    // Every cell that retains an output is an allocated-matvec cell, so the
    // largest declared retained output is one MATVEC_ROWS-bit `BitVec`.
    let retaining = cells()
        .iter()
        .filter(|cell| matches!(cell.workload, dense_parity_harness::Workload::Matvec { .. }))
        .count();
    assert_eq!(retaining, family_cells(Question::AllocatedMatvec).len());
    assert_eq!(BitVec::zeros(MATVEC_ROWS).words().len(), MATVEC_ROWS.div_ceil(64));
    assert_eq!(RETAINED_OUTPUT_BYTES, size_of::<BitVec>() + MATVEC_ROWS / 64 * 8 + 16);
    assert_eq!(MAX_RETAINED_OUTPUTS, RETAINED_BUDGET_BYTES / RETAINED_OUTPUT_BYTES);
    assert_eq!(RETAINED_PEAK_BYTES, MAX_RETAINED_OUTPUTS * RETAINED_OUTPUT_BYTES);
    assert!(RETAINED_PEAK_BYTES <= RETAINED_BUDGET_BYTES);

    // The bound covers a window of the protocol's target length whenever one
    // call costs at least this many nanoseconds; `admit` refuses a faster cell
    // before its window opens, so the floor is a declared limit, not a host
    // claim. A prior per-call cost of this family is the row
    // `matvec-1024x4096` of `dev/bench_results/5cbb6545/tables.md`; it is
    // context and decides nothing here.
    let covered_ns_per_call =
        u64::from(SHARED_SETTINGS.window_target_ms) * 1_000_000 / MAX_RETAINED_OUTPUTS as u64;
    assert!(covered_ns_per_call < 1_000, "{covered_ns_per_call}");
}

#[test]
fn a_windowed_execution_retains_and_releases_through_the_arm_arrangement() {
    // The arm's arrangement: the body retains each output and the post-window
    // callback releases the batch. A zero-window smoke never reaches it, so the
    // borrow discipline and the release boundary are checked here. The windows
    // are one millisecond of a trivial body and carry no performance claim.
    let banks = MatvecBanks::build(65, 512, Cache::Warm, 11);
    let retained = std::cell::RefCell::new(OutputSink::default());
    let mut body = |bank: usize, item: usize| {
        let item = banks.item(bank, item);
        retained.borrow_mut().keep(item.matrix.matvec(&item.vector));
    };
    let mut released = 0usize;
    let samples = run_windows(
        WindowPlan {
            cache: Cache::Warm,
            cold_calls: None,
            windows: 1,
            window_target_ms: 1,
            banks: 1,
            items: 1,
        },
        &mut body,
        |progress| {
            let mut retained = retained.borrow_mut();
            match progress {
                tuning_campaign_support::timing::TimingProgress::CalibrationComplete { calls } => {
                    retained.admit("matvec-r1024-8w-warm", calls)
                }
                tuning_campaign_support::timing::TimingProgress::WindowComplete(sample) => {
                    released += 1;
                    // The window retained one output per call and released none
                    // until here, which `release_window` refuses otherwise.
                    retained.release_window(sample.calls)
                }
            }
            .map_err(std::io::Error::other)
        },
    )
    .expect("the windowed execution succeeds");
    assert_eq!(samples.len(), 1);
    assert!(samples[0].calls > 0);
    assert_eq!(released, 1);
    assert_eq!(retained.borrow().held(), 0);
}

#[test]
fn the_oracle_covers_every_frozen_boundary_and_passes() {
    let report = oracle::run().expect("the oracle passes");
    let names: BTreeSet<&str> = report.iter().map(|case| case.name.as_str()).collect();
    for columns in BOUNDARY_BITS {
        for rows in BOUNDARY_BITS {
            assert!(
                names.contains(format!("matvec-cols-{columns}-rows-{rows}").as_str()),
                "{columns}x{rows}"
            );
        }
    }
    for words in ALL_WORDS {
        for shape in [MatvecShape::Full, MatvecShape::Tail1] {
            assert!(names.contains(format!("matvec-{words}w-{}", shape.id()).as_str()));
        }
        assert!(names.contains(format!("and-popcnt-{words}w").as_str()));
    }
    for words in ANCHOR_WORDS {
        assert!(names.contains(format!("matvec-anchor-{words}w").as_str()));
    }
    assert!(report.iter().all(|case| case.checks > 0));
}

#[test]
fn every_anchor_stride_resolves_the_lane_its_build_fixes() {
    let route = if cfg!(feature = "simd") { Route::MatvecA } else { Route::MatvecScalarReference };
    for words in ANCHOR_WORDS {
        assert!(words >= SIMD_LANE_MIN_WORDS);
        let columns = MatvecShape::Full.columns(words);
        let banks = MatvecBanks::allocated(words, MatvecShape::Full, Cache::Warm, 5);
        let item = banks.item(0, 0);
        let facts = verify_shape(&item.matrix, &item.vector, MATVEC_ROWS, columns)
            .expect("the frozen shape holds");
        assert_eq!(facts.stride_words, words);
        verify_lane(facts, route).expect("the build's lane");
        // The other build cannot stand in: its lane differs at every anchor.
        let other = if route == Route::MatvecA {
            Route::MatvecScalarReference
        } else {
            Route::MatvecA
        };
        assert!(verify_lane(facts, other).is_err(), "{words}");
    }
}

#[test]
fn a_mismatched_shape_makes_the_cell_unavailable() {
    let banks = MatvecBanks::allocated(8, MatvecShape::Full, Cache::Warm, 7);
    let item = banks.item(0, 0);
    assert!(verify_shape(&item.matrix, &item.vector, MATVEC_ROWS, 512).is_ok());
    assert!(verify_shape(&item.matrix, &item.vector, MATVEC_ROWS + 1, 512).is_err());
    assert!(verify_shape(&item.matrix, &item.vector, MATVEC_ROWS, 576).is_err());
}

#[test]
fn a_route_refuses_the_build_it_cannot_serve() {
    for route in Route::ALL {
        let served = route.check_build().is_ok();
        assert_eq!(served, route.simd_build() == cfg!(feature = "simd"), "{}", route.id());
    }
    assert_eq!(Route::MatvecA.question(), Question::AllocatedMatvec);
    assert_eq!(Route::MatvecScalarReference.question(), Question::AllocatedMatvec);
    assert_eq!(Route::AndPopcntB.question(), Question::IsolatedFusedParity);
    assert_eq!(Route::M4riPeerGf2.question(), Question::MatvecVsM4ri);
    assert!(Route::parse("matvec-c").is_err());
}

#[test]
fn a_cell_workload_names_the_question_every_arm_of_that_cell_serves() {
    // The gf2 arm refuses a cell whose workload belongs to another question,
    // so the workload's question and the plan's arms agree on every frozen cell.
    for cell in cells() {
        assert_eq!(cell.workload.question(), cell.question, "{}", cell.cell_id);
        let (baseline, candidate) = campaign::cell_arms(&cell);
        for arm in [baseline, candidate] {
            match Route::parse(arm) {
                Ok(route) => assert_eq!(route.question(), cell.question, "{}", cell.cell_id),
                Err(_) => assert_eq!(arm, M4RI_ARM, "{}", cell.cell_id),
            }
        }
    }
}

/// One qualified install's mapping as the kernel reports it, beside the
/// entries a mapping table carries that name no loadable comparator: an
/// anonymous mapping, a pseudo-file, a different library, and a backing file
/// the kernel has marked deleted.
const MAPS: &str = "\
7f2c00000000-7f2c00021000 r--p 00000000 08:02 5241987 /opt/m4ri/lib/libm4ri.so.2.0.1
7f2c00021000-7f2c00100000 r-xp 00021000 08:02 5241987 /opt/m4ri/lib/libm4ri.so.2.0.1
7f2c00200000-7f2c00280000 r-xp 00000000 08:02 5241000 /usr/lib/libc.so.6
7f2c00300000-7f2c00321000 rw-p 00000000 00:00 0 
7f2c00400000-7f2c00421000 rw-p 00000000 00:00 0 [heap]
7f2c00500000-7f2c00521000 r-xp 00000000 08:02 5241988 /tmp/stale/libm4ri.so.2 (deleted)
";

#[test]
fn the_mapping_table_names_the_comparator_object_this_process_loaded() {
    let mapped = external::mapped_libraries(MAPS, "m4ri");
    assert_eq!(
        mapped.into_iter().collect::<Vec<_>>(),
        vec!["/opt/m4ri/lib/libm4ri.so.2.0.1"]
    );
    assert!(external::mapped_libraries(MAPS, "gmp").is_empty());

    // The real table is the authority: this build links no comparator, so it
    // maps none and the arm's check refuses rather than assuming one.
    let found = external::loaded_library("m4ri");
    if cfg!(feature = "m4ri") {
        found.expect("an m4ri build maps the library it links");
    } else {
        let refusal = found.expect_err("this build links no m4ri");
        assert!(refusal.contains("no libm4ri.so is mapped"), "{refusal}");
    }
}

#[test]
fn a_substituted_comparator_library_refuses_a_timed_run() {
    let identity = LibraryIdentity {
        path: "/opt/m4ri/lib/libm4ri.so.2.0.1".to_owned(),
        sha256: "0".repeat(64),
    };
    let record = dense_parity_harness::cells::QUALIFICATION_RECORD;
    external::verify_pinned(&identity, &identity.sha256, record).expect("the qualified object");

    let pinned = "1".repeat(64);
    let refusal = external::verify_pinned(&identity, &pinned, record)
        .expect_err("a different object refuses");
    assert!(refusal.contains(&identity.path), "{refusal}");
    assert!(refusal.contains(&identity.sha256), "{refusal}");
    assert!(refusal.contains(&pinned), "{refusal}");
    assert!(refusal.contains(record), "{refusal}");
}

#[test]
fn the_comparator_provenance_names_the_object_it_loaded() {
    let identity = LibraryIdentity {
        path: "/opt/m4ri/lib/libm4ri.so.2.0.1".to_owned(),
        sha256: "a".repeat(64),
    };
    let shape = M4RI_SHAPES[0];
    let path = external::comparator_selected_path(shape, false, &identity);
    assert!(path.contains("m4ri/mzd_mul(y,A,x,0)/fresh-whole-consumer"), "{path}");
    assert!(path.contains(&format!("rows={}/cols={}", shape.rows, shape.cols)), "{path}");
    assert!(path.contains(&format!("loaded={}", identity.path)), "{path}");
    assert!(path.contains(&format!("sha256={}", identity.sha256)), "{path}");
    assert!(
        external::comparator_selected_path(shape, true, &identity).contains("retained-state")
    );
}

#[test]
fn every_anchor_stride_is_a_confirmatory_cell_of_both_gf2_questions() {
    for question in [Question::IsolatedFusedParity, Question::AllocatedMatvec] {
        let anchors: BTreeSet<usize> = family_cells(question)
            .iter()
            .filter(|cell| cell.anchor)
            .map(|cell| cell.workload.stride_words())
            .collect();
        assert_eq!(anchors, ANCHOR_WORDS.into_iter().collect::<BTreeSet<_>>());
    }
}

#[test]
fn the_projected_plan_covers_every_declared_cell_with_declared_builds() {
    let expected_arms = [
        (Question::IsolatedFusedParity, 2),
        (Question::AllocatedMatvec, 3),
        (Question::MatvecVsM4ri, 2),
    ];
    for (question, arms) in expected_arms {
        let addendum = campaign::addendum(question, ISSUE, FROZEN);
        let plan = campaign::plan(
            question,
            &addendum,
            &plan_inputs(
                Some("/nonexistent/dense-arm-scalar"),
                Some("/nonexistent/dense-m4ri-arm"),
                Some(2),
                Some(6),
            ),
        )
        .expect("the plan projects");
        plan.validate(&addendum)
            .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
        assert_eq!(plan.cells.len(), addendum.cells.len());
        assert_eq!(plan.arms.len(), arms, "{}", question.family_id());
    }
}

#[test]
fn each_external_and_reference_arm_needs_its_own_executable() {
    let comparator = campaign::addendum(Question::MatvecVsM4ri, ISSUE, FROZEN);
    assert!(campaign::plan(
        Question::MatvecVsM4ri,
        &comparator,
        &plan_inputs(None, None, None, None)
    )
    .is_err());
    assert!(campaign::plan(
        Question::MatvecVsM4ri,
        &comparator,
        &plan_inputs(None, Some("/nonexistent/dense-m4ri-arm"), None, None)
    )
    .is_ok());

    let allocated = campaign::addendum(Question::AllocatedMatvec, ISSUE, FROZEN);
    assert!(campaign::plan(
        Question::AllocatedMatvec,
        &allocated,
        &plan_inputs(None, None, None, None)
    )
    .is_err());
}

#[test]
fn the_comparator_arm_is_the_external_build_and_the_gf2_arms_are_not() {
    let addendum = campaign::addendum(Question::MatvecVsM4ri, ISSUE, FROZEN);
    let plan = campaign::plan(
        Question::MatvecVsM4ri,
        &addendum,
        &plan_inputs(None, Some("/nonexistent/dense-m4ri-arm"), None, None),
    )
    .expect("the plan projects");
    let external = plan.arms.get(M4RI_ARM).expect("the comparator arm");
    assert_eq!(external.executable, "/nonexistent/dense-m4ri-arm");
    assert!(external.environment.is_empty());
    let peer = plan.arms.get(Route::M4riPeerGf2.id()).expect("the gf2 peer arm");
    assert_eq!(peer.executable, "/nonexistent/dense-arm");
}

#[test]
fn a_reseeded_campaign_addendum_projects_no_plan() {
    let question = Question::AllocatedMatvec;
    let mut addendum = campaign::addendum(question, ISSUE, FROZEN);
    addendum.cells[0].workload.seed ^= 1;
    let outcome = campaign::plan(
        question,
        &addendum,
        &plan_inputs(Some("/nonexistent/dense-arm-scalar"), None, None, None),
    );
    assert!(outcome.is_err());

    let mut renamed = campaign::addendum(question, ISSUE, FROZEN);
    renamed.cells[0].cell_id = "matvec-r1024-10w-warm".to_owned();
    assert!(campaign::plan(
        question,
        &renamed,
        &plan_inputs(Some("/nonexistent/dense-arm-scalar"), None, None, None)
    )
    .is_err());
}

#[test]
fn whole_consumer_cells_charge_conversion_costs() {
    for cell in cells() {
        let kernel = matches!(cell.workload, dense_parity_harness::Workload::AndPopcnt { .. });
        assert_eq!(cell.whole_consumer(), !kernel, "{}", cell.cell_id);
    }
    for question in Question::ALL {
        let addendum = campaign::addendum(question, ISSUE, FROZEN);
        for declaration in &addendum.cells {
            if declaration.metric_kind == MetricKind::WholeConsumer {
                assert!(declaration.conversion_costs_included, "{}", declaration.cell_id);
            }
        }
    }
}

#[test]
fn every_qualified_comparator_shape_is_a_frozen_cell() {
    let ids: BTreeSet<String> = family_cells(Question::MatvecVsM4ri)
        .iter()
        .map(|cell| cell.cell_id.clone())
        .collect();
    for shape in M4RI_SHAPES {
        assert!(ids.contains(&format!("m4ri-gap-{}-warm", shape.suffix)), "{}", shape.suffix);
    }
    let confirmatory: Vec<&str> = family_cells(Question::MatvecVsM4ri)
        .iter()
        .filter(|cell| cell.anchor)
        .map(|cell| cell.cell_id.as_str())
        .collect::<Vec<_>>()
        .iter()
        .map(|id| if *id == "m4ri-gap-65x512-warm" { "65x512" } else { "65x4096" })
        .collect();
    assert_eq!(confirmatory, vec!["65x512", "65x4096"]);
}

/// Repository-relative paths of two producing inputs a scratch closure pins:
/// one harness source and one campaign-runner source.
const HARNESS_SOURCE: &str = "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/lib.rs";
const RUNNER_SOURCE: &str = "dev/tools/tuning-campaign-support/src/campaign.rs";
const MANIFEST: &str = "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-producing-inputs.json";

/// An empty directory under this crate's own scratch space, replacing whatever
/// an earlier run of the same test left there.
fn scratch_tree(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch root");
    root
}

/// Builds a scratch repository whose producing-input closure pins a harness
/// source, a campaign-runner source and the manifest itself, all committed.
fn scratch_closure(name: &str) -> std::path::PathBuf {
    let root = scratch_tree(name);
    for path in [HARNESS_SOURCE, RUNNER_SOURCE, MANIFEST] {
        std::fs::create_dir_all(root.join(path).parent().expect("parent")).expect("parent");
    }
    std::fs::write(root.join(HARNESS_SOURCE), "harness\n").expect("harness source");
    std::fs::write(root.join(RUNNER_SOURCE), "runner\n").expect("runner source");
    let closure = serde_json::json!({
        "schema": "tuning-campaign-producing-inputs-v1",
        "behavior_sources": [HARNESS_SOURCE, RUNNER_SOURCE],
        "lifecycle_sources": [RUNNER_SOURCE],
        "build_inputs": [HARNESS_SOURCE, MANIFEST, RUNNER_SOURCE],
    });
    std::fs::write(
        root.join(MANIFEST),
        serde_json::to_string_pretty(&closure).expect("manifest"),
    )
    .expect("manifest");
    git(&root, &["init", "--quiet"]);
    git(&root, &["add", "--all"]);
    git(&root, &["commit", "--quiet", "-m", "scratch closure"]);
    root
}

fn git(root: &std::path::Path, arguments: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .env("GIT_AUTHOR_NAME", "harness")
        .env("GIT_AUTHOR_EMAIL", "harness@example.invalid")
        .env("GIT_COMMITTER_NAME", "harness")
        .env("GIT_COMMITTER_EMAIL", "harness@example.invalid")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git runs");
    assert!(status.success(), "git {arguments:?}");
}

#[test]
fn a_committed_and_clean_closure_admits_a_timed_run() {
    let root = scratch_closure("closure-clean");
    let checked = inputs::check(&root, MANIFEST, &[]).expect("the closure is clean");
    assert_eq!(checked, vec![HARNESS_SOURCE, MANIFEST, RUNNER_SOURCE]);
    std::fs::remove_dir_all(&root).expect("the scratch closure is removed");
}

#[test]
fn a_dirty_harness_source_refuses_a_timed_run() {
    let root = scratch_closure("closure-dirty-harness");
    std::fs::write(root.join(HARNESS_SOURCE), "harness edited after the build\n")
        .expect("dirty harness source");
    let refusal = inputs::check(&root, MANIFEST, &[]).expect_err("a dirty harness source refuses");
    assert!(refusal.contains(HARNESS_SOURCE), "{refusal}");
    assert!(!refusal.contains(RUNNER_SOURCE), "{refusal}");
    std::fs::remove_dir_all(&root).expect("the scratch closure is removed");
}

#[test]
fn a_dirty_runner_source_refuses_a_timed_run() {
    let root = scratch_closure("closure-dirty-runner");
    std::fs::write(root.join(RUNNER_SOURCE), "runner edited after the build\n")
        .expect("dirty runner source");
    let refusal = inputs::check(&root, MANIFEST, &[]).expect_err("a dirty runner source refuses");
    assert!(refusal.contains(RUNNER_SOURCE), "{refusal}");
    assert!(!refusal.contains(HARNESS_SOURCE), "{refusal}");
    std::fs::remove_dir_all(&root).expect("the scratch closure is removed");
}

/// The generator that both writes and freshness-checks the closure.
const CLOSURE_GENERATOR: &str = "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/make-dense-producing-inputs.py";

/// Regenerates the closure of `root` and compares it with `root`'s committed
/// manifest, writing nothing.
fn closure_freshness(root: &std::path::Path) -> std::process::Output {
    std::process::Command::new("python3")
        .arg("-B")
        .arg(CLOSURE_GENERATOR)
        .arg("--check")
        .current_dir(root)
        .output()
        .expect("the closure generator runs")
}

#[test]
fn a_source_added_to_a_measured_crate_fails_the_closure_freshness_check() {
    let root = repository_root();
    let inputs = tuning_campaign_support::provenance::ProducingInputs::read_at(&root, MANIFEST)
        .expect("the committed closure decodes");
    // Every path the generator enumerates is a build input, so copying the
    // build inputs alone yields a tree whose closure is the committed one.
    let scratch = scratch_tree("closure-freshness");
    for path in &inputs.build_inputs {
        let target = scratch.join(path);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("parent");
        std::fs::copy(root.join(path), &target).expect("copy a producing input");
    }
    let current = closure_freshness(&scratch);
    assert!(
        current.status.success(),
        "the copied tree does not reproduce its own closure: {}",
        String::from_utf8_lossy(&current.stderr)
    );

    let added = "crates/gf2-core/src/scratch_measured_source.rs";
    std::fs::write(scratch.join(added), "pub fn added() {}\n").expect("the added source");
    let stale = closure_freshness(&scratch);
    assert!(
        !stale.status.success(),
        "a source added to a measured crate passed the check"
    );
    let refusal = String::from_utf8_lossy(&stale.stderr);
    assert!(refusal.contains(added), "{refusal}");
    std::fs::remove_dir_all(&scratch).expect("the scratch tree is removed");
}

#[test]
fn the_porcelain_status_decodes_renames_and_untracked_entries() {
    let stdout = b"?? new.rs\0 M dev/tools/a.rs\0R  dev/tools/b.rs\0dev/tools/old.rs\0".to_vec();
    let status = inputs::parse_status(&stdout).expect("porcelain decodes");
    assert_eq!(status.get("new.rs").map(String::as_str), Some("??"));
    assert_eq!(status.get("dev/tools/a.rs").map(String::as_str), Some(" M"));
    // A rename dirties both the reported path and its recorded origin.
    assert_eq!(status.get("dev/tools/b.rs").map(String::as_str), Some("R "));
    assert_eq!(status.get("dev/tools/old.rs").map(String::as_str), Some("R "));
}

#[test]
fn an_untracked_closure_path_refuses_a_timed_run() {
    let root = scratch_closure("closure-untracked");
    let added = "dev/tools/tuning-campaign-support/src/uncommitted.rs";
    std::fs::write(root.join(added), "uncommitted\n").expect("untracked source");
    let mut closure: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(MANIFEST)).expect("manifest"))
            .expect("manifest decodes");
    closure["build_inputs"] = serde_json::json!([HARNESS_SOURCE, MANIFEST, RUNNER_SOURCE, added]);
    std::fs::write(
        root.join(MANIFEST),
        serde_json::to_string_pretty(&closure).expect("manifest"),
    )
    .expect("manifest");
    git(&root, &["add", MANIFEST]);
    git(&root, &["commit", "--quiet", "-m", "extend the closure"]);
    let refusal = inputs::check(&root, MANIFEST, &[]).expect_err("an untracked input refuses");
    assert!(refusal.contains(added), "{refusal}");
    std::fs::remove_dir_all(&root).expect("the scratch closure is removed");
}

#[test]
fn an_extra_campaign_input_joins_the_closure() {
    let root = scratch_closure("closure-extra");
    let ledger = "dev/bench_results/2037941f/dense-allocated-matvec-ledger.jsonl";
    std::fs::create_dir_all(root.join(ledger).parent().expect("parent")).expect("parent");
    std::fs::write(root.join(ledger), "").expect("ledger");
    let extra = vec![ledger.to_owned()];
    let refusal = inputs::check(&root, MANIFEST, &extra).expect_err("an untracked ledger refuses");
    assert!(refusal.contains(ledger), "{refusal}");
    git(&root, &["add", ledger]);
    git(&root, &["commit", "--quiet", "-m", "commit the ledger"]);
    let checked = inputs::check(&root, MANIFEST, &extra).expect("the extended closure is clean");
    assert!(checked.contains(&ledger.to_owned()));
    std::fs::remove_dir_all(&root).expect("the scratch closure is removed");
}

/// The repository root, five directories above this crate's manifest.
fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("the harness crate sits five directories below the repository root")
        .canonicalize()
        .expect("the repository root resolves")
}

/// Decodes `cargo metadata` for one manifest without touching its lock file.
fn cargo_metadata(root: &std::path::Path, manifest: &str, all_features: bool) -> serde_json::Value {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = std::process::Command::new(cargo);
    command
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .arg("--manifest-path")
        .arg(manifest);
    if all_features {
        command.arg("--all-features");
    }
    let output = command.output().expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata {manifest}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata decodes")
}

/// Package ids of one resolved graph, or of the subgraph rooted at `from`.
fn resolved_ids(metadata: &serde_json::Value, from: Option<&str>) -> BTreeSet<String> {
    let packages = metadata["packages"].as_array().expect("packages");
    let identifier =
        |package: &serde_json::Value| package["id"].as_str().expect("a package id").to_owned();
    let Some(name) = from else {
        return packages.iter().map(identifier).collect();
    };
    let start = packages
        .iter()
        .find(|package| package["name"] == name)
        .map(identifier)
        .unwrap_or_else(|| panic!("{name} is not a package of this workspace"));
    let edges: std::collections::BTreeMap<&str, Vec<&str>> = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolve nodes")
        .iter()
        .map(|node| {
            let deps = node["deps"]
                .as_array()
                .expect("node deps")
                .iter()
                .map(|dep| dep["pkg"].as_str().expect("a dep package id"))
                .collect();
            (node["id"].as_str().expect("a node id"), deps)
        })
        .collect();
    let mut reached = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(id) = pending.pop() {
        if !reached.insert(id.clone()) {
            continue;
        }
        for dependency in edges.get(id.as_str()).into_iter().flatten() {
            pending.push((*dependency).to_owned());
        }
    }
    reached
}

/// Repository-relative manifests and lock files one resolved graph builds from.
///
/// Every reached package inside the repository contributes its own
/// `Cargo.toml`, and the resolved workspace contributes the root manifest that
/// its members inherit from together with the lock that pins the resolution.
fn build_manifests(
    root: &std::path::Path,
    metadata: &serde_json::Value,
    from: Option<&str>,
) -> BTreeSet<String> {
    let reached = resolved_ids(metadata, from);
    let relative = |path: &str| {
        std::path::Path::new(path)
            .strip_prefix(root)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    };
    let mut manifests: BTreeSet<String> = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter(|package| reached.contains(package["id"].as_str().expect("a package id")))
        .filter_map(|package| relative(package["manifest_path"].as_str().expect("a manifest")))
        .collect();
    let workspace = relative(metadata["workspace_root"].as_str().expect("a workspace root"))
        .expect("the workspace root is inside the repository");
    for file in ["Cargo.toml", "Cargo.lock"] {
        manifests.insert(
            std::path::Path::new(&workspace)
                .join(file)
                .to_string_lossy()
                .into_owned(),
        );
    }
    manifests
}

#[test]
fn the_closure_names_every_manifest_a_timed_executable_builds_from() {
    let root = repository_root();
    let own = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the harness crate resolves")
        .strip_prefix(&root)
        .expect("the harness crate is inside the repository")
        .join("Cargo.toml")
        .to_string_lossy()
        .into_owned();

    // The arms are built from the harness workspace with every feature, so
    // that graph carries the M4RI arm too; the runner and the acceptance
    // binary are built from the root workspace as `-p tuning-campaign-support`.
    let arms = cargo_metadata(&root, &own, true);
    let support = "dev/tools/tuning-campaign-support/Cargo.toml";
    let runner = cargo_metadata(&root, support, false);
    let mut expected = build_manifests(&root, &arms, None);
    expected.extend(build_manifests(&root, &runner, Some("tuning-campaign-support")));
    assert!(
        expected.contains(&own) && expected.contains(support),
        "the derivation reached neither executable's manifest: {expected:?}"
    );

    let inputs = tuning_campaign_support::provenance::ProducingInputs::read_at(&root, MANIFEST)
        .expect("the committed closure decodes");
    let declared: BTreeSet<String> = inputs.build_inputs.iter().cloned().collect();
    let omitted: Vec<&String> = expected.difference(&declared).collect();
    assert!(
        omitted.is_empty(),
        "the producing-input closure omits the build manifests {omitted:?}"
    );
    // The window guard reads the closure manifest to decide what to check, so
    // the manifest is itself a provenance-controlling build input.
    assert!(declared.contains(MANIFEST), "the producing-input closure omits itself");
}
