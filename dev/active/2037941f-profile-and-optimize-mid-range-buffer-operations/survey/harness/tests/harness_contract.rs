//! Behavioral contract of the logical-buffer measurement harness.
//!
//! The survey harness is its own workspace, so the repository CI contract
//! never reaches it: these tests and the launcher's `build` subcommand are the
//! only thing that runs it.

use logical_buffer_harness::campaign;
use logical_buffer_harness::cells::{
    cells, family_cells, Cache, Layout, NrTarget, Question, RowShape, Workload, ALL_WORDS,
    ANCHOR_WORDS, BOUNDARY_BITS, CAMPAIGN_SEED, NR_TARGETS, STREAMING_BANKS, STREAMING_BANK_BYTES,
};
use logical_buffer_harness::fixture::{RowBanks, XorBanks, SLAB_ALIGN};
use logical_buffer_harness::inputs;
use logical_buffer_harness::oracle;
use logical_buffer_harness::routes::{nr_construct, run_windows, verify_nr, Route, WindowPlan};
use logical_buffer_harness::wire::{Case, Request};
use std::collections::BTreeSet;
use tuning_campaign_support::abtest::SplitMix64;
use tuning_campaign_support::protocol::{CellRole, FamilyAddendum};
use tuning_campaign_support::transport;

#[test]
fn the_cell_table_reproduces_the_frozen_matrices() {
    let table = cells();
    let counts: Vec<usize> = Question::ALL
        .iter()
        .map(|question| family_cells(*question).len())
        .collect();
    // 5 primary + 2 neighbours + 5 offset + 5 streaming; the NR question has
    // its five selected routes plus one cold repeat of the primary.
    assert_eq!(counts, vec![17, 17, 6, 12]);
    assert_eq!(table.len(), counts.iter().sum::<usize>());

    let ids: Vec<&str> = table.iter().map(|cell| cell.cell_id.as_str()).collect();
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    assert_eq!(ids[0], "xor-8w-a64-warm");
    assert_eq!(ids[5], "xor-7w-a64-warm");
    assert_eq!(ids[6], "xor-66w-a64-warm");
    assert_eq!(ids[7], "xor-8w-o8-warm");
    assert_eq!(ids[12], "xor-8w-a64-streaming");
    assert_eq!(ids[17], "row-xor-8w-full-warm");
    assert_eq!(ids[34], "nr-construct-bg2-256-49-z9-8w-warm");
    assert_eq!(ids[39], "nr-construct-bg2-256-49-z9-8w-cold");
    assert_eq!(ids[40], "isal-base-gap-8w-a64-warm");

    // The anchors of every question are its confirmatory family; the NR
    // question's confirmatory family is the whole selected-route table.
    for question in Question::ALL {
        let anchors = family_cells(question)
            .iter()
            .filter(|cell| cell.anchor)
            .count();
        assert_eq!(anchors, 5, "{}", question.family_id());
    }
    assert!(table
        .iter()
        .all(|cell| !cell.anchor || cell.cache == Cache::Warm));
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
        let addendum = campaign::addendum(question, "bb769456", "2026-09-15T03:56:39Z");
        addendum
            .validate()
            .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
        assert_eq!(addendum.family.id, question.family_id());
        assert_eq!(
            addendum.family_wise.ledger_path.as_deref(),
            Some(question.ledger_path())
        );
        // A pilot transcription resolves no measurement resolution, so no cell
        // of it can be confirmatory.
        assert!(addendum.effect.measurement_resolution.is_none());
        assert!(addendum.effect.resolution_evidence.is_none());
        assert!(addendum
            .cells
            .iter()
            .all(|cell| cell.role == CellRole::Exploratory));
        assert_eq!(addendum.cells.len(), family_cells(question).len());

        let encoded = serde_json::to_vec(&addendum).expect("encodes");
        assert_eq!(FamilyAddendum::decode(&encoded).expect("decodes"), addendum);
    }
}

