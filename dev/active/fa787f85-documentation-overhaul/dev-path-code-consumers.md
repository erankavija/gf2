# Code consumers of `dev/`, `docs/` and `crates/*/docs` paths

Status: partial (wound down on coordinator request). Scanned: `crates/`,
`dev/tools/`, `dev/scripts/`, `scripts/`, `contrib/`, `.github/`, `tests/`,
`Cargo.toml`, `.jit/*.toml`, `packages/`, `.gitignore`, `.claude/`,
`dev/benchmarks/**` tooling, `dev/research/**`, `dev/studies/**` scripts,
`docs/presentations/**`, and `dev/active/**` scripts (aggregated only).
Comment-only lines (`//`, `#`, `;`, docstrings) are excluded. Not scanned:
`proofs/` Lean sources beyond a grep (comment mentions only), `dev/sessions`,
`dev/plans`, `.codex/`, `.config/`, `benches/`, `benchmarks/` beyond a grep
(no code hits), and the inside of `dev/active/*/survey/*` Rust crates line by
line (only aggregated target counts).

Mode legend:

- `read`: file read/written/copied at that path; updating the literal suffices.
- `identity`: the path string is compared against, or digested into, committed
  data (receipts, profiles, fixtures, closures); a move requires migrating or
  re-pinning that data as well.
- `include`: compile-time `include_str!`/`include_bytes!` relative to the
  source file; compile fails on move.
- `derived`: path built from an issue or campaign id at runtime
  (`format!("dev/active/{issue}/...")`); the directory layout, not one file, is
  pinned.
- `CI/config`: workflow step, gate command, workspace member, or config key.
- `text`: path appears only in an emitted message or help text.

## dev/active entries that are code-pinned

Known already: `f547c394`, `02b8137c`. Newly found:

| Entry | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md` | `dev/tools/tuning-campaign-support/src/protocol.rs:29` | `pub const CONTRACT_PATH` | identity |
| same | `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:147` | `ArtifactPin::capture(root, stage, CONTRACT_PATH, ...)` | read (live, digested into every receipt) |
| same | `dev/tools/tuning-campaign-support/src/receipt.rs:1030` | P-02 rule: receipt `contract.path` must equal `CONTRACT_PATH` | identity (applies to every committed receipt under `dev/bench_results/`) |
| same | `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:342,610` | `pin(repo, CONTRACT_PATH)` / `ArtifactPin::capture` | read |
| `dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv` | `dev/scripts/ccx1-bench-flock.sh:82` | error message | text |
| `dev/active/a83583e0/campaign-declaration.json` | `dev/tools/tuning-campaign-support/tests/driver_launcher.rs:23-29,75,241` | `format!("dev/active/{issue}/campaign-declaration.json")`, `fs::copy` from repo | read |
| same | `dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs:273,276-283,3142` | `relative_path(issue)` + `for_campaign(.., "gf2-a83583e0-...")` | derived read |
| same | `dev/scripts/validate-tuning-extent-campaign.py:427,4127` | `declaration_path(issue)` must be listed in the producing manifest `lifecycle_sources` | identity (vs `dev/active/a83583e0/producing-build-inputs.json`) |
| same | `dev/scripts/tuning-extent-campaign.sh:30` | `declaration=dev/active/$issue/campaign-declaration.json` | derived read |
| `dev/active/dbd8787d/campaign-declaration.json` | `dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs:3147` | `for_campaign(.., "gf2-dbd8787d-...")` | derived read |
| `dev/active/26465e6c/superseded/v1/addendum-popcount.json` | `dev/tools/tuning-campaign-support/tests/addendum_schema_versions.rs:40` | `VERSIONED_FIXTURES` literal, `fs::read(root.join(..))` | read |
| `dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-confirmation.json` | `dev/tools/tuning-campaign-support/tests/addendum_schema_versions.rs:48` | same | read |
| `dev/active/5cbb6545/addendum-popcount-v4-confirmation.json` | `dev/tools/tuning-campaign-support/tests/addendum_schema_versions.rs:52` | same | read |
| `dev/active/c077a88b/findings.md`, `dev/active/a203a23c/receipt-reevaluation.json` | `dev/scripts/receipt-input-omissions.json:12-13,22-23,...` | `documents` field | text (checker uses only `receipt`/`path` tuples) |
| `dev/active/c077a88b/survey/harness/Cargo.lock` (snapshot copy) | `dev/scripts/receipt-input-omissions.json:8,18,28,38,48` | `path` under `dev/bench_results/c077a88b/.../inputs/producing/dev/active/c077a88b/...` | identity (tuple must stay "missing"; the path is inside `dev/bench_results`, so a `dev/active` move does not affect it) |
| `dev/active/d1b4f85e/smoke-run.md`, `dev/active/591a1c5e/smoke-run.md` | `dev/active/fd9d5416/tests/test_render_receipt.py:63-64` | `REPO / "dev/active/..."` | read |
| `dev/active/1d0da41f`, `dev/active/53c5a8c0` | `dev/active/{1d0da41f,53c5a8c0}/survey/summarize.py:25-26,31-32` | `os.path.join(ROOT, "dev", "active", id)` | read (self) |
| `dev/active/<epic>/survey/*` harness crates (12fdeb5b, eda07788 x3, 2037941f x2, c7113c5a, 6fb89a3c, 04b85d10, 1d4fd63d x2, 53c5a8c0, 1c602857) | each `Cargo.toml` | `tuning-campaign-support = { path = "../../../../../dev/tools/tuning-campaign-support" }` | relative dep; breaks if the crate's depth changes |
| `dev/active/1d4fd63d/survey/annotate-asm.py` | `crates/gf2-kernels-simd/src/x86/asm/transpose.asm.txt:1406,1415,1423` | `;` comment | excluded (comment) |

Receipts under `dev/bench_results/**/receipt.json` pin `dev/active/<issue>/...`
path strings with digests for 25 issues (3be770d5, c077a88b, 2037941f-..., 6fb89a3c,
6c6b09b1, f547c394, 26465e6c, 5cbb6545, 12fdeb5b, 07ca8585, 53c5a8c0, 19513245,
1a379447-..., c7113c5a, eda07788, 04b85d10, f63a2464, 1d4fd63d, 1d0da41f,
c04dd4ac-..., ad2a6a58, 00dd43c3, 1c602857, 4c1e441f, bc091474). These are
data, not code: `check-receipt-input-snapshots.py` verifies the snapshot copies
under each receipt directory, so a `dev/active` move leaves CI green except
where a path is also checked by a constant (P-02: protocol, contract,
addendum schema).

Scripts living inside `dev/active/<epic>/` (aggregate, from 951 code lines):
nearly every zen3 campaign epic (00dd43c3, 04b85d10, 07ca8585, 12fdeb5b,
19513245, 1c602857, 1d4fd63d, 2037941f-..., 26465e6c, 3be770d5, 4c1e441f,
4e732b56, 53c5a8c0, 5cbb6545, 6c6b09b1, 6fb89a3c, a203a23c, ad2a6a58,
b8206228-..., bc091474, bdc507a3, c04dd4ac-..., c077a88b, c7113c5a, eda07788,
f63a2464, fd9d5416, 34d85cb9, 7d7c647c, ae03bcd0-general-bch, e6ea0dde, f8dd4dde)
has `.py`/`.sh`/`.rs` files with absolute repo-relative literals naming its own
directory, `dev/bench_results/<id>`, `dev/scripts/`, `dev/tools/tuning-campaign-support`,
and sibling epics (f547c394, 1a379447-..., c7113c5a, 3be770d5, c077a88b,
6c6b09b1, eda07788, c04dd4ac-..., 00dd43c3, 4e732b56, 2037941f-..., bc091474,
d1b4f85e, 591a1c5e). `b8206228-permanent-statistics/generate_manifest_draft.py`
names `dev/studies/{047b62ed,91605d4d,6c7fcb38,b488f02c}` and
`dev/benchmarks/permanent_campaign/*`.

## dev/active/f547c394 (known)

| Consumer | Construct | Mode |
| --- | --- | --- |
| `dev/tools/tuning-campaign-support/src/protocol.rs:25,27` | `PROTOCOL_PATH`, `ADDENDUM_SCHEMA_PATH` consts | identity (P-02 in `receipt.rs:1029`) |
| `.../src/protocol.rs:1102` | `producing-inputs.json` literal | read |
| `.../src/bin/benchmark-ab-runner.rs:145-150` | `ArtifactPin::capture` | read |
| `.../tests/protocol_contracts.rs:62-63,85,186,478,621,696,1082-1092,2396,2600,3499,4185,4335,4378` | literals, `fs::read` | read |
| `.../tests/addendum_schema_versions.rs:35-51` | schema file literals | read |
| `dev/scripts/check-campaign-producing-closure.py:33` | `CLOSURE` const | read (CI) |
| `dev/scripts/check-addendum-schema-versions.py:53,56` | `SCHEMA_DIR`, `VERSIONS_TABLE` | read (CI) |
| `dev/active/f547c394/producing-inputs.json` | lists `dev/bench_results/f547c394/run-smoke-v3.sh`, `dev/scripts/ccx1-bench-flock.sh`, `dev/tools/tuning-campaign-support/**` | identity (closure checked by CI; digests in receipts) |

## dev/active/02b8137c (known)

| Consumer | Construct | Mode |
| --- | --- | --- |
| `crates/gf2-sim/src/permanent_campaign/validation.rs:68,71,74` | `FROZEN_*` consts | read |
| `crates/gf2-sim/src/permanent_campaign/validation.rs:1363` | `preregistration_identity.path != "dev/active/02b8137c/pre-draw-validation-v1-preregistration.json"` | identity |
| `crates/gf2-sim/tests/permanent_validation.rs:39-40,653,680,1636-1672` | literals, `fs::read`, `read_dir` | read |

## dev/active/3f664839

Content-identified: `validate_design`
(`crates/gf2-sim/src/permanent_rare_event/artifact.rs:4246`) checks revision,
blob and content SHA-256; fixtures carry the path as data.

## dev/archive

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/archive/b7157be6-osd/active/plan.md` | `crates/gf2-coding/tests/ebch_128_64_reference.rs:131`; `crates/gf2-coding/tests/data/ebch_128_64_reference.json:22` | `assert_eq!` on fixture field | identity |
| `dev/archive` | `.jit/config.toml:162` | `archive_root` | config |

## dev/studies

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/studies/b488f02c/literature-search-2026-08-08.md` | `crates/gf2-sim/src/bin/permanent_campaign.rs:57-59` | `Q5_Q7_SEARCH_PATH` + `Q5_Q7_SEARCH_SHA256` | identity |
| same | `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:178,188,214` | `include_bytes!("../../../../dev/studies/...")` + path literal | include + identity |
| same | `crates/gf2-sim/tests/permanent_campaign_bin.rs:13,210-212` | `INTERPRETATION_SOURCES`, copied from repo | read |
| `dev/studies/{047b62ed,91605d4d,6c7fcb38,b488f02c}` | `dev/active/b8206228-permanent-statistics/generate_manifest_draft.py:45-47,98-100`; `dev/studies/*/analysis.py` | literals | read |
| `dev/studies` | `.jit/config.toml:163,165` | `managed_paths`, `issue_scoped_areas` | config |

## dev/simulation_results

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/simulation_results/permanent-zero-fraction` | `crates/gf2-sim/src/permanent_campaign/schema.rs:45` | `pub const DATASET_HOME`; `provenance.rs:100-129,408` classify paths relative to it | identity |
| `.../permanent-zero-fraction-20260829`, `manifest.json`, `protocol.md` | `crates/gf2-sim/src/permanent_campaign/validation.rs:66,76,78` | consts | read |
| same | `crates/gf2-sim/src/permanent_rare_event/artifact.rs:84-90` | consts with `MANIFEST_SHA256`, `PROTOCOL_SHA256` | identity |
| `.../scheinerman2024-q3-targets-v1.csv` | `crates/gf2-sim/src/bin/permanent_campaign.rs:54-56` | const + sha256 | identity |
| same | `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:173-181,205,1511` | `include_bytes!` | include |
| same | `crates/gf2-sim/tests/permanent_campaign_bin.rs:12,184,206,537` | literals | read |
| same | `scripts/permanent_zero_fraction_analysis.py:506` | `_source_table_path()` | read |
| `.../permanent-zero-fraction/fixtures` | `tests/test_permanent_zero_fraction_analysis.py:20` | `FIXTURES` | read |
| `.../permanent-zero-fraction/...` | `crates/gf2-sim/tests/permanent_validation.rs:231,1105-1109,1551`; `crates/gf2-sim/tests/permanent_rare_event_artifacts.rs:1385,2986`; `coordinator_tests.rs:161,181,247,1173,1668` | literals | read |
| `dev/simulation_results/permanent-rare-event` | `crates/gf2-sim/tests/permanent_rare_event_artifacts.rs:79-92,160,200,722,2733-2775,2858,2942` | fixture literals | identity (artifact_root recorded in receipts) |
| `dev/simulation_results/osd-ebch-128-64/schema1/ebch_osd_awgn.json` | `crates/gf2-sim/tests/osd_campaign_protocol.rs:807` | `join` + `fs::read` | read |
| `dev/simulation_results/{fig1,fig3}_*.csv/json`, `phase1_final/*`, `phase1_comparison_report.md` | `crates/gf2-coding/tests/grand_phase1_smoke.rs:417-434,473-476` | `is_file()`/`exists()` asserts, reads | read (asserts on presence and absence) |
| `dev/simulation_results/fig7_*` | `dev/scripts/gen_fig7_report.sh:5-9` | vars | read/write |
| `dev/simulation_results[/sub]` | `dev/campaigns/*.toml` (`output_dir`) | config | write |
| `dev/simulation_results` | `docs/presentations/figures/generate_grand_comparison_plots.py:31` | `REPO / "dev" / "simulation_results"` | read |
| `dev/simulation_results/permanent-zero-fraction/` | `dev/benchmarks/permanent_campaign/accelerator_launch_costs_v1.py:22`, `determinant_cost_v5.py:45` | consts | read |
| `dev/simulation_results/*.progress.jsonl` | `.gitignore:91` | ignore pattern | config |