#[test]
fn a_changed_margin_or_cell_no_longer_matches_the_transcription() {
    let question = Question::IsolatedXor;
    let derived = campaign::addendum(question, "bb769456", "2026-09-15T03:56:39Z");

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
        r#"{"schema":"zen3-benchmark-arm-request-v1","cell_id":"xor-8w-a64-warm","#,
        r#""arm":"public-xor-a","role":"baseline","pair":0,"#,
        r#""case":{"layout":"a64","question":"isolated-xor","seed":7,"words":8},"#,
        r#""cache_state":"warm","windows":5,"window_target_ms":100,"cpus":[0],"#,
        r#""workers_declared":1}"#
    );
    let decoded: Request = transport::decode_case(warm).expect("the mirror decodes");
    assert_eq!(decoded.cell_id, "xor-8w-a64-warm");
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
        r#"{"schema":"zen3-benchmark-arm-request-v1","#,
        r#""cell_id":"nr-construct-bg2-256-49-z9-8w-cold","arm":"nr-construct-a","#,
        r#""role":"baseline","pair":0,"case":{"base_graph":2,"#,
        r#""question":"nr-bg2-construction","seed":7,"target_k":49,"target_n":256},"#,
        r#""cache_state":"cold","cold_calls":1,"windows":5,"window_target_ms":100,"#,
        r#""cpus":[0],"workers_declared":1}"#
    );
    let decoded: Request = transport::decode_case(cold).expect("the mirror decodes");
    assert_eq!(decoded.cold_calls, Some(1));
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
    let unfrozen_route = Case::NrBg2Construction {
        base_graph: 2,
        target_n: 512,
        target_k: 100,
        seed: 1,
    };
    assert!(unfrozen_route.workload().is_err());

    let unfrozen_layout = Case::IsolatedXor {
        words: 8,
        layout: "a32".to_owned(),
        seed: 1,
    };
    assert!(unfrozen_layout.workload().is_err());

    // ISA-L cells use only the aligned layout; an unaligned view is outside
    // the qualified interface.
    let unaligned = Case::IsalBaseGap {
        words: 8,
        layout: "o8".to_owned(),
        seed: 1,
    };
    assert!(unaligned.workload().is_err());
}

#[test]
fn fixture_views_sit_at_the_layout_the_cell_declares() {
    for words in ALL_WORDS {
        for layout in [Layout::A64, Layout::O8] {
            let banks = XorBanks::build(words, layout, Cache::Warm, 1);
            let (source, destination) = banks.addresses_mod_64(0, 0);
            assert_eq!(source, layout.offset_bytes() % SLAB_ALIGN);
            assert_eq!(destination, layout.offset_bytes() % SLAB_ALIGN);
            assert_eq!(banks.bases_mod_64(0), (0, 0));
        }
    }
}