## dev/benchmarks

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/benchmarks/tuning_profiles` | `dev/tools/.../tuning-extent-campaign-driver.rs:1041,2754,4674`; `dev/scripts/validate-tuning-extent-campaign.py:99,3389,4095,4199-4203` | `TUNING_EVIDENCE` const; receipt destination equality | identity |
| `dev/benchmarks/tuning_profiles/gf2-a83583e0-20260930t230000z-2728298.md` | `crates/gf2-core/tests/tuning_profile_committed.rs:107`; `crates/gf2-core/tests/support/measured_format2.rs:10`; `crates/gf2-algebra/tests/tuning_repository_envelopes.rs:21`; profile JSON `crates/gf2-{core,algebra}/data/tuning-profiles/*.json`, `dev/reference_data/tuning-profiles/*.json` (`receipt` field) | `assert_eq!` vs profile field | identity |
| `dev/benchmarks/tuning_profiles/*` (fixtures) | `crates/gf2-core/tests/tuning_envelope_v2.rs:201`; `crates/gf2-core/benches/tuning_calibration.rs:5609,5822,5844,11323`; `crates/gf2-algebra/benches/tuning_calibration.rs:1966`; `crates/gf2-core/benches/selector_non_regression.rs:2574-2575` | `RepoRelPath::parse` literals | read (fixture strings) |
| `dev/benchmarks/permanent_campaign/{exact-anchors,backend-selection-v1-equivalence}.csv` | `crates/gf2-sim/src/permanent_campaign/validation.rs:79,81`; `tests/permanent_validation.rs:252` | consts | read |
| `dev/benchmarks/permanent_campaign/accelerator-launch-costs-v1.csv` | `crates/gf2-sim/tests/permanent_campaign_bin.rs:539` | literal | read |
| `dev/benchmarks/permanent_campaign/coordinator-costs.csv` | `crates/gf2-sim/src/permanent_campaign/coordinator_tests.rs:945` | literal | read |
| `dev/benchmarks/permanent_campaign/batched-f3-avx2.csv` | `crates/gf2-algebra/benches/batched_f3_permanent.rs:457` | CLI default | write |
| `dev/benchmarks/permanent_campaign/{backend-selection-v1.md,probe-costs-de5f7414.csv}` | `dev/research/permanent-sampling-feas/src/campaign_selection.rs:27,30-38` | `include_str!("../../../benchmarks/...")` + `PROBE_COSTS_PATH` + sha256 | include + identity |
| `dev/benchmarks/permanent_campaign/*` | `dev/benchmarks/permanent_campaign/{accelerator_launch_costs_v1,determinant_cost_v5,summarize_premeasure_v1}.py` and tests | consts incl. self path digests | identity |
| `dev/benchmarks/gf2_algebra_permanent` | `crates/gf2-algebra/benches/s1_n36_speedup.rs:275`; `examples/{parallel_chunk_sweep.rs:86,paper_repro_slope.rs:92,parallel_scaling_sweep.rs:107}`; `scripts/permanent-repro.sh:106`; `scripts/plot_permanent_benchmarks.py:520,526`; `dev/research/permanent_gpu_{crossover,speedup}` | literals, CLI defaults | write/read |
| `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv` | `crates/gf2-algebra/examples/permanent_demo.rs:233,245` | message text | text |
| `dev/benchmarks/gf2_algebra_permanent/README.md` | `docs/presentations/ae82bd73-gf2-algebra-permanent/talk.html:124,162` | `<a href="../../../dev/benchmarks/...">` | relative link |
| `dev/benchmarks/perm_uniformity` | `scripts/perm-uniformity-repro.sh:43`; `scripts/perm-uniformity-gpu-repro.sh:59`; `dev/research/perm_uniformity_gpu/src/main.rs:812` (`SSOT_DIR`) | consts | write |
| `dev/benchmarks/gf2-sim/diagnostic-dumps` | `crates/gf2-sim/src/executor/failure.rs:276` (`default_dump_dir`); `.gitignore:115` | default | write |
| `dev/benchmarks/dvb_t2_awgn/plot.py` | `crates/gf2-sim/src/bin/dvb_t2_awgn_campaign.rs:713` | help text | text |
| `dev/benchmarks/permanent[-backend-selection]/test-receipt.json`, `dev/benchmarks/permanent/backend.json` | `crates/gf2-sim/src/permanent_campaign/{schedule.rs:2512,fixture.rs:114,coordinator_tests.rs:53}`; `tests/permanent_campaign_bin.rs:49` | fixture literals | identity (test fixtures) |
| `dev/benchmarks` | `.jit/config.toml:163` | `managed_paths` | config |

## dev/bench_results

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/bench_results/` prefix | `dev/scripts/check-receipt-input-snapshots.py:136`; `dev/scripts/check-addendum-schema-versions.py:181,198` | `path.startswith("dev/bench_results/")` receipt enumeration | identity (CI) |
| `dev/bench_results/c077a88b/2026-09-08-*/receipt.json` (+ snapshot paths) | `dev/scripts/receipt-input-omissions.json:7-8,17-18,27-28,37-38,47-48` | `(receipt, path)` tuples; stale entries fail CI | identity |
| `dev/bench_results/26465e6c/v3-and-popcnt-pilot` | `dev/tools/.../tests/protocol_contracts.rs:3906` | `evaluate_version(&dir, Some(3))` | read |
| `dev/bench_results/f547c394/...` | `dev/tools/.../tests/protocol_contracts.rs:336,453-454,476,492,996,1761-1765,2482,2664,3601` | fixture literals | read/identity (fixtures) |
| `dev/bench_results/f547c394/run-smoke-v3.sh` | `dev/active/f547c394/producing-inputs.json` | closure entry | identity |
| `dev/bench_results/e0251af3-rank-event-rates.md` | `crates/gf2-algebra/tests/rank_event_rates.rs:20,292` | `RECEIPT_FILE` | write |
| `dev/bench_results/{88ca7d2f,4e732b56}` | `dev/active/fd9d5416/render_receipt.py:550-551`, `tests/test_render_receipt.py:115-116,150`; `dev/active/6fb89a3c/survey/verify-bit-mapping.py:26` | defaults | read |
| `dev/bench_results` | `.jit/config.toml:163,165` | config | config |

## dev/reference_data

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/reference_data/osd_ebch_128_64_fossorier1994.md` | `crates/gf2-sim/src/bin/ebch_osd_awgn_campaign.rs:449` | `artifact_identity(&repository, ...)` | identity (recorded in receipts) |
| same | `crates/gf2-coding/tests/ebch_128_64_reference.rs:135`; `tests/data/ebch_128_64_reference.json:23` | `assert_eq!` | identity |
| `dev/reference_data/tuning-profiles/{conservative,gf2-a83583e0-...}.json` | `crates/gf2-algebra/tests/tuning_repository_envelopes.rs:12,18`; `dev/tools/tuning-profile-compose/src/main.rs:545` | `include_str!` | include |
| `dev/reference_data/tuning-profiles/{campaign}.json` | `dev/tools/.../tuning-extent-campaign-driver.rs:2782`; `dev/scripts/validate-tuning-extent-campaign.py:4202` | publication destination | derived identity |
| `dev/reference_data/fig_gldpc_sogrand.csv`, `dev/reference_data/scripts/compare_results.py` | `dev/scripts/gen_fig7_report.sh:6,19,25` | vars, `python3 ...` | read |
| `dev/reference_data` | `docs/presentations/figures/generate_grand_comparison_plots.py:32` | `REPO / "dev" / "reference_data"` | read |

## dev/campaigns

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/campaigns/phase1_fig{1,3}.toml` (+ absence of `phase1_fig1_highstat`, `phase1_fig3_sogrand`) | `crates/gf2-coding/tests/grand_phase1_smoke.rs:409-412,503` | `is_file()`/`!exists()` asserts; report text `contains` | identity |
| `dev/campaigns/osd-reference.json` | `crates/gf2-sim/tests/osd_campaign_protocol.rs:64` | fixture literal | identity (fixture) |
| `dev/campaigns/*.toml` | `.claude/settings.local.json:130-134` | permission strings | config |

## dev/scripts

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/scripts/{validate-tuning-extent-campaign,check-campaign-producing-closure,check-receipt-input-snapshots,check-addendum-schema-versions}.py`, `tuning-extent-campaign.sh` | `scripts/cargo-ci.sh:237-244` | `run_step ... python3 dev/scripts/...` | CI |
| `dev/scripts/validate-tuning-extent-campaign.py` | `dev/tools/.../tuning-extent-campaign-driver.rs:1065` | `artifact(&repository.join(...))` | identity (validator digest in receipts) |
| same | `dev/tools/.../tuning-extent-campaign-driver.rs:4124,4680,4805,4934`; `crates/gf2-core/benches/tuning_calibration.rs:7113` | `CARGO_MANIFEST_DIR/../../scripts/...` | read |
| same | `dev/scripts/validate-tuning-extent-campaign.py:4126` | `behavior_sha256` key must equal this path | identity (vs producing-build-inputs manifests) |
| `dev/scripts/tuning-extent-campaign.sh` | `dev/tools/.../tests/driver_launcher.rs:94,148,198,245`; `driver.rs:3820` | `include_str!("../../../scripts/...")`, read | include |
| `dev/scripts/ccx1-bench-flock.sh` | `dev/tools/.../tests/protocol_contracts.rs:355,698,1022`; `dev/scripts/campaign_plan.py:122`; `dev/benchmarks/permanent_campaign/determinant_cost_v5.py:35` + tests; `dev/active/f547c394/producing-inputs.json`; receipts (451 pins) | `wrapper` literal | identity |
| `dev/scripts/ppc-baselines.json` | `dev/scripts/ppc-compare.sh:92` | default `MANIFEST` | read |
| `dev/scripts` | `dev/scripts/{campaign_tables,campaign_inputs,campaign_plan}.py:9-12` | `sys.path.insert` | read |
| `dev/scripts/receipt-input-omissions.json` | `dev/scripts/check-receipt-input-snapshots.py:44` | `OMISSIONS` | read |

## dev/tools

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/tools/tuning-campaign-support` | `Cargo.toml:9` (member); `crates/gf2-core/Cargo.toml:32`, `crates/gf2-algebra/Cargo.toml:36` (`path = "../../dev/tools/..."`); 13 `dev/active/*/survey/*/Cargo.toml` | path deps | build |
| same | `dev/scripts/check-campaign-producing-closure.py:34`; `dev/active/f547c394/producing-inputs.json`; receipts (6861 pins) | `CRATE`, closure entries | identity |
| `dev/tools/tuning-campaign-support/src/receipt.rs` | `dev/tools/.../tests/protocol_contracts.rs:2127,2253` | `read_to_string` | read |
| `dev/tools/tuning-profile-compose` | `scripts/cargo-ci.sh:267`; `dev/scripts/tuning-extent-campaign.sh:131` | `--manifest-path` | CI |
| same | `dev/tools/tuning-profile-compose/src/main.rs:21` (`TOOL_PATH`); profile JSON `assembly.tool` (6 files); `validate-tuning-extent-campaign.py:3411` | const vs `tool` field | identity |

## dev/research

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `dev/research/{f3_bipedal,permanent_gpu_crossover}` | `Cargo.toml:21-22` | workspace `exclude` | build |
| `dev/research/f3_bipedal` | `crates/gf2-kernels-simd/Cargo.toml:34` | path dev-dep | build |
| `dev/research/{permanent_gpu_crossover,permanent_gpu_speedup,perm_uniformity,perm_uniformity_gpu}/Cargo.toml` | `scripts/permanent-repro.sh:163,167,367-401`; `scripts/perm-uniformity-repro.sh:42`; `scripts/perm-uniformity-gpu-repro.sh:58` | `--manifest-path` | read |
| `dev/research` | `.github/workflows/ci.yml:178`, `ci-slow.yml:80` | `find crates dev/research -name Cargo.toml` | CI |
| `dev/research/{f3_bipedal,permanent-sampling-feas,permanent_wave_gpu}` | `.jit/gates.toml:195,324,344` | gate commands | config |

## dev/plans, dev/presentations, dev/sessions

Only `text` or config consumers: `scripts/asm-artefact-present.sh:104` (echo
`dev/plans/gf2_core_ppc_spiral.md`), `.jit/config.toml:163,165`, Lean doc
comments in `proofs/`.

## docs/

| Target | Consumer | Construct | Mode |
| --- | --- | --- | --- |
| `docs/presentations/figures` | `crates/gf2-coding/examples/gen_presentation_figures.rs:39` | `workspace_root().join("docs/presentations/figures")` | write |
| `docs/presentations/*/talk.html` | self-contained; `ae82bd73` links `../../../dev/benchmarks/gf2_algebra_permanent/README.md` | relative href | link |
| `docs` | `.jit/config.toml:164` | `permanent_paths` | config |
| `docs/lean4-verification-pipeline.md` | none in code (md mentions in `proofs/README.md`, `proofs/WORKAROUNDS.md`) | | |

## crates/*/docs

No non-comment consumer found: no `include_str!`, `#[doc = include_str!]`,
Cargo `readme`, CI step, or script references `crates/gf2-core/docs` or
`crates/gf2-coding/docs`.

## Other config consumers

- `.jit/config.toml:162-165`: `archive_root`, `managed_paths`, `permanent_paths`, `issue_scoped_areas`.
- `.jit/gates.toml`: `contrib/gates/*.sh`, `scripts/*.sh`, `dev/research/*` commands.
- `packages/sim-research/manifest.toml:581`: `doc_area = "dev/active"`.
- `.claude/settings.local.json:104-138`: permission strings naming `dev/simulation_results`, `dev/campaigns`.
- `.gitignore:91,115`.