#[test]
fn a_streaming_working_set_fills_every_declared_bank() {
    let banks = XorBanks::build(8, Layout::A64, Cache::Streaming, 1);
    assert!(banks.items() > 1);
    assert!(banks.working_set_bytes() >= STREAMING_BANKS * STREAMING_BANK_BYTES);
    let matrices = RowBanks::build(8, RowShape::Full, Cache::Streaming, 1);
    assert!(matrices.working_set_bytes() >= STREAMING_BANKS * STREAMING_BANK_BYTES);
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
    // Every cache policy the addendum declares reaches the arrangement pass,
    // and none of them enters the timing protocol.
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
fn the_oracle_covers_every_frozen_boundary_and_passes() {
    let report = oracle::run().expect("the oracle passes");
    let names: BTreeSet<&str> = report.iter().map(|case| case.name.as_str()).collect();
    for bits in BOUNDARY_BITS {
        assert!(
            names.contains(format!("xor-bits-{bits}").as_str()),
            "{bits}"
        );
    }
    for words in ALL_WORDS {
        for layout in [Layout::A64, Layout::O8] {
            assert!(names.contains(format!("xor-{words}w-{}", layout.id()).as_str()));
        }
        for shape in [RowShape::Full, RowShape::Tail63] {
            assert!(names.contains(format!("row-xor-{words}w-{}", shape.id()).as_str()));
        }
    }
    for target in NR_TARGETS {
        assert!(names.contains(format!("nr-construct-{}", target.suffix).as_str()));
    }
    assert!(report.iter().all(|case| case.checks > 0));
}

#[test]
fn a_returned_code_is_checked_against_its_frozen_declaration() {
    let target = NR_TARGETS[0];
    let code = nr_construct(&target);
    let facts = verify_nr(&code, &target).expect("the frozen route holds");
    assert_eq!(facts.lifting_factor, target.lifting_factor);
    assert_eq!(facts.stride_words, target.stride_words);

    let wrong = NrTarget {
        lifting_factor: target.lifting_factor + 1,
        ..target
    };
    assert!(
        verify_nr(&code, &wrong).is_err(),
        "a mismatch makes the cell unavailable rather than substituting a target"
    );
}

#[test]
fn every_anchor_word_count_is_a_frozen_cell_of_every_buffer_question() {
    for question in [
        Question::IsolatedXor,
        Question::PublicRowXor,
        Question::IsalBaseGap,
    ] {
        let anchors: BTreeSet<usize> = family_cells(question)
            .iter()
            .filter(|cell| cell.anchor)
            .filter_map(|cell| cell.workload.words())
            .collect();
        assert_eq!(anchors, ANCHOR_WORDS.into_iter().collect::<BTreeSet<_>>());
    }
}

#[test]
fn the_projected_plan_covers_every_declared_cell_with_declared_builds() {
    for question in Question::ALL {
        let addendum = campaign::addendum(question, "bb769456", "2026-09-15T03:56:39Z");
        let plan = campaign::plan(
            question,
            &addendum,
            "bb769456-contract-plan",
            1,
            tuning_campaign_support::protocol::ReceiptLabel::Pilot,
            "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaign.json",
            "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/logical-producing-inputs.json",
            "/tmp/gf2-contract.lock",
            "/nonexistent/logical-arm",
            Some("/nonexistent/logical-isal-arm"),
            Route::PublicXorB,
            Some(2),
            Some(6),
        )
        .expect("the plan projects");
        plan.validate(&addendum)
            .unwrap_or_else(|errors| panic!("{}: {}", question.family_id(), errors.join("; ")));
        assert_eq!(plan.cells.len(), addendum.cells.len());
        assert_eq!(plan.arms.len(), 2);
    }
}

#[test]
fn the_isal_family_needs_its_external_comparator_executable() {
    let question = Question::IsalBaseGap;
    let addendum = campaign::addendum(question, "bb769456", "2026-09-15T03:56:39Z");
    let outcome = campaign::plan(
        question,
        &addendum,
        "bb769456-contract-plan",
        1,
        tuning_campaign_support::protocol::ReceiptLabel::Pilot,
        "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaign.json",
        "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/logical-producing-inputs.json",
        "/tmp/gf2-contract.lock",
        "/nonexistent/logical-arm",
        None,
        Route::PublicXorB,
        None,
        None,
    );
    assert!(outcome.is_err());
}

#[test]
fn a_route_only_serves_its_own_question() {
    assert_eq!(Route::PublicXorA.question(), Question::IsolatedXor);
    assert_eq!(Route::ResolvedXor.question(), Question::IsolatedXor);
    assert_eq!(Route::RowXorB.question(), Question::PublicRowXor);
    assert_eq!(Route::NrConstructA.question(), Question::NrConstruction);
    assert_eq!(Route::IsalPeerGf2.question(), Question::IsalBaseGap);
    assert!(Route::parse("public-xor-c").is_err());
}

#[test]
fn whole_consumer_cells_charge_conversion_costs() {
    for cell in cells() {
        let whole = matches!(cell.workload, Workload::Nr(_));
        assert_eq!(cell.whole_consumer(), whole);
    }
    for question in Question::ALL {
        let addendum = campaign::addendum(question, "bb769456", "2026-09-15T03:56:39Z");
        for declaration in &addendum.cells {
            if declaration.metric_kind
                == tuning_campaign_support::protocol::MetricKind::WholeConsumer
            {
                assert!(
                    declaration.conversion_costs_included,
                    "{}",
                    declaration.cell_id
                );
            }
        }
    }
}

/// Repository-relative paths of two producing inputs a scratch closure pins:
/// one harness source and one campaign-runner source.
const HARNESS_SOURCE: &str = "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/harness/src/lib.rs";
const RUNNER_SOURCE: &str = "dev/tools/tuning-campaign-support/src/campaign.rs";

/// Builds a scratch repository whose producing-input closure pins a harness
/// source, a campaign-runner source and the manifest itself, all committed.
fn scratch_closure(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch root");
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

const MANIFEST: &str = "dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey/logical-producing-inputs.json";

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
}

#[test]
fn a_dirty_harness_source_refuses_a_timed_run() {
    let root = scratch_closure("closure-dirty-harness");
    std::fs::write(
        root.join(HARNESS_SOURCE),
        "harness edited after the build\n",
    )
    .expect("dirty harness source");
    let refusal = inputs::check(&root, MANIFEST, &[]).expect_err("a dirty harness source refuses");
    assert!(refusal.contains(HARNESS_SOURCE), "{refusal}");
    assert!(!refusal.contains(RUNNER_SOURCE), "{refusal}");
}

#[test]
fn a_dirty_runner_source_refuses_a_timed_run() {
    let root = scratch_closure("closure-dirty-runner");
    std::fs::write(root.join(RUNNER_SOURCE), "runner edited after the build\n")
        .expect("dirty runner source");
    let refusal = inputs::check(&root, MANIFEST, &[]).expect_err("a dirty runner source refuses");
    assert!(refusal.contains(RUNNER_SOURCE), "{refusal}");
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
}

#[test]
fn an_extra_campaign_input_joins_the_closure() {
    let root = scratch_closure("closure-extra");
    let ledger = "dev/bench_results/2037941f/logical-isolated-xor-ledger.jsonl";
    std::fs::create_dir_all(root.join(ledger).parent().expect("parent")).expect("parent");
    std::fs::write(root.join(ledger), "").expect("ledger");
    let extra = vec![ledger.to_owned()];
    let refusal = inputs::check(&root, MANIFEST, &extra).expect_err("an untracked ledger refuses");
    assert!(refusal.contains(ledger), "{refusal}");
    git(&root, &["add", ledger]);
    git(&root, &["commit", "--quiet", "-m", "commit the ledger"]);
    let checked = inputs::check(&root, MANIFEST, &extra).expect("the extended closure is clean");
    assert!(checked.contains(&ledger.to_owned()));
}

#[test]
fn the_porcelain_status_decodes_renames_and_untracked_entries() {
    let stdout = b"?? new.rs\0 M dev/tools/a.rs\0R  dev/tools/b.rs\0dev/tools/old.rs\0".to_vec();
    let status = inputs::parse_status(&stdout).expect("porcelain decodes");
    assert_eq!(status.get("new.rs").map(String::as_str), Some("??"));
    assert_eq!(status.get("dev/tools/a.rs").map(String::as_str), Some(" M"));
    // A rename dirties both the reported path and its recorded origin.
    assert_eq!(status.get("dev/tools/b.rs").map(String::as_str), Some("R "));
    assert_eq!(
        status.get("dev/tools/old.rs").map(String::as_str),
        Some("R ")
    );
}
